use crate::app_state::AppState;
use crate::models::Conversation;
use crate::storage;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use url::Url;

#[tauri::command]
pub(crate) fn open_external(app: AppHandle, url: String) -> Result<(), String> {
    let parsed = Url::parse(&url).map_err(|_| "링크 주소가 올바르지 않습니다.".to_string())?;
    if parsed.scheme() != "https" || parsed.host_str().is_none() || !parsed.username().is_empty() {
        return Err("안전한 HTTPS 링크만 열 수 있습니다.".into());
    }
    app.opener()
        .open_url(parsed.as_str(), None::<&str>)
        .map_err(|_| "기본 브라우저에서 링크를 열지 못했습니다.".to_string())
}

#[tauri::command]
pub(crate) fn export_conversation(
    app: AppHandle,
    conversation: Conversation,
    state: State<'_, AppState>,
) -> Result<String, String> {
    if conversation.id.is_empty() || conversation.id.len() > 128 {
        return Err("내보낼 대화 ID가 올바르지 않습니다.".into());
    }
    std::fs::create_dir_all(&state.export_dir)
        .map_err(|error| format!("내보내기 폴더 생성 실패: {error}"))?;
    let safe_id = conversation
        .id
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
        .take(96)
        .collect::<String>();
    if safe_id.is_empty() {
        return Err("내보낼 대화 ID가 올바르지 않습니다.".into());
    }
    let path = state.export_dir.join(format!("suisou-{safe_id}.md"));
    std::fs::write(&path, storage::conversation_markdown(&conversation))
        .map_err(|error| format!("대화 내보내기 실패: {error}"))?;
    #[cfg(desktop)]
    let _ = app.opener().reveal_item_in_dir(&path);
    #[cfg(mobile)]
    let _ = app;
    Ok(path.to_string_lossy().into_owned())
}
