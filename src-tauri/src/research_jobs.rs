use crate::fugu::FuguRuntime;
use crate::models::{
    Message, ResearchEvent, ResearchJob, ResearchJobUpdate, ResearchRequest, ResearchResponse,
    StartResearchResponse,
};
use crate::storage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::sync::{OnceLock, Weak};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

const RESEARCH_JOB_EVENT: &str = "research-job-event";
const JOURNAL_VERSION: u32 = 1;
const MAX_JOBS: usize = 200;
const MAX_RESEARCH_EVENTS: usize = 64;
const MAX_PARTIAL_ANSWER_BYTES: usize = 4 * 1024 * 1024;
const CHECKPOINT_INTERVAL_MILLIS: u64 = 1_000;
const CHECKPOINT_BYTES: usize = 16 * 1024;
static REGISTERED_MANAGER: OnceLock<Mutex<Weak<ResearchJobManager>>> = OnceLock::new();

pub trait BackgroundExecution: Send + Sync {
    fn start(&self, job: &ResearchJob) -> Result<(), String>;
    fn update(&self, job: &ResearchJob);
    fn stop(&self, request_id: &str, succeeded: bool);
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub struct NoopBackgroundExecution;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl BackgroundExecution for NoopBackgroundExecution {
    fn start(&self, _job: &ResearchJob) -> Result<(), String> {
        Ok(())
    }

    fn update(&self, _job: &ResearchJob) {}

    fn stop(&self, _request_id: &str, _succeeded: bool) {}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ResearchJobJournal {
    version: u32,
    jobs: Vec<ResearchJob>,
}

impl Default for ResearchJobJournal {
    fn default() -> Self {
        Self {
            version: JOURNAL_VERSION,
            jobs: Vec::new(),
        }
    }
}

struct ActiveJob {
    job: ResearchJob,
    cancellation: CancellationToken,
    finalizing: bool,
    progress_sequence: u64,
    background_started: bool,
    background_has_output: bool,
    checkpoint_at: u64,
    checkpoint_len: usize,
}

pub struct ResearchJobManager {
    app: AppHandle,
    fugu: Arc<FuguRuntime>,
    journal_path: PathBuf,
    workspace_path: PathBuf,
    workspace_lock: Arc<Mutex<()>>,
    background: Arc<dyn BackgroundExecution>,
    jobs: Mutex<HashMap<String, ActiveJob>>,
    journal_lock: Mutex<()>,
}

impl ResearchJobManager {
    pub fn new(
        app: AppHandle,
        fugu: Arc<FuguRuntime>,
        journal_path: PathBuf,
        workspace_path: PathBuf,
        workspace_lock: Arc<Mutex<()>>,
        background: Arc<dyn BackgroundExecution>,
    ) -> Result<Self, String> {
        let journal = load_journal(&journal_path)?;
        let jobs = journal
            .jobs
            .into_iter()
            .map(|mut job| {
                // A process restart means no in-memory finalization is still in
                // flight. Preserve the terminal payload, but never leave a
                // durable journal entry permanently marked provisional.
                job.finalizing = false;
                if job.status == "running" {
                    job.status = "interrupted".into();
                    job.stage = "interrupted".into();
                    job.error = Some(
                        "앱 실행이 종료되어 이전 연구를 이어갈 수 없습니다. 다시 시도해 주세요."
                            .into(),
                    );
                    job.updated_at = now_millis();
                }
                (
                    job.request_id.clone(),
                    ActiveJob {
                        checkpoint_at: job.updated_at,
                        checkpoint_len: job.partial_answer.len(),
                        background_has_output: !job.partial_answer.is_empty(),
                        job,
                        cancellation: CancellationToken::new(),
                        finalizing: false,
                        progress_sequence: 0,
                        background_started: false,
                    },
                )
            })
            .collect::<HashMap<_, _>>();
        let manager = Self {
            app,
            fugu,
            journal_path,
            workspace_path,
            workspace_lock,
            background,
            jobs: Mutex::new(jobs),
            journal_lock: Mutex::new(()),
        };
        manager.persist()?;
        Ok(manager)
    }

    pub fn register(manager: &Arc<Self>) {
        let slot = REGISTERED_MANAGER.get_or_init(|| Mutex::new(Weak::new()));
        if let Ok(mut registered) = slot.lock() {
            *registered = Arc::downgrade(manager);
        }
    }

    pub fn start(
        self: &Arc<Self>,
        conversation_id: String,
        workspace_revision: u64,
        assistant_message_id: String,
        question: String,
        request: ResearchRequest,
    ) -> Result<StartResearchResponse, String> {
        self.ensure_can_start(&conversation_id, &assistant_message_id, &question, &request)?;

        let now = now_millis();
        let job = ResearchJob {
            request_id: request.request_id.clone(),
            conversation_id,
            workspace_revision,
            workspace_persisted: false,
            finalizing: false,
            assistant_message_id,
            question,
            mode: request.mode.clone(),
            status: "running".into(),
            stage: "connecting".into(),
            partial_answer: String::new(),
            result: None,
            error: None,
            created_at: now,
            updated_at: now,
            events: vec![ResearchEvent {
                kind: "stage".into(),
                value: "connecting".into(),
                occurred_at: now,
            }],
        };
        let cancellation = CancellationToken::new();
        {
            let mut jobs = self
                .jobs
                .lock()
                .map_err(|_| "연구 작업 상태를 잠글 수 없습니다.".to_string())?;
            if jobs.contains_key(&job.request_id) {
                return Err("같은 ID의 연구 작업이 이미 존재합니다.".into());
            }
            jobs.insert(
                job.request_id.clone(),
                ActiveJob {
                    checkpoint_at: now,
                    checkpoint_len: 0,
                    background_has_output: false,
                    job: job.clone(),
                    cancellation: cancellation.clone(),
                    finalizing: false,
                    progress_sequence: 0,
                    background_started: false,
                },
            );
        }
        self.persist()?;
        let background_started = self.background.start(&job).is_ok();
        if background_started {
            if let Ok(mut jobs) = self.jobs.lock() {
                if let Some(active) = jobs.get_mut(&job.request_id) {
                    active.background_started = true;
                }
            }
        }
        let mut started_job = job.clone();
        started_job.workspace_revision = workspace_revision;
        self.emit_snapshot(&started_job);

        let manager = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            manager
                .run_job(request, cancellation, background_started)
                .await;
        });
        Ok(StartResearchResponse { job: started_job })
    }

    pub fn ensure_can_start(
        &self,
        conversation_id: &str,
        assistant_message_id: &str,
        question: &str,
        request: &ResearchRequest,
    ) -> Result<(), String> {
        validate_context_id(conversation_id, "대화")?;
        validate_context_id(assistant_message_id, "답변")?;
        crate::models::validate_research_request(request)?;
        if question.trim().is_empty() || question.chars().count() > 20_000 {
            return Err("질문 내용이 올바르지 않습니다.".into());
        }
        let jobs = self
            .jobs
            .lock()
            .map_err(|_| "연구 작업 상태를 잠글 수 없습니다.".to_string())?;
        if jobs
            .values()
            .any(|active| active.job.status == "running" || active.finalizing)
        {
            return Err("이전 연구 결과를 마무리하고 있습니다. 잠시 후 다시 시도해 주세요.".into());
        }
        if jobs.contains_key(&request.request_id) {
            return Err("같은 ID의 연구 작업이 이미 존재합니다.".into());
        }
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<ResearchJob>, String> {
        let jobs = self
            .jobs
            .lock()
            .map_err(|_| "연구 작업 상태를 잠글 수 없습니다.".to_string())?;
        let mut values = jobs
            .values()
            .map(|active| active.job.clone())
            .collect::<Vec<_>>();
        values.sort_by_key(|job| job.created_at);
        Ok(values)
    }

    pub fn has_running(&self) -> Result<bool, String> {
        self.jobs
            .lock()
            .map_err(|_| "연구 작업 상태를 잠글 수 없습니다.".to_string())
            .map(|jobs| jobs.values().any(|active| active.job.status == "running"))
    }

    pub fn get(&self, request_id: &str) -> Result<Option<ResearchJob>, String> {
        validate_request_id(request_id)?;
        self.jobs
            .lock()
            .map_err(|_| "연구 작업 상태를 잠글 수 없습니다.".to_string())
            .map(|jobs| jobs.get(request_id).map(|active| active.job.clone()))
    }

    pub fn cancel(&self, request_id: &str) -> Result<bool, String> {
        validate_request_id(request_id)?;
        let jobs = self
            .jobs
            .lock()
            .map_err(|_| "연구 작업 상태를 잠글 수 없습니다.".to_string())?;
        let Some(active) = jobs.get(request_id) else {
            return Ok(false);
        };
        if active.job.status != "running" || active.finalizing {
            return Ok(false);
        }
        active.cancellation.cancel();
        Ok(true)
    }

    pub fn discard(&self, request_id: &str) -> Result<bool, String> {
        validate_request_id(request_id)?;
        let removed = {
            let mut jobs = self
                .jobs
                .lock()
                .map_err(|_| "연구 작업 상태를 잠글 수 없습니다.".to_string())?;
            if let Some(active) = jobs.get(request_id) {
                if active.finalizing {
                    return Err("완료 결과를 저장 중인 연구 작업은 삭제할 수 없습니다.".into());
                }
                if active.job.status == "running" {
                    return Err("실행 중인 연구 작업은 삭제할 수 없습니다.".into());
                }
            }
            jobs.remove(request_id).is_some()
        };
        if removed {
            self.persist()?;
        }
        Ok(removed)
    }

    async fn run_job(
        self: Arc<Self>,
        request: ResearchRequest,
        cancellation: CancellationToken,
        background_started: bool,
    ) {
        let request_id = request.request_id.clone();
        let manager_for_events = Arc::clone(&self);
        let event_request_id = request_id.clone();
        let research = std::panic::AssertUnwindSafe(self.fugu.research(
            request,
            cancellation,
            move |kind, value| {
                manager_for_events.handle_progress(&event_request_id, kind, value);
            },
        ));
        let result = match futures_util::FutureExt::catch_unwind(research).await {
            Ok(result) => result,
            Err(_) => Err("연구 작업이 예기치 않게 중단되었습니다. 다시 시도해 주세요.".into()),
        };
        match result {
            Ok(response) => {
                if let Err(error) =
                    self.finish_with_result(&request_id, response, background_started)
                {
                    let _ =
                        self.finish_with_error(&request_id, "failed", error, background_started);
                }
            }
            Err(error) => {
                let status = if error.contains("중단") {
                    "cancelled"
                } else {
                    "failed"
                };
                let _ = self.finish_with_error(&request_id, status, error, background_started);
            }
        }
    }

    fn handle_progress(&self, request_id: &str, kind: &str, value: &str) {
        let now = now_millis();
        let (persist, background_update, sequence) = {
            let Ok(mut jobs) = self.jobs.lock() else {
                return;
            };
            let Some(active) = jobs.get_mut(request_id) else {
                return;
            };
            if active.job.status != "running" {
                return;
            }
            let mut persist = false;
            let mut background_update = None;
            match kind {
                "stage" => {
                    if active.job.stage == value {
                        return;
                    }
                    active.job.stage = value.to_owned();
                    push_research_event(
                        &mut active.job.events,
                        ResearchEvent {
                            kind: "stage".into(),
                            value: value.to_owned(),
                            occurred_at: now,
                        },
                    );
                    persist = true;
                    if active.background_started {
                        background_update = Some(active.job.clone());
                    }
                }
                "delta" => {
                    let had_output = !active.job.partial_answer.is_empty();
                    if active.job.partial_answer.len().saturating_add(value.len())
                        <= MAX_PARTIAL_ANSWER_BYTES
                    {
                        active.job.partial_answer.push_str(value);
                    }
                    let has_output = !active.job.partial_answer.is_empty();
                    if active.background_started && !active.background_has_output && has_output {
                        active.background_has_output = true;
                        background_update = Some(active.job.clone());
                    }
                    if now.saturating_sub(active.checkpoint_at) >= CHECKPOINT_INTERVAL_MILLIS
                        || active
                            .job
                            .partial_answer
                            .len()
                            .saturating_sub(active.checkpoint_len)
                            >= CHECKPOINT_BYTES
                    {
                        active.checkpoint_at = now;
                        active.checkpoint_len = active.job.partial_answer.len();
                        persist = true;
                    }
                    if had_output == has_output && value.is_empty() {
                        return;
                    }
                }
                _ => return,
            }
            active.job.updated_at = now;
            let sequence = if kind == "delta" {
                active.progress_sequence = active.progress_sequence.saturating_add(1);
                active.progress_sequence
            } else {
                0
            };
            (persist, background_update, sequence)
        };
        if persist {
            let _ = self.persist();
        }
        if let Some(job) = background_update {
            self.background.update(&job);
        }
        self.emit_progress(request_id, kind, value, sequence);
    }

    fn finish_with_result(
        &self,
        request_id: &str,
        response: ResearchResponse,
        background_started: bool,
    ) -> Result<(), String> {
        let mut job = {
            let mut jobs = self
                .jobs
                .lock()
                .map_err(|_| "연구 작업 상태를 잠글 수 없습니다.".to_string())?;
            let active = jobs
                .get_mut(request_id)
                .ok_or_else(|| "완료할 연구 작업을 찾지 못했습니다.".to_string())?;
            active.finalizing = true;
            active.job.finalizing = true;
            active.job.status = "complete".into();
            active.job.stage = "done".into();
            push_research_event(
                &mut active.job.events,
                ResearchEvent {
                    kind: "stage".into(),
                    value: "done".into(),
                    occurred_at: now_millis(),
                },
            );
            active.job.partial_answer = response.answer.clone();
            active.job.result = Some(response);
            active.job.error = None;
            active.job.updated_at = now_millis();
            active.job.clone()
        };
        // Publish the terminal payload before synchronous workspace I/O. A
        // mobile WebView can therefore unlock and display the answer even when
        // storage is slow; `finalizing` prevents it from racing the native
        // commit by persisting/discarding this provisional snapshot.
        self.emit_snapshot(&job);
        self.persist_terminal_to_workspace(&mut job);
        job.finalizing = false;
        job.updated_at = now_millis().max(job.updated_at);
        self.replace_job(job.clone())?;
        self.persist()?;
        if background_started {
            self.background.update(&job);
            self.background.stop(request_id, true);
        }
        self.emit_snapshot(&job);
        Ok(())
    }

    fn finish_with_error(
        &self,
        request_id: &str,
        status: &str,
        error: String,
        background_started: bool,
    ) -> Result<(), String> {
        let mut job = {
            let mut jobs = self
                .jobs
                .lock()
                .map_err(|_| "연구 작업 상태를 잠글 수 없습니다.".to_string())?;
            let active = jobs
                .get_mut(request_id)
                .ok_or_else(|| "완료할 연구 작업을 찾지 못했습니다.".to_string())?;
            active.finalizing = true;
            active.job.finalizing = true;
            active.job.status = status.to_owned();
            active.job.stage = status.to_owned();
            push_research_event(
                &mut active.job.events,
                ResearchEvent {
                    kind: "stage".into(),
                    value: status.to_owned(),
                    occurred_at: now_millis(),
                },
            );
            active.job.error = Some(error);
            active.job.updated_at = now_millis();
            active.job.clone()
        };
        self.emit_snapshot(&job);
        self.persist_terminal_to_workspace(&mut job);
        job.finalizing = false;
        job.updated_at = now_millis().max(job.updated_at);
        self.replace_job(job.clone())?;
        self.persist()?;
        if background_started {
            self.background.update(&job);
            self.background.stop(request_id, false);
        }
        self.emit_snapshot(&job);
        Ok(())
    }

    fn emit_snapshot(&self, job: &ResearchJob) {
        let _ = self.app.emit(
            RESEARCH_JOB_EVENT,
            ResearchJobUpdate {
                request_id: job.request_id.clone(),
                kind: "snapshot".into(),
                value: String::new(),
                sequence: 0,
                job: Some(job.clone()),
            },
        );
    }

    fn emit_progress(&self, request_id: &str, kind: &str, value: &str, sequence: u64) {
        let _ = self.app.emit(
            RESEARCH_JOB_EVENT,
            ResearchJobUpdate {
                request_id: request_id.to_owned(),
                kind: kind.to_owned(),
                value: value.to_owned(),
                sequence,
                job: None,
            },
        );
    }

    fn replace_job(&self, job: ResearchJob) -> Result<(), String> {
        let mut jobs = self
            .jobs
            .lock()
            .map_err(|_| "연구 작업 상태를 잠글 수 없습니다.".to_string())?;
        let active = jobs
            .get_mut(&job.request_id)
            .ok_or_else(|| "갱신할 연구 작업을 찾지 못했습니다.".to_string())?;
        active.job = job;
        active.finalizing = false;
        Ok(())
    }

    fn persist_terminal_to_workspace(&self, job: &mut ResearchJob) {
        match self.try_persist_terminal_to_workspace(job) {
            Ok(revision) => {
                job.workspace_revision = revision;
                job.workspace_persisted = true;
            }
            Err(error) => {
                job.workspace_persisted = false;
                job.error = Some(match job.error.take() {
                    Some(previous) => format!("{previous} 결과 저장 실패: {error}"),
                    None => format!("답변은 완료됐지만 대화 기록에 저장하지 못했습니다: {error}"),
                });
            }
        }
    }

    fn try_persist_terminal_to_workspace(&self, job: &ResearchJob) -> Result<u64, String> {
        let _guard = self
            .workspace_lock
            .lock()
            .map_err(|_| "대화 기록 저장을 잠글 수 없습니다.".to_string())?;
        let loaded = storage::load_workspace(&self.workspace_path);
        if loaded.warning.is_some() && !loaded.recovered_from_backup {
            return Err("기존 대화 기록을 복구하기 전에는 결과를 저장할 수 없습니다.".into());
        }
        let mut workspace = loaded.workspace;
        let Some(conversation) = workspace
            .conversations
            .iter_mut()
            .find(|conversation| conversation.id == job.conversation_id)
        else {
            return Err("결과를 적용할 대화를 찾지 못했습니다.".into());
        };
        if conversation
            .messages
            .iter()
            .any(|message| message.id == job.assistant_message_id)
        {
            return Ok(workspace.revision);
        }

        let (content, status, sources, usage) = if let Some(response) = &job.result {
            (
                response.answer.clone(),
                "complete".to_string(),
                response.sources.clone(),
                response.usage.clone(),
            )
        } else if !job.partial_answer.trim().is_empty() {
            (
                job.partial_answer.clone(),
                if job.status == "cancelled" {
                    "cancelled".to_string()
                } else {
                    "failed".to_string()
                },
                Vec::new(),
                None,
            )
        } else {
            return Ok(workspace.revision);
        };

        let now = now_millis();
        conversation.updated_at = now;
        conversation.messages.push(Message {
            id: job.assistant_message_id.clone(),
            role: "assistant".into(),
            content,
            created_at: now,
            status,
            sources,
            usage,
        });
        workspace.revision = workspace
            .revision
            .checked_add(1)
            .ok_or_else(|| "대화 기록의 저장 버전 한도에 도달했습니다.".to_string())?;
        storage::save_workspace(&self.workspace_path, &workspace)?;
        Ok(workspace.revision)
    }

    fn persist(&self) -> Result<(), String> {
        let _guard = self
            .journal_lock
            .lock()
            .map_err(|_| "연구 작업 저장을 잠글 수 없습니다.".to_string())?;
        let mut jobs = self
            .jobs
            .lock()
            .map_err(|_| "연구 작업 상태를 잠글 수 없습니다.".to_string())?
            .values()
            .map(|active| active.job.clone())
            .collect::<Vec<_>>();
        jobs.sort_by_key(|job| job.created_at);
        while jobs.len() > MAX_JOBS {
            let Some(index) = jobs.iter().position(|job| job.status != "running") else {
                break;
            };
            jobs.remove(index);
        }
        save_journal(
            &self.journal_path,
            &ResearchJobJournal {
                version: JOURNAL_VERSION,
                jobs,
            },
        )
    }
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn cancel_registered(request_id: &str) -> bool {
    let Some(slot) = REGISTERED_MANAGER.get() else {
        return false;
    };
    let Ok(registered) = slot.lock() else {
        return false;
    };
    registered
        .upgrade()
        .and_then(|manager| manager.cancel(request_id).ok())
        .unwrap_or(false)
}

fn validate_context_id(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 160
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
    {
        Err(format!("잘못된 {label} ID입니다."))
    } else {
        Ok(())
    }
}

fn validate_request_id(request_id: &str) -> Result<(), String> {
    validate_context_id(request_id, "요청")
}

fn push_research_event(events: &mut Vec<ResearchEvent>, event: ResearchEvent) {
    if events
        .last()
        .is_some_and(|previous| previous.kind == event.kind && previous.value == event.value)
    {
        return;
    }
    events.push(event);
    if events.len() > MAX_RESEARCH_EVENTS {
        events.drain(..events.len() - MAX_RESEARCH_EVENTS);
    }
}

fn load_journal(path: &Path) -> Result<ResearchJobJournal, String> {
    if !path.exists() {
        return Ok(ResearchJobJournal::default());
    }
    match read_journal(path) {
        Ok(journal) => Ok(journal),
        Err(primary_error) => {
            let backup = journal_backup_path(path);
            if let Ok(journal) = read_journal(&backup) {
                fs::copy(&backup, path).map_err(|error| {
                    format!(
                        "연구 작업 기록은 백업에서 읽었지만 기본 파일 복원에 실패했습니다: {error}"
                    )
                })?;
                return Ok(journal);
            }
            Err(format!(
                "기존 연구 작업 기록을 읽지 못했습니다. 손상된 기록을 덮어쓰지 않도록 앱 시작을 중단했습니다: {primary_error}"
            ))
        }
    }
}

fn read_journal(path: &Path) -> Result<ResearchJobJournal, String> {
    let bytes = fs::read(path).map_err(|error| format!("연구 작업 기록 읽기 실패: {error}"))?;
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("연구 작업 기록이 안전한 크기 제한을 초과했습니다.".into());
    }
    let journal: ResearchJobJournal = serde_json::from_slice(&bytes)
        .map_err(|error| format!("연구 작업 기록 해석 실패: {error}"))?;
    if journal.version != JOURNAL_VERSION || journal.jobs.len() > MAX_JOBS {
        return Err("지원하지 않는 연구 작업 기록입니다.".into());
    }
    for job in &journal.jobs {
        validate_request_id(&job.request_id)?;
        validate_context_id(&job.conversation_id, "대화")?;
        validate_context_id(&job.assistant_message_id, "답변")?;
        if job.partial_answer.len() > MAX_PARTIAL_ANSWER_BYTES {
            return Err("연구 작업의 부분 답변이 안전한 크기 제한을 초과했습니다.".into());
        }
    }
    Ok(journal)
}

fn save_journal(path: &Path, journal: &ResearchJobJournal) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "연구 작업 저장 경로가 올바르지 않습니다.".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("연구 작업 폴더 생성 실패: {error}"))?;
    let bytes = serde_json::to_vec_pretty(journal)
        .map_err(|error| format!("연구 작업 직렬화 실패: {error}"))?;
    let temporary = parent.join(format!(".research-jobs-{}.tmp", std::process::id()));
    let mut file = fs::File::create(&temporary)
        .map_err(|error| format!("연구 작업 임시 파일 생성 실패: {error}"))?;
    file.write_all(&bytes)
        .map_err(|error| format!("연구 작업 쓰기 실패: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("연구 작업 동기화 실패: {error}"))?;
    if read_journal(path).is_ok() {
        fs::copy(path, journal_backup_path(path))
            .map_err(|error| format!("연구 작업 백업 생성 실패: {error}"))?;
    }
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("연구 작업 저장 확정 실패: {error}")
    })?;
    let _ = fs::copy(path, journal_backup_path(path));
    Ok(())
}

fn journal_backup_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".bak");
    PathBuf::from(name)
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn job(status: &str) -> ResearchJob {
        ResearchJob {
            request_id: "request-1".into(),
            conversation_id: "conversation-1".into(),
            workspace_revision: 3,
            workspace_persisted: false,
            finalizing: false,
            assistant_message_id: "message-1".into(),
            question: "질문".into(),
            mode: "search".into(),
            status: status.into(),
            stage: status.into(),
            partial_answer: "부분 답변".into(),
            result: None,
            error: None,
            created_at: 1,
            updated_at: 2,
            events: Vec::new(),
        }
    }

    #[test]
    fn journal_round_trip_preserves_terminal_jobs() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("jobs.json");
        let mut completed = job("complete");
        completed.events = vec![ResearchEvent {
            kind: "stage".into(),
            value: "writing".into(),
            occurred_at: 2,
        }];
        let journal = ResearchJobJournal {
            version: JOURNAL_VERSION,
            jobs: vec![completed],
        };
        save_journal(&path, &journal).unwrap();
        let loaded = load_journal(&path).unwrap();
        assert_eq!(loaded.jobs[0].partial_answer, "부분 답변");
        assert_eq!(loaded.jobs[0].events[0].value, "writing");
    }

    #[test]
    fn progress_events_are_ordered_deduplicated_and_bounded() {
        let mut events = Vec::new();
        push_research_event(
            &mut events,
            ResearchEvent {
                kind: "stage".into(),
                value: "connecting".into(),
                occurred_at: 1,
            },
        );
        push_research_event(
            &mut events,
            ResearchEvent {
                kind: "stage".into(),
                value: "connecting".into(),
                occurred_at: 2,
            },
        );
        assert_eq!(events.len(), 1);

        for index in 0..MAX_RESEARCH_EVENTS + 4 {
            push_research_event(
                &mut events,
                ResearchEvent {
                    kind: "stage".into(),
                    value: format!("stage-{index}"),
                    occurred_at: index as u64 + 3,
                },
            );
        }
        assert_eq!(events.len(), MAX_RESEARCH_EVENTS);
        assert_eq!(events.last().unwrap().value, "stage-67");
        assert!(events[0].occurred_at < events.last().unwrap().occurred_at);
    }

    #[test]
    fn journal_rejects_unsafe_identifiers_and_oversized_partial_answers() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("jobs.json");
        let mut invalid = job("failed");
        invalid.request_id = "../request".into();
        save_journal(
            &path,
            &ResearchJobJournal {
                version: JOURNAL_VERSION,
                jobs: vec![invalid],
            },
        )
        .unwrap();
        assert!(load_journal(&path).is_err());

        let mut oversized = job("failed");
        oversized.partial_answer = "x".repeat(MAX_PARTIAL_ANSWER_BYTES + 1);
        save_journal(
            &path,
            &ResearchJobJournal {
                version: JOURNAL_VERSION,
                jobs: vec![oversized],
            },
        )
        .unwrap();
        assert!(load_journal(&path).is_err());
    }

    #[test]
    fn journal_recovers_a_valid_backup_without_overwriting_both_corrupt_copies() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("jobs.json");
        let backup = journal_backup_path(&path);
        let journal = ResearchJobJournal {
            version: JOURNAL_VERSION,
            jobs: vec![job("complete")],
        };
        fs::write(&backup, serde_json::to_vec(&journal).unwrap()).unwrap();
        fs::write(&path, b"not-json").unwrap();

        let loaded = load_journal(&path).unwrap();
        assert_eq!(loaded.jobs[0].request_id, "request-1");
        assert_eq!(read_journal(&path).unwrap().jobs.len(), 1);

        fs::write(&backup, b"also-broken").unwrap();
        fs::write(&path, b"still-broken").unwrap();
        assert!(load_journal(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"still-broken");
    }
}
