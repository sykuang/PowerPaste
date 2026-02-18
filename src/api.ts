import { invoke } from "@tauri-apps/api/core";

export type SyncProvider =
  | "icloud_drive"
  | "one_drive"
  | "google_drive"
  | "custom_folder";

export type Theme = "light" | "dark" | "system";

export type UiMode = "floating" | "fixed";

export type ClipboardItemKind = "text" | "image" | "file";

export type Settings = {
  device_id: string;
  sync_enabled: boolean;
  sync_provider: SyncProvider | null;
  sync_folder: string | null;
  sync_salt_b64: string | null;
  hotkey: string;
  theme: Theme;
  ui_mode: UiMode;
  /** macOS only: show app icon in Dock (default false = menu bar app only) */
  show_dock_icon: boolean;
  /** History retention in days (null = forever) - synced across devices */
  history_retention_days: number | null;
  /** Whether trash bin is enabled (synced across devices) */
  trash_enabled: boolean;
  /** Trash retention in days (null = forever) - synced across devices */
  trash_retention_days: number | null;
  /** Connected OAuth providers with account info */
  connected_providers: ConnectedProviderInfo[];
  /** Whether to launch the app on system startup */
  launch_at_startup: boolean;
};

export type ConnectedProviderInfo = {
  provider: SyncProvider;
  account_email: string;
  account_id: string;
};

export type ClipboardItem = {
  id: string;
  kind: ClipboardItemKind;
  text: string;
  created_at_ms: number;
  pinned: boolean;
  /** Optional pinboard name for user-created pinboards */
  pinboard: string | null;
  /** For image items: width in pixels */
  image_width?: number;
  /** For image items: height in pixels */
  image_height?: number;
  /** For image items: size in bytes */
  image_size_bytes?: number;
  /** For image items: original MIME type (e.g., image/jpeg) */
  image_mime?: string;
  /** For file items: file path(s) separated by newlines */
  file_paths?: string;
  /** Content type hint for preview: "url", "image", "file", or null for plain text */
  content_type?: string;
  /** Name of the app that was frontmost when this item was copied */
  source_app_name?: string;
  /** Bundle ID of the source app (e.g., "com.apple.Safari") */
  source_app_bundle_id?: string;
  /** Whether the item is in trash */
  is_trashed?: boolean;
  /** Timestamp when the item was moved to trash (ms since epoch) */
  deleted_at_ms?: number;
};

export type PermissionsStatus = {
  platform: "macos" | "windows" | "linux" | "unknown";
  can_paste: boolean;
  automation_ok: boolean;
  accessibility_ok: boolean;
  details: string | null;
  /** Whether running as a bundled .app (true) or dev binary (false) */
  is_bundled: boolean;
  /** The path to the executable that needs permissions */
  executable_path: string;
};

export async function getSettings(): Promise<Settings> {
  return invoke("get_settings");
}

export async function setSyncSettings(args: {
  enabled: boolean;
  provider: SyncProvider | null;
  folder: string | null;
  passphrase?: string | null;
  theme?: Theme;
}): Promise<Settings> {
  return invoke("set_sync_settings", {
    enabled: args.enabled,
    provider: args.provider,
    folder: args.folder,
    passphrase: args.passphrase ?? null,
    theme: args.theme ?? null,
  });
}

export async function setHotkey(hotkey: string): Promise<Settings> {
  return invoke("set_hotkey", { hotkey });
}

export async function setUiMode(uiMode: UiMode): Promise<Settings> {
  return invoke("set_ui_mode", { uiMode });
}

export async function listItems(args: {
  limit: number;
  query?: string;
}): Promise<ClipboardItem[]> {
  return invoke("list_items", {
    limit: args.limit,
    query: args.query ?? null,
  });
}

/**
 * Get the image data as a base64 data URL for an image clipboard item.
 * Returns null if the item is not an image or has no stored data.
 */
export async function getImageData(id: string): Promise<string | null> {
  return invoke("get_image_data", { id });
}

export async function setItemPinned(id: string, pinned: boolean): Promise<void> {
  return invoke("set_item_pinned", { id, pinned });
}

export async function setItemPinboard(id: string, pinboard: string | null): Promise<void> {
  return invoke("set_item_pinboard", { id, pinboard });
}

export async function listPinboards(): Promise<string[]> {
  return invoke("list_pinboards");
}

/**
 * Check if a file path exists on the filesystem.
 * Used to determine whether to show file preview or treat as text.
 */
export async function checkFileExists(path: string): Promise<boolean> {
  return invoke("check_file_exists", { path });
}

export async function deleteItem(id: string): Promise<void> {
  return invoke("delete_item", { id });
}

export async function writeClipboardText(text: string): Promise<void> {
  return invoke("write_clipboard_text", { text });
}

export async function writeClipboardFiles(paths: string[]): Promise<void> {
  return invoke("write_clipboard_files", { paths });
}

export async function pasteText(text: string): Promise<void> {
  return invoke("paste_text", { text });
}

export async function pasteItem(id: string): Promise<void> {
  return invoke("paste_item", { id });
}

export async function checkPermissions(): Promise<PermissionsStatus> {
  return invoke("check_permissions");
}

export async function openAccessibilitySettings(): Promise<void> {
  return invoke("open_accessibility_settings");
}

export async function openAutomationSettings(): Promise<void> {
  return invoke("open_automation_settings");
}

/** Trigger the macOS Accessibility permission prompt (auto-adds app to list). Returns current trusted status. */
export async function requestAccessibilityPermission(): Promise<boolean> {
  return invoke("request_accessibility_permission");
}

/** Trigger the macOS Automation permission prompt via test osascript. Returns true if already granted. */
export async function requestAutomationPermission(): Promise<boolean> {
  return invoke("request_automation_permission");
}

export async function syncNow(): Promise<{ imported: number }> {
  return invoke("sync_now");
}

export async function setOverlayPreferredSize(args: { width: number; height: number }): Promise<void> {
  return invoke("set_overlay_preferred_size", {
    width: args.width,
    height: args.height,
  });
}

export async function hideMainWindow(): Promise<void> {
  // Log stack trace to identify the caller
  const stack = new Error().stack;
  console.log("[powerpaste] hideMainWindow called from:", stack);
  try {
    await invoke("hide_main_window");
    console.log("[powerpaste] hideMainWindow invoke completed");
  } catch (e) {
    console.error("[powerpaste] hideMainWindow error:", e);
    throw e;
  }
}

export async function closeWindowByLabel(label: string): Promise<void> {
  console.log("[powerpaste] closeWindowByLabel called with label:", label);
  try {
    await invoke("close_window_by_label", { label });
    console.log("[powerpaste] closeWindowByLabel completed");
  } catch (e) {
    console.error("[powerpaste] closeWindowByLabel error:", e);
    throw e;
  }
}

export async function enableMouseEvents(): Promise<void> {
  return invoke("enable_mouse_events");
}

export async function setShowDockIcon(show: boolean): Promise<Settings> {
  return invoke("set_show_dock_icon", { show });
}

export async function setLaunchAtStartup(enabled: boolean): Promise<Settings> {
  return invoke("set_launch_at_startup", { enabled });
}

/** Set the theme (light, dark, or system) */
export async function setTheme(theme: Theme): Promise<Settings> {
  return invoke("set_theme", { theme });
}

/** Set history retention in days (null = forever) - synced across devices */
export async function setHistoryRetention(days: number | null): Promise<Settings> {
  return invoke("set_history_retention", { days });
}

/** Enable or disable trash bin - synced across devices */
export async function setTrashEnabled(enabled: boolean): Promise<Settings> {
  return invoke("set_trash_enabled", { enabled });
}

/** Set trash retention in days (null = forever) - synced across devices */
export async function setTrashRetention(days: number | null): Promise<Settings> {
  return invoke("set_trash_retention", { days });
}

/** Result of connecting a sync provider via OAuth */
export type ConnectedProviderResult = {
  provider: SyncProvider;
  accountEmail: string;
  accountId: string;
};

/** Connect a sync provider via OAuth */
export async function connectSyncProvider(provider: SyncProvider): Promise<ConnectedProviderResult> {
  return invoke("connect_sync_provider", { provider });
}

/** Disconnect a sync provider */
export async function disconnectSyncProvider(provider: SyncProvider): Promise<void> {
  return invoke("disconnect_sync_provider", { provider });
}

/** List clipboard items with pagination */
export async function listItemsPaginated(args: {
  limit: number;
  offset: number;
  query?: string;
  includeTrashed?: boolean;
}): Promise<{ items: ClipboardItem[]; total: number }> {
  return invoke("list_items_paginated", {
    limit: args.limit,
    offset: args.offset,
    query: args.query ?? null,
    includeTrashed: args.includeTrashed ?? false,
  });
}

/** List trashed items with pagination */
export async function listTrashedItems(args: {
  limit: number;
  offset: number;
}): Promise<{ items: ClipboardItem[]; total: number }> {
  return invoke("list_trashed_items", {
    limit: args.limit,
    offset: args.offset,
  });
}

/** Get the count of items in trash */
export async function getTrashCount(): Promise<number> {
  return invoke("get_trash_count");
}

/** Restore an item from trash */
export async function restoreFromTrash(id: string): Promise<void> {
  return invoke("restore_from_trash", { id });
}

/** Permanently delete an item (bypass trash) */
export async function deleteItemForever(id: string): Promise<void> {
  return invoke("delete_item_forever", { id });
}

/** Move an item to the top of the list by updating its timestamp */
export async function touchItem(id: string): Promise<boolean> {
  return invoke("touch_item", { id });
}

/** Empty the trash (permanently delete all trashed items) */
export async function emptyTrash(): Promise<void> {
  return invoke("empty_trash");
}

/** List pinboard items with pagination */
export async function listPinboardItemsPaginated(args: {
  limit: number;
  offset: number;
  pinboard?: string | null;
}): Promise<{ items: ClipboardItem[]; total: number }> {
  return invoke("list_pinboard_items_paginated", {
    limit: args.limit,
    offset: args.offset,
    pinboard: args.pinboard ?? null,
  });
}

/** Get the file system path to an app's icon by its bundle ID */
export async function getAppIconPath(bundleId: string): Promise<string | null> {
  return invoke("get_app_icon_path", { bundleId });
}
