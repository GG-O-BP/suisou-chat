use super::*;

impl AppState {
    pub(in crate::app) fn send_question(self) {
        let question = self.composer.get_clone_untracked().trim().to_owned();
        if question.is_empty() || self.is_running.get_untracked() {
            return;
        }
        if !self.storage_writable.get_untracked() {
            self.show_toast(
                "대화 기록 저장이 완료되거나 복구된 뒤 다시 시도해 주세요.",
                "warning",
            );
            return;
        }
        if question.chars().count() > 20_000 {
            self.show_toast("질문은 20,000자 이하로 입력해 주세요.", "error");
            return;
        }
        if !self.key_configured.get_untracked() {
            self.panel.set(Panel::Settings);
            self.show_toast("먼저 설정에서 Sakana API 키를 연결해 주세요.", "warning");
            return;
        }

        let active_id = self.active_id.get_clone_untracked();
        let (prior_messages, prior_chars) = self.workspace.with_untracked(|workspace| {
            current_conversation_ref(workspace, &active_id).map_or((0, 0), |conversation| {
                (
                    conversation.messages.len(),
                    conversation
                        .messages
                        .iter()
                        .map(|message| message.content.chars().count())
                        .sum(),
                )
            })
        });
        if prior_messages >= 199 || prior_chars.saturating_add(question.chars().count()) > 500_000 {
            self.show_toast(
                "이 대화에서 사용할 수 있는 문맥 한도에 도달했습니다. 새 대화에서 이어가 주세요.",
                "warning",
            );
            return;
        }

        let timestamp = now_millis();
        let conversation_id = if active_id.is_empty() {
            let id = new_id("conversation");
            let id_for_workspace = id.clone();
            self.workspace.update(|value| {
                value.conversations.push(Conversation {
                    id: id_for_workspace,
                    title: title_from_question(&question),
                    pinned: false,
                    created_at: timestamp,
                    updated_at: timestamp,
                    messages: Vec::new(),
                });
            });
            self.active_id.set(id.clone());
            id
        } else {
            active_id
        };

        self.workspace.update(|value| {
            if let Some(conversation) = value
                .conversations
                .iter_mut()
                .find(|conversation| conversation.id == conversation_id)
            {
                conversation.updated_at = timestamp;
                conversation.messages.push(Message {
                    id: new_id("message"),
                    role: "user".into(),
                    content: question.clone(),
                    created_at: timestamp,
                    status: "complete".into(),
                    sources: Vec::new(),
                    usage: None,
                });
            }
        });

        let request_id = new_id("request");
        let active_request_id = request_id.clone();
        batch(move || {
            self.composer.set(String::new());
            self.last_failed_question.set(String::new());
            self.reset_stream();
            self.selected_sources.set(Vec::new());
            self.stage.set("connecting".into());
            self.is_running.set(true);
            self.active_request.set(active_request_id);
        });
        self.persist_workspace();

        let request = self.workspace.with_untracked(|workspace| {
            let conversation = current_conversation_ref(workspace, &conversation_id)
                .cloned()
                .unwrap_or_default();
            ResearchRequest {
                request_id: request_id.clone(),
                model: workspace.settings.model.clone(),
                mode: workspace.settings.last_mode.clone(),
                reasoning: workspace.settings.reasoning.clone(),
                messages: conversation
                    .messages
                    .iter()
                    .map(|message| InputMessage {
                        role: message.role.clone(),
                        content: message.content.clone(),
                    })
                    .collect(),
            }
        });

        spawn_local_scoped(async move {
            let result =
                ipc::command::<_, ResearchResponse>("run_research", &ResearchArgs { request })
                    .await;
            if self.active_request.get_clone_untracked() != request_id {
                return;
            }
            self.flush_stream_delta();
            batch(move || {
                self.is_running.set(false);
                self.active_request.set(String::new());
            });
            match result {
                Ok(response) => {
                    self.workspace.update(|value| {
                        if let Some(conversation) = value
                            .conversations
                            .iter_mut()
                            .find(|conversation| conversation.id == conversation_id)
                        {
                            conversation.updated_at = now_millis();
                            conversation.messages.push(Message {
                                id: new_id("message"),
                                role: "assistant".into(),
                                content: response.answer,
                                created_at: now_millis(),
                                status: "complete".into(),
                                sources: response.sources.clone(),
                                usage: response.usage,
                            });
                        }
                    });
                    batch(move || {
                        self.selected_sources.set(response.sources);
                        self.reset_stream();
                        self.stage.set("done".into());
                    });
                    self.persist_workspace();
                }
                Err(error) => {
                    let partial = self.streamed_text.get_clone_untracked();
                    let status = if error.contains("중단") {
                        "cancelled"
                    } else {
                        "failed"
                    };
                    if !partial.trim().is_empty() {
                        self.workspace.update(|value| {
                            if let Some(conversation) = value
                                .conversations
                                .iter_mut()
                                .find(|conversation| conversation.id == conversation_id)
                            {
                                conversation.updated_at = now_millis();
                                conversation.messages.push(Message {
                                    id: new_id("message"),
                                    role: "assistant".into(),
                                    content: partial,
                                    created_at: now_millis(),
                                    status: status.into(),
                                    sources: Vec::new(),
                                    usage: None,
                                });
                            }
                        });
                        self.persist_workspace();
                    }
                    batch(move || {
                        self.stage.set(status.into());
                        self.reset_stream();
                        self.last_failed_question.set(question);
                    });
                    let mut error = error;
                    if error.contains("인증") || error.contains("API 키") {
                        if let Err(clear_error) =
                            ipc::command_unit("clear_api_key", &EmptyArgs {}).await
                        {
                            error = format!("{error} {clear_error}");
                            let _ = ipc::command_unit("forget_api_key", &EmptyArgs {}).await;
                        }
                        batch(move || {
                            self.key_configured.set(false);
                            self.panel.set(Panel::Settings);
                        });
                    }
                    self.show_toast(error, "error");
                }
            }
        });
    }

    pub(in crate::app) fn cancel_request(self) {
        let request_id = self.active_request.get_clone_untracked();
        spawn_local_scoped(async move {
            match ipc::command::<_, bool>("cancel_research", &RequestIdArgs { request_id }).await {
                Ok(true) => self.show_toast("답변 생성을 중단했습니다.", "info"),
                Ok(false) => self.show_toast("이미 완료된 요청입니다.", "info"),
                Err(error) => self.show_toast(error, "error"),
            }
        });
    }
}
