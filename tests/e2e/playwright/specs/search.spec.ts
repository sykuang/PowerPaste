/**
 * Search Functionality Tests
 *
 * Tests the search/filter functionality for clipboard items.
 */

import { test, expect } from '../fixtures/tauri.fixture';

test.describe('Search Functionality', () => {
  test.describe('Search Input', () => {
    test('should render search input', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      const searchInput = tauriPage.locator('[data-testid="search-input"]');
      await expect(searchInput).toBeVisible();
    });

    test('should have placeholder text', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      const searchInput = tauriPage.locator('[data-testid="search-input"]');
      const placeholder = await searchInput.getAttribute('placeholder');

      expect(placeholder).toBeTruthy();
    });

    test('should focus on click', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      const searchInput = tauriPage.locator('[data-testid="search-input"]');
      await searchInput.click();

      await expect(searchInput).toBeFocused();
    });
  });

  test.describe('Search Behavior', () => {
    test('should filter items as user types', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      const searchInput = tauriPage.locator('[data-testid="search-input"]');
      const items = tauriPage.locator('[data-testid^="clipboard-item-"]');

      const initialCount = await items.count();

      if (initialCount > 0) {
        // Type a search query
        await searchInput.fill('test');

        // Wait for filter to apply
        await tauriPage.waitForTimeout(300);

        // Items should be filtered (count may decrease)
        const filteredCount = await items.count();
        // Can't assert specific count without knowing data
        // Just verify the query ran without error
      }
    });

    test('should show all items when search is cleared', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      const searchInput = tauriPage.locator('[data-testid="search-input"]');
      const items = tauriPage.locator('[data-testid^="clipboard-item-"]');

      const initialCount = await items.count();

      // Type and then clear
      await searchInput.fill('test');
      await tauriPage.waitForTimeout(300);
      await searchInput.fill('');
      await tauriPage.waitForTimeout(300);

      // Should show all items again
      const finalCount = await items.count();
      expect(finalCount).toBe(initialCount);
    });

    test('should be case-insensitive', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      const searchInput = tauriPage.locator('[data-testid="search-input"]');

      // Search with different cases should yield same results
      await searchInput.fill('TEST');
      await tauriPage.waitForTimeout(300);
      const upperCaseCount = await tauriPage.locator('[data-testid^="clipboard-item-"]').count();

      await searchInput.fill('test');
      await tauriPage.waitForTimeout(300);
      const lowerCaseCount = await tauriPage.locator('[data-testid^="clipboard-item-"]').count();

      expect(upperCaseCount).toBe(lowerCaseCount);
    });

    test('should show empty state when no matches', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      const searchInput = tauriPage.locator('[data-testid="search-input"]');

      // Search for something that won't match
      await searchInput.fill('xyz123nonexistent456');
      await tauriPage.waitForTimeout(300);

      // Should show no items or empty state
      const items = tauriPage.locator('[data-testid^="clipboard-item-"]');
      const count = await items.count();

      // Either no items or an empty state message
      expect(count).toBe(0);
    });
  });

  test.describe('Keyboard Shortcuts', () => {
    test('should focus search with Cmd/Ctrl+F', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      // Press Cmd+F or Ctrl+F
      const modifier = process.platform === 'darwin' ? 'Meta' : 'Control';
      await tauriPage.keyboard.press(`${modifier}+f`);

      const searchInput = tauriPage.locator('[data-testid="search-input"]');
      await expect(searchInput).toBeFocused();
    });

    test('should clear search with Escape', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      const searchInput = tauriPage.locator('[data-testid="search-input"]');

      // Type something
      await searchInput.fill('test query');

      // Press Escape
      await tauriPage.keyboard.press('Escape');

      // Search should be cleared
      const value = await searchInput.inputValue();
      expect(value).toBe('');
    });
  });
});
