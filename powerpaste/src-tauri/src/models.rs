use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardItem {
    pub id: Uuid,
    pub kind: ClipboardItemKind,
    pub text: String,
    pub created_at_ms: i64,
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipboardItemKind {
    Text,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncProvider {
    IcloudDrive,
    OneDrive,
    GoogleDrive,
    CustomFolder,
}
