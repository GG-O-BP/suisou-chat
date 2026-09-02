use crate::models::Provider;
use reqwest::StatusCode;
use serde_json::Value;
use zeroize::Zeroizing;

pub(super) fn cancelled<T>() -> Result<T, String> {
    Err("요청이 중단되었습니다.".into())
}

pub(super) fn network_error(error: reqwest::Error, provider: Provider) -> String {
    if error.is_timeout() {
        format!(
            "응답 대기 시간이 초과되었습니다. {}는 오래 걸릴 수 있으니 다시 시도해 주세요.",
            provider.label()
        )
    } else if error.is_connect() {
        format!(
            "{} API에 연결할 수 없습니다. 네트워크 상태를 확인해 주세요.",
            provider.label()
        )
    } else {
        "네트워크 응답을 처리하지 못했습니다. 잠시 후 다시 시도해 주세요.".into()
    }
}

pub(super) fn key_verification_network_error(error: reqwest::Error) -> String {
    if error.is_timeout() {
        "API 키 확인 시간이 초과되었습니다. 네트워크 상태를 확인한 뒤 다시 시도해 주세요.".into()
    } else {
        network_error(error, Provider::Sakana)
    }
}

pub(super) async fn http_error(response: reqwest::Response, provider: Provider) -> String {
    let status = response.status();
    let retry_after = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let remote_error = if response.content_length().unwrap_or(0) <= 16_384 {
        response.bytes().await.ok()
    } else {
        None
    }
    .and_then(|bytes| {
        (bytes.len() <= 16_384)
            .then(|| serde_json::from_slice::<Value>(&bytes).ok())
            .flatten()
    })
    .map(|value| parse_remote_error(&value));
    let remote_message = remote_error
        .as_ref()
        .and_then(|error| error.message.clone());

    match status {
        StatusCode::UNAUTHORIZED => {
            format!(
                "API 키가 유효하지 않습니다. {} Console에서 새 키를 확인해 주세요.",
                provider.label()
            )
        }
        StatusCode::FORBIDDEN => format!(
            "이 계정에는 선택한 {} 모델 또는 기능의 권한이 없습니다.",
            provider.label()
        ),
        StatusCode::TOO_MANY_REQUESTS => {
            too_many_requests_message(retry_after.as_deref(), provider, remote_error.as_ref())
        }
        StatusCode::BAD_REQUEST => remote_message
            .unwrap_or_else(|| format!("요청 형식을 {} API가 거부했습니다.", provider.label())),
        status if status.is_server_error() => {
            format!(
                "{} API가 일시적으로 불안정합니다. 잠시 후 다시 시도해 주세요.",
                provider.label()
            )
        }
        _ => remote_message
            .unwrap_or_else(|| format!("{} API 요청이 실패했습니다 ({status}).", provider.label())),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RemoteError {
    code: Option<String>,
    message: Option<String>,
}

fn parse_remote_error(value: &Value) -> RemoteError {
    let error = value.get("error").filter(|error| !error.is_null());
    RemoteError {
        code: error
            .and_then(|error| error.get("code"))
            .and_then(json_scalar),
        message: error
            .and_then(|error| error.get("message"))
            .or_else(|| value.get("message"))
            .and_then(Value::as_str)
            .map(clean_remote_error),
    }
}

fn json_scalar(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn too_many_requests_message(
    retry_after: Option<&str>,
    provider: Provider,
    remote_error: Option<&RemoteError>,
) -> String {
    if provider == Provider::Zai
        && remote_error
            .and_then(|error| error.code.as_deref())
            .is_some_and(|code| code == "1113")
    {
        return "Z.ai GLM Coding Plan 잔액 또는 리소스 패키지가 부족합니다. Z.ai 콘솔에서 구독과 결제 상태를 확인한 뒤 다시 시도해 주세요.".into();
    }

    let basis = retry_after.map_or_else(
        || "요청 한도에 도달했습니다. 잠시 후 다시 시도해 주세요.".to_owned(),
        |seconds| format!("요청 한도에 도달했습니다. {seconds}초 후 다시 시도해 주세요."),
    );
    remote_error
        .and_then(|error| error.message.clone())
        .map(|message| format!("{basis} — {message}"))
        .unwrap_or(basis)
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_string_and_numeric_remote_error_codes() {
        let string_code = parse_remote_error(&json!({
            "error": {
                "code": "1113",
                "message": "Insufficient balance or no resource package."
            }
        }));
        assert_eq!(string_code.code.as_deref(), Some("1113"));
        assert_eq!(
            string_code.message.as_deref(),
            Some("Insufficient balance or no resource package.")
        );

        let numeric_code = parse_remote_error(&json!({
            "error": {"code": 1302, "message": "Rate limit reached for requests"}
        }));
        assert_eq!(numeric_code.code.as_deref(), Some("1302"));
    }

    #[test]
    fn distinguishes_zai_balance_errors_from_request_rate_limits() {
        let balance = parse_remote_error(&json!({
            "error": {
                "code": "1113",
                "message": "Insufficient balance or no resource package. Please recharge."
            }
        }));
        let balance_message = too_many_requests_message(None, Provider::Zai, Some(&balance));
        assert!(balance_message.contains("Coding Plan 잔액 또는 리소스 패키지"));
        assert!(balance_message.contains("구독과 결제 상태"));
        assert!(!balance_message.contains("요청 한도"));

        let rate_limit = RemoteError {
            code: Some("1302".into()),
            message: Some("Rate limit reached for requests".into()),
        };
        let rate_limit_message =
            too_many_requests_message(Some("30"), Provider::Zai, Some(&rate_limit));
        assert!(rate_limit_message.contains("요청 한도"));
        assert!(rate_limit_message.contains("30초"));
        assert!(rate_limit_message.contains("Rate limit reached"));
    }
}
