use crate::app_state::AppState;
use crate::commands::workspace::save_workspace_snapshot;
use crate::models::{
    provider_for_model, ResearchJob, ResearchRequest, StartResearchResponse, Workspace,
};
use tauri::State;

// The frontend sends the start payload as flat camelCase fields (see
// `ResearchArgs` in `src/app/state/mod.rs`). Tauri derives one IPC payload key
// per command parameter, so these must stay as individual arguments — wrapping
// them in a single struct would make Tauri look for a nonexistent `args` key
// and fail every call with "missing required key args".
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) fn start_research(
    conversation_id: String,
    assistant_message_id: String,
    question: String,
    request: ResearchRequest,
    workspace: Workspace,
    state: State<'_, AppState>,
) -> Result<StartResearchResponse, String> {
    let provider = provider_for_model(&request.model)?;
    if !state.fugu.has_key(provider) {
        return Err(format!("{} 키를 먼저 연결해 주세요.", provider.key_label()));
    }
    state.research_jobs.ensure_can_start(
        &conversation_id,
        &assistant_message_id,
        &question,
        &request,
    )?;
    let workspace_revision =
        save_workspace_snapshot(workspace, &state.workspace_path, &state.save_lock)?;
    state.research_jobs.start(
        conversation_id,
        workspace_revision,
        assistant_message_id,
        question,
        request,
    )
}

#[tauri::command]
pub(crate) fn cancel_research(
    request_id: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    if request_id.len() > 128 {
        return Err("잘못된 요청 ID입니다.".into());
    }
    state.research_jobs.cancel(&request_id)
}

#[tauri::command]
pub(crate) fn list_research_jobs(state: State<'_, AppState>) -> Result<Vec<ResearchJob>, String> {
    state.research_jobs.list()
}

#[tauri::command]
pub(crate) fn get_research_job(
    request_id: String,
    state: State<'_, AppState>,
) -> Result<Option<ResearchJob>, String> {
    state.research_jobs.get(&request_id)
}

#[tauri::command]
pub(crate) fn discard_research_job(
    request_id: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    state.research_jobs.discard(&request_id)
}
