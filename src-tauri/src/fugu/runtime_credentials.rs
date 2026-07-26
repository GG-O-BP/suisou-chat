use super::*;

impl FuguRuntime {
    pub fn new() -> Result<Self, String> {
        let (api_key_store, initialization_notice): (Arc<dyn ApiKeyStore>, Option<String>) =
            match SystemApiKeyStore::new() {
                Ok(store) => (Arc::new(store), None),
                Err(error) => (
                    Arc::new(UnavailableApiKeyStore::new(error.clone())),
                    Some(error),
                ),
            };
        Self::new_with_store(api_key_store, initialization_notice)
    }

    pub(super) fn new_with_store(
        api_key_store: Arc<dyn ApiKeyStore>,
        initialization_notice: Option<String>,
    ) -> Result<Self, String> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .timeout(Duration::from_secs(2 * 60 * 60))
            .user_agent("Suisou/0.1")
            .build()
            .map_err(|error| format!("HTTP 클라이언트 초기화 실패: {error}"))?;

        let mut credential_notice = initialization_notice;
        let api_key = if credential_notice.is_none() {
            match api_key_store.load() {
                Ok(Some(api_key)) if valid_key(api_key.as_str()) => Some(api_key),
                Ok(Some(_)) => {
                    credential_notice = Some(if api_key_store.delete().is_ok() {
                        "저장된 API 키 형식이 올바르지 않아 보안 저장소에서 제거했습니다.".into()
                    } else {
                        "저장된 API 키 형식이 올바르지 않으며 보안 저장소에서도 제거하지 못했습니다. 보안 저장소를 잠금 해제한 뒤 다시 연결 해제해 주세요."
                                .into()
                    });
                    None
                }
                Ok(None) => None,
                Err(error) => {
                    credential_notice = Some(error);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            client,
            api_key: Mutex::new(api_key),
            api_key_store,
            key_update: Mutex::new(()),
            credential_notice: Mutex::new(credential_notice),
        })
    }

    pub fn has_key(&self) -> bool {
        let Ok(_update) = self.key_update.lock() else {
            return false;
        };
        self.api_key
            .lock()
            .map(|key| key.is_some())
            .unwrap_or(false)
    }

    pub fn credential_notice(&self) -> Option<String> {
        self.credential_notice
            .lock()
            .ok()
            .and_then(|notice| notice.clone())
    }

    pub async fn connect(self: &Arc<Self>, key: String) -> Result<ConnectionInfo, String> {
        let key = normalize_key(key)?;
        let connection = self.verify_key(key.as_str()).await?;
        self.store_key_async(key).await?;
        Ok(connection)
    }

    pub(super) async fn store_key_async(
        self: &Arc<Self>,
        key: Zeroizing<String>,
    ) -> Result<(), String> {
        // Linux Secret Service exposes a synchronous API that internally calls
        // `block_on`. Running it on a Tokio worker would nest runtimes and panic.
        let runtime = Arc::clone(self);
        match tokio::task::spawn_blocking(move || runtime.store_key(key)).await {
            Ok(result) => result,
            Err(_) => {
                let error = CREDENTIAL_TASK_FAILED.to_string();
                self.set_credential_notice(Some(error.clone()));
                Err(error)
            }
        }
    }

    pub(super) fn store_key(&self, key: Zeroizing<String>) -> Result<(), String> {
        let _update = self
            .key_update
            .lock()
            .map_err(|_| "API 키 저장 작업을 잠글 수 없습니다.".to_string())?;
        let mut stored = self
            .api_key
            .lock()
            .map_err(|_| "API 키 저장소를 잠글 수 없습니다.".to_string())?;
        if let Err(error) = self.api_key_store.save(key.as_str()) {
            self.set_credential_notice(Some(error.clone()));
            return Err(error);
        }
        *stored = Some(key);
        self.set_credential_notice(None);
        Ok(())
    }

    pub async fn clear_key(self: &Arc<Self>) -> Result<(), String> {
        // Deletion can use the same blocking Secret Service facade as saving.
        let runtime = Arc::clone(self);
        match tokio::task::spawn_blocking(move || runtime.clear_key_blocking()).await {
            Ok(result) => result,
            Err(_) => {
                let error = CREDENTIAL_TASK_FAILED.to_string();
                self.set_credential_notice(Some(error.clone()));
                Err(error)
            }
        }
    }

    pub(super) fn clear_key_blocking(&self) -> Result<(), String> {
        let _update = self
            .key_update
            .lock()
            .map_err(|_| "API 키 삭제 작업을 잠글 수 없습니다.".to_string())?;
        let mut stored = self
            .api_key
            .lock()
            .map_err(|_| "API 키 저장소를 잠글 수 없습니다.".to_string())?;
        *stored = None;
        drop(stored);
        match self.api_key_store.delete() {
            Ok(()) => {
                self.set_credential_notice(None);
                Ok(())
            }
            Err(error) => {
                self.set_credential_notice(Some(error.clone()));
                Err(error)
            }
        }
    }

    pub fn forget_key(&self) {
        let Ok(_update) = self.key_update.lock() else {
            return;
        };
        if let Ok(mut stored) = self.api_key.lock() {
            *stored = None;
        }
    }

    pub(super) fn key(&self) -> Result<Zeroizing<String>, String> {
        let _update = self
            .key_update
            .lock()
            .map_err(|_| "API 키 저장 작업을 잠글 수 없습니다.".to_string())?;
        self.api_key
            .lock()
            .map_err(|_| "API 키 저장소를 잠글 수 없습니다.".to_string())?
            .clone()
            .ok_or_else(|| "Sakana API 키를 먼저 연결해 주세요.".to_string())
    }

    async fn verify_key(&self, key: &str) -> Result<ConnectionInfo, String> {
        let response = self
            .client
            .get(format!("{API_ROOT}/models"))
            .bearer_auth(key)
            .timeout(KEY_VERIFICATION_TIMEOUT)
            .send()
            .await
            .map_err(key_verification_network_error)?;
        if !response.status().is_success() {
            return Err(http_error(response).await);
        }
        if response.content_length().unwrap_or(0) > MAX_RESPONSE_BYTES as u64 {
            return Err("모델 목록 응답이 안전한 크기 제한을 초과했습니다.".into());
        }
        let bytes = response
            .bytes()
            .await
            .map_err(key_verification_network_error)?;
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err("모델 목록 응답이 안전한 크기 제한을 초과했습니다.".into());
        }
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|_| "모델 목록 응답을 읽지 못했습니다.".to_string())?;
        let mut models = value
            .get("data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|model| model.get("id").and_then(Value::as_str))
            .filter(|id| id.starts_with("fugu"))
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        models.sort();
        models.dedup();
        if models.is_empty() {
            return Err("이 API 키에서 사용할 수 있는 Fugu 모델을 찾지 못했습니다.".into());
        }
        Ok(ConnectionInfo {
            message: "Sakana API에 안전하게 연결되었습니다.".into(),
            models,
        })
    }

    fn set_credential_notice(&self, value: Option<String>) {
        if let Ok(mut notice) = self.credential_notice.lock() {
            *notice = value;
        }
    }
}
