use crate::models::{Settings, SyncProvider, UiMode};
use crate::paths::{app_data_dir, settings_path};
use base64::Engine as _;
use rand::RngCore;
use std::fs;

const KEYRING_SERVICE: &str = "PowerPaste";
const KEYRING_ACCOUNT: &str = "sync-passphrase";
const DEFAULT_HOTKEY: &str = "Command+Shift+V";
const DEFAULT_THEME: &str = "system";

pub fn get<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<Settings, String> {
    load_or_init_settings(app)
}

pub fn load_or_init_settings<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<Settings, String> {
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
        if s.theme.trim().is_empty() {
            s.theme = DEFAULT_THEME.to_string();
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
        theme: DEFAULT_THEME.to_string(),
        ui_mode: UiMode::default(),
        show_dock_icon: false,
    };
    save_settings(app, &s)?;
    Ok(s)
}

pub fn set_theme<R: tauri::Runtime>(app: &tauri::AppHandle<R>, mut settings: Settings, theme: String) -> Result<Settings, String> {
    let t = theme.trim();
    if t.is_empty() {
        return Err("theme cannot be empty".to_string());
    }
    // Keep this as a simple string so we can add themes without migrations.
    settings.theme = t.to_string();
    save_settings(app, &settings)?;
    Ok(settings)
}

pub fn set_ui_mode<R: tauri::Runtime>(app: &tauri::AppHandle<R>, mut settings: Settings, ui_mode: UiMode) -> Result<Settings, String> {
    settings.ui_mode = ui_mode;
    save_settings(app, &settings)?;
    Ok(settings)
}

pub fn set_show_dock_icon<R: tauri::Runtime>(app: &tauri::AppHandle<R>, mut settings: Settings, show: bool) -> Result<Settings, String> {
    settings.show_dock_icon = show;
    save_settings(app, &settings)?;
    Ok(settings)
}

pub fn save_settings<R: tauri::Runtime>(app: &tauri::AppHandle<R>, settings: &Settings) -> Result<(), String> {
    let dir = app_data_dir(app)?;
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create app data dir: {e}"))?;
    let path = settings_path(app)?;
    let raw = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("failed to serialize settings: {e}"))?;
    fs::write(path, raw).map_err(|e| format!("failed to write settings: {e}"))
}

pub fn set_sync_config<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
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

pub fn set_hotkey<R: tauri::Runtime>(app: &tauri::AppHandle<R>, mut settings: Settings, hotkey: String) -> Result<Settings, String> {
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

pub fn ensure_sync_salt_b64<R: tauri::Runtime>(app: &tauri::AppHandle<R>, mut settings: Settings) -> Result<Settings, String> {
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
