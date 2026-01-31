/**
 * Settings Modal Tests
 *
 * Tests the settings modal UI and functionality including
 * theme switching, sync configuration, and dock icon settings.
 */

import { test, expect } from '../fixtures/tauri.fixture';

test.describe('Settings Modal', () => {
  test.describe('Opening and Closing', () => {
    test('should open settings modal when clicking settings button', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      const settingsButton = tauriPage.locator('[data-testid="settings-button"]');
      await settingsButton.click();

      const modal = tauriPage.locator('[data-testid="settings-modal"]');
      await expect(modal).toBeVisible();
    });

    test('should close modal when clicking close button', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      // Open modal
      await tauriPage.locator('[data-testid="settings-button"]').click();
      const modal = tauriPage.locator('[data-testid="settings-modal"]');
      await expect(modal).toBeVisible();

      // Close modal
      await tauriPage.locator('[data-testid="settings-close-button"]').click();
      await expect(modal).not.toBeVisible();
    });

    test('should close modal when pressing Escape', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      // Open modal
      await tauriPage.locator('[data-testid="settings-button"]').click();
      const modal = tauriPage.locator('[data-testid="settings-modal"]');
      await expect(modal).toBeVisible();

      // Press Escape
      await tauriPage.keyboard.press('Escape');
      await expect(modal).not.toBeVisible();
    });
  });

  test.describe('Theme Switching', () => {
    test('should display theme options', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      // Open settings
      await tauriPage.locator('[data-testid="settings-button"]').click();

      // Look for theme selector
      const themeSelector = tauriPage.locator('[data-testid="theme-selector"]');
      await expect(themeSelector).toBeVisible();
    });

    test('should switch to Glass theme', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      await tauriPage.locator('[data-testid="settings-button"]').click();

      const glassOption = tauriPage.locator('[data-testid="theme-glass"]');
      await glassOption.click();

      // Verify theme is applied (check for theme class on body or container)
      const body = tauriPage.locator('body');
      const className = await body.getAttribute('class');
      expect(className).toContain('glass');
    });

    test('should switch to Midnight theme', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      await tauriPage.locator('[data-testid="settings-button"]').click();

      const midnightOption = tauriPage.locator('[data-testid="theme-midnight"]');
      await midnightOption.click();

      const body = tauriPage.locator('body');
      const className = await body.getAttribute('class');
      expect(className).toContain('midnight');
    });

    test('should switch to Aurora theme', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      await tauriPage.locator('[data-testid="settings-button"]').click();

      const auroraOption = tauriPage.locator('[data-testid="theme-aurora"]');
      await auroraOption.click();

      const body = tauriPage.locator('body');
      const className = await body.getAttribute('class');
      expect(className).toContain('aurora');
    });

    test('should persist theme selection', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      // Select a theme
      await tauriPage.locator('[data-testid="settings-button"]').click();
      await tauriPage.locator('[data-testid="theme-midnight"]').click();
      await tauriPage.locator('[data-testid="settings-close-button"]').click();

      // Reopen settings
      await tauriPage.locator('[data-testid="settings-button"]').click();

      // Verify the theme is still selected
      const midnightOption = tauriPage.locator('[data-testid="theme-midnight"]');
      const isSelected = await midnightOption.getAttribute('data-selected');
      expect(isSelected).toBe('true');
    });
  });

  test.describe('Hotkey Configuration', () => {
    test('should display current hotkey', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      await tauriPage.locator('[data-testid="settings-button"]').click();

      const hotkeyDisplay = tauriPage.locator('[data-testid="hotkey-display"]');
      await expect(hotkeyDisplay).toBeVisible();

      const text = await hotkeyDisplay.textContent();
      // Default hotkey is Cmd+Shift+V or Ctrl+Shift+V
      expect(text).toContain('V');
    });

    test('should have hotkey edit button', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      await tauriPage.locator('[data-testid="settings-button"]').click();

      const editButton = tauriPage.locator('[data-testid="hotkey-edit-button"]');
      await expect(editButton).toBeVisible();
    });
  });

  test.describe('Sync Settings', () => {
    test('should display sync toggle', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      await tauriPage.locator('[data-testid="settings-button"]').click();

      const syncToggle = tauriPage.locator('[data-testid="sync-enabled-toggle"]');
      await expect(syncToggle).toBeVisible();
    });

    test('should show sync provider options when enabled', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      await tauriPage.locator('[data-testid="settings-button"]').click();

      // Enable sync
      const syncToggle = tauriPage.locator('[data-testid="sync-enabled-toggle"]');
      await syncToggle.click();

      // Provider options should appear
      const providerSelector = tauriPage.locator('[data-testid="sync-provider-selector"]');
      await expect(providerSelector).toBeVisible();
    });
  });

  test.describe('Dock Icon Settings (macOS)', () => {
    test.skip(process.platform !== 'darwin', 'macOS only');

    test('should display dock icon toggle', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      await tauriPage.locator('[data-testid="settings-button"]').click();

      const dockToggle = tauriPage.locator('[data-testid="dock-icon-toggle"]');
      await expect(dockToggle).toBeVisible();
    });

    test('should toggle dock icon visibility', async ({ tauriPage }) => {
      await tauriPage.waitForLoadState('domcontentloaded');

      await tauriPage.locator('[data-testid="settings-button"]').click();

      const dockToggle = tauriPage.locator('[data-testid="dock-icon-toggle"]');
      const initialState = await dockToggle.getAttribute('aria-checked');

      await dockToggle.click();

      const newState = await dockToggle.getAttribute('aria-checked');
      expect(newState).not.toBe(initialState);
    });
  });
});
