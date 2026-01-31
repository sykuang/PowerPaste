import { useState, useEffect } from "react";
import { Theme, UiMode, SyncProvider, Settings, setShowDockIcon } from "../api";

interface SettingsModalProps {
  settings: Settings;
  onClose: () => void;
  closeOnBackdrop?: boolean;
  /** Current platform, used to conditionally show macOS-only settings */
  platform?: "macos" | "windows" | "linux" | "unknown";
  onSave: (args: {
    hotkey: string;
    enabled: boolean;
    provider: SyncProvider | null;
    folder: string | null;
    passphrase?: string | null;
    theme: Theme;
    uiMode: UiMode;
  }) => Promise<void>;
  onPickFolder: () => Promise<string | null>;
}

export function SettingsModal(props: SettingsModalProps) {
  const closeOnBackdrop = props.closeOnBackdrop ?? true;
  const isMac = props.platform === "macos";
  const [hotkey, setHotkeyValue] = useState(props.settings.hotkey);
  const [enabled, setEnabled] = useState(props.settings.sync_enabled);
  const [provider, setProvider] = useState<SyncProvider | null>(props.settings.sync_provider);
  const [folder, setFolder] = useState<string | null>(props.settings.sync_folder);
  const [passphrase, setPassphrase] = useState<string>("");
  const [theme, setTheme] = useState<Theme>(props.settings.theme ?? "glass");
  const [uiMode, setUiMode] = useState<UiMode>(props.settings.ui_mode ?? "floating");
  const [showDockIconState, setShowDockIconState] = useState(props.settings.show_dock_icon ?? false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string>("");

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  return (
    <div
      className={closeOnBackdrop ? "modalBackdrop" : "modalBackdrop modalBackdropStatic"}
      onClick={closeOnBackdrop ? props.onClose : undefined}
    >
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="modalHeader">
          <div className="modalTitle">Settings</div>
          <button className="btn" onClick={props.onClose}>
            Close
          </button>
        </div>

        <div className="section">
          <label className="label">Hotkey</label>
          <input
            className="input"
            value={hotkey}
            onChange={(e) => setHotkeyValue(e.currentTarget.value)}
            placeholder="Command+Shift+V"
          />
          <div className="hint">
            Press this global shortcut to toggle PowerPaste. Default is Command+Shift+V.
          </div>
        </div>

        <div className="section">
          <label className="label">Theme</label>
          <select
            className="select"
            value={theme}
            onChange={(e) => setTheme(e.currentTarget.value as Theme)}
          >
            <option value="glass">Glass (Light)</option>
            <option value="midnight">Midnight (Dark)</option>
            <option value="aurora">Aurora</option>
          </select>
          <div className="hint">Affects the main overlay and Settings window.</div>
        </div>

        <div className="section">
          <label className="label">UI Mode</label>
          <select
            className="select"
            value={uiMode}
            onChange={(e) => setUiMode(e.currentTarget.value as UiMode)}
          >
            <option value="floating">Floating (near cursor)</option>
            <option value="fixed">Fixed (bottom of screen)</option>
          </select>
          <div className="hint">Floating mode positions the overlay near your cursor. Fixed mode anchors it to the bottom of the screen.</div>
        </div>

        {isMac && (
          <div className="section">
            <label className="checkbox">
              <input
                type="checkbox"
                checked={showDockIconState}
                onChange={async (e) => {
                  const checked = e.currentTarget.checked;
                  setShowDockIconState(checked);
                  try {
                    await setShowDockIcon(checked);
                  } catch (err) {
                    console.error("Failed to set dock icon visibility:", err);
                  }
                }}
              />
              Show icon in Dock
            </label>
            <div className="hint">
              When disabled, PowerPaste runs as a menu bar app only.
            </div>
          </div>
        )}

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
          <div className="hint">
            This passphrase encrypts the sync file. Use the same passphrase on every device.
          </div>
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
                  hotkey: hotkey.trim(),
                  theme,
                  uiMode,
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
