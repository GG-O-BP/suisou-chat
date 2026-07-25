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

pub(super) fn stage_index(stage: &str) -> i32 {
    match stage {
        "connecting" => 0,
        "searching" | "creating" => 1,
        "reasoning" => 2,
        "writing" | "done" => 3,
        _ => -1,
    }
}

pub(super) fn stage_label(stage: &str, mode: &str) -> &'static str {
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

pub(super) fn stage_depth(stage: &str, mode: &str) -> i32 {
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

pub(super) fn mode_depth(mode: &str) -> &'static str {
    match mode {
        "quick" => "SURFACE · 40 M",
        "deep" => "ABYSS · 1,880 M",
        "create" => "ATELIER · 720 M",
        _ => "REEF · 480 M",
    }
}
