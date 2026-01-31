import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
  checkPermissions,
  deleteItem,
  enableMouseEvents,
  getSettings,
  listCategories,
  listItems,
  openAccessibilitySettings,
  openAutomationSettings,
  pasteText,
  hideMainWindow,
  setItemCategory,
  setItemPinned,
  setOverlayPreferredSize,
  setHotkey,
  setSyncSettings,
  setUiMode,
  syncNow,
  writeClipboardText,
  type ClipboardItem,
  type PermissionsStatus,
  type Settings,
} from "./api";
import "./App.css";
import { SettingsModal } from "./components/SettingsModal";
import { PermissionsModal } from "./components/PermissionsModal";
import { BottomTray } from "./components/BottomTray";
import { SaveToTabModal } from "./components/SaveToTabModal";

const IS_SETTINGS_WINDOW =
  typeof window !== "undefined" &&
  new URLSearchParams(window.location.search).get("settings") === "1";

function isSearchInputTarget(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  return !!el && el.tagName?.toLowerCase() === "input" && el.classList.contains("search");
}

function isEditableTarget(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  if (!el) return false;
  const tag = el.tagName?.toLowerCase();
  if (tag === "input" || tag === "textarea" || tag === "select") return true;
  if (el.isContentEditable) return true;
  return false;
}

function App() {
  const [items, setItems] = useState<ClipboardItem[]>([]);
  const [query, setQuery] = useState("");
  const [settings, setSettings] = useState<Settings | null>(null);
  const [syncStatus, setSyncStatus] = useState<string>("");

  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [lastSelectedIndex, setLastSelectedIndex] = useState<number | null>(null);

  const [showPermissions, setShowPermissions] = useState(false);
  const [permissions, setPermissions] = useState<PermissionsStatus | null>(null);
  const [checkingPermissions, setCheckingPermissions] = useState(false);

  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; item: ClipboardItem } | null>(null);

  // Category/tab state
  const [categories, setCategories] = useState<string[]>([]);
  const [activeTab, setActiveTab] = useState<string | null>(null); // null = Clipboard (recent history)
  const [saveToTabItem, setSaveToTabItem] = useState<ClipboardItem | null>(null);

  const lastSentOverlaySizeRef = useRef<{ w: number; h: number }>({ w: 0, h: 0 });
  const searchInputRef = useRef<HTMLInputElement>(null);

  const filteredQuery = useMemo(() => query.trim(), [query]);

  const trayItems = useMemo(() => {
    let filtered = [...items];
    
    // Filter by active tab
    if (activeTab === null) {
      // Clipboard tab: show items without a category (recent clipboard history)
      filtered = filtered.filter((item) => !item.pin_category);
    } else {
      // Custom tab: show items with matching category
      filtered = filtered.filter((item) => item.pin_category === activeTab);
    }
    
    filtered.sort((a, b) => {
      if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
      return b.created_at_ms - a.created_at_ms;
    });
    return filtered;
  }, [items, activeTab]);

  // Keep refs to avoid stale closures in event handlers
  const trayItemsRef = useRef(trayItems);
  const selectedIdsRef = useRef(selectedIds);
  useEffect(() => {
    trayItemsRef.current = trayItems;
  }, [trayItems]);
  useEffect(() => {
    selectedIdsRef.current = selectedIds;
  }, [selectedIds]);

  const selectedItems = useMemo(() => {
    if (selectedIds.size === 0) return [];
    // With only the BottomTray visible, selection should apply to tray cards.
    return trayItems.filter((it) => selectedIds.has(it.id));
  }, [trayItems, selectedIds]);

  const copySelected = useCallback(async () => {
    if (selectedItems.length === 0) return;

    let clearAfterMs = 1200;
    try {
      const text = selectedItems.map((it) => it.text).join("\n\n");
      await writeClipboardText(text);
      setSyncStatus(
        selectedItems.length === 1
          ? "Copied selected item"
          : `Copied ${selectedItems.length} selected items`
      );
    } catch (err) {
      setSyncStatus(String(err));
      clearAfterMs = 5000;
    } finally {
      setTimeout(() => setSyncStatus(""), clearAfterMs);
    }
  }, [selectedItems]);

  const selectAll = useCallback(() => {
    // Use refs to always get the latest values, avoiding stale closure issues
    const currentItems = trayItemsRef.current;
    const currentSelectedIds = selectedIdsRef.current;
    console.log("[powerpaste] selectAll called, items count:", currentItems.length, "selected:", currentSelectedIds.size);
    
    if (currentItems.length === 0) {
      console.log("[powerpaste] selectAll: no items to select");
      return;
    }
    
    // Toggle: if all items are already selected, deselect all
    const allSelected = currentItems.length > 0 && 
      currentItems.every((it) => currentSelectedIds.has(it.id));
    
    if (allSelected) {
      console.log("[powerpaste] selectAll: all already selected, deselecting all");
      setSelectedIds(new Set());
      setLastSelectedIndex(null);
    } else {
      console.log("[powerpaste] selectAll: selecting all items");
      setSelectedIds(new Set(currentItems.map((it) => it.id)));
      setLastSelectedIndex(currentItems.length - 1);
    }
  }, []); // No dependencies - uses refs instead

  const clearSelection = useCallback(() => {
    setSelectedIds(new Set());
    setLastSelectedIndex(null);
  }, []);

  const handleDelete = useCallback(async (item: ClipboardItem) => {
    let clearAfterMs = 1200;
    try {
      // If the item being deleted is selected, delete all selected items
      const itemsToDelete = selectedIds.has(item.id) && selectedIds.size > 0
        ? Array.from(selectedIds)
        : [item.id];

      // Delete all items in parallel
      await Promise.all(itemsToDelete.map(id => deleteItem(id)));
      
      const count = itemsToDelete.length;
      setSyncStatus(count === 1 ? "Item deleted" : `${count} items deleted`);
      
      // Clear deleted items from selection
      setSelectedIds((prev) => {
        const next = new Set(prev);
        itemsToDelete.forEach(id => next.delete(id));
        return next;
      });
      
      await reload();
    } catch (e) {
      setSyncStatus(String(e));
      clearAfterMs = 5000;
    } finally {
      setTimeout(() => setSyncStatus(""), clearAfterMs);
    }
  }, [selectedIds]);

  const closeContextMenu = useCallback(() => {
    setContextMenu(null);
  }, []);

  const togglePinned = useCallback(
    async (item: ClipboardItem) => {
      let clearAfterMs = 1200;
      try {
        await setItemPinned(item.id, !item.pinned);
        setSyncStatus(item.pinned ? "Unpinned" : "Pinned");
        await reload();
      } catch (e) {
        setSyncStatus(String(e));
        clearAfterMs = 5000;
      } finally {
        setTimeout(() => setSyncStatus(""), clearAfterMs);
      }
    },
    []
  );

  const handleSaveToTab = useCallback(
    async (item: ClipboardItem, category: string) => {
      let clearAfterMs = 1200;
      try {
        await setItemCategory(item.id, category);
        setSyncStatus(`Saved to "${category}"`);
        await reload();
      } catch (e) {
        setSyncStatus(String(e));
        clearAfterMs = 5000;
      } finally {
        setTimeout(() => setSyncStatus(""), clearAfterMs);
      }
    },
    []
  );

  const handleRemoveFromTab = useCallback(
    async (item: ClipboardItem) => {
      let clearAfterMs = 1200;
      try {
        await setItemCategory(item.id, null);
        setSyncStatus("Removed from tab");
        await reload();
      } catch (e) {
        setSyncStatus(String(e));
        clearAfterMs = 5000;
      } finally {
        setTimeout(() => setSyncStatus(""), clearAfterMs);
      }
    },
    []
  );

  const selectIndex = useCallback(
    (index: number, opts: { additive?: boolean; range?: boolean } = {}) => {
      const itemAtIndex = trayItems[index];
      if (!itemAtIndex) return;

      setSelectedIds((prev) => {
        const next = new Set(prev);
        const additive = opts.additive ?? false;
        const range = opts.range ?? false;

        if (!additive && !range) {
          next.clear();
          next.add(itemAtIndex.id);
          return next;
        }

        if (range && lastSelectedIndex !== null) {
          const start = Math.min(lastSelectedIndex, index);
          const end = Math.max(lastSelectedIndex, index);
          if (!additive) next.clear();
          for (let i = start; i <= end; i++) {
            const it = trayItems[i];
            if (it) next.add(it.id);
          }
          return next;
        }

        // additive toggle
        if (next.has(itemAtIndex.id)) next.delete(itemAtIndex.id);
        else next.add(itemAtIndex.id);
        return next;
      });

      setLastSelectedIndex(index);
    },
    [trayItems, lastSelectedIndex]
  );

  const selectItem = useCallback(
    (item: ClipboardItem, opts: { additive?: boolean; range?: boolean } = {}) => {
      const index = trayItems.findIndex((it) => it.id === item.id);
      if (index >= 0) {
        selectIndex(index, opts);
        return;
      }
      // If the item isn't in the visible tray, ignore selection.
    },
    [trayItems, selectIndex]
  );

  async function reload() {
    const [s, it, cats] = await Promise.all([
      getSettings(),
      listItems({ limit: 500, query: filteredQuery || undefined }),
      listCategories(),
    ]);
    setSettings(s);
    setItems(it);
    setCategories(cats);
  }

  useEffect(() => {
    if (!settings?.theme) return;
    document.documentElement.dataset.theme = settings.theme;
  }, [settings?.theme]);

  // Enable mouse events for settings window (fixes macOS click-through issue)
  useEffect(() => {
    if (!IS_SETTINGS_WINDOW) return;
    enableMouseEvents().catch((e) => {
      console.error("[powerpaste] failed to enable mouse events:", e);
    });
  }, []);

  useEffect(() => {
    if (IS_SETTINGS_WINDOW) return;

    let raf: number | null = null;
    let timeout: number | null = null;

    const measureAndSend = () => {
      if (raf !== null) cancelAnimationFrame(raf);
      raf = requestAnimationFrame(() => {
        const header = document.querySelector<HTMLElement>(".topbar");
        const tray = document.querySelector<HTMLElement>(".bottomTray");
        if (!header || !tray) return;

        const headerH = header.getBoundingClientRect().height;
        const trayH = tray.getBoundingClientRect().height;

        const nextH = Math.max(1, Math.ceil(headerH + trayH));
        const nextW = Math.max(1, Math.ceil(document.documentElement.clientWidth));

        const dw = Math.abs(nextW - lastSentOverlaySizeRef.current.w);
        const dh = Math.abs(nextH - lastSentOverlaySizeRef.current.h);
        if (dw < 2 && dh < 2) return;

        lastSentOverlaySizeRef.current.w = nextW;
        lastSentOverlaySizeRef.current.h = nextH;

        // Best-effort: if we're in the browser preview, this invoke will fail; ignore.
        void setOverlayPreferredSize({ width: nextW, height: nextH }).catch(() => undefined);
      });
    };

    // Delay a bit to allow fonts/layout to settle.
    timeout = window.setTimeout(measureAndSend, 50);

    const onResize = () => measureAndSend();
    window.addEventListener("resize", onResize);

    const ro =
      typeof ResizeObserver !== "undefined"
        ? new ResizeObserver(() => measureAndSend())
        : null;
    if (ro) {
      const header = document.querySelector<HTMLElement>(".topbar");
      const tray = document.querySelector<HTMLElement>(".bottomTray");
      if (header) ro.observe(header);
      if (tray) ro.observe(tray);
    }

    return () => {
      window.removeEventListener("resize", onResize);
      ro?.disconnect();
      if (raf !== null) cancelAnimationFrame(raf);
      if (timeout !== null) window.clearTimeout(timeout);
    };
  }, []);

  useEffect(() => {
    void reload();
  }, [filteredQuery]);

  // Close context menu when clicking elsewhere
  useEffect(() => {
    if (!contextMenu) return;
    const handleClick = () => closeContextMenu();
    document.addEventListener("click", handleClick);
    return () => document.removeEventListener("click", handleClick);
  }, [contextMenu, closeContextMenu]);

  // Close context menu with Escape
  useEffect(() => {
    if (!contextMenu) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        closeContextMenu();
      }
    };
    document.addEventListener("keydown", onKeyDown, { capture: true });
    return () => document.removeEventListener("keydown", onKeyDown, { capture: true });
  }, [contextMenu, closeContextMenu]);

  // Disable browser's default context menu globally (we use custom context menu)
  useEffect(() => {
    if (IS_SETTINGS_WINDOW) return;
    const handleContextMenu = (e: MouseEvent) => {
      // Only prevent on card elements
      const target = e.target as HTMLElement;
      if (target.closest(".trayCard")) {
        e.preventDefault();
      }
    };
    document.addEventListener("contextmenu", handleContextMenu);
    return () => document.removeEventListener("contextmenu", handleContextMenu);
  }, []);

  useEffect(() => {
    let cancelled = false;

    void (async () => {
      setCheckingPermissions(true);
      try {
        const res = await checkPermissions();
        if (cancelled) return;
        setPermissions(res);
        setShowPermissions(!res.can_paste);
      } catch (e) {
        if (cancelled) return;
        setPermissions({
          platform: "unknown",
          can_paste: false,
          automation_ok: false,
          accessibility_ok: false,
          details: String(e),
        });
        setShowPermissions(true);
      } finally {
        if (!cancelled) setCheckingPermissions(false);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      // DEBUG: Log all keydown events to diagnose Cmd+A issue
      console.log("[powerpaste] keydown:", e.key, "meta:", e.metaKey, "ctrl:", e.ctrlKey, "target:", (e.target as HTMLElement)?.tagName);

      // In the main overlay, Cmd/Ctrl+A/C are meant to operate on cards,
      // even if the search input is focused.
      if (isEditableTarget(e.target) && (IS_SETTINGS_WINDOW || !isSearchInputTarget(e.target))) {
        console.log("[powerpaste] skipping - editable target in settings or non-search");
        return;
      }

      const isMod = e.metaKey || e.ctrlKey;

      const key = e.key.toLowerCase();

      if (key === "escape") {
        clearSelection();
        return;
      }

      if (!isMod) return;

      if (key === "a") {
        // On macOS, Cmd+A may be handled by the native "Select All" menu before bubbling.
        // Capture-phase listener + preventDefault ensures it selects tray cards instead.
        console.log("[powerpaste] Cmd+A detected! Calling selectAll()");
        e.preventDefault();
        e.stopPropagation();
        selectAll();
        return;
      }

      if (key === "c") {
        e.preventDefault();
        e.stopPropagation();
        void copySelected();
        return;
      }

      if (key === "v") {
        // Paste first selected item (or first tray item if nothing selected)
        e.preventDefault();
        e.stopPropagation();
        const currentItems = trayItemsRef.current;
        const currentSelectedIds = selectedIdsRef.current;
        const selectedList = currentItems.filter((it) => currentSelectedIds.has(it.id));
        const itemToPaste = selectedList.length > 0 ? selectedList[0] : currentItems[0];
        if (itemToPaste) {
          // Inline paste logic to avoid stale closure issues
          void (async () => {
            try {
              await pasteText(itemToPaste.text);
              // Note: hideMainWindow is now called by the backend before pasting
            } catch {
              // Errors handled in UI
            }
          })();
        }
        return;
      }
    }

    // Register on both document and window to maximize reliability across
    // browsers/webviews (some environments dispatch shortcuts differently).
    document.addEventListener("keydown", onKeyDown, { capture: true });
    window.addEventListener("keydown", onKeyDown, { capture: true });
    return () => {
      document.removeEventListener("keydown", onKeyDown, { capture: true });
      window.removeEventListener("keydown", onKeyDown, { capture: true });
    };
  }, [clearSelection, copySelected, selectAll]);

  // Listen for panel_shown event to focus the search input
  useEffect(() => {
    if (IS_SETTINGS_WINDOW) return;

    let unlisten: (() => void) | null = null;
    void (async () => {
      unlisten = await listen("powerpaste://panel_shown", () => {
        console.log("[powerpaste] panel_shown event received, focusing search input");
        // Multiple attempts with increasing delays to ensure focus works
        const focusInput = () => {
          const input = searchInputRef.current;
          if (input) {
            input.focus();
            // Also try selecting the content to make focus more obvious
            input.select();
            console.log("[powerpaste] focus() called on search input, activeElement:", document.activeElement?.tagName);
          }
        };
        // Try immediately
        focusInput();
        // Try after a frame
        requestAnimationFrame(focusInput);
        // Try after a short delay
        setTimeout(focusInput, 50);
        setTimeout(focusInput, 150);
      });
    })();
    return () => {
      unlisten?.();
    };
  }, []);

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

  useEffect(() => {
    if (IS_SETTINGS_WINDOW) return;

    let unlistenSelectAll: (() => void) | null = null;
    let unlistenCopySelected: (() => void) | null = null;

    void (async () => {
      unlistenSelectAll = await listen("powerpaste://select_all", () => {
        // Always select all cards when menu shortcut is triggered.
        // The backend only sends this event when the overlay is active.
        console.log("[powerpaste] select_all event received, calling selectAll()");
        setSyncStatus("Selected all cards");
        setTimeout(() => setSyncStatus(""), 900);
        selectAll();
      });

      unlistenCopySelected = await listen("powerpaste://copy_selected", () => {
        // Always copy selected cards when menu shortcut is triggered.
        console.log("[powerpaste] copy_selected event received, calling copySelected()");

        // If nothing is selected, show a quick hint so we can tell the shortcut was received.
        if (selectedIds.size === 0) {
          setSyncStatus("No selected cards");
          setTimeout(() => setSyncStatus(""), 900);
          return;
        }
        void copySelected().then(() => {
          // Hide the UI after copying via menu
          console.log("[powerpaste] copySelected: calling hideMainWindow");
          void hideMainWindow().catch(() => undefined);
        });
      });
    })();

    return () => {
      unlistenSelectAll?.();
      unlistenCopySelected?.();
    };
  }, [copySelected, selectAll, selectedIds]);

  async function onCopy(item: ClipboardItem) {
    console.log("[powerpaste] onCopy called for item:", item.id);
    let clearAfterMs = 1200;
    try {
      await writeClipboardText(item.text);
      setSyncStatus("Copied to clipboard");
      // Hide the UI after copying
      console.log("[powerpaste] onCopy: calling hideMainWindow");
      await hideMainWindow().catch(() => undefined);
    } catch (e) {
      setSyncStatus(String(e));
      clearAfterMs = 5000;
    } finally {
      setTimeout(() => setSyncStatus(""), clearAfterMs);
    }
  }

  async function onPaste(item: ClipboardItem) {
    console.log("[powerpaste] onPaste called for item:", item.id, item.text.substring(0, 50));
    let clearAfterMs = 1200;
    try {
      console.log("[powerpaste] calling pasteText...");
      await pasteText(item.text);
      console.log("[powerpaste] pasteText completed");
      setSyncStatus("Pasted");
      // Note: hideMainWindow is now called by the backend before pasting
    } catch (e) {
      console.error("[powerpaste] pasteText error:", e);
      setSyncStatus(String(e));
      clearAfterMs = 5000;
      setShowPermissions(true);
    } finally {
      setTimeout(() => setSyncStatus(""), clearAfterMs);
    }
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

  async function openSettingsWindow() {
    if (IS_SETTINGS_WINDOW) return;
    try {
      const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
      const existing = await WebviewWindow.getByLabel("settings");
      if (existing) {
        await existing.show();
        await existing.setFocus();
        return;
      }

      // In dev mode, use the current origin; in production, use relative path
      const isDev = window.location.hostname === "localhost";
      const settingsUrl = isDev 
        ? `${window.location.origin}/?settings=1`
        : "index.html?settings=1";

      const win = new WebviewWindow("settings", {
        url: settingsUrl,
        title: "Settings — PowerPaste",
        width: 860,
        height: 640,
        minWidth: 720,
        minHeight: 520,
        resizable: true,
        decorations: false,
        transparent: true,
        shadow: false,
      });

      // Best-effort: if window creation fails, surface it in status.
      win.once("tauri://error", (e) => {
        setSyncStatus(String((e as { payload?: unknown }).payload ?? "Failed to open Settings"));
        setTimeout(() => setSyncStatus(""), 5000);
      });
    } catch (e) {
      setSyncStatus(String(e));
      setTimeout(() => setSyncStatus(""), 5000);
    }
  }

  async function closeCurrentWindow() {
    try {
      const mod = await import("@tauri-apps/api/webviewWindow");
      const current =
        typeof mod.getCurrentWebviewWindow === "function"
          ? mod.getCurrentWebviewWindow()
          : null;
      if (current) {
        await current.close();
        return;
      }
    } catch {
      // ignore
    }

    // Fallback for browser preview.
    window.close();
  }

  if (IS_SETTINGS_WINDOW) {
    return (
      <div className="app settingsWindow">
        {settings ? (
          <SettingsModal
            settings={settings}
            onClose={() => void closeCurrentWindow()}
            closeOnBackdrop={false}
            platform={permissions?.platform ?? "unknown"}
            onSave={async (next) => {
              const updatedHotkey = await setHotkey(next.hotkey);
              const updatedUiMode = await setUiMode(next.uiMode);
              const updated = await setSyncSettings({
                enabled: next.enabled,
                provider: next.provider,
                folder: next.folder,
                passphrase: next.passphrase,
                theme: next.theme,
              });
              setSettings({ ...updated, hotkey: updatedHotkey.hotkey, ui_mode: updatedUiMode.ui_mode });
              await closeCurrentWindow();
            }}
            onPickFolder={pickFolder}
          />
        ) : (
          <div className="status">Loading settings…</div>
        )}
      </div>
    );
  }

  return (
    <div className="app">
      <header className="topbar">
        <div className="topbarTabs" role="tablist" aria-label="Tray categories">
          <button 
            className={`topbarTab${activeTab === null ? " isActive" : ""}`}
            role="tab" 
            aria-selected={activeTab === null}
            type="button"
            onClick={() => setActiveTab(null)}
          >
            📋 Clipboard
          </button>
          {categories.map((cat) => (
            <button
              key={cat}
              className={`topbarTab${activeTab === cat ? " isActive" : ""}`}
              role="tab"
              aria-selected={activeTab === cat}
              type="button"
              onClick={() => setActiveTab(cat)}
            >
              {cat}
            </button>
          ))}
        </div>

        <div className="topbarCenter">
          <input
            ref={searchInputRef}
            className="search"
            value={query}
            onChange={(e) => setQuery(e.currentTarget.value)}
            onKeyDownCapture={(e) => {
              console.log("[powerpaste] search input keydown:", e.key, "meta:", e.metaKey);
              const isMod = e.metaKey || e.ctrlKey;
              if (!isMod) return;

              const key = e.key.toLowerCase();

              if (key === "a") {
                console.log("[powerpaste] search input Cmd+A - selecting all cards");
                e.preventDefault();
                e.stopPropagation();
                selectAll();
                return;
              }

              if (key === "c") {
                console.log("[powerpaste] search input Cmd+C - copying selected");
                e.preventDefault();
                e.stopPropagation();
                void copySelected();
              }
            }}
            placeholder="Search..."
            autoFocus
          />
          {syncStatus && <span className="topbarStatus">{syncStatus}</span>}
        </div>

        <div className="actions">
          <button 
            className="btnIcon" 
            onClick={onSyncNow}
            aria-label="Sync now"
            title="Sync now"
          >
            ⟳
          </button>
          <button 
            className="btnIcon" 
            onClick={() => void openSettingsWindow()}
            aria-label="Settings"
            title="Settings"
          >
            ⚙️
          </button>
          <button
            className="btnIcon"
            type="button"
            onClick={() => {
              console.log("[powerpaste] Close button clicked: calling hideMainWindow");
              void hideMainWindow().catch(() => undefined);
            }}
            aria-label="Close"
            title="Close"
          >
            ✕
          </button>
        </div>
      </header>


      {/* Top list view removed — only keep the BottomTray card UI. */}

      <BottomTray
        items={trayItems}
        selectedIds={selectedIds}
        categories={categories}
        activeTab={activeTab}
        onTabChange={setActiveTab}
        onSelect={selectItem}
        onCopy={onCopy}
        onPaste={onPaste}
        onDelete={handleDelete}
        onTogglePin={togglePinned}
        onSaveToTab={(item) => setSaveToTabItem(item)}
        onRemoveFromTab={handleRemoveFromTab}
        contextMenu={contextMenu}
        onContextMenu={(x, y, item) => setContextMenu({ x, y, item })}
        onCloseContextMenu={closeContextMenu}
      />

      {/* Save to Tab Modal */}
      {saveToTabItem && (
        <SaveToTabModal
          categories={categories}
          onSave={(category) => {
            void handleSaveToTab(saveToTabItem, category);
            setSaveToTabItem(null);
          }}
          onCancel={() => setSaveToTabItem(null)}
        />
      )}

      {showPermissions ? (
        <PermissionsModal
          checking={checkingPermissions}
          status={permissions}
          onClose={() => {
            setShowPermissions(false);
          }}
          onRecheck={async () => {
            setCheckingPermissions(true);
            try {
              const res = await checkPermissions();
              setPermissions(res);
              if (res.can_paste) {
                setShowPermissions(false);
              }
            } finally {
              setCheckingPermissions(false);
            }
          }}
          onOpenAccessibility={() => void openAccessibilitySettings()}
          onOpenAutomation={() => void openAutomationSettings()}
        />
      ) : null}
    </div>
  );
}

export default App;
