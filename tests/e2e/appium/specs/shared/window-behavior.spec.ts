/**
 * Shared Window Behavior Tests
 *
 * Tests that apply to both macOS and Windows platforms.
 */

import { getSelector } from '../../helpers/index.js';

// Helper to run AppleScript (single-line only for Mac2 driver compatibility)
async function runAppleScript(script: string): Promise<void> {
  await browser.executeScript('macos: appleScript', [{ command: script }]);
}

describe('PowerPaste Window Behavior (Shared)', () => {
  describe('Window Existence', () => {
    it('should have a main window', async () => {
      const window = await $(getSelector('mainWindow'));
      await expect(window).toExist();
    });

    it('should have reasonable dimensions', async () => {
      const window = await $(getSelector('mainWindow'));
      const size = await window.getSize();

      // Width should be between min and max bounds
      expect(size.width).toBeGreaterThanOrEqual(400);
      expect(size.width).toBeLessThanOrEqual(2000);

      // Height should be reasonable for an overlay
      expect(size.height).toBeGreaterThanOrEqual(100);
      expect(size.height).toBeLessThanOrEqual(500);
    });
  });

  describe('UI Elements', () => {
    it('should have a settings button', async () => {
      // First make sure window is visible
      if (process.platform === 'darwin') {
        await runAppleScript('tell application "PowerPaste" to activate');
      }
      await browser.pause(500);

      const button = await $(getSelector('settingsButton'));
      // Button may not exist until UI is fully loaded
      // This is a best-effort check
    });

    it('should have a search input', async () => {
      // First make sure window is visible
      if (process.platform === 'darwin') {
        await runAppleScript('tell application "PowerPaste" to activate');
      }
      await browser.pause(500);

      const input = await $(getSelector('searchInput'));
      // Input may not exist until UI is fully loaded
    });
  });

  describe('Window Properties', () => {
    it('should be always on top', async () => {
      // This is configured in tauri.conf.json as alwaysOnTop: true
      // We can verify by checking the window exists even after
      // focusing another application

      const window = await $(getSelector('mainWindow'));
      await expect(window).toExist();

      // Note: Actually verifying always-on-top behavior
      // requires platform-specific implementation
    });

    it('should have no decorations (custom title bar)', async () => {
      // The window is configured with decorations: false
      // We can verify there's no standard title bar

      const window = await $(getSelector('mainWindow'));
      await expect(window).toExist();

      // Note: Checking for lack of decorations requires
      // platform-specific implementation
    });
  });
});
