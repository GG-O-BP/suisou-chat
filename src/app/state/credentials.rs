use super::*;

impl AppState {
    pub(in crate::app) fn connect_key(self, provider: &'static str) {
        let api_key = if provider == "zai" {
            self.zai_key_input.take()
        } else {
            self.sakana_key_input.take()
        };
        let busy = if provider == "zai" {
            self.zai_key_busy
        } else {
            self.sakana_key_busy
        };
        let provider_label = provider_label(provider);
        batch(move || {
            busy.set(true);
            self.show_toast(
                "API 키 확인 후 운영체제 보안 저장소의 잠금 해제 창이 나타나면 완료해 주세요.",
                "info",
            );
        });
        spawn_local_scoped(async move {
            let result = ipc::command::<_, ConnectionInfo>(
                "connect_api_key",
                &ApiKeyArgs {
                    api_key,
                    provider: provider.into(),
                },
            )
            .await;
            busy.set(false);
            match result {
                Ok(info) => {
                    let returned_provider = info.provider.clone();
                    let configured = if provider == "zai" {
                        self.zai_key_configured
                    } else {
                        self.sakana_key_configured
                    };
                    let connection_message = if provider == "zai" {
                        self.zai_connection_message
                    } else {
                        self.sakana_connection_message
                    };
                    let model_note = if info.models.is_empty() {
                        String::new()
                    } else {
                        format!(" · 사용 가능한 모델 {}개", info.models.len())
                    };
                    let provider_matches = returned_provider == provider;
                    batch(move || {
                        if provider_matches {
                            configured.set(true);
                            connection_message.set(format!("{}{model_note}", info.message));
                        }
                    });
                    if provider_matches {
                        self.show_toast(
                            format!(
                                "{provider_label} 연결 상태를 확인하고 키를 안전하게 저장했습니다."
                            ),
                            "success",
                        );
                    } else {
                        self.show_toast("API 공급자 응답이 일치하지 않습니다.", "error");
                    }
                }
                Err(error) => self.show_toast(error, "error"),
            }
        });
    }

    pub(in crate::app) fn clear_key(self, provider: &'static str) {
        let provider_is_running = self.is_running.get_untracked()
            && self.running_provider.get_clone_untracked() == provider;
        if provider_is_running {
            self.show_toast(
                format!(
                    "실행 중인 {} 답변을 먼저 중단한 뒤 API 키 연결을 해제해 주세요.",
                    provider_label(provider)
                ),
                "warning",
            );
            return;
        }
        spawn_local_scoped(async move {
            let result = ipc::command_unit(
                "clear_api_key",
                &ProviderArgs {
                    provider: provider.into(),
                },
            )
            .await;
            let configured = if provider == "zai" {
                self.zai_key_configured
            } else {
                self.sakana_key_configured
            };
            let connection_message = if provider == "zai" {
                self.zai_connection_message
            } else {
                self.sakana_connection_message
            };
            batch(move || {
                configured.set(false);
                connection_message.set(String::new());
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

    pub(in crate::app) fn key_configured_for(self, provider: &str) -> bool {
        if provider == "zai" {
            self.zai_key_configured.get_untracked()
        } else {
            self.sakana_key_configured.get_untracked()
        }
    }
}
