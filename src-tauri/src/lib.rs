mod clipboard;
mod db;
mod models;
mod paths;
mod settings_store;
mod sync;

use models::{ClipboardItem, Settings, SyncProvider};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::Manager;
use uuid::Uuid;

struct AppState {
    watcher: Mutex<Option<clipboard::ClipboardWatcher>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SyncNowResult {
    imported: u32,
}

#[tauri::command]
fn get_settings(app: tauri::AppHandle) -> Result<Settings, String> {
    settings_store::load_or_init_settings(&app)
}

#[tauri::command]
fn set_sync_settings(
    app: tauri::AppHandle,
    enabled: bool,
    provider: Option<SyncProvider>,
    folder: Option<String>,
    passphrase: Option<String>,
) -> Result<Settings, String> {
    let settings = settings_store::load_or_init_settings(&app)?;

    if let Some(pw) = passphrase {
        if !pw.trim().is_empty() {
            settings_store::save_sync_passphrase(pw.trim())?;
        }
    }

    if !enabled {
        let _ = settings_store::clear_sync_passphrase();
    }

    let settings = settings_store::set_sync_config(&app, settings, enabled, provider, folder)?;
    if settings.sync_enabled {
        let settings = settings_store::ensure_sync_salt_b64(&app, settings)?;
        return Ok(settings);
    }
    Ok(settings)
}

#[tauri::command]
fn list_items(app: tauri::AppHandle, limit: u32, query: Option<String>) -> Result<Vec<ClipboardItem>, String> {
    db::list_items(&app, limit, query)
}

#[tauri::command]
fn set_item_pinned(app: tauri::AppHandle, id: String, pinned: bool) -> Result<(), String> {
    let id = Uuid::parse_str(&id).map_err(|_| "invalid id".to_string())?;
    db::set_pinned(&app, id, pinned)
}

#[tauri::command]
fn delete_item(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let id = Uuid::parse_str(&id).map_err(|_| "invalid id".to_string())?;
    db::delete_item(&app, id)
}

#[tauri::command]
fn write_clipboard_text(text: String) -> Result<(), String> {
    clipboard::set_clipboard_text(&text)
}

#[tauri::command]
fn sync_now(app: tauri::AppHandle) -> Result<SyncNowResult, String> {
    let imported = sync::import_now(&app)?;
    sync::export_now(&app)?;
    Ok(SyncNowResult { imported })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            watcher: Mutex::new(None),
        })
        .setup(|app| {
            let handle = app.handle().clone();

            let _ = settings_store::load_or_init_settings(&handle);

            let watcher = clipboard::ClipboardWatcher::start(handle.clone());
            let state: tauri::State<'_, AppState> = app.state();
            let mut guard = state.watcher.lock().unwrap_or_else(|e| e.into_inner());
            *guard = Some(watcher);

            std::thread::spawn(move || loop {
                let _ = sync::import_now(&handle);
                let _ = sync::export_now(&handle);
                std::thread::sleep(std::time::Duration::from_secs(15));
            });

            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_settings,
            set_sync_settings,
            list_items,
            set_item_pinned,
            delete_item,
            write_clipboard_text,
            sync_now
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
