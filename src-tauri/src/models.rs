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
    /// For image items: original MIME type (e.g., image/jpeg)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_mime: Option<String>,
    /// For file items: file path(s) separated by newlines
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_paths: Option<String>,
    /// Content type hint for preview: "url", "image", "file", or null for plain text
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// Name of the app that was frontmost when this item was copied
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_app_name: Option<String>,
    /// Bundle ID of the source app (e.g., "com.apple.Safari")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_app_bundle_id: Option<String>,
    /// Whether this item is in the trash
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_trashed: Option<bool>,
    /// Timestamp when the item was moved to trash (ms since epoch)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_at_ms: Option<i64>,
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
    /// History retention in days (None = forever) - synced across devices
    #[serde(default)]
    pub history_retention_days: Option<i32>,
    /// Whether trash bin is enabled (default true) - synced across devices
    #[serde(default = "default_trash_enabled")]
    pub trash_enabled: bool,
    /// Trash retention in days (None = forever, default 30) - synced across devices
    #[serde(default = "default_trash_retention")]
    pub trash_retention_days: Option<i32>,
    /// Connected OAuth providers with account info
    #[serde(default)]
    pub connected_providers: Vec<ConnectedProviderInfo>,
    /// Whether to launch the app on system startup
    #[serde(default)]
    pub launch_at_startup: bool,
}

fn default_trash_enabled() -> bool {
    true
}

fn default_trash_retention() -> Option<i32> {
    Some(30)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedProviderInfo {
    pub provider: SyncProvider,
    pub account_email: String,
    pub account_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SyncProvider {
    IcloudDrive,
    OneDrive,
    GoogleDrive,
    CustomFolder,
}
