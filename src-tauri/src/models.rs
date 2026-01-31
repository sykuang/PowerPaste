use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardItem {
    pub id: Uuid,
    pub kind: ClipboardItemKind,
    pub text: String,
    pub created_at_ms: i64,
    pub pinned: bool,
    /// Optional category name for user-created tabs (e.g., "Work Links", "API Keys")
    #[serde(default)]
    pub pin_category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipboardItemKind {
    Text,
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
