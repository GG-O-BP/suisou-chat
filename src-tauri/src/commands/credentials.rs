use crate::app_state::AppState;
use crate::models::ConnectionInfo;
use tauri::State;

#[tauri::command]
pub(crate) async fn connect_api_key(
    api_key: String,
    state: State<'_, AppState>,
) -> Result<ConnectionInfo, String> {
    state.fugu.connect(api_key).await
}

#[tauri::command]
pub(crate) async fn clear_api_key(state: State<'_, AppState>) -> Result<(), String> {
    if state.research_jobs.has_running()? {
        return Err("실행 중인 답변을 먼저 중단한 뒤 API 키 연결을 해제해 주세요.".into());
    }
    state.fugu.clear_key().await
}

#[tauri::command]
pub(crate) fn forget_api_key(state: State<'_, AppState>) -> Result<(), String> {
    if state.research_jobs.has_running()? {
        return Err("실행 중인 답변을 먼저 중단한 뒤 API 키 연결을 해제해 주세요.".into());
    }
    state.fugu.forget_key();
    Ok(())
}
