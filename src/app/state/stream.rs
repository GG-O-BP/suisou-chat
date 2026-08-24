use super::*;

impl AppState {
    pub(in crate::app) fn queue_stream_delta(
        self,
        request_id: String,
        sequence: u64,
        delta: String,
    ) {
        if sequence != 0 {
            let previous = self.last_stream_sequence.get_untracked();
            if previous != 0 && sequence != previous.saturating_add(1) {
                self.refresh_active_research_job();
            }
            if sequence <= previous {
                return;
            }
            self.last_stream_sequence.set(sequence);
        }
        if self.pending_stream_request.get_clone_untracked() != request_id {
            batch(move || {
                self.pending_stream.set(String::new());
                self.pending_stream_request.set(request_id);
            });
        }
        self.pending_stream
            .update(|pending| pending.push_str(&delta));
        self.flush_pending_stream();
    }

    /// Move any buffered delta text into the visible answer signal.
    ///
    /// This is deliberately synchronous. Native callbacks are re-entered through
    /// the captured Sycamore scope in `AppRuntime`; adding a second per-delta
    /// `requestAnimationFrame` callback only creates another asynchronous boundary
    /// in the hottest path. Appending directly is cheap: the answer renders into a
    /// single text node and viewport auto-scroll is already throttled to the
    /// one-second research clock.
    pub(in crate::app) fn schedule_pending_stream(self) {
        self.flush_pending_stream();
    }

    fn flush_pending_stream(self) {
        let pending_request = self.pending_stream_request.get_clone_untracked();
        if self.active_request.get_clone_untracked() != pending_request {
            batch(move || {
                self.pending_stream.set(String::new());
                self.pending_stream_request.set(String::new());
            });
            return;
        }
        let pending = self.pending_stream.replace(String::new());
        if !pending.is_empty() {
            self.streamed_text
                .update(|streamed| streamed.push_str(&pending));
        }
    }

    pub(in crate::app) fn reset_stream(self) {
        self.pending_stream.set(String::new());
        self.pending_stream_request.set(String::new());
        self.streamed_text.set(String::new());
        self.last_stream_sequence.set(0);
    }
}
