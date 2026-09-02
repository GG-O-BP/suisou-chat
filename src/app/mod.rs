use crate::icons::icon;
use crate::ipc;
use crate::markdown::render_markdown;
use crate::models::{
    format_relative_time, new_id, now_millis, provider_for_model, provider_key_label,
    provider_label, remove_conversation, stage_requires_terminal_reconciliation,
    terminal_job_action, title_from_question, BootstrapResponse, ConnectionInfo, Conversation,
    InputMessage, Message, ResearchEvent, ResearchJob, ResearchJobObservation, ResearchJobUpdate,
    ResearchRequest, Source, StartResearchResponse, Workspace,
};
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use sycamore::futures::{spawn_local, spawn_local_scoped};
use sycamore::prelude::*;
use sycamore::web::events::{Event, KeyboardEvent, SubmitEvent};
use sycamore::web::{Suspense, Transition};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

mod browser;
mod components;
mod runtime;
mod state;

use browser::{
    dismiss_boot_screen, install_global_shortcuts, mark_runtime_platform, reset_viewport_scroll,
};
use components::{OverlayLayer, SettingsPanel, Sidebar, SourcesPanel, WorkspaceView};
use runtime::AppRuntime;
use state::{AppState, Panel};

#[component]
pub fn App() -> View {
    mark_runtime_platform();
    let state = AppState::new();
    provide_context(state);
    install_global_shortcuts(state);
    on_mount(reset_viewport_scroll);
    on_mount(dismiss_boot_screen);

    view! {
        div(class=move || format!("app-shell {}", if state.panel.get() != Panel::None { "panel-open" } else { "" })) {
            a(class="skip-link", href="#main-content") { "본문으로 건너뛰기" }
            AppRuntime {}
            Sidebar {}
            WorkspaceView {}
            SourcesPanel {}
            SettingsPanel {}
            OverlayLayer {}
        }
    }
}
