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
        let observed_request_id = request_id.clone();
        let active_assistant_id = assistant_message_id.clone();
        let started_at = now_millis();
        batch(move || {
            self.composer.set(String::new());
            self.last_failed_question.set(String::new());
            self.reset_stream();
            self.selected_sources.set(Vec::new());
            self.stage.set("connecting".into());
            self.research_started_at.set(started_at);
            self.stage_started_at.set(started_at);
            self.research_clock.set(started_at);
            self.research_events.set(vec![ResearchEvent {
                kind: "stage".into(),
                value: "connecting".into(),
                occurred_at: started_at,
            }]);
            self.is_running.set(true);
            self.active_request.set(active_request_id);
            self.active_assistant_message.set(active_assistant_id);
            self.research_start_pending.set(true);
            self.research_job_observations.update(|observations| {
                observations.remove(&observed_request_id);
            });
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
            self.research_start_pending.set(false);
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
                        self.observe_stage("failed".into(), now_millis());
                        self.last_failed_question.set(question);
                    });
                    self.show_toast(error, "error");
                }
            }
        });
    }

    pub(in crate::app) fn apply_research_job(self, job: ResearchJob) {
        if job.status == "running" {
            let is_current = self.active_request.get_clone_untracked() == job.request_id;
            if !is_current && self.is_running.get_untracked() {
                return;
            }
        }
        if job.status != "running" {
            // A terminal delivery. Decide what to do without letting the
            // de-duplication gate swallow the UI unlock: duplicate or
            // out-of-order terminal deliveries (the same completion arriving via
            // both the snapshot event and the 1s poll, or after its observation
            // was already recorded) must still release the composer, stop
            // button, and conversation navigation. See `terminal_job_action`.
            let is_active = self.active_request.get_clone_untracked() == job.request_id;
            let fresh = self.accept_research_job(&job);
            let action = terminal_job_action(is_active, fresh);
            if action.do_terminal_work && is_active {
                self.restore_research_observation(&job);
            }
            if action.unlock_active {
                self.finalize_active_request();
            }
            if action.do_terminal_work {
                self.merge_and_discard_terminal_job(&job);
            }
            return;
        }

        if !self.accept_research_job(&job) {
            return;
        }
        // Running job: adopt it as the active request if we are not already
        // tracking it, then reconcile the streamed text with the durable
        // checkpoint without discarding characters already shown.
        let is_current = self.active_request.get_clone_untracked() == job.request_id;
        if !is_current {
            let request_id = job.request_id.clone();
            let assistant_message_id = job.assistant_message_id.clone();
            batch(move || {
                self.active_request.set(request_id);
                self.active_assistant_message.set(assistant_message_id);
                self.is_running.set(true);
                self.last_failed_question.set(String::new());
                self.reset_stream();
            });
        }
        self.restore_research_observation(&job);
        let checkpoint = job.partial_answer;
        let streamed = self.streamed_text.get_clone_untracked();
        let pending = self.pending_stream.get_clone_untracked();
        let mut received = String::with_capacity(streamed.len() + pending.len());
        received.push_str(&streamed);
        received.push_str(&pending);
        if let Some(suffix) = checkpoint.strip_prefix(&received) {
            if !suffix.is_empty() {
                self.pending_stream.update(|queued| queued.push_str(suffix));
                self.pending_stream_request.set(job.request_id);
                self.schedule_pending_stream();
            }
        }
    }

    /// Clears all "a request is in flight" UI state. Idempotent: calling it when
    /// nothing is running is a harmless no-op, so it is always safe to invoke on
    /// any terminal or missing-job signal for the active request.
    fn finalize_active_request(self) {
        batch(move || {
            self.is_running.set(false);
            self.active_request.set(String::new());
            self.active_assistant_message.set(String::new());
            self.reset_stream();
        });
    }

    /// Persists a terminal job into the workspace and, if it was merged,
    /// discards it from the native journal. Safe to skip when a duplicate
    /// delivery was already merged.
    fn merge_and_discard_terminal_job(self, job: &ResearchJob) {
        if self.merge_terminal_research_job(job) {
            let request_id = job.request_id.clone();
            spawn_local(async move {
                let _ =
                    ipc::command::<_, bool>("discard_research_job", &RequestIdArgs { request_id })
                        .await;
            });
        }
    }

    fn merge_terminal_research_job(self, job: &ResearchJob) -> bool {
        let is_active_conversation = self.active_id.get_clone_untracked() == job.conversation_id;
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
            } else if !job.finalizing {
                // The native commit failed after a provisional terminal
                // snapshot was already rendered. Preserve that visible answer
                // through the normal frontend persistence queue, and only
                // discard the durable job after that save succeeds.
                self.persist_workspace_after_research(job.request_id.clone());
                if let Some(error) = &job.error {
                    self.show_toast(error.clone(), "error");
                }
            }
            self.apply_terminal_job_feedback(job, is_active_conversation);
            return job.workspace_persisted;
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
            return !job.finalizing;
        }

        if !content.is_empty() {
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
            if !job.finalizing {
                if job.workspace_persisted {
                    self.workspace.update_silent(|workspace| {
                        workspace.revision = workspace.revision.max(job.workspace_revision);
                    });
                } else {
                    self.persist_workspace_after_research(job.request_id.clone());
                }
            }
        } else if job.workspace_persisted {
            self.reload_workspace_after_research(job.workspace_revision);
        }

        self.apply_terminal_job_feedback(job, is_active_conversation);
        job.workspace_persisted
    }

    fn apply_terminal_job_feedback(self, job: &ResearchJob, is_active_conversation: bool) {
        if job.status == "complete" {
            self.last_failed_question.set(String::new());
            if is_active_conversation {
                let sources = job
                    .result
                    .as_ref()
                    .map(|response| response.sources.clone())
                    .unwrap_or_default();
                self.selected_sources.set(sources);
            }
        } else {
            if is_active_conversation {
                self.last_failed_question.set(job.question.clone());
            }
            if let Some(error) = &job.error {
                self.show_toast(error.clone(), "error");
            }
        }
    }

    fn accept_research_job(self, job: &ResearchJob) -> bool {
        let observation = ResearchJobObservation::from_job(job);
        let previous = self
            .research_job_observations
            .with_untracked(|observations| observations.get(&job.request_id).copied());
        if !observation.should_replace(previous) {
            return false;
        }
        let request_id = job.request_id.clone();
        self.research_job_observations.update(|observations| {
            observations.insert(request_id, observation);
        });
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

    pub(in crate::app) fn reconcile_research_jobs(self, jobs: Vec<ResearchJob>) {
        let active_request = self.active_request.get_clone_untracked();
        let active_present =
            active_request.is_empty() || jobs.iter().any(|job| job.request_id == active_request);
        let mut latest_running = None;

        for job in jobs {
            if job.status != "running" || job.request_id == active_request {
                self.apply_research_job(job);
            } else if active_request.is_empty()
                && latest_running
                    .as_ref()
                    .is_none_or(|current: &ResearchJob| job.updated_at > current.updated_at)
            {
                latest_running = Some(job);
            }
        }

        if !active_present {
            let observed = self
                .research_job_observations
                .with_untracked(|observations| observations.contains_key(&active_request));
            if observed {
                self.clear_missing_active_research(&active_request);
            }
        } else if active_request.is_empty() {
            if let Some(job) = latest_running {
                self.apply_research_job(job);
            }
        }
    }

    pub(in crate::app) fn refresh_active_research_job(self) {
        if !self.is_running.get_untracked() || self.research_sync_busy.get_untracked() {
            return;
        }
        self.reconcile_active_research_job();
    }

    pub(in crate::app) fn reconcile_completed_research_job(self, request_id: String) {
        if !self.is_running.get_untracked()
            || request_id.is_empty()
            || self.active_request.get_clone_untracked() != request_id
        {
            return;
        }
        // A completed stage must never keep the composer, stop button, and
        // conversation navigation locked. Clear the visual in-flight state
        // immediately; the authoritative terminal job below supplies the
        // answer and durable status.
        self.observe_stage_value("done".into(), now_millis());
        self.finalize_active_request();
        self.research_sync_busy.set(true);
        spawn_local(async move {
            match ipc::command::<_, Option<ResearchJob>>(
                "get_research_job",
                &RequestIdArgs {
                    request_id: request_id.clone(),
                },
            )
            .await
            {
                Ok(Some(job)) => self.apply_research_job(job),
                Ok(None) => {
                    // The native side may already have persisted and discarded
                    // the terminal job. Reloading the workspace still recovers
                    // the completed assistant answer without relocking the UI.
                    self.reload_workspace_after_research(0);
                }
                Err(error) => {
                    self.show_toast(
                        format!(
                            "처리 완료 상태를 확인하지 못했습니다. 입력 잠금은 해제했습니다: {error}"
                        ),
                        "warning",
                    );
                }
            }
            self.research_sync_busy.set(false);
        });
    }

    fn reconcile_active_research_job(self) {
        if !self.is_running.get_untracked() {
            return;
        }
        let request_id = self.active_request.get_clone_untracked();
        if request_id.is_empty() {
            self.clear_missing_active_research(&request_id);
            return;
        }
        self.research_sync_busy.set(true);
        spawn_local(async move {
            match ipc::command::<_, Option<ResearchJob>>(
                "get_research_job",
                &RequestIdArgs {
                    request_id: request_id.clone(),
                },
            )
            .await
            {
                Ok(Some(job)) => self.apply_research_job(job),
                Ok(None) => {
                    let observed = self
                        .research_job_observations
                        .with_untracked(|observations| observations.contains_key(&request_id));
                    if observed || !self.research_start_pending.get_untracked() {
                        self.clear_missing_active_research(&request_id);
                    }
                }
                Err(error) => self.show_toast(error, "error"),
            }
            self.research_sync_busy.set(false);
        });
    }

    fn clear_missing_active_research(self, request_id: &str) {
        if !self.is_running.get_untracked()
            || self.active_request.get_clone_untracked() != request_id
        {
            return;
        }
        let terminal_stage = matches!(
            self.stage.get_clone_untracked().as_str(),
            "done" | "failed" | "cancelled" | "interrupted"
        );
        let had_partial = !self.streamed_text.with_untracked(String::is_empty);
        let last_question = if !terminal_stage && had_partial {
            let active_id = self.active_id.get_clone_untracked();
            self.workspace.with_untracked(|workspace| {
                current_conversation_ref(workspace, &active_id)
                    .and_then(|conversation| {
                        conversation
                            .messages
                            .iter()
                            .rev()
                            .find(|message| message.role == "user")
                    })
                    .map(|message| message.content.clone())
                    .unwrap_or_default()
            })
        } else {
            String::new()
        };
        batch(move || {
            self.is_running.set(false);
            self.active_request.set(String::new());
            self.active_assistant_message.set(String::new());
            self.reset_stream();
        });
        if !terminal_stage {
            self.observe_stage("interrupted".into(), now_millis());
            if !last_question.is_empty() {
                self.last_failed_question.set(last_question);
            }
            self.show_toast(
                "실행 중인 작업을 찾지 못해 입력 잠금을 해제했습니다.",
                "warning",
            );
        }
    }

    pub(in crate::app) fn cancel_request(self) {
        let request_id = self.active_request.get_clone_untracked();
        let cancelled_request_id = request_id.clone();
        spawn_local_scoped(async move {
            match ipc::command::<_, bool>("cancel_research", &RequestIdArgs { request_id }).await {
                Ok(true) => self.show_toast("답변 생성을 중단했습니다.", "info"),
                Ok(false) => {
                    self.show_toast("완료된 답변을 불러오는 중입니다.", "info");
                    self.reconcile_completed_research_job(cancelled_request_id);
                }
                Err(error) => self.show_toast(error, "error"),
            }
            self.reconcile_active_research_job();
        });
    }

    pub(in crate::app) fn observe_stage(self, stage: String, occurred_at: u64) {
        if stage == "done" && self.is_running.get_untracked() {
            let request_id = self.active_request.get_clone_untracked();
            self.reconcile_completed_research_job(request_id);
            return;
        }
        self.observe_stage_value(stage, occurred_at);
    }

    fn observe_stage_value(self, stage: String, occurred_at: u64) {
        let changed = self.stage.get_clone_untracked() != stage;
        if changed {
            self.stage.set(stage.clone());
            self.stage_started_at.set(occurred_at);
        }
        self.research_clock.set(occurred_at);
        if changed {
            self.research_events.update(|events| {
                if events
                    .last()
                    .is_some_and(|event| event.kind == "stage" && event.value == stage)
                {
                    return;
                }
                events.push(ResearchEvent {
                    kind: "stage".into(),
                    value: stage,
                    occurred_at,
                });
                if events.len() > 64 {
                    events.drain(..events.len() - 64);
                }
            });
        }
    }

    fn restore_research_observation(self, job: &ResearchJob) {
        let mut events = job.events.clone();
        if events.is_empty() {
            events.push(ResearchEvent {
                kind: "stage".into(),
                value: job.stage.clone(),
                occurred_at: job.created_at,
            });
        }
        let stage_started_at = events
            .iter()
            .rev()
            .find(|event| event.kind == "stage" && event.value == job.stage)
            .map_or(job.updated_at, |event| event.occurred_at);
        self.stage.set(job.stage.clone());
        self.research_started_at.set(job.created_at);
        self.stage_started_at.set(stage_started_at);
        self.research_clock.set(now_millis());
        self.research_events.set(events);
    }
}
