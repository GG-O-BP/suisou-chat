use super::*;

impl AppState {
    pub(in crate::app) fn connect_key(self) {
        let api_key = self.key_input.take();
        batch(move || {
            self.key_busy.set(true);
            self.show_toast(
                "API 키 확인 후 운영체제 보안 저장소의 잠금 해제 창이 나타나면 완료해 주세요.",
                "info",
            );
        });
        spawn_local_scoped(async move {
            let result =
                ipc::command::<_, ConnectionInfo>("connect_api_key", &ApiKeyArgs { api_key }).await;
            self.key_busy.set(false);
            match result {
                Ok(info) => {
                    let model_note = if info.models.is_empty() {
                        String::new()
                    } else {
                        format!(" · 사용 가능한 모델 {}개", info.models.len())
                    };
                    batch(move || {
                        self.key_configured.set(true);
                        self.connection_message
                            .set(format!("{}{model_note}", info.message));
                    });
                    self.show_toast(
                        "Sakana API 연결을 확인하고 키를 안전하게 저장했습니다.",
                        "success",
                    );
                }
                Err(error) => self.show_toast(error, "error"),
            }
        });
    }

    pub(in crate::app) fn clear_key(self) {
        if self.is_running.get_untracked() {
            self.show_toast(
                "실행 중인 답변을 먼저 중단한 뒤 API 키 연결을 해제해 주세요.",
                "warning",
            );
            return;
        }
        spawn_local_scoped(async move {
            let result = ipc::command_unit("clear_api_key", &EmptyArgs {}).await;
            batch(move || {
                self.key_configured.set(false);
                self.connection_message.set(String::new());
            });
            match result {
                Ok(()) => self.show_toast(
                    "API 키를 메모리와 운영체제 보안 저장소에서 제거했습니다.",
                    "success",
                ),
                Err(error) => self.show_toast(error, "error"),
            }
        });
    }
}
