use super::super::*;
use super::{Composer, MessageView, RetryBanner, StreamingMessage, Welcome};
use crate::app::browser::*;
use crate::app::state::*;

#[component]
pub(crate) fn WorkspaceView() -> View {
    view! {
        main(id="main-content", class="workspace") {
            TopBar {}
            Transcript {}
            Composer {}
        }
    }
}

#[component]
pub(crate) fn TopBar() -> View {
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
pub(crate) fn Transcript() -> View {
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
pub(crate) fn ConversationTranscript() -> View {
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
