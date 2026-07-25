use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

struct MemoryApiKeyStore {
    value: Mutex<Option<String>>,
    fail_load: AtomicBool,
    fail_save: AtomicBool,
    fail_delete: AtomicBool,
}

struct RuntimeStartingApiKeyStore {
    value: Mutex<Option<String>>,
}

impl RuntimeStartingApiKeyStore {
    fn run_nested_runtime() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {});
    }
}

impl ApiKeyStore for RuntimeStartingApiKeyStore {
    fn load(&self) -> Result<Option<Zeroizing<String>>, String> {
        Ok(self.value.lock().unwrap().clone().map(Zeroizing::new))
    }

    fn save(&self, api_key: &str) -> Result<(), String> {
        Self::run_nested_runtime();
        *self.value.lock().unwrap() = Some(api_key.to_owned());
        Ok(())
    }

    fn delete(&self) -> Result<(), String> {
        Self::run_nested_runtime();
        *self.value.lock().unwrap() = None;
        Ok(())
    }
}

impl MemoryApiKeyStore {
    fn new(value: Option<&str>) -> Self {
        Self {
            value: Mutex::new(value.map(ToOwned::to_owned)),
            fail_load: AtomicBool::new(false),
            fail_save: AtomicBool::new(false),
            fail_delete: AtomicBool::new(false),
        }
    }

    fn value(&self) -> Option<String> {
        self.value.lock().unwrap().clone()
    }
}

impl ApiKeyStore for MemoryApiKeyStore {
    fn load(&self) -> Result<Option<Zeroizing<String>>, String> {
        if self.fail_load.load(Ordering::SeqCst) {
            return Err("load failed".into());
        }
        Ok(self.value().map(Zeroizing::new))
    }

    fn save(&self, api_key: &str) -> Result<(), String> {
        if self.fail_save.load(Ordering::SeqCst) {
            return Err("save failed".into());
        }
        *self.value.lock().unwrap() = Some(api_key.to_owned());
        Ok(())
    }

    fn delete(&self) -> Result<(), String> {
        if self.fail_delete.load(Ordering::SeqCst) {
            return Err("delete failed".into());
        }
        *self.value.lock().unwrap() = None;
        Ok(())
    }
}

#[test]
fn sse_parser_preserves_split_utf8_chunks() {
    let payload = "event: response.output_text.delta\ndata: {\"delta\":\"수조 속 답변\"}\n\n";
    let bytes = payload.as_bytes();
    let split = payload.find('속').unwrap() + 1;
    let mut buffer = bytes[..split].to_vec();
    assert!(take_sse_frame(&mut buffer).is_none());
    buffer.extend_from_slice(&bytes[split..]);
    let frame = take_sse_frame(&mut buffer).unwrap();
    let (event, data) = parse_sse_frame(&frame).unwrap();
    assert_eq!(event, "response.output_text.delta");
    assert!(data.contains("수조 속 답변"));
}

#[test]
fn extracts_answer_citations_and_usage() {
    let value = json!({
        "output": [{
            "type": "message",
            "content": [{
                "type": "output_text",
                "text": "검증된 답변",
                "annotations": [
                    {"type":"url_citation", "title":"공식 자료", "url":"https://www.example.com/report#part"},
                    {"type":"url_citation", "title":"중복", "url":"https://www.example.com/report"},
                    {"type":"url_citation", "title":"차단", "url":"http://unsafe.example"}
                ]
            }]
        }],
        "usage": {"input_tokens": 10, "output_tokens": 20, "total_tokens": 30}
    });
    assert_eq!(extract_answer(&value), "검증된 답변");
    let sources = extract_sources(&value);
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].domain, "example.com");
    assert_eq!(extract_usage(&value).unwrap().total_tokens, 30);
}

#[test]
fn accepts_crlf_sse_frames() {
    let mut buffer = b"event: done\r\ndata: {}\r\n\r\nnext".to_vec();
    let frame = take_sse_frame(&mut buffer).unwrap();
    assert_eq!(parse_sse_frame(&frame).unwrap().0, "done");
    assert_eq!(buffer, b"next");
}

#[test]
fn stream_limits_are_large_enough_for_research_but_bounded() {
    const {
        assert!(MAX_SSE_FRAME_BYTES >= 256 * 1024);
        assert!(MAX_ANSWER_BYTES >= MAX_SSE_FRAME_BYTES);
        assert!(MAX_RESPONSE_BYTES >= MAX_ANSWER_BYTES);
    }
}

#[test]
fn creative_mode_has_dedicated_guidance_and_room_for_long_form_work() {
    assert!(instructions("create").contains("creative collaborator"));
    assert_eq!(output_limit("create"), 8_000);
}

#[test]
fn validates_key_without_assuming_a_prefix() {
    assert!(normalize_key("future-key-format-123".into()).is_ok());
    assert!(normalize_key("contains whitespace".into()).is_err());
    assert!(normalize_key("short".into()).is_err());
}

#[test]
fn restores_a_valid_key_from_secure_storage() {
    let store = Arc::new(MemoryApiKeyStore::new(Some("persisted-key-123")));
    let runtime = FuguRuntime::new_with_store(store, None).unwrap();

    assert!(runtime.has_key());
    assert_eq!(runtime.key().unwrap().as_str(), "persisted-key-123");
    assert_eq!(runtime.credential_notice(), None);
}

#[test]
fn rejects_and_removes_an_invalid_persisted_key() {
    let store = Arc::new(MemoryApiKeyStore::new(Some("bad key")));
    let runtime = FuguRuntime::new_with_store(store.clone(), None).unwrap();

    assert!(!runtime.has_key());
    assert_eq!(store.value(), None);
    assert!(runtime
        .credential_notice()
        .unwrap()
        .contains("형식이 올바르지 않아"));
}

#[test]
fn save_failure_does_not_replace_the_active_key() {
    let store = Arc::new(MemoryApiKeyStore::new(Some("previous-key-123")));
    let runtime = FuguRuntime::new_with_store(store.clone(), None).unwrap();
    store.fail_save.store(true, Ordering::SeqCst);

    assert_eq!(
        runtime
            .store_key(Zeroizing::new("replacement-key-456".into()))
            .unwrap_err(),
        "save failed"
    );
    assert_eq!(runtime.key().unwrap().as_str(), "previous-key-123");
    assert_eq!(store.value().as_deref(), Some("previous-key-123"));
}

#[test]
fn clear_removes_memory_even_when_secure_delete_fails() {
    let store = Arc::new(MemoryApiKeyStore::new(Some("persisted-key-123")));
    let runtime = FuguRuntime::new_with_store(store.clone(), None).unwrap();
    store.fail_delete.store(true, Ordering::SeqCst);

    assert_eq!(runtime.clear_key_blocking().unwrap_err(), "delete failed");
    assert!(!runtime.has_key());
    assert_eq!(store.value().as_deref(), Some("persisted-key-123"));
    assert_eq!(
        runtime.credential_notice().as_deref(),
        Some("delete failed")
    );
}

#[test]
fn load_failure_is_nonfatal_but_reported() {
    let store = Arc::new(MemoryApiKeyStore::new(None));
    store.fail_load.store(true, Ordering::SeqCst);

    let runtime = FuguRuntime::new_with_store(store, None).unwrap();

    assert!(!runtime.has_key());
    assert_eq!(runtime.credential_notice().as_deref(), Some("load failed"));
}

#[test]
fn secure_store_round_trip_restores_the_key() {
    let store = Arc::new(MemoryApiKeyStore::new(None));
    let first_runtime = FuguRuntime::new_with_store(store.clone(), None).unwrap();
    first_runtime
        .store_key(Zeroizing::new("round-trip-key-123".into()))
        .unwrap();
    drop(first_runtime);

    let restored_runtime = FuguRuntime::new_with_store(store.clone(), None).unwrap();
    assert_eq!(
        restored_runtime.key().unwrap().as_str(),
        "round-trip-key-123"
    );
    restored_runtime.clear_key_blocking().unwrap();
    drop(restored_runtime);

    let cleared_runtime = FuguRuntime::new_with_store(store, None).unwrap();
    assert!(!cleared_runtime.has_key());
}

#[tokio::test(flavor = "multi_thread")]
async fn secure_store_operations_do_not_nest_tokio_runtimes() {
    let store = Arc::new(RuntimeStartingApiKeyStore {
        value: Mutex::new(None),
    });
    let runtime = Arc::new(FuguRuntime::new_with_store(store.clone(), None).unwrap());

    runtime
        .store_key_async(Zeroizing::new("runtime-safe-key-123".into()))
        .await
        .unwrap();
    assert_eq!(
        store.value.lock().unwrap().as_deref(),
        Some("runtime-safe-key-123")
    );

    runtime.clear_key().await.unwrap();
    assert_eq!(*store.value.lock().unwrap(), None);
    assert!(!runtime.has_key());
}
