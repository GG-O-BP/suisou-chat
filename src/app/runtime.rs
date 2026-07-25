use super::*;
use crate::app::browser::update_theme;
use crate::app::state::EmptyArgs;

#[component]
pub(super) fn AppRuntime() -> View {
    let state = use_context::<AppState>();

    spawn_local_scoped(async move {
        match ipc::listen::<ResearchEvent, _>("research-event", move |event| {
            if event.request_id != state.active_request.get_clone_untracked() {
                return;
            }
            match event.kind.as_str() {
                "delta" => state.queue_stream_delta(event.request_id, event.value),
                "stage" => state.stage.set(event.value),
                _ => {}
            }
        })
        .await
        {
            Ok(listener) => on_cleanup(move || listener.unlisten()),
            Err(error) => state.show_toast(error, "error"),
        }
    });

    view! {
        Suspense(fallback=View::default) {
            BootstrapWorkspace {}
        }
    }
}

#[component]
pub(super) async fn BootstrapWorkspace() -> View {
    let state = use_context::<AppState>();
    match ipc::command::<_, BootstrapResponse>("bootstrap", &EmptyArgs {}).await {
        Ok(response) => {
            update_theme(&response.workspace.settings.theme);
            let notices = [response.recovery_notice, response.credential_notice]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            batch(move || {
                state.selected_sources.set(Vec::new());
                state.active_id.set(String::new());
                state.key_configured.set(response.key_configured);
                state.connection_message.set(if response.key_configured {
                    "보안 저장소에서 자동 복원됨".into()
                } else {
                    String::new()
                });
                state.storage_label.set(response.storage_label);
                state.storage_writable.set(response.storage_writable);
                state.workspace.set(response.workspace);
                state.is_loading.set(false);
            });
            if !notices.is_empty() {
                state.show_toast(notices.join(" "), "warning");
            }
        }
        Err(error) => {
            state.is_loading.set(false);
            state.show_toast(error, "error");
        }
    }
    View::default()
}
