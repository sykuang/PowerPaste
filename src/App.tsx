import { useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
  deleteItem,
  getSettings,
  listItems,
  setItemPinned,
  setSyncSettings,
  syncNow,
  writeClipboardText,
  type ClipboardItem,
  type Settings,
  type SyncProvider,
} from "./api";
import "./App.css";

function App() {
  const [items, setItems] = useState<ClipboardItem[]>([]);
  const [query, setQuery] = useState("");
  const [settings, setSettings] = useState<Settings | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [syncStatus, setSyncStatus] = useState<string>("");

  const filteredQuery = useMemo(() => query.trim(), [query]);

  async function reload() {
    const [s, it] = await Promise.all([
      getSettings(),
      listItems({ limit: 500, query: filteredQuery || undefined }),
    ]);
    setSettings(s);
    setItems(it);
  }

  useEffect(() => {
    void reload();
  }, [filteredQuery]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    void (async () => {
      const h = await listen<ClipboardItem>("powerpaste://new_item", () => {
        void reload();
      });
      unlisten = h;
    })();
    return () => {
      unlisten?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function onCopy(item: ClipboardItem) {
    await writeClipboardText(item.text);
    setSyncStatus("Copied to clipboard");
    setTimeout(() => setSyncStatus(""), 1200);
  }

  async function onTogglePinned(item: ClipboardItem) {
    await setItemPinned(item.id, !item.pinned);
    await reload();
  }

  async function onDelete(item: ClipboardItem) {
    await deleteItem(item.id);
    await reload();
  }

  async function onSyncNow() {
    setSyncStatus("Syncing...");
    try {
      const res = await syncNow();
      setSyncStatus(`Sync complete (imported ${res.imported})`);
      await reload();
    } catch (e) {
      setSyncStatus(String(e));
    } finally {
      setTimeout(() => setSyncStatus(""), 2500);
    }
  }

  async function pickFolder() {
    const result = await open({ directory: true, multiple: false });
    if (typeof result === "string") return result;
    return null;
  }

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <div className="brandTitle">PowerPaste</div>
          <div className="brandSub">Cross-platform clipboard history + folder sync</div>
        </div>

        <input
          className="search"
          value={query}
          onChange={(e) => setQuery(e.currentTarget.value)}
          placeholder="Search your clipboard history..."
        />

        <div className="actions">
          <button className="btn" onClick={onSyncNow}>
            Sync now
          </button>
          <button className="btn" onClick={() => setShowSettings(true)}>
            Settings
          </button>
        </div>
      </header>

      {syncStatus ? <div className="status">{syncStatus}</div> : null}

      <main className="content">
        <div className="listHeader">
          <div>
            {items.length} items{filteredQuery ? ` (filtered)` : ""}
          </div>
          <div className="muted">Click an item to copy it back.</div>
        </div>

        <div className="list">
          {items.map((item) => {
            const firstLine = item.text.split(/\r?\n/)[0] ?? "";
            return (
              <div key={item.id} className="rowItem">
                <button className="pin" onClick={() => onTogglePinned(item)} title="Pin">
                  {item.pinned ? "★" : "☆"}
                </button>
                <button className="item" onClick={() => onCopy(item)}>
                  <div className="itemTitle">{firstLine || "(empty)"}</div>
                  <div className="itemMeta">
                    {new Date(item.created_at_ms).toLocaleString()} • {item.text.length} chars
                  </div>
                </button>
                <button className="danger" onClick={() => onDelete(item)} title="Delete">
                  Delete
                </button>
              </div>
            );
          })}
        </div>
      </main>

      {showSettings && settings ? (
        <SettingsModal
          settings={settings}
          onClose={() => setShowSettings(false)}
          onSave={async (next) => {
            const updated = await setSyncSettings(next);
            setSettings(updated);
            setShowSettings(false);
          }}
          onPickFolder={pickFolder}
        />
      ) : null}
    </div>
  );
}

function SettingsModal(props: {
  settings: Settings;
  onClose: () => void;
  onSave: (args: {
    enabled: boolean;
    provider: SyncProvider | null;
    folder: string | null;
    passphrase?: string | null;
  }) => Promise<void>;
  onPickFolder: () => Promise<string | null>;
}) {
  const [enabled, setEnabled] = useState(props.settings.sync_enabled);
  const [provider, setProvider] = useState<SyncProvider | null>(props.settings.sync_provider);
  const [folder, setFolder] = useState<string | null>(props.settings.sync_folder);
  const [passphrase, setPassphrase] = useState<string>("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string>("");

  return (
    <div className="modalBackdrop" onClick={props.onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="modalHeader">
          <div className="modalTitle">Settings</div>
          <button className="btn" onClick={props.onClose}>
            Close
          </button>
        </div>

        <div className="section">
          <label className="checkbox">
            <input type="checkbox" checked={enabled} onChange={(e) => setEnabled(e.currentTarget.checked)} />
            Enable folder sync
          </label>
          <div className="hint">
            Works with iCloud Drive / OneDrive / Google Drive by selecting their local synced folder on each device.
          </div>
        </div>

        <div className="section">
          <label className="label">Provider</label>
          <select
            className="select"
            value={provider ?? "custom_folder"}
            onChange={(e) => setProvider(e.currentTarget.value as SyncProvider)}
            disabled={!enabled}
          >
            <option value="icloud_drive">iCloud Drive (folder)</option>
            <option value="one_drive">OneDrive (folder)</option>
            <option value="google_drive">Google Drive (folder)</option>
            <option value="custom_folder">Custom folder</option>
          </select>
        </div>

        <div className="section">
          <label className="label">Sync folder</label>
          <div className="rowInline">
            <input
              className="input"
              value={folder ?? ""}
              onChange={(e) => setFolder(e.currentTarget.value || null)}
              placeholder="Pick a folder..."
              disabled={!enabled}
            />
            <button
              className="btn"
              disabled={!enabled}
              onClick={async () => {
                const p = await props.onPickFolder();
                if (p) setFolder(p);
              }}
            >
              Browse
            </button>
          </div>
        </div>

        <div className="section">
          <label className="label">Encryption passphrase</label>
          <input
            className="input"
            type="password"
            value={passphrase}
            onChange={(e) => setPassphrase(e.currentTarget.value)}
            placeholder="Set / update passphrase (stored in OS keychain)"
            disabled={!enabled}
          />
          <div className="hint">This passphrase encrypts the sync file. Use the same passphrase on every device.</div>
        </div>

        {error ? <div className="error">{error}</div> : null}

        <div className="modalFooter">
          <button
            className="btnPrimary"
            disabled={saving}
            onClick={async () => {
              setSaving(true);
              setError("");
              try {
                await props.onSave({
                  enabled,
                  provider,
                  folder,
                  passphrase: passphrase.trim() ? passphrase : null,
                });
              } catch (e) {
                setError(String(e));
              } finally {
                setSaving(false);
              }
            }}
          >
            Save
          </button>
        </div>
      </div>
    </div>
  );
}

export default App;
