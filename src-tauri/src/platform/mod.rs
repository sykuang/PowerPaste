//! Platform-specific implementations.
//!
//! Each platform module exports the same public interface so call sites
//! don't need `#[cfg]` attributes.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::*;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod linux;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub use linux::*;

use arboard::{Clipboard, ImageData};
use std::borrow::Cow;

/// Decode encoded image bytes and set on clipboard via arboard (shared fallback).
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
