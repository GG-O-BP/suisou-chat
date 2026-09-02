use super::super::*;
use crate::app::browser::*;
use crate::app::state::*;

#[component]
pub(crate) fn Composer() -> View {
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
                            small { "PROCESSING" }
                            strong { (move || stage_label(&state.stage.get_clone(), &selected_mode.get_clone())) }
                        }
                        span(class="mobile-dive-depth") {
                            (move || format_elapsed(
                                state.research_clock.get().saturating_sub(
                                    state.research_started_at.get()
                                )
                            ))
                        }
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
            }
            div(class="composer") {
                div(class="capsule-seal", aria-hidden="true") {
                    span {}
                    (move || if selected_mode.get_clone() == "create" { "CREATIVE" } else { "RESEARCH" })
                    span {}
                }
                label(class="sr-only", r#for="question-input") { "질문 입력" }
                textarea(
                    r#ref=input_ref,
                    id="question-input",
                    bind:value=state.composer,
                    on:keydown=keydown,
                placeholder=move || if selected_mode.get_clone() == "create" { "어떤 글을 써 볼까요?" } else { "무엇을 알아볼까요?" },
                    rows="1",
                    maxlength="20000",
                    disabled=move || state.is_running.get() || !state.storage_writable.get()
                ) {}
                div(class="composer-bottom") {
                    div(class="model-controls") {
                        label {
                            span(class="sr-only") { "Fugu 모델" }
                            select(disabled=move || state.is_running.get() || !state.storage_writable.get(), on:change=move |event: Event| {
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
                            select(disabled=move || state.is_running.get() || !state.storage_writable.get(), on:change=move |event: Event| {
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
                        "Enter로 전송 · Shift+Enter로 줄바꿈 · 장르, 분위기, 길이, 독자를 알려주면 더 정교하게 만들 수 있습니다"
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
pub(crate) struct ModeButtonProps {
    value: &'static str,
    index: &'static str,
    label: &'static str,
    detail: &'static str,
    icon_name: &'static str,
}

#[component]
pub(crate) fn ModeButton(props: ModeButtonProps) -> View {
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
            disabled=move || state.is_running.get() || !state.storage_writable.get(),
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
