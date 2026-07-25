use super::*;

impl AppState {
    pub(in crate::app) fn queue_stream_delta(self, request_id: String, delta: String) {
        if self.pending_stream_request.get_clone_untracked() != request_id {
            batch(move || {
                self.pending_stream.set(String::new());
                self.pending_stream_request.set(request_id);
            });
        }
        self.pending_stream
            .update(|pending| pending.push_str(&delta));
        if self.stream_frame_pending.get_untracked() {
            return;
        }
        self.stream_frame_pending.set(true);

        let callback = Closure::once_into_js(move || {
            self.stream_frame_pending.set(false);
            let pending_request = self.pending_stream_request.get_clone_untracked();
            if self.active_request.get_clone_untracked() != pending_request {
                batch(move || {
                    self.pending_stream.set(String::new());
                    self.pending_stream_request.set(String::new());
                });
                return;
            }
            self.flush_stream_delta();
        });
        let requested = web_sys::window().is_some_and(|window| {
            window
                .request_animation_frame(callback.unchecked_ref())
                .is_ok()
        });
        if !requested {
            self.stream_frame_pending.set(false);
            self.flush_stream_delta();
        }
    }

    pub(in crate::app) fn flush_stream_delta(self) {
        let pending = self.pending_stream.replace(String::new());
        self.pending_stream_request.set(String::new());
        if !pending.is_empty() {
            self.streamed_text
                .update(|streamed| streamed.push_str(&pending));
        }
    }

    pub(in crate::app) fn reset_stream(self) {
        batch(move || {
            self.pending_stream.set(String::new());
            self.pending_stream_request.set(String::new());
            self.streamed_text.set(String::new());
        });
    }
}
