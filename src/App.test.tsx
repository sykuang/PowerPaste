import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/event", () => {
  return {
    listen: vi.fn(async () => {
      return () => {
        // no-op unlisten
      };
    }),
  };
});

vi.mock("@tauri-apps/plugin-dialog", () => {
  return {
    open: vi.fn(async () => null),
  };
});

vi.mock("./api", () => {
  return {
    getSettings: vi.fn(async () => ({
      device_id: "test-device",
      sync_enabled: false,
      sync_provider: null,
      sync_folder: null,
      sync_salt_b64: null,
      hotkey: "Ctrl+Shift+V",
      theme: "glass",
    })),
    listItems: vi.fn(async () => [
      {
        id: "1",
        kind: "text",
        text: "Hello world",
        created_at_ms: 1,
        pinned: false,
      },
      {
        id: "2",
        kind: "text",
        text: "Second item",
        created_at_ms: 2,
        pinned: true,
      },
    ]),
    setHotkey: vi.fn(async () => ({
      device_id: "test-device",
      sync_enabled: false,
      sync_provider: null,
      sync_folder: null,
      sync_salt_b64: null,
      hotkey: "Ctrl+Shift+V",
      theme: "glass",
    })),
    setSyncSettings: vi.fn(async () => ({
      device_id: "test-device",
      sync_enabled: false,
      sync_provider: null,
      sync_folder: null,
      sync_salt_b64: null,
      hotkey: "Ctrl+Shift+V",
      theme: "glass",
    })),
    setItemPinned: vi.fn(async () => undefined),
    deleteItem: vi.fn(async () => undefined),
    writeClipboardText: vi.fn(async () => undefined),
    pasteText: vi.fn(async () => undefined),
    checkPermissions: vi.fn(async () => ({
      platform: "macos",
      can_paste: true,
      automation_ok: true,
      accessibility_ok: true,
      details: null,
    })),
    openAccessibilitySettings: vi.fn(async () => undefined),
    openAutomationSettings: vi.fn(async () => undefined),
    syncNow: vi.fn(async () => ({ imported: 0 })),
    setOverlayPreferredSize: vi.fn(async () => undefined),
    hideMainWindow: vi.fn(async () => undefined),
  };
});

import App from "./App";
import { writeClipboardText } from "./api";

describe("App", () => {
  it("renders and shows tray clipboard items", async () => {
    render(<App />);

    expect(screen.getByText("PowerPaste")).toBeInTheDocument();
    expect(
      screen.getByPlaceholderText("Search your clipboard history..."),
    ).toBeInTheDocument();

    // Reload runs on mount; wait for mocked items to appear in the bottom tray.
    expect(await screen.findByRole("region", { name: /quick copy tray/i })).toBeInTheDocument();
    expect((await screen.findAllByText("Hello world")).length).toBeGreaterThan(0);
  });

  it("Cmd/Ctrl+A selects all tray cards", async () => {
    render(<App />);

    const tray = await screen.findByRole("region", { name: /quick copy tray/i });

    // Ensure tray cards are rendered first.
    await within(tray).findAllByText("Hello world");
    const cards = Array.from(tray.querySelectorAll<HTMLElement>(".trayCard"));
    expect(cards.length).toBeGreaterThan(0);

    // Trigger the global select-all handler.
    fireEvent.keyDown(window, { key: "a", metaKey: true });

    await waitFor(() => {
      expect(cards.every((el) => el.classList.contains("isSelected"))).toBe(true);
    });
  });

  it("Cmd/Ctrl+C copies selected tray cards", async () => {
    const writeClipboardTextMock = vi.mocked(writeClipboardText);
    writeClipboardTextMock.mockClear();

    render(<App />);

    const tray = await screen.findByRole("region", { name: /quick copy tray/i });
    await within(tray).findAllByText("Hello world");
    const cards = Array.from(tray.querySelectorAll<HTMLElement>(".trayCard"));
    expect(cards.length).toBeGreaterThan(0);

    // Select all cards then copy.
    fireEvent.keyDown(window, { key: "a", metaKey: true });

    await waitFor(() => {
      expect(cards.every((el) => el.classList.contains("isSelected"))).toBe(true);
    });

    const expectedText = cards
      .map((card) => card.querySelector<HTMLElement>(".trayCardText")?.textContent?.trim() ?? "")
      .filter(Boolean)
      .join("\n\n");
    expect(expectedText).not.toBe("");

    fireEvent.keyDown(window, { key: "c", metaKey: true });

    await waitFor(() => {
      expect(writeClipboardTextMock).toHaveBeenCalledWith(expectedText);
    });
  });

  it("single click selects card (does not copy)", async () => {
    const writeClipboardTextMock = vi.mocked(writeClipboardText);
    writeClipboardTextMock.mockClear();

    render(<App />);

    const tray = await screen.findByRole("region", { name: /quick copy tray/i });
    await within(tray).findAllByText("Hello world");
    const cards = Array.from(tray.querySelectorAll<HTMLElement>(".trayCard"));
    expect(cards.length).toBeGreaterThan(0);

    fireEvent.click(cards[0]!);

    await waitFor(() => {
      expect(cards[0]!.classList.contains("isSelected")).toBe(true);
    });
    expect(writeClipboardTextMock).not.toHaveBeenCalled();
  });

  it("Cmd/Ctrl+click toggles multi-select", async () => {
    render(<App />);

    const tray = await screen.findByRole("region", { name: /quick copy tray/i });
    await within(tray).findAllByText("Hello world");
    const cards = Array.from(tray.querySelectorAll<HTMLElement>(".trayCard"));
    expect(cards.length).toBeGreaterThanOrEqual(2);

    fireEvent.click(cards[0]!);
    fireEvent.click(cards[1]!, { metaKey: true });

    await waitFor(() => {
      expect(cards[0]!.classList.contains("isSelected")).toBe(true);
      expect(cards[1]!.classList.contains("isSelected")).toBe(true);
    });
  });

  it("double click copies card content", async () => {
    const writeClipboardTextMock = vi.mocked(writeClipboardText);
    writeClipboardTextMock.mockClear();

    render(<App />);

    const tray = await screen.findByRole("region", { name: /quick copy tray/i });
    await within(tray).findAllByText("Hello world");
    const cards = Array.from(tray.querySelectorAll<HTMLElement>(".trayCard"));
    expect(cards.length).toBeGreaterThan(0);

    const expectedText =
      cards[0]?.querySelector<HTMLElement>(".trayCardText")?.textContent?.trim() ?? "";
    expect(expectedText).not.toBe("");

    fireEvent.doubleClick(cards[0]!);

    await waitFor(() => {
      expect(writeClipboardTextMock).toHaveBeenCalledWith(expectedText);
    });
  });
});
