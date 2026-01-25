mod clipboard;
mod db;
mod models;
mod paths;
mod settings_store;
mod sync;

use models::{ClipboardItem, Settings, SyncProvider};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use uuid::Uuid;

const OVERLAY_HEIGHT_PX: u32 = 260;

struct AppState {
    watcher: Mutex<Option<clipboard::ClipboardWatcher>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SyncNowResult {
    imported: u32,
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
) -> Result<Settings, String> {
    let settings = settings_store::load_or_init_settings(&app)?;

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
fn list_items(app: tauri::AppHandle, limit: u32, query: Option<String>) -> Result<Vec<ClipboardItem>, String> {
    db::list_items(&app, limit, query)
}

#[tauri::command]
fn set_item_pinned(app: tauri::AppHandle, id: String, pinned: bool) -> Result<(), String> {
    let id = Uuid::parse_str(&id).map_err(|_| "invalid id".to_string())?;
    db::set_pinned(&app, id, pinned)
}

#[tauri::command]
fn delete_item(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let id = Uuid::parse_str(&id).map_err(|_| "invalid id".to_string())?;
    db::delete_item(&app, id)
}

#[tauri::command]
fn write_clipboard_text(text: String) -> Result<(), String> {
    clipboard::set_clipboard_text(&text)
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
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicBool, Ordering};

    static OVERLAY_PANEL_PTR: OnceLock<usize> = OnceLock::new();
    static PANEL_INIT_RETRY_SCHEDULED: AtomicBool = AtomicBool::new(false);

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
        // SAFETY: We store a +1 retained pointer, and only use it on main thread.
        let panel: Retained<NSPanel> = unsafe {
            Retained::retain((*stored as *mut NSPanel).cast()).ok_or("failed to retain NSPanel")?
        };

        let is_visible = panel.isVisible();
        if is_visible {
            exception::catch(std::panic::AssertUnwindSafe(|| {
                panel.orderOut(None);
            }))
            .map_err(|e| format!("objective-c exception hiding panel: {e:?}"))?;
            eprintln!("[powerpaste] macos overlay panel hide");
            return Ok(());
        }

        // Show: activate app (helps on some systems), then order front.
        let app = NSApplication::sharedApplication(mtm);
        let _ = app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        app.activateIgnoringOtherApps(true);

        // Recompute frame each show (handles display changes, fullscreen spaces, etc.).
        let screen = panel
            .screen()
            .or_else(|| NSScreen::mainScreen(mtm))
            .ok_or("no screen found")?;
        let screen_frame: NSRect = screen.frame();
        let mut target = screen_frame;
        target.size.height = (OVERLAY_HEIGHT_PX as f64).min(screen_frame.size.height);
        target.origin.x = screen_frame.origin.x;
        target.origin.y = screen_frame.origin.y;
        target.size.width = screen_frame.size.width;

        exception::catch(std::panic::AssertUnwindSafe(|| {
            panel.setLevel(NSScreenSaverWindowLevel);
            panel.setFrame_display(target, true);
            panel.orderFrontRegardless();
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
    let mut target = screen_frame;
    target.size.height = (OVERLAY_HEIGHT_PX as f64).min(screen_frame.size.height);
    target.origin.x = screen_frame.origin.x;
    target.origin.y = screen_frame.origin.y;

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
    panel.setBecomesKeyOnlyIfNeeded(true);

    // Keep a retained pointer around for future toggles.
    let raw = Retained::as_ptr(&panel) as usize;
    let _ = OVERLAY_PANEL_PTR.set(raw);

    // First show.
    let app = NSApplication::sharedApplication(mtm);
    let _ = app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    app.activateIgnoringOtherApps(true);

    exception::catch(std::panic::AssertUnwindSafe(|| {
        panel.setLevel(NSScreenSaverWindowLevel);
        panel.setFrame_display(target, true);
        panel.orderFrontRegardless();
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

    Ok(())
}

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

    // Use the full monitor bounds so the overlay can cover the Dock when needed.
    // (If you prefer staying above the Dock, switch to monitor.work_area() instead.)
    let size = monitor.size();
    let pos = monitor.position();

    let height = OVERLAY_HEIGHT_PX.min(size.height);
    let width = size.width;
    let x = pos.x;
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
        let panel_style = (style | NSWindowStyleMask::NonactivatingPanel | NSWindowStyleMask::FullSizeContentView)
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
                // In practice, showing above another app's fullscreen Space is much more
                // reliable when our app is activated.
                let app = NSApplication::sharedApplication(mtm);

                // Try to behave like an "agent" app (Paste-like). This can influence
                // how windows participate in fullscreen spaces.
                let _ = app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

                app.activateIgnoringOtherApps(true);

                let screen = ns_window
                    .screen()
                    .or_else(|| NSScreen::mainScreen(mtm));

                if let Some(screen) = screen {
                    let screen_frame: NSRect = screen.frame();
                    let mut target = screen_frame;
                    target.size.height = (OVERLAY_HEIGHT_PX as f64).min(screen_frame.size.height);
                    target.origin.y = screen_frame.origin.y;
                    target.origin.x = screen_frame.origin.x;
                    target.size.width = screen_frame.size.width;
                    ns_window.setFrame_display(target, true);
                }
            }

            // Ensure the window is ordered above everything else at its level.
            ns_window.orderFrontRegardless();
        } else {
            ns_window.setLevel(NSNormalWindowLevel);
        }
    }))
    .map_err(|e| format!("objective-c exception setting window level: {e:?}"))?;

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
    let quit_item = MenuItem::with_id(manager, "tray_quit", "Quit", true, None::<String>)
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            watcher: Mutex::new(None),
        })
        .setup(|app| {
            let handle = app.handle().clone();

            // Initialize settings early.
            if let Ok(settings) = settings_store::load_or_init_settings(&handle) {
                if let Err(e) = register_hotkey(&handle, &settings.hotkey) {
                    eprintln!("[powerpaste] failed to register hotkey '{}': {e}", settings.hotkey);
                }
            }

            // Start hidden; the global hotkey toggles the UI.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();

                let window_for_event = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        #[cfg(target_os = "macos")]
                        {
                            if let Err(e) = macos_set_overlay_window_active(&window_for_event, false) {
                                eprintln!("[powerpaste] macos overlay deactivate failed: {e}");
                            }
                        }
                        let _ = window_for_event.hide();
                    }
                });
            }

            #[cfg(desktop)]
            {
                let _ = setup_tray(app);
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
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_settings,
            set_hotkey,
            set_sync_settings,
            list_items,
            set_item_pinned,
            delete_item,
            write_clipboard_text,
            sync_now
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
