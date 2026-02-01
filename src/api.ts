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
  /** For file items: file path(s) separated by newlines */
  file_paths?: string;
  /** Content type hint for preview: "url", "image", "file", or null for plain text */
  content_type?: string;
  /** Name of the app that was frontmost when this item was copied */
  source_app_name?: string;
  /** Bundle ID of the source app (e.g., "com.apple.Safari") */
  source_app_bundle_id?: string;
};

export type PermissionsStatus = {
  platform: "macos" | "windows" | "linux" | "unknown";
  can_paste: boolean;
  automation_ok: boolean;
  accessibility_ok: boolean;
  details: string | null;
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

export async function deleteItem(id: string): Promise<void> {
  return invoke("delete_item", { id });
}

export async function writeClipboardText(text: string): Promise<void> {
  return invoke("write_clipboard_text", { text });
}

export async function pasteText(text: string): Promise<void> {
  return invoke("paste_text", { text });
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

export async function enableMouseEvents(): Promise<void> {
  return invoke("enable_mouse_events");
}

export async function setShowDockIcon(show: boolean): Promise<Settings> {
  return invoke("set_show_dock_icon", { show });
}
/** Get the file system path to an app's icon by its bundle ID */
export async function getAppIconPath(bundleId: string): Promise<string | null> {
  return invoke("get_app_icon_path", { bundleId });
}