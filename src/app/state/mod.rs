mod conversations;
mod credentials;
mod export;
mod persistence;
mod research;
mod stream;

use super::*;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Panel {
    None,
    Sidebar,
    Sources,
    Settings,
}

#[derive(Clone, Copy)]
pub(super) struct AppState {
    pub(super) workspace: Signal<Workspace>,
    pub(super) active_id: Signal<String>,
    pub(super) composer: Signal<String>,
    pub(super) search_query: Signal<String>,
    pub(super) selected_sources: Signal<Vec<Source>>,
    pub(super) panel: Signal<Panel>,
    pub(super) is_loading: Signal<bool>,
    pub(super) is_running: Signal<bool>,
    pub(super) active_request: Signal<String>,
    pub(super) active_assistant_message: Signal<String>,
    pub(super) stage: Signal<String>,
    pub(super) research_started_at: Signal<u64>,
    pub(super) stage_started_at: Signal<u64>,
    pub(super) research_clock: Signal<u64>,
    pub(super) research_events: Signal<Vec<ResearchEvent>>,
    pub(super) streamed_text: Signal<String>,
    pub(super) pending_stream: Signal<String>,
    pub(super) pending_stream_request: Signal<String>,
    pub(super) stream_frame_pending: Signal<bool>,
    pub(super) key_configured: Signal<bool>,
    pub(super) key_input: Signal<String>,
    pub(super) key_busy: Signal<bool>,
    pub(super) connection_message: Signal<String>,
    pub(super) save_state: Signal<String>,
    pub(super) last_failed_question: Signal<String>,
    pub(super) toast: Signal<String>,
    pub(super) toast_kind: Signal<String>,
    pub(super) storage_label: Signal<String>,
    pub(super) storage_writable: Signal<bool>,
    persistence_queue: Signal<VecDeque<PersistenceRequest>>,
    pub(super) persistence_busy: Signal<bool>,
    delete_rollback: Signal<Option<DeleteRollback>>,
    next_rollback_id: Signal<u64>,
}

impl AppState {
    pub(super) fn new() -> Self {
        Self {
            workspace: create_signal(Workspace::default()),
            active_id: create_signal(String::new()),
            composer: create_signal(String::new()),
            search_query: create_signal(String::new()),
            selected_sources: create_signal(Vec::new()),
            panel: create_signal(Panel::None),
            is_loading: create_signal(true),
            is_running: create_signal(false),
            active_request: create_signal(String::new()),
            active_assistant_message: create_signal(String::new()),
            stage: create_signal(String::new()),
            research_started_at: create_signal(0),
            stage_started_at: create_signal(0),
            research_clock: create_signal(0),
            research_events: create_signal(Vec::new()),
            streamed_text: create_signal(String::new()),
            pending_stream: create_signal(String::new()),
            pending_stream_request: create_signal(String::new()),
            stream_frame_pending: create_signal(false),
            key_configured: create_signal(false),
            key_input: create_signal(String::new()),
            key_busy: create_signal(false),
            connection_message: create_signal(String::new()),
            save_state: create_signal(String::new()),
            last_failed_question: create_signal(String::new()),
            toast: create_signal(String::new()),
            toast_kind: create_signal(String::from("info")),
            storage_label: create_signal(String::from("이 기기에만 저장됨")),
            storage_writable: create_signal(true),
            persistence_queue: create_signal(VecDeque::new()),
            persistence_busy: create_signal(false),
            delete_rollback: create_signal(None),
            next_rollback_id: create_signal(0),
        }
    }

    pub(in crate::app) fn show_toast(self, message: impl Into<String>, kind: &str) {
        let message = message.into();
        batch(move || {
            self.toast.set(message);
            self.toast_kind.set(kind.into());
        });
    }

    pub(in crate::app) fn close_panel(self) {
        self.panel.set(Panel::None);
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiKeyArgs {
    api_key: String,
}

#[derive(Clone, Serialize)]
struct WorkspaceArgs {
    workspace: Workspace,
}

#[derive(Clone)]
struct PersistenceRequest {
    workspace: Workspace,
    success_message: Option<&'static str>,
    delete_rollback_id: Option<u64>,
}

#[derive(Clone)]
struct DeleteRollback {
    id: u64,
    conversation: Conversation,
    index: usize,
    was_active: bool,
    selected_sources: Vec<Source>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RequestIdArgs {
    request_id: String,
}

#[derive(Clone, Serialize)]
struct ResearchArgs {
    #[serde(rename = "conversationId")]
    conversation_id: String,
    #[serde(rename = "assistantMessageId")]
    assistant_message_id: String,
    question: String,
    request: ResearchRequest,
    workspace: Workspace,
}

#[derive(Clone, Serialize)]
pub(super) struct UrlArgs {
    pub(super) url: String,
}

#[derive(Clone, Serialize)]
struct ExportArgs {
    conversation: Conversation,
}

#[derive(Clone, Serialize)]
pub(super) struct EmptyArgs {}

pub(super) fn current_conversation_ref<'a>(
    workspace: &'a Workspace,
    active_id: &str,
) -> Option<&'a Conversation> {
    workspace
        .conversations
        .iter()
        .find(|conversation| conversation.id == active_id)
}

pub(super) fn source_list(workspace: &Workspace, active_id: &str) -> Vec<Source> {
    current_conversation_ref(workspace, active_id)
        .and_then(|conversation| {
            conversation
                .messages
                .iter()
                .rev()
                .find(|message| message.role == "assistant" && !message.sources.is_empty())
        })
        .map(|message| message.sources.clone())
        .unwrap_or_default()
}
