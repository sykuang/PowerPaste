import { useCallback, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Menu, MenuItem, PredefinedMenuItem } from "@tauri-apps/api/menu";
import { LogicalPosition } from "@tauri-apps/api/dpi";
import { ClipboardItem } from "../api";
import { ContentPreview } from "./ContentPreview";
import { AppIcon } from "./AppIcon";

interface TrayCardProps {
  item: ClipboardItem;
  isSelected: boolean;
  selectedCount: number;
  isTrashView: boolean;
  onSelect: (item: ClipboardItem, opts?: { additive?: boolean; range?: boolean }) => void;
  onCopy: (item: ClipboardItem) => void;
  onPaste: (item: ClipboardItem) => void;
  onDelete: (item: ClipboardItem) => void;
  onTogglePin: (item: ClipboardItem) => void;
  onSaveToPinboard: (item: ClipboardItem) => void;
  onRemoveFromPinboard: (item: ClipboardItem) => void;
  onRestore?: (item: ClipboardItem) => void;
  onDeleteForever?: (item: ClipboardItem) => void;
}

function TrayCard({ item, isSelected, selectedCount, isTrashView, onSelect, onCopy, onPaste, onDelete, onTogglePin, onSaveToPinboard, onRemoveFromPinboard, onRestore, onDeleteForever }: TrayCardProps) {
  const [titleColor, setTitleColor] = useState<string | null>(null);
  // Show native context menu for card
  const showCardContextMenu = async (x: number, y: number) => {
    const menuItems: (MenuItem | PredefinedMenuItem)[] = [];

    if (isTrashView) {
      // Trash view: show Restore and Delete Forever
      const restoreLabel = isSelected && selectedCount > 1
        ? `Restore ${selectedCount} items`
        : "Restore";
      const deleteForeverLabel = isSelected && selectedCount > 1
        ? `Delete ${selectedCount} items forever`
        : "Delete forever";

      menuItems.push(
        await MenuItem.new({ text: restoreLabel, action: () => onRestore?.(item) }),
        await PredefinedMenuItem.new({ item: "Separator" }),
        await MenuItem.new({ text: deleteForeverLabel, action: () => onDeleteForever?.(item) }),
      );
    } else {
      // Normal view: show standard actions
      const deleteLabel = isSelected && selectedCount > 1 
        ? `Delete ${selectedCount} items` 
        : "Delete";

      menuItems.push(
        await MenuItem.new({ text: "Copy", action: () => onCopy(item) }),
        await MenuItem.new({ text: "Paste", action: () => onPaste(item) }),
        await PredefinedMenuItem.new({ item: "Separator" }),
        await MenuItem.new({ 
          text: item.pinned ? "Unpin" : "Pin", 
          action: () => onTogglePin(item) 
        }),
      );
      
      if (item.pinboard) {
        menuItems.push(
          await MenuItem.new({ 
            text: "Remove from pinboard", 
            action: () => onRemoveFromPinboard(item) 
          })
        );
      } else {
        menuItems.push(
          await MenuItem.new({ 
            text: "Save to pinboard...", 
            action: () => onSaveToPinboard(item) 
          })
        );
      }
      
      menuItems.push(
        await PredefinedMenuItem.new({ item: "Separator" }),
        await MenuItem.new({ text: deleteLabel, action: () => onDelete(item) }),
      );
    }

    const menu = await Menu.new({ items: menuItems });
    await menu.popup(new LogicalPosition(x, y));
  };

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
        console.log("[powerpaste] trayCard SINGLE-click for item:", item.id);
        const additive = e.metaKey || e.ctrlKey;
        const range = e.shiftKey;
        onSelect(item, { additive, range });
      }}
      onDoubleClick={(e) => {
        console.log("[powerpaste] trayCard DOUBLE-click detected for item:", item.id);
        e.preventDefault();
        e.stopPropagation();
        onPaste(item);
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        void showCardContextMenu(e.clientX, e.clientY);
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
  isTrashView: boolean;
  onPinboardChange: (pinboard: string | null) => void;
  onSelect: (item: ClipboardItem, opts?: { additive?: boolean; range?: boolean }) => void;
  onCopy: (item: ClipboardItem) => void;
  onPaste: (item: ClipboardItem) => void;
  onDelete: (item: ClipboardItem) => void;
  onTogglePin: (item: ClipboardItem) => void;
  onSaveToPinboard: (item: ClipboardItem) => void;
  onRemoveFromPinboard: (item: ClipboardItem) => void;
  onRestore?: (item: ClipboardItem) => void;
  onDeleteForever?: (item: ClipboardItem) => void;
  onEmptyTrash?: () => void;
  uiMode?: "floating" | "fixed";
}

export function BottomTray(props: BottomTrayProps) {
  const trayCardsRef = useRef<HTMLDivElement>(null);
  const isFloating = props.uiMode === "floating";

  // Gap between cards (matches CSS clamp values)
  const gap = 12; // Use max value from clamp(6px, 0.6vw, 12px)
  const cardWidth = 280;
  const cardHeight = 100; // Estimated height for floating mode

  // Virtualizer configuration
  const virtualizer = useVirtualizer({
    count: props.items.length,
    getScrollElement: () => trayCardsRef.current,
    estimateSize: () => (isFloating ? cardHeight : cardWidth + gap),
    horizontal: !isFloating,
    overscan: 3,
  });

  // Convert vertical mouse wheel to horizontal scroll (only in fixed mode)
  const handleWheel = useCallback((e: React.WheelEvent<HTMLDivElement>) => {
    // In floating mode, let vertical scroll work naturally
    if (isFloating) return;
    
    const container = trayCardsRef.current;
    if (!container) return;
    
    // If there's horizontal scroll (e.g., trackpad), let it work naturally
    if (Math.abs(e.deltaX) > Math.abs(e.deltaY)) return;
    
    // Convert vertical scroll to horizontal
    if (e.deltaY !== 0) {
      e.preventDefault();
      container.scrollLeft += e.deltaY;
    }
  }, [isFloating]);

  const virtualItems = virtualizer.getVirtualItems();

  return (
    <div className="bottomTray" role="region" aria-label="Quick copy tray">
      <div 
        ref={trayCardsRef}
        className="trayCards" 
        aria-label="Clipboard items"
        onWheel={handleWheel}
      >
        {/* Inner container with total size for virtualization */}
        <div
          className="trayCardsInner"
          style={{
            position: "relative",
            width: isFloating ? "100%" : virtualizer.getTotalSize(),
            height: isFloating ? virtualizer.getTotalSize() : "100%",
          }}
        >
          {virtualItems.map((virtualItem) => {
            const item = props.items[virtualItem.index];
            return (
              <div
                key={item.id}
                className="trayCardWrapper"
                style={{
                  position: "absolute",
                  top: isFloating ? virtualItem.start : 0,
                  left: isFloating ? 0 : virtualItem.start,
                  width: isFloating ? "100%" : cardWidth,
                  height: isFloating ? cardHeight : "100%",
                }}
              >
                <TrayCard
                  item={item}
                  isSelected={props.selectedIds.has(item.id)}
                  selectedCount={props.selectedIds.size}
                  isTrashView={props.isTrashView}
                  onSelect={props.onSelect}
                  onCopy={props.onCopy}
                  onPaste={props.onPaste}
                  onDelete={props.onDelete}
                  onTogglePin={props.onTogglePin}
                  onSaveToPinboard={props.onSaveToPinboard}
                  onRemoveFromPinboard={props.onRemoveFromPinboard}
                  onRestore={props.onRestore}
                  onDeleteForever={props.onDeleteForever}
                />
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
