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
        let assistant_message_id = new_id("message");
        let active_request_id = request_id.clone();
        let active_assistant_id = assistant_message_id.clone();
        batch(move || {
            self.composer.set(String::new());
            self.last_failed_question.set(String::new());
            self.reset_stream();
            self.selected_sources.set(Vec::new());
            self.stage.set("connecting".into());
            self.is_running.set(true);
            self.active_request.set(active_request_id);
            self.active_assistant_message.set(active_assistant_id);
        });

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
        let workspace = self.workspace.get_clone_untracked();

        spawn_local_scoped(async move {
            let result = ipc::command::<_, StartResearchResponse>(
                "start_research",
                &ResearchArgs {
                    conversation_id,
                    assistant_message_id,
                    question: question.clone(),
                    request,
                    workspace,
                },
            )
            .await;
            match result {
                Ok(response) => {
                    self.workspace.update_silent(|workspace| {
                        workspace.revision = response.job.workspace_revision;
                    });
                    self.apply_research_job(response.job);
                }
                Err(error) => {
                    if let Ok(response) =
                        ipc::command::<_, BootstrapResponse>("bootstrap", &EmptyArgs {}).await
                    {
                        let active_id = self.active_id.get_clone_untracked();
                        let active_exists = response
                            .workspace
                            .conversations
                            .iter()
                            .any(|conversation| conversation.id == active_id);
                        batch(move || {
                            self.workspace.set(response.workspace);
                            self.storage_writable.set(response.storage_writable);
                            self.storage_label.set(response.storage_label);
                            if !active_exists {
                                self.active_id.set(String::new());
                                self.selected_sources.set(Vec::new());
                            }
                        });
                    }
                    batch(move || {
                        self.is_running.set(false);
                        self.active_request.set(String::new());
                        self.active_assistant_message.set(String::new());
                        self.stage.set("failed".into());
                        self.last_failed_question.set(question);
                    });
                    self.show_toast(error, "error");
                }
            }
        });
    }

    pub(in crate::app) fn apply_research_job(self, job: ResearchJob) {
        if job.status == "running" {
            if self.active_request.get_clone_untracked() != job.request_id {
                batch(move || {
                    self.active_request.set(job.request_id.clone());
                    self.active_assistant_message
                        .set(job.assistant_message_id.clone());
                    self.is_running.set(true);
                    self.last_failed_question.set(String::new());
                    self.reset_stream();
                });
            }
            batch(move || {
                self.stage.set(job.stage);
                self.streamed_text.set(job.partial_answer);
            });
            return;
        }

        let is_current = self.active_request.get_clone_untracked() == job.request_id;
        let merged = self.merge_terminal_research_job(&job);
        if is_current {
            batch(move || {
                self.is_running.set(false);
                self.active_request.set(String::new());
                self.active_assistant_message.set(String::new());
                self.stage.set(job.stage.clone());
                self.reset_stream();
            });
        }

        if merged {
            let request_id = job.request_id.clone();
            spawn_local(async move {
                let _ =
                    ipc::command::<_, bool>("discard_research_job", &RequestIdArgs { request_id })
                        .await;
            });
        }
    }

    fn merge_terminal_research_job(self, job: &ResearchJob) -> bool {
        let existing = self.workspace.with_untracked(|workspace| {
            workspace.conversations.iter().any(|conversation| {
                conversation
                    .messages
                    .iter()
                    .any(|message| message.id == job.assistant_message_id)
            })
        });
        if existing {
            if job.workspace_persisted {
                self.workspace.update_silent(|workspace| {
                    workspace.revision = workspace.revision.max(job.workspace_revision);
                });
            }
            return true;
        }

        let (content, sources, usage, status) = if let Some(response) = &job.result {
            (
                response.answer.clone(),
                response.sources.clone(),
                response.usage.clone(),
                "complete".to_string(),
            )
        } else if !job.partial_answer.trim().is_empty() {
            (
                job.partial_answer.clone(),
                Vec::new(),
                None,
                if job.status == "cancelled" {
                    "cancelled".to_string()
                } else {
                    "failed".to_string()
                },
            )
        } else {
            (String::new(), Vec::new(), None, job.status.clone())
        };

        let conversation_exists = self.workspace.with_untracked(|workspace| {
            workspace
                .conversations
                .iter()
                .any(|conversation| conversation.id == job.conversation_id)
        });
        if !conversation_exists {
            self.show_toast(
                "완료된 연구가 있지만 대상 대화가 삭제되어 결과를 적용하지 않았습니다.",
                "warning",
            );
            return true;
        }

        if job.workspace_persisted {
            self.reload_workspace_after_research(job.workspace_revision);
        } else if !content.is_empty() {
            let conversation_id = job.conversation_id.clone();
            let assistant_message_id = job.assistant_message_id.clone();
            self.workspace.update(|value| {
                if let Some(conversation) = value
                    .conversations
                    .iter_mut()
                    .find(|conversation| conversation.id == conversation_id)
                {
                    conversation.updated_at = now_millis();
                    conversation.messages.push(Message {
                        id: assistant_message_id,
                        role: "assistant".into(),
                        content,
                        created_at: now_millis(),
                        status,
                        sources: sources.clone(),
                        usage,
                    });
                }
            });
            self.persist_workspace();
        }

        if job.status == "complete" && job.workspace_persisted {
            batch(move || {
                self.selected_sources.set(sources);
                self.last_failed_question.set(String::new());
            });
        } else {
            self.last_failed_question.set(job.question.clone());
            if let Some(error) = &job.error {
                self.show_toast(error.clone(), "error");
            }
        }
        true
    }

    fn reload_workspace_after_research(self, expected_revision: u64) {
        spawn_local(async move {
            match ipc::command::<_, BootstrapResponse>("bootstrap", &EmptyArgs {}).await {
                Ok(response) if response.workspace_revision >= expected_revision => {
                    let active_id = self.active_id.get_clone_untracked();
                    let sources = source_list(&response.workspace, &active_id);
                    self.workspace.set(response.workspace);
                    self.storage_writable.set(response.storage_writable);
                    self.storage_label.set(response.storage_label);
                    self.selected_sources.set(sources);
                }
                Ok(_) => self.show_toast(
                    "완료된 답변의 최신 대화 기록을 불러오지 못했습니다. 앱을 다시 열어 주세요.",
                    "warning",
                ),
                Err(error) => self.show_toast(error, "error"),
            }
        });
    }

    pub(in crate::app) fn restore_research_jobs(self, jobs: Vec<ResearchJob>) {
        let mut running = None;
        for job in jobs {
            if job.status == "running" {
                if running
                    .as_ref()
                    .is_none_or(|current: &ResearchJob| job.updated_at > current.updated_at)
                {
                    running = Some(job);
                }
            } else {
                self.apply_research_job(job);
            }
        }
        if let Some(job) = running {
            self.apply_research_job(job);
        }
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
