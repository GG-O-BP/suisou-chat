use reqwest::StatusCode;
use serde_json::Value;
use zeroize::Zeroizing;

pub(super) fn cancelled<T>() -> Result<T, String> {
    Err("요청이 중단되었습니다.".into())
}

pub(super) fn network_error(error: reqwest::Error) -> String {
    if error.is_timeout() {
        "응답 대기 시간이 초과되었습니다. Fugu Ultra는 오래 걸릴 수 있으니 다시 시도해 주세요."
            .into()
    } else if error.is_connect() {
        "Sakana API에 연결할 수 없습니다. 네트워크 상태를 확인해 주세요.".into()
    } else {
        "네트워크 응답을 처리하지 못했습니다. 잠시 후 다시 시도해 주세요.".into()
    }
}

pub(super) fn key_verification_network_error(error: reqwest::Error) -> String {
    if error.is_timeout() {
        "API 키 확인 시간이 초과되었습니다. 네트워크 상태를 확인한 뒤 다시 시도해 주세요.".into()
    } else {
        network_error(error)
    }
}

pub(super) async fn http_error(response: reqwest::Response) -> String {
    let status = response.status();
    let retry_after = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let remote_message = if response.content_length().unwrap_or(0) <= 16_384 {
        response.bytes().await.ok()
    } else {
        None
    }
    .and_then(|bytes| {
        (bytes.len() <= 16_384)
            .then(|| serde_json::from_slice::<Value>(&bytes).ok())
            .flatten()
    })
    .and_then(|value| {
        value
            .pointer("/error/message")
            .or_else(|| value.get("message"))
            .and_then(Value::as_str)
            .map(clean_remote_error)
    });

    match status {
        StatusCode::UNAUTHORIZED => {
            "API 키가 유효하지 않습니다. Sakana Console에서 새 키를 확인해 주세요.".into()
        }
        StatusCode::FORBIDDEN => "이 계정에는 선택한 Fugu 모델 또는 기능의 권한이 없습니다.".into(),
        StatusCode::TOO_MANY_REQUESTS => retry_after
            .map(|seconds| format!("요청 한도에 도달했습니다. {seconds}초 후 다시 시도해 주세요."))
            .unwrap_or_else(|| "요청 한도에 도달했습니다. 잠시 후 다시 시도해 주세요.".into()),
        StatusCode::BAD_REQUEST => {
            remote_message.unwrap_or_else(|| "요청 형식을 Sakana API가 거부했습니다.".into())
        }
        status if status.is_server_error() => {
            "Sakana API가 일시적으로 불안정합니다. 잠시 후 다시 시도해 주세요.".into()
        }
        _ => {
            remote_message.unwrap_or_else(|| format!("Sakana API 요청이 실패했습니다 ({status})."))
        }
    }
}

pub(super) fn clean_remote_error(message: &str) -> String {
    truncate_chars(message.trim(), 300)
}

pub(super) fn normalize_key(key: String) -> Result<Zeroizing<String>, String> {
    let key = Zeroizing::new(key);
    let trimmed = key.trim();
    if !valid_key(trimmed) {
        return Err("API 키 형식이 올바르지 않습니다.".into());
    }
    Ok(Zeroizing::new(trimmed.to_owned()))
}

pub(super) fn valid_key(key: &str) -> bool {
    (12..=512).contains(&key.len()) && !key.chars().any(char::is_whitespace)
}

pub(super) fn truncate_chars(value: &str, maximum: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(maximum).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}
