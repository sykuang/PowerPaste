import { useCallback, useMemo, useRef, useState } from "react";
import { ClipboardItem } from "../api";
import { ContentPreview } from "./ContentPreview";
import { AppIcon } from "./AppIcon";

interface TrayCardProps {
  item: ClipboardItem;
  isSelected: boolean;
  onSelect: (item: ClipboardItem, opts?: { additive?: boolean; range?: boolean }) => void;
  onCopy: (item: ClipboardItem) => void;
  onPaste: (item: ClipboardItem) => void;
  onContextMenu: (x: number, y: number, item: ClipboardItem) => void;
}

function TrayCard({ item, isSelected, onSelect, onCopy, onPaste, onContextMenu }: TrayCardProps) {
  const [titleColor, setTitleColor] = useState<string | null>(null);

  // Determine title based on content type
  const title = useMemo(() => {
    if (item.kind === "image") {
      const dims = item.image_width && item.image_height
        ? `${item.image_width}×${item.image_height}`
        : "";
      return `Image${dims ? ` (${dims})` : ""}`;
    }
    if (item.content_type === "url") {
      try {
        const url = new URL(item.text.trim());
        return url.hostname.replace(/^www\./, "");
      } catch {
        return item.text.split(/\r?\n/)[0]?.trim() || "(URL)";
      }
    }
    if (item.content_type === "file" || item.kind === "file") {
      const paths = (item.file_paths || item.text).split("\n").filter(Boolean);
      if (paths.length > 1) return `${paths.length} files`;
      const fileName = paths[0]?.split("/").pop() || paths[0]?.split("\\").pop() || "File";
      return fileName;
    }
    return (item.text.split(/\r?\n/)[0] ?? "").trim() || "(empty)";
  }, [item]);

  // Determine meta info based on content type
  const meta = useMemo(() => {
    const parts: string[] = [];
    if (item.pinned) parts.push("Pinned");
    
    if (item.kind === "image") {
      if (item.image_size_bytes) {
        const kb = item.image_size_bytes / 1024;
        parts.push(kb >= 1024 ? `${(kb / 1024).toFixed(1)} MB` : `${kb.toFixed(0)} KB`);
      }
    } else if (item.content_type === "url") {
      parts.push("Link");
    } else if (item.content_type === "file" || item.kind === "file") {
      parts.push("File");
    } else {
      parts.push(`${item.text.length} chars`);
    }
    
    return parts.join(" • ");
  }, [item]);

  // Add content-type specific class
  const cardClasses = [
    "trayCard",
    isSelected ? "isSelected" : "",
    item.kind === "image" ? "trayCardImage" : "",
    item.content_type === "url" ? "trayCardUrl" : "",
    item.content_type === "file" || item.kind === "file" ? "trayCardFile" : "",
  ].filter(Boolean).join(" ");

  return (
    <div
      className={cardClasses}
      role="button"
      tabIndex={0}
      onClick={(e) => {
        const additive = e.metaKey || e.ctrlKey;
        const range = e.shiftKey;
        onSelect(item, { additive, range });
      }}
      onDoubleClick={(e) => {
        console.log("[powerpaste] trayCard double-click detected for item:", item.id);
        e.preventDefault();
        e.stopPropagation();
        onPaste(item);
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(e.clientX, e.clientY, item);
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onPaste(item);
        }
      }}
      title="Click to select • Double-click to paste • Right-click for options"
    >
      <div 
        className="trayCardTop"
        style={titleColor ? { backgroundColor: `${titleColor}20`, borderColor: `${titleColor}40` } : undefined}
      >
        {item.source_app_bundle_id && (
          <AppIcon
            bundleId={item.source_app_bundle_id}
            appName={item.source_app_name}
            size={36}
            className="trayCardAppIcon"
            onColorExtracted={setTitleColor}
          />
        )}
        <div className="trayCardTitle">
          {title}
        </div>
        <button
          className="trayCopyBtn"
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onSelect(item);
            void onCopy(item);
          }}
          title="Copy"
        >
          Copy
        </button>
      </div>
      <div className="trayCardBody">
        <ContentPreview item={item} />
        <div className="trayCardMeta">{meta}</div>
      </div>
    </div>
  );
}

interface BottomTrayProps {
  items: ClipboardItem[];
  selectedIds: Set<string>;
  pinboards: string[];
  activePinboard: string | null; // null = Clipboard
  onPinboardChange: (pinboard: string | null) => void;
  onSelect: (item: ClipboardItem, opts?: { additive?: boolean; range?: boolean }) => void;
  onCopy: (item: ClipboardItem) => void;
  onPaste: (item: ClipboardItem) => void;
  onDelete: (item: ClipboardItem) => void;
  onTogglePin: (item: ClipboardItem) => void;
  onSaveToPinboard: (item: ClipboardItem) => void;
  onRemoveFromPinboard: (item: ClipboardItem) => void;
  contextMenu: { x: number; y: number; item: ClipboardItem } | null;
  onContextMenu: (x: number, y: number, item: ClipboardItem) => void;
  onCloseContextMenu: () => void;
}

export function BottomTray(props: BottomTrayProps) {
  const trayCardsRef = useRef<HTMLDivElement>(null);

  // Convert vertical mouse wheel to horizontal scroll
  const handleWheel = useCallback((e: React.WheelEvent<HTMLDivElement>) => {
    const container = trayCardsRef.current;
    if (!container) return;
    
    // If there's horizontal scroll (e.g., trackpad), let it work naturally
    if (Math.abs(e.deltaX) > Math.abs(e.deltaY)) return;
    
    // Convert vertical scroll to horizontal
    if (e.deltaY !== 0) {
      e.preventDefault();
      container.scrollLeft += e.deltaY;
    }
  }, []);

  const clampedMenuPosition = useMemo(() => {
    if (!props.contextMenu) return null;
    const MENU_W = 220;
    const MENU_H = 190;
    const margin = 8;

    // Bottom-align the menu with the cursor (cursor at menu bottom)
    let top = props.contextMenu.y - MENU_H;
    
    // Ensure menu doesn't go above the top of the screen
    if (top < margin) {
      top = margin;
    }

    const maxLeft = Math.max(margin, window.innerWidth - MENU_W - margin);
    const left = Math.min(Math.max(props.contextMenu.x, margin), maxLeft);
    
    return { left, top };
  }, [props.contextMenu]);

  // Determine if context menu item is part of selection
  const contextItemIsSelected = useMemo(() => {
    if (!props.contextMenu) return false;
    return props.selectedIds.has(props.contextMenu.item.id);
  }, [props.contextMenu, props.selectedIds]);

  // Count of items that will be deleted
  const deleteCount = useMemo(() => {
    if (!props.contextMenu) return 0;
    return contextItemIsSelected && props.selectedIds.size > 0 
      ? props.selectedIds.size 
      : 1;
  }, [props.contextMenu, contextItemIsSelected, props.selectedIds]);

  return (
    <div className="bottomTray" role="region" aria-label="Quick copy tray">
      <div 
        ref={trayCardsRef}
        className="trayCards" 
        aria-label="Clipboard items"
        onWheel={handleWheel}
      >
        {props.items.map((item) => (
          <TrayCard
            key={item.id}
            item={item}
            isSelected={props.selectedIds.has(item.id)}
            onSelect={props.onSelect}
            onCopy={props.onCopy}
            onPaste={props.onPaste}
            onContextMenu={props.onContextMenu}
          />
        ))}
      </div>

      {/* Context Menu */}
      {props.contextMenu && (
        <div
          className="contextMenu"
          style={{
            position: "fixed",
            left: clampedMenuPosition?.left ?? props.contextMenu.x,
            top: clampedMenuPosition?.top ?? props.contextMenu.y,
          }}
          onClick={(e) => e.stopPropagation()}
        >
          <div className="contextMenuHeader">
            <div className="contextMenuTitle" title={props.contextMenu.item.text}>
              {(props.contextMenu.item.text.split(/\r?\n/)[0] ?? "").trim() || "(empty)"}
            </div>
            <button
              type="button"
              className="contextMenuClose"
              onClick={() => props.onCloseContextMenu()}
              aria-label="Close menu"
              title="Close"
            >
              ×
            </button>
          </div>
          <button
            className="contextMenuItem"
            onClick={() => {
              props.onCopy(props.contextMenu!.item);
              props.onCloseContextMenu();
            }}
          >
            Copy
          </button>
          <button
            className="contextMenuItem"
            onClick={() => {
              props.onPaste(props.contextMenu!.item);
              props.onCloseContextMenu();
            }}
          >
            Paste
          </button>
          
          {/* Show different actions based on whether item is in a pinboard */}
          {props.contextMenu.item.pinboard ? (
            <button
              className="contextMenuItem"
              onClick={() => {
                props.onRemoveFromPinboard(props.contextMenu!.item);
                props.onCloseContextMenu();
              }}
            >
              Remove from pinboard
            </button>
          ) : (
            <button
              className="contextMenuItem"
              onClick={() => {
                props.onSaveToPinboard(props.contextMenu!.item);
                props.onCloseContextMenu();
              }}
            >
              Save to pinboard...
            </button>
          )}
          
          <div className="contextMenuDivider" />
          <button
            className="contextMenuItem contextMenuItemDanger"
            onClick={() => {
              props.onDelete(props.contextMenu!.item);
              props.onCloseContextMenu();
            }}
          >
            {deleteCount > 1 ? `Delete ${deleteCount} items` : "Delete"}
          </button>
        </div>
      )}
    </div>
  );
}
