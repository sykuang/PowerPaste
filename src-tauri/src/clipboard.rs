use crate::db;
use arboard::Clipboard;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// Maximum image size in bytes to store (5MB)
const MAX_IMAGE_SIZE_BYTES: u64 = 5 * 1024 * 1024;

/// Detect text previews without interpreting path text as a copied file.
fn detect_content_type(text: &str) -> Option<String> {
    let trimmed = text.trim();
    
    // Check for URL patterns
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        // Single URL on one line
        if !trimmed.contains('\n') && !trimmed.contains(' ') {
            return Some("url".to_string());
        }
    }
    
    None
}

/// Calculate hash for image deduplication
fn image_hash(data: &[u8]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}

/// Calculate hash for text deduplication, keeping file copies distinct from path text.
fn text_hash(text: &str, content_type: Option<&str>) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    (text.trim(), content_type).hash(&mut hasher);
    hasher.finish()
}


#[derive(Clone)]
pub struct ClipboardWatcher {
    stop_flag: Arc<Mutex<bool>>,
}

impl ClipboardWatcher {
    pub fn start(app: AppHandle) -> Self {
        let stop_flag = Arc::new(Mutex::new(false));
        let stop_flag_thread = stop_flag.clone();

        std::thread::spawn(move || {
            let mut clipboard = Clipboard::new();
            let mut last_text_hash: Option<u64> = None;
            let mut last_image_hash: Option<u64> = None;
            let mut last_change_count: i64 = crate::platform::get_clipboard_change_count();
            let mut sleep_ms: u64 = 250;
            // Track if we already processed this clipboard change (for multi-format handling)
            let mut processed_this_change: bool = false;
            // Cache source app info when clipboard changes, to avoid querying after user switches apps
            let mut cached_source_app: Option<(Option<String>, Option<String>)> = None;

            eprintln!("[powerpaste] clipboard watcher started, initial change_count={}", last_change_count);

            loop {
                if *stop_flag_thread.lock().unwrap_or_else(|e| e.into_inner()) {
                    eprintln!("[powerpaste] clipboard watcher stopping");
                    break;
                }

                // Check if clipboard has changed using platform change count
                let current_change_count = crate::platform::get_clipboard_change_count();
                if current_change_count != 0 {
                    if current_change_count == last_change_count {
                        // Clipboard hasn't changed, skip this iteration
                        std::thread::sleep(Duration::from_millis(300));
                        continue;
                    }
                    // New clipboard change detected - query frontmost app IMMEDIATELY
                    // before user has a chance to switch apps
                    eprintln!("[powerpaste] clipboard changed: {} -> {}", last_change_count, current_change_count);
                    cached_source_app = Some(crate::platform::query_frontmost_app_info());
                    last_change_count = current_change_count;
                    processed_this_change = false;
                    sleep_ms = 250;
                }

                // If we already processed this clipboard change, skip
                if processed_this_change {
                    std::thread::sleep(Duration::from_millis(300));
                    continue;
                }

                let mut did_change = false;
                let mut handled = false;

                // Native file copies take precedence over accompanying image or text formats.
                if let Some(file_paths) = crate::platform::get_clipboard_file_urls() {
                    handled = true;
                    let text = file_paths.join("\n");
                    let current_hash = text_hash(&text, Some("file"));

                    if last_text_hash != Some(current_hash) {
                        let (source_app_name, source_app_bundle_id) = cached_source_app
                            .clone()
                            .unwrap_or_else(|| (None, None));

                        match db::insert_text_with_source_app(
                            &app,
                            &text,
                            Some("file".to_string()),
                            source_app_name,
                            source_app_bundle_id,
                        ) {
                            Ok(Some(item)) => {
                                last_text_hash = Some(current_hash);
                                processed_this_change = true;
                                did_change = true;
                                let _ = app.emit("powerpaste://new_item", item);
                            }
                            Ok(None) => {
                                last_text_hash = Some(current_hash);
                                processed_this_change = true;
                                did_change = true;
                            }
                            Err(e) => {
                                eprintln!("[powerpaste] failed to insert file paths: {e}");
                            }
                        }
                    }
                    sleep_ms = 250;
                }

                if !handled {
                    if let Some(ref mut cb) = clipboard.as_mut().ok() {
                        if let Ok(image) = cb.get_image() {
                            let image_bytes = image.bytes.as_ref();
                            let hash = image_hash(image_bytes);

                            // Skip if same as last image (fallback for non-macOS or same content)
                            if last_image_hash != Some(hash) {
                                let size_bytes = image_bytes.len() as u64;

                                // Only store if within size limit
                                if size_bytes <= MAX_IMAGE_SIZE_BYTES {
                                    // Use cached source app info from when clipboard changed
                                    let (source_app_name, source_app_bundle_id) = cached_source_app
                                        .clone()
                                        .unwrap_or_else(|| (None, None));
                                    eprintln!("[powerpaste] inserting image from {:?}", source_app_name);
                                    let encoded_image = crate::platform::get_clipboard_image_encoded();

                                    match db::insert_image_with_source_app(
                                        &app,
                                        image_bytes,
                                        image.width as u32,
                                        image.height as u32,
                                        source_app_name,
                                        source_app_bundle_id,
                                        encoded_image,
                                    ) {
                                        Ok(Some(item)) => {
                                            last_image_hash = Some(hash);
                                            processed_this_change = true;
                                            did_change = true;
                                            handled = true;
                                            let _ = app.emit("powerpaste://new_item", item);
                                        }
                                        Ok(None) => {
                                            last_image_hash = Some(hash);
                                            processed_this_change = true;
                                            did_change = true;
                                            handled = true;
                                        }
                                        Err(e) => {
                                            eprintln!("[powerpaste] failed to insert image: {e}");
                                        }
                                    }
                                } else {
                                    // Image too large, skip but update hash to avoid retrying
                                    last_image_hash = Some(hash);
                                    processed_this_change = true;
                                    did_change = true;
                                    handled = true;
                                    eprintln!("[powerpaste] skipped large image: {} bytes", size_bytes);
                                }
                            }
                            if handled {
                                sleep_ms = 250;
                            }
                        }
                    }
                }

                // Fall back to text
                if !handled {
                    let text = match clipboard.as_mut().ok().and_then(|c| c.get_text().ok()) {
                        Some(t) => t,
                        None => {
                            sleep_ms = (sleep_ms + 100).min(1000);
                            std::thread::sleep(Duration::from_millis(sleep_ms));
                            continue;
                        }
                    };

                    // Use hash comparison to avoid issues with minor formatting differences
                    let content_type = detect_content_type(&text);
                    let current_hash = text_hash(&text, content_type.as_deref());
                    if last_text_hash == Some(current_hash) {
                        sleep_ms = (sleep_ms + 100).min(1000);
                        std::thread::sleep(Duration::from_millis(sleep_ms));
                        continue;
                    }

                    // Use cached source app info from when clipboard changed
                    let (source_app_name, source_app_bundle_id) = cached_source_app
                        .clone()
                        .unwrap_or_else(|| (None, None));

                    match db::insert_text_with_source_app(&app, &text, content_type, source_app_name, source_app_bundle_id) {
                        Ok(Some(item)) => {
                            last_text_hash = Some(current_hash);
                            processed_this_change = true;
                            did_change = true;
                            let _ = app.emit("powerpaste://new_item", item);
                        }
                        Ok(None) => {
                            last_text_hash = Some(current_hash);
                            processed_this_change = true;
                            did_change = true;
                        }
                        Err(e) => {
                            eprintln!("[powerpaste] failed to insert text: {e}");
                        }
                    }
                }

                if did_change {
                    sleep_ms = 250;
                } else {
                    sleep_ms = (sleep_ms + 100).min(1000);
                }
                std::thread::sleep(Duration::from_millis(sleep_ms));
            }
        });

        Self { stop_flag }
    }

    pub fn stop(&self) {
        if let Ok(mut guard) = self.stop_flag.lock() {
            let _was = *guard;
            *guard = true;
        }
    }
}

pub fn set_clipboard_text(text: &str) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| format!("clipboard init failed: {e}"))?;
    clipboard
        .set_text(text.to_string())
        .map_err(|e| format!("clipboard set failed: {e}"))
}

/// Write encoded image bytes to the clipboard, preserving format when possible.
pub fn set_clipboard_image_encoded(bytes: &[u8], mime: Option<&str>) -> Result<(), String> {
    crate::platform::set_clipboard_image_encoded(bytes, mime)
}

/// Write file paths to the clipboard
pub fn set_clipboard_files(paths: &[String]) -> Result<(), String> {
    crate::platform::set_clipboard_files(paths)
}

#[cfg(test)]
mod tests {
    use super::{detect_content_type, text_hash};

    #[test]
    fn copied_path_text_is_not_a_file() {
        let image_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("icons/icon.png")
            .canonicalize()
            .unwrap();
        let image_path = image_path.to_str().unwrap();

        for text in [
            image_path.to_string(),
            format!("  {image_path}\n"),
            format!("file://{image_path}"),
            r"C:\Photos\photo.jpg".to_string(),
            "C:/Photos/photo.jpg".to_string(),
            "ordinary text".to_string(),
        ] {
            assert_eq!(detect_content_type(&text), None, "{text}");
            assert_ne!(text_hash(&text, None), text_hash(&text, Some("file")));
        }

        assert_eq!(
            text_hash(image_path, None),
            text_hash(&format!("  {image_path}\n"), None)
        );
        assert_eq!(
            detect_content_type("https://example.com/photo.jpg"),
            Some("url".to_string())
        );
        assert_eq!(detect_content_type("https://example.com/ some text"), None);
    }
}
