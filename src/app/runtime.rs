use super::*;
use crate::app::browser::update_theme;
use crate::app::state::EmptyArgs;

#[component]
pub(super) fn AppRuntime() -> View {
    let state = use_context::<AppState>();
    // Tauri delivers native events (`research-job-event`, `tauri://resumed`) and
    // `setInterval` fires on bare JS ticks where Sycamore's thread-local reactive
    // root is not set. Writing to any signal from those callbacks would call
    // `Root::global().expect(...)` with no root installed and trap the WASM module
    // (`RuntimeError: Unreachable code` / `Out of bounds memory access`), freezing
    // the streamed answer mid-way with the composer still locked. Capture the
    // global reactive scope once and run every native-event body inside it so the
    // root is always present when signals are mutated.
    let scope = use_global_scope();

    on_mount(move || {
        let Some(window) = web_sys::window() else {
            return;
        };
        let callback = Closure::<dyn FnMut()>::new(move || {
            scope.run_in(|| {
                if state.is_running.get_untracked() {
                    state.research_clock.set(now_millis());
                    state.refresh_active_research_job();
                }
            });
        });
        if let Ok(interval) = window.set_interval_with_callback_and_timeout_and_arguments_0(
            callback.as_ref().unchecked_ref(),
            1_000,
        ) {
            on_cleanup(move || {
                window.clear_interval_with_handle(interval);
                drop(callback);
            });
        }
    });

    spawn_local_scoped(async move {
        match ipc::listen::<ResearchJobUpdate, _>("research-job-event", move |event| {
            scope.run_in(|| {
                if state.is_loading.get_untracked() {
                    return;
                }
                match event.kind.as_str() {
                    "snapshot" => {
                        if let Some(job) = event.job {
                            state.apply_research_job(job);
                        }
                    }
                    "stage" if event.request_id == state.active_request.get_clone_untracked() => {
                        if stage_requires_terminal_reconciliation(&event.value) {
                            // `done` is a terminal signal, not an ordinary progress
                            // stage. Reconcile it immediately so the mobile UI can
                            // never show "처리 완료" while remaining locked.
                            state.reconcile_completed_research_job(event.request_id);
                        } else {
                            state.observe_stage(event.value, now_millis());
                        }
                    }
                    "delta" if event.request_id == state.active_request.get_clone_untracked() => {
                        state.queue_stream_delta(event.request_id, event.sequence, event.value);
                    }
                    _ => {}
                }
            });
        })
        .await
        {
            Ok(listener) => on_cleanup(move || listener.unlisten()),
            Err(error) => state.show_toast(error, "error"),
        }
    });

    spawn_local_scoped(async move {
        match ipc::listen::<(), _>("tauri://resumed", move |_| {
            let state = state;
            let scope = scope;
            spawn_local(async move {
                match ipc::command::<_, Vec<ResearchJob>>("list_research_jobs", &EmptyArgs {}).await
                {
                    Ok(jobs) => scope.run_in(|| state.reconcile_research_jobs(jobs)),
                    Err(error) => scope.run_in(|| state.show_toast(error, "error")),
                }
            });
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
                let mut workspace = response.workspace;
                workspace.revision = response.workspace_revision;
                state.workspace.set(workspace);
                state.is_loading.set(false);
            });
            if !notices.is_empty() {
                state.show_toast(notices.join(" "), "warning");
            }
            match ipc::command::<_, Vec<ResearchJob>>("list_research_jobs", &EmptyArgs {}).await {
                Ok(jobs) => state.reconcile_research_jobs(jobs),
                Err(error) => state.show_toast(error, "error"),
            }
        }
        Err(error) => {
            state.is_loading.set(false);
            state.show_toast(error, "error");
        }
    }
    View::default()
}
