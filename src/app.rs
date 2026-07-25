use crate::icons::icon;
use crate::ipc;
use crate::markdown::{render_markdown, render_streaming_markdown};
use crate::models::{
    format_relative_time, new_id, now_millis, remove_conversation, title_from_question,
    BootstrapResponse, ConnectionInfo, Conversation, InputMessage, Message, ResearchEvent,
    ResearchRequest, ResearchResponse, Source, Workspace,
};
use serde::Serialize;
use std::collections::VecDeque;
use sycamore::futures::{spawn_local, spawn_local_scoped};
use sycamore::prelude::*;
use sycamore::web::events::{Event, KeyboardEvent, SubmitEvent};
use sycamore::web::{Suspense, Transition};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

#[derive(Clone, Copy, PartialEq)]
enum Panel {
    None,
    Sidebar,
    Sources,
    Settings,
}

#[derive(Clone, Copy)]
struct AppState {
    workspace: Signal<Workspace>,
    active_id: Signal<String>,
    composer: Signal<String>,
    search_query: Signal<String>,
    selected_sources: Signal<Vec<Source>>,
    panel: Signal<Panel>,
    is_loading: Signal<bool>,
    is_running: Signal<bool>,
    active_request: Signal<String>,
    stage: Signal<String>,
    streamed_text: Signal<String>,
    pending_stream: Signal<String>,
    pending_stream_request: Signal<String>,
    stream_frame_pending: Signal<bool>,
    key_configured: Signal<bool>,
    key_input: Signal<String>,
    key_busy: Signal<bool>,
    connection_message: Signal<String>,
    save_state: Signal<String>,
    last_failed_question: Signal<String>,
    toast: Signal<String>,
    toast_kind: Signal<String>,
    storage_label: Signal<String>,
    storage_writable: Signal<bool>,
    persistence_queue: Signal<VecDeque<PersistenceRequest>>,
    persistence_busy: Signal<bool>,
    delete_rollback: Signal<Option<DeleteRollback>>,
    next_rollback_id: Signal<u64>,
}

impl AppState {
    fn new() -> Self {
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
            stage: create_signal(String::new()),
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

    fn show_toast(self, message: impl Into<String>, kind: &str) {
        let message = message.into();
        batch(move || {
            self.toast.set(message);
            self.toast_kind.set(kind.into());
        });
    }

    fn queue_stream_delta(self, request_id: String, delta: String) {
        if self.pending_stream_request.get_clone_untracked() != request_id {
            batch(move || {
                self.pending_stream.set(String::new());
                self.pending_stream_request.set(request_id);
            });
        }
        self.pending_stream
            .update(|pending| pending.push_str(&delta));
        if self.stream_frame_pending.get_untracked() {
            return;
        }
        self.stream_frame_pending.set(true);

        let callback = Closure::once_into_js(move || {
            self.stream_frame_pending.set(false);
            let pending_request = self.pending_stream_request.get_clone_untracked();
            if self.active_request.get_clone_untracked() != pending_request {
                batch(move || {
                    self.pending_stream.set(String::new());
                    self.pending_stream_request.set(String::new());
                });
                return;
            }
            self.flush_stream_delta();
        });
        let requested = web_sys::window().is_some_and(|window| {
            window
                .request_animation_frame(callback.unchecked_ref())
                .is_ok()
        });
        if !requested {
            self.stream_frame_pending.set(false);
            self.flush_stream_delta();
        }
    }

    fn flush_stream_delta(self) {
        let pending = self.pending_stream.replace(String::new());
        self.pending_stream_request.set(String::new());
        if !pending.is_empty() {
            self.streamed_text
                .update(|streamed| streamed.push_str(&pending));
        }
    }

    fn reset_stream(self) {
        batch(move || {
            self.pending_stream.set(String::new());
            self.pending_stream_request.set(String::new());
            self.streamed_text.set(String::new());
        });
    }

    fn close_panel(self) {
        self.panel.set(Panel::None);
    }

    fn persist_workspace(self) {
        let delete_rollback_id = self
            .delete_rollback
            .with_untracked(|rollback| rollback.as_ref().map(|rollback| rollback.id));
        self.persist_workspace_with_message(None, delete_rollback_id);
    }

    fn persist_workspace_with_message(
        self,
        success_message: Option<&'static str>,
        delete_rollback_id: Option<u64>,
    ) {
        self.persistence_queue.update(|queue| {
            queue.push_back(PersistenceRequest {
                workspace: self.workspace.get_clone_untracked(),
                success_message,
                delete_rollback_id,
            });
        });
        self.storage_writable.set(false);
        self.save_state.set("saving".into());
        self.persist_next_workspace();
    }

    fn persist_next_workspace(self) {
        if self.persistence_busy.get_untracked() {
            return;
        }
        let Some(mut request) = self
            .persistence_queue
            .with_untracked(|queue| queue.front().cloned())
        else {
            return;
        };
        self.persistence_queue.update(|queue| {
            queue.pop_front();
        });
        request.workspace.revision = self
            .workspace
            .with_untracked(|workspace| workspace.revision);
        self.persistence_busy.set(true);

        // This queue can start the next save after the originating event handler's
        // reactive scope has been destroyed. Keep the task on the app-owned signals
        // instead of binding it to that short-lived event scope.
        spawn_local(async move {
            let result = ipc::command::<_, u64>(
                "save_workspace",
                &WorkspaceArgs {
                    workspace: request.workspace,
                },
            )
            .await;
            if !self.workspace.is_alive() {
                return;
            }
            let succeeded = match result {
                Ok(revision) => {
                    // Revision is persistence metadata and is not rendered. Updating it
                    // silently avoids invalidating every workspace-derived selector.
                    self.workspace
                        .update_silent(|value| value.revision = revision);
                    if request.delete_rollback_id.is_some_and(|request_id| {
                        self.delete_rollback.with_untracked(|rollback| {
                            rollback
                                .as_ref()
                                .is_some_and(|rollback| rollback.id == request_id)
                        })
                    }) {
                        self.delete_rollback.set(None);
                    }
                    if request.delete_rollback_id.is_none() {
                        if let Some(message) = request.success_message {
                            self.show_toast(message, "success");
                        }
                    }
                    true
                }
                Err(error) => {
                    if self.persistence_queue.with_untracked(VecDeque::is_empty) {
                        if request.delete_rollback_id.is_some_and(|request_id| {
                            self.delete_rollback.with_untracked(|rollback| {
                                rollback
                                    .as_ref()
                                    .is_some_and(|rollback| rollback.id == request_id)
                            })
                        }) {
                            self.restore_deleted_conversation();
                        }
                        self.save_state.set(format!("error:{error}"));
                        self.show_toast(
                            format!("변경 사항을 저장하지 못했습니다: {error}"),
                            "error",
                        );
                    }
                    false
                }
            };
            self.persistence_busy.set(false);
            if self.persistence_queue.with_untracked(VecDeque::is_empty) {
                if succeeded {
                    self.save_state.set("saved".into());
                    self.storage_writable.set(true);
                    if request.delete_rollback_id.is_some() {
                        if let Some(message) = request.success_message {
                            self.show_toast(message, "success");
                        }
                    }
                }
            } else {
                self.save_state.set("saving".into());
                self.persist_next_workspace();
            }
        });
    }

    fn restore_deleted_conversation(self) {
        let Some(rollback) = self.delete_rollback.take() else {
            return;
        };
        let conversation_id = rollback.conversation.id.clone();
        self.workspace.update(|workspace| {
            if workspace
                .conversations
                .iter()
                .any(|conversation| conversation.id == conversation_id)
            {
                return;
            }
            let index = rollback.index.min(workspace.conversations.len());
            workspace.conversations.insert(index, rollback.conversation);
        });
        if rollback.was_active && self.active_id.with_untracked(String::is_empty) {
            batch(move || {
                self.active_id.set(conversation_id);
                self.selected_sources.set(rollback.selected_sources);
            });
            reset_viewport_scroll();
        }
    }

    fn select_conversation(self, id: String) {
        if self.persistence_busy.get_untracked()
            || !self.persistence_queue.with_untracked(VecDeque::is_empty)
        {
            return;
        }
        let sources = self
            .workspace
            .with_untracked(|workspace| source_list(workspace, &id));
        batch(move || {
            self.active_id.set(id);
            self.selected_sources.set(sources);
            self.close_panel();
        });
        reset_viewport_scroll();
    }

    fn new_conversation(self) {
        if self.persistence_busy.get_untracked()
            || !self.persistence_queue.with_untracked(VecDeque::is_empty)
        {
            return;
        }
        batch(move || {
            self.active_id.set(String::new());
            self.selected_sources.set(Vec::new());
            self.composer.set(String::new());
            self.close_panel();
        });
        reset_viewport_scroll();
    }

    fn delete_conversation(self, id: String) {
        if id.is_empty() {
            return;
        }
        let exists = self.workspace.with_untracked(|workspace| {
            workspace
                .conversations
                .iter()
                .any(|conversation| conversation.id == id)
        });
        if !exists {
            return;
        }
        let is_active = self.active_id.get_clone_untracked() == id;
        let rollback_id = self.next_rollback_id.get_untracked().saturating_add(1);
        self.next_rollback_id.set(rollback_id);
        let rollback = self.workspace.with_untracked(|workspace| {
            workspace
                .conversations
                .iter()
                .position(|conversation| conversation.id == id)
                .map(|index| DeleteRollback {
                    id: rollback_id,
                    conversation: workspace.conversations[index].clone(),
                    index,
                    was_active: is_active,
                    selected_sources: self.selected_sources.get_clone_untracked(),
                })
        });
        let Some(rollback) = rollback else {
            return;
        };
        self.delete_rollback.set(Some(rollback));
        batch(move || {
            self.workspace.update(|value| {
                remove_conversation(value, &id);
            });
            if is_active {
                self.active_id.set(String::new());
                self.selected_sources.set(Vec::new());
            }
        });
        if is_active {
            reset_viewport_scroll();
        }
        self.persist_workspace_with_message(Some("대화 기록을 삭제했습니다."), Some(rollback_id));
    }

    fn toggle_pin(self) {
        if !self.storage_writable.get_untracked() {
            return;
        }
        let id = self.active_id.get_clone_untracked();
        self.workspace.update(|value| {
            if let Some(conversation) = value
                .conversations
                .iter_mut()
                .find(|conversation| conversation.id == id)
            {
                conversation.pinned = !conversation.pinned;
                conversation.updated_at = now_millis();
            }
        });
        self.persist_workspace();
    }

    fn retry_question(self) {
        let question = self.last_failed_question.get_clone_untracked();
        if question.is_empty() {
            return;
        }
        if !self.storage_writable.get_untracked() {
            self.show_toast(
                "대화 기록 저장이 완료되거나 복구된 뒤 다시 시도해 주세요.",
                "warning",
            );
            return;
        }
        if !self.key_configured.get_untracked() {
            self.panel.set(Panel::Settings);
            self.show_toast("먼저 설정에서 Sakana API 키를 연결해 주세요.", "warning");
            return;
        }
        let failed_id = self.active_id.get_clone_untracked();
        batch(move || {
            self.composer.set(question);
            self.workspace.update(|value| {
                if let Some(conversation) = value
                    .conversations
                    .iter_mut()
                    .find(|conversation| conversation.id == failed_id)
                {
                    if matches!(
                        conversation
                            .messages
                            .last()
                            .map(|message| message.status.as_str()),
                        Some("failed" | "cancelled")
                    ) {
                        conversation.messages.pop();
                    }
                    if conversation
                        .messages
                        .last()
                        .is_some_and(|message| message.role == "user")
                    {
                        conversation.messages.pop();
                    }
                }
            });
        });
        self.send_question();
    }

    fn send_question(self) {
        let question = self.composer.get_clone_untracked().trim().to_owned();
        if question.is_empty() || self.is_running.get_untracked() {
            return;
        }
        if !self.storage_writable.get_untracked() {
            self.show_toast(
                "대화 기록 저장이 완료되거나 복구된 뒤 다시 시도해 주세요.",
                "warning",
            );
            return;
        }
        if question.chars().count() > 20_000 {
            self.show_toast("질문은 20,000자 이하로 입력해 주세요.", "error");
            return;
        }
        if !self.key_configured.get_untracked() {
            self.panel.set(Panel::Settings);
            self.show_toast("먼저 설정에서 Sakana API 키를 연결해 주세요.", "warning");
            return;
        }

        let active_id = self.active_id.get_clone_untracked();
        let (prior_messages, prior_chars) = self.workspace.with_untracked(|workspace| {
            current_conversation_ref(workspace, &active_id).map_or((0, 0), |conversation| {
                (
                    conversation.messages.len(),
                    conversation
                        .messages
                        .iter()
                        .map(|message| message.content.chars().count())
                        .sum(),
                )
            })
        });
        if prior_messages >= 199 || prior_chars.saturating_add(question.chars().count()) > 500_000 {
            self.show_toast(
                "이 대화에서 사용할 수 있는 문맥 한도에 도달했습니다. 새 대화에서 이어가 주세요.",
                "warning",
            );
            return;
        }

        let timestamp = now_millis();
        let conversation_id = if active_id.is_empty() {
            let id = new_id("conversation");
            let id_for_workspace = id.clone();
            self.workspace.update(|value| {
                value.conversations.push(Conversation {
                    id: id_for_workspace,
                    title: title_from_question(&question),
                    pinned: false,
                    created_at: timestamp,
                    updated_at: timestamp,
                    messages: Vec::new(),
                });
            });
            self.active_id.set(id.clone());
            id
        } else {
            active_id
        };

        self.workspace.update(|value| {
            if let Some(conversation) = value
                .conversations
                .iter_mut()
                .find(|conversation| conversation.id == conversation_id)
            {
                conversation.updated_at = timestamp;
                conversation.messages.push(Message {
                    id: new_id("message"),
                    role: "user".into(),
                    content: question.clone(),
                    created_at: timestamp,
                    status: "complete".into(),
                    sources: Vec::new(),
                    usage: None,
                });
            }
        });

        let request_id = new_id("request");
        let active_request_id = request_id.clone();
        batch(move || {
            self.composer.set(String::new());
            self.last_failed_question.set(String::new());
            self.reset_stream();
            self.selected_sources.set(Vec::new());
            self.stage.set("connecting".into());
            self.is_running.set(true);
            self.active_request.set(active_request_id);
        });
        self.persist_workspace();

        let request = self.workspace.with_untracked(|workspace| {
            let conversation = current_conversation_ref(workspace, &conversation_id)
                .cloned()
                .unwrap_or_default();
            ResearchRequest {
                request_id: request_id.clone(),
                model: workspace.settings.model.clone(),
                mode: workspace.settings.last_mode.clone(),
                reasoning: workspace.settings.reasoning.clone(),
                messages: conversation
                    .messages
                    .iter()
                    .map(|message| InputMessage {
                        role: message.role.clone(),
                        content: message.content.clone(),
                    })
                    .collect(),
            }
        });

        spawn_local_scoped(async move {
            let result =
                ipc::command::<_, ResearchResponse>("run_research", &ResearchArgs { request })
                    .await;
            if self.active_request.get_clone_untracked() != request_id {
                return;
            }
            self.flush_stream_delta();
            batch(move || {
                self.is_running.set(false);
                self.active_request.set(String::new());
            });
            match result {
                Ok(response) => {
                    self.workspace.update(|value| {
                        if let Some(conversation) = value
                            .conversations
                            .iter_mut()
                            .find(|conversation| conversation.id == conversation_id)
                        {
                            conversation.updated_at = now_millis();
                            conversation.messages.push(Message {
                                id: new_id("message"),
                                role: "assistant".into(),
                                content: response.answer,
                                created_at: now_millis(),
                                status: "complete".into(),
                                sources: response.sources.clone(),
                                usage: response.usage,
                            });
                        }
                    });
                    batch(move || {
                        self.selected_sources.set(response.sources);
                        self.reset_stream();
                        self.stage.set("done".into());
                    });
                    self.persist_workspace();
                }
                Err(error) => {
                    let partial = self.streamed_text.get_clone_untracked();
                    let status = if error.contains("중단") {
                        "cancelled"
                    } else {
                        "failed"
                    };
                    if !partial.trim().is_empty() {
                        self.workspace.update(|value| {
                            if let Some(conversation) = value
                                .conversations
                                .iter_mut()
                                .find(|conversation| conversation.id == conversation_id)
                            {
                                conversation.updated_at = now_millis();
                                conversation.messages.push(Message {
                                    id: new_id("message"),
                                    role: "assistant".into(),
                                    content: partial,
                                    created_at: now_millis(),
                                    status: status.into(),
                                    sources: Vec::new(),
                                    usage: None,
                                });
                            }
                        });
                        self.persist_workspace();
                    }
                    batch(move || {
                        self.stage.set(status.into());
                        self.reset_stream();
                        self.last_failed_question.set(question);
                    });
                    let mut error = error;
                    if error.contains("인증") || error.contains("API 키") {
                        if let Err(clear_error) =
                            ipc::command_unit("clear_api_key", &EmptyArgs {}).await
                        {
                            error = format!("{error} {clear_error}");
                            let _ = ipc::command_unit("forget_api_key", &EmptyArgs {}).await;
                        }
                        batch(move || {
                            self.key_configured.set(false);
                            self.panel.set(Panel::Settings);
                        });
                    }
                    self.show_toast(error, "error");
                }
            }
        });
    }

    fn cancel_request(self) {
        let request_id = self.active_request.get_clone_untracked();
        spawn_local_scoped(async move {
            match ipc::command::<_, bool>("cancel_research", &RequestIdArgs { request_id }).await {
                Ok(true) => self.show_toast("답변 생성을 중단했습니다.", "info"),
                Ok(false) => self.show_toast("이미 완료된 요청입니다.", "info"),
                Err(error) => self.show_toast(error, "error"),
            }
        });
    }

    fn connect_key(self) {
        let api_key = self.key_input.take();
        batch(move || {
            self.key_busy.set(true);
            self.show_toast(
                "API 키 확인 후 운영체제 보안 저장소의 잠금 해제 창이 나타나면 완료해 주세요.",
                "info",
            );
        });
        spawn_local_scoped(async move {
            let result =
                ipc::command::<_, ConnectionInfo>("connect_api_key", &ApiKeyArgs { api_key }).await;
            self.key_busy.set(false);
            match result {
                Ok(info) => {
                    let model_note = if info.models.is_empty() {
                        String::new()
                    } else {
                        format!(" · 사용 가능한 모델 {}개", info.models.len())
                    };
                    batch(move || {
                        self.key_configured.set(true);
                        self.connection_message
                            .set(format!("{}{model_note}", info.message));
                    });
                    self.show_toast(
                        "Sakana API 연결을 확인하고 키를 안전하게 저장했습니다.",
                        "success",
                    );
                }
                Err(error) => self.show_toast(error, "error"),
            }
        });
    }

    fn clear_key(self) {
        spawn_local_scoped(async move {
            let result = ipc::command_unit("clear_api_key", &EmptyArgs {}).await;
            batch(move || {
                self.key_configured.set(false);
                self.connection_message.set(String::new());
            });
            match result {
                Ok(()) => self.show_toast(
                    "API 키를 메모리와 운영체제 보안 저장소에서 제거했습니다.",
                    "success",
                ),
                Err(error) => self.show_toast(error, "error"),
            }
        });
    }

    fn export_current(self) {
        let active_id = self.active_id.get_clone_untracked();
        let conversation = self
            .workspace
            .with_untracked(|workspace| current_conversation_ref(workspace, &active_id).cloned());
        let Some(conversation) = conversation else {
            return;
        };
        spawn_local_scoped(async move {
            match ipc::command::<_, String>("export_conversation", &ExportArgs { conversation })
                .await
            {
                Ok(_) => self.show_toast("Markdown 파일로 내보냈습니다.", "success"),
                Err(error) => self.show_toast(error, "error"),
            }
        });
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
    request: ResearchRequest,
}

#[derive(Clone, Serialize)]
struct UrlArgs {
    url: String,
}

#[derive(Clone, Serialize)]
struct ExportArgs {
    conversation: Conversation,
}

#[derive(Clone, Serialize)]
struct EmptyArgs {}

fn current_conversation_ref<'a>(
    workspace: &'a Workspace,
    active_id: &str,
) -> Option<&'a Conversation> {
    workspace
        .conversations
        .iter()
        .find(|conversation| conversation.id == active_id)
}

fn source_list(workspace: &Workspace, active_id: &str) -> Vec<Source> {
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

fn update_theme(theme: &str) {
    if let Some(document) = web_sys::window().and_then(|window| window.document()) {
        if let Some(root) = document.document_element() {
            if theme == "system" {
                let _ = root.remove_attribute("data-theme");
            } else {
                let _ = root.set_attribute("data-theme", theme);
            }
        }
    }
}

fn is_mobile_viewport() -> bool {
    web_sys::window()
        .and_then(|window| window.inner_width().ok())
        .and_then(|width| width.as_f64())
        .is_some_and(|width| width <= 860.0)
}

/// Registers document-level keyboard shortcuts for the whole app:
/// - `Escape` closes any open panel from anywhere, so keyboard users can dismiss
///   the modal settings/sources dialogs even when focus is inside them.
/// - `Ctrl`/`Cmd` + `N` starts a new conversation, matching the shortcut hint the
///   sidebar already advertises next to the "새 대화" button.
fn install_global_shortcuts(state: AppState) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let handler =
        Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |event: web_sys::KeyboardEvent| {
            if event.key() == "Escape" {
                if state.panel.get_untracked() != Panel::None {
                    event.prevent_default();
                    state.close_panel();
                }
                return;
            }
            if event.default_prevented() {
                return;
            }
            let new_conversation_combo = (event.meta_key() || event.ctrl_key())
                && !event.shift_key()
                && !event.alt_key()
                && matches!(event.key().as_str(), "n" | "N");
            if new_conversation_combo {
                if state.is_running.get_untracked() || !state.storage_writable.get_untracked() {
                    return;
                }
                event.prevent_default();
                state.new_conversation();
            }
        });
    let target = document.clone();
    let _ = target.add_event_listener_with_callback("keydown", handler.as_ref().unchecked_ref());
    on_cleanup(move || {
        let _ = document
            .remove_event_listener_with_callback("keydown", handler.as_ref().unchecked_ref());
        drop(handler);
    });
}

fn reset_viewport_scroll() {
    let Some(window) = web_sys::window() else {
        return;
    };
    window.scroll_to_with_x_and_y(0.0, 0.0);
    if let Some(document) = window.document() {
        if let Some(root) = document.document_element() {
            root.set_scroll_top(0);
        }
        if let Some(body) = document.body() {
            body.set_scroll_top(0);
        }
    }
}

fn open_url(state: AppState, url: String) {
    spawn_local_scoped(async move {
        if let Err(error) = ipc::command_unit("open_external", &UrlArgs { url }).await {
            state.show_toast(error, "error");
        }
    });
}

fn open_markdown_link(state: AppState, event: web_sys::MouseEvent) {
    let Some(element) = event
        .target()
        .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
        .and_then(|target| target.closest("a").ok().flatten())
    else {
        return;
    };
    let Some(url) = element.get_attribute("href") else {
        return;
    };
    event.prevent_default();
    if url.is_empty() {
        state.show_toast("안전한 HTTPS 링크만 열 수 있습니다.", "warning");
    } else {
        open_url(state, url);
    }
}

fn copy_text(state: AppState, text: String) {
    spawn_local_scoped(async move {
        let result = if let Some(window) = web_sys::window() {
            wasm_bindgen_futures::JsFuture::from(window.navigator().clipboard().write_text(&text))
                .await
                .map(|_| ())
                .map_err(|_| ())
        } else {
            Err(())
        };
        if result.is_ok() {
            state.show_toast("답변을 클립보드에 복사했습니다.", "success");
        } else {
            state.show_toast("클립보드에 복사하지 못했습니다.", "error");
        }
    });
}

fn select_value(event: Event) -> Option<String> {
    event
        .target()?
        .dyn_into::<web_sys::HtmlSelectElement>()
        .ok()
        .map(|element| element.value())
}

fn stage_index(stage: &str) -> i32 {
    match stage {
        "connecting" => 0,
        "searching" | "creating" => 1,
        "reasoning" => 2,
        "writing" | "done" => 3,
        _ => -1,
    }
}

fn stage_label(stage: &str, mode: &str) -> &'static str {
    match (stage, mode) {
        ("creating", _) => "아이디어를 빚는 중",
        ("reasoning", "create") => "구성과 목소리를 다듬는 중",
        ("writing", "create") => "창작물을 쓰는 중",
        ("done", "create") => "창작 완료",
        ("connecting", _) => "Sakana에 연결 중",
        ("searching", _) => "웹 자료를 찾는 중",
        ("reasoning", _) => "출처를 비교하는 중",
        ("writing", _) => "답변을 작성하는 중",
        ("cancelled", _) => "중단됨",
        _ => "답변 준비 중",
    }
}

fn stage_depth(stage: &str, mode: &str) -> i32 {
    if mode == "create" {
        return match stage {
            "connecting" => 120,
            "creating" | "reasoning" => 360,
            "writing" | "done" => 720,
            _ => 0,
        };
    }
    match stage {
        "connecting" => 120,
        "searching" => 480,
        "reasoning" => 1_240,
        "writing" | "done" => 1_880,
        _ => 0,
    }
}

fn mode_depth(mode: &str) -> &'static str {
    match mode {
        "quick" => "SURFACE · 40 M",
        "deep" => "ABYSS · 1,880 M",
        "create" => "ATELIER · 720 M",
        _ => "REEF · 480 M",
    }
}

#[component]
pub fn App() -> View {
    let state = AppState::new();
    provide_context(state);
    install_global_shortcuts(state);
    on_mount(reset_viewport_scroll);

    view! {
        div(class=move || format!("app-shell {}", if state.panel.get() != Panel::None { "panel-open" } else { "" })) {
            a(class="skip-link", href="#main-content") { "본문으로 건너뛰기" }
            AppRuntime {}
            Sidebar {}
            WorkspaceView {}
            SourcesPanel {}
            SettingsPanel {}
            OverlayLayer {}
        }
    }
}

#[component]
fn AppRuntime() -> View {
    let state = use_context::<AppState>();

    spawn_local_scoped(async move {
        match ipc::listen::<ResearchEvent, _>("research-event", move |event| {
            if event.request_id != state.active_request.get_clone_untracked() {
                return;
            }
            match event.kind.as_str() {
                "delta" => state.queue_stream_delta(event.request_id, event.value),
                "stage" => state.stage.set(event.value),
                _ => {}
            }
        })
        .await
        {
            Ok(listener) => on_cleanup(move || listener.unlisten()),
            Err(error) => state.show_toast(error, "error"),
        }
    });

    view! {
        Suspense(fallback=View::default) {
            BootstrapWorkspace {}
        }
    }
}

#[component]
async fn BootstrapWorkspace() -> View {
    let state = use_context::<AppState>();
    match ipc::command::<_, BootstrapResponse>("bootstrap", &EmptyArgs {}).await {
        Ok(response) => {
            update_theme(&response.workspace.settings.theme);
            let notices = [response.recovery_notice, response.credential_notice]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            batch(move || {
                state.selected_sources.set(Vec::new());
                state.active_id.set(String::new());
                state.key_configured.set(response.key_configured);
                state.connection_message.set(if response.key_configured {
                    "보안 저장소에서 자동 복원됨".into()
                } else {
                    String::new()
                });
                state.storage_label.set(response.storage_label);
                state.storage_writable.set(response.storage_writable);
                state.workspace.set(response.workspace);
                state.is_loading.set(false);
            });
            if !notices.is_empty() {
                state.show_toast(notices.join(" "), "warning");
            }
        }
        Err(error) => {
            state.is_loading.set(false);
            state.show_toast(error, "error");
        }
    }
    View::default()
}

#[derive(Clone, PartialEq)]
struct HistoryEntry {
    id: String,
    title: String,
    pinned: bool,
    updated_at: u64,
}

#[component]
fn Sidebar() -> View {
    let state = use_context::<AppState>();
    let filtered_conversations = create_selector(move || {
        let query = state.search_query.get_clone().to_lowercase();
        state.workspace.with(|workspace| {
            let mut values = workspace
                .conversations
                .iter()
                .filter(|conversation| {
                    query.is_empty()
                        || conversation.title.to_lowercase().contains(&query)
                        || conversation
                            .messages
                            .iter()
                            .any(|message| message.content.to_lowercase().contains(&query))
                })
                .map(|conversation| HistoryEntry {
                    id: conversation.id.clone(),
                    title: conversation.title.clone(),
                    pinned: conversation.pinned,
                    updated_at: conversation.updated_at,
                })
                .collect::<Vec<_>>();
            values.sort_by_key(|conversation| {
                (
                    !conversation.pinned,
                    std::cmp::Reverse(conversation.updated_at),
                )
            });
            values
        })
    });
    let is_history_empty = create_selector(move || {
        state
            .workspace
            .with(|workspace| workspace.conversations.is_empty())
    });

    view! {
        aside(class=move || format!("sidebar {}", if state.panel.get() == Panel::Sidebar { "visible" } else { "" }), aria-label="대화 기록") {
            div(class="brand") {
                div(class="brand-mark", aria-hidden="true") {
                    span(class="water-line") {}
                    span(class="fugu-dot dot-one") {}
                    span(class="fugu-dot dot-two") {}
                    span(class="fugu-dot dot-three") {}
                }
                div { strong { "SUISOU" } small { "RESEARCH COMPANION" } }
                button(class="icon-button mobile-only", aria-label="메뉴 닫기", on:click=move |_| state.close_panel()) { (icon("close")) }
            }

            button(
                class="new-research",
                disabled=move || state.is_running.get() || !state.storage_writable.get(),
                on:click=move |_| state.new_conversation()
            ) {
                (icon("plus"))
                span { "새 대화" }
                kbd { "⌘ N" }
            }

            label(class="history-search") {
                span(class="sr-only") { "대화 기록 검색" }
                (icon("search"))
                input(bind:value=state.search_query, placeholder="기록에서 검색", autocomplete="off")
            }

            nav(class="history-list", aria-label="저장된 대화") {
                (if is_history_empty.get() {
                    view! {
                        div(class="history-empty") {
                            span(class="empty-ripple") {}
                            p { "첫 질문이 여기에 기록됩니다." }
                        }
                    }
                } else {
                    View::default()
                })
                Keyed(
                    list=filtered_conversations,
                    key=|conversation| (
                        conversation.id.clone(),
                        conversation.updated_at,
                        conversation.pinned,
                        conversation.title.clone(),
                    ),
                    view=move |conversation: HistoryEntry| view! {
                        HistoryItem(conversation=conversation)
                    }
                )
            }

            div(class="sidebar-footer") {
                div(class="storage-status") {
                    span(class="status-light") {}
                    div {
                        strong { (state.storage_label) }
                        small { (move || match state.save_state.get_clone().as_str() {
                            "saving" => "저장 중…",
                            value if value.starts_with("error:") => "저장 오류",
                            _ => "오프라인에서도 기록 열람 가능",
                        }) }
                    }
                }
                button(class="settings-button", on:click=move |_| state.panel.set(Panel::Settings)) {
                    (icon("settings"))
                    "설정"
                }
            }
        }
    }
}

#[derive(Props)]
struct HistoryItemProps {
    conversation: HistoryEntry,
}

#[component]
fn HistoryItem(props: HistoryItemProps) -> View {
    let state = use_context::<AppState>();
    let conversation = props.conversation;
    let id = conversation.id.clone();
    let class_id = id.clone();
    let click_id = id.clone();
    let delete_id = id.clone();
    let delete_label = format!("{} 대화 삭제", conversation.title);
    view! {
        div(
            class=move || format!("history-item {}", if state.active_id.get_clone() == class_id { "active" } else { "" })
        ) {
            button(
                class="history-select",
                aria-current=move || if state.active_id.get_clone() == id { "page" } else { "false" },
                disabled=move || state.is_running.get() || !state.storage_writable.get(),
                on:click=move |_| state.select_conversation(click_id.clone())
            ) {
                span(class="history-glyph") { (if conversation.pinned { icon("pin") } else { icon("search") }) }
                span(class="history-copy") {
                    strong { (conversation.title) }
                    small { (format_relative_time(conversation.updated_at)) }
                }
            }
            button(
                class="history-delete",
                aria-label=delete_label,
                title="대화 삭제",
                disabled=move || {
                    state.is_running.get()
                        || state.persistence_busy.get()
                        || !state.storage_writable.get()
                },
                on:click=move |_| state.delete_conversation(delete_id.clone())
            ) {
                (icon("trash"))
            }
        }
    }
}

#[component]
fn WorkspaceView() -> View {
    view! {
        main(id="main-content", class="workspace") {
            TopBar {}
            Transcript {}
            Composer {}
        }
    }
}

#[component]
fn TopBar() -> View {
    let state = use_context::<AppState>();
    let current_title = create_memo(move || {
        let active_id = state.active_id.get_clone();
        state.workspace.with(|workspace| {
            current_conversation_ref(workspace, &active_id)
                .map(|conversation| conversation.title.clone())
                .unwrap_or_else(|| "새 대화".into())
        })
    });
    let source_count = state.selected_sources.map(Vec::len);

    view! {
        header(class="topbar") {
            div(class="topbar-start") {
                button(class="icon-button mobile-only", aria-label="대화 기록 열기", on:click=move |_| state.panel.set(Panel::Sidebar)) { (icon("menu")) }
                div(class="conversation-heading") {
                    small { "CURRENT DIVE" }
                    strong { (current_title) }
                }
            }
            div(class="topbar-actions") {
                span(
                    class=move || format!("connection-pill {}", if state.key_configured.get() { "connected" } else { "disconnected" }),
                    role="status",
                    aria-label=move || if state.key_configured.get() { "Sakana Fugu 연결됨" } else { "API 키 필요" }
                ) {
                    span(class="connection-dot") {}
                    span(class="connection-label") {
                        (move || if state.key_configured.get() { "Sakana Fugu 연결됨" } else { "API 키 필요" })
                    }
                    span(class="connection-label-mobile", aria-hidden="true") {
                        (move || if state.key_configured.get() { "연결됨" } else { "키 필요" })
                    }
                }
                button(class="icon-button", aria-label="출처 패널 열기", on:click=move |_| state.panel.set(Panel::Sources)) {
                    (icon("sources"))
                    (if source_count.get() > 0 {
                        view! { span(class="count-badge") { (source_count) } }
                    } else {
                        View::default()
                    })
                }
            }
        }
    }
}

#[component]
fn Transcript() -> View {
    let state = use_context::<AppState>();
    let transcript_ref = create_node_ref();
    let message_count = create_selector(move || {
        let active_id = state.active_id.get_clone();
        state.workspace.with(|workspace| {
            current_conversation_ref(workspace, &active_id)
                .map(|conversation| conversation.messages.len())
                .unwrap_or(0)
        })
    });

    create_effect(on(
        (
            message_count,
            state.streamed_text,
            state.stage,
            state.is_running,
            state.active_id,
        ),
        move || {
            message_count.track();
            state.streamed_text.track();
            state.stage.track();
            state.is_running.track();
            state.active_id.track();
            let transcript_ref = transcript_ref;
            sycamore::web::queue_microtask(move || {
                reset_viewport_scroll();
                if let Some(element) = transcript_ref
                    .try_get()
                    .and_then(|node| node.dyn_into::<web_sys::Element>().ok())
                {
                    if state.active_id.with_untracked(String::is_empty) {
                        element.set_scroll_top(0);
                    } else {
                        element.set_scroll_top(element.scroll_height());
                    }
                }
            });
        },
    ));

    view! {
        section(r#ref=transcript_ref, class="transcript", aria-label="대화") {
            Transition(fallback=|| view! {
                div(class="loading-state", role="status") {
                    span(class="sonar-loader") {}
                    p { "대화를 불러오는 중…" }
                }
            }) {
                (if state.is_loading.get() {
                    view! {
                        div(class="loading-state", role="status") {
                            span(class="sonar-loader") {}
                            p { "대화를 불러오는 중…" }
                        }
                    }
                } else if state.active_id.get_clone().is_empty() {
                    view! { Welcome {} }
                } else {
                    view! { ConversationTranscript {} }
                })
            }
        }
    }
}

#[component]
fn ConversationTranscript() -> View {
    let state = use_context::<AppState>();
    let active_messages = create_selector(move || {
        let active_id = state.active_id.get_clone();
        state.workspace.with(|workspace| {
            current_conversation_ref(workspace, &active_id)
                .map(|conversation| conversation.messages.clone())
                .unwrap_or_default()
        })
    });
    view! {
        div(class="message-stack") {
            Keyed(
                list=active_messages,
                key=|message| message.id.clone(),
                view=move |message: Message| view! { MessageView(message=message) }
            )
            StreamingMessage {}
            RetryBanner {}
        }
    }
}

#[component]
fn Welcome() -> View {
    let state = use_context::<AppState>();
    view! {
        section(class="welcome") {
            h1(class="sr-only") { "Suisou 심해 리서치 관측소" }
            div(class="welcome-observatory") {
                div(class="observatory-datum", aria-hidden="true") {
                    span { "SURFACE DATUM" }
                    i {}
                    span { "OBS · 01" }
                }
                div(class="observation-tank", aria-hidden="true") {
                    div(class="tank-rim") {
                        span(class="rim-mark mark-north") { "00" }
                        span(class="rim-mark mark-east") { "90" }
                        span(class="rim-mark mark-south") { "180" }
                        span(class="rim-mark mark-west") { "270" }
                    }
                    div(class="tank-glass") {
                        span(class="water-caustic caustic-one") {}
                        span(class="water-caustic caustic-two") {}
                        span(class="bathymetry bathymetry-one") {}
                        span(class="bathymetry bathymetry-two") {}
                        span(class="bathymetry bathymetry-three") {}
                        span(class="specimen-light specimen-one") {}
                        span(class="specimen-light specimen-two") {}
                        span(class="specimen-light specimen-three") {}
                        div(class="tank-reticle") {
                            span {}
                            (icon("spark"))
                        }
                    }
                    div(class="tank-telemetry") {
                        span { "SAL 34.7" }
                        span { "480 M" }
                        span { "12.4°C" }
                    }
                }
                div(class="welcome-status") {
                    span(class=move || format!("status-beacon {}", if state.key_configured.get() { "ready" } else { "attention" })) {}
                    div {
                        small { "LIFE SUPPORT · OBSERVATORY 01" }
                        strong { (move || if state.key_configured.get() { "Sakana Fugu 준비 완료" } else { "Sakana API 연결 필요" }) }
                    }
                    (if !state.key_configured.get() {
                        view! {
                            button(on:click=move |_| state.panel.set(Panel::Settings)) {
                                "설정 열기"
                                (icon("external"))
                            }
                        }
                    } else {
                        view! { span(class="status-code") { "ONLINE" } }
                    })
                }
                div(class="observatory-depth-scale", aria-hidden="true") {
                    span { "000" }
                    i {}
                    span { "120" }
                    i {}
                    span { "480" }
                }
            }
            div(class="suggestion-deck") {
                div(class="suggestion-heading") {
                    span { "01—04" }
                    p { "예시 질문을 고르거나 직접 입력해 보세요" }
                }
                div(class="suggestion-grid") {
                    SuggestionButton(
                        value="이번 주 AI 에이전트 분야의 주요 발표를 출처별로 교차 검증해 줘",
                        index="01",
                        title="이번 주의 흐름",
                        description="AI 에이전트 주요 발표 교차 검증",
                        icon_name="globe",
                        tone="coral",
                        mode=""
                    )
                    SuggestionButton(
                        value="한국과 일본의 생성형 AI 정책을 공식 자료 중심으로 비교해 줘",
                        index="02",
                        title="정책 비교",
                        description="공식 자료의 차이와 공통점",
                        icon_name="deep",
                        tone="blue",
                        mode=""
                    )
                    SuggestionButton(
                        value="이 주장의 찬반 근거를 찾아 신뢰도와 한계를 표로 정리해 줘: ",
                        index="03",
                        title="주장 검증",
                        description="찬반 근거와 신뢰도 평가",
                        icon_name="search",
                        tone="gold",
                        mode=""
                    )
                    SuggestionButton(
                        value="늦은 밤 수족관을 배경으로, 오랜 친구 둘이 숨겨 둔 진심을 처음 꺼내는 짧은 대화 장면을 써 줘",
                        index="04",
                        title="장면과 대사",
                        description="분위기 있는 창작 장면 만들기",
                        icon_name="create",
                        tone="violet",
                        mode="create"
                    )
                }
            }
            p(class="privacy-note") { (icon("key")) " 질문은 Sakana로 전송됩니다. 개인정보·기밀은 입력하지 마세요." }
        }
    }
}

#[derive(Props)]
struct SuggestionButtonProps {
    value: &'static str,
    index: &'static str,
    title: &'static str,
    description: &'static str,
    icon_name: &'static str,
    tone: &'static str,
    mode: &'static str,
}

#[component]
fn SuggestionButton(props: SuggestionButtonProps) -> View {
    let state = use_context::<AppState>();
    view! {
        button(on:click=move |_| {
            if !props.mode.is_empty() {
                state.workspace.update(|workspace| workspace.settings.last_mode = props.mode.into());
                state.persist_workspace();
            }
            state.composer.set(props.value.into());
        }) {
            span(class="suggestion-index") { (props.index) }
            span(class=format!("suggestion-icon {}", props.tone)) { (icon(props.icon_name)) }
            span { strong { (props.title) } small { (props.description) } }
            span(class="suggestion-arrow", aria-hidden="true") { "↗" }
        }
    }
}

#[derive(Props)]
struct MessageViewProps {
    message: Message,
}

#[component]
fn MessageView(props: MessageViewProps) -> View {
    let state = use_context::<AppState>();
    let message = props.message;
    let is_assistant = message.role == "assistant";
    let role_class = message.role.clone();
    let status_class = message.status.clone();
    let content = message.content.clone();
    let footer_content = content.clone();
    let total_tokens = message.usage.as_ref().map(|usage| usage.total_tokens);
    let status_label = match message.status.as_str() {
        "failed" => Some("일부만 작성됨"),
        "cancelled" => Some("작성 중단됨"),
        _ => None,
    };
    let footer = if is_assistant {
        view! {
            AnswerFooter(
                content=footer_content.clone(),
                sources=message.sources.clone()
            ) {
                (total_tokens.map(|total| view! {
                    small(class="usage") { (format!("총 {total} tokens")) }
                }).unwrap_or_default())
            }
        }
    } else {
        View::default()
    };
    let body = if is_assistant {
        let html = render_markdown(&content);
        view! {
            div(
                class="message-body markdown-body",
                on:click=move |event| open_markdown_link(state, event),
                dangerously_set_inner_html=html
            )
        }
    } else {
        view! { div(class="message-body") { (content) } }
    };
    view! {
        article(class=format!("message {role_class} status-{status_class}")) {
            div(class="message-meta") {
                span(class="role-mark", aria-hidden="true") { (if is_assistant { "F" } else { "Q" }) }
                strong { (if is_assistant { "Sakana Fugu" } else { "질문" }) }
                (status_label.map(|label| view! { span(class="partial-label") { (label) } }).unwrap_or_default())
                time { (format_relative_time(message.created_at)) }
            }
            (body)
            (footer)
        }
    }
}

#[derive(Props)]
struct AnswerFooterProps {
    content: String,
    sources: Vec<Source>,
    children: Children,
}

#[component]
fn AnswerFooter(props: AnswerFooterProps) -> View {
    let state = use_context::<AppState>();
    let source_count = props.sources.len();
    let content = props.content;
    let sources = props.sources;
    let source_action = if source_count > 0 {
        view! {
            button(class="text-action", on:click=move |_| {
                let sources = sources.clone();
                batch(move || {
                    state.selected_sources.set(sources);
                    state.panel.set(Panel::Sources);
                });
            }) { (icon("sources")) (format!("출처 {source_count}")) }
        }
    } else {
        View::default()
    };
    let usage_view = props.children.call();
    view! {
        div(class="answer-footer") {
            div(class="answer-actions") {
                button(class="text-action", on:click=move |_| copy_text(state, content.clone())) { (icon("copy")) "복사" }
                (source_action)
            }
            (usage_view)
        }
    }
}

#[component]
fn StreamingMessage() -> View {
    let state = use_context::<AppState>();
    let selected_mode = create_memo(move || {
        state
            .workspace
            .with(|workspace| workspace.settings.last_mode.clone())
    });
    let is_creative = create_selector(move || selected_mode.get_clone() == "create");
    let stage_progress = create_selector(move || stage_index(&state.stage.get_clone()));
    let stage_percent =
        create_selector(move || ((stage_index(&state.stage.get_clone()) + 1) * 25).max(0));
    view! {
        (if state.is_running.get() {
            view! {
                article(class="message assistant streaming", aria-busy="true") {
                    div(class="message-meta") {
                        span(class="role-mark sonar") { span {} }
                        strong { "Sakana Fugu" }
                        span(class="research-stage") { (move || stage_label(&state.stage.get_clone(), &selected_mode.get_clone())) }
                    }
                    (if state.streamed_text.with(String::is_empty) {
                        view! {
                            div(
                                class=move || format!("research-progress stage-{}", state.stage.get_clone()),
                                role="progressbar",
                                aria-label="답변 준비 상태",
                                aria-valuemin="0",
                                aria-valuemax="100",
                                aria-valuenow=move || stage_percent.get().to_string()
                            ) {
                                div(class="dive-telemetry") {
                                    div {
                                        small { "LIVE DEPTH" }
                                        strong { (move || format!("{:04} M", stage_depth(&state.stage.get_clone(), &selected_mode.get_clone()))) }
                                    }
                                    span(class="telemetry-signal") { "● SIGNAL STABLE" }
                                }
                                div(class="depth-gauge", aria-hidden="true") {
                                    span(class="depth-line") {}
                                    span(class="depth-fill") {}
                                    span(class="depth-capsule") { (icon("deep")) }
                                    span(class="depth-reading") { (move || format!("{}m", stage_depth(&state.stage.get_clone(), &selected_mode.get_clone()))) }
                                }
                                div(class="progress-steps") {
                                    ProgressStep(
                                        index="01",
                                        code="SEAL",
                                        label="Sakana 연결",
                                        active=MaybeDyn::from(move || stage_progress.get() >= 0),
                                        current=MaybeDyn::from(move || state.stage.get_clone() == "connecting")
                                    )
                                    ProgressStep(
                                        index="02",
                                        code=if is_creative.get() { "SPARK" } else { "SONAR" },
                                        label=if is_creative.get() { "아이디어 발상" } else { "웹 자료 검색" },
                                        active=MaybeDyn::from(move || stage_progress.get() >= 1),
                                        current=MaybeDyn::from(move || matches!(state.stage.get_clone().as_str(), "searching" | "creating"))
                                    )
                                    ProgressStep(
                                        index="03",
                                        code=if is_creative.get() { "FORM" } else { "CURRENT" },
                                        label=if is_creative.get() { "구성과 목소리" } else { "출처 비교" },
                                        active=MaybeDyn::from(move || stage_progress.get() >= 2),
                                        current=MaybeDyn::from(move || state.stage.get_clone() == "reasoning")
                                    )
                                    ProgressStep(
                                        index="04",
                                        code=if is_creative.get() { "INK" } else { "LIGHT" },
                                        label=if is_creative.get() { "창작물 작성" } else { "답변 작성" },
                                        active=MaybeDyn::from(move || stage_progress.get() >= 3),
                                        current=MaybeDyn::from(move || state.stage.get_clone() == "writing")
                                    )
                                }
                            }
                        }
                    } else {
                        view! {
                            div(class="stream-reading-label") {
                                span { "FINDINGS ILLUMINATING" }
                                span { (move || format!("{:04} M", stage_depth(&state.stage.get_clone(), &selected_mode.get_clone()))) }
                            }
                            (move || {
                                let html = render_streaming_markdown(&state.streamed_text.get_clone());
                                view! {
                                    div(
                                        class="message-body markdown-body illuminated",
                                        on:click=move |event| open_markdown_link(state, event),
                                        dangerously_set_inner_html=html
                                    )
                                }
                            })
                            span(class="typing-cursor", aria-hidden="true") {}
                            span(class="sr-only", role="status", aria-live="polite") { "답변을 작성하고 있습니다." }
                        }
                    })
                }
            }
        } else {
            View::default()
        })
    }
}

#[derive(Props)]
struct ProgressStepProps {
    index: &'static str,
    code: &'static str,
    label: &'static str,
    active: MaybeDyn<bool>,
    current: MaybeDyn<bool>,
}

#[component]
fn ProgressStep(props: ProgressStepProps) -> View {
    let active_for_class = props.active.clone();
    let active_for_icon = props.active;
    let current_for_class = props.current.clone();
    let current_for_aria = props.current;
    view! {
        div(
            class=move || format!("progress-step {} {}", if active_for_class.get() { "active" } else { "" }, if current_for_class.get() { "current" } else { "" }),
            aria-current=move || if current_for_aria.get() { "step" } else { "false" }
        ) {
            span(class="step-index") { (props.index) }
            span(class="step-dot") { (if active_for_icon.get() { icon("check") } else { View::default() }) }
            span(class="step-copy") {
                small { (props.code) }
                strong { (props.label) }
            }
        }
    }
}

#[component]
fn RetryBanner() -> View {
    let state = use_context::<AppState>();
    view! {
        (if !state.is_running.get() && !state.last_failed_question.with(String::is_empty) {
            view! {
                div(class=move || format!("retry-banner {}", state.stage.get_clone()), role="status") {
                    span(class="retry-signal", aria-hidden="true") {}
                    span { (move || if state.stage.get_clone() == "cancelled" { "답변 생성을 중단했습니다." } else { "연결이 불안정해 답변을 끝까지 작성하지 못했습니다." }) }
                    button(on:click=move |_| state.retry_question()) { (icon("retry")) "다시 시도" }
                }
            }
        } else {
            View::default()
        })
    }
}

#[component]
fn Composer() -> View {
    let state = use_context::<AppState>();
    let input_ref = create_node_ref();
    let can_send = create_selector(move || {
        !state.composer.with(|value| value.trim().is_empty())
            && state.storage_writable.get()
            && !state.is_running.get()
    });
    let model = create_memo(move || {
        state
            .workspace
            .with(|workspace| workspace.settings.model.clone())
    });
    let reasoning = create_memo(move || {
        state
            .workspace
            .with(|workspace| workspace.settings.reasoning.clone())
    });
    let selected_mode = create_memo(move || {
        state
            .workspace
            .with(|workspace| workspace.settings.last_mode.clone())
    });

    on_mount(move || {
        reset_viewport_scroll();
        if is_mobile_viewport() {
            return;
        }
        if let Some(input) = input_ref
            .try_get()
            .and_then(|node| node.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let _ = input.focus();
        }
    });

    create_effect(on(state.is_running, move || {
        if !state.is_running.get_untracked() && !is_mobile_viewport() {
            let input_ref = input_ref;
            sycamore::web::queue_microtask(move || {
                if let Some(input) = input_ref
                    .try_get()
                    .and_then(|node| node.dyn_into::<web_sys::HtmlElement>().ok())
                {
                    let _ = input.focus();
                }
            });
        }
    }));

    let submit = move |event: SubmitEvent| {
        event.prevent_default();
        state.send_question();
    };
    let keydown = move |event: KeyboardEvent| {
        if event.key() == "Enter" && !event.shift_key() && !event.is_composing() {
            event.prevent_default();
            state.send_question();
        }
    };

    view! {
        form(
            class=move || format!(
                "composer-wrap mode-{} {} {}",
                selected_mode.get_clone(),
                if state.storage_writable.get() { "" } else { "read-only" },
                if state.is_running.get() { "is-running" } else { "" }
            ),
            on:submit=submit
        ) {
            (if state.is_running.get() {
                view! {
                    div(class="mobile-dive-control", role="status", aria-live="polite") {
                        span(class="mobile-dive-signal", aria-hidden="true") {}
                        div {
                            small { "LIVE DIVE" }
                            strong { (move || stage_label(&state.stage.get_clone(), &selected_mode.get_clone())) }
                        }
                        span(class="mobile-dive-depth") { (move || format!("{:04} M", stage_depth(&state.stage.get_clone(), &selected_mode.get_clone()))) }
                        button(
                            r#type="button",
                            class="mobile-stop-button",
                            aria-label="답변 생성 중지",
                            on:click=move |_| state.cancel_request()
                        ) {
                            (icon("stop"))
                            span { "답변 중단" }
                        }
                    }
                }
            } else {
                View::default()
            })
            div(class="capsule-rail") {
                div(class="mode-tabs", role="radiogroup", aria-label="답변 방식") {
                    ModeButton(value="quick", index="01", label="빠른 답변", detail="웹 검색 없이", icon_name="spark")
                    ModeButton(value="search", index="02", label="웹 검색", detail="출처와 함께", icon_name="globe")
                    ModeButton(value="deep", index="03", label="심층 조사", detail="넓고 깊게", icon_name="deep")
                    ModeButton(value="create", index="04", label="창작", detail="글·대사·아이디어", icon_name="create")
                }
                div(class="capsule-depth", aria-live="polite") {
                    small { "DIVE PROFILE" }
                    strong { (move || mode_depth(&selected_mode.get_clone())) }
                }
            }
            div(class="composer") {
                div(class="capsule-seal", aria-hidden="true") {
                    span {}
                    (move || if selected_mode.get_clone() == "create" { "CREATIVE CAPSULE" } else { "RESEARCH CAPSULE" })
                    span {}
                }
                label(class="sr-only", r#for="question-input") { "질문 입력" }
                textarea(
                    r#ref=input_ref,
                    id="question-input",
                    bind:value=state.composer,
                    on:keydown=keydown,
                    placeholder=move || if selected_mode.get_clone() == "create" { "어떤 이야기를 함께 만들어 볼까요?" } else { "무엇을 깊이 알아볼까요?" },
                    rows="1",
                    maxlength="20000",
                    disabled=move || state.is_running.get() || !state.storage_writable.get()
                ) {}
                div(class="composer-bottom") {
                    div(class="model-controls") {
                        label {
                            span(class="sr-only") { "Fugu 모델" }
                            select(disabled=move || !state.storage_writable.get(), on:change=move |event: Event| {
                                if let Some(value) = select_value(event) {
                                    state.workspace.update(|workspace| workspace.settings.model = value);
                                    state.persist_workspace();
                                }
                            }) {
                                option(value="fugu", selected=move || model.get_clone() == "fugu") { "Fugu" }
                                option(value="fugu-ultra", selected=move || model.get_clone() != "fugu") { "Fugu Ultra" }
                            }
                        }
                        span(class="control-divider") {}
                        label {
                            span(class="sr-only") { "추론 강도" }
                            select(disabled=move || !state.storage_writable.get(), on:change=move |event: Event| {
                                if let Some(value) = select_value(event) {
                                    state.workspace.update(|workspace| workspace.settings.reasoning = value);
                                    state.persist_workspace();
                                }
                            }) {
                                option(value="high", selected=move || reasoning.get_clone() == "high") { "High" }
                                option(value="xhigh", selected=move || reasoning.get_clone() == "xhigh") { "X-High" }
                                option(value="max", selected=move || reasoning.get_clone() == "max") { "Max" }
                            }
                        }
                    }
                    (if state.is_running.get() {
                        view! { button(class="send-button stop", r#type="button", aria-label="답변 생성 중지", on:click=move |_| state.cancel_request()) { (icon("stop")) } }
                    } else {
                        view! { button(class="send-button", r#type="submit", aria-label="질문 보내기", disabled=move || !can_send.get()) { (icon("send")) } }
                    })
                }
            }
            p(class=move || format!("composer-hint {}", if state.storage_writable.get() { "" } else { "storage-error" }), role=move || if state.storage_writable.get() { "note" } else { "status" }) {
                (move || match state.save_state.get_clone().as_str() {
                    "saving" => {
                        "대화 기록을 안전하게 저장하는 중입니다. 완료되면 입력을 다시 사용할 수 있습니다."
                    }
                    value if value.starts_with("error:") => {
                        "대화 기록을 저장하지 못했습니다. 앱을 다시 열어 최신 기록을 확인해 주세요."
                    }
                    _ if !state.storage_writable.get() => {
                        "저장된 대화를 복구해야 해서 현재 읽기 전용입니다. 새 질문을 보내거나 기록을 저장할 수 없습니다."
                    }
                    _ if selected_mode.get_clone() == "create" => {
                        "Enter로 전송 · Shift+Enter로 줄바꿈 · 장르, 분위기, 길이, 독자를 알려주면 더 정교하게 만들 수 있어요"
                    }
                    _ => {
                        "Enter로 전송 · Shift+Enter로 줄바꿈 · 출처는 반드시 원문에서 다시 확인하세요"
                    }
                })
            }
        }
    }
}

#[derive(Props)]
struct ModeButtonProps {
    value: &'static str,
    index: &'static str,
    label: &'static str,
    detail: &'static str,
    icon_name: &'static str,
}

#[component]
fn ModeButton(props: ModeButtonProps) -> View {
    let state = use_context::<AppState>();
    let selected = create_selector(move || {
        state
            .workspace
            .with(|workspace| workspace.settings.last_mode == props.value)
    });
    view! {
        button(
            r#type="button",
            role="radio",
            aria-checked=move || selected.get().to_string(),
            class=move || if selected.get() { "active" } else { "" },
            disabled=move || !state.storage_writable.get(),
            on:click=move |_| {
                state.workspace.update(|workspace| workspace.settings.last_mode = props.value.into());
                state.persist_workspace();
            }
        ) {
            span(class="mode-index") { (props.index) }
            span(class="mode-icon") { (icon(props.icon_name)) }
            span(class="mode-copy") {
                strong { (props.label) }
                small { (props.detail) }
            }
        }
    }
}

#[component]
fn SourcesPanel() -> View {
    let state = use_context::<AppState>();
    let sources_empty = create_selector(move || state.selected_sources.with(Vec::is_empty));
    view! {
        aside(class=move || format!("sources-panel {}", if state.panel.get() == Panel::Sources { "visible" } else { "" }), role="dialog", aria-modal="true", aria-hidden=move || (state.panel.get() != Panel::Sources).to_string(), aria-label="출처") {
            div(class="panel-header") {
                div { small { "EVIDENCE DECK" } h2 { "검색·인용 출처" } }
                button(class="icon-button", aria-label="출처 패널 닫기", on:click=move |_| state.close_panel()) { (icon("close")) }
            }
            (if sources_empty.get() {
                view! {
                    div(class="sources-empty") {
                        span(class="source-rings") { (icon("sources")) }
                        h3 { "아직 출처가 없습니다" }
                        p { "웹 검색이나 심층 조사로 질문하면 Sakana Fugu가 참고한 자료를 여기에 모아 보여 줍니다. 빠른 답변과 창작 모드는 웹을 검색하지 않습니다." }
                    }
                }
            } else {
                view! {
                    div(class="source-list") {
                        Indexed(
                            list=state.selected_sources,
                            view=move |source: Source| view! { SourceView(source=source) }
                        )
                    }
                }
            })
        }
    }
}

#[derive(Props)]
struct SourceViewProps {
    source: Source,
}

#[component]
fn SourceView(props: SourceViewProps) -> View {
    let state = use_context::<AppState>();
    let source = props.source;
    let index = state
        .selected_sources
        .with_untracked(|sources| sources.iter().position(|item| item.id == source.id))
        .unwrap_or(0)
        + 1;
    let url = source.url;
    let snippet = if source.snippet.is_empty() {
        View::default()
    } else {
        view! { p { (source.snippet) } }
    };
    view! {
        article(class="source-card") {
            div(class="source-index") {
                span { (format!("{index:02}")) }
                i(aria-hidden="true") {}
            }
            div(class="source-content") {
                small { "SPECIMEN · " (source.domain) }
                h3 { (source.title) }
                (snippet)
                button(on:click=move |_| open_url(state, url.clone())) { "원문 열기" (icon("external")) }
            }
        }
    }
}

#[component]
fn SettingsPanel() -> View {
    let state = use_context::<AppState>();
    let has_active_conversation = create_selector(move || !state.active_id.with(String::is_empty));
    let active_conversation_id = create_memo(move || state.active_id.get_clone());
    view! {
        aside(class=move || format!("settings-panel {}", if state.panel.get() == Panel::Settings { "visible" } else { "" }), role="dialog", aria-modal="true", aria-hidden=move || (state.panel.get() != Panel::Settings).to_string(), aria-label="설정") {
            div(class="panel-header") {
                div { small { "CONTROL ROOM" } h2 { "설정" } }
                button(class="icon-button", aria-label="설정 닫기", on:click=move |_| state.close_panel()) { (icon("close")) }
            }
            div(class="settings-content") {
                section(class="setting-section") {
                    div(class="setting-title") { span(class="setting-number") { "01" } div { h3 { "Sakana API" } p { "키는 운영체제 보안 저장소에 보관되며 앱 시작 시 자동 복원됩니다." } } }
                    (if state.key_configured.get() {
                        view! {
                            div(class="key-connected") {
                                span { (icon("check")) }
                                div {
                                    strong { "Sakana Fugu 준비 완료" }
                                    small { (if state.connection_message.with(String::is_empty) {
                                        "이 기기의 보안 저장소에 저장됨".into()
                                    } else {
                                        state.connection_message.get_clone()
                                    }) }
                                }
                                button(on:click=move |_| state.clear_key()) { "연결 해제" }
                            }
                        }
                    } else {
                        view! {
                            form(class="key-form", on:submit=move |event: SubmitEvent| {
                                event.prevent_default();
                                state.connect_key();
                            }) {
                                label(r#for="api-key") { "Sakana API key" }
                                div(class="key-input-row") {
                                    input(id="api-key", r#type="password", bind:value=state.key_input, autocomplete="off", placeholder="키 붙여넣기", disabled=state.key_busy)
                                    button(r#type="submit", disabled=move || state.key_busy.get() || state.key_input.with(|value| value.trim().is_empty())) {
                                        (move || if state.key_busy.get() { "확인 중…" } else { "연결" })
                                    }
                                }
                                p { "키는 대화 기록이나 브라우저 저장소, 로그에 남기지 않고 운영체제 보안 저장소에만 보관합니다." }
                            }
                        }
                    })
                }

                section(class="setting-section") {
                    div(class="setting-title") { span(class="setting-number") { "02" } div { h3 { "화면" } p { "사용 환경에 맞는 화면 밝기를 선택하세요." } } }
                    div(class="segmented-control") {
                        ThemeButton(value="system", label="시스템")
                        ThemeButton(value="light", label="라이트")
                        ThemeButton(value="dark", label="다크")
                    }
                }

                section(class="setting-section caution") {
                    div(class="setting-title") { span(class="setting-number") { "03" } div { h3 { "데이터와 개인정보" } p { "대화 기록은 이 기기에만 저장합니다. 답변을 만들 때는 질문과 대화 내용을 Sakana로 전송합니다." } } }
                    ul {
                        li { "개인정보·건강·금융·회사 기밀을 입력하지 마세요." }
                        li { "Sakana의 보존·학습 설정과 약관을 배포 전에 확인하세요." }
                        li { "기기 간 동기화는 아직 제공하지 않습니다." }
                    }
                    button(class="policy-link", on:click=move |_| open_url(state, "https://console.sakana.ai/privacy-policy".into())) { "Sakana 개인정보 정책" (icon("external")) }
                }

                (if has_active_conversation.get() {
                    view! {
                        section(class="setting-section conversation-tools") {
                            h3 { "현재 대화" }
                            div(class="tool-row") {
                                button(disabled=move || !state.storage_writable.get(), on:click=move |_| state.toggle_pin()) { (icon("pin")) "고정 전환" }
                                button(on:click=move |_| state.export_current()) { (icon("export")) "Markdown 내보내기" }
                                button(
                                    class="danger",
                                    disabled=move || {
                                        state.is_running.get()
                                            || state.persistence_busy.get()
                                            || !state.storage_writable.get()
                                    },
                                    on:click=move |_| state.delete_conversation(active_conversation_id.get_clone())
                                ) { (icon("trash")) "삭제" }
                            }
                        }
                    }
                } else {
                    View::default()
                })
            }
        }
    }
}

#[derive(Props)]
struct ThemeButtonProps {
    value: &'static str,
    label: &'static str,
}

#[component]
fn ThemeButton(props: ThemeButtonProps) -> View {
    let state = use_context::<AppState>();
    let selected = create_selector(move || {
        state
            .workspace
            .with(|workspace| workspace.settings.theme == props.value)
    });
    view! {
        button(
            class=move || if selected.get() { "active" } else { "" },
            disabled=move || !state.storage_writable.get(),
            on:click=move |_| {
                update_theme(props.value);
                state.workspace.update(|workspace| workspace.settings.theme = props.value.into());
                state.persist_workspace();
            }
        ) { (props.label) }
    }
}

#[component]
fn OverlayLayer() -> View {
    let state = use_context::<AppState>();
    let has_panel = create_selector(move || state.panel.get() != Panel::None);
    let has_toast = create_selector(move || !state.toast.with(String::is_empty));
    view! {
        (if has_panel.get() {
            view! { button(class="scrim", aria-label="패널 닫기", on:click=move |_| state.close_panel()) {} }
        } else {
            View::default()
        })
        (if has_toast.get() {
            view! {
                div(class=move || format!("toast {}", state.toast_kind.get_clone()), role="status", aria-live="polite") {
                    span { (state.toast) }
                    button(aria-label="알림 닫기", on:click=move |_| state.toast.set(String::new())) { (icon("close")) }
                }
            }
        } else {
            View::default()
        })
    }
}
