mod clipboard;
mod db;
mod models;
mod paths;
mod settings_store;
mod sync;

use models::{ClipboardItem, Settings, SyncProvider};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::sync::OnceLock;
use tauri::Emitter;
use tauri::Manager;
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use uuid::Uuid;

#[cfg(desktop)]
fn debug_log_path() -> Option<std::path::PathBuf> {
    // A stable location users can find even when launching from Finder.
    // macOS: ~/Library/Logs/PowerPaste/powerpaste-debug.log
    let home = std::env::var("HOME").ok()?;
    Some(
        std::path::PathBuf::from(home)
            .join("Library")
            .join("Logs")
            .join("PowerPaste")
            .join("powerpaste-debug.log"),
    )
}

#[cfg(desktop)]
fn append_debug_log(line: &str) {
    use std::io::Write;

    let Some(path) = debug_log_path() else {
        return;
    };

    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }

    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "{line}");
    }
}

// Overlay sizing: computed from the active monitor each time we show the window.
// These constants act as clamps so the UI stays usable across tiny/huge screens.
const OVERLAY_WIDTH_FRACTION: f64 = 0.62;
const OVERLAY_HEIGHT_FRACTION: f64 = 0.22;
const OVERLAY_MIN_WIDTH_PX: u32 = 820;
const OVERLAY_MAX_WIDTH_PX: u32 = 1200;
const OVERLAY_MIN_HEIGHT_PX: u32 = 230;
const OVERLAY_MAX_HEIGHT_PX: u32 = 270;

// Preferred overlay size, optionally set by the frontend at runtime.
static OVERLAY_PREFERRED_SIZE: OnceLock<Mutex<Option<(u32, u32)>>> = OnceLock::new();

fn set_overlay_preferred_size_global(w: u32, h: u32) {
    let cell = OVERLAY_PREFERRED_SIZE.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock().unwrap_or_else(|e| e.into_inner());
    *guard = Some((w, h));
}

fn get_overlay_preferred_size_global() -> Option<(u32, u32)> {
    let cell = OVERLAY_PREFERRED_SIZE.get_or_init(|| Mutex::new(None));
    let guard = cell.lock().unwrap_or_else(|e| e.into_inner());
    *guard
}

fn overlay_size_for_monitor(monitor_width: u32, monitor_height: u32) -> (u32, u32) {
    if let Some((pref_w, pref_h)) = get_overlay_preferred_size_global() {
        // Clamp preferred size to reasonable bounds and the current monitor.
        let width = pref_w
            .max(OVERLAY_MIN_WIDTH_PX)
            .min(OVERLAY_MAX_WIDTH_PX)
            .min(monitor_width);
        let height = pref_h
            .max(OVERLAY_MIN_HEIGHT_PX)
            .min(OVERLAY_MAX_HEIGHT_PX)
            .min(monitor_height);
        return (width, height);
    }

    let mut width = ((monitor_width as f64) * OVERLAY_WIDTH_FRACTION).round() as u32;
    let mut height = ((monitor_height as f64) * OVERLAY_HEIGHT_FRACTION).round() as u32;

    width = width.clamp(OVERLAY_MIN_WIDTH_PX, OVERLAY_MAX_WIDTH_PX).min(monitor_width);
    height = height
        .clamp(OVERLAY_MIN_HEIGHT_PX, OVERLAY_MAX_HEIGHT_PX)
        .min(monitor_height);

    (width, height)
}

#[cfg(target_os = "macos")]
static OVERLAY_PANEL_PTR: OnceLock<usize> = OnceLock::new();

/// Stores the local keyboard event monitor object pointer (leaked).
/// We need to keep it alive while the panel is visible.
#[cfg(target_os = "macos")]
static KEYBOARD_MONITOR_PTR: OnceLock<Mutex<Option<usize>>> = OnceLock::new();

/// Stores the global mouse click monitor object pointer (leaked).
/// Used to detect clicks outside the panel to hide it.
#[cfg(target_os = "macos")]
static CLICK_OUTSIDE_MONITOR_PTR: OnceLock<Mutex<Option<usize>>> = OnceLock::new();

#[cfg(target_os = "macos")]
static PANEL_INIT_RETRY_SCHEDULED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(target_os = "macos")]
static LAST_FRONTMOST_APP_NAME: OnceLock<Mutex<Option<String>>> = OnceLock::new();

#[cfg(target_os = "macos")]
fn macos_set_last_frontmost_app_name(name: String) {
    let cell = LAST_FRONTMOST_APP_NAME.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock().unwrap_or_else(|e| e.into_inner());
    *guard = Some(name);
}

#[cfg(target_os = "macos")]
fn macos_get_last_frontmost_app_name() -> Option<String> {
    let cell = LAST_FRONTMOST_APP_NAME.get_or_init(|| Mutex::new(None));
    let guard = cell.lock().unwrap_or_else(|e| e.into_inner());
    guard.clone()
}

#[cfg(target_os = "macos")]
fn macos_query_frontmost_app_name() -> Option<String> {
    use std::process::Command;
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
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

#[cfg(target_os = "macos")]
fn macos_get_cursor_position() -> Option<(f64, f64)> {
    use objc2_app_kit::NSEvent;
    
    let loc = NSEvent::mouseLocation();
    Some((loc.x, loc.y))
}

/// Find the screen that contains the mouse cursor.
#[cfg(target_os = "macos")]
fn macos_screen_containing_cursor(mtm: objc2::MainThreadMarker) -> Option<objc2::rc::Retained<objc2_app_kit::NSScreen>> {
    use objc2_app_kit::NSScreen;
    
    let (cursor_x, cursor_y) = macos_get_cursor_position()?;
    
    let screens = NSScreen::screens(mtm);
    let count = screens.len();
    
    for i in 0..count {
        let screen: objc2::rc::Retained<NSScreen> = unsafe {
            objc2::msg_send_id![&*screens, objectAtIndex: i]
        };
        let frame = screen.frame();
        
        if cursor_x >= frame.origin.x && cursor_x < frame.origin.x + frame.size.width
            && cursor_y >= frame.origin.y && cursor_y < frame.origin.y + frame.size.height
        {
            return Some(screen);
        }
    }
    
    None
}

/// Install a local keyboard event monitor to capture Cmd+A and Cmd+C.
/// NSPanel overlays don't receive menu bar shortcuts, so we capture them directly.
#[cfg(target_os = "macos")]
fn macos_install_keyboard_monitor<R: tauri::Runtime>(app_handle: tauri::AppHandle<R>) {
    use block2::StackBlock;
    use objc2::rc::Retained;
    use objc2_app_kit::{NSEvent, NSEventMask, NSEventType};
    use std::ptr::NonNull;

    // Check if already installed
    let cell = KEYBOARD_MONITOR_PTR.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_some() {
        return; // Already installed
    }

    eprintln!("[powerpaste] installing keyboard monitor");

    let app_for_block = app_handle.clone();
    
    // Create a block that handles keyboard events
    // The block signature is: fn(NonNull<NSEvent>) -> *mut NSEvent
    let handler = StackBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        // SAFETY: NSEvent pointer is valid during callback
        let event_ref: &NSEvent = unsafe { event.as_ref() };
        
        let event_type = event_ref.r#type();
        if event_type != NSEventType::KeyDown {
            return event.as_ptr(); // Pass through non-keydown events
        }
        
        let modifiers = event_ref.modifierFlags();
        // Check for Command key (bit 20 = 0x100000)
        let has_cmd = modifiers.0 & (1 << 20) != 0;
        
        if !has_cmd {
            return event.as_ptr(); // Pass through non-Cmd events
        }
        
        // Get the key character
        let chars = event_ref.charactersIgnoringModifiers();
        let key = chars.map(|s| s.to_string()).unwrap_or_default();
        
        match key.to_lowercase().as_str() {
            "a" => {
                eprintln!("[powerpaste] keyboard monitor: Cmd+A captured");
                if let Some(window) = app_for_block.get_webview_window("main") {
                    let _ = window.emit(FRONTEND_EVENT_SELECT_ALL, ());
                }
                std::ptr::null_mut() // Consume the event
            }
            "c" => {
                eprintln!("[powerpaste] keyboard monitor: Cmd+C captured");
                if let Some(window) = app_for_block.get_webview_window("main") {
                    let _ = window.emit(FRONTEND_EVENT_COPY_SELECTED, ());
                }
                std::ptr::null_mut() // Consume the event
            }
            _ => event.as_ptr(), // Pass through other Cmd+key combos
        }
    });

    // Install the monitor for key down events
    let monitor = unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(
            NSEventMask::KeyDown,
            &handler,
        )
    };
    
    if let Some(m) = monitor {
        // Store the monitor pointer (leaked to keep it alive)
        let ptr = Retained::into_raw(m) as usize;
        *guard = Some(ptr);
        eprintln!("[powerpaste] keyboard monitor installed successfully");
    } else {
        eprintln!("[powerpaste] failed to install keyboard monitor");
    }
}

/// Remove the local keyboard event monitor when panel is hidden.
#[cfg(target_os = "macos")]
fn macos_remove_keyboard_monitor() {
    use objc2_app_kit::NSEvent;

    let cell = KEYBOARD_MONITOR_PTR.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock().unwrap_or_else(|e| e.into_inner());
    
    if let Some(ptr) = guard.take() {
        eprintln!("[powerpaste] removing keyboard monitor");
        unsafe {
            // Reconstruct the retained object and let it drop
            let monitor: *mut objc2::runtime::AnyObject = ptr as *mut _;
            NSEvent::removeMonitor(&*monitor);
        }
    }
}

/// Install a global mouse click monitor to detect clicks outside the panel.
/// NSPanel with NonactivatingPanel style doesn't trigger focus-lost events,
/// so we need to monitor global mouse clicks to hide the panel.
#[cfg(target_os = "macos")]
fn macos_install_click_outside_monitor<R: tauri::Runtime>(app_handle: tauri::AppHandle<R>) {
    use block2::StackBlock;
    use objc2::rc::Retained;
    use objc2_app_kit::{NSEvent, NSEventMask, NSPanel};
    use std::ptr::NonNull;

    // Check if already installed
    let cell = CLICK_OUTSIDE_MONITOR_PTR.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_some() {
        return; // Already installed
    }

    eprintln!("[powerpaste] installing click-outside monitor");

    let app_for_block = app_handle.clone();
    
    // Create a block that handles global mouse click events
    // Global monitor receives NonNull<NSEvent> and returns nothing (void)
    let handler = StackBlock::new(move |event: NonNull<NSEvent>| {
        // Check if we have a panel
        let Some(stored) = OVERLAY_PANEL_PTR.get() else {
            return;
        };
        
        // SAFETY: We store a valid NSPanel pointer
        let panel: Retained<NSPanel> = match unsafe {
            Retained::retain((*stored as *mut NSPanel).cast())
        } {
            Some(p) => p,
            None => return,
        };
        
        if !panel.isVisible() {
            return;
        }
        
        // SAFETY: NSEvent pointer is valid during callback
        let event_ref: &NSEvent = unsafe { event.as_ref() };
        
        // For global monitors, locationInWindow returns screen coordinates 
        // (since the event is not associated with any of our windows)
        let screen_location = event_ref.locationInWindow();
        
        // Check if click is inside the panel frame
        let panel_frame = panel.frame();
        let inside = screen_location.x >= panel_frame.origin.x
            && screen_location.x < panel_frame.origin.x + panel_frame.size.width
            && screen_location.y >= panel_frame.origin.y
            && screen_location.y < panel_frame.origin.y + panel_frame.size.height;
        
        if !inside {
            eprintln!("[powerpaste] click outside panel detected, hiding");
            // Hide the panel
            let app_clone = app_for_block.clone();
            let _ = app_for_block.run_on_main_thread(move || {
                let _ = macos_hide_overlay_panel_if_visible(&app_clone);
            });
        }
    });

    // Install the monitor for left and right mouse down events (global = outside our app)
    let mask = NSEventMask::LeftMouseDown.union(NSEventMask::RightMouseDown);
    let monitor = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask, &handler);
    
    if let Some(m) = monitor {
        // Store the monitor pointer (leaked to keep it alive)
        let ptr = Retained::into_raw(m) as usize;
        *guard = Some(ptr);
        eprintln!("[powerpaste] click-outside monitor installed successfully");
    } else {
        eprintln!("[powerpaste] failed to install click-outside monitor");
    }
}

/// Remove the global mouse click monitor when panel is hidden.
#[cfg(target_os = "macos")]
fn macos_remove_click_outside_monitor() {
    use objc2_app_kit::NSEvent;

    let cell = CLICK_OUTSIDE_MONITOR_PTR.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock().unwrap_or_else(|e| e.into_inner());
    
    if let Some(ptr) = guard.take() {
        eprintln!("[powerpaste] removing click-outside monitor");
        unsafe {
            let monitor: *mut objc2::runtime::AnyObject = ptr as *mut _;
            NSEvent::removeMonitor(&*monitor);
        }
    }
}

// Global UI mode storage (updated from settings on toggle)
static CURRENT_UI_MODE: OnceLock<Mutex<models::UiMode>> = OnceLock::new();

fn set_current_ui_mode(mode: models::UiMode) {
    let cell = CURRENT_UI_MODE.get_or_init(|| Mutex::new(models::UiMode::default()));
    let mut guard = cell.lock().unwrap_or_else(|e| e.into_inner());
    *guard = mode;
}

fn get_current_ui_mode() -> models::UiMode {
    let cell = CURRENT_UI_MODE.get_or_init(|| Mutex::new(models::UiMode::default()));
    let guard = cell.lock().unwrap_or_else(|e| e.into_inner());
    *guard
}

struct AppState {
    watcher: Mutex<Option<clipboard::ClipboardWatcher>>,
}

#[cfg(desktop)]
const MENU_ID_SELECT_ALL: &str = "pp_select_all";

#[cfg(desktop)]
const MENU_ID_COPY_SELECTED: &str = "pp_copy_selected";

#[cfg(desktop)]
const FRONTEND_EVENT_SELECT_ALL: &str = "powerpaste://select_all";

#[cfg(desktop)]
const FRONTEND_EVENT_COPY_SELECTED: &str = "powerpaste://copy_selected";

#[tauri::command]
fn set_overlay_preferred_size(
    app: tauri::AppHandle,
    width: u32,
    height: u32,
) -> Result<(), String> {
    // Basic sanity limits.
    if width == 0 || height == 0 {
        return Err("invalid size".to_string());
    }

    set_overlay_preferred_size_global(width, height);

    // Best-effort: if the overlay is currently visible, apply the new size immediately.
    #[cfg(target_os = "macos")]
    {
        let _ = app.run_on_main_thread(move || {
            let _ = macos_resize_overlay_panel_if_present(width, height);
        });
    }

    #[cfg(not(target_os = "macos"))]
    {
        if let Some(window) = app.get_webview_window("main") {
            let window_for_task = window.clone();
            let _ = app.run_on_main_thread(move || {
                let _ = position_as_bottom_overlay(&window_for_task);
            });
        }
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn macos_resize_overlay_panel_if_present(width: u32, height: u32) -> Result<(), String> {
    use objc2::exception;
    use objc2::rc::Retained;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSScreen, NSPanel};
    use objc2_foundation::NSRect;

    let Some(stored) = OVERLAY_PANEL_PTR.get() else {
        return Ok(());
    };

    let mtm = MainThreadMarker::new().ok_or("not on main thread")?;

    // SAFETY: We store a leaked, valid NSPanel pointer (as usize). We only use it on main thread.
    let panel: Retained<NSPanel> = unsafe {
        Retained::retain((*stored as *mut NSPanel).cast()).ok_or("failed to retain NSPanel")?
    };

    if !panel.isVisible() {
        return Ok(());
    }

    let screen = panel
        .screen()
        .or_else(|| NSScreen::mainScreen(mtm))
        .ok_or("no screen found")?;
    let screen_frame: NSRect = screen.frame();

    let target_w = (width as f64).min(screen_frame.size.width);
    let target_h = (height as f64).min(screen_frame.size.height);

    let mut target = screen_frame;
    target.size.width = target_w;
    target.size.height = target_h;
    target.origin.x = screen_frame.origin.x + (screen_frame.size.width - target.size.width) / 2.0;
    target.origin.y = screen_frame.origin.y;

    exception::catch(std::panic::AssertUnwindSafe(|| {
        panel.setFrame_display(target, true);
    }))
    .map_err(|e| format!("objective-c exception resizing panel: {e:?}"))?;

    Ok(())
}

#[tauri::command]
fn hide_main_window(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let app_for_task = app.clone();
        let _ = app.run_on_main_thread(move || {
            let _ = macos_hide_overlay_panel_if_visible(&app_for_task);
        });
        return Ok(());
    }

    #[cfg(not(target_os = "macos"))]
    {
        let window = app
            .get_webview_window("main")
            .ok_or_else(|| "main window not found".to_string())?;
        window.hide().map_err(|e| format!("failed to hide window: {e}"))?;
        return Ok(());
    }
}

#[cfg(target_os = "macos")]
fn macos_hide_overlay_panel_if_visible<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<(), String> {
    use objc2::exception;
    use objc2::rc::Retained;
    use objc2_app_kit::NSPanel;

    if let Some(stored) = OVERLAY_PANEL_PTR.get() {
        // SAFETY: We store a leaked, valid NSPanel pointer (as usize). We only use it on main thread.
        let panel: Retained<NSPanel> = unsafe {
            Retained::retain((*stored as *mut NSPanel).cast()).ok_or("failed to retain NSPanel")?
        };
        if panel.isVisible() {
            exception::catch(std::panic::AssertUnwindSafe(|| {
                panel.orderOut(None);
            }))
            .map_err(|e| format!("objective-c exception hiding panel: {e:?}"))?;
            
            // Remove monitors when hiding
            macos_remove_keyboard_monitor();
            macos_remove_click_outside_monitor();
            
            return Ok(());
        }
    }

    // Fallback: if the panel isn't initialized yet, hide the main window.
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct SyncNowResult {
    imported: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct PermissionsStatus {
    platform: String,
    can_paste: bool,
    automation_ok: bool,
    accessibility_ok: bool,
    details: Option<String>,
}

#[tauri::command]
fn get_settings(app: tauri::AppHandle) -> Result<Settings, String> {
    settings_store::load_or_init_settings(&app)
}

#[tauri::command]
fn set_hotkey(app: tauri::AppHandle, hotkey: String) -> Result<Settings, String> {
    let settings = settings_store::load_or_init_settings(&app)?;
    let settings = settings_store::set_hotkey(&app, settings, hotkey)?;
    register_hotkey(&app, &settings.hotkey)?;
    Ok(settings)
}

#[tauri::command]
fn set_sync_settings(
    app: tauri::AppHandle,
    enabled: bool,
    provider: Option<SyncProvider>,
    folder: Option<String>,
    passphrase: Option<String>,
    theme: Option<String>,
) -> Result<Settings, String> {
    let settings = settings_store::load_or_init_settings(&app)?;
    let settings = if let Some(t) = theme {
        settings_store::set_theme(&app, settings, t)?
    } else {
        settings
    };

    if let Some(pw) = passphrase {
        if !pw.trim().is_empty() {
            settings_store::save_sync_passphrase(pw.trim())?;
        }
    }

    if !enabled {
        // If user turns off sync, also remove the stored passphrase.
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
fn set_ui_mode(app: tauri::AppHandle, ui_mode: models::UiMode) -> Result<Settings, String> {
    let settings = settings_store::load_or_init_settings(&app)?;
    let settings = settings_store::set_ui_mode(&app, settings, ui_mode)?;
    set_current_ui_mode(ui_mode);
    Ok(settings)
}

#[tauri::command]
fn set_show_dock_icon(app: tauri::AppHandle, show: bool) -> Result<Settings, String> {
    let settings = settings_store::load_or_init_settings(&app)?;
    let settings = settings_store::set_show_dock_icon(&app, settings, show)?;
    
    #[cfg(target_os = "macos")]
    {
        apply_dock_icon_visibility(show);
    }
    
    Ok(settings)
}

/// Apply macOS dock icon visibility based on setting.
#[cfg(target_os = "macos")]
fn apply_dock_icon_visibility(show: bool) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
    
    let Some(mtm) = MainThreadMarker::new() else { return };
    let ns_app = NSApplication::sharedApplication(mtm);
    
    let policy = if show {
        NSApplicationActivationPolicy::Regular
    } else {
        NSApplicationActivationPolicy::Accessory
    };
    let _ = ns_app.setActivationPolicy(policy);
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
fn set_item_category(app: tauri::AppHandle, id: String, category: Option<String>) -> Result<(), String> {
    let id = Uuid::parse_str(&id).map_err(|_| "invalid id".to_string())?;
    // Normalize: empty string becomes None
    let category = category.filter(|s| !s.trim().is_empty());
    db::set_pin_category(&app, id, category)
}

#[tauri::command]
fn list_categories(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    db::list_categories(&app)
}

#[tauri::command]
fn delete_item(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let id = Uuid::parse_str(&id).map_err(|_| "invalid id".to_string())?;
    db::delete_item(&app, id)
}

/// Ensure the calling window accepts mouse events (fixes macOS click-through issues).
#[tauri::command]
fn enable_mouse_events(window: tauri::WebviewWindow) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSWindow;
        use objc2::rc::Retained;

        let ptr = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| window.ns_window())) {
            Ok(Ok(p)) => p,
            Ok(Err(e)) => return Err(format!("failed to get ns_window: {e}")),
            Err(_) => return Err("ns_window panicked".to_string()),
        };

        let ns_window: Retained<NSWindow> = unsafe {
            Retained::retain(ptr.cast()).ok_or("failed to retain NSWindow")?
        };

        ns_window.setIgnoresMouseEvents(false);
    }
    Ok(())
}

#[tauri::command]
fn write_clipboard_text(text: String) -> Result<(), String> {
    clipboard::set_clipboard_text(&text)
}

#[tauri::command]
fn paste_text(app: tauri::AppHandle, text: String) -> Result<(), String> {
    clipboard::set_clipboard_text(&text)?;

    // Hide our UI before pasting so the keystroke targets the other app.
    #[cfg(target_os = "macos")]
    {
        let app_for_hide = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Some(window) = app_for_hide.get_webview_window("main") {
                let _ = macos_toggle_overlay_panel(&window);
            }
        });
    }

    #[cfg(not(target_os = "macos"))]
    {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.hide();
        }
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        use std::time::Duration;

        if let Some(app_name) = macos_get_last_frontmost_app_name() {
            let escaped = app_name.replace('"', "\\\"");
            let script = format!("tell application \"{}\" to activate", escaped);
            let _ = Command::new("osascript").args(["-e", &script]).output();
        }

        // Wait briefly for the OS to restore focus before sending the paste keystroke.
        // Without this, Cmd+V often targets our overlay window (or nothing).
        for _ in 0..20 {
            if let Some(name) = macos_query_frontmost_app_name() {
                if name != "PowerPaste" {
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }

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
    }

    Ok(())
}

#[tauri::command]
fn check_permissions() -> Result<PermissionsStatus, String> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        let automation = Command::new("osascript")
            .args([
                "-e",
                "tell application \"System Events\" to get name of first application process whose frontmost is true",
            ])
            .output();

        let (automation_ok, mut details) = match automation {
            Ok(out) if out.status.success() => (true, None),
            Ok(out) => {
                let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
                (false, Some(if msg.is_empty() { "Automation check failed".to_string() } else { msg }))
            }
            Err(e) => (false, Some(format!("Automation check failed: {e}"))),
        };

        let accessibility = Command::new("osascript")
            .args([
                "-e",
                // Empty keystroke: should be a no-op but still exercises Accessibility permission.
                "tell application \"System Events\" to keystroke \"\"",
            ])
            .output();

        let (accessibility_ok, acc_details) = match accessibility {
            Ok(out) if out.status.success() => (true, None),
            Ok(out) => {
                let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
                (false, Some(if msg.is_empty() { "Accessibility check failed".to_string() } else { msg }))
            }
            Err(e) => (false, Some(format!("Accessibility check failed: {e}"))),
        };

        if details.is_none() {
            details = acc_details;
        }

        let can_paste = automation_ok && accessibility_ok;
        return Ok(PermissionsStatus {
            platform: "macos".to_string(),
            can_paste,
            automation_ok,
            accessibility_ok,
            details,
        });
    }

    #[cfg(target_os = "windows")]
    {
        return Ok(PermissionsStatus {
            platform: "windows".to_string(),
            can_paste: false,
            automation_ok: true,
            accessibility_ok: true,
            details: Some("Paste automation is not implemented on Windows yet.".to_string()),
        });
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        return Ok(PermissionsStatus {
            platform: "linux".to_string(),
            can_paste: false,
            automation_ok: true,
            accessibility_ok: true,
            details: Some("Paste automation is not implemented on this platform yet.".to_string()),
        });
    }
}

#[tauri::command]
fn open_accessibility_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .status()
            .map_err(|e| format!("failed to open Accessibility settings: {e}"))?;
        return Ok(());
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Not supported on this platform".to_string())
    }
}

#[tauri::command]
fn open_automation_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Automation")
            .status()
            .map_err(|e| format!("failed to open Automation settings: {e}"))?;
        return Ok(());
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Not supported on this platform".to_string())
    }
}

#[tauri::command]
fn sync_now(app: tauri::AppHandle) -> Result<SyncNowResult, String> {
    let imported = sync::import_now(&app)?;
    // Export after importing to propagate merged state.
    sync::export_now(&app)?;
    Ok(SyncNowResult { imported })
}

fn register_hotkey(app: &tauri::AppHandle, hotkey: &str) -> Result<(), String> {
    // Keep behavior simple: only one global shortcut is active at a time.
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| format!("failed to unregister hotkeys: {e}"))?;

    app.global_shortcut()
        .on_shortcut(hotkey.trim(), move |app, _shortcut, event| {
            if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                // IMPORTANT (macOS): This callback may come from an OS event handler where
                // unwinding is not allowed. Never let a panic escape, and do UI work on
                // the main thread.
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    eprintln!("[powerpaste] hotkey pressed");

                    let app_handle = app.clone();
                    let app_handle_for_task = app_handle.clone();
                    let _ = app_handle.run_on_main_thread(move || {
                        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            if let Err(e) = toggle_main_window(&app_handle_for_task) {
                                eprintln!("[powerpaste] hotkey toggle failed: {e}");
                            }
                        }));
                    });
                }));
            }
        })
        .map_err(|e| format!("failed to register hotkey '{}': {e}", hotkey.trim()))?;

    // Best-effort verification (helps debug when the OS rejects or blocks shortcuts).
    let hk = hotkey.trim();
    let registered = app.global_shortcut().is_registered(hk);
    eprintln!("[powerpaste] hotkey registered={registered} ({hk})");

    Ok(())
}

fn toggle_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<(), String> {
    // Load UI mode from settings before toggling
    if let Ok(settings) = settings_store::get(app) {
        set_current_ui_mode(settings.ui_mode);
    }
    
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;

    // macOS fullscreen Spaces: a normal NSWindow often can't overlay another app's
    // fullscreen space, even at very high window levels. We use a native NSPanel
    // overlay instead.
    #[cfg(target_os = "macos")]
    {
        return macos_toggle_overlay_panel(&window);
    }

    #[cfg(not(target_os = "macos"))]
    {
        let visible = window
            .is_visible()
            .map_err(|e| format!("failed to check window visibility: {e}"))?;

        let minimized = window
            .is_minimized()
            .map_err(|e| format!("failed to check window minimized state: {e}"))?;

        if visible && !minimized {
            window.hide().map_err(|e| format!("failed to hide window: {e}"))?;
            return Ok(());
        }

        position_as_bottom_overlay(&window)?;
        if minimized {
            let _ = window.unminimize();
        }
        window.show().map_err(|e| format!("failed to show window: {e}"))?;
        window
            .set_focus()
            .map_err(|e| format!("failed to focus window: {e}"))?;
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn macos_toggle_overlay_panel<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) -> Result<(), String> {
    use objc2::exception;
    use objc2::rc::Retained;
    use objc2::MainThreadMarker;
    use objc2::MainThreadOnly;
    use objc2_app_kit::{
        NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSPanel, NSScreen,
        NSScreenSaverWindowLevel, NSView, NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask,
    };
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    use std::sync::atomic::Ordering;

    fn try_get_retained_ns_window<R2: tauri::Runtime>(
        window: &tauri::WebviewWindow<R2>,
    ) -> Result<Option<Retained<NSWindow>>, String> {
        let ptr_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| window.ns_window()));
        match ptr_result {
            Ok(Ok(ptr)) => {
                if ptr.is_null() {
                    return Ok(None);
                }
                // SAFETY: When non-null, Tauri returns a valid NSWindow pointer for this window.
                let retained = unsafe { Retained::retain(ptr.cast()) };
                Ok(retained)
            }
            Ok(Err(e)) => {
                eprintln!("[powerpaste] ns_window not available yet: {e}");
                Ok(None)
            }
            Err(_) => {
                eprintln!("[powerpaste] ns_window panicked (window likely not realized yet)");
                Ok(None)
            }
        }
    }

    let mtm = MainThreadMarker::new().ok_or("not on main thread")?;

    // If the panel is already created, NEVER call `window.ns_window()` again.
    // We reparent the original window's contentView into the panel; after that,
    // tao/Tauri may consider the underlying NSView missing and panic.
    if let Some(stored) = OVERLAY_PANEL_PTR.get() {
        // SAFETY: We store a leaked, valid NSPanel pointer (as usize). We only use it on main thread.
        let panel: Retained<NSPanel> = unsafe {
            Retained::retain((*stored as *mut NSPanel).cast()).ok_or("failed to retain NSPanel")?
        };

        let is_visible = panel.isVisible();
        if is_visible {
            exception::catch(std::panic::AssertUnwindSafe(|| {
                panel.orderOut(None);
            }))
            .map_err(|e| format!("objective-c exception hiding panel: {e:?}"))?;

            // Remove monitors when hiding
            macos_remove_keyboard_monitor();
            macos_remove_click_outside_monitor();

            // Restore agent/app-less behavior when hidden.
            let app = NSApplication::sharedApplication(mtm);
            let _ = app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
            eprintln!("[powerpaste] macos overlay panel hide");
            #[cfg(desktop)]
            append_debug_log("[powerpaste] macos overlay panel hide");
            return Ok(());
        }

        // Snapshot the current frontmost app before we activate ourselves.
        // This lets us restore focus when the user chooses an item to paste.
        if let Some(name) = macos_query_frontmost_app_name() {
            if name != "PowerPaste" {
                macos_set_last_frontmost_app_name(name);
            }
        }

        // Show: activate app (helps on some systems), then order front.
        let app = NSApplication::sharedApplication(mtm);
        let _ = app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);

        // Recompute frame each show (handles display changes, fullscreen spaces, etc.).
        let screen = panel
            .screen()
            .or_else(|| NSScreen::mainScreen(mtm))
            .ok_or("no screen found")?;
        let screen_frame: NSRect = screen.frame();
        let (w, h) = overlay_size_for_monitor(
            screen_frame.size.width.max(1.0).round() as u32,
            screen_frame.size.height.max(1.0).round() as u32,
        );
        let mut target = screen_frame;
        target.size.width = (w as f64).min(screen_frame.size.width);
        target.size.height = (h as f64).min(screen_frame.size.height);
        
        // Position based on UI mode
        let ui_mode = get_current_ui_mode();
        match ui_mode {
            models::UiMode::Floating => {
                // Position near cursor
                if let Some((cursor_x, cursor_y)) = macos_get_cursor_position() {
                    // Center horizontally on cursor, position above cursor
                    let half_width = target.size.width / 2.0;
                    let mut x = cursor_x - half_width;
                    let mut y = cursor_y - target.size.height - 10.0; // 10px above cursor
                    
                    // Clamp to screen bounds
                    x = x.max(screen_frame.origin.x).min(screen_frame.origin.x + screen_frame.size.width - target.size.width);
                    y = y.max(screen_frame.origin.y).min(screen_frame.origin.y + screen_frame.size.height - target.size.height);
                    
                    target.origin.x = x;
                    target.origin.y = y;
                } else {
                    // Fallback to center-bottom if cursor position unavailable
                    target.origin.x = screen_frame.origin.x + (screen_frame.size.width - target.size.width) / 2.0;
                    target.origin.y = screen_frame.origin.y;
                }
            }
            models::UiMode::Fixed => {
                // Fixed at bottom, 90% screen width, centered
                target.size.width = screen_frame.size.width * 0.9;
                target.origin.x = screen_frame.origin.x + (screen_frame.size.width - target.size.width) / 2.0;
                target.origin.y = screen_frame.origin.y;
            }
        }

        exception::catch(std::panic::AssertUnwindSafe(|| {
            panel.setLevel(NSScreenSaverWindowLevel);
            panel.setFrame_display(target, true);
            panel.orderFrontRegardless();
            panel.makeKeyWindow();
        }))
        .map_err(|e| format!("objective-c exception showing panel: {e:?}"))?;

        let level = panel.level();
        let frame = panel.frame();
        eprintln!("[powerpaste] macos overlay panel show level={level}");
        #[cfg(desktop)]
        append_debug_log(&format!("[powerpaste] macos overlay panel show level={level}"));
        eprintln!(
            "[powerpaste] macos panel.frame x={} y={} w={} h={}",
            frame.origin.x,
            frame.origin.y,
            frame.size.width,
            frame.size.height
        );

        // Install keyboard monitor to capture Cmd+A/C in the overlay
        macos_install_keyboard_monitor(window.app_handle().clone());
        // Install click-outside monitor to hide when clicking elsewhere
        macos_install_click_outside_monitor(window.app_handle().clone());

        return Ok(());
    }

    // If the webview isn't realized yet (common when we start hidden), calling
    // Tauri's `ns_window()` may panic inside tao when `ns_view` is None.
    // Avoid crashing: realize the window, then retry once shortly after.
    let ns_window = match try_get_retained_ns_window(window)? {
        Some(w) => w,
        None => {
            let _ = window.show();
            let _ = window.unminimize();

            if !PANEL_INIT_RETRY_SCHEDULED.swap(true, Ordering::SeqCst) {
                let window_for_retry = window.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(120));
                    let app = window_for_retry.app_handle();
                    let window_for_task = window_for_retry.clone();
                    let _ = app.run_on_main_thread(move || {
                        let _ = macos_toggle_overlay_panel(&window_for_task);
                        PANEL_INIT_RETRY_SCHEDULED.store(false, Ordering::SeqCst);
                    });
                });
            }

            eprintln!("[powerpaste] overlay not ready; realizing window and retrying");
            return Ok(());
        }
    };

    // Create the panel using the screen's full frame, then move the existing
    // contentView (WKWebView) into it.
    let screen = ns_window
        .screen()
        .or_else(|| NSScreen::mainScreen(mtm))
        .ok_or("no screen found")?;

    let screen_frame: NSRect = screen.frame();
    let (w, h) = overlay_size_for_monitor(
        screen_frame.size.width.max(1.0).round() as u32,
        screen_frame.size.height.max(1.0).round() as u32,
    );
    let mut target = screen_frame;
    target.size.width = (w as f64).min(screen_frame.size.width);
    target.size.height = (h as f64).min(screen_frame.size.height);
    
    // Position based on UI mode
    let ui_mode = get_current_ui_mode();
    match ui_mode {
        models::UiMode::Floating => {
            // Position near cursor
            if let Some((cursor_x, cursor_y)) = macos_get_cursor_position() {
                let half_width = target.size.width / 2.0;
                let mut x = cursor_x - half_width;
                let mut y = cursor_y - target.size.height - 10.0;
                
                x = x.max(screen_frame.origin.x).min(screen_frame.origin.x + screen_frame.size.width - target.size.width);
                y = y.max(screen_frame.origin.y).min(screen_frame.origin.y + screen_frame.size.height - target.size.height);
                
                target.origin.x = x;
                target.origin.y = y;
            } else {
                target.origin.x = screen_frame.origin.x + (screen_frame.size.width - target.size.width) / 2.0;
                target.origin.y = screen_frame.origin.y;
            }
        }
        models::UiMode::Fixed => {
            // Fixed at bottom, 90% screen width, centered
            target.size.width = screen_frame.size.width * 0.9;
            target.origin.x = screen_frame.origin.x + (screen_frame.size.width - target.size.width) / 2.0;
            target.origin.y = screen_frame.origin.y;
        }
    }

    let style = NSWindowStyleMask::Borderless
        | NSWindowStyleMask::NonactivatingPanel
        | NSWindowStyleMask::FullSizeContentView;

    let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
        NSPanel::alloc(mtm),
        target,
        style,
        NSBackingStoreType::Buffered,
        false,
    );

    // Move the WebView content view from the original NSWindow into this panel.
    // IMPORTANT: keep a placeholder contentView in the original NSWindow; tao/Tauri
    // may panic later if the backing NSView disappears.
    let content_view = ns_window
        .contentView()
        .ok_or("ns_window.contentView was None")?;

    let placeholder_frame = NSRect {
        origin: NSPoint { x: 0.0, y: 0.0 },
        size: NSSize {
            width: 1.0,
            height: 1.0,
        },
    };
    let placeholder = NSView::initWithFrame(NSView::alloc(mtm), placeholder_frame);
    ns_window.setContentView(Some(&placeholder));
    panel.setContentView(Some(&content_view));

    // Set rounded corners on the panel window itself
    unsafe {
        use objc2::runtime::AnyObject;
        use objc2_app_kit::NSColor;
        
        // Make the panel background transparent so corners show
        panel.setOpaque(false);
        panel.setBackgroundColor(Some(&NSColor::clearColor()));
        
        // Get the panel's contentView (the webview we just set)
        if let Some(view) = panel.contentView() {
            // Enable layer-backing on the view
            let _: () = objc2::msg_send![&*view, setWantsLayer: true];
            
            // Get the layer and set corner radius
            let layer: *mut AnyObject = objc2::msg_send![&*view, layer];
            if !layer.is_null() {
                let corner_radius = 18.0f64;
                let _: () = objc2::msg_send![layer, setCornerRadius: corner_radius];
                let _: () = objc2::msg_send![layer, setMasksToBounds: true];
                
                eprintln!("[powerpaste] set corner radius on panel content view layer");
            }
            
            // Helper to recursively find WKWebView and set transparent background
            fn find_and_configure_webview(view: *mut AnyObject, depth: usize) {
                if view.is_null() || depth > 10 {
                    return;
                }
                
                unsafe {
                    use objc2::runtime::AnyObject;
                    
                    // Get class name
                    let class_name: *const AnyObject = objc2::msg_send![view, className];
                    if class_name.is_null() {
                        return;
                    }
                    let class_str: *const std::ffi::c_char = objc2::msg_send![class_name, UTF8String];
                    if class_str.is_null() {
                        return;
                    }
                    let class_cstr = std::ffi::CStr::from_ptr(class_str);
                    let class_name_str = class_cstr.to_string_lossy();
                    
                    let indent = "  ".repeat(depth);
                    eprintln!("[powerpaste] {}view class: {}", indent, class_name_str);
                    
                    // If this is a WKWebView, configure it for transparency
                    if class_name_str == "WKWebView" {
                        eprintln!("[powerpaste] {}Found WKWebView! Configuring transparency...", indent);
                        
                        // Set drawsBackground to false (private API)
                        let _: () = objc2::msg_send![view, _setDrawsBackground: false];
                        
                        // Set layer corner radius
                        let _: () = objc2::msg_send![view, setWantsLayer: true];
                        let wk_layer: *mut AnyObject = objc2::msg_send![view, layer];
                        if !wk_layer.is_null() {
                            let _: () = objc2::msg_send![wk_layer, setCornerRadius: 18.0f64];
                            let _: () = objc2::msg_send![wk_layer, setMasksToBounds: true];
                        }
                        
                        eprintln!("[powerpaste] {}WKWebView configured", indent);
                    }
                    
                    // Also set layer on WryWebViewParent
                    if class_name_str == "WryWebViewParent" {
                        let _: () = objc2::msg_send![view, setWantsLayer: true];
                        let parent_layer: *mut AnyObject = objc2::msg_send![view, layer];
                        if !parent_layer.is_null() {
                            let _: () = objc2::msg_send![parent_layer, setCornerRadius: 18.0f64];
                            let _: () = objc2::msg_send![parent_layer, setMasksToBounds: true];
                        }
                    }
                    
                    // Recursively check subviews
                    let subviews: *const AnyObject = objc2::msg_send![view, subviews];
                    if !subviews.is_null() {
                        let count: usize = objc2::msg_send![subviews, count];
                        for i in 0..count {
                            let subview: *mut AnyObject = objc2::msg_send![subviews, objectAtIndex: i];
                            find_and_configure_webview(subview, depth + 1);
                        }
                    }
                }
            }
            
            // Start recursive search from content view
            find_and_configure_webview(&*view as *const _ as *mut AnyObject, 0);
        }
    }

    // Configure behavior to appear above fullscreen spaces.
    let current = panel.collectionBehavior();
    let next = current
        | NSWindowCollectionBehavior::CanJoinAllSpaces
        | NSWindowCollectionBehavior::CanJoinAllApplications
        | NSWindowCollectionBehavior::FullScreenAuxiliary
        | NSWindowCollectionBehavior::Transient
        | NSWindowCollectionBehavior::Stationary
        | NSWindowCollectionBehavior::IgnoresCycle;
    panel.setCollectionBehavior(next);

    panel.setLevel(NSScreenSaverWindowLevel);
    panel.setHidesOnDeactivate(false);
    panel.setIgnoresMouseEvents(false);
    panel.setFloatingPanel(true);
    panel.setBecomesKeyOnlyIfNeeded(false);

    // Keep a retained pointer around for future toggles.
    let raw = Retained::as_ptr(&panel) as usize;
    let _ = OVERLAY_PANEL_PTR.set(raw);

    // First show.
    let app = NSApplication::sharedApplication(mtm);
    let _ = app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    if let Some(name) = macos_query_frontmost_app_name() {
        if name != "PowerPaste" {
            macos_set_last_frontmost_app_name(name);
        }
    }
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);

    exception::catch(std::panic::AssertUnwindSafe(|| {
        panel.setLevel(NSScreenSaverWindowLevel);
        panel.setFrame_display(target, true);
        panel.orderFrontRegardless();
        panel.makeKeyWindow();
    }))
    .map_err(|e| format!("objective-c exception showing panel: {e:?}"))?;

    let level = panel.level();
    let frame = panel.frame();
    eprintln!("[powerpaste] macos overlay panel show level={level}");
    eprintln!(
        "[powerpaste] macos panel.frame x={} y={} w={} h={}",
        frame.origin.x,
        frame.origin.y,
        frame.size.width,
        frame.size.height
    );

    // Install keyboard monitor to capture Cmd+A/C in the overlay
    macos_install_keyboard_monitor(window.app_handle().clone());
    // Install click-outside monitor to hide when clicking elsewhere
    macos_install_click_outside_monitor(window.app_handle().clone());

    // Leak the panel so it remains valid even after being hidden.
    std::mem::forget(panel);
    Ok(())
}

#[cfg_attr(target_os = "macos", allow(dead_code))]
fn position_as_bottom_overlay<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) -> Result<(), String> {
    window
        .set_always_on_top(true)
        .map_err(|e| format!("failed to set always-on-top: {e}"))?;

    // Helps the overlay appear above full-screen/other spaces on macOS.
    let _ = window.set_visible_on_all_workspaces(true);

    #[cfg(target_os = "macos")]
    {
        if let Err(e) = macos_configure_overlay_window(window) {
            eprintln!("[powerpaste] macos overlay config failed: {e}");
        }
    }

    let monitor = window
        .current_monitor()
        .map_err(|e| format!("failed to get current monitor: {e}"))?
        .or_else(|| {
            window
                .primary_monitor()
                .ok()
                .flatten()
        })
        .ok_or_else(|| "no monitor found".to_string())?;

    // Use the full monitor bounds so the overlay can cover the Dock/taskbar when needed.
    let size = monitor.size();
    let pos = monitor.position();

    let (width, height) = overlay_size_for_monitor(size.width, size.height);
    let x = pos.x + ((size.width.saturating_sub(width)) / 2) as i32;
    let y = pos.y + (size.height.saturating_sub(height)) as i32;

    window
        .set_size(tauri::Size::Physical(tauri::PhysicalSize { width, height }))
        .map_err(|e| format!("failed to set window size: {e}"))?;
    window
        .set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }))
        .map_err(|e| format!("failed to set window position: {e}"))?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn macos_configure_overlay_window<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) -> Result<(), String> {
    use objc2::rc::Retained;
    use objc2::exception;
    use objc2_app_kit::{
        NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask,
    };

    let ptr_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| window.ns_window()));
    let ptr = match ptr_result {
        Ok(Ok(ptr)) if !ptr.is_null() => ptr,
        Ok(Ok(_)) => return Err("ns_window was null".to_string()),
        Ok(Err(e)) => return Err(format!("failed to get ns_window: {e}")),
        Err(_) => return Err("ns_window panicked (window likely not realized yet)".to_string()),
    };

    // SAFETY: Tauri returns a valid NSWindow pointer for this window.
    let ns_window: Retained<NSWindow> = unsafe { Retained::retain(ptr.cast()).ok_or("failed to retain NSWindow")? };

    // Some AppKit calls can raise Objective-C exceptions. Catch them so they don't
    // unwind into Rust (which would abort the process).
    exception::catch(std::panic::AssertUnwindSafe(|| {
        let current = ns_window.collectionBehavior();
        let next = current
            | NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::CanJoinAllApplications
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::Transient
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::IgnoresCycle;
        ns_window.setCollectionBehavior(next);

        // Try to opt into AppKit's "panel" behavior even though we started from an NSWindow.
        // This can affect how the window participates in fullscreen Spaces.
        let style = ns_window.styleMask();
        let panel_style = (style | NSWindowStyleMask::FullSizeContentView)
            & !NSWindowStyleMask::Titled;
        ns_window.setStyleMask(panel_style);
    }))
    .map_err(|e| format!("objective-c exception configuring window: {e:?}"))?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn macos_set_overlay_window_active<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
    active: bool,
) -> Result<(), String> {
    use objc2::rc::Retained;
    use objc2::exception;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{
        NSApplication, NSApplicationActivationPolicy, NSScreen, NSScreenSaverWindowLevel,
        NSPopUpMenuWindowLevel, NSNormalWindowLevel, NSWindow,
    };
    use objc2_foundation::NSRect;

    let ptr_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| window.ns_window()));
    let ptr = match ptr_result {
        Ok(Ok(ptr)) if !ptr.is_null() => ptr,
        Ok(Ok(_)) => return Err("ns_window was null".to_string()),
        Ok(Err(e)) => return Err(format!("failed to get ns_window: {e}")),
        Err(_) => return Err("ns_window panicked (window likely not realized yet)".to_string()),
    };

    // SAFETY: Tauri returns a valid NSWindow pointer for this window.
    let ns_window: Retained<NSWindow> = unsafe { Retained::retain(ptr.cast()).ok_or("failed to retain NSWindow")? };

    // Ensure collection behavior is set even if this is called before first show.
    // (We keep configuration separate from level changes so we can restore the level on hide.)
    // Best-effort: don't fail just because configuring behavior fails.
    let _ = macos_configure_overlay_window(window);

    exception::catch(std::panic::AssertUnwindSafe(|| {
        if active {
            // This is the most reliable way to cover the Dock and appear above fullscreen Spaces.
            // We only apply it while the overlay is visible.
            ns_window.setLevel(NSScreenSaverWindowLevel);

            // Keep visible even when our app isn't active (e.g., user is in another app's fullscreen).
            ns_window.setHidesOnDeactivate(false);
            ns_window.setIgnoresMouseEvents(false);

            // If the requested level doesn't stick (some systems restrict it), fall back to
            // a high but more conventional level.
            let current = ns_window.level();
            if current < NSScreenSaverWindowLevel {
                ns_window.setLevel(NSPopUpMenuWindowLevel);
            }

            // Force the frame using the full screen frame (not visibleFrame) so it can
            // extend into the Dock area.
            if let Some(mtm) = MainThreadMarker::new() {
                // Snapshot the current frontmost app before we activate ourselves.
                // This lets us restore focus when the user chooses an item to paste.
                if let Some(name) = macos_query_frontmost_app_name() {
                    if name != "PowerPaste" {
                        macos_set_last_frontmost_app_name(name);
                    }
                }

                // In practice, showing above another app's fullscreen Space is much more
                // reliable when our app is activated.
                let app = NSApplication::sharedApplication(mtm);

                // Try to behave like an "agent" app (Paste-like). This can influence
                // how windows participate in fullscreen spaces.
                let _ = app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

                #[allow(deprecated)]
                app.activateIgnoringOtherApps(true);

                let screen = ns_window
                    .screen()
                    .or_else(|| NSScreen::mainScreen(mtm));

                if let Some(screen) = screen {
                    let screen_frame: NSRect = screen.frame();
                    let (w, h) = overlay_size_for_monitor(
                        screen_frame.size.width.max(1.0).round() as u32,
                        screen_frame.size.height.max(1.0).round() as u32,
                    );
                    let mut target = screen_frame;
                    target.size.width = (w as f64).min(screen_frame.size.width);
                    target.size.height = (h as f64).min(screen_frame.size.height);
                    target.origin.x = screen_frame.origin.x + (screen_frame.size.width - target.size.width) / 2.0;
                    target.origin.y = screen_frame.origin.y;
                    ns_window.setFrame_display(target, true);
                }
            }

            // Ensure the window is ordered above everything else at its level,
            // and becomes key so it receives keyboard events.
            ns_window.makeKeyAndOrderFront(None);
        } else {
            ns_window.setLevel(NSNormalWindowLevel);
        }
    }))
    .map_err(|e| format!("objective-c exception setting window level: {e:?}"))?;

    // When hidden, revert to Accessory so we don't stick in the Dock/menubar.
    if !active {
        if let Some(mtm) = MainThreadMarker::new() {
            let app = NSApplication::sharedApplication(mtm);
            let _ = app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        }
    }

    // Helpful diagnostics when tuning macOS behavior.
    let level = ns_window.level();
    let frame = ns_window.frame();
    eprintln!("[powerpaste] macos overlay active={active} ns_window.level={level}");
    eprintln!(
        "[powerpaste] macos ns_window.frame x={} y={} w={} h={}",
        frame.origin.x,
        frame.origin.y,
        frame.size.width,
        frame.size.height
    );

    Ok(())
}

#[cfg(desktop)]
fn setup_tray<R: tauri::Runtime, M: tauri::Manager<R>>(manager: &M) -> Result<(), String> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let toggle_item = MenuItem::with_id(manager, "tray_toggle", "Show/Hide", true, None::<String>)
        .map_err(|e| format!("failed to create tray menu item: {e}"))?;
    let quit_item = MenuItem::with_id(manager, "tray_quit", "Exit", true, None::<String>)
        .map_err(|e| format!("failed to create tray menu item: {e}"))?;
    let sep = PredefinedMenuItem::separator(manager)
        .map_err(|e| format!("failed to create tray menu separator: {e}"))?;

    let menu = Menu::with_items(manager, &[&toggle_item, &sep, &quit_item])
        .map_err(|e| format!("failed to create tray menu: {e}"))?;

    let mut builder = TrayIconBuilder::with_id("powerpaste_tray")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "tray_toggle" => {
                let _ = toggle_main_window(app);
            }
            "tray_quit" => {
                let state: tauri::State<'_, AppState> = app.state();
                if let Ok(guard) = state.watcher.lock() {
                    if let Some(watcher) = guard.as_ref() {
                        watcher.stop();
                    }
                }
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                if let Err(e) = toggle_main_window(tray.app_handle()) {
                    eprintln!("[powerpaste] tray toggle failed: {e}");
                }
            }
        })
        .tooltip("PowerPaste");

    if let Some(icon) = manager.app_handle().default_window_icon() {
        builder = builder.icon(icon.clone()).icon_as_template(true);
    }

    builder
        .build(manager)
        .map_err(|e| format!("failed to build tray icon: {e}"))?;
    Ok(())
}

#[cfg(desktop)]
fn setup_app_menu<R: tauri::Runtime, M: tauri::Manager<R>>(manager: &M) -> Result<(), String> {
    use tauri::menu::{Menu, PredefinedMenuItem, Submenu};
    
    // Build a custom menu with our own Edit submenu that has custom Select All / Copy items.
    // These items trigger our on_menu_event handler, which emits events to the frontend.
    // This ensures Cmd+A/C work even when the NSPanel overlay is active.
    
    let app_handle = manager.app_handle();
    
    // App submenu (About, Quit, etc.)
    let about = PredefinedMenuItem::about(app_handle, Some("About PowerPaste"), None)
        .map_err(|e| format!("failed to create About menu: {e}"))?;
    let separator = PredefinedMenuItem::separator(app_handle)
        .map_err(|e| format!("failed to create separator: {e}"))?;
    let hide = PredefinedMenuItem::hide(app_handle, Some("Hide PowerPaste"))
        .map_err(|e| format!("failed to create Hide menu: {e}"))?;
    let hide_others = PredefinedMenuItem::hide_others(app_handle, Some("Hide Others"))
        .map_err(|e| format!("failed to create Hide Others menu: {e}"))?;
    let show_all = PredefinedMenuItem::show_all(app_handle, Some("Show All"))
        .map_err(|e| format!("failed to create Show All menu: {e}"))?;
    let quit = PredefinedMenuItem::quit(app_handle, Some("Quit PowerPaste"))
        .map_err(|e| format!("failed to create Quit menu: {e}"))?;
    
    let app_menu = Submenu::with_items(
        app_handle,
        "PowerPaste",
        true,
        &[&about, &separator, &hide, &hide_others, &show_all, &separator, &quit],
    )
    .map_err(|e| format!("failed to create app submenu: {e}"))?;
    
    // Edit submenu with custom Select All and Copy items that we handle ourselves.
    // Using our custom menu item IDs so on_menu_event can forward them to the frontend.
    let edit_separator = PredefinedMenuItem::separator(app_handle)
        .map_err(|e| format!("failed to create separator: {e}"))?;
    
    // Undo/Redo/Cut/Paste use predefined items (they work with text fields).
    let undo = PredefinedMenuItem::undo(app_handle, Some("Undo"))
        .map_err(|e| format!("failed to create Undo menu: {e}"))?;
    let redo = PredefinedMenuItem::redo(app_handle, Some("Redo"))
        .map_err(|e| format!("failed to create Redo menu: {e}"))?;
    let cut = PredefinedMenuItem::cut(app_handle, Some("Cut"))
        .map_err(|e| format!("failed to create Cut menu: {e}"))?;
    let paste = PredefinedMenuItem::paste(app_handle, Some("Paste"))
        .map_err(|e| format!("failed to create Paste menu: {e}"))?;
    
    // IMPORTANT (macOS): Cmd+A/C are routed through native Edit menu actions.
    // Use predefined (native) items so the OS reliably dispatches them.
    // We still forward the resulting menu events to the frontend in `.on_menu_event`.
    let copy_item = PredefinedMenuItem::copy(app_handle, Some("Copy"))
        .map_err(|e| format!("failed to create Copy menu: {e}"))?;

    let select_all_item = PredefinedMenuItem::select_all(app_handle, Some("Select All"))
        .map_err(|e| format!("failed to create Select All menu: {e}"))?;
    
    let edit_menu = Submenu::with_items(
        app_handle,
        "Edit",
        true,
        &[&undo, &redo, &edit_separator, &cut, &copy_item, &paste, &edit_separator, &select_all_item],
    )
    .map_err(|e| format!("failed to create edit submenu: {e}"))?;
    
    // Window submenu (Minimize, Zoom, etc.)
    let minimize = PredefinedMenuItem::minimize(app_handle, Some("Minimize"))
        .map_err(|e| format!("failed to create Minimize menu: {e}"))?;
    let zoom = PredefinedMenuItem::maximize(app_handle, Some("Zoom"))
        .map_err(|e| format!("failed to create Zoom menu: {e}"))?;
    let close = PredefinedMenuItem::close_window(app_handle, Some("Close"))
        .map_err(|e| format!("failed to create Close menu: {e}"))?;
    
    let window_menu = Submenu::with_items(
        app_handle,
        "Window",
        true,
        &[&minimize, &zoom, &close],
    )
    .map_err(|e| format!("failed to create window submenu: {e}"))?;
    
    let menu = Menu::with_items(app_handle, &[&app_menu, &edit_menu, &window_menu])
        .map_err(|e| format!("failed to build app menu: {e}"))?;

    let _previous = menu
        .set_as_app_menu()
        .map_err(|e| format!("failed to set app menu: {e}"))?;

    Ok(())
}

#[cfg(desktop)]
fn debug_log_menu_event_id(id: &str) {
    // Always log menu events for debugging.
    eprintln!("[powerpaste] menu event id={id}");
    append_debug_log(&format!("[powerpaste] menu event id={id}"));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            watcher: Mutex::new(None),
        })
        .setup(|app| {
            let handle = app.handle().clone();

            // CRITICAL: Set activation policy BEFORE any windows are shown.
            // This ensures macOS doesn't show the dock icon on launch.
            #[cfg(target_os = "macos")]
            {
                use objc2::MainThreadMarker;
                use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
                
                if let Some(mtm) = MainThreadMarker::new() {
                    let ns_app = NSApplication::sharedApplication(mtm);
                    // Default to Accessory (no dock icon). Will be changed if show_dock_icon is true.
                    let _ = ns_app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
                }
            }

            #[cfg(desktop)]
            {
                if let Some(path) = debug_log_path() {
                    append_debug_log(&format!("[powerpaste] started (debug log: {})", path.display()));
                } else {
                    append_debug_log("[powerpaste] started (debug log: <unknown>)");
                }
            }

            // Initialize settings early.
            if let Ok(settings) = settings_store::load_or_init_settings(&handle) {
                if let Err(e) = register_hotkey(&handle, &settings.hotkey) {
                    eprintln!("[powerpaste] failed to register hotkey '{}': {e}", settings.hotkey);
                }
                
                // Apply saved dock icon preference (macOS only).
                #[cfg(target_os = "macos")]
                {
                    if settings.show_dock_icon {
                        apply_dock_icon_visibility(true);
                    }
                }
            }

            // Start hidden; the global hotkey toggles the UI.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();

                // Open DevTools if POWERPASTE_DEVTOOLS_PORT is set (for E2E testing with Playwright).
                // The port value itself is currently not used by Tauri's WebView, but this env var
                // signals that we're in a test environment and should open DevTools.
                #[cfg(debug_assertions)]
                {
                    if std::env::var("POWERPASTE_DEVTOOLS_PORT").is_ok() {
                        eprintln!("[powerpaste] opening devtools for E2E testing");
                        window.open_devtools();
                    }
                }

                let window_for_event = window.clone();
                window.on_window_event(move |event| {
                    match event {
                        tauri::WindowEvent::CloseRequested { api, .. } => {
                            api.prevent_close();
                            #[cfg(target_os = "macos")]
                            {
                                if let Err(e) = macos_set_overlay_window_active(&window_for_event, false) {
                                    eprintln!("[powerpaste] macos overlay deactivate failed: {e}");
                                }
                            }
                            let _ = window_for_event.hide();
                        }
                        tauri::WindowEvent::Focused(false) => {
                            // Hide when clicking outside the window
                            #[cfg(target_os = "macos")]
                            {
                                if let Err(e) = macos_set_overlay_window_active(&window_for_event, false) {
                                    eprintln!("[powerpaste] macos overlay deactivate failed: {e}");
                                }
                            }
                            let _ = window_for_event.hide();
                        }
                        _ => {}
                    }
                });
            }

            #[cfg(desktop)]
            {
                let _ = setup_tray(app);
                if let Err(e) = setup_app_menu(app) {
                    eprintln!("[powerpaste] failed to set up app menu shortcuts: {e}");
                }
            }

            // Start clipboard watcher.
            let watcher = clipboard::ClipboardWatcher::start(handle.clone());
            let state: tauri::State<'_, AppState> = app.state();
            let mut guard = state.watcher.lock().unwrap_or_else(|e| e.into_inner());
            *guard = Some(watcher);

            // Periodic sync loop (folder-based; works with iCloud Drive / OneDrive / Google Drive folders).
            std::thread::spawn(move || loop {
                // Import then export; ignore errors (UI has manual sync).
                let _ = sync::import_now(&handle);
                let _ = sync::export_now(&handle);
                std::thread::sleep(std::time::Duration::from_secs(15));
            });

            Ok(())
        })
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            #[cfg(desktop)]
            debug_log_menu_event_id(id);

            // macOS note: the default menu includes Cmd+A (Select All) and Cmd+C (Copy).
            // We forward those to the frontend so the app works even when the WebView
            // doesn't receive key events.
            match id {
                MENU_ID_SELECT_ALL | "select_all" | "selectall" => {
                    eprintln!("[powerpaste] menu shortcut: select_all (id={id})");
                    #[cfg(desktop)]
                    append_debug_log(&format!("[powerpaste] menu shortcut: select_all (id={id})"));
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.emit(FRONTEND_EVENT_SELECT_ALL, ());
                    }
                }
                MENU_ID_COPY_SELECTED | "copy" => {
                    eprintln!("[powerpaste] menu shortcut: copy (id={id})");
                    #[cfg(desktop)]
                    append_debug_log(&format!("[powerpaste] menu shortcut: copy (id={id})"));
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.emit(FRONTEND_EVENT_COPY_SELECTED, ());
                    }
                }
                _ => {}
            }
        })
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_settings,
            set_hotkey,
            set_sync_settings,
            set_ui_mode,
            list_items,
            set_overlay_preferred_size,
            hide_main_window,
            set_item_pinned,
            set_item_category,
            list_categories,
            delete_item,
            enable_mouse_events,
            write_clipboard_text,
            paste_text,
            check_permissions,
            open_accessibility_settings,
            open_automation_settings,
            sync_now,
            set_show_dock_icon
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
