import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { Menu, MenuItem, PredefinedMenuItem } from "@tauri-apps/api/menu";
import { LogicalPosition } from "@tauri-apps/api/dpi";
import {
  checkPermissions,
  deleteItem,
  enableMouseEvents,
  getSettings,
  listPinboards,
  listItems,
  openAccessibilitySettings,
  openAutomationSettings,
  pasteText,
  hideMainWindow,
  setItemPinboard,
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
import { SaveToPinboardModal } from "./components/SaveToPinboardModal";
import { useSystemAccentColor } from "./hooks/useSystemAccentColor";

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

// Available icons for pinboards
const PINBOARD_ICONS: Record<string, { path: string; viewBox?: string }> = {
  clock: { path: "M8 4v4l3 3M8 14A6 6 0 1 0 8 2a6 6 0 0 0 0 12Z" },
  star: { path: "M8 1l2 5h5l-4 3.5 1.5 5.5L8 12l-4.5 3 1.5-5.5L1 6h5l2-5z" },
  heart: { path: "M8 14s-6-4-6-7.5a3.5 3.5 0 0 1 6-2.5 3.5 3.5 0 0 1 6 2.5c0 3.5-6 7.5-6 7.5z" },
  bookmark: { path: "M3 2h10v13l-5-3-5 3V2z" },
  folder: { path: "M2 4h5l2 2h5v8H2V4z" },
  tag: { path: "M1 8V2h6l7 7-6 6-7-7zm3-3a1 1 0 1 0 0-2 1 1 0 0 0 0 2z" },
  code: { path: "M5 4L1 8l4 4M11 4l4 4-4 4M9 2l-2 12" },
  link: { path: "M6.5 11.5l3-3M10 6a2.5 2.5 0 0 1 0 5H8M6 5a2.5 2.5 0 0 1 0 5h2" },
  image: { path: "M2 3h12v10H2V3zm3 4a1 1 0 1 0 0-2 1 1 0 0 0 0 2zm7 5l-3-4-2 2-2-1-3 3" },
  file: { path: "M4 2h5l4 4v8H4V2zm5 0v4h4" },
  music: { path: "M6 14a2 2 0 1 1 0-4 2 2 0 0 1 0 4zm6-2a2 2 0 1 1 0-4 2 2 0 0 1 0 4zM6 12V4l6-2v8" },
  video: { path: "M2 4h9v8H2V4zm9 2l3-2v8l-3-2" },
  mail: { path: "M2 4h12v8H2V4zm0 0l6 4 6-4" },
  home: { path: "M2 8l6-5 6 5v6H9V9H7v5H2V8z" },
  work: { path: "M6 4V2h4v2h4v10H2V4h4zm0 0h4" },
  circle: { path: "M8 14A6 6 0 1 0 8 2a6 6 0 0 0 0 12Z", viewBox: "0 0 16 16" },
  square: { path: "M3 3h10v10H3V3z" },
  dot: { path: "M8 10a2 2 0 1 0 0-4 2 2 0 0 0 0 4z" },
};

function PinboardIcon({ iconKey, size = 14 }: { iconKey: string; size?: number }) {
  const icon = PINBOARD_ICONS[iconKey] || PINBOARD_ICONS.circle;
  return (
    <svg width={size} height={size} viewBox={icon.viewBox || "0 0 16 16"} fill="none" xmlns="http://www.w3.org/2000/svg">
      <path d={icon.path} stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
    </svg>
  );
}

function App() {
  // Apply system accent color
  useSystemAccentColor();

  const [items, setItems] = useState<ClipboardItem[]>([]);
  const [query, setQuery] = useState("");
  const [searchExpanded, setSearchExpanded] = useState(false);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [syncStatus, setSyncStatus] = useState<string>("");

  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [lastSelectedIndex, setLastSelectedIndex] = useState<number | null>(null);

  const [showPermissions, setShowPermissions] = useState(false);
  const [permissions, setPermissions] = useState<PermissionsStatus | null>(null);
  const [checkingPermissions, setCheckingPermissions] = useState(false);

  // Pinboard state
  const [pinboards, setPinboards] = useState<string[]>([]);
  const [pinboardIcons, setPinboardIcons] = useState<Record<string, string>>({}); // pinboard name -> icon key
  const [clipboardIcon, setClipboardIcon] = useState<string>("clock"); // icon key for Clipboard History
  const [activePinboard, setActivePinboard] = useState<string | null>(null); // null = Clipboard (recent history)
  const [saveToPinboardItem, setSaveToPinboardItem] = useState<ClipboardItem | null>(null);
  const [showNewPinboardModal, setShowNewPinboardModal] = useState(false);
  const [newPinboardName, setNewPinboardName] = useState("");
  
  // Pinboard context menu and edit state
  const [editingPinboard, setEditingPinboard] = useState<string | null>(null); // pinboard being renamed
  const [editingPinboardName, setEditingPinboardName] = useState("");
  const [showIconPicker, setShowIconPicker] = useState<{ pinboard: string | null } | null>(null); // null = clipboard

  // Show native context menu for pinboards
  const showPinboardContextMenu = useCallback(async (e: React.MouseEvent, pinboard: string | null) => {
    e.preventDefault();
    
    try {
      const menuItems: (MenuItem | PredefinedMenuItem)[] = [];
      
      // Change Icon option (for both clipboard and pinboards)
      const changeIconItem = await MenuItem.new({
        text: "Change Icon...",
        action: () => setShowIconPicker({ pinboard }),
      });
      menuItems.push(changeIconItem);

      // Options only for custom pinboards (not clipboard)
      if (pinboard !== null) {
        const separator = await PredefinedMenuItem.new({ item: "Separator" });
        menuItems.push(separator);

        const renameItem = await MenuItem.new({
          text: "Rename...",
          action: () => {
            setEditingPinboard(pinboard);
            setEditingPinboardName(pinboard);
          },
        });
        menuItems.push(renameItem);

        const separator2 = await PredefinedMenuItem.new({ item: "Separator" });
        menuItems.push(separator2);

        const deleteItem = await MenuItem.new({
          text: "Delete",
          action: () => {
            setPinboards((prev) => prev.filter((p) => p !== pinboard));
            if (activePinboard === pinboard) {
              setActivePinboard(null);
            }
            setPinboardIcons((prev) => {
              const { [pinboard]: _, ...rest } = prev;
              return rest;
            });
          },
        });
        menuItems.push(deleteItem);
      }

      const menu = await Menu.new({ items: menuItems });
      await menu.popup(new LogicalPosition(e.clientX, e.clientY));
    } catch (err) {
      console.error("[powerpaste] Failed to show context menu:", err);
    }
  }, [activePinboard]);

  const lastSentOverlaySizeRef = useRef<{ w: number; h: number }>({ w: 0, h: 0 });
  const searchInputRef = useRef<HTMLInputElement>(null);

  const filteredQuery = useMemo(() => query.trim(), [query]);

  const trayItems = useMemo(() => {
    let filtered = [...items];
    
    // Filter by active pinboard
    if (activePinboard === null) {
      // Clipboard tab: show items without a pinboard (recent clipboard history)
      filtered = filtered.filter((item) => !item.pinboard);
    } else {
      // Custom pinboard: show items with matching pinboard
      filtered = filtered.filter((item) => item.pinboard === activePinboard);
    }
    
    filtered.sort((a, b) => {
      if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
      return b.created_at_ms - a.created_at_ms;
    });
    return filtered;
  }, [items, activePinboard]);

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

  const handleSaveToPinboard = useCallback(
    async (item: ClipboardItem, pinboard: string) => {
      let clearAfterMs = 1200;
      try {
        await setItemPinboard(item.id, pinboard);
        setSyncStatus(`Saved to "${pinboard}"`);
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

  const handleRemoveFromPinboard = useCallback(
    async (item: ClipboardItem) => {
      let clearAfterMs = 1200;
      try {
        await setItemPinboard(item.id, null);
        setSyncStatus("Removed from pinboard");
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
    const [s, it, pbs] = await Promise.all([
      getSettings(),
      listItems({ limit: 500, query: filteredQuery || undefined }),
      listPinboards(),
    ]);
    setSettings(s);
    setItems(it);
    setPinboards(pbs);
  }

  // Apply theme with system preference detection and live sync
  useEffect(() => {
    const theme = settings?.theme;
    if (!theme) return;

    const applyTheme = (resolvedTheme: "light" | "dark") => {
      document.documentElement.dataset.theme = resolvedTheme;
    };

    if (theme === "system") {
      // Detect OS preference
      const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
      const handleChange = (e: MediaQueryListEvent | MediaQueryList) => {
        applyTheme(e.matches ? "dark" : "light");
      };
      
      // Apply initial value
      handleChange(mediaQuery);
      
      // Listen for OS theme changes in real-time
      mediaQuery.addEventListener("change", handleChange);
      return () => mediaQuery.removeEventListener("change", handleChange);
    } else {
      // Direct theme: light or dark
      applyTheme(theme);
    }
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

  // Disable browser's default context menu on card elements (we use native Tauri menus)
  useEffect(() => {
    if (IS_SETTINGS_WINDOW) return;
    const handleContextMenu = (e: MouseEvent) => {
      // Only prevent on card elements - native menu will show instead
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

  // Listen for settings_changed event to apply theme and other settings immediately
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    void (async () => {
      const h = await listen<Settings>("settings_changed", (event) => {
        console.log("[powerpaste] settings_changed event received:", event.payload);
        setSettings(event.payload);
      });
      unlisten = h;
    })();
    return () => {
      unlisten?.();
    };
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
        {/* All controls centered together */}
        <div className="topbarCenter" role="tablist" aria-label="Pinboards">
          {/* Search */}
          {!searchExpanded ? (
            <div
              className="topbarIconBtn"
              onClick={() => {
                setSearchExpanded(true);
                setTimeout(() => searchInputRef.current?.focus(), 50);
              }}
              role="button"
              aria-label="Search"
              tabIndex={0}
            >
              <svg
                width="16"
                height="16"
                viewBox="0 0 16 16"
                fill="none"
                xmlns="http://www.w3.org/2000/svg"
              >
                <path d="M7 12A5 5 0 1 0 7 2a5 5 0 0 0 0 10ZM14 14l-3.5-3.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
              </svg>
            </div>
          ) : (
            <input
              ref={searchInputRef}
              className="searchInput"
              value={query}
              onChange={(e) => setQuery(e.currentTarget.value)}
              onKeyDownCapture={(e) => {
                console.log("[powerpaste] search input keydown:", e.key, "meta:", e.metaKey);
                
                if (e.key === "Escape") {
                  setQuery("");
                  setSearchExpanded(false);
                  searchInputRef.current?.blur();
                  return;
                }
                
                const isMod = e.metaKey || e.ctrlKey;
                if (!isMod) return;

                const key = e.key.toLowerCase();
                if (key === "a") {
                  e.preventDefault();
                  e.stopPropagation();
                  selectAll();
                  return;
                }
                if (key === "c") {
                  e.preventDefault();
                  e.stopPropagation();
                  void copySelected();
                }
              }}
              onBlur={() => {
                if (!query.trim()) {
                  setSearchExpanded(false);
                }
              }}
              placeholder="Search..."
            />
          )}

          {/* Clipboard History tab */}
          <button 
            className={`topbarPinboard${activePinboard === null ? " isActive" : ""}`}
            role="tab" 
            aria-selected={activePinboard === null}
            type="button"
            onClick={() => setActivePinboard(null)}
            onContextMenu={(e) => void showPinboardContextMenu(e, null)}
          >
            <PinboardIcon iconKey={clipboardIcon} />
            Clipboard History
          </button>

          {/* Custom pinboards */}
          {pinboards.map((pb) => (
            <button
              key={pb}
              className={`topbarPinboard${activePinboard === pb ? " isActive" : ""}`}
              role="tab"
              aria-selected={activePinboard === pb}
              type="button"
              onClick={() => setActivePinboard(pb)}
              onContextMenu={(e) => void showPinboardContextMenu(e, pb)}
            >
              <PinboardIcon iconKey={pinboardIcons[pb] || "dot"} />
              {editingPinboard === pb ? (
                <input
                  className="pinboardRenameInput"
                  value={editingPinboardName}
                  onChange={(e) => setEditingPinboardName(e.target.value)}
                  onBlur={() => {
                    if (editingPinboardName.trim() && editingPinboardName !== pb) {
                      // Rename pinboard
                      setPinboards((prev) => prev.map((p) => p === pb ? editingPinboardName.trim() : p));
                      // Move icon to new name
                      if (pinboardIcons[pb]) {
                        setPinboardIcons((prev) => {
                          const { [pb]: icon, ...rest } = prev;
                          return { ...rest, [editingPinboardName.trim()]: icon };
                        });
                      }
                      if (activePinboard === pb) {
                        setActivePinboard(editingPinboardName.trim());
                      }
                    }
                    setEditingPinboard(null);
                  }}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      (e.target as HTMLInputElement).blur();
                    } else if (e.key === "Escape") {
                      setEditingPinboard(null);
                    }
                  }}
                  autoFocus
                  onClick={(e) => e.stopPropagation()}
                />
              ) : (
                pb
              )}
            </button>
          ))}

          {/* Add pinboard button */}
          <div
            className="topbarIconBtn"
            role="button"
            aria-label="Add pinboard"
            title="Add pinboard"
            tabIndex={0}
            onClick={() => {
              setNewPinboardName("");
              setShowNewPinboardModal(true);
            }}
          >
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
              <path d="M8 3v10M3 8h10" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/>
            </svg>
          </div>
        </div>

        {/* More menu button */}
        <div
          className="topbarIconBtn topbarSettings"
          role="button"
          aria-label="More options"
          title="More options"
          tabIndex={0}
          onClick={async (e) => {
            const menuItems = [
              await MenuItem.new({ text: "Settings...", action: () => void openSettingsWindow() }),
              await PredefinedMenuItem.new({ item: "Separator" }),
              await MenuItem.new({ text: "Close", action: () => void closeCurrentWindow() }),
            ];
            const menu = await Menu.new({ items: menuItems });
            await menu.popup(new LogicalPosition(e.clientX, e.clientY));
          }}
        >
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
            <circle cx="3" cy="8" r="1.5" fill="currentColor"/>
            <circle cx="8" cy="8" r="1.5" fill="currentColor"/>
            <circle cx="13" cy="8" r="1.5" fill="currentColor"/>
          </svg>
        </div>

        {/* Status messages */}
        {syncStatus && <span className="topbarStatus">{syncStatus}</span>}
      </header>


      {/* Top list view removed — only keep the BottomTray card UI. */}

      <BottomTray
        items={trayItems}
        selectedIds={selectedIds}
        pinboards={pinboards}
        activePinboard={activePinboard}
        onPinboardChange={setActivePinboard}
        onSelect={selectItem}
        onCopy={onCopy}
        onPaste={onPaste}
        onDelete={handleDelete}
        onTogglePin={togglePinned}
        onSaveToPinboard={(item) => setSaveToPinboardItem(item)}
        onRemoveFromPinboard={handleRemoveFromPinboard}
      />

      {/* Save to Pinboard Modal */}
      {saveToPinboardItem && (
        <SaveToPinboardModal
          pinboards={pinboards}
          onSave={(pinboard) => {
            void handleSaveToPinboard(saveToPinboardItem, pinboard);
            setSaveToPinboardItem(null);
          }}
          onCancel={() => setSaveToPinboardItem(null)}
        />
      )}

      {/* New Pinboard Modal */}
      {showNewPinboardModal && (
        <div className="modalBackdrop" onClick={() => setShowNewPinboardModal(false)}>
          <div className="modal newPinboardModal" onClick={(e) => e.stopPropagation()}>
            <h3>New Pinboard</h3>
            <input
              className="input"
              type="text"
              value={newPinboardName}
              onChange={(e) => setNewPinboardName(e.target.value)}
              placeholder="Pinboard name"
              autoFocus
              onKeyDown={(e) => {
                if (e.key === "Enter" && newPinboardName.trim()) {
                  setPinboards((prev) => [...prev, newPinboardName.trim()]);
                  setShowNewPinboardModal(false);
                } else if (e.key === "Escape") {
                  setShowNewPinboardModal(false);
                }
              }}
            />
            <div className="modalActions">
              <button className="btn" onClick={() => setShowNewPinboardModal(false)}>
                Cancel
              </button>
              <button
                className="btn btnPrimary"
                onClick={() => {
                  if (newPinboardName.trim()) {
                    setPinboards((prev) => [...prev, newPinboardName.trim()]);
                    setShowNewPinboardModal(false);
                  }
                }}
                disabled={!newPinboardName.trim()}
              >
                Create
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Icon Picker Modal */}
      {showIconPicker && (
        <div className="modalBackdrop" onClick={() => setShowIconPicker(null)}>
          <div className="modal iconPickerModal" onClick={(e) => e.stopPropagation()}>
            <h3>Choose Icon</h3>
            <div className="iconGrid">
              {Object.keys(PINBOARD_ICONS).map((key) => (
                <button
                  key={key}
                  className={`iconOption${
                    (showIconPicker.pinboard === null ? clipboardIcon : pinboardIcons[showIconPicker.pinboard] || "dot") === key
                      ? " isSelected"
                      : ""
                  }`}
                  onClick={() => {
                    if (showIconPicker.pinboard === null) {
                      setClipboardIcon(key);
                    } else {
                      setPinboardIcons((prev) => ({ ...prev, [showIconPicker.pinboard!]: key }));
                    }
                    setShowIconPicker(null);
                  }}
                  title={key}
                >
                  <PinboardIcon iconKey={key} size={18} />
                </button>
              ))}
            </div>
            <div className="modalActions">
              <button className="btn" onClick={() => setShowIconPicker(null)}>
                Cancel
              </button>
            </div>
          </div>
        </div>
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
