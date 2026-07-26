use super::super::*;
use crate::app::browser::*;
use crate::app::state::*;

#[component]
pub(crate) fn SourcesPanel() -> View {
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
pub(crate) struct SourceViewProps {
    source: Source,
}

#[component]
pub(crate) fn SourceView(props: SourceViewProps) -> View {
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
pub(crate) fn SettingsPanel() -> View {
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
                                button(
                                    disabled=move || state.is_running.get(),
                                    on:click=move |_| state.clear_key()
                                ) { "연결 해제" }
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
                                button(disabled=move || state.is_running.get() || !state.storage_writable.get(), on:click=move |_| state.toggle_pin()) { (icon("pin")) "고정 전환" }
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
pub(crate) struct ThemeButtonProps {
    value: &'static str,
    label: &'static str,
}

#[component]
pub(crate) fn ThemeButton(props: ThemeButtonProps) -> View {
    let state = use_context::<AppState>();
    let selected = create_selector(move || {
        state
            .workspace
            .with(|workspace| workspace.settings.theme == props.value)
    });
    view! {
        button(
            class=move || if selected.get() { "active" } else { "" },
            disabled=move || state.is_running.get() || !state.storage_writable.get(),
            on:click=move |_| {
                update_theme(props.value);
                state.workspace.update(|workspace| workspace.settings.theme = props.value.into());
                state.persist_workspace();
            }
        ) { (props.label) }
    }
}

#[component]
pub(crate) fn OverlayLayer() -> View {
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
