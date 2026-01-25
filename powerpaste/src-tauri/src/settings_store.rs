use crate::models::{Settings, SyncProvider};
use crate::paths::{app_data_dir, settings_path};
use base64::Engine as _;
use rand::RngCore;
use std::fs;

const KEYRING_SERVICE: &str = "PowerPaste";
const KEYRING_ACCOUNT: &str = "sync-passphrase";
const DEFAULT_HOTKEY: &str = "Command+Shift+V";

pub fn load_or_init_settings(app: &tauri::AppHandle) -> Result<Settings, String> {
    let settings_path = settings_path(app)?;
    if let Ok(raw) = fs::read_to_string(&settings_path) {
        let mut s: Settings = serde_json::from_str(&raw)
            .map_err(|e| format!("failed to parse settings.json: {e}"))?;
        if s.device_id.is_empty() {
            s.device_id = new_device_id();
        }
        if s.hotkey.trim().is_empty() {
            s.hotkey = DEFAULT_HOTKEY.to_string();
        }
        save_settings(app, &s)?;
        return Ok(s);
    }

    let s = Settings {
        device_id: new_device_id(),
        sync_enabled: false,
        sync_provider: None,
        sync_folder: None,
        sync_salt_b64: None,
        hotkey: DEFAULT_HOTKEY.to_string(),
    };
    save_settings(app, &s)?;
    Ok(s)
}

pub fn save_settings(app: &tauri::AppHandle, settings: &Settings) -> Result<(), String> {
    let dir = app_data_dir(app)?;
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create app data dir: {e}"))?;
    let path = settings_path(app)?;
    let raw = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("failed to serialize settings: {e}"))?;
    fs::write(path, raw).map_err(|e| format!("failed to write settings: {e}"))
}

pub fn set_sync_config(
    app: &tauri::AppHandle,
    mut settings: Settings,
    enabled: bool,
    provider: Option<SyncProvider>,
    folder: Option<String>,
) -> Result<Settings, String> {
    settings.sync_enabled = enabled;
    settings.sync_provider = provider;
    settings.sync_folder = folder;
    save_settings(app, &settings)?;
    Ok(settings)
}

pub fn set_hotkey(app: &tauri::AppHandle, mut settings: Settings, hotkey: String) -> Result<Settings, String> {
    let hk = hotkey.trim();
    if hk.is_empty() {
        return Err("hotkey cannot be empty".to_string());
    }
    settings.hotkey = hk.to_string();
    save_settings(app, &settings)?;
    Ok(settings)
}

pub fn load_sync_passphrase() -> Result<Option<String>, String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
        .map_err(|e| format!("keyring init failed: {e}"))?;
    match entry.get_password() {
        Ok(pw) => Ok(Some(pw)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("keyring read failed: {e}")),
    }
}

pub fn save_sync_passphrase(passphrase: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
        .map_err(|e| format!("keyring init failed: {e}"))?;
    entry
        .set_password(passphrase)
        .map_err(|e| format!("keyring write failed: {e}"))
}

pub fn clear_sync_passphrase() -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
        .map_err(|e| format!("keyring init failed: {e}"))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("keyring delete failed: {e}")),
    }
}

pub fn ensure_sync_salt_b64(app: &tauri::AppHandle, mut settings: Settings) -> Result<Settings, String> {
    if settings.sync_salt_b64.is_some() {
        return Ok(settings);
    }
    let mut salt = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut salt);
    settings.sync_salt_b64 = Some(base64::engine::general_purpose::STANDARD.encode(salt));
    save_settings(app, &settings)?;
    Ok(settings)
}

fn new_device_id() -> String {
    // Random, stable per-install identifier.
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}
