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
        configure_native_store().map_err(|_| STORE_UNAVAILABLE.to_string())?;
        let entry = native_entry().map_err(|_| STORE_UNAVAILABLE.to_string())?;
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
            Err(_) => Err(LOAD_FAILED.to_string()),
        }
    }

    fn save(&self, api_key: &str) -> Result<(), String> {
        let entry = self
            .entry
            .lock()
            .map_err(|_| STORE_LOCK_FAILED.to_string())?;
        entry
            .set_password(api_key)
            .map_err(|_| SAVE_FAILED.to_string())
    }

    fn delete(&self) -> Result<(), String> {
        let entry = self
            .entry
            .lock()
            .map_err(|_| STORE_LOCK_FAILED.to_string())?;
        match entry.delete_credential() {
            Ok(()) | Err(Error::NoEntry) => Ok(()),
            Err(_) => Err(DELETE_FAILED.to_string()),
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
fn configure_native_store() -> keyring_core::Result<()> {
    let store = zbus_secret_service_keyring_store::Store::new()?;
    keyring_core::set_default_store(store);
    Ok(())
}

#[cfg(target_os = "linux")]
fn native_entry() -> keyring_core::Result<Entry> {
    if is_wsl() {
        let modifiers =
            HashMap::from([("target", SERVICE_NAME), ("label", "Suisou Sakana API key")]);
        Entry::new_with_modifiers(SERVICE_NAME, API_KEY_ACCOUNT, &modifiers)
    } else {
        Entry::new(SERVICE_NAME, API_KEY_ACCOUNT)
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
fn configure_native_store() -> keyring_core::Result<()> {
    let store = windows_native_keyring_store::Store::new()?;
    keyring_core::set_default_store(store);
    Ok(())
}

#[cfg(target_os = "windows")]
fn native_entry() -> keyring_core::Result<Entry> {
    let modifiers = HashMap::from([("persistence", "Local")]);
    Entry::new_with_modifiers(SERVICE_NAME, API_KEY_ACCOUNT, &modifiers)
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn native_entry() -> keyring_core::Result<Entry> {
    Entry::new(SERVICE_NAME, API_KEY_ACCOUNT)
}

#[cfg(target_os = "macos")]
fn configure_native_store() -> keyring_core::Result<()> {
    let store = apple_native_keyring_store::keychain::Store::new()?;
    keyring_core::set_default_store(store);
    Ok(())
}

#[cfg(target_os = "ios")]
fn configure_native_store() -> keyring_core::Result<()> {
    let store = apple_native_keyring_store::protected::Store::new()?;
    keyring_core::set_default_store(store);
    Ok(())
}

#[cfg(target_os = "android")]
fn configure_native_store() -> keyring_core::Result<()> {
    let store = android_native_keyring_store::Store::new()?;
    keyring_core::set_default_store(store);
    Ok(())
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "windows",
    target_os = "macos",
    target_os = "ios",
    target_os = "android"
)))]
fn configure_native_store() -> keyring_core::Result<()> {
    Err(Error::NotSupportedByStore(
        "이 플랫폼에는 지원되는 보안 저장소가 없습니다.".into(),
    ))
}
