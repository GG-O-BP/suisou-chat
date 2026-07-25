mod policy;
mod response;
mod runtime_credentials;
mod runtime_research;
mod stream;
mod transport;

use crate::credentials::{ApiKeyStore, SystemApiKeyStore, UnavailableApiKeyStore};
use crate::models::{validate_research_request, ConnectionInfo, ResearchRequest, ResearchResponse};
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::WebviewWindow;
use tokio_util::sync::CancellationToken;
use zeroize::Zeroizing;

use policy::{instructions, output_limit};
use response::{extract_answer, extract_sources, extract_usage};
use stream::consume_stream;
use transport::{
    cancelled, emit, http_error, key_verification_network_error, network_error, normalize_key,
    valid_key,
};

const API_ROOT: &str = "https://api.sakana.ai/v1";
const RESEARCH_EVENT: &str = "research-event";
const KEY_VERIFICATION_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_SSE_FRAME_BYTES: usize = 1024 * 1024;
const MAX_ANSWER_BYTES: usize = 4 * 1024 * 1024;
const CREDENTIAL_TASK_FAILED: &str =
    "API 키 보안 저장소 작업이 예기치 않게 중단되었습니다. 앱을 다시 시작한 뒤 시도해 주세요.";

pub struct FuguRuntime {
    client: Client,
    api_key: Mutex<Option<Zeroizing<String>>>,
    api_key_store: Arc<dyn ApiKeyStore>,
    key_update: Mutex<()>,
    credential_notice: Mutex<Option<String>>,
    active: Mutex<HashMap<String, CancellationToken>>,
}

#[cfg(test)]
use stream::{parse_sse_frame, take_sse_frame};

#[cfg(test)]
mod tests;
