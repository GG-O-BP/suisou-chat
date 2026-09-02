use super::*;

impl FuguRuntime {
    pub fn new() -> Result<Self, String> {
        let mut stores = Vec::new();
        for provider in Provider::ALL {
            let (store, notice): (Arc<dyn ApiKeyStore>, Option<String>) =
                match SystemApiKeyStore::new(provider) {
                    Ok(store) => (Arc::new(store), None),
                    Err(error) => (
                        Arc::new(UnavailableApiKeyStore::new(error.clone())),
                        Some(error),
                    ),
                };
            stores.push((provider, store, notice));
        }
        let (sakana_provider, sakana_store, sakana_notice) = stores.remove(0);
        let (zai_provider, zai_store, zai_notice) =
            stores.pop().expect("Z.ai secure store is initialized");
        debug_assert_eq!(zai_provider, Provider::Zai);
        debug_assert_eq!(sakana_provider, Provider::Sakana);
        Self::new_with_stores(sakana_store, zai_store, sakana_notice, zai_notice)
    }

    pub(super) fn new_with_stores(
        sakana_store: Arc<dyn ApiKeyStore>,
        zai_store: Arc<dyn ApiKeyStore>,
        sakana_initialization_notice: Option<String>,
        zai_initialization_notice: Option<String>,
    ) -> Result<Self, String> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .timeout(Duration::from_secs(2 * 60 * 60))
            .user_agent("Suisou/0.1")
            .build()
            .map_err(|error| format!("HTTP 클라이언트 초기화 실패: {error}"))?;

        let mut credentials = HashMap::new();
        credentials.insert(
            Provider::Sakana,
            Self::restore_credential(sakana_store, sakana_initialization_notice, Provider::Sakana),
        );
        credentials.insert(
            Provider::Zai,
            Self::restore_credential(zai_store, zai_initialization_notice, Provider::Zai),
        );

        Ok(Self {
            client,
            credentials,
            key_update: Mutex::new(()),
        })
    }

    /// Kept as a focused test constructor for the original Sakana credential
    /// path. The Z.ai slot shares the in-memory test store but is never used
    /// by these legacy tests.
    #[cfg(test)]
    pub(super) fn new_with_store(
        api_key_store: Arc<dyn ApiKeyStore>,
        initialization_notice: Option<String>,
    ) -> Result<Self, String> {
        Self::new_with_stores(
            Arc::clone(&api_key_store),
            api_key_store,
            initialization_notice,
            None,
        )
    }

    fn restore_credential(
        store: Arc<dyn ApiKeyStore>,
        initialization_notice: Option<String>,
        provider: Provider,
    ) -> ProviderCredential {
        let mut notice = initialization_notice;
        let api_key = if notice.is_none() {
            match store.load() {
                Ok(Some(api_key)) if valid_key(api_key.as_str()) => Some(api_key),
                Ok(Some(_)) => {
                    notice = Some(if store.delete().is_ok() {
                        format!(
                            "저장된 {} 키 형식이 올바르지 않아 보안 저장소에서 제거했습니다.",
                            provider.key_label()
                        )
                    } else {
                        format!(
                            "저장된 {} 키 형식이 올바르지 않으며 보안 저장소에서도 제거하지 못했습니다. 보안 저장소를 잠금 해제한 뒤 다시 연결 해제해 주세요.",
                            provider.key_label()
                        )
                    });
                    None
                }
                Ok(None) => None,
                Err(error) => {
                    notice = Some(error);
                    None
                }
            }
        } else {
            None
        };
        ProviderCredential {
            api_key: Mutex::new(api_key),
            store,
            notice: Mutex::new(notice),
        }
    }

    fn credential(&self, provider: Provider) -> Result<&ProviderCredential, String> {
        self.credentials
            .get(&provider)
            .ok_or_else(|| "API 공급자 상태를 초기화하지 못했습니다.".to_string())
    }

    pub fn has_key(&self, provider: Provider) -> bool {
        let Ok(_update) = self.key_update.lock() else {
            return false;
        };
        self.credential(provider)
            .ok()
            .and_then(|credential| credential.api_key.lock().ok())
            .is_some_and(|key| key.is_some())
    }

    pub fn credential_notice(&self) -> Option<String> {
        let notices = Provider::ALL
            .into_iter()
            .filter_map(|provider| {
                self.credential(provider).ok().and_then(|credential| {
                    credential
                        .notice
                        .lock()
                        .ok()
                        .and_then(|notice| notice.clone())
                })
            })
            .collect::<Vec<_>>();
        let mut unique = Vec::new();
        for notice in notices {
            if !unique.contains(&notice) {
                unique.push(notice);
            }
        }
        if unique.is_empty() {
            None
        } else {
            Some(unique.join(" "))
        }
    }

    pub async fn connect(
        self: &Arc<Self>,
        provider: Provider,
        key: String,
    ) -> Result<ConnectionInfo, String> {
        let key = normalize_key(key)?;
        let connection = self.verify_key(provider, key.as_str()).await?;
        self.store_key_async(provider, key).await?;
        Ok(connection)
    }

    pub(super) async fn store_key_async(
        self: &Arc<Self>,
        provider: Provider,
        key: Zeroizing<String>,
    ) -> Result<(), String> {
        // Linux Secret Service exposes a synchronous API that internally calls
        // `block_on`. Running it on a Tokio worker would nest runtimes and panic.
        let runtime = Arc::clone(self);
        match tokio::task::spawn_blocking(move || runtime.store_key(provider, key)).await {
            Ok(result) => result,
            Err(_) => {
                let error = CREDENTIAL_TASK_FAILED.to_string();
                self.set_credential_notice(provider, Some(error.clone()));
                Err(error)
            }
        }
    }

    pub(super) fn store_key(
        &self,
        provider: Provider,
        key: Zeroizing<String>,
    ) -> Result<(), String> {
        let _update = self
            .key_update
            .lock()
            .map_err(|_| "API 키 저장 작업을 잠글 수 없습니다.".to_string())?;
        let credential = self.credential(provider)?;
        let mut stored = credential
            .api_key
            .lock()
            .map_err(|_| "API 키 저장소를 잠글 수 없습니다.".to_string())?;
        if let Err(error) = credential.store.save(key.as_str()) {
            self.set_credential_notice(provider, Some(error.clone()));
            return Err(error);
        }
        *stored = Some(key);
        self.set_credential_notice(provider, None);
        Ok(())
    }

    pub async fn clear_key(self: &Arc<Self>, provider: Provider) -> Result<(), String> {
        // Deletion can use the same blocking Secret Service facade as saving.
        let runtime = Arc::clone(self);
        match tokio::task::spawn_blocking(move || runtime.clear_key_blocking(provider)).await {
            Ok(result) => result,
            Err(_) => {
                let error = CREDENTIAL_TASK_FAILED.to_string();
                self.set_credential_notice(provider, Some(error.clone()));
                Err(error)
            }
        }
    }

    pub(super) fn clear_key_blocking(&self, provider: Provider) -> Result<(), String> {
        let _update = self
            .key_update
            .lock()
            .map_err(|_| "API 키 삭제 작업을 잠글 수 없습니다.".to_string())?;
        let credential = self.credential(provider)?;
        let mut stored = credential
            .api_key
            .lock()
            .map_err(|_| "API 키 저장소를 잠글 수 없습니다.".to_string())?;
        *stored = None;
        drop(stored);
        match credential.store.delete() {
            Ok(()) => {
                self.set_credential_notice(provider, None);
                Ok(())
            }
            Err(error) => {
                self.set_credential_notice(provider, Some(error.clone()));
                Err(error)
            }
        }
    }

    pub fn forget_key(&self, provider: Provider) {
        let Ok(_update) = self.key_update.lock() else {
            return;
        };
        if let Ok(credential) = self.credential(provider) {
            if let Ok(mut stored) = credential.api_key.lock() {
                *stored = None;
            }
        }
    }

    pub(super) fn key(&self, provider: Provider) -> Result<Zeroizing<String>, String> {
        let _update = self
            .key_update
            .lock()
            .map_err(|_| "API 키 저장 작업을 잠글 수 없습니다.".to_string())?;
        self.credential(provider)?
            .api_key
            .lock()
            .map_err(|_| "API 키 저장소를 잠글 수 없습니다.".to_string())?
            .clone()
            .ok_or_else(|| format!("{} 키를 먼저 연결해 주세요.", provider.key_label()))
    }

    async fn verify_key(&self, provider: Provider, key: &str) -> Result<ConnectionInfo, String> {
        match provider {
            Provider::Sakana => self.verify_sakana_key(key).await,
            Provider::Zai => Ok(ConnectionInfo {
                provider,
                message: format!(
                    "{} 키 형식을 확인했습니다. 첫 요청에서 계정 권한과 모델 접근을 확인합니다.",
                    provider.key_label()
                ),
                models: vec!["glm-5.3".into()],
            }),
        }
    }

    async fn verify_sakana_key(&self, key: &str) -> Result<ConnectionInfo, String> {
        let response = self
            .client
            .get(format!("{API_ROOT}/models"))
            .bearer_auth(key)
            .timeout(KEY_VERIFICATION_TIMEOUT)
            .send()
            .await
            .map_err(key_verification_network_error)?;
        if !response.status().is_success() {
            return Err(http_error(response, Provider::Sakana).await);
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
            provider: Provider::Sakana,
            message: "Sakana API에 안전하게 연결되었습니다.".into(),
            models,
        })
    }

    fn set_credential_notice(&self, provider: Provider, value: Option<String>) {
        if let Ok(credential) = self.credential(provider) {
            if let Ok(mut notice) = credential.notice.lock() {
                *notice = value;
            }
        }
    }
}
