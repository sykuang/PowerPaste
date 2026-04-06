use std::process::Command;
use std::sync::{Mutex, OnceLock};

// --- Last frontmost app tracking ---

static LAST_FRONTMOST_APP_NAME: OnceLock<Mutex<Option<String>>> = OnceLock::new();

pub fn set_last_frontmost_app_name(name: String) {
    let cell = LAST_FRONTMOST_APP_NAME.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock().unwrap_or_else(|e| e.into_inner());
    *guard = Some(name);
}

#[allow(dead_code)]
pub fn get_last_frontmost_app_name() -> Option<String> {
    let cell = LAST_FRONTMOST_APP_NAME.get_or_init(|| Mutex::new(None));
    let guard = cell.lock().unwrap_or_else(|e| e.into_inner());
    guard.clone()
}

// --- Frontmost app querying ---

pub fn query_frontmost_app_name() -> Option<String> {
    let output = Command::new("osascript")
        .args([
            "-e",
            "tell application \"System Events\" to get name of first application process whose frontmost is true",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if name.is_empty() { None } else { Some(name) }
}

fn query_frontmost_app_bundle_id() -> Option<String> {
    let output = Command::new("osascript")
        .args([
            "-e",
            "tell application \"System Events\" to get bundle identifier of first application process whose frontmost is true",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let bundle_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if bundle_id.is_empty() { None } else { Some(bundle_id) }
}

pub fn query_frontmost_app_info() -> (Option<String>, Option<String>) {
    (query_frontmost_app_name(), query_frontmost_app_bundle_id())
}

// --- Cursor position ---

pub fn get_cursor_position() -> Option<(f64, f64)> {
    use objc2_app_kit::NSEvent;
    let loc = NSEvent::mouseLocation();
    Some((loc.x, loc.y))
}

// --- Perform paste ---

pub fn perform_paste(app: &tauri::AppHandle) -> Result<(), String> {
    use std::time::Duration;
    use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

    eprintln!("[powerpaste] paste_text: starting...");

    let hidden = Arc::new(AtomicBool::new(false));
    let hidden_clone = hidden.clone();
    let app = app.clone();

    let _ = app.run_on_main_thread(move || {
        use objc2::rc::Retained;
        use objc2::exception;
        use objc2_app_kit::{NSPanel, NSApplication};
        use objc2::MainThreadMarker;

        eprintln!("[powerpaste] paste_text: on main thread, hiding panel...");

        if let Some(stored) = crate::OVERLAY_PANEL_PTR.get() {
            let panel: Option<Retained<NSPanel>> = unsafe {
                Retained::retain((*stored as *mut NSPanel).cast())
            };
            if let Some(panel) = panel {
                if panel.isVisible() {
                    eprintln!("[powerpaste] paste_text: ordering out panel");
                    let _ = exception::catch(std::panic::AssertUnwindSafe(|| {
                        panel.orderOut(None);
                    }));
                    crate::macos_remove_keyboard_monitor();
                    crate::macos_remove_click_outside_monitor();
                    crate::macos_remove_mouse_focus_monitor();
                }
            }
        }

        if let Some(mtm) = MainThreadMarker::new() {
            let ns_app = NSApplication::sharedApplication(mtm);
            eprintln!("[powerpaste] paste_text: hiding NSApplication");
            ns_app.hide(None);
        }

        hidden_clone.store(true, Ordering::SeqCst);
        eprintln!("[powerpaste] paste_text: panel hidden, app hidden");
    });

    for _ in 0..50 {
        if hidden.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    std::thread::sleep(Duration::from_millis(200));

    eprintln!("[powerpaste] paste_text: sending Cmd+V...");
    let output = Command::new("osascript")
        .args([
            "-e",
            "tell application \"System Events\" to keystroke \"v\" using command down",
        ])
        .output()
        .map_err(|e| format!("failed to run osascript for paste: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let msg = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            "osascript paste failed".to_string()
        };
        return Err(format!(
            "Paste failed. On macOS this requires Accessibility + Automation permissions. \
System Settings → Privacy & Security → Accessibility (enable PowerPaste) and \
Privacy & Security → Automation (allow controlling System Events). Details: {msg}"
        ));
    }
    eprintln!("[powerpaste] paste_text: done!");

    Ok(())
}

// --- Permissions ---

pub fn check_permissions() -> Result<crate::PermissionsStatus, String> {
    let exe_path = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let is_bundled = exe_path.contains(".app/Contents/MacOS/");

    let accessibility_ok: bool = unsafe {
        #[link(name = "ApplicationServices", kind = "framework")]
        extern "C" {
            fn AXIsProcessTrusted() -> u8;
        }
        let result = AXIsProcessTrusted();
        eprintln!("[powerpaste] AXIsProcessTrusted returned: {}", result);
        result != 0
    };
    eprintln!("[powerpaste] accessibility_ok: {}, exe_path: {}, is_bundled: {}", accessibility_ok, exe_path, is_bundled);

    let automation = Command::new("osascript")
        .args([
            "-e",
            "tell application \"System Events\" to get name of first application process whose frontmost is true",
        ])
        .output();

    let (automation_ok, details) = match automation {
        Ok(out) if out.status.success() => (true, None),
        Ok(out) => {
            let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
            (false, Some(if msg.is_empty() { "Automation check failed".to_string() } else { msg }))
        }
        Err(e) => (false, Some(format!("Automation check failed: {e}"))),
    };

    let can_paste = automation_ok && accessibility_ok;

    let final_details = if !accessibility_ok && !automation_ok {
        Some("Both Accessibility and Automation permissions are required.".to_string())
    } else if !accessibility_ok {
        Some("Accessibility permission is required.".to_string())
    } else {
        details
    };

    Ok(crate::PermissionsStatus {
        platform: "macos".to_string(),
        can_paste,
        automation_ok,
        accessibility_ok,
        details: final_details,
        is_bundled,
        executable_path: exe_path,
    })
}

pub fn open_accessibility_settings() -> Result<(), String> {
    Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .status()
        .map_err(|e| format!("failed to open Accessibility settings: {e}"))?;
    Ok(())
}

pub fn open_automation_settings() -> Result<(), String> {
    Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Automation")
        .status()
        .map_err(|e| format!("failed to open Automation settings: {e}"))?;
    Ok(())
}

pub fn request_accessibility_permission() -> Result<bool, String> {
    let trusted: bool = unsafe {
        #[link(name = "CoreFoundation", kind = "framework")]
        extern "C" {
            fn CFStringCreateWithCString(
                alloc: *const std::ffi::c_void,
                c_str: *const u8,
                encoding: u32,
            ) -> *const std::ffi::c_void;
            fn CFDictionaryCreate(
                alloc: *const std::ffi::c_void,
                keys: *const *const std::ffi::c_void,
                values: *const *const std::ffi::c_void,
                num_values: isize,
                key_callbacks: *const std::ffi::c_void,
                value_callbacks: *const std::ffi::c_void,
            ) -> *const std::ffi::c_void;
            fn CFRelease(cf: *const std::ffi::c_void);
            static kCFBooleanTrue: *const std::ffi::c_void;
            static kCFTypeDictionaryKeyCallBacks: u8;
            static kCFTypeDictionaryValueCallBacks: u8;
        }

        #[link(name = "ApplicationServices", kind = "framework")]
        extern "C" {
            fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> u8;
        }

        // "AXTrustedCheckOptionPrompt" — the key that triggers the prompt
        let key_cstr = b"AXTrustedCheckOptionPrompt\0";
        let key = CFStringCreateWithCString(
            std::ptr::null(),
            key_cstr.as_ptr(),
            0x08000100, // kCFStringEncodingUTF8
        );

        let keys = [key];
        let values = [kCFBooleanTrue];
        let dict = CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            &kCFTypeDictionaryKeyCallBacks as *const u8 as *const std::ffi::c_void,
            &kCFTypeDictionaryValueCallBacks as *const u8 as *const std::ffi::c_void,
        );

        let result = AXIsProcessTrustedWithOptions(dict);
        eprintln!("[powerpaste] AXIsProcessTrustedWithOptions(prompt:true) returned: {}", result);

        CFRelease(dict);
        CFRelease(key);

        result != 0
    };
    Ok(trusted)
}

pub fn request_automation_permission() -> Result<bool, String> {
    let output = Command::new("osascript")
        .args([
            "-e",
            "tell application \"System Events\" to return 1",
        ])
        .output()
        .map_err(|e| format!("failed to run osascript: {e}"))?;

    let ok = output.status.success();
    if !ok {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("[powerpaste] request_automation_permission osascript stderr: {}", stderr);
    }
    Ok(ok)
}

// --- App icon ---

pub fn get_app_icon_path(app: &tauri::AppHandle, bundle_id: &str) -> Result<Option<String>, String> {
    let cache_dir = crate::paths::app_data_dir(app)
        .map_err(|e| format!("failed to get app data dir: {e}"))?
        .join("icon_cache");

    std::fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("failed to create icon cache dir: {e}"))?;

    let safe_bundle_id = bundle_id.replace(|c: char| !c.is_alphanumeric() && c != '.', "_");
    let cached_png = cache_dir.join(format!("{}.png", safe_bundle_id));

    if cached_png.exists() {
        return Ok(Some(cached_png.to_string_lossy().to_string()));
    }

    let output = Command::new("mdfind")
        .args([&format!("kMDItemCFBundleIdentifier == '{}'", bundle_id)])
        .output()
        .map_err(|e| format!("failed to run mdfind: {e}"))?;

    if !output.status.success() {
        return Ok(None);
    }

    let path = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(|s| s.to_string());

    if let Some(app_path) = path {
        let plist_path = format!("{}/Contents/Info.plist", app_path);

        let icon_output = Command::new("defaults")
            .args(["read", &plist_path, "CFBundleIconFile"])
            .output();

        let mut icns_path: Option<String> = None;

        if let Ok(icon_out) = icon_output {
            if icon_out.status.success() {
                let mut icon_name = String::from_utf8_lossy(&icon_out.stdout).trim().to_string();
                if !icon_name.ends_with(".icns") {
                    icon_name.push_str(".icns");
                }
                let icon_path = format!("{}/Contents/Resources/{}", app_path, icon_name);
                if std::path::Path::new(&icon_path).exists() {
                    icns_path = Some(icon_path);
                }
            }
        }

        if icns_path.is_none() {
            let fallbacks = ["AppIcon.icns", "app.icns", "icon.icns"];
            for fallback in fallbacks {
                let icon_path = format!("{}/Contents/Resources/{}", app_path, fallback);
                if std::path::Path::new(&icon_path).exists() {
                    icns_path = Some(icon_path);
                    break;
                }
            }
        }

        if let Some(icns) = icns_path {
            let sips_result = Command::new("sips")
                .args([
                    "-s", "format", "png",
                    "-z", "64", "64",
                    &icns,
                    "--out", &cached_png.to_string_lossy()
                ])
                .output();

            if let Ok(sips_out) = sips_result {
                if sips_out.status.success() && cached_png.exists() {
                    return Ok(Some(cached_png.to_string_lossy().to_string()));
                }
            }
        }
    }

    Ok(None)
}

// --- Window configuration (no-op on macOS, handled by NSPanel) ---

pub fn configure_floating_window<R: tauri::Runtime>(_window: &tauri::WebviewWindow<R>) {}

// --- Browser accelerator keys (no-op on macOS) ---

pub fn suspend_browser_accelerator_keys(_webview: &tauri::Webview) {}

pub fn resume_browser_accelerator_keys(_webview: &tauri::Webview) {}

// --- Clipboard: change count ---

pub fn get_clipboard_change_count() -> i64 {
    use objc2_app_kit::NSPasteboard;
    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.changeCount() as i64
}

// --- Clipboard: file URLs ---

fn percent_decode(input: &str) -> String {
    let mut result = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&input[i + 1..i + 3], 16) {
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

pub fn get_clipboard_file_urls() -> Option<Vec<String>> {
    use objc2_app_kit::NSPasteboard;
    use objc2_foundation::NSString;

    let pasteboard = NSPasteboard::generalPasteboard();
    let file_url_type = NSString::from_str("public.file-url");

    let items = pasteboard.pasteboardItems()?;
    let mut paths = Vec::new();

    for item in items.iter() {
        if let Some(url_string) = item.stringForType(&file_url_type) {
            let url_str = url_string.to_string();
            if let Some(path) = url_str.strip_prefix("file://") {
                let decoded = percent_decode(path);
                if !decoded.is_empty() {
                    paths.push(decoded);
                }
            }
        }
    }

    if !paths.is_empty() { Some(paths) } else { None }
}

// --- Clipboard: image ---

pub fn get_clipboard_image_encoded() -> Option<crate::db::EncodedImage> {
    use objc2_app_kit::NSPasteboard;
    use objc2_foundation::NSString;

    let pasteboard = NSPasteboard::generalPasteboard();

    let types: &[(&str, &str)] = &[
        ("public.png", "image/png"),
        ("public.jpeg", "image/jpeg"),
        ("org.webmproject.webp", "image/webp"),
        ("public.tiff", "image/tiff"),
    ];

    for (uti, mime) in types {
        let pb_type = NSString::from_str(uti);
        if let Some(data) = pasteboard.dataForType(&pb_type) {
            let bytes = unsafe { data.as_bytes_unchecked() };
            return Some(crate::db::EncodedImage {
                bytes: bytes.to_vec(),
                mime: mime.to_string(),
            });
        }
    }
    None
}

fn mime_to_uti(mime: &str) -> Option<&'static str> {
    match mime {
        "image/png" => Some("public.png"),
        "image/jpeg" | "image/jpg" => Some("public.jpeg"),
        "image/webp" => Some("org.webmproject.webp"),
        "image/tiff" => Some("public.tiff"),
        _ => None,
    }
}

fn set_clipboard_image_native(bytes: &[u8], uti: &str) -> Result<(), String> {
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

pub fn set_clipboard_image_encoded(bytes: &[u8], mime: Option<&str>) -> Result<(), String> {
    if let Some(mime) = mime {
        if let Some(uti) = mime_to_uti(mime) {
            if set_clipboard_image_native(bytes, uti).is_ok() {
                return Ok(());
            }
        }
    }
    super::set_clipboard_image_decoded(bytes)
}

// --- Clipboard: set files ---

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
