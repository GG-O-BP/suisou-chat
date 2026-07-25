use keyring_core::{Entry, Error};
#[cfg(any(target_os = "linux", target_os = "windows"))]
use std::collections::HashMap;
use std::sync::Mutex;
use zeroize::Zeroizing;

const SERVICE_NAME: &str = "com.ggobp.suisou-chat";
const API_KEY_ACCOUNT: &str = "sakana-api-key";
const STORE_UNAVAILABLE: &str =
    "운영체제 보안 저장소를 사용할 수 없습니다. 보안 저장소를 잠금 해제한 뒤 앱을 다시 시작해 주세요.";
const STORE_LOCK_FAILED: &str = "API 키 보안 저장소를 잠글 수 없습니다.";
const LOAD_FAILED: &str =
    "저장된 API 키를 운영체제 보안 저장소에서 읽지 못했습니다. 보안 저장소를 잠금 해제한 뒤 앱을 다시 시작해 주세요.";
const SAVE_FAILED: &str =
    "API 키를 운영체제 보안 저장소에 저장하지 못했습니다. 보안 저장소를 잠금 해제한 뒤 다시 시도해 주세요.";
const DELETE_FAILED: &str =
    "API 키를 현재 세션에서는 제거했지만 운영체제 보안 저장소에서 삭제하지 못했습니다. 보안 저장소를 잠금 해제한 뒤 다시 연결 해제해 주세요.";
const STORE_ACCESS_FAILED: &str =
    "운영체제 보안 저장소가 잠겨 있거나 잠금 해제 창이 취소되었습니다. 다른 창 뒤의 잠금 해제 창을 완료한 뒤 다시 시도해 주세요.";
const STORE_NOT_INITIALIZED: &str =
    "운영체제의 기본 로그인 키링이 초기화되지 않았습니다. 로그아웃 후 비밀번호로 다시 로그인하거나 ‘비밀번호 및 키’에서 Login 키링을 만들어 주세요.";
const STORE_AMBIGUOUS: &str =
    "운영체제 보안 저장소에 중복된 API 키 항목이 있어 사용할 수 없습니다. 비밀번호 및 키 앱에서 Suisou 항목을 정리한 뒤 다시 시도해 주세요.";

pub trait ApiKeyStore: Send + Sync {
    fn load(&self) -> Result<Option<Zeroizing<String>>, String>;
    fn save(&self, api_key: &str) -> Result<(), String>;
    fn delete(&self) -> Result<(), String>;
}

pub struct SystemApiKeyStore {
    entry: Mutex<Entry>,
}

impl SystemApiKeyStore {
    pub fn new() -> Result<Self, String> {
        let store = native_store().map_err(|_| STORE_UNAVAILABLE.to_string())?;
        let entry = native_entry(store.as_ref()).map_err(|_| STORE_UNAVAILABLE.to_string())?;
        Ok(Self {
            entry: Mutex::new(entry),
        })
    }
}

impl ApiKeyStore for SystemApiKeyStore {
    fn load(&self) -> Result<Option<Zeroizing<String>>, String> {
        let entry = self
            .entry
            .lock()
            .map_err(|_| STORE_LOCK_FAILED.to_string())?;
        match entry.get_password() {
            Ok(api_key) => Ok(Some(Zeroizing::new(api_key))),
            Err(Error::NoEntry) => Ok(None),
            Err(error) => Err(operation_error(error, LOAD_FAILED)),
        }
    }

    fn save(&self, api_key: &str) -> Result<(), String> {
        let entry = self
            .entry
            .lock()
            .map_err(|_| STORE_LOCK_FAILED.to_string())?;
        entry
            .set_password(api_key)
            .map_err(|error| operation_error(error, SAVE_FAILED))
    }

    fn delete(&self) -> Result<(), String> {
        let entry = self
            .entry
            .lock()
            .map_err(|_| STORE_LOCK_FAILED.to_string())?;
        match entry.delete_credential() {
            Ok(()) | Err(Error::NoEntry) => Ok(()),
            Err(error) => Err(operation_error(error, DELETE_FAILED)),
        }
    }
}

pub struct UnavailableApiKeyStore {
    message: String,
}

impl UnavailableApiKeyStore {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

impl ApiKeyStore for UnavailableApiKeyStore {
    fn load(&self) -> Result<Option<Zeroizing<String>>, String> {
        Err(self.message.clone())
    }

    fn save(&self, _api_key: &str) -> Result<(), String> {
        Err(self.message.clone())
    }

    fn delete(&self) -> Result<(), String> {
        Err(DELETE_FAILED.to_string())
    }
}

#[cfg(target_os = "linux")]
fn native_store() -> keyring_core::Result<std::sync::Arc<keyring_core::CredentialStore>> {
    zbus_secret_service_keyring_store::Store::new()
        .map(|store| store as std::sync::Arc<keyring_core::CredentialStore>)
}

#[cfg(target_os = "linux")]
fn native_entry(store: &keyring_core::CredentialStore) -> keyring_core::Result<Entry> {
    if is_wsl() {
        let modifiers =
            HashMap::from([("target", SERVICE_NAME), ("label", "Suisou Sakana API key")]);
        store.build(SERVICE_NAME, API_KEY_ACCOUNT, Some(&modifiers))
    } else {
        store.build(SERVICE_NAME, API_KEY_ACCOUNT, None)
    }
}

#[cfg(target_os = "linux")]
fn is_wsl() -> bool {
    std::env::var_os("WSL_DISTRO_NAME").is_some()
        || ["/proc/sys/kernel/osrelease", "/proc/version"]
            .into_iter()
            .filter_map(|path| std::fs::read_to_string(path).ok())
            .any(|value| value.to_ascii_lowercase().contains("microsoft"))
}

#[cfg(target_os = "windows")]
fn native_store() -> keyring_core::Result<std::sync::Arc<keyring_core::CredentialStore>> {
    windows_native_keyring_store::Store::new()
        .map(|store| store as std::sync::Arc<keyring_core::CredentialStore>)
}

#[cfg(target_os = "windows")]
fn native_entry(store: &keyring_core::CredentialStore) -> keyring_core::Result<Entry> {
    let modifiers = HashMap::from([("persistence", "Local")]);
    store.build(SERVICE_NAME, API_KEY_ACCOUNT, Some(&modifiers))
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn native_entry(store: &keyring_core::CredentialStore) -> keyring_core::Result<Entry> {
    store.build(SERVICE_NAME, API_KEY_ACCOUNT, None)
}

#[cfg(target_os = "macos")]
fn native_store() -> keyring_core::Result<std::sync::Arc<keyring_core::CredentialStore>> {
    apple_native_keyring_store::keychain::Store::new()
        .map(|store| store as std::sync::Arc<keyring_core::CredentialStore>)
}

#[cfg(target_os = "ios")]
fn native_store() -> keyring_core::Result<std::sync::Arc<keyring_core::CredentialStore>> {
    apple_native_keyring_store::protected::Store::new()
        .map(|store| store as std::sync::Arc<keyring_core::CredentialStore>)
}

#[cfg(target_os = "android")]
fn native_store() -> keyring_core::Result<std::sync::Arc<keyring_core::CredentialStore>> {
    android_native_keyring_store::Store::new()
        .map(|store| store as std::sync::Arc<keyring_core::CredentialStore>)
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "windows",
    target_os = "macos",
    target_os = "ios",
    target_os = "android"
)))]
fn native_store() -> keyring_core::Result<std::sync::Arc<keyring_core::CredentialStore>> {
    Err(Error::NotSupportedByStore(
        "이 플랫폼에는 지원되는 보안 저장소가 없습니다.".into(),
    ))
}

fn operation_error(error: Error, fallback: &str) -> String {
    match error {
        Error::NoStorageAccess(_) => STORE_ACCESS_FAILED.to_string(),
        Error::PlatformFailure(error) if store_is_not_initialized(error.as_ref()) => {
            STORE_NOT_INITIALIZED.to_string()
        }
        Error::PlatformFailure(error) if requires_store_unlock(error.as_ref()) => {
            STORE_ACCESS_FAILED.to_string()
        }
        Error::Ambiguous(_) => STORE_AMBIGUOUS.to_string(),
        _ => fallback.to_string(),
    }
}

fn store_is_not_initialized(mut error: &(dyn std::error::Error + 'static)) -> bool {
    loop {
        let message = error.to_string().to_ascii_lowercase();
        if [
            "object does not exist",
            "unknown method",
            "unknown object",
            "unknownmethod",
            "unknownobject",
        ]
        .into_iter()
        .any(|needle| message.contains(needle))
        {
            return true;
        }
        let Some(source) = error.source() else {
            return false;
        };
        error = source;
    }
}

fn requires_store_unlock(mut error: &(dyn std::error::Error + 'static)) -> bool {
    loop {
        let message = error.to_string().to_ascii_lowercase();
        if [
            "access denied",
            "authorization",
            "object locked",
            "permission denied",
            "prompt dismissed",
        ]
        .into_iter()
        .any(|needle| message.contains(needle))
        {
            return true;
        }
        let Some(source) = error.source() else {
            return false;
        };
        error = source;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_locked_storage_to_an_actionable_message() {
        let error = Error::NoStorageAccess(Box::new(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "locked",
        )));

        assert_eq!(operation_error(error, SAVE_FAILED), STORE_ACCESS_FAILED);
    }

    #[test]
    fn maps_ambiguous_entries_without_exposing_store_details() {
        assert_eq!(
            operation_error(Error::Ambiguous(Vec::new()), LOAD_FAILED),
            STORE_AMBIGUOUS
        );
    }

    #[test]
    fn maps_dismissed_unlock_prompt_to_an_actionable_message() {
        let error = Error::PlatformFailure(Box::new(std::io::Error::other(
            "SS error: prompt dismissed",
        )));

        assert_eq!(operation_error(error, SAVE_FAILED), STORE_ACCESS_FAILED);
    }

    #[test]
    fn maps_a_missing_default_collection_to_initialization_guidance() {
        let error = Error::PlatformFailure(Box::new(std::io::Error::other(
            "UnknownMethod: Object does not exist at path /org/freedesktop/secrets/collection/login",
        )));

        assert_eq!(operation_error(error, SAVE_FAILED), STORE_NOT_INITIALIZED);
    }

    #[test]
    fn maps_other_errors_to_the_operation_fallback() {
        assert_eq!(
            operation_error(Error::NoDefaultStore, SAVE_FAILED),
            SAVE_FAILED
        );
        let error = Error::PlatformFailure(Box::new(std::io::Error::other("unexpected failure")));
        assert_eq!(operation_error(error, SAVE_FAILED), SAVE_FAILED);
    }
}
