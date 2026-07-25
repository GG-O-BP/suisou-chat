use super::super::*;
use crate::app::state::*;

#[derive(Clone, PartialEq)]
struct HistoryEntry {
    id: String,
    title: String,
    pinned: bool,
    updated_at: u64,
}

#[component]
pub(crate) fn Sidebar() -> View {
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
pub(crate) struct HistoryItemProps {
    conversation: HistoryEntry,
}

#[component]
pub(crate) fn HistoryItem(props: HistoryItemProps) -> View {
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
