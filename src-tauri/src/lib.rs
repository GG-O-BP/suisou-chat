mod app_state;
mod commands;
mod credentials;
mod fugu;
mod models;
mod storage;

use app_state::AppState;
use commands::{
    bootstrap, cancel_research, clear_api_key, connect_api_key, export_conversation,
    forget_api_key, open_external, run_research, save_workspace,
};
use fugu::FuguRuntime;
use std::sync::{Arc, Mutex};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

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
                fugu: Arc::new(FuguRuntime::new()?),
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
