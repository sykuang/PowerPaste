import { useState, useEffect, useCallback, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  Theme,
  UiMode,
  SyncProvider,
  Settings,
  setShowDockIcon,
  setHotkey,
  setTheme as setThemeApi,
  setUiMode as setUiModeApi,
  setHistoryRetention,
  setTrashEnabled,
  setTrashRetention,
  setLaunchAtStartup,
  connectSyncProvider,
  disconnectSyncProvider,
} from "../api";
import { PowerPasteLogo } from "./PowerPasteLogo";
import { useAutoHideScrollbar } from "../hooks/useAutoHideScrollbar";

// Common system shortcuts that may conflict with user's hotkey
const CONFLICTING_SHORTCUTS = [
  "Command+C",
  "Command+V",
  "Command+X",
  "Command+Z",
  "Command+A",
  "Command+S",
  "Command+Q",
  "Command+W",
  "Command+Tab",
  "Command+Space",
  "Control+C",
  "Control+V",
  "Control+X",
  "Control+Z",
  "Control+A",
  "Control+S",
];

// Map key codes to display symbols
const KEY_SYMBOLS: Record<string, string> = {
  Command: "⌘",
  Control: "⌃",
  Alt: "⌥",
  Shift: "⇧",
  Meta: "⌘",
};

// Convert KeyboardEvent to Tauri hotkey format
function keyEventToHotkey(e: KeyboardEvent): string | null {
  const modifiers: string[] = [];
  if (e.metaKey) modifiers.push("Command");
  if (e.ctrlKey) modifiers.push("Control");
  if (e.altKey) modifiers.push("Alt");
  if (e.shiftKey) modifiers.push("Shift");

  // Ignore if only modifiers pressed
  const key = e.key;
  if (["Meta", "Control", "Alt", "Shift"].includes(key)) {
    return null;
  }

  // Need at least one modifier
  if (modifiers.length === 0) {
    return null;
  }

  // Normalize key name
  let normalizedKey = key.length === 1 ? key.toUpperCase() : key;
  // Handle special keys
  if (key === " ") normalizedKey = "Space";

  return [...modifiers, normalizedKey].join("+");
}

// Format hotkey for display with symbols
function formatHotkeyDisplay(hotkey: string): string {
  return hotkey
    .split("+")
    .map((part) => KEY_SYMBOLS[part] || part)
    .join(" ");
}

// Check if hotkey conflicts with common system shortcuts
function checkHotkeyConflict(hotkey: string): boolean {
  const normalized = hotkey.toLowerCase();
  return CONFLICTING_SHORTCUTS.some((s) => s.toLowerCase() === normalized);
}

export type RetentionDays = 7 | 30 | 90 | 365 | null; // null = forever

export interface ConnectedProvider {
  provider: SyncProvider;
  accountEmail: string;
  accountId: string;
}

interface SettingsModalProps {
  settings: Settings;
  onClose: () => void;
  closeOnBackdrop?: boolean;
  /** Current platform, used to conditionally show macOS-only settings */
  platform?: "macos" | "windows" | "linux" | "unknown";
  /** Connected sync providers with account info */
  connectedProviders?: ConnectedProvider[];
}

export function SettingsModal(props: SettingsModalProps) {
  const closeOnBackdrop = props.closeOnBackdrop ?? true;
  const isMac = props.platform === "macos";

  // Local state
  const [activeSection, setActiveSection] = useState<"general" | "appearance" | "storage" | "cloud">("general");
  const [hotkey, setHotkeyValue] = useState(props.settings.hotkey);
  const [theme, setTheme] = useState<Theme>(props.settings.theme ?? "system");
  const [uiMode, setUiMode] = useState<UiMode>(props.settings.ui_mode ?? "floating");
  const [showDockIconState, setShowDockIconState] = useState(props.settings.show_dock_icon ?? false);
  const [launchAtStartup, setLaunchAtStartupState] = useState(props.settings.launch_at_startup ?? false);
  const [historyRetention, setHistoryRetentionState] = useState<RetentionDays>(
    (props.settings.history_retention_days ?? null) as RetentionDays
  );
  const [trashEnabled, setTrashEnabledState] = useState(props.settings.trash_enabled ?? true);
  const [trashRetention, setTrashRetentionState] = useState<RetentionDays>(
    (props.settings.trash_retention_days ?? 30) as RetentionDays
  );
  const [connectedProviders, setConnectedProviders] = useState<ConnectedProvider[]>(
    props.connectedProviders ?? []
  );

  // Error states per setting
  const [hotkeyError, setHotkeyError] = useState<string | null>(null);
  const [hotkeyWarning, setHotkeyWarning] = useState<string | null>(null);
  const [themeError, setThemeError] = useState<string | null>(null);
  const [uiModeError, setUiModeError] = useState<string | null>(null);
  const [dockIconError, setDockIconError] = useState<string | null>(null);
  const [launchAtStartupError, setLaunchAtStartupError] = useState<string | null>(null);
  const [retentionError, setRetentionError] = useState<string | null>(null);
  const [trashError, setTrashError] = useState<string | null>(null);
  const [syncError, setSyncError] = useState<string | null>(null);
  const settingsContentRef = useRef<HTMLDivElement>(null);

  // Auto-hide scrollbar overlay
  useAutoHideScrollbar(settingsContentRef);

  const setSection = useCallback((section: typeof activeSection) => {
    setActiveSection(section);
    if (settingsContentRef.current) {
      settingsContentRef.current.scrollTo({ top: 0, behavior: "smooth" });
    }
  }, []);

  // Hotkey recording state
  const [isRecordingHotkey, setIsRecordingHotkey] = useState(false);
  const [pendingHotkey, setPendingHotkey] = useState<string | null>(null);
  const hotkeyInputRef = useRef<HTMLButtonElement>(null);

  // Confirmation dialog state
  const [confirmDialog, setConfirmDialog] = useState<{
    title: string;
    message: string;
    onConfirm: () => void;
  } | null>(null);

  useEffect(() => {
    // Apply theme with system preference detection for live preview
    const applyTheme = (resolvedTheme: "light" | "dark") => {
      document.documentElement.dataset.theme = resolvedTheme;
    };

    if (theme === "system") {
      const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
      const handleChange = (e: MediaQueryListEvent | MediaQueryList) => {
        applyTheme(e.matches ? "dark" : "light");
      };
      handleChange(mediaQuery);
      mediaQuery.addEventListener("change", handleChange);
      return () => mediaQuery.removeEventListener("change", handleChange);
    } else {
      applyTheme(theme);
    }
  }, [theme]);

  // Split-page mode: sidebar directly controls visible section.

  // Listen for settings_changed event to sync with external changes
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    void (async () => {
      const h = await listen<Settings>("settings_changed", (event) => {
        console.log("[powerpaste] SettingsModal: settings_changed event received:", event.payload);
        const s = event.payload;
        setHotkeyValue(s.hotkey);
        setTheme(s.theme ?? "system");
        setUiMode(s.ui_mode ?? "floating");
        setShowDockIconState(s.show_dock_icon ?? false);
        setLaunchAtStartupState(s.launch_at_startup ?? false);
        setHistoryRetentionState((s.history_retention_days ?? null) as RetentionDays);
        setTrashEnabledState(s.trash_enabled ?? true);
        setTrashRetentionState((s.trash_retention_days ?? 30) as RetentionDays);
      });
      unlisten = h;
    })();
    return () => {
      unlisten?.();
    };
  }, []);

  // Hotkey recording keyboard handler
  const handleHotkeyKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (!isRecordingHotkey) return;

      e.preventDefault();
      e.stopPropagation();

      // Escape cancels recording
      if (e.key === "Escape") {
        setIsRecordingHotkey(false);
        setPendingHotkey(null);
        return;
      }

      const newHotkey = keyEventToHotkey(e);
      if (newHotkey) {
        setPendingHotkey(newHotkey);
        setIsRecordingHotkey(false);

        // Check for conflicts
        if (checkHotkeyConflict(newHotkey)) {
          setHotkeyWarning("This may conflict with system shortcuts");
        } else {
          setHotkeyWarning(null);
        }

        // Auto-save
        void (async () => {
          try {
            await setHotkey(newHotkey);
            setHotkeyValue(newHotkey);
            setHotkeyError(null);
            setPendingHotkey(null);
          } catch (err) {
            setHotkeyError(String(err));
            // Revert to previous hotkey
            setPendingHotkey(null);
          }
        })();
      }
    },
    [isRecordingHotkey]
  );

  useEffect(() => {
    if (isRecordingHotkey) {
      window.addEventListener("keydown", handleHotkeyKeyDown);
      return () => window.removeEventListener("keydown", handleHotkeyKeyDown);
    }
  }, [isRecordingHotkey, handleHotkeyKeyDown]);

  // Auto-save handlers
  const handleThemeChange = async (newTheme: Theme) => {
    const prevTheme = theme;
    setTheme(newTheme);
    setThemeError(null);
    try {
      await setThemeApi(newTheme);
    } catch (err) {
      setThemeError(String(err));
      setTheme(prevTheme);
    }
  };

  const handleUiModeChange = async (newMode: UiMode) => {
    const prevMode = uiMode;
    setUiMode(newMode);
    setUiModeError(null);
    try {
      await setUiModeApi(newMode);
    } catch (err) {
      setUiModeError(String(err));
      setUiMode(prevMode);
    }
  };

  const handleDockIconChange = async (checked: boolean) => {
    const prev = showDockIconState;
    setShowDockIconState(checked);
    setDockIconError(null);
    try {
      await setShowDockIcon(checked);
    } catch (err) {
      setDockIconError(String(err));
      setShowDockIconState(prev);
    }
  };

  const handleLaunchAtStartupChange = async (checked: boolean) => {
    const prev = launchAtStartup;
    setLaunchAtStartupState(checked);
    setLaunchAtStartupError(null);
    try {
      await setLaunchAtStartup(checked);
    } catch (err) {
      setLaunchAtStartupError(String(err));
      setLaunchAtStartupState(prev);
    }
  };

  const handleHistoryRetentionChange = async (newRetention: RetentionDays) => {
    const prev = historyRetention;

    // If reducing retention, show confirmation
    if (prev !== null && newRetention !== null && newRetention < prev) {
      setConfirmDialog({
        title: "Delete older items?",
        message: `Changing from ${prev} days to ${newRetention} days will permanently delete items older than ${newRetention} days${trashEnabled ? " (moved to Trash first)" : ""} on all synced devices.`,
        onConfirm: async () => {
          setHistoryRetentionState(newRetention);
          setRetentionError(null);
          try {
            await setHistoryRetention(newRetention);
          } catch (err) {
            setRetentionError(String(err));
            setHistoryRetentionState(prev);
          }
          setConfirmDialog(null);
        },
      });
      return;
    }

    setHistoryRetentionState(newRetention);
    setRetentionError(null);
    try {
      await setHistoryRetention(newRetention);
    } catch (err) {
      setRetentionError(String(err));
      setHistoryRetentionState(prev);
    }
  };

  const handleTrashEnabledChange = async (checked: boolean) => {
    const prev = trashEnabled;
    setTrashEnabledState(checked);
    setTrashError(null);
    try {
      await setTrashEnabled(checked);
    } catch (err) {
      setTrashError(String(err));
      setTrashEnabledState(prev);
    }
  };

  const handleTrashRetentionChange = async (newRetention: RetentionDays) => {
    const prev = trashRetention;

    // If reducing retention, show confirmation
    if (prev !== null && newRetention !== null && newRetention < prev) {
      setConfirmDialog({
        title: "Permanently delete trash items?",
        message: `Changing from ${prev} days to ${newRetention} days will permanently delete trashed items older than ${newRetention} days on all synced devices.`,
        onConfirm: async () => {
          setTrashRetentionState(newRetention);
          setTrashError(null);
          try {
            await setTrashRetention(newRetention);
          } catch (err) {
            setTrashError(String(err));
            setTrashRetentionState(prev);
          }
          setConfirmDialog(null);
        },
      });
      return;
    }

    setTrashRetentionState(newRetention);
    setTrashError(null);
    try {
      await setTrashRetention(newRetention);
    } catch (err) {
      setTrashError(String(err));
      setTrashRetentionState(prev);
    }
  };

  const handleConnectProvider = async (provider: SyncProvider) => {
    setSyncError(null);
    try {
      const result = await connectSyncProvider(provider);
      setConnectedProviders((prev) => [
        ...prev.filter((p) => p.provider !== provider),
        result,
      ]);
    } catch (err) {
      setSyncError(String(err));
    }
  };

  const handleDisconnectProvider = async (provider: SyncProvider) => {
    setSyncError(null);
    try {
      await disconnectSyncProvider(provider);
      setConnectedProviders((prev) => prev.filter((p) => p.provider !== provider));
    } catch (err) {
      setSyncError(String(err));
    }
  };

  const getConnectedProvider = (provider: SyncProvider): ConnectedProvider | undefined => {
    return connectedProviders.find((p) => p.provider === provider);
  };

  const displayHotkey = pendingHotkey || hotkey;

  // SVG Icons
  const KeyboardIcon = () => (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <rect x="2" y="4" width="20" height="16" rx="2" />
      <path d="M6 8h.01M10 8h.01M14 8h.01M18 8h.01M6 12h.01M10 12h.01M14 12h.01M18 12h.01M8 16h8" />
    </svg>
  );

  const PaletteIcon = () => (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="13.5" cy="6.5" r="0.5" fill="currentColor" />
      <circle cx="17.5" cy="10.5" r="0.5" fill="currentColor" />
      <circle cx="8.5" cy="7.5" r="0.5" fill="currentColor" />
      <circle cx="6.5" cy="12.5" r="0.5" fill="currentColor" />
      <path d="M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.926 0 1.648-.746 1.648-1.688 0-.437-.18-.835-.437-1.125-.29-.289-.438-.652-.438-1.125a1.64 1.64 0 0 1 1.668-1.668h1.996c3.051 0 5.555-2.503 5.555-5.555C21.965 6.012 17.461 2 12 2z" />
    </svg>
  );

  const ClockIcon = () => (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="10" />
      <polyline points="12 6 12 12 16 14" />
    </svg>
  );

  const CloudIcon = () => (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M17.5 19H9a7 7 0 1 1 6.71-9h1.79a4.5 4.5 0 1 1 0 9Z" />
    </svg>
  );

  const CheckIcon = () => (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
      <polyline points="20 6 9 17 4 12" />
    </svg>
  );

  const activeMeta = {
    general: { label: "General", icon: <KeyboardIcon /> },
    appearance: { label: "Appearance", icon: <PaletteIcon /> },
    storage: { label: "Storage", icon: <ClockIcon /> },
    cloud: { label: "Cloud Sync", icon: <CloudIcon /> },
  }[activeSection];

  return (
    <div
      className={closeOnBackdrop ? "modalBackdrop" : "modalBackdrop modalBackdropStatic"}
      onClick={closeOnBackdrop ? props.onClose : undefined}
    >
      <div className="settingsModal" onClick={(e) => e.stopPropagation()}>
        <aside className="settingsSidebar">
          <div className="settingsSidebarHeader">
            <PowerPasteLogo size={28} />
            <div className="settingsSidebarTitle">PowerPaste</div>
          </div>
          <nav className="settingsNav" aria-label="Settings sections">
            <button
              className={`settingsNavItem ${activeSection === "general" ? "active" : ""}`}
              onClick={() => setSection("general")}
              type="button"
            >
              <span className="settingsNavIcon"><KeyboardIcon /></span>
              <span>General</span>
            </button>
            <button
              className={`settingsNavItem ${activeSection === "appearance" ? "active" : ""}`}
              onClick={() => setSection("appearance")}
              type="button"
            >
              <span className="settingsNavIcon"><PaletteIcon /></span>
              <span>Appearance</span>
            </button>
            <button
              className={`settingsNavItem ${activeSection === "storage" ? "active" : ""}`}
              onClick={() => setSection("storage")}
              type="button"
            >
              <span className="settingsNavIcon"><ClockIcon /></span>
              <span>Storage</span>
            </button>
            <button
              className={`settingsNavItem ${activeSection === "cloud" ? "active" : ""}`}
              onClick={() => setSection("cloud")}
              type="button"
            >
              <span className="settingsNavIcon"><CloudIcon /></span>
              <span>Cloud Sync</span>
            </button>
          </nav>
          <div className="settingsSidebarFooter">
            <button className="settingsNavItem settingsNavHelp" type="button">
              <span className="settingsNavIcon">?</span>
              <span>Help Center</span>
            </button>
          </div>
        </aside>

        <div className="settingsMain">
          <div className="settingsMainHeader">
            <div className="settingsMainTitleGroup">
              <h1 className="settingsMainTitle">Settings</h1>
              <p className="settingsMainSubtitle">Fine-tune how PowerPaste behaves across devices.</p>
            </div>
            <div className="settingsHeaderBadge" aria-live="polite">
              <span className="settingsHeaderBadgeIcon">{activeMeta.icon}</span>
              <span>{activeMeta.label}</span>
            </div>
          </div>

          <div className="settingsContent" ref={settingsContentRef}>
          {activeSection === "general" && (
          <section id="settings-general" className="settingsSection settingsCard">
            <div className="settingsSectionHeader">
              <KeyboardIcon />
              <h2 className="settingsSectionTitle">General</h2>
            </div>

            {/* Hotkey */}
            <div className="settingsRow">
              <div className="settingsRowLabel">
                <span className="settingsRowTitle">Global Hotkey</span>
                <span className="settingsRowHint">Trigger PowerPaste from anywhere</span>
              </div>
              <button
                ref={hotkeyInputRef}
                className={`hotkeyRecorder ${isRecordingHotkey ? "recording" : ""}`}
                onClick={() => {
                  setIsRecordingHotkey(true);
                  setHotkeyError(null);
                }}
                onBlur={() => {
                  if (isRecordingHotkey) {
                    setIsRecordingHotkey(false);
                    setPendingHotkey(null);
                  }
                }}
              >
                {isRecordingHotkey ? (
                  <span className="hotkeyPlaceholder">Press keys...</span>
                ) : (
                  <span className="hotkeyKeys">{formatHotkeyDisplay(displayHotkey)}</span>
                )}
              </button>
            </div>
            {hotkeyError && <div className="settingsError">{hotkeyError}</div>}
            {hotkeyWarning && <div className="settingsWarning">{hotkeyWarning}</div>}

            {/* UI Mode */}
            <div className="settingsRow">
              <div className="settingsRowLabel">
                <span className="settingsRowTitle">Window Position</span>
                <span className="settingsRowHint">How the overlay appears on screen</span>
              </div>
              <select
                className="settingsSelect"
                value={uiMode}
                onChange={(e) => handleUiModeChange(e.currentTarget.value as UiMode)}
              >
                <option value="floating">Near cursor</option>
                <option value="fixed">Bottom of screen</option>
              </select>
            </div>
            {uiModeError && <div className="settingsError">{uiModeError}</div>}

            {/* Show Dock Icon (macOS only) */}
            {isMac && (
              <div className="settingsRow">
                <div className="settingsRowLabel">
                  <span className="settingsRowTitle">Show in Dock</span>
                  <span className="settingsRowHint">Display app icon in the Dock</span>
                </div>
                <label className="settingsToggle">
                  <input
                    type="checkbox"
                    checked={showDockIconState}
                    onChange={(e) => handleDockIconChange(e.currentTarget.checked)}
                  />
                  <span className="settingsToggleTrack">
                    <span className="settingsToggleThumb" />
                  </span>
                </label>
              </div>
            )}
            {dockIconError && <div className="settingsError">{dockIconError}</div>}

            {/* Launch at Startup */}
            <div className="settingsRow">
              <div className="settingsRowLabel">
                <span className="settingsRowTitle">Launch on Login</span>
                <span className="settingsRowHint">Start PowerPaste automatically when you sign in</span>
              </div>
              <label className="settingsToggle">
                <input
                  type="checkbox"
                  checked={launchAtStartup}
                  onChange={(e) => handleLaunchAtStartupChange(e.currentTarget.checked)}
                />
                <span className="settingsToggleTrack">
                  <span className="settingsToggleThumb" />
                </span>
              </label>
            </div>
            {launchAtStartupError && <div className="settingsError">{launchAtStartupError}</div>}
          </section>
          )}

          {/* Appearance Section */}
          {activeSection === "appearance" && (
          <section id="settings-appearance" className="settingsSection settingsCard">
            <div className="settingsSectionHeader">
              <PaletteIcon />
              <h2 className="settingsSectionTitle">Appearance</h2>
            </div>

            {/* Theme */}
            <div className="settingsRow">
              <div className="settingsRowLabel">
                <span className="settingsRowTitle">Theme</span>
                <span className="settingsRowHint">Choose your preferred color scheme</span>
              </div>
              <div className="settingsThemeGroup">
                <button
                  className={`settingsThemeBtn ${theme === "system" ? "active" : ""}`}
                  onClick={() => handleThemeChange("system")}
                >
                  Auto
                </button>
                <button
                  className={`settingsThemeBtn ${theme === "light" ? "active" : ""}`}
                  onClick={() => handleThemeChange("light")}
                >
                  Light
                </button>
                <button
                  className={`settingsThemeBtn ${theme === "dark" ? "active" : ""}`}
                  onClick={() => handleThemeChange("dark")}
                >
                  Dark
                </button>
              </div>
            </div>
            {themeError && <div className="settingsError">{themeError}</div>}
          </section>
          )}

          {/* Storage Section */}
          {activeSection === "storage" && (
          <section id="settings-storage" className="settingsSection settingsCard">
            <div className="settingsSectionHeader">
              <ClockIcon />
              <h2 className="settingsSectionTitle">Storage</h2>
            </div>

            {/* History Retention */}
            <div className="settingsRow">
              <div className="settingsRowLabel">
                <span className="settingsRowTitle">History Retention</span>
                <span className="settingsRowHint">How long to keep clipboard items</span>
              </div>
              <select
                className="settingsSelect"
                value={historyRetention ?? "forever"}
                onChange={(e) => {
                  const val = e.currentTarget.value;
                  handleHistoryRetentionChange(val === "forever" ? null : (Number(val) as RetentionDays));
                }}
              >
                <option value="7">7 days</option>
                <option value="30">30 days</option>
                <option value="90">90 days</option>
                <option value="365">1 year</option>
                <option value="forever">Forever</option>
              </select>
            </div>
            {retentionError && <div className="settingsError">{retentionError}</div>}

            {/* Trash Bin */}
            <div className="settingsRow">
              <div className="settingsRowLabel">
                <span className="settingsRowTitle">Enable Trash</span>
                <span className="settingsRowHint">Keep deleted items recoverable</span>
              </div>
              <label className="settingsToggle">
                <input
                  type="checkbox"
                  checked={trashEnabled}
                  onChange={(e) => handleTrashEnabledChange(e.currentTarget.checked)}
                />
                <span className="settingsToggleTrack">
                  <span className="settingsToggleThumb" />
                </span>
              </label>
            </div>
            {trashError && !trashEnabled && <div className="settingsError">{trashError}</div>}

            {trashEnabled && (
              <div className="settingsRow settingsSubRow">
                <div className="settingsRowLabel">
                  <span className="settingsRowTitle">Trash Retention</span>
                  <span className="settingsRowHint">When to permanently delete trashed items</span>
                </div>
                <select
                  className="settingsSelect"
                  value={trashRetention ?? "forever"}
                  onChange={(e) => {
                    const val = e.currentTarget.value;
                    handleTrashRetentionChange(val === "forever" ? null : (Number(val) as RetentionDays));
                  }}
                >
                  <option value="7">7 days</option>
                  <option value="14">14 days</option>
                  <option value="30">30 days</option>
                  <option value="forever">Forever</option>
                </select>
              </div>
            )}
            {trashError && trashEnabled && <div className="settingsError">{trashError}</div>}
          </section>
          )}

          {/* Cloud Sync Section */}
          {activeSection === "cloud" && (
          <section id="settings-cloud" className="settingsSection settingsCard">
            <div className="settingsSectionHeader">
              <CloudIcon />
              <h2 className="settingsSectionTitle">Cloud Sync</h2>
            </div>
            <p className="settingsSectionDesc">
              Connect cloud providers to sync clipboard history across your devices.
            </p>

            <div className="settingsProviderList">
              {/* iCloud */}
              <div className={`settingsProviderRow ${getConnectedProvider("icloud_drive") ? "connected" : ""}`}>
                <div className="settingsProviderIcon icloud">
                  <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
                    <path d="M13.004 19.942c2.937-.016 5.878-.016 8.822.003.04-3.033-.04-5.989.024-9.013-.016-.016-.036-.044-.06-.06-2.01-2.003-4.8-3.064-7.538-3.136-2.202-.058-4.351.543-6.192 1.75-.004.003-.008.002-.012.006A7.515 7.515 0 0 0 2 16.43c0 4.14 3.358 7.5 7.5 7.5l3.504.012zM6.77 8.5c.56-.44 1.181-.81 1.85-1.093A7.966 7.966 0 0 1 13.9 6.5c2.37 0 4.53.887 6.18 2.35A5.5 5.5 0 0 1 19.5 19h-10A7.5 7.5 0 0 1 6.77 8.5z"/>
                  </svg>
                </div>
                <div className="settingsProviderInfo">
                  <span className="settingsProviderName">iCloud</span>
                  {getConnectedProvider("icloud_drive") && (
                    <span className="settingsProviderAccount">
                      {getConnectedProvider("icloud_drive")?.accountEmail}
                    </span>
                  )}
                </div>
                {getConnectedProvider("icloud_drive") ? (
                  <button
                    className="settingsProviderBtn connected"
                    onClick={() => handleDisconnectProvider("icloud_drive")}
                  >
                    <CheckIcon />
                    Connected
                  </button>
                ) : (
                  <button
                    className="settingsProviderBtn"
                    onClick={() => handleConnectProvider("icloud_drive")}
                  >
                    Connect
                  </button>
                )}
              </div>

              {/* OneDrive */}
              <div className={`settingsProviderRow ${getConnectedProvider("one_drive") ? "connected" : ""}`}>
                <div className="settingsProviderIcon onedrive">
                  <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
                    <path d="M10.617 14.758l3.763-2.27 4.2 2.522-3.765 2.27-4.198-2.522zm.884-4.34l6.387 3.844-3.79 2.287-6.388-3.844 3.79-2.287zm-4.147 6.268l3.79 2.287-6.388 3.842L.966 18.97l3.79-2.284h2.598zm16.646-2.77L18.234 11 12 14.758l5.766 3.47 6.234-3.759v-.753z"/>
                  </svg>
                </div>
                <div className="settingsProviderInfo">
                  <span className="settingsProviderName">OneDrive</span>
                  {getConnectedProvider("one_drive") && (
                    <span className="settingsProviderAccount">
                      {getConnectedProvider("one_drive")?.accountEmail}
                    </span>
                  )}
                </div>
                {getConnectedProvider("one_drive") ? (
                  <button
                    className="settingsProviderBtn connected"
                    onClick={() => handleDisconnectProvider("one_drive")}
                  >
                    <CheckIcon />
                    Connected
                  </button>
                ) : (
                  <button
                    className="settingsProviderBtn"
                    onClick={() => handleConnectProvider("one_drive")}
                  >
                    Connect
                  </button>
                )}
              </div>

              {/* Google Drive */}
              <div className={`settingsProviderRow ${getConnectedProvider("google_drive") ? "connected" : ""}`}>
                <div className="settingsProviderIcon gdrive">
                  <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
                    <path d="M7.71 3.5l-5.16 8.94 2.58 4.47h5.16l-2.58-4.47L12.88 3.5H7.71zm8.58 0L8.58 16.91h5.16l7.71-13.41h-5.16zM12 9.5l-2.58 4.47L12 18.44l2.58-4.47L12 9.5z"/>
                  </svg>
                </div>
                <div className="settingsProviderInfo">
                  <span className="settingsProviderName">Google Drive</span>
                  {getConnectedProvider("google_drive") && (
                    <span className="settingsProviderAccount">
                      {getConnectedProvider("google_drive")?.accountEmail}
                    </span>
                  )}
                </div>
                {getConnectedProvider("google_drive") ? (
                  <button
                    className="settingsProviderBtn connected"
                    onClick={() => handleDisconnectProvider("google_drive")}
                  >
                    <CheckIcon />
                    Connected
                  </button>
                ) : (
                  <button
                    className="settingsProviderBtn"
                    onClick={() => handleConnectProvider("google_drive")}
                  >
                    Connect
                  </button>
                )}
              </div>
            </div>

            {syncError && <div className="settingsError">{syncError}</div>}
          </section>
          )}
        </div>
        </div>
      </div>

      {/* Confirmation Dialog */}
      {confirmDialog && (
        <div className="confirmDialogBackdrop" onClick={() => setConfirmDialog(null)}>
          <div className="confirmDialog" onClick={(e) => e.stopPropagation()}>
            <div className="confirmDialogHeader">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
                <line x1="12" y1="9" x2="12" y2="13" />
                <line x1="12" y1="17" x2="12.01" y2="17" />
              </svg>
              <h3 className="confirmDialogTitle">{confirmDialog.title}</h3>
            </div>
            <p className="confirmDialogMessage">{confirmDialog.message}</p>
            <div className="confirmDialogActions">
              <button className="confirmDialogBtn cancel" onClick={() => setConfirmDialog(null)}>
                Cancel
              </button>
              <button className="confirmDialogBtn danger" onClick={confirmDialog.onConfirm}>
                Delete & Apply
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
