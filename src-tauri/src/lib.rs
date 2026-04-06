mod clipboard;
mod db;
mod models;
mod paths;
pub(crate) mod platform;
mod settings_store;
mod sync;

use models::{ClipboardItem, ConnectedProviderInfo, Settings, SyncProvider};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::sync::OnceLock;
use tauri::Emitter;
use tauri::Manager;
use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use uuid::Uuid;

// tauri-nspanel imports for macOS panel support
#[cfg(target_os = "macos")]
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, ManagerExt as NspanelManagerExt, PanelLevel, StyleMask,
    WebviewWindowExt as NspanelWebviewWindowExt,
};

// Define the PowerPaste panel with tauri-nspanel
#[cfg(target_os = "macos")]
tauri_panel! {
    panel!(PowerPastePanel {
        config: {
            // Allow the panel to receive keyboard input (essential for search)
            can_become_key_window: true,
            // Don't make it the main window (we're a utility panel)
            can_become_main_window: false,
            // Floating panel stays above other windows
            is_floating_panel: true
        }
    })

    panel_event!(PowerPastePanelEventHandler {
        window_did_become_key(notification: &NSNotification) -> (),
        window_did_resign_key(notification: &NSNotification) -> ()
    })
}

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
// Uses screen fractions for responsive sizing across all display sizes.

// Fixed mode: horizontal strip at bottom (like Paste app)
const FIXED_WIDTH_FRACTION: f64 = 0.99;
const FIXED_HEIGHT_FRACTION: f64 = 0.23;

// Floating mode: narrow vertical popup near cursor
const FLOATING_WIDTH_PX: u32 = 320;      // Fixed width for consistent card layout
const FLOATING_HEIGHT_FRACTION: f64 = 0.50;  // Up to 50% of screen height
const FLOATING_MAX_HEIGHT_PX: u32 = 480;     // Cap max height

// Preferred overlay size, optionally set by the frontend at runtime.
static OVERLAY_PREFERRED_SIZE: OnceLock<Mutex<Option<(u32, u32)>>> = OnceLock::new();

fn set_overlay_preferred_size_global(w: u32, h: u32) {
    let cell = OVERLAY_PREFERRED_SIZE.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock().unwrap_or_else(|e| e.into_inner());
    *guard = Some((w, h));
}

#[allow(dead_code)]
fn get_overlay_preferred_size_global() -> Option<(u32, u32)> {
    let cell = OVERLAY_PREFERRED_SIZE.get_or_init(|| Mutex::new(None));
    let guard = cell.lock().unwrap_or_else(|e| e.into_inner());
    *guard
}

fn overlay_size_for_monitor(monitor_width: u32, monitor_height: u32, ui_mode: models::UiMode) -> (u32, u32) {
    match ui_mode {
        models::UiMode::Floating => {
            // Floating mode: narrow vertical popup
            let width = FLOATING_WIDTH_PX;
            let height = ((monitor_height as f64) * FLOATING_HEIGHT_FRACTION).round() as u32;
            let height = height.min(FLOATING_MAX_HEIGHT_PX);
            (width, height)
        }
        models::UiMode::Fixed => {
            // Fixed mode: wide horizontal strip at bottom
            let width = ((monitor_width as f64) * FIXED_WIDTH_FRACTION).round() as u32;
            let height = ((monitor_height as f64) * FIXED_HEIGHT_FRACTION).round() as u32;
            (width, height)
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) static OVERLAY_PANEL_PTR: OnceLock<usize> = OnceLock::new();

/// Stores the local keyboard event monitor object pointer (leaked).
/// We need to keep it alive while the panel is visible.
#[cfg(target_os = "macos")]
static KEYBOARD_MONITOR_PTR: OnceLock<Mutex<Option<usize>>> = OnceLock::new();

/// Stores the local mouse click monitor object pointer (leaked).
/// Used to handle left-clicks inside the panel to ensure webview focus.
#[cfg(target_os = "macos")]
static MOUSE_FOCUS_MONITOR_PTR: OnceLock<Mutex<Option<usize>>> = OnceLock::new();

/// Stores the global mouse click monitor object pointer (leaked).
/// Used to detect clicks outside the panel to hide it.
#[cfg(target_os = "macos")]
static CLICK_OUTSIDE_MONITOR_PTR: OnceLock<Mutex<Option<usize>>> = OnceLock::new();

#[cfg(target_os = "macos")]
#[allow(dead_code)]
static PANEL_INIT_RETRY_SCHEDULED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Explicit panel visibility state to avoid race conditions with window.is_visible().
/// Used by toggle logic to track whether the panel should be shown or hidden.
static IS_PANEL_VISIBLE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Generation counter to detect stale hide requests.
/// Incremented each time the panel is shown. Hide requests check this to avoid
/// hiding a panel that was shown after the hide was requested.
#[cfg(target_os = "macos")]
static PANEL_SHOW_GENERATION: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// Timestamp (in milliseconds) when the click-outside monitor was installed.
/// Used to ignore events that arrive immediately after installation (grace period).
#[cfg(target_os = "macos")]
#[allow(dead_code)]
static CLICK_MONITOR_INSTALL_TIME_MS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// Get the cursor position (delegates to platform module).
/// `macos_screen_containing_cursor` below still needs the macOS-specific return.

/// Find the screen that contains the mouse cursor.
#[cfg(target_os = "macos")]
#[allow(deprecated)]
fn macos_screen_containing_cursor(mtm: objc2::MainThreadMarker) -> Option<objc2::rc::Retained<objc2_app_kit::NSScreen>> {
    use objc2_app_kit::NSScreen;
    
    let (cursor_x, cursor_y) = platform::get_cursor_position()?;
    
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
#[allow(dead_code)]
fn macos_install_keyboard_monitor<R: tauri::Runtime>(_app_handle: tauri::AppHandle<R>) {
    // TEMPORARILY DISABLED to debug keyboard input issue
    eprintln!("[powerpaste] keyboard monitor DISABLED for debugging");
}

#[cfg(target_os = "macos")]
#[allow(dead_code)]
fn macos_install_keyboard_monitor_real<R: tauri::Runtime>(app_handle: tauri::AppHandle<R>) {
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
        
        // Debug: log all keydown events
        let chars = event_ref.charactersIgnoringModifiers();
        let key = chars.as_ref().map(|s| s.to_string()).unwrap_or_default();
        eprintln!("[powerpaste] keyboard event: key='{}' type={:?}", key, event_type);
        
        let modifiers = event_ref.modifierFlags();
        // Check for Command key (bit 20 = 0x100000)
        let has_cmd = modifiers.0 & (1 << 20) != 0;
        
        if !has_cmd {
            eprintln!("[powerpaste] passing through regular key: '{}'", key);
            return event.as_ptr(); // Pass through non-Cmd events
        }
        
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
pub(crate) fn macos_remove_keyboard_monitor() {
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

/// Install a local mouse event monitor to handle left-clicks inside the panel.
/// NSPanel with NonactivatingPanel style doesn't automatically give focus to
/// views on first click, so we need to manually make the content view first responder.
#[cfg(target_os = "macos")]
#[allow(dead_code)]
fn macos_install_mouse_focus_monitor() {
    use block2::StackBlock;
    use objc2::rc::Retained;
    use objc2_app_kit::{NSEvent, NSEventMask, NSPanel};
    use std::ptr::NonNull;

    // Check if already installed
    let cell = MOUSE_FOCUS_MONITOR_PTR.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_some() {
        return; // Already installed
    }

    eprintln!("[powerpaste] installing mouse focus monitor");

    // Create a block that handles local mouse click events
    let handler = StackBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        // Check if we have a panel
        let Some(stored) = OVERLAY_PANEL_PTR.get() else {
            return event.as_ptr();
        };
        
        // SAFETY: We store a valid NSPanel pointer
        let panel: Retained<NSPanel> = match unsafe {
            Retained::retain((*stored as *mut NSPanel).cast())
        } {
            Some(p) => p,
            None => return event.as_ptr(),
        };
        
        if !panel.isVisible() {
            return event.as_ptr();
        }
        
        // Make the panel key window and ensure webview has focus
        panel.makeKeyWindow();
        
        // Find and focus the WryWebView
        if let Some(content_view) = panel.contentView() {
            use objc2::runtime::AnyObject;
            fn find_wry_webview(view: *mut AnyObject, depth: usize) -> Option<*mut AnyObject> {
                if view.is_null() || depth > 10 { return None; }
                unsafe {
                    let class_name: *const AnyObject = objc2::msg_send![view, className];
                    if !class_name.is_null() {
                        let class_str: *const std::ffi::c_char = objc2::msg_send![class_name, UTF8String];
                        if !class_str.is_null() {
                            let name = std::ffi::CStr::from_ptr(class_str).to_string_lossy();
                            if name == "WryWebView" || name == "WKWebView" {
                                return Some(view);
                            }
                        }
                    }
                    let subviews: *const AnyObject = objc2::msg_send![view, subviews];
                    if !subviews.is_null() {
                        let count: usize = objc2::msg_send![subviews, count];
                        for i in 0..count {
                            let subview: *mut AnyObject = objc2::msg_send![subviews, objectAtIndex: i];
                            if let Some(wv) = find_wry_webview(subview, depth + 1) { return Some(wv); }
                        }
                    }
                    None
                }
            }
            let cv_ptr = &*content_view as *const _ as *mut AnyObject;
            if let Some(webview) = find_wry_webview(cv_ptr, 0) {
                unsafe {
                    let wv_view: *const AnyObject = webview as *const AnyObject;
                    let _: bool = objc2::msg_send![&*panel, makeFirstResponder: wv_view];
                }
            }
        }
        
        // Pass the event through
        event.as_ptr()
    });

    // Install the monitor for left mouse down events (local = inside our app)
    let monitor = unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(
            NSEventMask::LeftMouseDown,
            &handler,
        )
    };
    
    if let Some(m) = monitor {
        let ptr = Retained::into_raw(m) as usize;
        *guard = Some(ptr);
        eprintln!("[powerpaste] mouse focus monitor installed successfully");
    } else {
        eprintln!("[powerpaste] failed to install mouse focus monitor");
    }
}

/// Remove the local mouse focus monitor when panel is hidden.
#[cfg(target_os = "macos")]
pub(crate) fn macos_remove_mouse_focus_monitor() {
    use objc2_app_kit::NSEvent;

    let cell = MOUSE_FOCUS_MONITOR_PTR.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock().unwrap_or_else(|e| e.into_inner());
    
    if let Some(ptr) = guard.take() {
        eprintln!("[powerpaste] removing mouse focus monitor");
        unsafe {
            let monitor: *mut objc2::runtime::AnyObject = ptr as *mut _;
            NSEvent::removeMonitor(&*monitor);
        }
    }
}

/// Install a global mouse click monitor to detect clicks outside the panel.
/// NSPanel with NonactivatingPanel style doesn't trigger focus-lost events,
/// so we need to monitor global mouse clicks to hide the panel.
#[cfg(target_os = "macos")]
#[allow(dead_code)]
fn macos_install_click_outside_monitor<R: tauri::Runtime>(app_handle: tauri::AppHandle<R>) {
    use block2::StackBlock;
    use objc2::rc::Retained;
    use objc2_app_kit::{NSEvent, NSEventMask, NSPanel};
    use std::ptr::NonNull;
    use std::sync::atomic::Ordering;
    use std::time::{SystemTime, UNIX_EPOCH};

    // Check if already installed
    let cell = CLICK_OUTSIDE_MONITOR_PTR.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_some() {
        return; // Already installed
    }

    eprintln!("[powerpaste] installing click-outside monitor");

    // Record the install time so we can ignore events during the grace period
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    CLICK_MONITOR_INSTALL_TIME_MS.store(now_ms, Ordering::SeqCst);

    let app_for_block = app_handle.clone();
    
    // Create a block that handles global mouse click events
    // Global monitor receives NonNull<NSEvent> and returns nothing (void)
    let handler = StackBlock::new(move |event: NonNull<NSEvent>| {
        // Grace period: ignore events that arrive within 200ms of monitor installation.
        // This prevents the hotkey press or other pending events from immediately hiding the panel.
        let install_time = CLICK_MONITOR_INSTALL_TIME_MS.load(Ordering::SeqCst);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        if now < install_time + 200 {
            eprintln!("[powerpaste] click-outside: ignoring event during grace period");
            return;
        }

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
pub(crate) fn macos_remove_click_outside_monitor() {
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

/// Get the system accent color as a hex string.
/// Returns a default blue (#2563EB) for now.
/// TODO: Implement actual macOS accent color detection once NSColor is safe to use.
#[tauri::command]
fn get_system_accent_color() -> String {
    "#2563EB".to_string()
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

#[cfg(desktop)]
const FRONTEND_EVENT_PANEL_SHOWN: &str = "powerpaste://panel_shown";

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
        panel.setFrame_display(target, false);
    }))
    .map_err(|e| format!("objective-c exception resizing panel: {e:?}"))?;

    Ok(())
}

#[tauri::command]
fn hide_main_window(app: tauri::AppHandle) -> Result<(), String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static HIDE_CALL_COUNTER: AtomicU64 = AtomicU64::new(0);
    let call_id = HIDE_CALL_COUNTER.fetch_add(1, Ordering::SeqCst);
    eprintln!("[powerpaste] hide_main_window COMMAND #{} called from frontend", call_id);
    
    // Update atomic visibility flag
    IS_PANEL_VISIBLE.store(false, Ordering::SeqCst);
    
    #[cfg(target_os = "macos")]
    {
        use std::sync::atomic::Ordering as AtomicOrdering;
        
        // Capture the current generation before scheduling the async task.
        // If the panel is shown again before our task runs, the generation will
        // have changed and we'll skip the stale hide.
        let gen_at_request = PANEL_SHOW_GENERATION.load(AtomicOrdering::SeqCst);
        
        let app_for_task = app.clone();
        let _ = app.run_on_main_thread(move || {
            let current_gen = PANEL_SHOW_GENERATION.load(AtomicOrdering::SeqCst);
            if current_gen != gen_at_request {
                eprintln!("[powerpaste] hide #{}: skipping stale hide request (gen {} vs current {})", call_id, gen_at_request, current_gen);
                return;
            }
            eprintln!("[powerpaste] hide #{}: executing hide (gen {})", call_id, current_gen);
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

#[tauri::command]
fn close_window_by_label(app: tauri::AppHandle, label: String) -> Result<(), String> {
    eprintln!("[powerpaste] close_window_by_label called with label: {}", label);
    if let Some(window) = app.get_webview_window(&label) {
        window.close().map_err(|e| format!("Failed to close window: {}", e))?;
        eprintln!("[powerpaste] window '{}' closed successfully", label);
        Ok(())
    } else {
        eprintln!("[powerpaste] window '{}' not found", label);
        Err(format!("Window '{}' not found", label))
    }
}

#[cfg(target_os = "macos")]
fn macos_hide_overlay_panel_if_visible<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<(), String> {
    use objc2::exception::catch;
    use std::panic::AssertUnwindSafe;
    use std::sync::atomic::Ordering;
    
    eprintln!("[powerpaste] macos_hide_overlay_panel_if_visible called");
    #[cfg(desktop)]
    append_debug_log("[powerpaste] macos_hide_overlay_panel_if_visible called");

    // Try to hide the tauri-nspanel first
    if let Ok(panel) = app.get_webview_panel("main") {
        eprintln!("[powerpaste] hiding tauri-nspanel");
        IS_PANEL_VISIBLE.store(false, Ordering::SeqCst);
        // Wrap hide in exception catcher to prevent TouchBar KVO crash
        let panel_clone = panel.clone();
        let result = catch(AssertUnwindSafe(move || {
            panel_clone.hide();
        }));
        if let Err(e) = result {
            eprintln!("[powerpaste] caught objc exception during panel hide: {:?}", e);
        }
        return Ok(());
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
pub(crate) struct PermissionsStatus {
    pub(crate) platform: String,
    pub(crate) can_paste: bool,
    pub(crate) automation_ok: bool,
    pub(crate) accessibility_ok: bool,
    pub(crate) details: Option<String>,
    /// Whether running as a bundled .app (true) or dev binary (false)
    pub(crate) is_bundled: bool,
    /// The path to the executable that needs permissions
    pub(crate) executable_path: String,
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
    
    // Emit settings_changed event so all windows update immediately
    let _ = app.emit("settings_changed", &settings);
    
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
    let settings = if settings.sync_enabled {
        settings_store::ensure_sync_salt_b64(&app, settings)?
    } else {
        settings
    };
    
    // Emit settings_changed event so all windows (main overlay, settings modal) update immediately
    let _ = app.emit("settings_changed", &settings);
    
    Ok(settings)
}

#[tauri::command]
fn set_ui_mode(app: tauri::AppHandle, ui_mode: models::UiMode) -> Result<Settings, String> {
    let settings = settings_store::load_or_init_settings(&app)?;
    let settings = settings_store::set_ui_mode(&app, settings, ui_mode)?;
    set_current_ui_mode(ui_mode);
    
    // Emit settings_changed event so all windows update immediately
    let _ = app.emit("settings_changed", &settings);
    
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

#[tauri::command]
fn set_launch_at_startup(app: tauri::AppHandle, enabled: bool) -> Result<Settings, String> {
    let settings = settings_store::load_or_init_settings(&app)?;
    let settings = settings_store::set_launch_at_startup(&app, settings, enabled)?;
    
    // Apply the autostart setting using tauri-plugin-autostart
    let autostart_manager = app.autolaunch();
    if enabled {
        autostart_manager
            .enable()
            .map_err(|e| format!("Failed to enable autostart: {e}"))?;
    } else {
        autostart_manager
            .disable()
            .map_err(|e| format!("Failed to disable autostart: {e}"))?;
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
fn set_theme(app: tauri::AppHandle, theme: String) -> Result<Settings, String> {
    let settings = settings_store::load_or_init_settings(&app)?;
    let settings = settings_store::set_theme(&app, settings, theme)?;
    let _ = app.emit("settings_changed", &settings);
    Ok(settings)
}

#[tauri::command]
fn set_history_retention(app: tauri::AppHandle, days: Option<i32>) -> Result<Settings, String> {
    let settings = settings_store::load_or_init_settings(&app)?;
    let settings = settings_store::set_history_retention(&app, settings, days)?;
    
    // If retention is set, trigger cleanup
    if let Some(d) = days {
        let trash_enabled = settings.trash_enabled;
        let _ = db::cleanup_old_items(&app, d, trash_enabled);
    }
    
    let _ = app.emit("settings_changed", &settings);
    Ok(settings)
}

#[tauri::command]
fn set_trash_enabled(app: tauri::AppHandle, enabled: bool) -> Result<Settings, String> {
    let settings = settings_store::load_or_init_settings(&app)?;
    let settings = settings_store::set_trash_enabled(&app, settings, enabled)?;
    let _ = app.emit("settings_changed", &settings);
    Ok(settings)
}

#[tauri::command]
fn set_trash_retention(app: tauri::AppHandle, days: Option<i32>) -> Result<Settings, String> {
    let settings = settings_store::load_or_init_settings(&app)?;
    let settings = settings_store::set_trash_retention(&app, settings, days)?;
    
    // If retention is set, trigger cleanup
    if let Some(d) = days {
        let _ = db::cleanup_old_trash(&app, d);
    }
    
    let _ = app.emit("settings_changed", &settings);
    Ok(settings)
}

#[tauri::command]
fn connect_sync_provider(app: tauri::AppHandle, provider: SyncProvider) -> Result<ConnectedProviderInfo, String> {
    // TODO: Implement OAuth flow for each provider
    // For now, return a stub that simulates a successful connection
    let provider_info = ConnectedProviderInfo {
        provider: provider.clone(),
        account_email: match &provider {
            SyncProvider::IcloudDrive => "user@icloud.com".to_string(),
            SyncProvider::OneDrive => "user@outlook.com".to_string(),
            SyncProvider::GoogleDrive => "user@gmail.com".to_string(),
            SyncProvider::CustomFolder => return Err("Custom folder does not support OAuth".to_string()),
        },
        account_id: format!("{:?}-user-id", provider),
    };
    
    let settings = settings_store::load_or_init_settings(&app)?;
    let settings = settings_store::add_connected_provider(&app, settings, provider_info.clone())?;
    let _ = app.emit("settings_changed", &settings);
    
    Ok(provider_info)
}

#[tauri::command]
fn disconnect_sync_provider(app: tauri::AppHandle, provider: SyncProvider) -> Result<(), String> {
    let settings = settings_store::load_or_init_settings(&app)?;
    let settings = settings_store::remove_connected_provider(&app, settings, provider)?;
    let _ = app.emit("settings_changed", &settings);
    Ok(())
}

/// Paginated list result
#[derive(Serialize, Deserialize)]
struct PaginatedItems {
    items: Vec<ClipboardItem>,
    total: u32,
}

#[tauri::command]
fn list_items_paginated(
    app: tauri::AppHandle,
    limit: u32,
    offset: u32,
    query: Option<String>,
    include_trashed: Option<bool>,
) -> Result<PaginatedItems, String> {
    if include_trashed.unwrap_or(false) {
        let (items, total) = db::list_trashed_items(&app, limit, offset)?;
        Ok(PaginatedItems { items, total })
    } else {
        let (items, total) = db::list_items_paginated(&app, limit, offset, query)?;
        Ok(PaginatedItems { items, total })
    }
}

#[tauri::command]
fn list_trashed_items(app: tauri::AppHandle, limit: u32, offset: u32) -> Result<PaginatedItems, String> {
    let (items, total) = db::list_trashed_items(&app, limit, offset)?;
    Ok(PaginatedItems { items, total })
}

#[tauri::command]
fn get_trash_count(app: tauri::AppHandle) -> Result<u32, String> {
    db::get_trash_count(&app)
}

#[tauri::command]
fn restore_from_trash(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let id = Uuid::parse_str(&id).map_err(|_| "invalid id".to_string())?;
    db::restore_from_trash(&app, id)
}

#[tauri::command]
fn delete_item_forever(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let id = Uuid::parse_str(&id).map_err(|_| "invalid id".to_string())?;
    db::delete_item_forever(&app, id)
}

#[tauri::command]
fn touch_item(app: tauri::AppHandle, id: String) -> Result<bool, String> {
    let id = Uuid::parse_str(&id).map_err(|_| "invalid id".to_string())?;
    db::touch_item(&app, id)
}

#[tauri::command]
fn empty_trash(app: tauri::AppHandle) -> Result<u32, String> {
    db::empty_trash(&app)
}

#[tauri::command]
fn list_pinboard_items_paginated(
    app: tauri::AppHandle,
    limit: u32,
    offset: u32,
    pinboard: Option<String>,
) -> Result<PaginatedItems, String> {
    let (items, total) = db::list_pinboard_items_paginated(&app, limit, offset, pinboard)?;
    Ok(PaginatedItems { items, total })
}

#[tauri::command]
fn list_items(app: tauri::AppHandle, limit: u32, query: Option<String>) -> Result<Vec<ClipboardItem>, String> {
    let result = db::list_items(&app, limit, query);
    if let Ok(ref items) = result {
        eprintln!("[powerpaste] list_items returned {} items", items.len());
        if !items.is_empty() {
            eprintln!("[powerpaste] first item: id={}, kind={:?}, created_at_ms={}, pinned={}", 
                items[0].id, items[0].kind, items[0].created_at_ms, items[0].pinned);
            if items.len() > 1 {
                eprintln!("[powerpaste] second item: id={}, kind={:?}, created_at_ms={}, pinned={}", 
                    items[1].id, items[1].kind, items[1].created_at_ms, items[1].pinned);
            }
        }
    }
    result
}

#[tauri::command]
fn get_image_data(app: tauri::AppHandle, id: String) -> Result<Option<String>, String> {
    let id = Uuid::parse_str(&id).map_err(|_| "invalid id".to_string())?;
    db::get_image_data(&app, id)
}

#[tauri::command]
fn set_item_pinned(app: tauri::AppHandle, id: String, pinned: bool) -> Result<(), String> {
    let id = Uuid::parse_str(&id).map_err(|_| "invalid id".to_string())?;
    db::set_pinned(&app, id, pinned)
}

#[tauri::command]
fn set_item_pinboard(app: tauri::AppHandle, id: String, pinboard: Option<String>) -> Result<(), String> {
    let id = Uuid::parse_str(&id).map_err(|_| "invalid id".to_string())?;
    // Normalize: empty string becomes None
    let pinboard = pinboard.filter(|s| !s.trim().is_empty());
    db::set_pinboard(&app, id, pinboard)
}

#[tauri::command]
fn list_pinboards(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    db::list_pinboards(&app)
}

/// Check if a file path exists on the filesystem
#[tauri::command]
fn check_file_exists(path: String) -> bool {
    std::path::Path::new(&path).exists()
}

#[tauri::command]
fn delete_item(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let id = Uuid::parse_str(&id).map_err(|_| "invalid id".to_string())?;
    
    // Check if trash is enabled - if so, move to trash instead of permanent delete
    let settings = settings_store::get(&app)?;
    if settings.trash_enabled {
        db::trash_item(&app, id)
    } else {
        db::delete_item(&app, id)
    }
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

/// Get the icon path for an application by its bundle ID.
/// Returns the path to a PNG version of the app's icon that can be displayed in webview.
#[tauri::command]
fn get_app_icon_path(app: tauri::AppHandle, bundle_id: String) -> Result<Option<String>, String> {
    platform::get_app_icon_path(&app, &bundle_id)
}

#[tauri::command]
fn write_clipboard_text(text: String) -> Result<(), String> {
    clipboard::set_clipboard_text(&text)
}

#[tauri::command]
fn write_clipboard_files(paths: Vec<String>) -> Result<(), String> {
    clipboard::set_clipboard_files(&paths)
}

#[tauri::command]
fn paste_text(app: tauri::AppHandle, text: String) -> Result<(), String> {
    clipboard::set_clipboard_text(&text)?;

    // Move the pasted item to the top immediately so it appears as the most recent card.
    // This avoids relying on clipboard change detection (which can be unchanged if the
    // same text was already on the clipboard).
    match db::insert_text_with_source_app(&app, &text, None, None, None) {
        Ok(Some(item)) => {
            let _ = app.emit("powerpaste://new_item", item);
        }
        Ok(None) => {}
        Err(e) => {
            eprintln!("[powerpaste] paste_text: failed to bump item: {e}");
        }
    }

    perform_paste(&app)?;

    Ok(())
}

fn perform_paste(app: &tauri::AppHandle) -> Result<(), String> {
    platform::perform_paste(app)
}

#[tauri::command]
fn paste_item(app: tauri::AppHandle, id: String) -> Result<(), String> {
    use crate::models::ClipboardItemKind;
    let uuid = Uuid::parse_str(&id).map_err(|e| format!("invalid item id: {e}"))?;
    let item = db::get_item_by_id(&app, uuid)?
        .ok_or_else(|| "clipboard item not found".to_string())?;

    let is_file = item.kind == ClipboardItemKind::File
        || item.content_type.as_deref() == Some("file");

    eprintln!("[powerpaste] paste_item: id={}, kind={:?}, content_type={:?}, is_file={}", id, item.kind, item.content_type, is_file);

    if is_file {
        let paths_str = item.file_paths.clone().unwrap_or_else(|| item.text.clone());
        let paths: Vec<String> = paths_str
            .lines()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        eprintln!("[powerpaste] paste_item: setting {} file paths on clipboard", paths.len());
        clipboard::set_clipboard_files(&paths)?;
        eprintln!("[powerpaste] paste_item: file paths set successfully");
    } else if item.kind == ClipboardItemKind::Image {
        let (bytes, mime) = db::get_image_encoded_bytes(&app, uuid)?
            .ok_or_else(|| "image data missing for item".to_string())?;
        clipboard::set_clipboard_image_encoded(&bytes, mime.as_deref())?;
    } else {
        clipboard::set_clipboard_text(&item.text)?;
    }

    // Move item to the top and refresh UI.
    if db::touch_item(&app, uuid)? {
        if let Some(updated) = db::get_item_by_id(&app, uuid)? {
            let _ = app.emit("powerpaste://new_item", updated);
        }
    }

    perform_paste(&app)?;
    Ok(())
}

#[tauri::command]
fn check_permissions() -> Result<PermissionsStatus, String> {
    platform::check_permissions()
}

#[tauri::command]
fn open_accessibility_settings() -> Result<(), String> {
    platform::open_accessibility_settings()
}

#[tauri::command]
fn open_automation_settings() -> Result<(), String> {
    platform::open_automation_settings()
}

/// Trigger the macOS Accessibility permission prompt via AXIsProcessTrustedWithOptions.
/// This shows the system dialog and auto-adds the app to the Accessibility list.
#[tauri::command]
fn request_accessibility_permission() -> Result<bool, String> {
    platform::request_accessibility_permission()
}
/// targeting System Events. This causes macOS to show the "allow control" dialog.
#[tauri::command]
fn request_automation_permission() -> Result<bool, String> {
    platform::request_automation_permission()
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
                            #[cfg(target_os = "macos")]
                            {
                                if let Err(e) = toggle_main_window_wry(&app_handle_for_task) {
                                    eprintln!("[powerpaste] hotkey toggle failed: {e}");
                                }
                            }
                            #[cfg(not(target_os = "macos"))]
                            {
                                if let Err(e) = toggle_main_window(&app_handle_for_task) {
                                    eprintln!("[powerpaste] hotkey toggle failed: {e}");
                                }
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

/// Temporarily unregister the global hotkey AND disable browser accelerator keys
/// so every key combo reaches the webview (used while the hotkey recorder is active).
#[tauri::command]
fn suspend_hotkey(app: tauri::AppHandle, webview: tauri::Webview) -> Result<(), String> {
    eprintln!("[powerpaste] suspending global hotkey for recording");
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| format!("failed to unregister hotkeys: {e}"))?;

    platform::suspend_browser_accelerator_keys(&webview);

    Ok(())
}

/// Re-register the global hotkey and restore browser accelerator keys after recording.
#[tauri::command]
fn resume_hotkey(app: tauri::AppHandle, webview: tauri::Webview) -> Result<(), String> {
    let settings = settings_store::load_or_init_settings(&app)?;
    eprintln!("[powerpaste] resuming global hotkey: {}", settings.hotkey);
    register_hotkey(&app, &settings.hotkey)?;

    platform::resume_browser_accelerator_keys(&webview);

    Ok(())
}

/// Toggle using standard Tauri window (fallback for debugging keyboard input issues)
#[cfg(target_os = "macos")]
#[allow(dead_code)]
fn toggle_standard_window<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) -> Result<(), String> {
    use objc2::MainThreadMarker;
    use objc2::rc::Retained;
    use objc2_app_kit::{NSApplication, NSScreen, NSWindow};
    use objc2_foundation::NSRect;
    use std::sync::atomic::Ordering;
    
    // Use atomic flag instead of window.is_visible() to avoid race conditions
    // on first toggle when the window hasn't been realized yet
    let is_visible = IS_PANEL_VISIBLE.load(Ordering::SeqCst);
    
    eprintln!("[powerpaste] toggle_standard_window: is_visible={}", is_visible);
    
    if is_visible {
        IS_PANEL_VISIBLE.store(false, Ordering::SeqCst);
        window.hide().map_err(|e| format!("failed to hide: {e}"))?;
        eprintln!("[powerpaste] standard window hidden");
    } else {
        IS_PANEL_VISIBLE.store(true, Ordering::SeqCst);
        
        // Get ns_window and configure it
        let ptr = window.ns_window().map_err(|e| format!("ns_window error: {e}"))?;
        let ns_window: Retained<NSWindow> = unsafe {
            Retained::retain(ptr.cast()).ok_or("failed to retain NSWindow")?
        };
        
        // Activate app and show window
        if let Some(mtm) = MainThreadMarker::new() {
            let app = NSApplication::sharedApplication(mtm);
            #[allow(deprecated)]
            app.activateIgnoringOtherApps(true);
            
            // Calculate proper size and position like NSPanel code
            if let Some(screen) = NSScreen::mainScreen(mtm) {
                let screen_frame: NSRect = screen.frame();
                let ui_mode = get_current_ui_mode();
                let (w, h) = overlay_size_for_monitor(
                    screen_frame.size.width.max(1.0).round() as u32,
                    screen_frame.size.height.max(1.0).round() as u32,
                    ui_mode,
                );
                
                // Position at bottom center of screen
                // Tauri uses top-left origin where y=0 is top
                // We want the window touching the bottom edge
                let x = (screen_frame.size.width - w as f64) / 2.0;
                let y = screen_frame.size.height - h as f64;
                
                // Set size and position BEFORE showing
                window.set_size(tauri::LogicalSize::new(w, h)).ok();
                window.set_position(tauri::LogicalPosition::new(x as i32, y as i32)).ok();
                
                eprintln!("[powerpaste] standard window resized to {}x{} at ({}, {})", w, h, x, y);
            }
        }
        
        window.show().map_err(|e| format!("failed to show: {e}"))?;
        window.unminimize().map_err(|e| format!("failed to unminimize: {e}"))?;
        window.set_focus().map_err(|e| format!("failed to focus: {e}"))?;
        
        // Make window key
        ns_window.makeKeyAndOrderFront(None);
        
        // Emit panel shown event for JS focus
        let _ = window.emit(FRONTEND_EVENT_PANEL_SHOWN, ());
        
        eprintln!("[powerpaste] standard window shown and focused");
    }
    
    Ok(())
}

#[allow(dead_code)]
fn toggle_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<(), String> {
    // Load UI mode from settings before toggling
    if let Ok(settings) = settings_store::get(app) {
        set_current_ui_mode(settings.ui_mode);
    }
    
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;

    #[cfg(target_os = "macos")]
    {
        // Use standard window for now, toggle_nspanel_overlay is available for Wry runtime
        return toggle_standard_window(&window);
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

        let ui_mode = get_current_ui_mode();
        match ui_mode {
            models::UiMode::Floating => position_floating_near_cursor(&window)?,
            models::UiMode::Fixed => position_as_bottom_overlay(&window)?,
        }

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

/// Toggle using tauri-nspanel. This is the main entry point for Wry runtime.
#[cfg(target_os = "macos")]
fn toggle_main_window_wry(app: &tauri::AppHandle) -> Result<(), String> {
    // Load UI mode from settings before toggling
    if let Ok(settings) = settings_store::get(app) {
        set_current_ui_mode(settings.ui_mode);
    }
    
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;

    toggle_nspanel_overlay(app, &window)
}

/// Static to track if click-outside monitor is installed
#[cfg(target_os = "macos")]
static CLICK_MONITOR_INSTALLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Install a global event monitor to detect clicks outside the panel.
/// This replaces the window_did_resign_key handler which caused TouchBar KVO crashes.
#[cfg(target_os = "macos")]
fn install_click_outside_monitor(app: &tauri::AppHandle) {
    use block2::StackBlock;
    use objc2::runtime::AnyObject;
    use objc2::msg_send;
    use objc2_app_kit::{NSEvent, NSEventMask, NSEventType};
    use std::sync::atomic::Ordering;
    use tauri_nspanel::ManagerExt as NspanelManagerExt;
    
    // Only install once
    if CLICK_MONITOR_INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }
    
    let app_handle = app.clone();
    
    // Create a local event monitor for mouse down events
    // We use addGlobalMonitorForEventsMatchingMask to catch clicks anywhere
    let handler = StackBlock::new(move |event: *mut NSEvent| {
        if event.is_null() {
            return;
        }
        
        unsafe {
            let event_ref = &*event;
            let event_type: NSEventType = event_ref.r#type();
            
            // Only handle left and right mouse down
            if event_type != NSEventType::LeftMouseDown && event_type != NSEventType::RightMouseDown {
                return;
            }
            
            // Check if panel exists and is visible
            if !IS_PANEL_VISIBLE.load(Ordering::SeqCst) {
                return;
            }
            
            // Get the panel
            if let Ok(panel) = app_handle.get_webview_panel("main") {
                let ns_panel = panel.as_panel();
                
                // Get the window that received the event
                let event_window: *mut AnyObject = msg_send![event_ref, window];
                
                // If click was in our panel's window, don't hide
                let panel_ptr: *const AnyObject = (ns_panel as *const objc2_app_kit::NSPanel).cast();
                if !event_window.is_null() && event_window as *const _ == panel_ptr {
                    return;
                }
                
                // Click was outside - hide the panel
                eprintln!("[powerpaste] click outside detected - hiding panel");
                IS_PANEL_VISIBLE.store(false, Ordering::SeqCst);
                // Note: We can't use objc2::exception::catch inside this block2 closure
                // because of lifetime issues. The hide here is "safe" because the
                // global event monitor runs on the main thread's event loop, and
                // exceptions will be caught by the outer exception mechanism.
                panel.hide();
            }
        }
    });
    
    unsafe {
        // Install global monitor
        let ns_event_class: *const AnyObject = msg_send![objc2::class!(NSEvent), class];
        let mask = NSEventMask::LeftMouseDown.0 | NSEventMask::RightMouseDown.0;
        let _monitor: *mut AnyObject = msg_send![
            ns_event_class,
            addGlobalMonitorForEventsMatchingMask: mask,
            handler: &*handler
        ];
        // Note: We don't remove the monitor since it should live for the app lifetime
        // The handler StackBlock needs to be leaked to keep the closure alive
        std::mem::forget(handler);
    }
    
    eprintln!("[powerpaste] installed click-outside monitor");
}

/// Convert window to panel EARLY during app setup, before any show operation.
/// This prevents TouchBar KVO observer crashes that occur when:
/// 1. Window is shown (TouchBar observers registered)
/// 2. Window converted to panel (responder chain changes)
/// 3. Panel hidden (TouchBar tries to remove non-existent observers -> crash)
#[cfg(target_os = "macos")]
fn convert_window_to_panel_early(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
) -> Result<(), String> {
    use objc2::MainThreadMarker;
    use std::sync::atomic::Ordering;
    use tauri_nspanel::{
        CollectionBehavior, ManagerExt as NspanelManagerExt, PanelLevel, StyleMask,
        WebviewWindowExt as NspanelWebviewWindowExt,
    };
    
    let _mtm = MainThreadMarker::new().ok_or("not on main thread")?;
    
    // Check if panel already exists
    if app.get_webview_panel("main").is_ok() {
        eprintln!("[powerpaste] panel already exists, skipping early conversion");
        return Ok(());
    }
    
    eprintln!("[powerpaste] converting window to panel EARLY (before any show)");
    
    // Disable TouchBar on window BEFORE conversion
    disable_touchbar_for_window(window);
    
    // Convert to panel
    let panel = window
        .to_panel::<PowerPastePanel>()
        .map_err(|e| format!("failed to convert window to panel: {e}"))?;
    
    // Disable TouchBar on panel too
    disable_touchbar_for_panel(&panel);
    
    // Configure panel
    panel.set_level(PanelLevel::ScreenSaver.value());
    panel.set_hides_on_deactivate(false);
    panel.set_works_when_modal(true);
    
    panel.set_style_mask(
        StyleMask::empty()
            .nonactivating_panel()
            .resizable()
            .into(),
    );
    
    panel.set_collection_behavior(
        CollectionBehavior::new()
            .full_screen_auxiliary()
            .can_join_all_spaces()
            .into(),
    );
    
    // Set up minimal event handler (no resign key - that causes crashes)
    let handler = PowerPastePanelEventHandler::new();
    
    let app_handle = app.clone();
    handler.window_did_become_key(move |_notification| {
        eprintln!("[powerpaste] panel became key window");
        if let Some(w) = app_handle.get_webview_window("main") {
            let _ = w.emit(FRONTEND_EVENT_PANEL_SHOWN, ());
        }
    });
    
    panel.set_event_handler(Some(handler.as_ref()));
    
    // Install click-outside monitor instead of resign key handler
    install_click_outside_monitor(app);
    
    // Pre-realize by showing then hiding the PANEL (not window).
    // Use alpha=0 so the brief show is completely invisible to the user.
    // Also set the correct frame before showing to avoid stale geometry.
    {
        use objc2_app_kit::NSScreen;
        use objc2_foundation::NSRect;
        let pre_mtm = MainThreadMarker::new().unwrap();
        if let Some(screen) = macos_screen_containing_cursor(pre_mtm)
            .or_else(|| NSScreen::mainScreen(pre_mtm))
        {
            let sf: NSRect = screen.frame();
            let ui_mode = get_current_ui_mode();
            let (pw, ph) = overlay_size_for_monitor(
                sf.size.width.max(1.0).round() as u32,
                sf.size.height.max(1.0).round() as u32,
                ui_mode,
            );
            let mut pre_frame = sf;
            pre_frame.size.width = (pw as f64).min(sf.size.width);
            pre_frame.size.height = (ph as f64).min(sf.size.height);
            pre_frame.origin.x = sf.origin.x + (sf.size.width - pre_frame.size.width) / 2.0;
            pre_frame.origin.y = sf.origin.y; // bottom
            panel.set_alpha_value(0.0);
            panel.as_panel().setFrame_display(pre_frame, false);
        } else {
            panel.set_alpha_value(0.0);
        }
    }
    panel.show();
    IS_PANEL_VISIBLE.store(true, Ordering::SeqCst);
    // Small delay to let the webview initialize
    std::thread::sleep(std::time::Duration::from_millis(50));
    panel.hide();
    IS_PANEL_VISIBLE.store(false, Ordering::SeqCst);
    // Restore alpha for future shows
    panel.set_alpha_value(1.0);
    
    eprintln!("[powerpaste] panel configured and pre-realized successfully");
    Ok(())
}

/// Install a global exception handler that intercepts TouchBar KVO exceptions.
/// These exceptions occur asynchronously during the run loop when the TouchBar
/// system tries to remove observers from our NSPanel that weren't properly registered.
/// Since we can't prevent these from being thrown, we swizzle NSException's raise
/// to catch and ignore this specific error pattern.
/// 
/// This uses method swizzling to replace NSWindow's removeObserver:forKeyPath:context:
/// with a safe version that skips the problematic TouchBar KVO removal.
#[cfg(target_os = "macos")]
fn install_touchbar_exception_handler() {
    use objc2::runtime::{AnyClass, AnyObject, Sel};
    use objc2::{class, msg_send, sel};
    use std::ffi::CStr;
    use std::os::raw::c_void;
    use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
    
    static HANDLER_INSTALLED: AtomicBool = AtomicBool::new(false);
    static ORIGINAL_IMP: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
    
    // Only install once
    if HANDLER_INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }
    
    unsafe {
        // Get NSWindow class
        let nswindow_class = class!(NSWindow);
        let selector = sel!(removeObserver:forKeyPath:context:);
        
        // Get the original method
        let method = {
            extern "C" {
                fn class_getInstanceMethod(cls: *const AnyClass, sel: Sel) -> *mut c_void;
                fn method_getImplementation(method: *mut c_void) -> *const c_void;
            }
            
            let method = class_getInstanceMethod(nswindow_class as *const _, selector);
            if method.is_null() {
                eprintln!("[powerpaste] failed to find removeObserver:forKeyPath:context: method");
                return;
            }
            
            // Save original implementation
            let original = method_getImplementation(method);
            ORIGINAL_IMP.store(original as *mut c_void, Ordering::SeqCst);
            
            method
        };
        
        // Our replacement implementation that skips problematic TouchBar KVO removals
        // This is extern "C" so it MUST NOT panic or unwind
        extern "C" fn safe_remove_observer(
            this: *mut AnyObject,
            _sel: Sel,
            observer: *mut AnyObject,
            key_path: *mut AnyObject,
            context: *mut c_void,
        ) {
            unsafe {
                // Check if the OBSERVER is TouchBar-related - skip ALL removals from TouchBar observers
                // This handles both "nextResponder" and "delegate" keypaths
                if !observer.is_null() {
                    // Get observer's class name
                    extern "C" {
                        fn class_getName(cls: *const AnyClass) -> *const std::ffi::c_char;
                    }
                    let observer_class: *const AnyClass = msg_send![observer, class];
                    if !observer_class.is_null() {
                        let class_name_ptr = class_getName(observer_class);
                        if !class_name_ptr.is_null() {
                            if let Ok(class_name) = CStr::from_ptr(class_name_ptr).to_str() {
                                if class_name.contains("TouchBar") {
                                    // Skip all KVO removals from TouchBar observers
                                    eprintln!("[powerpaste] skipping {} KVO removal from {}", 
                                        if key_path.is_null() { 
                                            "unknown".to_string() 
                                        } else {
                                            let utf8: *const std::ffi::c_char = msg_send![key_path, UTF8String];
                                            if utf8.is_null() {
                                                "unknown".to_string()
                                            } else {
                                                CStr::from_ptr(utf8).to_str().unwrap_or("unknown").to_string()
                                            }
                                        },
                                        class_name
                                    );
                                    return;
                                }
                            }
                        }
                    }
                }
                
                // For all other cases, call the original implementation directly
                // Do NOT use objc2::exception::catch - it can panic and extern "C" cannot unwind
                let original_ptr = ORIGINAL_IMP.load(Ordering::SeqCst);
                if !original_ptr.is_null() {
                    type RemoveObserverFn = extern "C" fn(*mut AnyObject, Sel, *mut AnyObject, *mut AnyObject, *mut c_void);
                    let original_fn: RemoveObserverFn = std::mem::transmute(original_ptr);
                    original_fn(this, sel!(removeObserver:forKeyPath:context:), observer, key_path, context);
                }
            }
        }
        
        // Swizzle the method
        extern "C" {
            fn method_setImplementation(method: *mut c_void, imp: *const c_void) -> *const c_void;
        }
        
        method_setImplementation(method, safe_remove_observer as *const c_void);
        eprintln!("[powerpaste] installed TouchBar KVO swizzle handler");
    }
}

/// Disable TouchBar integration globally at the NSApplication level.
/// This prevents macOS from automatically searching for TouchBar providers
/// which causes KVO observer crashes when windows are converted to NSPanel.
#[cfg(target_os = "macos")]
fn disable_touchbar_globally(app: &objc2_app_kit::NSApplication) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    
    unsafe {
        let app_ptr: *const AnyObject = (app as *const objc2_app_kit::NSApplication).cast();
        
        // Disable automatic TouchBar menu item customization
        let no: bool = false;
        let _: () = msg_send![app_ptr, setAutomaticCustomizeTouchBarMenuItemEnabled: no];
        
        eprintln!("[powerpaste] disabled TouchBar globally for NSApplication");
    }
}

/// Disable TouchBar integration for the given panel to prevent KVO observer crashes.
/// The crash occurs because macOS TouchBar system registers KVO observers on NSWindow,
/// and when the window is converted to NSPanel or when the panel is destroyed,
/// the observers become invalid and throw an exception.
#[cfg(target_os = "macos")]
fn disable_touchbar_for_panel(panel: &std::sync::Arc<dyn tauri_nspanel::Panel>) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use std::ptr;
    
    // Get the raw NSPanel pointer using as_panel()
    let ns_panel = panel.as_panel();
    
    // Set touchBar to nil to prevent TouchBar system from registering observers
    // This is safe because we don't use TouchBar functionality
    unsafe {
        // Cast the &NSPanel to a raw pointer
        let panel_ptr: *const AnyObject = (ns_panel as *const objc2_app_kit::NSPanel).cast();
        // setTouchBar: nil
        let _: () = msg_send![panel_ptr, setTouchBar: ptr::null::<AnyObject>()];
        eprintln!("[powerpaste] disabled TouchBar for panel");
    }
}

/// Disable TouchBar integration for a Tauri WebviewWindow.
/// This MUST be called BEFORE any show/hide operation to prevent macOS from 
/// registering TouchBar KVO observers that cause crashes when the window is
/// later converted to an NSPanel.
#[cfg(target_os = "macos")]
fn disable_touchbar_for_window(window: &tauri::WebviewWindow) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use std::ptr;
    
    // Get the raw NSWindow pointer from the Tauri window
    // We need to use the native handle
    if let Ok(ns_window) = window.ns_window() {
        unsafe {
            let window_ptr = ns_window as *mut AnyObject;
            // 1. Disable automatic TouchBar provider searching
            let no: bool = false;
            let _: () = msg_send![window_ptr, setAutorecalculatesKeyViewLoop: no];
            
            // 2. Set touchBar to nil to clear any existing touchbar
            let _: () = msg_send![window_ptr, setTouchBar: ptr::null::<AnyObject>()];
            
            // 3. Also disable on the contentView if it exists
            let content_view: *mut AnyObject = msg_send![window_ptr, contentView];
            if !content_view.is_null() {
                let _: () = msg_send![content_view, setTouchBar: ptr::null::<AnyObject>()];
            }
            
            eprintln!("[powerpaste] disabled TouchBar for window and contentView");
        }
    } else {
        eprintln!("[powerpaste] failed to get NSWindow for TouchBar disable");
    }
}

/// Toggle the NSPanel overlay using tauri-nspanel library.
/// This properly handles keyboard focus/input that was broken with raw NSPanel code.
/// Note: This function works only with the Wry runtime since that's what PowerPastePanel implements.
#[cfg(target_os = "macos")]
fn toggle_nspanel_overlay(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
) -> Result<(), String> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSScreen};
    use objc2_foundation::NSRect;
    use std::sync::atomic::Ordering;

    let mtm = MainThreadMarker::new().ok_or("not on main thread")?;

    // Check if panel already exists
    let panel = match app.get_webview_panel("main") {
        Ok(p) => p,
        Err(_) => {
            // First time: convert window to panel
            eprintln!("[powerpaste] converting window to panel with tauri-nspanel");
            
            let panel = window
                .to_panel::<PowerPastePanel>()
                .map_err(|e| format!("failed to convert window to panel: {e}"))?;
            
            // Disable TouchBar on this panel to prevent KVO observer crashes
            // The crash occurs because TouchBar system registers KVO observers on NSWindow
            // that become invalid when the window is converted to NSPanel
            disable_touchbar_for_panel(&panel);
            
            // Configure panel - use ScreenSaver level to appear above Dock
            panel.set_level(PanelLevel::ScreenSaver.value());
            panel.set_hides_on_deactivate(false);
            panel.set_works_when_modal(true);
            
            // Set style mask for non-activating panel that can still receive input
            panel.set_style_mask(
                StyleMask::empty()
                    .nonactivating_panel()
                    .resizable()
                    .into(),
            );
            
            // Configure collection behavior for fullscreen support
            panel.set_collection_behavior(
                CollectionBehavior::new()
                    .full_screen_auxiliary()
                    .can_join_all_spaces()
                    .into(),
            );
            
            // Set up event handlers
            let handler = PowerPastePanelEventHandler::new();
            
            let app_handle = app.clone();
            handler.window_did_become_key(move |_notification| {
                eprintln!("[powerpaste] panel became key window");
                if let Some(w) = app_handle.get_webview_window("main") {
                    let _ = w.emit(FRONTEND_EVENT_PANEL_SHOWN, ());
                }
            });
            
            // NOTE: We intentionally do NOT use window_did_resign_key handler here.
            // Using it causes a TouchBar KVO observer crash because:
            // 1. macOS TouchBar system observes "nextResponder" on NSWindow
            // 2. When window resigns key, the responder chain changes
            // 3. The TouchBar observer tries to clean itself up but the observer
            //    registration got confused during window-to-panel conversion
            // 4. Crash: "Cannot remove observer... because it is not registered"
            //
            // Instead, we use a global event monitor to detect clicks outside the panel.
            // See install_click_outside_monitor below.
            
            panel.set_event_handler(Some(handler.as_ref()));
            
            // Install a click-outside monitor to hide the panel
            install_click_outside_monitor(&app);
            
            eprintln!("[powerpaste] panel configured successfully");
            panel
        }
    };

    // Toggle visibility using atomic flag
    let was_visible = IS_PANEL_VISIBLE.load(Ordering::SeqCst);
    
    if was_visible {
        eprintln!("[powerpaste] hiding nspanel overlay");
        IS_PANEL_VISIBLE.store(false, Ordering::SeqCst);
        // Wrap hide in exception catcher to prevent TouchBar KVO crash
        {
            use objc2::exception::catch;
            use std::panic::AssertUnwindSafe;
            let panel_clone = panel.clone();
            let result = catch(AssertUnwindSafe(move || {
                panel_clone.hide();
            }));
            if let Err(e) = result {
                eprintln!("[powerpaste] caught objc exception during panel hide: {:?}", e);
            }
        }
    } else {
        eprintln!("[powerpaste] showing nspanel overlay");
        
        // Remember frontmost app before showing
        if let Some(name) = platform::query_frontmost_app_name() {
            platform::set_last_frontmost_app_name(name.clone());
            eprintln!("[powerpaste] saved frontmost app: {}", name);
        }
        
        // Position and size the panel based on UI mode
        if let Some(screen) = macos_screen_containing_cursor(mtm)
            .or_else(|| NSScreen::mainScreen(mtm))
        {
            let screen_frame: NSRect = screen.frame();
            let ui_mode = get_current_ui_mode();
            let (w, h) = overlay_size_for_monitor(
                screen_frame.size.width.max(1.0).round() as u32,
                screen_frame.size.height.max(1.0).round() as u32,
                ui_mode,
            );
            
            // Calculate position based on UI mode
            let (x, y) = match ui_mode {
                models::UiMode::Floating => {
                    // Position below cursor (vertically arranged cards)
                    if let Some((cursor_x, cursor_y)) = platform::get_cursor_position() {
                        // Align left edge to cursor, position below cursor
                        let mut calc_x = cursor_x;
                        // Position 10px below cursor (in Cocoa coordinates, y increases upward)
                        // So we subtract height + gap to go below
                        let mut calc_y = cursor_y - h as f64 - 10.0;
                        
                        // Clamp to screen bounds
                        calc_x = calc_x.max(screen_frame.origin.x).min(screen_frame.origin.x + screen_frame.size.width - w as f64);
                        calc_y = calc_y.max(screen_frame.origin.y).min(screen_frame.origin.y + screen_frame.size.height - h as f64);
                        
                        (calc_x, calc_y)
                    } else {
                        // Fallback to center if cursor position unavailable
                        let calc_x = (screen_frame.size.width - w as f64) / 2.0;
                        let calc_y = (screen_frame.size.height - h as f64) / 2.0;
                        (calc_x, calc_y)
                    }
                }
                models::UiMode::Fixed => {
                    // Fixed at bottom center of screen
                    let calc_x = (screen_frame.size.width - w as f64) / 2.0;
                    let calc_y = 10.0; // Small offset from bottom edge
                    (calc_x, calc_y)
                }
            };
            
            // Set frame atomically using Cocoa API (avoids two-step Tauri set_size + set_position
            // which can flash the panel at the wrong position for one frame).
            {
                use objc2_foundation::{NSPoint, NSSize, NSRect as CRect};
                let target_frame = CRect {
                    origin: NSPoint { x, y },
                    size: NSSize { width: w as f64, height: h as f64 },
                };
                // Hide with alpha before positioning to avoid any visible flash
                panel.set_alpha_value(0.0);
                panel.as_panel().setFrame_display(target_frame, false);
                eprintln!("[powerpaste] panel sized to {}x{} at ({}, {}) [ui_mode={:?}]", w, h, x, y, ui_mode);
            }
        }
        
        // Activate app
        let ns_app = NSApplication::sharedApplication(mtm);
        #[allow(deprecated)]
        ns_app.activateIgnoringOtherApps(true);
        
        // Show and make key, then restore alpha
        IS_PANEL_VISIBLE.store(true, Ordering::SeqCst);
        panel.show_and_make_key();
        panel.set_alpha_value(1.0);
        
        // Emit panel shown event
        let _ = window.emit(FRONTEND_EVENT_PANEL_SHOWN, ());
    }
    
    Ok(())
}

#[cfg(target_os = "macos")]
#[allow(dead_code)]
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
            IS_PANEL_VISIBLE.store(false, Ordering::SeqCst);
            exception::catch(std::panic::AssertUnwindSafe(|| {
                panel.orderOut(None);
            }))
            .map_err(|e| format!("objective-c exception hiding panel: {e:?}"))?;

            // Remove monitors when hiding
            macos_remove_keyboard_monitor();
            macos_remove_click_outside_monitor();
            macos_remove_mouse_focus_monitor();

            // Restore agent/app-less behavior when hidden.
            let app = NSApplication::sharedApplication(mtm);
            let _ = app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
            eprintln!("[powerpaste] macos overlay panel hide");
            #[cfg(desktop)]
            append_debug_log("[powerpaste] macos overlay panel hide");
            return Ok(());
        }

        // Mark as visible before showing
        IS_PANEL_VISIBLE.store(true, Ordering::SeqCst);

        // Snapshot the current frontmost app before we activate ourselves.
        // This lets us restore focus when the user chooses an item to paste.
        // Skip dev tools that shouldn't be paste targets.
        eprintln!("[powerpaste] querying frontmost app...");
        if let Some(name) = platform::query_frontmost_app_name() {
            eprintln!("[powerpaste] frontmost app query returned: {}", name);
            let skip_apps = ["node", "PowerPaste", "Code Helper"];
            if !skip_apps.iter().any(|s| name.contains(s)) {
                eprintln!("[powerpaste] recording frontmost app: {}", name);
                platform::set_last_frontmost_app_name(name);
            } else {
                eprintln!("[powerpaste] skipping frontmost app (dev tool): {}", name);
            }
        } else {
            eprintln!("[powerpaste] frontmost app query returned None");
        }

        // Show: activate app (helps on some systems), then order front.
        // Use Regular policy temporarily to ensure we can receive keyboard input
        let app = NSApplication::sharedApplication(mtm);
        let _ = app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);

        // Recompute frame each show (handles display changes, fullscreen spaces, etc.).
        let screen = macos_screen_containing_cursor(mtm)
            .or_else(|| panel.screen())
            .or_else(|| NSScreen::mainScreen(mtm))
            .ok_or("no screen found")?;
        let screen_frame: NSRect = screen.frame();
        let ui_mode = get_current_ui_mode();
        let (w, h) = overlay_size_for_monitor(
            screen_frame.size.width.max(1.0).round() as u32,
            screen_frame.size.height.max(1.0).round() as u32,
            ui_mode,
        );
        let mut target = screen_frame;
        target.size.width = (w as f64).min(screen_frame.size.width);
        target.size.height = (h as f64).min(screen_frame.size.height);
        
        // Position based on UI mode
        match ui_mode {
            models::UiMode::Floating => {
                // Position below cursor (vertically arranged cards)
                if let Some((cursor_x, cursor_y)) = platform::get_cursor_position() {
                    // Align left edge to cursor, position below cursor
                    let mut x = cursor_x;
                    let mut y = cursor_y - target.size.height - 10.0; // 10px below cursor
                    
                    // Clamp to screen bounds
                    x = x.max(screen_frame.origin.x).min(screen_frame.origin.x + screen_frame.size.width - target.size.width);
                    y = y.max(screen_frame.origin.y).min(screen_frame.origin.y + screen_frame.size.height - target.size.height);
                    
                    target.origin.x = x;
                    target.origin.y = y;
                } else {
                    // Fallback to center if cursor position unavailable
                    target.origin.x = screen_frame.origin.x + (screen_frame.size.width - target.size.width) / 2.0;
                    target.origin.y = screen_frame.origin.y + (screen_frame.size.height - target.size.height) / 2.0;
                }
            }
            models::UiMode::Fixed => {
                // Fixed at bottom, use FIXED_WIDTH_FRACTION for consistency with first-show
                target.origin.x = screen_frame.origin.x + (screen_frame.size.width - target.size.width) / 2.0;
                target.origin.y = screen_frame.origin.y;
            }
        }

        // Increment the show generation to invalidate any pending hide requests.
        // This prevents race conditions where a hide scheduled before this show
        // would incorrectly hide the newly-shown panel.
        PANEL_SHOW_GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        // Hide with alpha before repositioning to prevent visible flash
        unsafe {
            let _: () = objc2::msg_send![&*panel, setAlphaValue: 0.0f64];
        }

        exception::catch(std::panic::AssertUnwindSafe(|| {
            panel.setLevel(NSScreenSaverWindowLevel);
            panel.setFrame_display(target, false);
            panel.orderFrontRegardless();
            panel.makeKeyWindow();
            // Make the WryWebView first responder so text inputs receive keyboard events
            if let Some(content_view) = panel.contentView() {
                // Find WryWebView in the view hierarchy (this is the actual webview, not WryWebViewParent)
                use objc2::runtime::AnyObject;
                fn find_wry_webview(view: *mut AnyObject, depth: usize) -> Option<*mut AnyObject> {
                    if view.is_null() || depth > 10 { return None; }
                    unsafe {
                        let class_name: *const AnyObject = objc2::msg_send![view, className];
                        if !class_name.is_null() {
                            let class_str: *const std::ffi::c_char = objc2::msg_send![class_name, UTF8String];
                            if !class_str.is_null() {
                                let name = std::ffi::CStr::from_ptr(class_str).to_string_lossy();
                                // Match WryWebView specifically (not WryWebViewParent)
                                if name == "WryWebView" || name == "WKWebView" {
                                    return Some(view);
                                }
                            }
                        }
                        let subviews: *const AnyObject = objc2::msg_send![view, subviews];
                        if !subviews.is_null() {
                            let count: usize = objc2::msg_send![subviews, count];
                            for i in 0..count {
                                let subview: *mut AnyObject = objc2::msg_send![subviews, objectAtIndex: i];
                                if let Some(wv) = find_wry_webview(subview, depth + 1) { return Some(wv); }
                            }
                        }
                        None
                    }
                }
                let cv_ptr = &*content_view as *const _ as *mut AnyObject;
                if let Some(webview) = find_wry_webview(cv_ptr, 0) {
                    unsafe {
                        let wv_view: *const AnyObject = webview as *const AnyObject;
                        let _: bool = objc2::msg_send![&*panel, makeFirstResponder: wv_view];
                        eprintln!("[powerpaste] made WryWebView first responder");
                    }
                } else {
                    // Fallback to content view
                    panel.makeFirstResponder(Some(&content_view));
                    eprintln!("[powerpaste] WryWebView not found, using content view as first responder");
                }
            }
        }))
        .map_err(|e| format!("objective-c exception showing panel: {e:?}"))?;

        // Restore alpha now that frame is committed
        unsafe {
            let _: () = objc2::msg_send![&*panel, setAlphaValue: 1.0f64];
        }

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
        // Install mouse focus monitor to handle left-clicks for text input
        macos_install_mouse_focus_monitor();

        // Notify frontend that panel is shown so it can focus the search input
        let _ = window.emit(FRONTEND_EVENT_PANEL_SHOWN, ());

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
    let screen = macos_screen_containing_cursor(mtm)
        .or_else(|| ns_window.screen())
        .or_else(|| NSScreen::mainScreen(mtm))
        .ok_or("no screen found")?;

    let screen_frame: NSRect = screen.frame();
    let ui_mode = get_current_ui_mode();
    let (w, h) = overlay_size_for_monitor(
        screen_frame.size.width.max(1.0).round() as u32,
        screen_frame.size.height.max(1.0).round() as u32,
        ui_mode,
    );
    let mut target = screen_frame;
    target.size.width = (w as f64).min(screen_frame.size.width);
    target.size.height = (h as f64).min(screen_frame.size.height);
    
    // Position based on UI mode
    match ui_mode {
        models::UiMode::Floating => {
            // Position below cursor (vertically arranged cards)
            if let Some((cursor_x, cursor_y)) = platform::get_cursor_position() {
                // Align left edge to cursor, position below cursor
                let mut x = cursor_x;
                let mut y = cursor_y - target.size.height - 10.0;
                
                x = x.max(screen_frame.origin.x).min(screen_frame.origin.x + screen_frame.size.width - target.size.width);
                y = y.max(screen_frame.origin.y).min(screen_frame.origin.y + screen_frame.size.height - target.size.height);
                
                target.origin.x = x;
                target.origin.y = y;
            } else {
                target.origin.x = screen_frame.origin.x + (screen_frame.size.width - target.size.width) / 2.0;
                target.origin.y = screen_frame.origin.y + (screen_frame.size.height - target.size.height) / 2.0;
            }
        }
        models::UiMode::Fixed => {
            // Fixed at bottom, use FIXED_WIDTH_FRACTION for consistency
            target.origin.x = screen_frame.origin.x + (screen_frame.size.width - target.size.width) / 2.0;
            target.origin.y = screen_frame.origin.y;
        }
    }

    // Note: Removed NonactivatingPanel to allow proper text input focus.
    // The tradeoff is that the previous app loses focus when the panel appears,
    // but we restore it when pasting using macos_set_last_frontmost_app_name.
    let style = NSWindowStyleMask::Borderless
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
    panel.setAcceptsMouseMovedEvents(true);
    panel.setFloatingPanel(true);
    panel.setBecomesKeyOnlyIfNeeded(false);
    panel.setWorksWhenModal(true);

    // Keep a retained pointer around for future toggles.
    let raw = Retained::as_ptr(&panel) as usize;
    let _ = OVERLAY_PANEL_PTR.set(raw);

    // First show.
    // Use Regular policy temporarily to ensure we can receive keyboard input
    let app = NSApplication::sharedApplication(mtm);
    let _ = app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    // Snapshot the current frontmost app (skip dev tools)
    eprintln!("[powerpaste] (first show) querying frontmost app...");
    if let Some(name) = platform::query_frontmost_app_name() {
        eprintln!("[powerpaste] (first show) frontmost app: {}", name);
        let skip_apps = ["node", "PowerPaste", "Code Helper"];
        if !skip_apps.iter().any(|s| name.contains(s)) {
            eprintln!("[powerpaste] (first show) recording frontmost app: {}", name);
            platform::set_last_frontmost_app_name(name);
        } else {
            eprintln!("[powerpaste] (first show) skipping (dev tool): {}", name);
        }
    } else {
        eprintln!("[powerpaste] (first show) no frontmost app found");
    }
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);

    // Hide with alpha before repositioning to prevent visible flash
    unsafe {
        let _: () = objc2::msg_send![&*panel, setAlphaValue: 0.0f64];
    }

    exception::catch(std::panic::AssertUnwindSafe(|| {
        panel.setLevel(NSScreenSaverWindowLevel);
        panel.setFrame_display(target, false);
        panel.orderFrontRegardless();
        panel.makeKeyWindow();
        // Make the WryWebView first responder so text inputs receive keyboard events
        if let Some(content_view) = panel.contentView() {
            // Find WryWebView in the view hierarchy (this is the actual webview, not WryWebViewParent)
            use objc2::runtime::AnyObject;
            fn find_wry_webview(view: *mut AnyObject, depth: usize) -> Option<*mut AnyObject> {
                if view.is_null() || depth > 10 { return None; }
                unsafe {
                    let class_name: *const AnyObject = objc2::msg_send![view, className];
                    if !class_name.is_null() {
                        let class_str: *const std::ffi::c_char = objc2::msg_send![class_name, UTF8String];
                        if !class_str.is_null() {
                            let name = std::ffi::CStr::from_ptr(class_str).to_string_lossy();
                            // Match WryWebView specifically (not WryWebViewParent)
                            // WryWebView is the actual WKWebView subclass in wry/tauri
                            if name == "WryWebView" || name == "WKWebView" {
                                eprintln!("[powerpaste] found webview: {}", name);
                                return Some(view);
                            }
                        }
                    }
                    let subviews: *const AnyObject = objc2::msg_send![view, subviews];
                    if !subviews.is_null() {
                        let count: usize = objc2::msg_send![subviews, count];
                        for i in 0..count {
                            let subview: *mut AnyObject = objc2::msg_send![subviews, objectAtIndex: i];
                            if let Some(wv) = find_wry_webview(subview, depth + 1) { return Some(wv); }
                        }
                    }
                    None
                }
            }
            let cv_ptr = &*content_view as *const _ as *mut AnyObject;
            if let Some(webview) = find_wry_webview(cv_ptr, 0) {
                unsafe {
                    let wv_view: *const AnyObject = webview as *const AnyObject;
                    let _: bool = objc2::msg_send![&*panel, makeFirstResponder: wv_view];
                    eprintln!("[powerpaste] (first show) made WryWebView first responder");
                }
            } else {
                // Fallback to content view
                panel.makeFirstResponder(Some(&content_view));
                eprintln!("[powerpaste] (first show) WryWebView not found, using content view");
            }
        }
    }))
    .map_err(|e| format!("objective-c exception showing panel: {e:?}"))?;

    // Restore alpha now that frame is committed
    unsafe {
        let _: () = objc2::msg_send![&*panel, setAlphaValue: 1.0f64];
    }

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
    // Install mouse focus monitor to handle left-clicks for text input
    macos_install_mouse_focus_monitor();

    // Notify frontend that panel is shown so it can focus the search input
    let _ = window.emit(FRONTEND_EVENT_PANEL_SHOWN, ());

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

    let ui_mode = get_current_ui_mode();
    let (width, height) = overlay_size_for_monitor(size.width, size.height, ui_mode);

    // On Windows, use the work area (excludes taskbar on any edge) for positioning.
    // On macOS, use full screen bounds (NSPanel floats above the Dock).
    #[cfg(target_os = "windows")]
    let (x, y) = {
        use windows::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPI_GETWORKAREA};
        use windows::Win32::Foundation::RECT;
        let mut work_area = RECT::default();
        let got_work_area = unsafe {
            SystemParametersInfoW(
                SPI_GETWORKAREA,
                0,
                Some(&mut work_area as *mut RECT as *mut _),
                Default::default(),
            )
        };
        if got_work_area.is_ok() {
            let wa_width = (work_area.right - work_area.left) as u32;
            let x = work_area.left + ((wa_width.saturating_sub(width)) / 2) as i32;
            let y = work_area.bottom - height as i32;
            (x, y)
        } else {
            let x = pos.x + ((size.width.saturating_sub(width)) / 2) as i32;
            let y = pos.y + (size.height.saturating_sub(height)) as i32;
            (x, y)
        }
    };
    #[cfg(not(target_os = "windows"))]
    let (x, y) = {
        let x = pos.x + ((size.width.saturating_sub(width)) / 2) as i32;
        let y = pos.y + (size.height.saturating_sub(height)) as i32;
        (x, y)
    };

    window
        .set_size(tauri::Size::Physical(tauri::PhysicalSize { width, height }))
        .map_err(|e| format!("failed to set window size: {e}"))?;
    window
        .set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }))
        .map_err(|e| format!("failed to set window position: {e}"))?;
    Ok(())
}

/// Position the window near the cursor for floating mode (non-macOS).
#[cfg(not(target_os = "macos"))]
fn position_floating_near_cursor<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) -> Result<(), String> {
    window
        .set_always_on_top(true)
        .map_err(|e| format!("failed to set always-on-top: {e}"))?;

    platform::configure_floating_window(window);

    let monitor = window
        .current_monitor()
        .map_err(|e| format!("failed to get current monitor: {e}"))?
        .or_else(|| window.primary_monitor().ok().flatten())
        .ok_or_else(|| "no monitor found".to_string())?;

    let mon_size = monitor.size();
    let mon_pos = monitor.position();
    let scale = monitor.scale_factor();

    let (width, height) = overlay_size_for_monitor(mon_size.width, mon_size.height, models::UiMode::Floating);

    window
        .set_size(tauri::Size::Physical(tauri::PhysicalSize { width, height }))
        .map_err(|e| format!("failed to set window size: {e}"))?;

    // Try to position below the cursor
    if let Some((cx, cy)) = platform::get_cursor_position() {
        let cx = cx as i32;
        let cy = cy as i32;
        let mut x = cx;
        // 10 logical pixels below cursor
        let mut y = cy + (10.0 * scale) as i32;

        let mon_right = mon_pos.x + mon_size.width as i32;
        let mon_bottom = mon_pos.y + mon_size.height as i32;

        // Keep within monitor bounds
        if x + width as i32 > mon_right {
            x = mon_right - width as i32;
        }
        if x < mon_pos.x {
            x = mon_pos.x;
        }
        if y + height as i32 > mon_bottom {
            // If no room below cursor, show above
            y = cy - height as i32 - (10.0 * scale) as i32;
        }
        if y < mon_pos.y {
            y = mon_pos.y;
        }

        window
            .set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }))
            .map_err(|e| format!("failed to set window position: {e}"))?;
        return Ok(());
    }

    // Fallback: center on screen
    let x = mon_pos.x + ((mon_size.width.saturating_sub(width)) / 2) as i32;
    let y = mon_pos.y + ((mon_size.height.saturating_sub(height)) / 2) as i32;
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
                // Skip dev tools that shouldn't be paste targets.
                eprintln!("[powerpaste] (hotkey) querying frontmost app...");
                if let Some(name) = platform::query_frontmost_app_name() {
                    eprintln!("[powerpaste] (hotkey) frontmost app: {}", name);
                    let skip_apps = ["node", "PowerPaste", "Code Helper"];
                    if !skip_apps.iter().any(|s| name.contains(s)) {
                        eprintln!("[powerpaste] (hotkey) recording frontmost app: {}", name);
                        platform::set_last_frontmost_app_name(name);
                    } else {
                        eprintln!("[powerpaste] (hotkey) skipping (dev tool): {}", name);
                    }
                } else {
                    eprintln!("[powerpaste] (hotkey) no frontmost app found");
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
                    let ui_mode = get_current_ui_mode();
                    let (w, h) = overlay_size_for_monitor(
                        screen_frame.size.width.max(1.0).round() as u32,
                        screen_frame.size.height.max(1.0).round() as u32,
                        ui_mode,
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

#[cfg(all(desktop, target_os = "macos"))]
fn setup_tray(app: &tauri::App) -> Result<(), String> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    use tauri::Manager;

    let toggle_item = MenuItem::with_id(app, "tray_toggle", "Show/Hide", true, None::<String>)
        .map_err(|e| format!("failed to create tray menu item: {e}"))?;
    let quit_item = MenuItem::with_id(app, "tray_quit", "Exit", true, None::<String>)
        .map_err(|e| format!("failed to create tray menu item: {e}"))?;
    let sep = PredefinedMenuItem::separator(app)
        .map_err(|e| format!("failed to create tray menu separator: {e}"))?;

    let menu = Menu::with_items(app, &[&toggle_item, &sep, &quit_item])
        .map_err(|e| format!("failed to create tray menu: {e}"))?;

    let mut builder = TrayIconBuilder::with_id("powerpaste_tray")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "tray_toggle" => {
                let _ = toggle_main_window_wry(app);
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
                if let Err(e) = toggle_main_window_wry(tray.app_handle()) {
                    eprintln!("[powerpaste] tray toggle failed: {e}");
                }
            }
        })
        .tooltip("PowerPaste");

    // Load dedicated tray template icon (44x44 for Retina/HiDPI, handled as template)
    // Decode PNG to raw RGBA bytes for tauri::image::Image
    let tray_icon_bytes = include_bytes!("../icons/tray-icon.png");
    if let Ok(img) = image::load_from_memory(tray_icon_bytes) {
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        let icon = tauri::image::Image::new_owned(rgba.into_raw(), width, height);
        builder = builder.icon(icon).icon_as_template(true);
    } else if let Some(icon) = app.app_handle().default_window_icon() {
        // Fallback to default window icon if tray icon fails to load
        builder = builder.icon(icon.clone()).icon_as_template(true);
    }

    builder
        .build(app)
        .map_err(|e| format!("failed to build tray icon: {e}"))?;
    Ok(())
}

#[cfg(all(desktop, not(target_os = "macos")))]
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
    // Install a global exception handler to catch the TouchBar KVO exception
    // BEFORE any Objective-C code runs, by setting an uncaught exception handler
    #[cfg(target_os = "macos")]
    {
        install_touchbar_exception_handler();
    }
    
    let mut builder = tauri::Builder::default()
        .manage(AppState {
            watcher: Mutex::new(None),
        });
    
    // Register tauri-nspanel plugin for macOS panel support
    #[cfg(target_os = "macos")]
    {
        builder = builder.plugin(tauri_nspanel::init());
    }
    
    builder
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
                    
                    // Disable TouchBar UI at app level to prevent KVO observer crashes
                    // when windows are converted to NSPanel
                    disable_touchbar_globally(&ns_app);
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
                
                // Apply saved launch at startup setting
                let autostart_manager = handle.autolaunch();
                if settings.launch_at_startup {
                    let _ = autostart_manager.enable();
                } else {
                    let _ = autostart_manager.disable();
                }
            }

            // Start hidden; the global hotkey toggles the UI.
            // CRITICAL: Convert window to panel IMMEDIATELY before any show operation.
            // If we show the window first, macOS TouchBar system registers KVO observers
            // that become orphaned when the window is converted to NSPanel, causing crashes.
            if let Some(window) = app.get_webview_window("main") {
                #[cfg(target_os = "macos")]
                {
                    // Convert to panel BEFORE any show - this prevents TouchBar observer issues
                    if let Err(e) = convert_window_to_panel_early(&handle, &window) {
                        eprintln!("[powerpaste] early panel conversion failed: {e}");
                    }
                }

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
                                // If NSPanel overlay is in use, hide it instead of the Tauri window
                                if OVERLAY_PANEL_PTR.get().is_some() {
                                    let app = window_for_event.app_handle();
                                    let _ = macos_hide_overlay_panel_if_visible(&app);
                                } else {
                                    if let Err(e) = macos_set_overlay_window_active(&window_for_event, false) {
                                        eprintln!("[powerpaste] macos overlay deactivate failed: {e}");
                                    }
                                    let _ = window_for_event.hide();
                                }
                            }
                            #[cfg(not(target_os = "macos"))]
                            {
                                let _ = window_for_event.hide();
                            }
                        }
                        tauri::WindowEvent::Focused(false) => {
                            eprintln!("[powerpaste] WindowEvent::Focused(false) received");
                            // When using the NSPanel overlay, ignore Tauri window focus events.
                            // The NSPanel has its own click-outside monitor to detect when to hide.
                            #[cfg(target_os = "macos")]
                            {
                                if OVERLAY_PANEL_PTR.get().is_some() {
                                    // NSPanel is handling its own visibility; ignore this event
                                    eprintln!("[powerpaste] ignoring focus event because NSPanel exists");
                                    return;
                                }
                                if let Err(e) = macos_set_overlay_window_active(&window_for_event, false) {
                                    eprintln!("[powerpaste] macos overlay deactivate failed: {e}");
                                }
                            }
                            // Update atomic flag when window loses focus and hides
                            IS_PANEL_VISIBLE.store(false, std::sync::atomic::Ordering::SeqCst);
                            let _ = window_for_event.hide();
                        }
                        _ => {}
                    }
                });
            }

            #[cfg(desktop)]
            {
                match setup_tray(app) {
                    Ok(_) => eprintln!("[powerpaste] tray icon setup successful"),
                    Err(e) => eprintln!("[powerpaste] tray icon setup FAILED: {e}"),
                }
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
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::AppleScript, Some(vec!["--hidden"])))
        // Disable browser accelerator keys (Ctrl+Shift+V, Ctrl+F, etc.) on Windows
        // so all key events pass through to JS. Applied to every webview on page load.
        .on_page_load(|webview, _payload| {
            #[cfg(target_os = "windows")]
            {
                let label = webview.label().to_string();
                let _ = webview.with_webview(move |wv| {
                    unsafe {
                        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
                        use windows::core::Interface;
                        let controller = wv.controller();
                        if let Ok(core) = controller.CoreWebView2() {
                            if let Ok(settings) = core.Settings() {
                                if let Ok(settings3) = settings.cast::<ICoreWebView2Settings3>() {
                                    let _ = settings3.SetAreBrowserAcceleratorKeysEnabled(false.into());
                                    eprintln!("[powerpaste] disabled browser accelerator keys for '{label}'");
                                }
                            }
                        }
                    }
                });
            }
            let _ = _payload;
        })
        // Note: tauri_plugin_opener removed - it was interfering with double-clicks
        .invoke_handler(tauri::generate_handler![
            get_settings,
            set_hotkey,
            set_sync_settings,
            set_ui_mode,
            set_theme,
            set_history_retention,
            set_trash_enabled,
            set_trash_retention,
            connect_sync_provider,
            disconnect_sync_provider,
            list_items,
            list_items_paginated,
            list_trashed_items,
            list_pinboard_items_paginated,
            get_trash_count,
            restore_from_trash,
            delete_item_forever,
            touch_item,
            empty_trash,
            get_image_data,
            get_app_icon_path,
            set_overlay_preferred_size,
            hide_main_window,
            close_window_by_label,
            set_item_pinned,
            set_item_pinboard,
            list_pinboards,
            check_file_exists,
            delete_item,
            enable_mouse_events,
            write_clipboard_text,
            write_clipboard_files,
            paste_text,
            paste_item,
            check_permissions,
            open_accessibility_settings,
            open_automation_settings,
            request_accessibility_permission,
            request_automation_permission,
            sync_now,
            set_show_dock_icon,
            set_launch_at_startup,
            get_system_accent_color,
            suspend_hotkey,
            resume_hotkey
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            #[allow(clippy::single_match)]
            match event {
                tauri::RunEvent::ExitRequested { .. } => {
                    // Clean up NSPanel before exit to prevent TouchBar KVO observer crash
                    #[cfg(target_os = "macos")]
                    {
                        cleanup_nspanel_before_exit(app);
                    }
                    #[cfg(not(target_os = "macos"))]
                    { let _ = app; } // Suppress unused warning on non-macOS
                }
                _ => {}
            }
        });
}

/// Clean up the NSPanel before the app exits to prevent TouchBar KVO observer crash.
/// The crash occurs because macOS TouchBar system registers observers on NSWindow,
/// and when the window is destroyed without properly removing these observers,
/// it throws an exception: "Cannot remove observer for key path from object
/// because it is not registered as an observer."
#[cfg(target_os = "macos")]
fn cleanup_nspanel_before_exit(app: &tauri::AppHandle) {
    use objc2::exception::catch;
    use std::panic::AssertUnwindSafe;
    use tauri_nspanel::ManagerExt as NspanelManagerExt;
    
    eprintln!("[powerpaste] cleaning up NSPanel before exit");
    
    // Try to get and hide the panel with exception catching
    if let Ok(panel) = app.get_webview_panel("main") {
        // Hide the panel first (with exception catching)
        {
            let panel_clone = panel.clone();
            let result = catch(AssertUnwindSafe(move || {
                panel_clone.hide();
            }));
            if let Err(e) = result {
                eprintln!("[powerpaste] caught objc exception during panel hide: {:?}", e);
            }
        };
        
        // Clone for use inside closure
        let panel_clone = panel.clone();
        
        // Wrap panel cleanup in exception catcher to prevent crash
        // This catches any Objective-C exceptions thrown during cleanup
        let result = catch(AssertUnwindSafe(|| {
            // Set event handler to None to remove any delegate callbacks
            panel_clone.set_event_handler(None);
        }));
        
        if let Err(e) = result {
            eprintln!("[powerpaste] caught objc exception during panel cleanup: {:?}", e);
        }
    }
    
    eprintln!("[powerpaste] NSPanel cleanup complete");
}
