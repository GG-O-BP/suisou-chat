use crate::app_state::AppState;
use crate::models::{BootstrapResponse, Workspace};
use crate::storage;
use tauri::State;

#[tauri::command]
pub(crate) fn bootstrap(state: State<'_, AppState>) -> BootstrapResponse {
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
pub(crate) fn save_workspace(
    mut workspace: Workspace,
    state: State<'_, AppState>,
) -> Result<u64, String> {
    let _guard = state
        .save_lock
        .lock()
        .map_err(|_| "저장 작업을 잠글 수 없습니다.".to_string())?;
    let loaded = storage::load_workspace(&state.workspace_path);
    if loaded.warning.is_some() && !loaded.recovered_from_backup {
        return Err("기존 대화 기록을 복구하기 전에는 덮어쓸 수 없습니다.".into());
    }
    workspace.revision = next_workspace_revision(workspace.revision, loaded.workspace.revision)?;
    storage::save_workspace(&state.workspace_path, &workspace)?;
    Ok(workspace.revision)
}

fn next_workspace_revision(client_revision: u64, stored_revision: u64) -> Result<u64, String> {
    if client_revision != stored_revision {
        return Err(
            "다른 저장 작업이 먼저 완료되었습니다. 최신 대화 기록을 다시 불러온 뒤 시도해 주세요."
                .into(),
        );
    }
    client_revision
        .checked_add(1)
        .ok_or_else(|| "대화 기록의 저장 버전 한도에 도달했습니다.".to_string())
}

#[cfg(test)]
mod tests {
    use super::next_workspace_revision;

    #[test]
    fn workspace_revision_advances_only_from_the_latest_snapshot() {
        assert_eq!(next_workspace_revision(7, 7).unwrap(), 8);
        assert!(next_workspace_revision(6, 7).is_err());
        assert!(next_workspace_revision(8, 7).is_err());
        assert!(next_workspace_revision(u64::MAX, u64::MAX).is_err());
    }
}
