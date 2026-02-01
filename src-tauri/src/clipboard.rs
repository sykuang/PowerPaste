use crate::db;
use crate::macos_query_frontmost_app_info;
use arboard::Clipboard;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// Maximum image size in bytes to store (5MB)
const MAX_IMAGE_SIZE_BYTES: u64 = 5 * 1024 * 1024;

/// Detect content type from text
fn detect_content_type(text: &str) -> Option<String> {
    let trimmed = text.trim();
    
    // Check for URL patterns
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        // Single URL on one line
        if !trimmed.contains('\n') && !trimmed.contains(' ') {
            return Some("url".to_string());
        }
    }
    
    // Check for file paths (macOS/Unix and Windows)
    if trimmed.starts_with('/') || trimmed.starts_with("file://") {
        if std::path::Path::new(trimmed).exists() {
            return Some("file".to_string());
        }
    }
    
    // Windows paths
    if trimmed.len() >= 3 {
        let chars: Vec<char> = trimmed.chars().take(3).collect();
        if chars.len() >= 3 && chars[0].is_ascii_alphabetic() && chars[1] == ':' && (chars[2] == '\\' || chars[2] == '/') {
            return Some("file".to_string());
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
            let mut last_text: Option<String> = None;
            let mut last_image_hash: Option<u64> = None;

            loop {
                if *stop_flag_thread.lock().unwrap_or_else(|e| e.into_inner()) {
                    break;
                }

                // Try to get image from clipboard first
                if let Some(ref mut cb) = clipboard.as_mut().ok() {
                    if let Ok(image) = cb.get_image() {
                        let image_bytes = image.bytes.as_ref();
                        let hash = image_hash(image_bytes);
                        
                        // Skip if same as last image
                        if last_image_hash != Some(hash) {
                            let size_bytes = image_bytes.len() as u64;
                            
                            // Only store if within size limit
                            if size_bytes <= MAX_IMAGE_SIZE_BYTES {
                                // Query source app info
                                let (source_app_name, source_app_bundle_id) = macos_query_frontmost_app_info();
                                
                                match db::insert_image_with_source_app(
                                    &app,
                                    image_bytes,
                                    image.width as u32,
                                    image.height as u32,
                                    source_app_name,
                                    source_app_bundle_id,
                                ) {
                                    Ok(Some(item)) => {
                                        last_image_hash = Some(hash);
                                        last_text = None; // Reset text tracking
                                        let _ = app.emit("powerpaste://new_item", item);
                                    }
                                    Ok(None) => {
                                        last_image_hash = Some(hash);
                                    }
                                    Err(e) => {
                                        eprintln!("[powerpaste] failed to insert image: {e}");
                                    }
                                }
                            } else {
                                // Image too large, skip but update hash to avoid retrying
                                last_image_hash = Some(hash);
                                eprintln!("[powerpaste] skipped large image: {} bytes", size_bytes);
                            }
                            
                            std::thread::sleep(Duration::from_millis(500));
                            continue;
                        }
                    }
                }

                // Fall back to text
                let text = match clipboard.as_mut().ok().and_then(|c| c.get_text().ok()) {
                    Some(t) => t,
                    None => {
                        std::thread::sleep(Duration::from_millis(500));
                        continue;
                    }
                };

                if last_text.as_deref() == Some(&text) {
                    std::thread::sleep(Duration::from_millis(500));
                    continue;
                }

                // Detect content type for the text
                let content_type = detect_content_type(&text);
                
                // Query source app info
                let (source_app_name, source_app_bundle_id) = macos_query_frontmost_app_info();

                match db::insert_text_with_source_app(&app, &text, content_type, source_app_name, source_app_bundle_id) {
                    Ok(Some(item)) => {
                        last_text = Some(text);
                        last_image_hash = None; // Reset image tracking
                        let _ = app.emit("powerpaste://new_item", item);
                    }
                    Ok(None) => {
                        last_text = Some(text);
                    }
                    Err(e) => {
                        eprintln!("[powerpaste] failed to insert text: {e}");
                    }
                }

                std::thread::sleep(Duration::from_millis(500));
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
