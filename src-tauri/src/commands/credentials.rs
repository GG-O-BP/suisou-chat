use crate::app_state::AppState;
use crate::models::{parse_provider, ConnectionInfo, Provider};
use tauri::State;

fn provider(value: Option<String>) -> Result<Provider, String> {
    parse_provider(value.as_deref().unwrap_or("sakana"))
}

#[tauri::command]
pub(crate) async fn connect_api_key(
    api_key: String,
    provider_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<ConnectionInfo, String> {
    state.fugu.connect(provider(provider_name)?, api_key).await
}

#[tauri::command]
pub(crate) async fn clear_api_key(
    provider_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let provider = provider(provider_name)?;
    if state.research_jobs.has_running(provider)? {
        return Err(format!(
            "실행 중인 {} 답변을 먼저 중단한 뒤 API 키 연결을 해제해 주세요.",
            provider.label()
        ));
    }
    state.fugu.clear_key(provider).await
}

#[tauri::command]
pub(crate) fn forget_api_key(
    provider_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let provider = provider(provider_name)?;
    if state.research_jobs.has_running(provider)? {
        return Err(format!(
            "실행 중인 {} 답변을 먼저 중단한 뒤 API 키 연결을 해제해 주세요.",
            provider.label()
        ));
    }
    state.fugu.forget_key(provider);
    Ok(())
}
