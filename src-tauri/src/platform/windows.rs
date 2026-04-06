use std::process::Command;
use tauri::Manager;

// --- Last frontmost app tracking (no-op on Windows) ---

pub fn set_last_frontmost_app_name(_name: String) {}

#[allow(dead_code)]
pub fn get_last_frontmost_app_name() -> Option<String> {
    None
}

// --- Frontmost app querying ---

pub fn query_frontmost_app_info() -> (Option<String>, Option<String>) {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId,
    };

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return (None, None);
        }

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return (None, None);
        }

        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok();
        let handle = match handle {
            Some(h) => h,
            None => return (None, None),
        };

        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT(0),
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(handle);

        if ok.is_err() || len == 0 {
            return (None, None);
        }

        let exe_path = String::from_utf16_lossy(&buf[..len as usize]);
        // Extract just the filename without extension as the "app name"
        let name = std::path::Path::new(&exe_path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string());
        // Use the full exe path as the "bundle id" equivalent on Windows
        let bundle_id = Some(exe_path);

        (name, bundle_id)
    }
}

// --- Cursor position ---

pub fn get_cursor_position() -> Option<(f64, f64)> {
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    use windows::Win32::Foundation::POINT;
    let mut pt = POINT { x: 0, y: 0 };
    let ok = unsafe { GetCursorPos(&mut pt) };
    if ok.is_ok() {
        Some((pt.x as f64, pt.y as f64))
    } else {
        None
    }
}

// --- Perform paste ---

pub fn perform_paste(app: &tauri::AppHandle) -> Result<(), String> {
    use std::time::Duration;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
        KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL, VK_V,
    };

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    // Wait for focus to return to the previous app
    std::thread::sleep(Duration::from_millis(200));

    eprintln!("[powerpaste] perform_paste: sending Ctrl+V on Windows...");

    let inputs = [
        // Ctrl down
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_CONTROL,
                    wScan: 0,
                    dwFlags: Default::default(),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        // V down
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(VK_V.0),
                    wScan: 0,
                    dwFlags: Default::default(),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        // V up
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(VK_V.0),
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        // Ctrl up
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_CONTROL,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
    ];

    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent != inputs.len() as u32 {
        return Err(format!(
            "SendInput only injected {sent}/{} events",
            inputs.len()
        ));
    }
    eprintln!("[powerpaste] perform_paste: Ctrl+V sent successfully");

    Ok(())
}

// --- Permissions ---

pub fn check_permissions() -> Result<crate::PermissionsStatus, String> {
    let exe_path = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    Ok(crate::PermissionsStatus {
        platform: "windows".to_string(),
        can_paste: true,
        automation_ok: true,
        accessibility_ok: true,
        details: None,
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

pub fn get_app_icon_path(app: &tauri::AppHandle, bundle_id: &str) -> Result<Option<String>, String> {
    // On Windows, bundle_id is the full exe path
    let exe_path = bundle_id;

    if !std::path::Path::new(exe_path).exists() {
        return Ok(None);
    }

    let cache_dir = crate::paths::app_data_dir(app)
        .map_err(|e| format!("failed to get app data dir: {e}"))?
        .join("icon_cache");
    std::fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("failed to create icon cache dir: {e}"))?;

    let safe_name = exe_path
        .replace(|c: char| !c.is_alphanumeric() && c != '.', "_");
    let cached_png = cache_dir.join(format!("{}.png", safe_name));

    if cached_png.exists() {
        return Ok(Some(cached_png.to_string_lossy().to_string()));
    }

    // Use PowerShell to extract the icon from the exe and save as PNG
    let ps_script = format!(
        r#"Add-Type -AssemblyName System.Drawing; $icon = [System.Drawing.Icon]::ExtractAssociatedIcon('{}'); if ($icon) {{ $bmp = $icon.ToBitmap(); $bmp.Save('{}', [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose(); $icon.Dispose() }}"#,
        exe_path.replace('\'', "''"),
        cached_png.to_string_lossy().replace('\'', "''")
    );

    let result = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
        .output();

    if let Ok(out) = result {
        if out.status.success() && cached_png.exists() {
            return Ok(Some(cached_png.to_string_lossy().to_string()));
        }
    }

    Ok(None)
}

// --- Window configuration ---

/// Configure the window as a frameless floating popup on Windows:
/// - Remove from taskbar / Alt+Tab (WS_EX_TOOLWINDOW)
/// - Rounded corners on Windows 11+ (DWM)
/// - Drop shadow via DWM
pub fn configure_floating_window<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWA_USE_IMMERSIVE_DARK_MODE,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongW, SetWindowLongW, GWL_EXSTYLE, WS_EX_TOOLWINDOW, WS_EX_APPWINDOW,
    };

    // Skip taskbar via Tauri API
    let _ = window.set_skip_taskbar(true);

    // Access the raw HWND to set window styles and DWM attributes
    if let Ok(raw) = window.hwnd() {
        let hwnd = HWND(raw.0);
        unsafe {
            // Set WS_EX_TOOLWINDOW to hide from taskbar and Alt+Tab,
            // and remove WS_EX_APPWINDOW if present
            let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
            let new_style = (ex_style | WS_EX_TOOLWINDOW.0) & !WS_EX_APPWINDOW.0;
            SetWindowLongW(hwnd, GWL_EXSTYLE, new_style as i32);

            // Enable rounded corners on Windows 11+
            // DWM_WINDOW_CORNER_PREFERENCE: DWMWCP_ROUND = 2
            let corner_pref: u32 = 2; // DWMWCP_ROUND
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &corner_pref as *const u32 as *const _,
                std::mem::size_of::<u32>() as u32,
            );

            // Enable dark mode title bar to match app theme
            let dark_mode: u32 = 1;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &dark_mode as *const u32 as *const _,
                std::mem::size_of::<u32>() as u32,
            );
        }
    }
}

// --- Browser accelerator keys ---

pub fn suspend_browser_accelerator_keys(webview: &tauri::Webview) {
    let _ = webview.with_webview(move |wv| {
        unsafe {
            use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
            use windows::core::Interface;
            let controller = wv.controller();
            if let Ok(core) = controller.CoreWebView2() {
                if let Ok(settings) = core.Settings() {
                    if let Ok(settings3) = settings.cast::<ICoreWebView2Settings3>() {
                        let _ = settings3.SetAreBrowserAcceleratorKeysEnabled(false.into());
                        eprintln!("[powerpaste] disabled browser accelerator keys for recording");
                    }
                }
            }
        }
    });
}

pub fn resume_browser_accelerator_keys(webview: &tauri::Webview) {
    let _ = webview.with_webview(move |wv| {
        unsafe {
            use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
            use windows::core::Interface;
            let controller = wv.controller();
            if let Ok(core) = controller.CoreWebView2() {
                if let Ok(settings) = core.Settings() {
                    if let Ok(settings3) = settings.cast::<ICoreWebView2Settings3>() {
                        let _ = settings3.SetAreBrowserAcceleratorKeysEnabled(true.into());
                        eprintln!("[powerpaste] re-enabled browser accelerator keys");
                    }
                }
            }
        }
    });
}

// --- Clipboard: change count ---

pub fn get_clipboard_change_count() -> i64 {
    use windows::Win32::System::DataExchange::GetClipboardSequenceNumber;
    unsafe { GetClipboardSequenceNumber() as i64 }
}

// --- Clipboard: file URLs ---

pub fn get_clipboard_file_urls() -> Option<Vec<String>> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, OpenClipboard,
    };
    use windows::Win32::System::Ole::CF_HDROP;
    use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};

    unsafe {
        if OpenClipboard(Some(HWND::default())).is_err() {
            return None;
        }

        let result = (|| -> Option<Vec<String>> {
            let handle = GetClipboardData(CF_HDROP.0 as u32).ok()?;
            let hdrop = HDROP(handle.0);

            let count = DragQueryFileW(hdrop, 0xFFFFFFFF, None);
            if count == 0 {
                return None;
            }

            let mut paths = Vec::new();
            for i in 0..count {
                let len = DragQueryFileW(hdrop, i, None);
                if len == 0 {
                    continue;
                }
                let mut buf = vec![0u16; (len + 1) as usize];
                DragQueryFileW(hdrop, i, Some(&mut buf));
                // Remove trailing null
                if let Some(pos) = buf.iter().position(|&c| c == 0) {
                    buf.truncate(pos);
                }
                let path = String::from_utf16_lossy(&buf);
                if !path.is_empty() {
                    paths.push(path);
                }
            }

            if paths.is_empty() { None } else { Some(paths) }
        })();

        let _ = CloseClipboard();
        result
    }
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

    // Encode as PNG
    let mut png_data = Vec::new();
    {
        let encoder = image::codecs::png::PngEncoder::new(&mut png_data);
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

pub fn set_clipboard_files(paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Err("no file paths provided".to_string());
    }

    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GHND,
    };
    use windows::Win32::System::Ole::CF_HDROP;

    // Build DROPFILES structure + double-null terminated file list (UTF-16)
    let header_size = std::mem::size_of::<windows::Win32::UI::Shell::DROPFILES>();
    let mut file_data: Vec<u16> = Vec::new();
    for p in paths {
        eprintln!("[powerpaste] set_clipboard_files: path={}", p);
        let wide: Vec<u16> = p.encode_utf16().collect();
        file_data.extend_from_slice(&wide);
        file_data.push(0); // null-terminate each path
    }
    file_data.push(0); // double-null terminate

    let data_bytes = file_data.len() * 2; // u16 = 2 bytes each
    let total = header_size + data_bytes;

    unsafe {
        // Open clipboard first so we don't leak memory if it fails
        if OpenClipboard(Some(HWND::default())).is_err() {
            return Err("OpenClipboard failed".to_string());
        }
        let _ = EmptyClipboard();

        let hmem = match GlobalAlloc(GHND, total) {
            Ok(h) => h,
            Err(e) => {
                let _ = CloseClipboard();
                return Err(format!("GlobalAlloc failed: {e}"));
            }
        };
        let ptr = GlobalLock(hmem) as *mut u8;
        if ptr.is_null() {
            let _ = CloseClipboard();
            return Err("GlobalLock failed".to_string());
        }

        // Write DROPFILES header
        let pfiles = header_size as u32;
        std::ptr::copy_nonoverlapping(&pfiles as *const u32 as *const u8, ptr, 4);
        // pt.x=0, pt.y=0, fNC=0 (bytes 4..16 = zero, already zeroed by GHND)
        // fWide = TRUE (1) at offset 16
        let f_wide: u32 = 1;
        std::ptr::copy_nonoverlapping(
            &f_wide as *const u32 as *const u8,
            ptr.add(16),
            4,
        );
        // Copy file paths
        std::ptr::copy_nonoverlapping(
            file_data.as_ptr() as *const u8,
            ptr.add(header_size),
            data_bytes,
        );

        let _ = GlobalUnlock(hmem);

        let handle = windows::Win32::Foundation::HANDLE(hmem.0);
        // On success, SetClipboardData takes ownership of hmem
        let result = SetClipboardData(CF_HDROP.0 as u32, Some(handle));
        let _ = CloseClipboard();

        result.map_err(|e| format!("SetClipboardData failed: {e}"))?;
    }

    Ok(())
}
