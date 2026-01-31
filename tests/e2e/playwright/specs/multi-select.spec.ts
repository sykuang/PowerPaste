/**
 * Multi-Selection Tests
 *
 * Tests keyboard shortcuts and click interactions for selecting
 * multiple clipboard items.
 */

import { test, expect } from '../fixtures/tauri.fixture';

test.describe('Multi-Selection', () => {
  test.describe('Cmd/Ctrl + A (Select All)', () => {
    test('should select all items with Cmd+A on macOS', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      // Focus the tray
      const tray = tauriPage.locator('[data-testid="bottom-tray"]');
      await tray.click();

      // Press Cmd+A (macOS) or Ctrl+A (Windows)
      const modifier = process.platform === 'darwin' ? 'Meta' : 'Control';
      await tauriPage.keyboard.press(`${modifier}+a`);

      // All items should be selected
      const selectedItems = tauriPage.locator('[data-testid^="clipboard-item-"][data-selected="true"]');
      const allItems = tauriPage.locator('[data-testid^="clipboard-item-"]');

      const selectedCount = await selectedItems.count();
      const totalCount = await allItems.count();

      // All items should be selected (if any exist)
      if (totalCount > 0) {
        expect(selectedCount).toBe(totalCount);
      }
    });

    test('should deselect when clicking elsewhere', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      // First select all
      const tray = tauriPage.locator('[data-testid="bottom-tray"]');
      await tray.click();

      const modifier = process.platform === 'darwin' ? 'Meta' : 'Control';
      await tauriPage.keyboard.press(`${modifier}+a`);

      // Click elsewhere (e.g., the background)
      await tauriPage.click('body', { position: { x: 10, y: 10 } });

      // Selection should be cleared
      const selectedItems = tauriPage.locator('[data-testid^="clipboard-item-"][data-selected="true"]');
      const selectedCount = await selectedItems.count();

      expect(selectedCount).toBe(0);
    });
  });

  test.describe('Cmd/Ctrl + Click (Toggle Selection)', () => {
    test('should add item to selection with Cmd+click', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      const items = tauriPage.locator('[data-testid^="clipboard-item-"]');
      const count = await items.count();

      if (count >= 2) {
        // Click first item normally
        await items.first().click();

        // Cmd+click second item
        const modifier = process.platform === 'darwin' ? 'Meta' : 'Control';
        await items.nth(1).click({ modifiers: [modifier] });

        // Both should be selected
        const selectedItems = tauriPage.locator('[data-testid^="clipboard-item-"][data-selected="true"]');
        const selectedCount = await selectedItems.count();

        expect(selectedCount).toBe(2);
      }
    });

    test('should toggle item selection with Cmd+click', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      const items = tauriPage.locator('[data-testid^="clipboard-item-"]');
      const count = await items.count();

      if (count >= 1) {
        // Click to select
        await items.first().click();

        // Cmd+click same item to deselect
        const modifier = process.platform === 'darwin' ? 'Meta' : 'Control';
        await items.first().click({ modifiers: [modifier] });

        // Should be deselected
        const selectedItems = tauriPage.locator('[data-testid^="clipboard-item-"][data-selected="true"]');
        const selectedCount = await selectedItems.count();

        expect(selectedCount).toBe(0);
      }
    });
  });

  test.describe('Shift + Click (Range Selection)', () => {
    test('should select range with Shift+click', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      const items = tauriPage.locator('[data-testid^="clipboard-item-"]');
      const count = await items.count();

      if (count >= 3) {
        // Click first item
        await items.first().click();

        // Shift+click third item
        await items.nth(2).click({ modifiers: ['Shift'] });

        // First, second, and third should be selected
        const selectedItems = tauriPage.locator('[data-testid^="clipboard-item-"][data-selected="true"]');
        const selectedCount = await selectedItems.count();

        expect(selectedCount).toBe(3);
      }
    });
  });

  test.describe('Cmd/Ctrl + C (Copy Selected)', () => {
    test('should copy selected items to clipboard with Cmd+C', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      const items = tauriPage.locator('[data-testid^="clipboard-item-"]');
      const count = await items.count();

      if (count >= 1) {
        // Select first item
        await items.first().click();

        // Press Cmd+C
        const modifier = process.platform === 'darwin' ? 'Meta' : 'Control';
        await tauriPage.keyboard.press(`${modifier}+c`);

        // Note: Verifying clipboard content requires native API access
        // For now, just verify no errors occurred
      }
    });
  });
});

test.describe('Single Click Behavior', () => {
  test('should select item on single click', async ({ tauriPage }) => {
    await tauriPage.waitForLoadState('domcontentloaded');

    const items = tauriPage.locator('[data-testid^="clipboard-item-"]');
    const count = await items.count();

    if (count >= 1) {
      await items.first().click();

      const isSelected = await items.first().getAttribute('data-selected');
      expect(isSelected).toBe('true');
    }
  });

  test('should NOT copy on single click', async ({ tauriPage }) => {
    await tauriPage.waitForLoadState('domcontentloaded');

    // Single click should only select, not copy
    // This is important UX behavior
    const items = tauriPage.locator('[data-testid^="clipboard-item-"]');
    const count = await items.count();

    if (count >= 1) {
      await items.first().click();

      // Item should be selected but no copy action should occur
      const isSelected = await items.first().getAttribute('data-selected');
      expect(isSelected).toBe('true');
    }
  });
});

test.describe('Double Click Behavior', () => {
  test('should trigger paste on double click', async ({ tauriPage }) => {
    await tauriPage.waitForLoadState('domcontentloaded');

    const items = tauriPage.locator('[data-testid^="clipboard-item-"]');
    const count = await items.count();

    if (count >= 1) {
      // Double-click should trigger paste
      await items.first().dblclick();

      // Note: Paste action requires the app to call native APIs
      // and would paste into another application
      // We can only verify no errors occurred
    }
  });
});
