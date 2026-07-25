use super::*;

impl AppState {
    pub(in crate::app) fn persist_workspace(self) {
        let delete_rollback_id = self
            .delete_rollback
            .with_untracked(|rollback| rollback.as_ref().map(|rollback| rollback.id));
        self.persist_workspace_with_message(None, delete_rollback_id);
    }

    pub(in crate::app) fn persist_workspace_with_message(
        self,
        success_message: Option<&'static str>,
        delete_rollback_id: Option<u64>,
    ) {
        self.persistence_queue.update(|queue| {
            queue.push_back(PersistenceRequest {
                workspace: self.workspace.get_clone_untracked(),
                success_message,
                delete_rollback_id,
            });
        });
        self.storage_writable.set(false);
        self.save_state.set("saving".into());
        self.persist_next_workspace();
    }

    pub(in crate::app) fn persist_next_workspace(self) {
        if self.persistence_busy.get_untracked() {
            return;
        }
        let Some(mut request) = self
            .persistence_queue
            .with_untracked(|queue| queue.front().cloned())
        else {
            return;
        };
        self.persistence_queue.update(|queue| {
            queue.pop_front();
        });
        request.workspace.revision = self
            .workspace
            .with_untracked(|workspace| workspace.revision);
        self.persistence_busy.set(true);

        // This queue can start the next save after the originating event handler's
        // reactive scope has been destroyed. Keep the task on the app-owned signals
        // instead of binding it to that short-lived event scope.
        spawn_local(async move {
            let result = ipc::command::<_, u64>(
                "save_workspace",
                &WorkspaceArgs {
                    workspace: request.workspace,
                },
            )
            .await;
            if !self.workspace.is_alive() {
                return;
            }
            let succeeded = match result {
                Ok(revision) => {
                    // Revision is persistence metadata and is not rendered. Updating it
                    // silently avoids invalidating every workspace-derived selector.
                    self.workspace
                        .update_silent(|value| value.revision = revision);
                    if request.delete_rollback_id.is_some_and(|request_id| {
                        self.delete_rollback.with_untracked(|rollback| {
                            rollback
                                .as_ref()
                                .is_some_and(|rollback| rollback.id == request_id)
                        })
                    }) {
                        self.delete_rollback.set(None);
                    }
                    if request.delete_rollback_id.is_none() {
                        if let Some(message) = request.success_message {
                            self.show_toast(message, "success");
                        }
                    }
                    true
                }
                Err(error) => {
                    if self.persistence_queue.with_untracked(VecDeque::is_empty) {
                        if request.delete_rollback_id.is_some_and(|request_id| {
                            self.delete_rollback.with_untracked(|rollback| {
                                rollback
                                    .as_ref()
                                    .is_some_and(|rollback| rollback.id == request_id)
                            })
                        }) {
                            self.restore_deleted_conversation();
                        }
                        self.save_state.set(format!("error:{error}"));
                        self.show_toast(
                            format!("변경 사항을 저장하지 못했습니다: {error}"),
                            "error",
                        );
                    }
                    false
                }
            };
            self.persistence_busy.set(false);
            if self.persistence_queue.with_untracked(VecDeque::is_empty) {
                if succeeded {
                    self.save_state.set("saved".into());
                    self.storage_writable.set(true);
                    if request.delete_rollback_id.is_some() {
                        if let Some(message) = request.success_message {
                            self.show_toast(message, "success");
                        }
                    }
                }
            } else {
                self.save_state.set("saving".into());
                self.persist_next_workspace();
            }
        });
    }

    pub(in crate::app) fn restore_deleted_conversation(self) {
        let Some(rollback) = self.delete_rollback.take() else {
            return;
        };
        let conversation_id = rollback.conversation.id.clone();
        self.workspace.update(|workspace| {
            if workspace
                .conversations
                .iter()
                .any(|conversation| conversation.id == conversation_id)
            {
                return;
            }
            let index = rollback.index.min(workspace.conversations.len());
            workspace.conversations.insert(index, rollback.conversation);
        });
        if rollback.was_active && self.active_id.with_untracked(String::is_empty) {
            batch(move || {
                self.active_id.set(conversation_id);
                self.selected_sources.set(rollback.selected_sources);
            });
            reset_viewport_scroll();
        }
    }
}
