use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Workspace {
    pub version: u32,
    pub revision: u64,
    pub conversations: Vec<Conversation>,
    pub settings: Settings,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    #[serde(default = "default_provider")]
    pub provider: String,
    pub model: String,
    pub reasoning: String,
    pub theme: String,
    pub last_mode: String,
    pub language: String,
    pub sync_mode: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            provider: "sakana".into(),
            model: "fugu".into(),
            reasoning: "high".into(),
            theme: "system".into(),
            last_mode: "search".into(),
            language: "auto".into(),
            sync_mode: "local".into(),
        }
    }
}

fn default_provider() -> String {
    "sakana".into()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub pinned: bool,
    pub created_at: u64,
    pub updated_at: u64,
    pub messages: Vec<Message>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Message {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created_at: u64,
    pub status: String,
    pub sources: Vec<Source>,
    pub usage: Option<Usage>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Source {
    pub id: String,
    pub title: String,
    pub url: String,
    pub domain: String,
    pub snippet: String,
    pub retrieved_at: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub orchestration_tokens: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BootstrapResponse {
    pub workspace: Workspace,
    pub workspace_revision: u64,
    pub credentials: Vec<CredentialStatus>,
    pub credential_notice: Option<String>,
    pub recovery_notice: Option<String>,
    pub storage_label: String,
    pub storage_writable: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CredentialStatus {
    pub provider: String,
    pub key_configured: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct InputMessage {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResearchRequest {
    pub request_id: String,
    pub model: String,
    pub mode: String,
    pub reasoning: String,
    pub messages: Vec<InputMessage>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResearchResponse {
    pub request_id: String,
    pub answer: String,
    pub sources: Vec<Source>,
    pub usage: Option<Usage>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResearchJob {
    pub request_id: String,
    pub conversation_id: String,
    #[serde(default)]
    pub workspace_revision: u64,
    #[serde(default)]
    pub workspace_persisted: bool,
    #[serde(default)]
    pub finalizing: bool,
    pub assistant_message_id: String,
    pub question: String,
    pub mode: String,
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default = "default_model")]
    pub model: String,
    pub status: String,
    pub stage: String,
    pub partial_answer: String,
    pub result: Option<ResearchResponse>,
    pub error: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(default)]
    pub events: Vec<ResearchEvent>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResearchJobObservation {
    pub updated_at: u64,
    pub terminal: bool,
    pub finalizing: bool,
    pub partial_answer_bytes: usize,
    pub event_count: usize,
    pub workspace_persisted: bool,
    pub has_result: bool,
    pub error_bytes: usize,
}

impl ResearchJobObservation {
    pub fn from_job(job: &ResearchJob) -> Self {
        Self {
            updated_at: job.updated_at,
            terminal: job.status != "running",
            finalizing: job.finalizing,
            partial_answer_bytes: job.partial_answer.len(),
            event_count: job.events.len(),
            workspace_persisted: job.workspace_persisted,
            has_result: job.result.is_some(),
            error_bytes: job.error.as_ref().map_or(0, String::len),
        }
    }

    pub fn should_replace(self, previous: Option<Self>) -> bool {
        let Some(previous) = previous else {
            return true;
        };

        if previous.terminal && !self.terminal {
            return false;
        }
        if !previous.terminal && self.terminal {
            return true;
        }
        if self.terminal && previous.terminal {
            if !previous.finalizing && self.finalizing {
                return false;
            }
            if previous.finalizing && !self.finalizing {
                return true;
            }
        }

        if self.updated_at != previous.updated_at {
            return self.updated_at > previous.updated_at;
        }

        self.partial_answer_bytes > previous.partial_answer_bytes
            || self.event_count > previous.event_count
            || (self.workspace_persisted && !previous.workspace_persisted)
            || (self.has_result && !previous.has_result)
            || self.error_bytes > previous.error_bytes
    }
}

#[cfg(test)]
fn merge_stream_checkpoint(current: &str, checkpoint: &str) -> Option<String> {
    if checkpoint.len() <= current.len() {
        return None;
    }
    if checkpoint.starts_with(current) {
        return Some(checkpoint.to_owned());
    }
    None
}

/// Number of characters to reveal from the streaming backlog in a single
/// animation frame.
///
/// The reveal amount is proportional to the outstanding backlog so that a
/// large backlog drains within a bounded, roughly constant number of frames
/// instead of trickling for many seconds. This matters because the answer can
/// arrive all at once — for example when a completion snapshot precedes the
/// live deltas, or when a suspended Android WebView resumes and receives the
/// whole durable checkpoint in one update. A fixed per-frame budget would keep
/// the request "running" (and the composer locked) long after the answer is
/// actually finished. A small live delta still animates smoothly because the
/// per-frame floor keeps up with token-sized deltas without visible jumps.
#[cfg(test)]
pub fn stream_reveal_len(backlog_chars: usize) -> usize {
    if backlog_chars == 0 {
        return 0;
    }
    const MIN_REVEAL: usize = 3;
    const SMOOTHING_DIVISOR: usize = 6;
    backlog_chars
        .div_ceil(SMOOTHING_DIVISOR)
        .max(MIN_REVEAL)
        .min(backlog_chars)
}

/// What the UI must do when a terminal (non-running) research job is observed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct TerminalJobAction {
    /// Clear the in-flight UI state (running flag, active request, composer
    /// lock, stop button). This MUST NOT depend on whether the delivery is a
    /// fresh observation: duplicate or out-of-order terminal deliveries (the
    /// same completion arriving via both the snapshot event and the poll, or
    /// after the observation was already recorded) still have to release the
    /// lock, otherwise the composer stays disabled, the stop button keeps
    /// returning "already complete", and conversation navigation stays blocked.
    pub unlock_active: bool,
    /// Run the one-time merge/persist/observation-restore work. This is gated on
    /// freshness so a duplicate delivery does not append the answer twice.
    pub do_terminal_work: bool,
}

/// Decide how the frontend should react to a terminal research job.
///
/// * `is_active_request` — the job is the one the UI is currently waiting on.
/// * `observation_is_fresh` — the de-duplication layer accepted this delivery
///   as new (not a stale/duplicate/out-of-order snapshot).
pub fn terminal_job_action(
    is_active_request: bool,
    observation_is_fresh: bool,
) -> TerminalJobAction {
    TerminalJobAction {
        // Releasing the active request's lock is unconditional and idempotent.
        unlock_active: is_active_request,
        // Merge/persist only once per distinct terminal delivery.
        do_terminal_work: observation_is_fresh,
    }
}

/// Whether a native progress-stage event must be reconciled as terminal rather
/// than rendered as an active process.
pub fn stage_requires_terminal_reconciliation(stage: &str) -> bool {
    stage == "done"
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct ResearchEvent {
    pub kind: String,
    pub value: String,
    pub occurred_at: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct StartResearchResponse {
    pub job: ResearchJob,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ResearchJobUpdate {
    pub request_id: String,
    pub kind: String,
    pub value: String,
    #[serde(default)]
    pub sequence: u64,
    pub job: Option<ResearchJob>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConnectionInfo {
    pub provider: String,
    pub message: String,
    pub models: Vec<String>,
}

fn default_model() -> String {
    "fugu".into()
}

pub fn provider_for_model(model: &str) -> &'static str {
    if model == "glm-5.3" {
        "zai"
    } else {
        "sakana"
    }
}

pub fn provider_label(provider: &str) -> &'static str {
    match provider {
        "zai" => "Z.ai GLM",
        _ => "Sakana",
    }
}

pub fn new_id(prefix: &str) -> String {
    let timestamp = js_sys::Date::now() as u64;
    let random = (js_sys::Math::random() * 1_000_000_000.0) as u64;
    format!("{prefix}-{timestamp}-{random}")
}

pub fn now_millis() -> u64 {
    js_sys::Date::now() as u64
}

pub fn remove_conversation(workspace: &mut Workspace, id: &str) -> bool {
    if id.is_empty() {
        return false;
    }
    let previous_len = workspace.conversations.len();
    workspace
        .conversations
        .retain(|conversation| conversation.id != id);
    workspace.conversations.len() != previous_len
}

pub fn title_from_question(question: &str) -> String {
    let normalized = question.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = normalized.chars();
    let title = chars.by_ref().take(42).collect::<String>();
    if chars.next().is_some() {
        format!("{title}…")
    } else if title.is_empty() {
        "새 대화".into()
    } else {
        title
    }
}

pub fn format_relative_time(timestamp: u64) -> String {
    let elapsed = now_millis().saturating_sub(timestamp) / 1_000;
    match elapsed {
        0..=59 => "방금".into(),
        60..=3_599 => format!("{}분 전", elapsed / 60),
        3_600..=86_399 => format!("{}시간 전", elapsed / 3_600),
        86_400..=604_799 => format!("{}일 전", elapsed / 86_400),
        _ => {
            let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(timestamp as f64));
            format!("{:02}.{:02}", date.get_month() + 1, date.get_date())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_is_trimmed_and_bounded() {
        assert_eq!(
            title_from_question("  한국의   인공지능 정책  "),
            "한국의 인공지능 정책"
        );
        assert_eq!(title_from_question(" \n\t "), "새 대화");
        assert!(title_from_question(&"가".repeat(100)).chars().count() <= 43);
    }

    #[test]
    fn conversation_removal_preserves_unrelated_history_order() {
        let mut workspace = Workspace {
            conversations: vec![
                Conversation {
                    id: "first".into(),
                    ..Conversation::default()
                },
                Conversation {
                    id: "remove".into(),
                    ..Conversation::default()
                },
                Conversation {
                    id: "last".into(),
                    ..Conversation::default()
                },
            ],
            ..Workspace::default()
        };

        assert!(remove_conversation(&mut workspace, "remove"));
        assert_eq!(
            workspace
                .conversations
                .iter()
                .map(|conversation| conversation.id.as_str())
                .collect::<Vec<_>>(),
            ["first", "last"]
        );
    }

    #[test]
    fn conversation_removal_is_a_noop_for_empty_or_unknown_ids() {
        let mut workspace = Workspace {
            conversations: vec![Conversation {
                id: "kept".into(),
                ..Conversation::default()
            }],
            ..Workspace::default()
        };

        assert!(!remove_conversation(&mut workspace, ""));
        assert!(!remove_conversation(&mut workspace, "missing"));
        assert_eq!(workspace.conversations[0].id, "kept");
    }

    #[test]
    fn research_job_observations_are_monotonic_and_terminal() {
        let running = ResearchJobObservation {
            updated_at: 10,
            terminal: false,
            finalizing: false,
            partial_answer_bytes: 100,
            event_count: 2,
            workspace_persisted: false,
            has_result: false,
            error_bytes: 0,
        };
        assert!(running.should_replace(None));
        assert!(!running.should_replace(Some(running)));

        let richer_same_millis = ResearchJobObservation {
            partial_answer_bytes: 120,
            ..running
        };
        assert!(richer_same_millis.should_replace(Some(running)));

        let older_running = ResearchJobObservation {
            updated_at: 9,
            partial_answer_bytes: 200,
            ..running
        };
        assert!(!older_running.should_replace(Some(richer_same_millis)));

        let terminal = ResearchJobObservation {
            updated_at: 9,
            terminal: true,
            finalizing: true,
            has_result: true,
            ..running
        };
        assert!(terminal.should_replace(Some(richer_same_millis)));

        let finalized = ResearchJobObservation {
            finalizing: false,
            workspace_persisted: true,
            ..terminal
        };
        assert!(finalized.should_replace(Some(terminal)));
        assert!(!terminal.should_replace(Some(finalized)));

        let stale_terminal = ResearchJobObservation {
            updated_at: 9,
            terminal: true,
            ..finalized
        };
        assert!(!stale_terminal.should_replace(Some(finalized)));

        let late_running = ResearchJobObservation {
            updated_at: 20,
            terminal: false,
            partial_answer_bytes: 500,
            ..running
        };
        assert!(!late_running.should_replace(Some(finalized)));
    }

    #[test]
    fn stream_checkpoints_only_advance_matching_content() {
        assert_eq!(
            merge_stream_checkpoint("앞부분", "앞부분과 이어진 내용"),
            Some("앞부분과 이어진 내용".into())
        );
        assert_eq!(merge_stream_checkpoint("앞부분", "앞부분"), None);
        assert_eq!(
            merge_stream_checkpoint("앞부분과 최신 델타", "앞부분"),
            None
        );
        assert_eq!(merge_stream_checkpoint("앞부분", "다른 내용"), None);
    }

    #[test]
    fn stream_reveal_is_smooth_for_small_backlogs_and_bounded_for_large_ones() {
        // Nothing queued reveals nothing.
        assert_eq!(stream_reveal_len(0), 0);
        // A tiny live delta reveals the whole backlog at once (no visible lag),
        // but never asks for more characters than are available.
        assert_eq!(stream_reveal_len(1), 1);
        assert_eq!(stream_reveal_len(2), 2);
        for backlog in 1..=8 {
            assert!(stream_reveal_len(backlog) <= backlog);
        }
        // A large backlog (e.g. a completion snapshot or a resumed Android
        // WebView delivering the whole answer at once) drains within a bounded
        // number of frames instead of trickling for many seconds. At 60fps a
        // frame budget this large finishes well under a second.
        let huge = 60_000;
        let reveal = stream_reveal_len(huge);
        assert!(reveal >= huge / 7, "backlog should drain quickly: {reveal}");

        // Draining a large backlog frame-by-frame always terminates and never
        // stalls: each step removes at least one character and converges fast.
        let mut remaining = huge;
        let mut frames = 0;
        while remaining > 0 {
            let step = stream_reveal_len(remaining);
            assert!(step >= 1);
            remaining -= step;
            frames += 1;
            assert!(frames < 200, "reveal must converge quickly");
        }
    }

    #[test]
    fn terminal_delivery_always_unlocks_active_request_even_when_deduplicated() {
        // Fresh terminal for the active request: unlock and do the merge work.
        let fresh = terminal_job_action(true, true);
        assert!(fresh.unlock_active);
        assert!(fresh.do_terminal_work);

        // Duplicate/out-of-order terminal for the active request (dedup rejects
        // it): the UI MUST still unlock, but the merge work is skipped. This is
        // the reported bug — a second terminal delivery must not leave the
        // composer disabled, the stop button live, and navigation blocked.
        let duplicate = terminal_job_action(true, false);
        assert!(duplicate.unlock_active);
        assert!(!duplicate.do_terminal_work);

        // Fresh terminal for a non-active request (e.g. a prior conversation's
        // background job): do the merge work, but never touch the active lock.
        let other_fresh = terminal_job_action(false, true);
        assert!(!other_fresh.unlock_active);
        assert!(other_fresh.do_terminal_work);

        // Duplicate terminal for a non-active request: nothing to do.
        let other_duplicate = terminal_job_action(false, false);
        assert!(!other_duplicate.unlock_active);
        assert!(!other_duplicate.do_terminal_work);
    }

    #[test]
    fn completed_stage_is_terminal_but_failure_stages_remain_snapshot_driven() {
        assert!(stage_requires_terminal_reconciliation("done"));
        assert!(!stage_requires_terminal_reconciliation("writing"));
        assert!(!stage_requires_terminal_reconciliation("failed"));
        assert!(!stage_requires_terminal_reconciliation("cancelled"));
    }
}
