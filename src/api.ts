import { invoke } from "@tauri-apps/api/core";

export type SyncProvider =
  | "icloud_drive"
  | "one_drive"
  | "google_drive"
  | "custom_folder";

export type Settings = {
  device_id: string;
  sync_enabled: boolean;
  sync_provider: SyncProvider | null;
  sync_folder: string | null;
  sync_salt_b64: string | null;
  hotkey: string;
};

export type ClipboardItem = {
  id: string;
  kind: "text";
  text: string;
  created_at_ms: number;
  pinned: boolean;
};

export async function getSettings(): Promise<Settings> {
  return invoke("get_settings");
}

export async function setSyncSettings(args: {
  enabled: boolean;
  provider: SyncProvider | null;
  folder: string | null;
  passphrase?: string | null;
}): Promise<Settings> {
  return invoke("set_sync_settings", {
    enabled: args.enabled,
    provider: args.provider,
    folder: args.folder,
    passphrase: args.passphrase ?? null,
  });
}

export async function setHotkey(hotkey: string): Promise<Settings> {
  return invoke("set_hotkey", { hotkey });
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

export async function setItemPinned(id: string, pinned: boolean): Promise<void> {
  return invoke("set_item_pinned", { id, pinned });
}

export async function deleteItem(id: string): Promise<void> {
  return invoke("delete_item", { id });
}

export async function writeClipboardText(text: string): Promise<void> {
  return invoke("write_clipboard_text", { text });
}

export async function syncNow(): Promise<{ imported: number }> {
  return invoke("sync_now");
}
