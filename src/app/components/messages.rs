use super::super::*;
use crate::app::browser::*;
use crate::app::state::*;

#[derive(Props)]
pub(crate) struct MessageViewProps {
    message: Message,
}

#[component]
pub(crate) fn MessageView(props: MessageViewProps) -> View {
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
pub(crate) struct AnswerFooterProps {
    content: String,
    sources: Vec<Source>,
    children: Children,
}

#[component]
pub(crate) fn AnswerFooter(props: AnswerFooterProps) -> View {
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
pub(crate) fn StreamingMessage() -> View {
    let state = use_context::<AppState>();
    let selected_mode = create_memo(move || {
        state
            .workspace
            .with(|workspace| workspace.settings.last_mode.clone())
    });
    let is_creative = create_selector(move || selected_mode.get_clone() == "create");
    // Gate the progress-vs-answer branch on a *memoized* boolean. Reading
    // `streamed_text` directly inside the `(if …)` below would re-subscribe the
    // conditional to the full answer signal, so every streamed token would tear
    // down and rebuild the entire chosen branch (including the large answer text
    // node). Captured frames showed the transcript flipping between full content
    // and blank many times per second — the "위아래로 마구 흔들리는" flicker.
    // A selector only notifies when the emptiness actually flips, so the subtree
    // is built once and only the inner text node updates per token.
    let has_stream = create_selector(move || !state.streamed_text.with(String::is_empty));
    view! {
        (if state.is_running.get() {
            view! {
                article(class="message assistant streaming", aria-busy="true") {
                    div(class="message-meta") {
                        span(class="role-mark sonar") { span {} }
                        strong { "Sakana Fugu" }
                        span(class="research-stage") { (move || stage_label(&state.stage.get_clone(), &selected_mode.get_clone())) }
                    }
                    (if !has_stream.get() {
                        view! {
                            div(
                                class=move || format!("research-progress stage-{}", state.stage.get_clone()),
                                role="status",
                                aria-label="실시간 처리 상태"
                            ) {
                                div(class="observation-header") {
                                    div(class="observation-title") {
                                        small { "PROCESS STATUS" }
                                        strong {
                                            (move || stage_label(
                                                &state.stage.get_clone(),
                                                &selected_mode.get_clone()
                                            ))
                                        }
                                    }
                                    div(class="observation-time") {
                                        small { "ELAPSED" }
                                        strong {
                                            (move || format_elapsed(
                                                state.research_clock.get().saturating_sub(
                                                    state.research_started_at.get()
                                                )
                                            ))
                                        }
                                    }
                                }
                                div(class="observation-status") {
                                    span(class="observation-beacon", aria-hidden="true") {}
                                    span { "REQUEST ACTIVE" }
                                    span(class="observation-separator", aria-hidden="true") {}
                                    span {
                                        (move || format!(
                                            "현재 상태 {}",
                                            format_elapsed(
                                                state.research_clock.get().saturating_sub(
                                                    state.stage_started_at.get()
                                                )
                                            )
                                        ))
                                    }
                                }
                                div(class="observation-timeline", aria-hidden="true") {
                                    div(class="timeline-ruler") {
                                        span { "START" }
                                        span { "NOW" }
                                    }
                                    div(class="timeline-track") {
                                        span(class="timeline-past") {}
                                        (move || {
                                            state.research_events
                                                .get_clone()
                                                .into_iter()
                                                .enumerate()
                                                .map(|(index, event)| {
                                                    let position = event_position(
                                                        event.occurred_at,
                                                        state.research_started_at.get(),
                                                        state.research_clock.get(),
                                                    );
                                                    view! {
                                                        span(
                                                            class="timeline-event",
                                                            style=format!("left: {position:.2}%"),
                                                            data-index=(index + 1).to_string()
                                                        ) {}
                                                    }
                                                })
                                                .collect::<Vec<_>>()
                                        })
                                        span(class="timeline-now") {}
                                    }
                                }
                                p(class="observation-description") {
                                    (move || stage_description(
                                        &state.stage.get_clone(),
                                        &selected_mode.get_clone()
                                    ))
                                }
                                div(class="event-register") {
                                    div(class="register-heading") {
                                        span { "EVENT REGISTER" }
                                        span { "실제 수신 이벤트만 기록" }
                                    }
                                    ol {
                                        (move || {
                                            let started_at = state.research_started_at.get();
                                            state.research_events
                                                .get_clone()
                                                .into_iter()
                                                .map(|event| {
                                                    let stage = event.value;
                                                    let is_current =
                                                        state.stage.get_clone() == stage;
                                                    let event_class = if is_current {
                                                        "register-event current"
                                                    } else {
                                                        "register-event complete"
                                                    };
                                                    let code = event_code(&stage);
                                                    let label = stage_label(
                                                        &stage,
                                                        &selected_mode.get_clone(),
                                                    );
                                                    view! {
                                                        li(class=event_class) {
                                                            time {
                                                                (format_elapsed(
                                                                    event.occurred_at.saturating_sub(
                                                                        started_at
                                                                    )
                                                                ))
                                                            }
                                                            span(class="register-code") {
                                                                (code)
                                                            }
                                                            strong {
                                                                (label)
                                                            }
                                                        }
                                                    }
                                                })
                                                .collect::<Vec<_>>()
                                        })
                                    }
                                }
                            }
                        }
                    } else {
                        view! {
                            div(class="stream-reading-label") {
                                span { (if is_creative.get() { "OUTPUT ACTIVE" } else { "ANSWER STREAM ACTIVE" }) }
                                span {
                                    (move || format!(
                                        "{} · {}자",
                                        format_elapsed(
                                            state.research_clock.get().saturating_sub(
                                                state.research_started_at.get()
                                            )
                                        ),
                                        state.streamed_text.with(|text| text.chars().count())
                                    ))
                                }
                            }
                            // Render the in-flight answer as a normal text node. Replacing
                            // parsed HTML on every token can invalidate WebKit/Sycamore DOM
                            // wrappers while another delta is arriving, which manifested as
                            // `RuntimeError: Out of bounds memory access`. The terminal
                            // MessageView still renders the completed answer as Markdown.
                            div(
                                class="message-body markdown-body illuminated streaming-plain-text",
                                on:click=move |event| open_markdown_link(state, event)
                            ) {
                                (move || state.streamed_text.get_clone())
                            }
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

#[component]
pub(crate) fn RetryBanner() -> View {
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
