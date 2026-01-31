/**
 * Tabs/Categories Tests
 *
 * Tests the tab bar and "Save to Tab" modal for organizing
 * clipboard items into categories.
 */

import { test, expect } from '../fixtures/tauri.fixture';

test.describe('Tab Bar', () => {
  test.describe('Tab Rendering', () => {
    test('should render the tab bar', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      const tabBar = tauriPage.locator('[data-testid="tab-bar"]');
      await expect(tabBar).toBeVisible();
    });

    test('should have "All" tab by default', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      const allTab = tauriPage.locator('[data-testid="tab-all"]');
      await expect(allTab).toBeVisible();
    });

    test('should show "All" tab as selected by default', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      const allTab = tauriPage.locator('[data-testid="tab-all"]');
      const isSelected = await allTab.getAttribute('data-selected');
      expect(isSelected).toBe('true');
    });
  });

  test.describe('Tab Switching', () => {
    test('should switch tabs when clicked', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      // If there are category tabs, click one
      const categoryTabs = tauriPage.locator('[data-testid^="tab-"]:not([data-testid="tab-all"])');
      const count = await categoryTabs.count();

      if (count > 0) {
        await categoryTabs.first().click();

        const isSelected = await categoryTabs.first().getAttribute('data-selected');
        expect(isSelected).toBe('true');

        // "All" tab should no longer be selected
        const allTab = tauriPage.locator('[data-testid="tab-all"]');
        const allSelected = await allTab.getAttribute('data-selected');
        expect(allSelected).toBe('false');
      }
    });

    test('should filter items when switching tabs', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      const items = tauriPage.locator('[data-testid^="clipboard-item-"]');
      const allCount = await items.count();

      // Switch to a category tab if one exists
      const categoryTabs = tauriPage.locator('[data-testid^="tab-"]:not([data-testid="tab-all"])');
      const tabCount = await categoryTabs.count();

      if (tabCount > 0) {
        await categoryTabs.first().click();
        await tauriPage.waitForTimeout(300);

        const filteredCount = await items.count();
        // Filtered count should be <= all count
        expect(filteredCount).toBeLessThanOrEqual(allCount);
      }
    });
  });
});

test.describe('Save to Tab Modal', () => {
  test.describe('Opening Modal', () => {
    test('should open modal from context menu', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      const items = tauriPage.locator('[data-testid^="clipboard-item-"]');
      const count = await items.count();

      if (count > 0) {
        // Right-click to open context menu
        await items.first().click({ button: 'right' });

        // Click "Save to Tab" option
        const saveToTabOption = tauriPage.locator('[data-testid="context-menu-save-to-tab"]');
        await saveToTabOption.click();

        const modal = tauriPage.locator('[data-testid="save-to-tab-modal"]');
        await expect(modal).toBeVisible();
      }
    });
  });

  test.describe('Category Selection', () => {
    test('should display existing categories', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      // Open the modal (assuming we can trigger it)
      const items = tauriPage.locator('[data-testid^="clipboard-item-"]');
      const count = await items.count();

      if (count > 0) {
        await items.first().click({ button: 'right' });
        await tauriPage.locator('[data-testid="context-menu-save-to-tab"]').click();

        const categoryList = tauriPage.locator('[data-testid="category-list"]');
        await expect(categoryList).toBeVisible();
      }
    });

    test('should select a category', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      const items = tauriPage.locator('[data-testid^="clipboard-item-"]');
      const count = await items.count();

      if (count > 0) {
        await items.first().click({ button: 'right' });
        await tauriPage.locator('[data-testid="context-menu-save-to-tab"]').click();

        // Click a category if available
        const categories = tauriPage.locator('[data-testid^="category-option-"]');
        const categoryCount = await categories.count();

        if (categoryCount > 0) {
          await categories.first().click();

          // Modal should close after selection
          const modal = tauriPage.locator('[data-testid="save-to-tab-modal"]');
          await expect(modal).not.toBeVisible();
        }
      }
    });
  });

  test.describe('New Category Creation', () => {
    test('should have "New Category" input', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      const items = tauriPage.locator('[data-testid^="clipboard-item-"]');
      const count = await items.count();

      if (count > 0) {
        await items.first().click({ button: 'right' });
        await tauriPage.locator('[data-testid="context-menu-save-to-tab"]').click();

        const newCategoryInput = tauriPage.locator('[data-testid="new-category-input"]');
        await expect(newCategoryInput).toBeVisible();
      }
    });

    test('should create new category', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      const items = tauriPage.locator('[data-testid^="clipboard-item-"]');
      const count = await items.count();

      if (count > 0) {
        await items.first().click({ button: 'right' });
        await tauriPage.locator('[data-testid="context-menu-save-to-tab"]').click();

        // Type new category name
        const input = tauriPage.locator('[data-testid="new-category-input"]');
        const uniqueName = `Test Category ${Date.now()}`;
        await input.fill(uniqueName);

        // Submit
        await tauriPage.locator('[data-testid="create-category-button"]').click();

        // Modal should close
        const modal = tauriPage.locator('[data-testid="save-to-tab-modal"]');
        await expect(modal).not.toBeVisible();

        // New tab should appear
        const newTab = tauriPage.locator(`[data-testid^="tab-"]`, { hasText: uniqueName });
        await expect(newTab).toBeVisible();
      }
    });

    test('should not allow empty category name', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      const items = tauriPage.locator('[data-testid^="clipboard-item-"]');
      const count = await items.count();

      if (count > 0) {
        await items.first().click({ button: 'right' });
        await tauriPage.locator('[data-testid="context-menu-save-to-tab"]').click();

        // Try to create with empty name
        const createButton = tauriPage.locator('[data-testid="create-category-button"]');
        await createButton.click();

        // Modal should still be open (validation failed)
        const modal = tauriPage.locator('[data-testid="save-to-tab-modal"]');
        await expect(modal).toBeVisible();
      }
    });
  });

  test.describe('Modal Closing', () => {
    test('should close modal when clicking outside', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      const items = tauriPage.locator('[data-testid^="clipboard-item-"]');
      const count = await items.count();

      if (count > 0) {
        await items.first().click({ button: 'right' });
        await tauriPage.locator('[data-testid="context-menu-save-to-tab"]').click();

        const modal = tauriPage.locator('[data-testid="save-to-tab-modal"]');
        await expect(modal).toBeVisible();

        // Click outside the modal
        await tauriPage.click('body', { position: { x: 10, y: 10 } });

        await expect(modal).not.toBeVisible();
      }
    });

    test('should close modal with Escape key', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      const items = tauriPage.locator('[data-testid^="clipboard-item-"]');
      const count = await items.count();

      if (count > 0) {
        await items.first().click({ button: 'right' });
        await tauriPage.locator('[data-testid="context-menu-save-to-tab"]').click();

        const modal = tauriPage.locator('[data-testid="save-to-tab-modal"]');
        await expect(modal).toBeVisible();

        await tauriPage.keyboard.press('Escape');

        await expect(modal).not.toBeVisible();
      }
    });
  });
});
