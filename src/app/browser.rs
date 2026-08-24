use super::*;
use crate::app::state::UrlArgs;

pub(super) fn update_theme(theme: &str) {
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

pub(super) fn is_mobile_viewport() -> bool {
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
pub(super) fn install_global_shortcuts(state: AppState) {
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

pub(super) fn reset_viewport_scroll() {
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

pub(super) fn transcript_is_near_bottom(
    scroll_top: i32,
    client_height: i32,
    scroll_height: i32,
) -> bool {
    const FOLLOW_THRESHOLD_PX: i64 = 96;
    let distance = i64::from(scroll_height)
        .saturating_sub(i64::from(client_height))
        .saturating_sub(i64::from(scroll_top.max(0)))
        .max(0);
    distance <= FOLLOW_THRESHOLD_PX
}

pub(super) fn open_url(state: AppState, url: String) {
    spawn_local_scoped(async move {
        if let Err(error) = ipc::command_unit("open_external", &UrlArgs { url }).await {
            state.show_toast(error, "error");
        }
    });
}

pub(super) fn open_markdown_link(state: AppState, event: web_sys::MouseEvent) {
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

pub(super) fn copy_text(state: AppState, text: String) {
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

pub(super) fn select_value(event: Event) -> Option<String> {
    event
        .target()?
        .dyn_into::<web_sys::HtmlSelectElement>()
        .ok()
        .map(|element| element.value())
}

pub(super) fn stage_label(stage: &str, mode: &str) -> &'static str {
    match (stage, mode) {
        ("creating", _) => "창작 처리 활성",
        ("reasoning", "create") => "구성 처리 신호 감지",
        ("writing", "create") => "창작물 출력 수신 중",
        ("done", "create") => "창작 완료",
        ("connecting", _) => "요청 연결 중",
        ("searching", _) => "웹 탐색 활성",
        ("reasoning", _) => "응답 처리 활성",
        ("writing", _) => "답변 출력 수신 중",
        ("done", _) => "처리 완료",
        ("failed" | "interrupted", _) => "처리 중단",
        ("cancelled", _) => "사용자가 중단함",
        _ => "응답 처리 중",
    }
}

pub(super) fn stage_description(stage: &str, mode: &str) -> &'static str {
    match (stage, mode) {
        ("connecting", _) => "요청을 전달하고 응답 연결을 기다리고 있습니다.",
        ("searching", _) => "웹 검색 도구의 활동이 관측되었습니다.",
        ("creating", _) => "창작 요청이 처리 중이며 아직 출력은 시작되지 않았습니다.",
        ("reasoning", "create") => "API 스트림에서 구성 처리 신호가 관측되었습니다.",
        ("reasoning", _) => "응답 생성 처리가 활성 상태이며 아직 출력은 시작되지 않았습니다.",
        ("writing", "create") => "창작물 텍스트가 실제로 수신되고 있습니다.",
        ("writing", _) => "답변 텍스트가 실제로 수신되고 있습니다.",
        _ => "관측 가능한 다음 이벤트를 기다리고 있습니다.",
    }
}

pub(super) fn event_code(stage: &str) -> &'static str {
    match stage {
        "connecting" => "REQUEST",
        "searching" => "WEB TOOL",
        "creating" | "reasoning" => "PROCESS",
        "writing" => "OUTPUT",
        "done" => "COMPLETE",
        "cancelled" => "STOPPED",
        "failed" | "interrupted" => "INTERRUPTED",
        _ => "STATUS",
    }
}

pub(super) fn format_elapsed(duration_millis: u64) -> String {
    let total_seconds = duration_millis / 1_000;
    let hours = total_seconds / 3_600;
    let minutes = (total_seconds % 3_600) / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

pub(super) fn event_position(occurred_at: u64, started_at: u64, now: u64) -> f64 {
    let span = now.saturating_sub(started_at).max(1_000);
    let elapsed = occurred_at.saturating_sub(started_at).min(span);
    4.0 + (elapsed as f64 / span as f64) * 88.0
}

pub(super) fn mode_depth(mode: &str) -> &'static str {
    match mode {
        "quick" => "SURFACE · 40 M",
        "deep" => "ABYSS · 1,880 M",
        "create" => "ATELIER · 720 M",
        _ => "REEF · 480 M",
    }
}

#[cfg(test)]
mod tests {
    use super::transcript_is_near_bottom;

    #[test]
    fn transcript_follow_threshold_handles_short_near_and_scrolled_content() {
        assert!(transcript_is_near_bottom(0, 700, 500));
        assert!(transcript_is_near_bottom(300, 700, 1_000));
        assert!(transcript_is_near_bottom(210, 700, 1_000));
        assert!(!transcript_is_near_bottom(200, 700, 1_000));
        assert!(!transcript_is_near_bottom(-20, 700, 1_000));
    }
}
