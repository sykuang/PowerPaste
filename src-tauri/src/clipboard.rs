use crate::db;
use crate::macos_query_frontmost_app_info;
use arboard::{Clipboard, ImageData};
use std::borrow::Cow;
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
    use objc2_app_kit::NSPasteboard;
    use objc2_foundation::NSString;

    let pasteboard = NSPasteboard::generalPasteboard();
    let file_url_type = NSString::from_str("public.file-url");

    let items = pasteboard.pasteboardItems()?;
    let mut paths = Vec::new();

    for item in items.iter() {
        if let Some(url_string) = item.stringForType(&file_url_type) {
            let url_str = url_string.to_string();
            // Convert file:// URL to path
            if let Some(path) = url_str.strip_prefix("file://") {
                // URL-decode the path (e.g., %20 -> space)
                let decoded = percent_decode(path);
                if !decoded.is_empty() {
                    paths.push(decoded);
                }
            }
        }
    }

    if !paths.is_empty() {
        Some(paths)
    } else {
        None
    }
}

/// Simple percent-decoding for file URL paths
#[cfg(target_os = "macos")]
fn percent_decode(input: &str) -> String {
    let mut result = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(
                &input[i + 1..i + 3],
                16,
            ) {
                result.push(byte);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(result).unwrap_or_else(|_| input.to_string())
}

#[cfg(not(target_os = "macos"))]
fn get_clipboard_file_urls() -> Option<Vec<String>> {
    None
}

#[cfg(target_os = "macos")]
fn get_clipboard_image_encoded() -> Option<db::EncodedImage> {
    use objc2_app_kit::NSPasteboard;
    use objc2_foundation::NSString;

    let pasteboard = NSPasteboard::generalPasteboard();

    // Check for image types in order of preference
    let types: &[(&str, &str)] = &[
        ("public.png", "image/png"),
        ("public.jpeg", "image/jpeg"),
        ("org.webmproject.webp", "image/webp"),
        ("public.tiff", "image/tiff"),
    ];

    for (uti, mime) in types {
        let pb_type = NSString::from_str(uti);
        if let Some(data) = pasteboard.dataForType(&pb_type) {
            // Safety: we hold the Retained<NSData> so it won't be mutated
            let bytes = unsafe { data.as_bytes_unchecked() };
            return Some(db::EncodedImage {
                bytes: bytes.to_vec(),
                mime: mime.to_string(),
            });
        }
    }
    None
}

#[cfg(not(target_os = "macos"))]
fn get_clipboard_image_encoded() -> Option<db::EncodedImage> {
    None
}

#[cfg(target_os = "macos")]
fn mime_to_uti(mime: &str) -> Option<&'static str> {
    match mime {
        "image/png" => Some("public.png"),
        "image/jpeg" | "image/jpg" => Some("public.jpeg"),
        "image/webp" => Some("org.webmproject.webp"),
        "image/tiff" => Some("public.tiff"),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
fn set_clipboard_image_encoded_macos(bytes: &[u8], uti: &str) -> Result<(), String> {
    use objc2_app_kit::NSPasteboard;
    use objc2_foundation::{NSData, NSString};

    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();

    let ns_data = NSData::with_bytes(bytes);
    let pb_type = NSString::from_str(uti);

    if pasteboard.setData_forType(Some(&ns_data), &pb_type) {
        Ok(())
    } else {
        Err("failed to set image clipboard with original format".to_string())
    }
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
                                let encoded_image = get_clipboard_image_encoded();
                                
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
                        // No file URLs on the clipboard; fall through to text handling.
                        // Guard against regressions where we accidentally short-circuit text inserts.
                        debug_assert!(!handled, "no file URLs should not short-circuit text handling");
                        handled = false;
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

/// Write encoded image bytes to the clipboard, preserving format when possible.
pub fn set_clipboard_image_encoded(bytes: &[u8], mime: Option<&str>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        if let Some(mime) = mime {
            if let Some(uti) = mime_to_uti(mime) {
                if set_clipboard_image_encoded_macos(bytes, uti).is_ok() {
                    return Ok(());
                }
            }
        }
    }

    set_clipboard_image_decoded(bytes)
}

fn set_clipboard_image_decoded(encoded_bytes: &[u8]) -> Result<(), String> {
    let img = image::load_from_memory(encoded_bytes)
        .map_err(|e| format!("failed to decode image: {e}"))?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let bytes = rgba.into_raw();

    let mut clipboard = Clipboard::new().map_err(|e| format!("clipboard init failed: {e}"))?;
    clipboard
        .set_image(ImageData {
            width: width as usize,
            height: height as usize,
            bytes: Cow::Owned(bytes),
        })
        .map_err(|e| format!("clipboard set image failed: {e}"))
}

/// Write file paths to the clipboard (macOS: as file URLs that Finder can paste)
#[cfg(target_os = "macos")]
pub fn set_clipboard_files(paths: &[String]) -> Result<(), String> {
    use objc2::msg_send;
    use objc2_app_kit::NSPasteboard;
    use objc2_foundation::{NSArray, NSString, NSURL};

    if paths.is_empty() {
        return Err("no file paths provided".to_string());
    }

    eprintln!("[powerpaste] set_clipboard_files: writing {} paths", paths.len());

    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();

    let urls: Vec<_> = paths
        .iter()
        .map(|p| {
            eprintln!("[powerpaste] set_clipboard_files: path={}", p);
            NSURL::fileURLWithPath(&NSString::from_str(p))
        })
        .collect();

    let url_refs: Vec<&NSURL> = urls.iter().map(|u| u.as_ref()).collect();
    let array = NSArray::from_slice(&url_refs);

    // Use msg_send! to call writeObjects: directly, avoiding the generic type cast.
    // Safety: NSURL implements NSPasteboardWriting (declared in objc2-app-kit).
    let ok: bool = unsafe { msg_send![&*pasteboard, writeObjects: &*array] };
    eprintln!("[powerpaste] set_clipboard_files: writeObjects returned {}", ok);

    if !ok {
        return Err("writeObjects returned false".to_string());
    }

    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn set_clipboard_files(_paths: &[String]) -> Result<(), String> {
    Err("file clipboard is only supported on macOS".to_string())
}
