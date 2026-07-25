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
    state.fugu.clear_key().await
}

#[tauri::command]
pub(crate) fn forget_api_key(state: State<'_, AppState>) {
    state.fugu.forget_key();
}
