use crate::db;
use arboard::Clipboard;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

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

            loop {
                if *stop_flag_thread.lock().unwrap_or_else(|e| e.into_inner()) {
                    break;
                }

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

                match db::insert_text_if_new(&app, &text) {
                    Ok(Some(item)) => {
                        last_text = Some(text);
                        let _ = app.emit("powerpaste://new_item", item);
                    }
                    Ok(None) => {
                        last_text = Some(text);
                    }
                    Err(_) => {
                        // ignore transient DB errors
                    }
                }

                std::thread::sleep(Duration::from_millis(500));
            }
        });

        Self { stop_flag }
    }

    pub fn stop(&self) {
        if let Ok(mut guard) = self.stop_flag.lock() {
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
