use super::*;
use crate::models::InputMessage;
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
fn reports_incomplete_stream_reasons_without_discarding_partial_output() {
    let token_limit = json!({
        "response": {
            "incomplete_details": {
                "reason": "max_output_tokens"
            }
        }
    });
    assert!(incomplete_message(&token_limit).contains("출력 토큰 한도"));

    let filtered = json!({
        "incomplete_details": {
            "reason": "content_filter"
        }
    });
    assert!(incomplete_message(&filtered).contains("안전 정책"));
    assert!(incomplete_message(&json!({})).contains("완료하지 못했습니다"));
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
fn modes_have_dedicated_guidance_and_threefold_output_budgets() {
    assert!(instructions("create", Provider::Sakana).contains("creative collaborator"));
    assert!(instructions("create", Provider::Zai).contains("Z.ai GLM"));
    assert_eq!(output_limit("quick"), 9_000);
    assert_eq!(output_limit("search"), 18_000);
    assert_eq!(output_limit("deep"), 36_000);
    assert_eq!(output_limit("create"), 24_000);
}

#[test]
fn builds_a_glm_chat_completions_request_with_the_official_model_id() {
    let request = ResearchRequest {
        request_id: "request-glm".into(),
        model: "glm-5.3".into(),
        mode: "search".into(),
        reasoning: "xhigh".into(),
        messages: vec![InputMessage {
            role: "user".into(),
            content: "검증해 줘".into(),
        }],
    };
    let body = zai::request_body(&request);

    assert_eq!(body["model"], "glm-5.3");
    assert_eq!(body["reasoning_effort"], "high");
    assert_eq!(body["thinking"]["type"], "enabled");
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["tools"][0]["type"], "web_search");
    assert_eq!(body["tools"][0]["web_search"]["enable"], true);
}

#[test]
fn consumes_glm_stream_deltas_usage_and_web_search_sources() {
    let request = ResearchRequest {
        request_id: "request-glm-stream".into(),
        model: "glm-5.3".into(),
        mode: "search".into(),
        reasoning: "high".into(),
        messages: vec![InputMessage {
            role: "user".into(),
            content: "검증해 줘".into(),
        }],
    };
    let mut state = zai::GlmStreamState {
        answer: String::new(),
        metadata: json!({}),
        saw_finish: false,
        writing_started: false,
        last_output_at: None,
    };
    let mut stages = Vec::new();
    let mut emit = |kind: &str, value: &str| {
        stages.push((kind.to_owned(), value.to_owned()));
    };
    let mut noop = |_: &str, _: &str| {};
    let delta = json!({
        "choices": [{"index": 0, "delta": {"content": "검증된 답변"}, "finish_reason": null}]
    });
    zai::handle_frame(
        format!("data: {delta}\n\n").as_bytes(),
        &mut state,
        &mut emit,
    )
    .unwrap();
    let final_chunk = json!({
        "choices": [{"index": 0, "delta": {"content": ""}, "finish_reason": "stop"}],
        "usage": {"prompt_tokens": 11, "completion_tokens": 22, "total_tokens": 33, "completion_tokens_details": {"reasoning_tokens": 4}},
        "web_search": [
            {"title": "공식 자료", "link": "https://www.example.com/report#part", "content": "요약"},
            {"title": "userinfo", "link": "https://user:pass@example.com/secret", "content": "차단"},
            {"title": "http", "link": "http://example.com", "content": "차단"}
        ]
    });
    zai::handle_frame(
        format!("data: {final_chunk}\n\n").as_bytes(),
        &mut state,
        &mut noop,
    )
    .unwrap();
    let done = b"data: [DONE]\n\n";
    match zai::handle_frame(done, &mut state, &mut noop).unwrap() {
        zai::FrameOutcome::Done => {}
        zai::FrameOutcome::Continue => panic!("expected [DONE] to complete the GLM stream"),
    }

    let response = zai::finish_stream(&request, &state).unwrap();
    assert_eq!(response.answer, "검증된 답변");
    assert_eq!(response.sources.len(), 1);
    assert_eq!(response.sources[0].domain, "example.com");
    let usage = response.usage.unwrap();
    assert_eq!(
        (usage.input_tokens, usage.output_tokens, usage.total_tokens),
        (11, 22, 33)
    );
    assert_eq!(usage.orchestration_tokens, 4);
    assert!(stages.contains(&("stage".to_owned(), "writing".to_owned())));
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

    assert!(runtime.has_key(Provider::Sakana));
    assert_eq!(
        runtime.key(Provider::Sakana).unwrap().as_str(),
        "persisted-key-123"
    );
    assert_eq!(runtime.credential_notice(), None);
}

#[test]
fn rejects_and_removes_an_invalid_persisted_key() {
    let store = Arc::new(MemoryApiKeyStore::new(Some("bad key")));
    let runtime = FuguRuntime::new_with_store(store.clone(), None).unwrap();

    assert!(!runtime.has_key(Provider::Sakana));
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
            .store_key(
                Provider::Sakana,
                Zeroizing::new("replacement-key-456".into())
            )
            .unwrap_err(),
        "save failed"
    );
    assert_eq!(
        runtime.key(Provider::Sakana).unwrap().as_str(),
        "previous-key-123"
    );
    assert_eq!(store.value().as_deref(), Some("previous-key-123"));
}

#[test]
fn clear_removes_memory_even_when_secure_delete_fails() {
    let store = Arc::new(MemoryApiKeyStore::new(Some("persisted-key-123")));
    let runtime = FuguRuntime::new_with_store(store.clone(), None).unwrap();
    store.fail_delete.store(true, Ordering::SeqCst);

    assert_eq!(
        runtime.clear_key_blocking(Provider::Sakana).unwrap_err(),
        "delete failed"
    );
    assert!(!runtime.has_key(Provider::Sakana));
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

    assert!(!runtime.has_key(Provider::Sakana));
    assert_eq!(runtime.credential_notice().as_deref(), Some("load failed"));
}

#[test]
fn secure_store_round_trip_restores_the_key() {
    let store = Arc::new(MemoryApiKeyStore::new(None));
    let first_runtime = FuguRuntime::new_with_store(store.clone(), None).unwrap();
    first_runtime
        .store_key(
            Provider::Sakana,
            Zeroizing::new("round-trip-key-123".into()),
        )
        .unwrap();
    drop(first_runtime);

    let restored_runtime = FuguRuntime::new_with_store(store.clone(), None).unwrap();
    assert_eq!(
        restored_runtime.key(Provider::Sakana).unwrap().as_str(),
        "round-trip-key-123"
    );
    restored_runtime
        .clear_key_blocking(Provider::Sakana)
        .unwrap();
    drop(restored_runtime);

    let cleared_runtime = FuguRuntime::new_with_store(store, None).unwrap();
    assert!(!cleared_runtime.has_key(Provider::Sakana));
}

#[test]
fn sakana_and_zai_keys_are_restored_and_cleared_independently() {
    let sakana_store = Arc::new(MemoryApiKeyStore::new(Some("sakana-key-123")));
    let zai_store = Arc::new(MemoryApiKeyStore::new(None));
    let runtime = FuguRuntime::new_with_stores(
        Arc::clone(&sakana_store) as Arc<dyn ApiKeyStore>,
        Arc::clone(&zai_store) as Arc<dyn ApiKeyStore>,
        None,
        None,
    )
    .unwrap();

    assert!(runtime.has_key(Provider::Sakana));
    assert!(!runtime.has_key(Provider::Zai));
    runtime
        .store_key(Provider::Zai, Zeroizing::new("standard-zai-key-456".into()))
        .unwrap();
    assert_eq!(
        runtime.key(Provider::Zai).unwrap().as_str(),
        "standard-zai-key-456"
    );
    assert_eq!(sakana_store.value().as_deref(), Some("sakana-key-123"));
    assert_eq!(zai_store.value().as_deref(), Some("standard-zai-key-456"));

    runtime.clear_key_blocking(Provider::Zai).unwrap();
    assert!(runtime.has_key(Provider::Sakana));
    assert!(!runtime.has_key(Provider::Zai));
    assert!(zai_store.value().is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn secure_store_operations_do_not_nest_tokio_runtimes() {
    let store = Arc::new(RuntimeStartingApiKeyStore {
        value: Mutex::new(None),
    });
    let runtime = Arc::new(FuguRuntime::new_with_store(store.clone(), None).unwrap());

    runtime
        .store_key_async(
            Provider::Sakana,
            Zeroizing::new("runtime-safe-key-123".into()),
        )
        .await
        .unwrap();
    assert_eq!(
        store.value.lock().unwrap().as_deref(),
        Some("runtime-safe-key-123")
    );

    runtime.clear_key(Provider::Sakana).await.unwrap();
    assert_eq!(*store.value.lock().unwrap(), None);
    assert!(!runtime.has_key(Provider::Sakana));
}
