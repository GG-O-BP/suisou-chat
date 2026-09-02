use super::super::*;
use crate::app::state::*;

#[component]
pub(crate) fn Welcome() -> View {
    let state = use_context::<AppState>();
    view! {
        section(class="welcome") {
            h1(class="sr-only") { "Suisou AI 리서치" }
            div(class="welcome-observatory") {
                div(class="observatory-datum", aria-hidden="true") {
                    span { "REFERENCE DATUM" }
                    i {}
                    span { "STN · 01" }
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
                }
                div(class="welcome-status") {
                    span(class=move || format!("status-beacon {}", if state.key_configured.get() { "ready" } else { "attention" })) {}
                    div {
                        small { "CONNECTION STATUS" }
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
                        value="제품 발표를 여는 짧은 인사말을 써 줘",
                        index="04",
                        title="인사말 작성",
                        description="발표를 여는 짧은 문장 쓰기",
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
pub(crate) struct SuggestionButtonProps {
    value: &'static str,
    index: &'static str,
    title: &'static str,
    description: &'static str,
    icon_name: &'static str,
    tone: &'static str,
    mode: &'static str,
}

#[component]
pub(crate) fn SuggestionButton(props: SuggestionButtonProps) -> View {
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
