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

/// Calculate hash for text deduplication (normalizes whitespace)
fn text_hash(text: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    // Normalize: trim and collapse whitespace differences
    text.trim().hash(&mut hasher);
    hasher.finish()
}

/// Get file URLs from the clipboard (macOS only)
/// Returns a list of file paths if files are on the clipboard
#[cfg(target_os = "macos")]
fn get_clipboard_file_urls() -> Option<Vec<String>> {
    use std::process::Command;
    
    // Use Swift to read file URLs from NSPasteboard
    let swift_code = r#"
import Cocoa

let pasteboard = NSPasteboard.general

// Check for file URLs
if let urls = pasteboard.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL] {
    if !urls.isEmpty {
        for url in urls {
            print(url.path)
        }
    }
}
"#;
    
    let output = Command::new("swift")
        .args(["-e", swift_code])
        .output()
        .ok()?;
    
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let paths: Vec<String> = stdout
            .lines()
            .filter(|line| !line.is_empty())
            .map(|s| s.to_string())
            .collect();
        
        if !paths.is_empty() {
            return Some(paths);
        }
    }
    
    None
}

#[cfg(not(target_os = "macos"))]
fn get_clipboard_file_urls() -> Option<Vec<String>> {
    None
}

/// Get the clipboard change count (macOS only)
/// This increments every time the clipboard content changes
#[cfg(target_os = "macos")]
fn get_clipboard_change_count() -> i64 {
    use objc2_app_kit::NSPasteboard;
    
    // NSPasteboard.generalPasteboard is thread-safe for reading changeCount
    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.changeCount() as i64
}

#[cfg(not(target_os = "macos"))]
fn get_clipboard_change_count() -> i64 {
    // On non-macOS, return 0 - we'll fall back to hash-based detection
    0
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
            let mut last_change_count: i64 = get_clipboard_change_count();
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

                // Check if clipboard has changed using macOS change count
                let current_change_count = get_clipboard_change_count();
                if current_change_count != 0 {
                    if current_change_count == last_change_count {
                        // Clipboard hasn't changed, skip this iteration
                        std::thread::sleep(Duration::from_millis(300));
                        continue;
                    }
                    // New clipboard change detected - query frontmost app IMMEDIATELY
                    // before user has a chance to switch apps
                    eprintln!("[powerpaste] clipboard changed: {} -> {}", last_change_count, current_change_count);
                    cached_source_app = Some(macos_query_frontmost_app_info());
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

                // Try to get image from clipboard first
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

                // Check for file URLs on clipboard (e.g., from Finder)
                if !handled {
                    if let Some(file_paths) = get_clipboard_file_urls() {
                        // Join paths with newline for storage
                        let text = file_paths.join("\n");
                        let current_hash = text_hash(&text);
                        
                        if last_text_hash != Some(current_hash) {
                            // Use cached source app info from when clipboard changed
                            let (source_app_name, source_app_bundle_id) = cached_source_app
                                .clone()
                                .unwrap_or_else(|| (None, None));
                            
                            // Store as file content type
                            match db::insert_text_with_source_app(&app, &text, Some("file".to_string()), source_app_name, source_app_bundle_id) {
                                Ok(Some(item)) => {
                                    last_text_hash = Some(current_hash);
                                    processed_this_change = true;
                                    did_change = true;
                                    handled = true;
                                    let _ = app.emit("powerpaste://new_item", item);
                                }
                                Ok(None) => {
                                    last_text_hash = Some(current_hash);
                                    processed_this_change = true;
                                    did_change = true;
                                    handled = true;
                                }
                                Err(e) => {
                                    eprintln!("[powerpaste] failed to insert file paths: {e}");
                                }
                            }
                        } else {
                            handled = true;
                        }
                    } else {
                        handled = true;
                    }
                    if handled {
                        sleep_ms = 250;
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
                    let current_hash = text_hash(&text);
                    if last_text_hash == Some(current_hash) {
                        sleep_ms = (sleep_ms + 100).min(1000);
                        std::thread::sleep(Duration::from_millis(sleep_ms));
                        continue;
                    }

                    // Detect content type for the text
                    let content_type = detect_content_type(&text);
                
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

/// Write file paths to the clipboard (macOS: as file URLs that Finder can paste)
#[cfg(target_os = "macos")]
pub fn set_clipboard_files(paths: &[String]) -> Result<(), String> {
    use std::process::Command;
    
    if paths.is_empty() {
        return Err("no file paths provided".to_string());
    }
    
    // Use Swift to properly set NSPasteboard with file URLs
    // This ensures Finder can paste the files correctly
    let mut swift_code = String::from(r#"
import Cocoa

let pasteboard = NSPasteboard.general
pasteboard.clearContents()

var urls: [URL] = []
"#);
    
    for path in paths.iter() {
        // Escape the path for Swift string literal
        let escaped = path.replace('\\', "\\\\").replace('"', "\\\"");
        swift_code.push_str(&format!(
            "urls.append(URL(fileURLWithPath: \"{}\"))\n",
            escaped
        ));
    }
    
    swift_code.push_str(r#"
pasteboard.writeObjects(urls as [NSPasteboardWriting])
"#);
    
    let output = Command::new("swift")
        .args(["-e", &swift_code])
        .output()
        .map_err(|e| format!("failed to run swift: {e}"))?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("swift failed: {stderr}"));
    }
    
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn set_clipboard_files(_paths: &[String]) -> Result<(), String> {
    // TODO: Implement for Windows using OLE clipboard with CF_HDROP format
    Err("file clipboard not yet implemented on this platform".to_string())
}
