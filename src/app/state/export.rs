use super::*;

impl AppState {
    pub(in crate::app) fn export_current(self) {
        let active_id = self.active_id.get_clone_untracked();
        let conversation = self
            .workspace
            .with_untracked(|workspace| current_conversation_ref(workspace, &active_id).cloned());
        let Some(conversation) = conversation else {
            return;
        };
        spawn_local_scoped(async move {
            match ipc::command::<_, String>("export_conversation", &ExportArgs { conversation })
                .await
            {
                Ok(_) => self.show_toast("Markdown 파일로 내보냈습니다.", "success"),
                Err(error) => self.show_toast(error, "error"),
            }
        });
    }
}
