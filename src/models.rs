use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Workspace {
    pub version: u32,
    pub revision: u64,
    pub conversations: Vec<Conversation>,
    pub settings: Settings,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub model: String,
    pub reasoning: String,
    pub theme: String,
    pub last_mode: String,
    pub language: String,
    pub sync_mode: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            model: "fugu".into(),
            reasoning: "high".into(),
            theme: "system".into(),
            last_mode: "search".into(),
            language: "auto".into(),
            sync_mode: "local".into(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub pinned: bool,
    pub created_at: u64,
    pub updated_at: u64,
    pub messages: Vec<Message>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Message {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created_at: u64,
    pub status: String,
    pub sources: Vec<Source>,
    pub usage: Option<Usage>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Source {
    pub id: String,
    pub title: String,
    pub url: String,
    pub domain: String,
    pub snippet: String,
    pub retrieved_at: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub orchestration_tokens: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BootstrapResponse {
    pub workspace: Workspace,
    pub key_configured: bool,
    pub credential_notice: Option<String>,
    pub recovery_notice: Option<String>,
    pub storage_label: String,
    pub storage_writable: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct InputMessage {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResearchRequest {
    pub request_id: String,
    pub model: String,
    pub mode: String,
    pub reasoning: String,
    pub messages: Vec<InputMessage>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ResearchResponse {
    pub answer: String,
    pub sources: Vec<Source>,
    pub usage: Option<Usage>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConnectionInfo {
    pub message: String,
    pub models: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ResearchEvent {
    pub request_id: String,
    pub kind: String,
    pub value: String,
}

pub fn new_id(prefix: &str) -> String {
    let timestamp = js_sys::Date::now() as u64;
    let random = (js_sys::Math::random() * 1_000_000_000.0) as u64;
    format!("{prefix}-{timestamp}-{random}")
}

pub fn now_millis() -> u64 {
    js_sys::Date::now() as u64
}

pub fn title_from_question(question: &str) -> String {
    let normalized = question.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = normalized.chars();
    let title = chars.by_ref().take(42).collect::<String>();
    if chars.next().is_some() {
        format!("{title}…")
    } else if title.is_empty() {
        "새로운 탐구".into()
    } else {
        title
    }
}

pub fn format_relative_time(timestamp: u64) -> String {
    let elapsed = now_millis().saturating_sub(timestamp) / 1_000;
    match elapsed {
        0..=59 => "방금".into(),
        60..=3_599 => format!("{}분 전", elapsed / 60),
        3_600..=86_399 => format!("{}시간 전", elapsed / 3_600),
        86_400..=604_799 => format!("{}일 전", elapsed / 86_400),
        _ => {
            let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(timestamp as f64));
            format!("{:02}.{:02}", date.get_month() + 1, date.get_date())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_is_trimmed_and_bounded() {
        assert_eq!(
            title_from_question("  한국의   인공지능 정책  "),
            "한국의 인공지능 정책"
        );
        assert!(title_from_question(&"가".repeat(100)).chars().count() <= 43);
    }
}
