use super::*;

impl AppState {
    pub(in crate::app) fn select_conversation(self, id: String) {
        if self.persistence_busy.get_untracked()
            || !self.persistence_queue.with_untracked(VecDeque::is_empty)
        {
            return;
        }
        let sources = self
            .workspace
            .with_untracked(|workspace| source_list(workspace, &id));
        batch(move || {
            self.active_id.set(id);
            self.selected_sources.set(sources);
            self.close_panel();
        });
        reset_viewport_scroll();
    }

    pub(in crate::app) fn new_conversation(self) {
        if self.persistence_busy.get_untracked()
            || !self.persistence_queue.with_untracked(VecDeque::is_empty)
        {
            return;
        }
        batch(move || {
            self.active_id.set(String::new());
            self.selected_sources.set(Vec::new());
            self.composer.set(String::new());
            self.close_panel();
        });
        reset_viewport_scroll();
    }

    pub(in crate::app) fn delete_conversation(self, id: String) {
        if id.is_empty() {
            return;
        }
        let exists = self.workspace.with_untracked(|workspace| {
            workspace
                .conversations
                .iter()
                .any(|conversation| conversation.id == id)
        });
        if !exists {
            return;
        }
        let is_active = self.active_id.get_clone_untracked() == id;
        let rollback_id = self.next_rollback_id.get_untracked().saturating_add(1);
        self.next_rollback_id.set(rollback_id);
        let rollback = self.workspace.with_untracked(|workspace| {
            workspace
                .conversations
                .iter()
                .position(|conversation| conversation.id == id)
                .map(|index| DeleteRollback {
                    id: rollback_id,
                    conversation: workspace.conversations[index].clone(),
                    index,
                    was_active: is_active,
                    selected_sources: self.selected_sources.get_clone_untracked(),
                })
        });
        let Some(rollback) = rollback else {
            return;
        };
        self.delete_rollback.set(Some(rollback));
        batch(move || {
            self.workspace.update(|value| {
                remove_conversation(value, &id);
            });
            if is_active {
                self.active_id.set(String::new());
                self.selected_sources.set(Vec::new());
            }
        });
        if is_active {
            reset_viewport_scroll();
        }
        self.persist_workspace_with_message(Some("대화 기록을 삭제했습니다."), Some(rollback_id));
    }

    pub(in crate::app) fn toggle_pin(self) {
        if !self.storage_writable.get_untracked() {
            return;
        }
        let id = self.active_id.get_clone_untracked();
        self.workspace.update(|value| {
            if let Some(conversation) = value
                .conversations
                .iter_mut()
                .find(|conversation| conversation.id == id)
            {
                conversation.pinned = !conversation.pinned;
                conversation.updated_at = now_millis();
            }
        });
        self.persist_workspace();
    }

    pub(in crate::app) fn retry_question(self) {
        let question = self.last_failed_question.get_clone_untracked();
        if question.is_empty() {
            return;
        }
        if !self.storage_writable.get_untracked() {
            self.show_toast(
                "대화 기록 저장이 완료되거나 복구된 뒤 다시 시도해 주세요.",
                "warning",
            );
            return;
        }
        if !self.key_configured.get_untracked() {
            self.panel.set(Panel::Settings);
            self.show_toast("먼저 설정에서 Sakana API 키를 연결해 주세요.", "warning");
            return;
        }
        let failed_id = self.active_id.get_clone_untracked();
        batch(move || {
            self.composer.set(question);
            self.workspace.update(|value| {
                if let Some(conversation) = value
                    .conversations
                    .iter_mut()
                    .find(|conversation| conversation.id == failed_id)
                {
                    if matches!(
                        conversation
                            .messages
                            .last()
                            .map(|message| message.status.as_str()),
                        Some("failed" | "cancelled")
                    ) {
                        conversation.messages.pop();
                    }
                    if conversation
                        .messages
                        .last()
                        .is_some_and(|message| message.role == "user")
                    {
                        conversation.messages.pop();
                    }
                }
            });
        });
        self.send_question();
    }
}
