import { useMemo } from "react";
import { ClipboardItem } from "../api";

interface BottomTrayProps {
  items: ClipboardItem[];
  selectedIds: Set<string>;
  onSelect: (item: ClipboardItem, opts?: { additive?: boolean; range?: boolean }) => void;
  onCopy: (item: ClipboardItem) => void;
  onPaste: (item: ClipboardItem) => void;
  onDelete: (item: ClipboardItem) => void;
  onTogglePin: (item: ClipboardItem) => void;
  contextMenu: { x: number; y: number; item: ClipboardItem } | null;
  onContextMenu: (x: number, y: number, item: ClipboardItem) => void;
  onCloseContextMenu: () => void;
}

export function BottomTray(props: BottomTrayProps) {
  const clampedMenuPosition = useMemo(() => {
    if (!props.contextMenu) return null;
    const MENU_W = 220;
    const MENU_H = 190;
    const margin = 8;

    const maxLeft = Math.max(margin, window.innerWidth - MENU_W - margin);
    const maxTop = Math.max(margin, window.innerHeight - MENU_H - margin);

    const left = Math.min(Math.max(props.contextMenu.x, margin), maxLeft);
    const top = Math.min(Math.max(props.contextMenu.y, margin), maxTop);
    return { left, top };
  }, [props.contextMenu]);

  return (
    <div className="bottomTray" role="region" aria-label="Quick copy tray">
      <div className="trayHeader">
        <div className="trayTabs" role="tablist" aria-label="Tray categories">
          <button className="trayTab isActive" role="tab" aria-selected="true" type="button">
            Clipboard
          </button>
          <button className="trayTab" role="tab" aria-selected="false" type="button" disabled>
            Useful Links
          </button>
          <button className="trayTab" role="tab" aria-selected="false" type="button" disabled>
            Code Snippets
          </button>
          <button className="trayTab" role="tab" aria-selected="false" type="button" disabled>
            Assets
          </button>
        </div>
      </div>

      <div className="trayCards" aria-label="Clipboard items">
        {props.items.map((item) => {
          const title = (item.text.split(/\r?\n/)[0] ?? "").trim();
          const isSelected = props.selectedIds.has(item.id);
          return (
            <div
              key={item.id}
              className={`trayCard${isSelected ? " isSelected" : ""}`}
              role="button"
              tabIndex={0}
              onClick={(e) => {
                const additive = e.metaKey || e.ctrlKey;
                const range = e.shiftKey;
                props.onSelect(item, { additive, range });
              }}
              onDoubleClick={() => {
                props.onSelect(item);
                void props.onCopy(item);
              }}
              onContextMenu={(e) => {
                e.preventDefault();
                props.onContextMenu(e.clientX, e.clientY, item);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  props.onSelect(item);
                  void props.onCopy(item);
                }
              }}
              title="Click to select • Double-click to copy • Right-click for options"
            >
              <div className="trayCardTop">
                <div className="trayCardTitle">{title || "(empty)"}</div>
                <button
                  className="trayCopyBtn"
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    props.onSelect(item);
                    void props.onCopy(item);
                  }}
                  title="Copy"
                >
                  Copy
                </button>
              </div>
              <div className="trayCardBody">
                <div className="trayCardText">{item.text}</div>
                <div className="trayCardMeta">
                  {item.pinned ? "Pinned • " : ""}
                  {item.text.length} chars
                </div>
              </div>
            </div>
          );
        })}
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
          <button
            className="contextMenuItem"
            onClick={() => {
              props.onTogglePin(props.contextMenu!.item);
              props.onCloseContextMenu();
            }}
          >
            {props.contextMenu.item.pinned ? "Unpin" : "Pin"}
          </button>
          <div className="contextMenuDivider" />
          <button
            className="contextMenuItem contextMenuItemDanger"
            onClick={() => {
              props.onDelete(props.contextMenu!.item);
              props.onCloseContextMenu();
            }}
          >
            Delete
          </button>
        </div>
      )}
    </div>
  );
}
