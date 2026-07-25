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
pub(crate) struct ProgressStepProps {
    index: &'static str,
    code: &'static str,
    label: &'static str,
    active: MaybeDyn<bool>,
    current: MaybeDyn<bool>,
}

#[component]
pub(crate) fn ProgressStep(props: ProgressStepProps) -> View {
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
