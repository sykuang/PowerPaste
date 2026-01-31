use std::path::PathBuf;
use tauri::Manager;

pub fn app_data_dir<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))
}

pub fn settings_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join("settings.json"))
}

/// Returns the database path.
/// 
/// If `POWERPASTE_TEST_DB_PATH` environment variable is set, uses that path
/// for test isolation. Otherwise uses the default app data directory path.
pub fn db_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf, String> {
    // Check for test database path override (for E2E test isolation)
    if let Ok(test_path) = std::env::var("POWERPASTE_TEST_DB_PATH") {
        if !test_path.is_empty() {
            eprintln!("[powerpaste] using test database: {test_path}");
            return Ok(PathBuf::from(test_path));
        }
    }
    Ok(app_data_dir(app)?.join("powerpaste.sqlite3"))
}

pub fn sync_file_path<R: tauri::Runtime>(_app: &tauri::AppHandle<R>, folder: &str) -> Result<PathBuf, String> {
    let folder = PathBuf::from(folder);
    if folder.as_os_str().is_empty() {
        return Err("sync folder is empty".to_string());
    }
    Ok(folder.join("powerpaste.sync.json"))
}
