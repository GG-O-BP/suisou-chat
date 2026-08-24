mod app_state;
mod background;
mod commands;
mod credentials;
mod fugu;
mod models;
mod research_jobs;
mod storage;

use app_state::AppState;
use commands::{
    bootstrap, cancel_research, clear_api_key, connect_api_key, discard_research_job,
    export_conversation, forget_api_key, get_research_job, list_research_jobs, open_external,
    save_workspace, start_research,
};
use fugu::FuguRuntime;
use research_jobs::ResearchJobManager;
use std::sync::{Arc, Mutex};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

#[cfg(all(feature = "e2e", not(debug_assertions)))]
compile_error!("the e2e WebDriver server must never be enabled in a release build");

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default().plugin(tauri_plugin_opener::init());
    #[cfg(feature = "e2e")]
    let builder = builder.plugin(tauri_plugin_wdio_webdriver::init());

    builder
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
            let fugu = Arc::new(FuguRuntime::new()?);
            let workspace_path = data_dir.join("workspace.json");
            let save_lock = Arc::new(Mutex::new(()));
            let research_jobs = Arc::new(ResearchJobManager::new(
                app.handle().clone(),
                Arc::clone(&fugu),
                data_dir.join("research-jobs.json"),
                workspace_path.clone(),
                Arc::clone(&save_lock),
                background::execution(app.handle()),
            )?);
            ResearchJobManager::register(&research_jobs);
            app.manage(AppState {
                fugu,
                research_jobs,
                workspace_path,
                export_dir: documents_dir.join("Suisou"),
                save_lock,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            save_workspace,
            connect_api_key,
            clear_api_key,
            forget_api_key,
            start_research,
            cancel_research,
            list_research_jobs,
            get_research_job,
            discard_research_job,
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

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_ggobp_suisou_1chat_background_SuisouResearchService_cancelResearch(
    mut env: jni::JNIEnv,
    _service: jni::objects::JObject,
    request_id: jni::objects::JString,
) -> jni::sys::jboolean {
    let Ok(request_id) = env.get_string(&request_id) else {
        return 0;
    };
    if research_jobs::cancel_registered(request_id.to_str().unwrap_or_default()) {
        1
    } else {
        0
    }
}

#[cfg(any(target_os = "android", target_os = "ios"))]
#[unsafe(no_mangle)]
/// Cancels the currently registered research job.
///
/// # Safety
///
/// `request_id` must point to a valid, NUL-terminated UTF-8 string for the
/// duration of this call.
pub unsafe extern "C" fn suisou_cancel_research(request_id: *const std::os::raw::c_char) -> bool {
    if request_id.is_null() {
        return false;
    }
    let request_id = std::ffi::CStr::from_ptr(request_id);
    request_id
        .to_str()
        .ok()
        .is_some_and(research_jobs::cancel_registered)
}
