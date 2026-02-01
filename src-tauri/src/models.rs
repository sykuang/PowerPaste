use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardItem {
    pub id: Uuid,
    pub kind: ClipboardItemKind,
    pub text: String,
    pub created_at_ms: i64,
    pub pinned: bool,
    /// Optional pinboard name for user-created pinboards (e.g., "Work Links", "API Keys")
    /// Field is named `pin_category` for backward compatibility, aliased to `pinboard` in JSON
    #[serde(default, alias = "pin_category")]
    pub pinboard: Option<String>,
    /// For image items: width in pixels
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_width: Option<u32>,
    /// For image items: height in pixels
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_height: Option<u32>,
    /// For image items: size in bytes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_size_bytes: Option<u64>,
    /// For file items: file path(s) separated by newlines
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_paths: Option<String>,
    /// Content type hint for preview: "url", "image", "file", or null for plain text
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ClipboardItemKind {
    Text,
    Image,
    File,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum UiMode {
    /// Floating UI near cursor position
    #[default]
    Floating,
    /// Fixed UI at the bottom of the screen
    Fixed,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    pub device_id: String,
    pub sync_enabled: bool,
    pub sync_provider: Option<SyncProvider>,
    pub sync_folder: Option<String>,
    pub sync_salt_b64: Option<String>,
    #[serde(default)]
    pub hotkey: String,
    #[serde(default)]
    pub theme: String,
    #[serde(default)]
    pub ui_mode: UiMode,
    /// macOS only: show app icon in Dock (default false = menu bar app only)
    #[serde(default)]
    pub show_dock_icon: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncProvider {
    IcloudDrive,
    OneDrive,
    GoogleDrive,
    CustomFolder,
}
