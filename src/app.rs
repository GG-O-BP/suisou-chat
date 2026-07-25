use crate::icons::icon;
use crate::ipc;
use crate::models::{
    format_relative_time, new_id, now_millis, title_from_question, BootstrapResponse,
    ConnectionInfo, Conversation, InputMessage, Message, ResearchEvent, ResearchRequest,
    ResearchResponse, Source, Workspace,
};
use serde::Serialize;
use sycamore::futures::spawn_local_scoped;
use sycamore::prelude::*;
use sycamore::web::events::{Event, KeyboardEvent, SubmitEvent};
use wasm_bindgen::JsCast;

#[derive(Clone, Copy, PartialEq)]
enum Panel {
    None,
    Sidebar,
    Sources,
    Settings,
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

fn persist_workspace(workspace: Signal<Workspace>, save_state: Signal<String>) {
    let snapshot = workspace.get_clone();
    save_state.set("saving".into());
    spawn_local_scoped(async move {
        match ipc::command::<_, u64>(
            "save_workspace",
            &WorkspaceArgs {
                workspace: snapshot,
            },
        )
        .await
        {
            Ok(revision) => {
                workspace.update(|value| value.revision = revision);
                save_state.set("saved".into());
            }
            Err(error) => {
                save_state.set(format!("error:{error}"));
            }
        }
    });
}

fn show_toast(message: String, kind: &str, toast: Signal<String>, toast_kind: Signal<String>) {
    toast.set(message);
    toast_kind.set(kind.into());
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

fn current_conversation(workspace: &Workspace, active_id: &str) -> Option<Conversation> {
    workspace
        .conversations
        .iter()
        .find(|conversation| conversation.id == active_id)
        .cloned()
}

fn source_list(workspace: &Workspace, active_id: &str) -> Vec<Source> {
    current_conversation(workspace, active_id)
        .map(|conversation| {
            conversation
                .messages
                .into_iter()
                .rev()
                .find(|message| message.role == "assistant" && !message.sources.is_empty())
                .map(|message| message.sources)
                .unwrap_or_default()
        })
        .unwrap_or_default()
}

fn set_active_conversation(
    id: String,
    active_id: Signal<String>,
    selected_sources: Signal<Vec<Source>>,
    panel: Signal<Panel>,
    workspace: Signal<Workspace>,
) {
    active_id.set(id.clone());
    selected_sources.set(source_list(&workspace.get_clone(), &id));
    panel.set(Panel::None);
}

fn new_conversation(
    active_id: Signal<String>,
    selected_sources: Signal<Vec<Source>>,
    composer: Signal<String>,
    panel: Signal<Panel>,
) {
    active_id.set(String::new());
    selected_sources.set(Vec::new());
    composer.set(String::new());
    panel.set(Panel::None);
}

fn delete_conversation(
    active_id: Signal<String>,
    workspace: Signal<Workspace>,
    selected_sources: Signal<Vec<Source>>,
    save_state: Signal<String>,
) {
    let id = active_id.get_clone();
    if id.is_empty() {
        return;
    }
    workspace.update(|value| {
        value
            .conversations
            .retain(|conversation| conversation.id != id)
    });
    active_id.set(String::new());
    selected_sources.set(Vec::new());
    persist_workspace(workspace, save_state);
}

fn toggle_pin(active_id: Signal<String>, workspace: Signal<Workspace>, save_state: Signal<String>) {
    let id = active_id.get_clone();
    workspace.update(|value| {
        if let Some(conversation) = value
            .conversations
            .iter_mut()
            .find(|conversation| conversation.id == id)
        {
            conversation.pinned = !conversation.pinned;
            conversation.updated_at = now_millis();
        }
    });
    persist_workspace(workspace, save_state);
}

#[allow(clippy::too_many_arguments)]
fn send_question(
    composer: Signal<String>,
    active_id: Signal<String>,
    workspace: Signal<Workspace>,
    key_configured: Signal<bool>,
    is_running: Signal<bool>,
    active_request: Signal<String>,
    stage: Signal<String>,
    streamed_text: Signal<String>,
    selected_sources: Signal<Vec<Source>>,
    last_failed_question: Signal<String>,
    save_state: Signal<String>,
    panel: Signal<Panel>,
    toast: Signal<String>,
    toast_kind: Signal<String>,
) {
    let question = composer.get_clone().trim().to_owned();
    if question.is_empty() || is_running.get() {
        return;
    }
    if question.chars().count() > 20_000 {
        show_toast(
            "질문은 20,000자 이하로 입력해 주세요.".into(),
            "error",
            toast,
            toast_kind,
        );
        return;
    }
    if !key_configured.get() {
        panel.set(Panel::Settings);
        show_toast(
            "먼저 설정에서 Sakana API 키를 연결해 주세요.".into(),
            "warning",
            toast,
            toast_kind,
        );
        return;
    }
    let active = current_conversation(&workspace.get_clone(), &active_id.get_clone());
    let prior_messages = active
        .as_ref()
        .map(|value| value.messages.len())
        .unwrap_or(0);
    let prior_chars = active
        .as_ref()
        .map(|value| {
            value
                .messages
                .iter()
                .map(|message| message.content.chars().count())
                .sum()
        })
        .unwrap_or(0usize);
    if prior_messages >= 199 || prior_chars.saturating_add(question.chars().count()) > 500_000 {
        show_toast(
            "대화 문맥 한도에 도달했습니다. 새 탐구에서 질문을 이어가 주세요.".into(),
            "warning",
            toast,
            toast_kind,
        );
        return;
    }

    let timestamp = now_millis();
    let conversation_id = if active_id.get_clone().is_empty() {
        let id = new_id("conversation");
        workspace.update(|value| {
            value.conversations.push(Conversation {
                id: id.clone(),
                title: title_from_question(&question),
                pinned: false,
                created_at: timestamp,
                updated_at: timestamp,
                messages: Vec::new(),
            });
        });
        active_id.set(id.clone());
        id
    } else {
        active_id.get_clone()
    };

    workspace.update(|value| {
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

    composer.set(String::new());
    last_failed_question.set(String::new());
    streamed_text.set(String::new());
    selected_sources.set(Vec::new());
    stage.set("connecting".into());
    is_running.set(true);
    let request_id = new_id("request");
    active_request.set(request_id.clone());
    persist_workspace(workspace, save_state);

    let snapshot = workspace.get_clone();
    let conversation = current_conversation(&snapshot, &conversation_id).unwrap_or_default();
    let request = ResearchRequest {
        request_id: request_id.clone(),
        model: snapshot.settings.model.clone(),
        mode: snapshot.settings.last_mode.clone(),
        reasoning: snapshot.settings.reasoning.clone(),
        messages: conversation
            .messages
            .iter()
            .map(|message| InputMessage {
                role: message.role.clone(),
                content: message.content.clone(),
            })
            .collect(),
    };

    spawn_local_scoped(async move {
        let result =
            ipc::command::<_, ResearchResponse>("run_research", &ResearchArgs { request }).await;
        if active_request.get_clone() != request_id {
            return;
        }
        is_running.set(false);
        active_request.set(String::new());
        match result {
            Ok(response) => {
                workspace.update(|value| {
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
                selected_sources.set(response.sources);
                streamed_text.set(String::new());
                stage.set("done".into());
                persist_workspace(workspace, save_state);
            }
            Err(error) => {
                let partial = streamed_text.get_clone();
                let status = if error.contains("중단") {
                    "cancelled"
                } else {
                    "failed"
                };
                if !partial.trim().is_empty() {
                    workspace.update(|value| {
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
                    persist_workspace(workspace, save_state);
                }
                stage.set(status.into());
                streamed_text.set(String::new());
                last_failed_question.set(question);
                let mut error = error;
                if error.contains("인증") || error.contains("API 키") {
                    if let Err(clear_error) =
                        ipc::command_unit("clear_api_key", &EmptyArgs {}).await
                    {
                        error = format!("{error} {clear_error}");
                        let _ = ipc::command_unit("forget_api_key", &EmptyArgs {}).await;
                    }
                    key_configured.set(false);
                    panel.set(Panel::Settings);
                }
                show_toast(error, "error", toast, toast_kind);
            }
        }
    });
}

#[component]
pub fn App() -> View {
    let workspace = create_signal(Workspace::default());
    let active_id = create_signal(String::new());
    let composer = create_signal(String::new());
    let search_query = create_signal(String::new());
    let selected_sources = create_signal(Vec::<Source>::new());
    let panel = create_signal(Panel::None);
    let is_loading = create_signal(true);
    let is_running = create_signal(false);
    let active_request = create_signal(String::new());
    let stage = create_signal(String::new());
    let streamed_text = create_signal(String::new());
    let key_configured = create_signal(false);
    let key_input = create_signal(String::new());
    let key_busy = create_signal(false);
    let connection_message = create_signal(String::new());
    let save_state = create_signal(String::new());
    let last_failed_question = create_signal(String::new());
    let toast = create_signal(String::new());
    let toast_kind = create_signal(String::from("info"));
    let storage_label = create_signal(String::from("이 기기에만 저장됨"));
    let storage_writable = create_signal(true);

    spawn_local_scoped(async move {
        let stream_result = ipc::listen::<ResearchEvent, _>("research-event", move |event| {
            if event.request_id != active_request.get_clone() {
                return;
            }
            match event.kind.as_str() {
                "delta" => streamed_text.update(|value| value.push_str(&event.value)),
                "stage" => stage.set(event.value),
                _ => {}
            }
        })
        .await;
        if let Err(error) = stream_result {
            show_toast(error, "error", toast, toast_kind);
        }
    });

    spawn_local_scoped(async move {
        match ipc::command::<_, BootstrapResponse>("bootstrap", &EmptyArgs {}).await {
            Ok(response) => {
                update_theme(&response.workspace.settings.theme);
                let first_id = response
                    .workspace
                    .conversations
                    .iter()
                    .max_by_key(|conversation| conversation.updated_at)
                    .map(|conversation| conversation.id.clone())
                    .unwrap_or_default();
                selected_sources.set(source_list(&response.workspace, &first_id));
                active_id.set(first_id);
                key_configured.set(response.key_configured);
                if response.key_configured {
                    connection_message.set("보안 저장소에서 자동 복원됨".into());
                }
                storage_label.set(response.storage_label);
                storage_writable.set(response.storage_writable);
                workspace.set(response.workspace);
                let notices = [response.recovery_notice, response.credential_notice]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>();
                if !notices.is_empty() {
                    show_toast(notices.join(" "), "warning", toast, toast_kind);
                }
            }
            Err(error) => show_toast(error, "error", toast, toast_kind),
        }
        is_loading.set(false);
    });

    let submit = move |event: SubmitEvent| {
        event.prevent_default();
        send_question(
            composer,
            active_id,
            workspace,
            key_configured,
            is_running,
            active_request,
            stage,
            streamed_text,
            selected_sources,
            last_failed_question,
            save_state,
            panel,
            toast,
            toast_kind,
        );
    };

    let composer_keydown = move |event: KeyboardEvent| {
        if event.key() == "Escape" && panel.get() != Panel::None {
            panel.set(Panel::None);
            return;
        }
        if event.key() == "Enter" && !event.shift_key() && !event.is_composing() {
            event.prevent_default();
            send_question(
                composer,
                active_id,
                workspace,
                key_configured,
                is_running,
                active_request,
                stage,
                streamed_text,
                selected_sources,
                last_failed_question,
                save_state,
                panel,
                toast,
                toast_kind,
            );
        }
    };

    view! {
        div(class=move || format!("app-shell {}", if panel.get() != Panel::None { "panel-open" } else { "" })) {
            a(class="skip-link", href="#main-content") { "본문으로 건너뛰기" }

            aside(class=move || format!("sidebar {}", if panel.get() == Panel::Sidebar { "visible" } else { "" }), aria-label="탐구 기록") {
                div(class="brand") {
                    div(class="brand-mark", aria-hidden="true") {
                        span(class="water-line") {}
                        span(class="fugu-dot dot-one") {}
                        span(class="fugu-dot dot-two") {}
                        span(class="fugu-dot dot-three") {}
                    }
                    div {
                        strong { "SUISOU" }
                        small { "RESEARCH COMPANION" }
                    }
                    button(class="icon-button mobile-only", aria-label="메뉴 닫기", on:click=move |_| panel.set(Panel::None)) { (icon("close")) }
                }

                button(class="new-research", disabled=move || is_running.get() || !storage_writable.get(), on:click=move |_| new_conversation(active_id, selected_sources, composer, panel)) {
                    (icon("plus"))
                    span { "새로운 탐구" }
                    kbd { "⌘ N" }
                }

                label(class="history-search") {
                    span(class="sr-only") { "대화 기록 검색" }
                    (icon("search"))
                    input(bind:value=search_query, placeholder="기록에서 검색", autocomplete="off")
                }

                nav(class="history-list", aria-label="저장된 탐구") {
                    (if workspace.get_clone().conversations.is_empty() {
                        view! {
                            div(class="history-empty") {
                                span(class="empty-ripple") {}
                                p { "첫 질문이 여기에 기록됩니다." }
                            }
                        }
                    } else {
                        view! {
                            Indexed(
                                list=move || {
                                    let query = search_query.get_clone().to_lowercase();
                                    let mut values = workspace.get_clone().conversations;
                                    values.retain(|conversation| {
                                        query.is_empty()
                                            || conversation.title.to_lowercase().contains(&query)
                                            || conversation.messages.iter().any(|message| message.content.to_lowercase().contains(&query))
                                    });
                                    values.sort_by_key(|conversation| (!conversation.pinned, std::cmp::Reverse(conversation.updated_at)));
                                    values
                                },
                                view=move |conversation: Conversation| {
                                    let id = conversation.id.clone();
                                    let class_id = id.clone();
                                    view! {
                                        button(
                                            class=move || format!("history-item {}", if active_id.get_clone() == class_id { "active" } else { "" }),
                                            aria-current=move || if active_id.get_clone() == id { "page" } else { "false" },
                                            disabled=is_running.get(),
                                            on:click=move |_| set_active_conversation(conversation.id.clone(), active_id, selected_sources, panel, workspace)
                                        ) {
                                            span(class="history-glyph") { (if conversation.pinned { icon("pin") } else { icon("search") }) }
                                            span(class="history-copy") {
                                                strong { (conversation.title.clone()) }
                                                small { (format_relative_time(conversation.updated_at)) }
                                            }
                                        }
                                    }
                                }
                            )
                        }
                    })
                }

                div(class="sidebar-footer") {
                    div(class="storage-status") {
                        span(class="status-light") {}
                        div {
                            strong { (storage_label) }
                            small { (move || match save_state.get_clone().as_str() {
                                "saving" => "저장 중…",
                                value if value.starts_with("error:") => "저장 오류",
                                _ => "오프라인에서도 기록 열람 가능",
                            }) }
                        }
                    }
                    button(class="settings-button", on:click=move |_| panel.set(Panel::Settings)) {
                        (icon("settings"))
                        "설정"
                    }
                }
            }

            main(id="main-content", class="workspace") {
                header(class="topbar") {
                    div(class="topbar-start") {
                        button(class="icon-button mobile-only", aria-label="탐구 기록 열기", on:click=move |_| panel.set(Panel::Sidebar)) { (icon("menu")) }
                        div(class="conversation-heading") {
                            small { "CURRENT DIVE" }
                            strong { (move || current_conversation(&workspace.get_clone(), &active_id.get_clone()).map(|value| value.title).unwrap_or_else(|| "새로운 탐구".into())) }
                        }
                    }
                    div(class="topbar-actions") {
                        span(class=move || format!("connection-pill {}", if key_configured.get() { "connected" } else { "disconnected" })) {
                            span(class="connection-dot") {}
                            (move || if key_configured.get() { "Fugu 연결됨" } else { "API 키 필요" })
                        }
                        button(class="icon-button", aria-label="출처 패널 열기", on:click=move |_| panel.set(Panel::Sources)) {
                            (icon("sources"))
                            (if !selected_sources.get_clone().is_empty() {
                                view! { span(class="count-badge") { (selected_sources.get_clone().len()) } }
                            } else { View::default() })
                        }
                    }
                }

                section(class="transcript", aria-label="대화") {
                    (if is_loading.get() {
                        view! {
                            div(class="loading-state", role="status") {
                                span(class="sonar-loader") {}
                                p { "작업 공간을 여는 중…" }
                            }
                        }
                    } else if active_id.get_clone().is_empty() {
                        view! {
                            section(class="welcome") {
                                div(class="welcome-orbit", aria-hidden="true") {
                                    span(class="orbit orbit-one") {}
                                    span(class="orbit orbit-two") {}
                                    div(class="fugu-core") { (icon("spark")) }
                                }
                                p(class="eyebrow") { "DIVE PAST THE OBVIOUS" }
                                h1 { "질문 아래의 " em { "근거" } "까지." }
                                p(class="welcome-copy") { "Sakana Fugu의 다중 에이전트 추론으로 웹을 교차 검증하고, 답보다 오래 남는 연구 기록을 만듭니다." }
                                div(class="suggestion-grid") {
                                    button(on:click=move |_| composer.set("이번 주 AI 에이전트 분야의 주요 발표를 출처별로 교차 검증해 줘".into())) {
                                        span(class="suggestion-icon coral") { (icon("globe")) }
                                        span { strong { "이번 주의 흐름" } small { "AI 에이전트 주요 발표 교차 검증" } }
                                    }
                                    button(on:click=move |_| composer.set("한국과 일본의 생성형 AI 정책을 공식 자료 중심으로 비교해 줘".into())) {
                                        span(class="suggestion-icon blue") { (icon("deep")) }
                                        span { strong { "정책 비교" } small { "공식 자료의 차이와 공통점" } }
                                    }
                                    button(on:click=move |_| composer.set("이 주장의 찬반 근거를 찾아 신뢰도와 한계를 표로 정리해 줘: ".into())) {
                                        span(class="suggestion-icon gold") { (icon("search")) }
                                        span { strong { "주장 검증" } small { "찬반 근거와 신뢰도 평가" } }
                                    }
                                }
                                p(class="privacy-note") { (icon("key")) " 질문은 Sakana로 전송됩니다. 개인정보·기밀은 입력하지 마세요." }
                            }
                        }
                    } else {
                        view! {
                            div(class="message-stack") {
                                Indexed(
                                    list=move || current_conversation(&workspace.get_clone(), &active_id.get_clone()).map(|value| value.messages).unwrap_or_default(),
                                    view=move |message: Message| message_view(message, selected_sources, panel, toast, toast_kind)
                                )

                                (if is_running.get() {
                                    view! {
                                        article(class="message assistant streaming", aria-live="polite") {
                                            div(class="message-meta") {
                                                span(class="role-mark sonar") { span {} }
                                                strong { "Suisou" }
                                                span(class="research-stage") { (stage_label(&stage.get_clone())) }
                                            }
                                            (if streamed_text.get_clone().is_empty() {
                                                view! {
                                                    div(class="research-progress") {
                                                        (progress_step("연결", stage_index(&stage.get_clone()) >= 0, stage.get_clone() == "connecting"))
                                                        (progress_step("검색", stage_index(&stage.get_clone()) >= 1, stage.get_clone() == "searching"))
                                                        (progress_step("검토", stage_index(&stage.get_clone()) >= 2, stage.get_clone() == "reasoning"))
                                                        (progress_step("작성", stage_index(&stage.get_clone()) >= 3, stage.get_clone() == "writing"))
                                                    }
                                                }
                                            } else {
                                                view! { div(class="message-body") { (streamed_text) span(class="typing-cursor") {} } }
                                            })
                                        }
                                    }
                                } else if !last_failed_question.get_clone().is_empty() {
                                    view! {
                                        div(class="retry-banner") {
                                            span { "답변 생성이 완료되지 않았습니다." }
                                            button(on:click=move |_| {
                                                composer.set(last_failed_question.get_clone());
                                                let failed_id = active_id.get_clone();
                                                workspace.update(|value| {
                                                    if let Some(conversation) = value.conversations.iter_mut().find(|conversation| conversation.id == failed_id) {
                                                        if conversation.messages.last().map(|message| message.status.as_str()) == Some("failed")
                                                            || conversation.messages.last().map(|message| message.status.as_str()) == Some("cancelled")
                                                        {
                                                            conversation.messages.pop();
                                                        }
                                                        if conversation.messages.last().map(|message| message.role.as_str()) == Some("user") { conversation.messages.pop(); }
                                                    }
                                                });
                                                persist_workspace(workspace, save_state);
                                                send_question(composer, active_id, workspace, key_configured, is_running, active_request, stage, streamed_text, selected_sources, last_failed_question, save_state, panel, toast, toast_kind);
                                            }) { (icon("retry")) "다시 시도" }
                                        }
                                    }
                                } else { View::default() })
                            }
                        }
                    })
                }

                form(class="composer-wrap", on:submit=submit) {
                    div(class="mode-tabs", role="radiogroup", aria-label="연구 방식") {
                        (mode_button("quick", "빠른 답변", "spark", workspace, save_state))
                        (mode_button("search", "웹 검색", "globe", workspace, save_state))
                        (mode_button("deep", "딥 리서치", "deep", workspace, save_state))
                    }
                    div(class="composer") {
                        label(class="sr-only", r#for="question-input") { "질문 입력" }
                        textarea(
                            id="question-input",
                            bind:value=composer,
                            on:keydown=composer_keydown,
                            placeholder="무엇을 깊이 알아볼까요?",
                            rows="1",
                            maxlength="20000",
                            disabled=move || is_running.get() || !storage_writable.get()
                        ) {}
                        div(class="composer-bottom") {
                            div(class="model-controls") {
                                label {
                                    span(class="sr-only") { "Fugu 모델" }
                                    select(on:change=move |event: Event| {
                                        if let Some(value) = select_value(event) {
                                            workspace.update(|state| state.settings.model = value);
                                            persist_workspace(workspace, save_state);
                                        }
                                    }) {
                                        option(value="fugu", selected=workspace.get_clone().settings.model == "fugu") { "Fugu" }
                                        option(value="fugu-ultra", selected=workspace.get_clone().settings.model != "fugu") { "Fugu Ultra" }
                                    }
                                }
                                span(class="control-divider") {}
                                label {
                                    span(class="sr-only") { "추론 강도" }
                                    select(on:change=move |event: Event| {
                                        if let Some(value) = select_value(event) {
                                            workspace.update(|state| state.settings.reasoning = value);
                                            persist_workspace(workspace, save_state);
                                        }
                                    }) {
                                        option(value="high", selected=workspace.get_clone().settings.reasoning == "high") { "High" }
                                        option(value="xhigh", selected=workspace.get_clone().settings.reasoning == "xhigh") { "X-High" }
                                        option(value="max", selected=workspace.get_clone().settings.reasoning == "max") { "Max" }
                                    }
                                }
                            }
                            (if is_running.get() {
                                view! { button(class="send-button stop", r#type="button", aria-label="답변 생성 중지", on:click=move |_| cancel_request(active_request, toast, toast_kind)) { (icon("stop")) } }
                            } else {
                                view! { button(class="send-button", r#type="submit", aria-label="질문 보내기", disabled=move || composer.get_clone().trim().is_empty() || !storage_writable.get()) { (icon("send")) } }
                            })
                        }
                    }
                    p(class="composer-hint") { "Enter로 전송 · Shift+Enter로 줄바꿈 · 출처는 반드시 원문에서 다시 확인하세요" }
                }
            }

            aside(class=move || format!("sources-panel {}", if panel.get() == Panel::Sources { "visible" } else { "" }), role="dialog", aria-modal="true", aria-hidden=move || (panel.get() != Panel::Sources).to_string(), aria-label="출처") {
                div(class="panel-header") {
                    div { small { "EVIDENCE DECK" } h2 { "검색·인용 출처" } }
                    button(class="icon-button", aria-label="출처 패널 닫기", on:click=move |_| panel.set(Panel::None)) { (icon("close")) }
                }
                (if selected_sources.get_clone().is_empty() {
                    view! {
                        div(class="sources-empty") {
                            span(class="source-rings") { (icon("sources")) }
                            h3 { "아직 출처가 없습니다" }
                            p { "웹 검색이나 딥 리서치로 질문하면 Fugu가 확인한 근거를 여기에 모읍니다." }
                        }
                    }
                } else {
                    view! {
                        div(class="source-list") {
                            Indexed(
                                list=selected_sources,
                                view=move |source: Source| {
                                    let index = selected_sources
                                        .get_clone()
                                        .iter()
                                        .position(|item| item.id == source.id)
                                        .unwrap_or(0)
                                        + 1;
                                    source_view(source, index, toast, toast_kind)
                                }
                            )
                        }
                    }
                })
            }

            aside(class=move || format!("settings-panel {}", if panel.get() == Panel::Settings { "visible" } else { "" }), role="dialog", aria-modal="true", aria-hidden=move || (panel.get() != Panel::Settings).to_string(), aria-label="설정") {
                div(class="panel-header") {
                    div { small { "CONTROL ROOM" } h2 { "설정" } }
                    button(class="icon-button", aria-label="설정 닫기", on:click=move |_| panel.set(Panel::None)) { (icon("close")) }
                }
                div(class="settings-content") {
                    section(class="setting-section") {
                        div(class="setting-title") { span(class="setting-number") { "01" } div { h3 { "Sakana API" } p { "키는 운영체제 보안 저장소에 보관되며 앱 시작 시 자동 복원됩니다." } } }
                        (if key_configured.get() {
                            view! {
                                div(class="key-connected") {
                                    span { (icon("check")) }
                                    div { strong { "Fugu 연결 준비 완료" } small { (if connection_message.get_clone().is_empty() { "이 기기의 보안 저장소에 저장됨".into() } else { connection_message.get_clone() }) } }
                                    button(on:click=move |_| clear_key(key_configured, connection_message, toast, toast_kind)) { "연결 해제" }
                                }
                            }
                        } else {
                            view! {
                                form(class="key-form", on:submit=move |event: SubmitEvent| {
                                    event.prevent_default();
                                    connect_key(key_input, key_busy, key_configured, connection_message, toast, toast_kind);
                                }) {
                                    label(r#for="api-key") { "Sakana API key" }
                                    div(class="key-input-row") {
                                        input(id="api-key", r#type="password", bind:value=key_input, autocomplete="off", placeholder="키 붙여넣기", disabled=key_busy.get())
                                        button(r#type="submit", disabled=move || key_busy.get() || key_input.get_clone().trim().is_empty()) { (move || if key_busy.get() { "확인 중…" } else { "연결" }) }
                                    }
                                    p { "키는 작업 공간 파일·브라우저 저장소·로그가 아닌 운영체제 보안 저장소에 기록됩니다." }
                                }
                            }
                        })
                    }

                    section(class="setting-section") {
                        div(class="setting-title") { span(class="setting-number") { "02" } div { h3 { "화면" } p { "환경과 선호에 맞는 명암을 선택합니다." } } }
                        div(class="segmented-control") {
                            (theme_button("system", "시스템", workspace, save_state))
                            (theme_button("light", "라이트", workspace, save_state))
                            (theme_button("dark", "다크", workspace, save_state))
                        }
                    }

                    section(class="setting-section caution") {
                        div(class="setting-title") { span(class="setting-number") { "03" } div { h3 { "데이터와 개인정보" } p { "대화 기록은 이 기기에 저장되지만, 질문과 문맥은 답변 생성을 위해 Sakana로 전송됩니다." } } }
                        ul {
                            li { "개인정보·건강·금융·회사 기밀을 입력하지 마세요." }
                            li { "Sakana의 보존·학습 설정과 약관을 배포 전에 확인하세요." }
                            li { "기기 간 동기화는 아직 제공하지 않습니다." }
                        }
                        button(class="policy-link", on:click=move |_| open_url("https://console.sakana.ai/privacy-policy".into(), toast, toast_kind)) { "Sakana 개인정보 정책" (icon("external")) }
                    }

                    (if !active_id.get_clone().is_empty() {
                        view! {
                            section(class="setting-section conversation-tools") {
                                h3 { "현재 대화" }
                                div(class="tool-row") {
                                    button(on:click=move |_| toggle_pin(active_id, workspace, save_state)) { (icon("pin")) "고정 전환" }
                                    button(on:click=move |_| export_current(active_id, workspace, toast, toast_kind)) { (icon("export")) "Markdown 내보내기" }
                                    button(class="danger", disabled=is_running.get(), on:click=move |_| delete_conversation(active_id, workspace, selected_sources, save_state)) { (icon("trash")) "삭제" }
                                }
                            }
                        }
                    } else { View::default() })
                }
            }

            (if panel.get() != Panel::None {
                view! { button(class="scrim", aria-label="패널 닫기", on:click=move |_| panel.set(Panel::None)) {} }
            } else { View::default() })

            (if !toast.get_clone().is_empty() {
                view! {
                    div(class=format!("toast {}", toast_kind.get_clone()), role="status", aria-live="polite") {
                        span { (toast.get_clone()) }
                        button(aria-label="알림 닫기", on:click=move |_| toast.set(String::new())) { (icon("close")) }
                    }
                }
            } else { View::default() })
        }
    }
}

fn mode_button(
    value: &'static str,
    label: &'static str,
    icon_name: &'static str,
    workspace: Signal<Workspace>,
    save_state: Signal<String>,
) -> View {
    view! {
        button(
            r#type="button",
            role="radio",
            aria-checked=move || (workspace.get_clone().settings.last_mode == value).to_string(),
            class=move || if workspace.get_clone().settings.last_mode == value { "active" } else { "" },
            on:click=move |_| {
                workspace.update(|state| state.settings.last_mode = value.into());
                persist_workspace(workspace, save_state);
            }
        ) { (icon(icon_name)) (label) }
    }
}

fn theme_button(
    value: &'static str,
    label: &'static str,
    workspace: Signal<Workspace>,
    save_state: Signal<String>,
) -> View {
    view! {
        button(
            class=move || if workspace.get_clone().settings.theme == value { "active" } else { "" },
            on:click=move |_| {
                update_theme(value);
                workspace.update(|state| state.settings.theme = value.into());
                persist_workspace(workspace, save_state);
            }
        ) { (label) }
    }
}

fn message_view(
    message: Message,
    selected_sources: Signal<Vec<Source>>,
    panel: Signal<Panel>,
    toast: Signal<String>,
    toast_kind: Signal<String>,
) -> View {
    let is_assistant = message.role == "assistant";
    let role_class = message.role.clone();
    let content = message.content.clone();
    let created_at = message.created_at;
    let status_label = match message.status.as_str() {
        "failed" => Some("완료되지 않은 부분 답변"),
        "cancelled" => Some("중단된 부분 답변"),
        _ => None,
    };
    let footer = if is_assistant {
        answer_footer(
            message.content,
            message.sources,
            message.usage.map(|usage| usage.total_tokens),
            selected_sources,
            panel,
            toast,
            toast_kind,
        )
    } else {
        View::default()
    };
    view! {
        article(class=format!("message {role_class}")) {
            div(class="message-meta") {
                span(class="role-mark") { (if is_assistant { "水" } else { "나" }) }
                strong { (if is_assistant { "Suisou" } else { "나의 질문" }) }
                (status_label.map(|label| view! { span(class="partial-label") { (label) } }).unwrap_or_default())
                time { (format_relative_time(created_at)) }
            }
            div(class="message-body") { (content) }
            (footer)
        }
    }
}

fn answer_footer(
    content: String,
    sources: Vec<Source>,
    total_tokens: Option<u64>,
    selected_sources: Signal<Vec<Source>>,
    panel: Signal<Panel>,
    toast: Signal<String>,
    toast_kind: Signal<String>,
) -> View {
    let source_count = sources.len();
    let source_action = if source_count > 0 {
        view! {
            button(class="text-action", on:click=move |_| {
                selected_sources.set(sources.clone());
                panel.set(Panel::Sources);
            }) { (icon("sources")) (format!("출처 {source_count}")) }
        }
    } else {
        View::default()
    };
    let usage_view = total_tokens
        .map(|total| view! { small(class="usage") { (format!("총 {total} tokens")) } })
        .unwrap_or_default();
    view! {
        div(class="answer-footer") {
            div(class="answer-actions") {
                button(class="text-action", on:click=move |_| copy_text(content.clone(), toast, toast_kind)) { (icon("copy")) "복사" }
                (source_action)
            }
            (usage_view)
        }
    }
}

fn source_view(
    source: Source,
    index: usize,
    toast: Signal<String>,
    toast_kind: Signal<String>,
) -> View {
    let url = source.url;
    let snippet_view = if source.snippet.is_empty() {
        View::default()
    } else {
        view! { p { (source.snippet) } }
    };
    view! {
        article(class="source-card") {
            div(class="source-index") { (format!("{index:02}")) }
            div(class="source-content") {
                small { (source.domain) }
                h3 { (source.title) }
                (snippet_view)
                button(on:click=move |_| open_url(url.clone(), toast, toast_kind)) { "원문 열기" (icon("external")) }
            }
        }
    }
}

fn progress_step(label: &'static str, active: bool, current: bool) -> View {
    view! {
        div(class=format!("progress-step {} {}", if active { "active" } else { "" }, if current { "current" } else { "" })) {
            span(class="step-dot") { (if active { icon("check") } else { View::default() }) }
            small { (label) }
        }
    }
}

fn stage_index(stage: &str) -> i32 {
    match stage {
        "connecting" => 0,
        "searching" => 1,
        "reasoning" => 2,
        "writing" | "done" => 3,
        _ => -1,
    }
}

fn stage_label(stage: &str) -> &'static str {
    match stage {
        "connecting" => "Sakana에 연결 중",
        "searching" => "웹에서 근거 수집 중",
        "reasoning" => "출처를 교차 검토 중",
        "writing" => "답변을 정리하는 중",
        "cancelled" => "중단됨",
        _ => "연구 중",
    }
}

fn select_value(event: Event) -> Option<String> {
    event
        .target()?
        .dyn_into::<web_sys::HtmlSelectElement>()
        .ok()
        .map(|element| element.value())
}

fn copy_text(text: String, toast: Signal<String>, toast_kind: Signal<String>) {
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
            show_toast(
                "답변을 클립보드에 복사했습니다.".into(),
                "success",
                toast,
                toast_kind,
            );
        } else {
            show_toast(
                "클립보드에 복사하지 못했습니다.".into(),
                "error",
                toast,
                toast_kind,
            );
        }
    });
}

fn open_url(url: String, toast: Signal<String>, toast_kind: Signal<String>) {
    spawn_local_scoped(async move {
        if let Err(error) = ipc::command_unit("open_external", &UrlArgs { url }).await {
            show_toast(error, "error", toast, toast_kind);
        }
    });
}

fn cancel_request(
    active_request: Signal<String>,
    toast: Signal<String>,
    toast_kind: Signal<String>,
) {
    let request_id = active_request.get_clone();
    spawn_local_scoped(async move {
        match ipc::command::<_, bool>("cancel_research", &RequestIdArgs { request_id }).await {
            Ok(true) => show_toast(
                "답변 생성을 중단했습니다.".into(),
                "info",
                toast,
                toast_kind,
            ),
            Ok(false) => show_toast("이미 완료된 요청입니다.".into(), "info", toast, toast_kind),
            Err(error) => show_toast(error, "error", toast, toast_kind),
        }
    });
}

fn connect_key(
    key_input: Signal<String>,
    key_busy: Signal<bool>,
    key_configured: Signal<bool>,
    connection_message: Signal<String>,
    toast: Signal<String>,
    toast_kind: Signal<String>,
) {
    let api_key = key_input.get_clone();
    key_input.set(String::new());
    key_busy.set(true);
    spawn_local_scoped(async move {
        let result =
            ipc::command::<_, ConnectionInfo>("connect_api_key", &ApiKeyArgs { api_key }).await;
        key_busy.set(false);
        match result {
            Ok(info) => {
                key_configured.set(true);
                let model_note = if info.models.is_empty() {
                    String::new()
                } else {
                    format!(" · {}개 Fugu 모델", info.models.len())
                };
                connection_message.set(format!("{}{model_note}", info.message));
                show_toast(
                    "Sakana API 연결을 확인하고 키를 안전하게 저장했습니다.".into(),
                    "success",
                    toast,
                    toast_kind,
                );
            }
            Err(error) => show_toast(error, "error", toast, toast_kind),
        }
    });
}

fn clear_key(
    key_configured: Signal<bool>,
    connection_message: Signal<String>,
    toast: Signal<String>,
    toast_kind: Signal<String>,
) {
    spawn_local_scoped(async move {
        match ipc::command_unit("clear_api_key", &EmptyArgs {}).await {
            Ok(()) => {
                key_configured.set(false);
                connection_message.set(String::new());
                show_toast(
                    "API 키를 메모리와 운영체제 보안 저장소에서 제거했습니다.".into(),
                    "success",
                    toast,
                    toast_kind,
                );
            }
            Err(error) => {
                key_configured.set(false);
                connection_message.set(String::new());
                show_toast(error, "error", toast, toast_kind);
            }
        }
    });
}

fn export_current(
    active_id: Signal<String>,
    workspace: Signal<Workspace>,
    toast: Signal<String>,
    toast_kind: Signal<String>,
) {
    let Some(conversation) = current_conversation(&workspace.get_clone(), &active_id.get_clone())
    else {
        return;
    };
    spawn_local_scoped(async move {
        match ipc::command::<_, String>("export_conversation", &ExportArgs { conversation }).await {
            Ok(_) => show_toast(
                "Markdown 파일로 내보냈습니다.".into(),
                "success",
                toast,
                toast_kind,
            ),
            Err(error) => show_toast(error, "error", toast, toast_kind),
        }
    });
}
