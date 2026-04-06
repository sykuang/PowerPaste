use tauri::Manager;

// --- Last frontmost app tracking (no-op) ---

pub fn set_last_frontmost_app_name(_name: String) {}

#[allow(dead_code)]
pub fn get_last_frontmost_app_name() -> Option<String> {
    None
}

// --- Frontmost app querying ---

pub fn query_frontmost_app_info() -> (Option<String>, Option<String>) {
    (None, None)
}

// --- Cursor position ---

pub fn get_cursor_position() -> Option<(f64, f64)> {
    None
}

// --- Perform paste ---

pub fn perform_paste(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    Ok(())
}

// --- Permissions ---

pub fn check_permissions() -> Result<crate::PermissionsStatus, String> {
    let exe_path = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    Ok(crate::PermissionsStatus {
        platform: "linux".to_string(),
        can_paste: false,
        automation_ok: true,
        accessibility_ok: true,
        details: Some("Paste automation is not implemented on this platform yet.".to_string()),
        is_bundled: true,
        executable_path: exe_path,
    })
}

pub fn open_accessibility_settings() -> Result<(), String> {
    Ok(())
}

pub fn open_automation_settings() -> Result<(), String> {
    Ok(())
}

pub fn request_accessibility_permission() -> Result<bool, String> {
    Ok(true)
}

pub fn request_automation_permission() -> Result<bool, String> {
    Ok(true)
}

// --- App icon ---

pub fn get_app_icon_path(_app: &tauri::AppHandle, _bundle_id: &str) -> Result<Option<String>, String> {
    Ok(None)
}

// --- Window configuration (no-op) ---

pub fn configure_floating_window<R: tauri::Runtime>(_window: &tauri::WebviewWindow<R>) {}

// --- Browser accelerator keys (no-op) ---

pub fn suspend_browser_accelerator_keys(_webview: &tauri::Webview) {}

pub fn resume_browser_accelerator_keys(_webview: &tauri::Webview) {}

// --- Clipboard: change count ---

pub fn get_clipboard_change_count() -> i64 {
    0
}

// --- Clipboard: file URLs ---

pub fn get_clipboard_file_urls() -> Option<Vec<String>> {
    None
}

// --- Clipboard: image ---

pub fn get_clipboard_image_encoded() -> Option<crate::db::EncodedImage> {
    // Use arboard to get image data, then encode as PNG
    let mut clipboard = arboard::Clipboard::new().ok()?;
    let img = clipboard.get_image().ok()?;
    let width = img.width as u32;
    let height = img.height as u32;
    let rgba_bytes = img.bytes.into_owned();

    if rgba_bytes.is_empty() || width == 0 || height == 0 {
        return None;
    }

    let mut png_data = Vec::new();
    {
        let mut encoder = image::codecs::png::PngEncoder::new(&mut png_data);
        use image::ImageEncoder;
        encoder
            .write_image(&rgba_bytes, width, height, image::ExtendedColorType::Rgba8)
            .ok()?;
    }

    Some(crate::db::EncodedImage {
        bytes: png_data,
        mime: "image/png".to_string(),
    })
}

pub fn set_clipboard_image_encoded(bytes: &[u8], _mime: Option<&str>) -> Result<(), String> {
    super::set_clipboard_image_decoded(bytes)
}

// --- Clipboard: set files ---

pub fn set_clipboard_files(_paths: &[String]) -> Result<(), String> {
    Err("file clipboard is not supported on this platform".to_string())
}
