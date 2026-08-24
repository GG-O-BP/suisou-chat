use serde::{Deserialize, Serialize};

pub const WORKSPACE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Workspace {
    pub version: u32,
    pub revision: u64,
    pub conversations: Vec<Conversation>,
    pub settings: Settings,
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            version: WORKSPACE_VERSION,
            revision: 0,
            conversations: Vec::new(),
            settings: Settings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub pinned: bool,
    pub created_at: u64,
    pub updated_at: u64,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
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

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Source {
    pub id: String,
    pub title: String,
    pub url: String,
    pub domain: String,
    pub snippet: String,
    pub retrieved_at: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub orchestration_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapResponse {
    pub workspace: Workspace,
    pub workspace_revision: u64,
    pub key_configured: bool,
    pub credential_notice: Option<String>,
    pub recovery_notice: Option<String>,
    pub storage_label: String,
    pub storage_writable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchRequest {
    pub request_id: String,
    pub model: String,
    pub mode: String,
    pub reasoning: String,
    pub messages: Vec<InputMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchResponse {
    pub request_id: String,
    pub answer: String,
    pub sources: Vec<Source>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchJob {
    pub request_id: String,
    pub conversation_id: String,
    #[serde(default)]
    pub workspace_revision: u64,
    #[serde(default)]
    pub workspace_persisted: bool,
    /// The remote response has ended, but the native layer is still committing
    /// the terminal answer to the workspace/journal.
    ///
    /// Frontends may unlock immediately and render the included result, but
    /// must not discard or independently persist this provisional snapshot.
    #[serde(default)]
    pub finalizing: bool,
    pub assistant_message_id: String,
    pub question: String,
    pub mode: String,
    pub status: String,
    pub stage: String,
    pub partial_answer: String,
    pub result: Option<ResearchResponse>,
    pub error: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(default)]
    pub events: Vec<ResearchEvent>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ResearchEvent {
    pub kind: String,
    pub value: String,
    pub occurred_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartResearchResponse {
    pub job: ResearchJob,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchJobUpdate {
    pub request_id: String,
    pub kind: String,
    pub value: String,
    #[serde(default)]
    pub sequence: u64,
    pub job: Option<ResearchJob>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub message: String,
    pub models: Vec<String>,
}

pub fn validate_workspace(workspace: &Workspace) -> Result<(), String> {
    if workspace.version != WORKSPACE_VERSION {
        return Err(format!(
            "지원하지 않는 대화 기록 버전입니다: {}",
            workspace.version
        ));
    }
    if workspace.conversations.len() > 5_000 {
        return Err("대화가 5,000개를 초과했습니다.".into());
    }
    if !matches!(
        workspace.settings.model.as_str(),
        "fugu" | "fugu-ultra" | "fugu-ultra-v1.0" | "fugu-ultra-v1.1"
    ) {
        return Err("지원하지 않는 Fugu 모델입니다.".into());
    }
    if !matches!(
        workspace.settings.reasoning.as_str(),
        "high" | "xhigh" | "max"
    ) {
        return Err("지원하지 않는 추론 강도입니다.".into());
    }
    if !matches!(
        workspace.settings.theme.as_str(),
        "system" | "light" | "dark"
    ) {
        return Err("지원하지 않는 테마입니다.".into());
    }
    if !matches!(
        workspace.settings.last_mode.as_str(),
        "quick" | "search" | "deep" | "create"
    ) {
        return Err("지원하지 않는 답변 모드입니다.".into());
    }

    let mut total_messages = 0usize;
    for conversation in &workspace.conversations {
        if conversation.id.is_empty() || conversation.id.len() > 128 {
            return Err("잘못된 대화 ID가 있습니다.".into());
        }
        if conversation.title.chars().count() > 160 {
            return Err("대화 제목이 너무 깁니다.".into());
        }
        total_messages += conversation.messages.len();
        if total_messages > 50_000 {
            return Err("메시지가 50,000개를 초과했습니다.".into());
        }
        for message in &conversation.messages {
            if message.id.is_empty() || message.id.len() > 160 {
                return Err("잘못된 메시지 ID가 있습니다.".into());
            }
            if !matches!(message.role.as_str(), "user" | "assistant") {
                return Err("잘못된 메시지 역할이 있습니다.".into());
            }
            if message.content.len() > 2_000_000 {
                return Err("메시지 하나가 2MB를 초과했습니다.".into());
            }
            if message.sources.len() > 200 {
                return Err("답변 하나의 출처가 200개를 초과했습니다.".into());
            }
        }
    }
    Ok(())
}

pub fn validate_research_request(request: &ResearchRequest) -> Result<(), String> {
    if request.request_id.is_empty()
        || request.request_id.len() > 128
        || !request
            .request_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
    {
        return Err("잘못된 요청 ID입니다.".into());
    }
    if !matches!(
        request.model.as_str(),
        "fugu" | "fugu-ultra" | "fugu-ultra-v1.0" | "fugu-ultra-v1.1"
    ) {
        return Err("지원하지 않는 Fugu 모델입니다.".into());
    }
    if !matches!(
        request.mode.as_str(),
        "quick" | "search" | "deep" | "create"
    ) {
        return Err("지원하지 않는 답변 모드입니다.".into());
    }
    if !matches!(request.reasoning.as_str(), "high" | "xhigh" | "max") {
        return Err("지원하지 않는 추론 강도입니다.".into());
    }
    if request.messages.is_empty() || request.messages.len() > 200 {
        return Err("요청에는 1개 이상, 200개 이하의 메시지가 필요합니다.".into());
    }

    let mut total_chars = 0usize;
    for message in &request.messages {
        if !matches!(message.role.as_str(), "user" | "assistant") {
            return Err("요청에 잘못된 메시지 역할이 있습니다.".into());
        }
        if message.content.trim().is_empty() {
            return Err("빈 메시지는 보낼 수 없습니다.".into());
        }
        total_chars += message.content.chars().count();
        if total_chars > 500_000 {
            return Err("대화 문맥이 너무 큽니다. 새 대화를 시작해 주세요.".into());
        }
    }
    if request.messages.last().map(|message| message.role.as_str()) != Some("user") {
        return Err("마지막 메시지는 사용자 질문이어야 합니다.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ResearchRequest {
        ResearchRequest {
            request_id: "req-123".into(),
            model: "fugu".into(),
            mode: "search".into(),
            reasoning: "high".into(),
            messages: vec![InputMessage {
                role: "user".into(),
                content: "검증해 줘".into(),
            }],
        }
    }

    #[test]
    fn accepts_supported_research_request() {
        assert!(validate_research_request(&request()).is_ok());

        let mut creative = request();
        creative.mode = "create".into();
        assert!(validate_research_request(&creative).is_ok());
    }

    #[test]
    fn rejects_arbitrary_model_and_identifier() {
        let mut value = request();
        value.model = "https://attacker.invalid/model".into();
        assert!(validate_research_request(&value).is_err());

        value.model = "fugu".into();
        value.request_id = "../secret".into();
        assert!(validate_research_request(&value).is_err());
    }

    #[test]
    fn rejects_assistant_as_final_message() {
        let mut value = request();
        value.messages[0].role = "assistant".into();
        assert!(validate_research_request(&value).is_err());
    }
}
