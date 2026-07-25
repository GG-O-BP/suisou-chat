mod credentials;
mod fugu;
mod models;
mod storage;

use fugu::FuguRuntime;
use models::{
    BootstrapResponse, ConnectionInfo, Conversation, ResearchRequest, ResearchResponse, Workspace,
};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;
use url::Url;

struct AppState {
    fugu: FuguRuntime,
    workspace_path: PathBuf,
    export_dir: PathBuf,
    save_lock: Mutex<()>,
}

#[tauri::command]
fn bootstrap(state: State<'_, AppState>) -> BootstrapResponse {
    let loaded = storage::load_workspace(&state.workspace_path);
    let storage_writable = loaded.warning.is_none() || loaded.recovered_from_backup;
    let recovery_notice = loaded.warning.clone();
    BootstrapResponse {
        workspace: loaded.workspace,
        key_configured: state.fugu.has_key(),
        credential_notice: state.fugu.credential_notice(),
        recovery_notice,
        storage_label: if storage_writable {
            "이 기기에만 저장됨".into()
        } else {
            "복구 필요 · 읽기 전용".into()
        },
        storage_writable,
    }
}

#[tauri::command]
fn save_workspace(mut workspace: Workspace, state: State<'_, AppState>) -> Result<u64, String> {
    let _guard = state
        .save_lock
        .lock()
        .map_err(|_| "저장 작업을 잠글 수 없습니다.".to_string())?;
    let loaded = storage::load_workspace(&state.workspace_path);
    if loaded.warning.is_some() && !loaded.recovered_from_backup {
        return Err("기존 작업 공간을 복구하기 전에는 덮어쓸 수 없습니다.".into());
    }
    workspace.revision = loaded
        .workspace
        .revision
        .max(workspace.revision)
        .saturating_add(1);
    storage::save_workspace(&state.workspace_path, &workspace)?;
    Ok(workspace.revision)
}

#[tauri::command]
async fn connect_api_key(
    api_key: String,
    state: State<'_, AppState>,
) -> Result<ConnectionInfo, String> {
    state.fugu.connect(api_key).await
}

#[tauri::command]
fn clear_api_key(state: State<'_, AppState>) -> Result<(), String> {
    state.fugu.clear_key()
}

#[tauri::command]
fn forget_api_key(state: State<'_, AppState>) {
    state.fugu.forget_key();
}

#[tauri::command]
async fn run_research(
    window: WebviewWindow,
    request: ResearchRequest,
    state: State<'_, AppState>,
) -> Result<ResearchResponse, String> {
    state.fugu.research(window, request).await
}

#[tauri::command]
fn cancel_research(request_id: String, state: State<'_, AppState>) -> Result<bool, String> {
    if request_id.len() > 128 {
        return Err("잘못된 요청 ID입니다.".into());
    }
    state.fugu.cancel(&request_id)
}

#[tauri::command]
fn open_external(app: AppHandle, url: String) -> Result<(), String> {
    let parsed = Url::parse(&url).map_err(|_| "링크 주소가 올바르지 않습니다.".to_string())?;
    if parsed.scheme() != "https" || parsed.host_str().is_none() || !parsed.username().is_empty() {
        return Err("안전한 HTTPS 링크만 열 수 있습니다.".into());
    }
    app.opener()
        .open_url(parsed.as_str(), None::<&str>)
        .map_err(|_| "기본 브라우저에서 링크를 열지 못했습니다.".to_string())
}

#[tauri::command]
fn export_conversation(
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let window_builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title("Suisou — AI Research Companion")
                .inner_size(1180.0, 780.0)
                .min_inner_size(360.0, 560.0);
            #[cfg(desktop)]
            let window_builder = window_builder.center().enable_clipboard_access();
            window_builder
                .build()
                .map_err(|error| format!("앱 창 생성 실패: {error}"))?;
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| format!("앱 데이터 경로 확인 실패: {error}"))?;
            let documents_dir = app
                .path()
                .document_dir()
                .unwrap_or_else(|_| data_dir.clone());
            app.manage(AppState {
                fugu: FuguRuntime::new()?,
                workspace_path: data_dir.join("workspace.json"),
                export_dir: documents_dir.join("Suisou"),
                save_lock: Mutex::new(()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            save_workspace,
            connect_api_key,
            clear_api_key,
            forget_api_key,
            run_research,
            cancel_research,
            open_external,
            export_conversation
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Suisou");
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_ggobp_suisou_1chat_MainActivity_initializeApiKeyStore(
    env: jni::JNIEnv,
    activity: jni::objects::JObject,
    context: jni::objects::JObject,
) {
    android_native_keyring_store::Java_io_crates_keyring_Keyring_00024Companion_initializeNdkContext(
        env, activity, context,
    );
}
