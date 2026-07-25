use crate::app_state::AppState;
use crate::models::{ResearchRequest, ResearchResponse};
use tauri::{State, WebviewWindow};

#[tauri::command]
pub(crate) async fn run_research(
    window: WebviewWindow,
    request: ResearchRequest,
    state: State<'_, AppState>,
) -> Result<ResearchResponse, String> {
    state.fugu.research(window, request).await
}

#[tauri::command]
pub(crate) fn cancel_research(
    request_id: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    if request_id.len() > 128 {
        return Err("잘못된 요청 ID입니다.".into());
    }
    state.fugu.cancel(&request_id)
}
