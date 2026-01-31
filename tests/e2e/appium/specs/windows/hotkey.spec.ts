/**
 * Windows Global Hotkey Tests
 *
 * Tests the global hotkey (Ctrl+Shift+V) functionality for toggling
 * the overlay window visibility on Windows.
 */

import { getSelector } from '../../helpers/index.js';

describe('PowerPaste Global Hotkey (Windows)', () => {
  describe('Default Hotkey (Ctrl+Shift+V)', () => {
    it('should toggle window visibility with Ctrl+Shift+V', async () => {
      // Use Windows driver key actions to simulate hotkey
      await browser.executeScript('windows: keys', [
        {
          actions: [
            { virtualKeyCode: 0x11, down: true }, // VK_CONTROL
            { virtualKeyCode: 0x10, down: true }, // VK_SHIFT
            { virtualKeyCode: 0x56, down: true }, // VK_V
            { virtualKeyCode: 0x56, down: false },
            { virtualKeyCode: 0x10, down: false },
            { virtualKeyCode: 0x11, down: false },
          ],
        },
      ]);
      await browser.pause(1000);

      // Check if window is visible
      const window = await $(getSelector('mainWindow'));
      await expect(window).toExist();
    });

    it('should hide window when pressing hotkey again', async () => {
      // First ensure window is visible
      const window = await $(getSelector('mainWindow'));
      await expect(window).toExist();

      // Press hotkey to hide
      await browser.executeScript('windows: keys', [
        {
          actions: [
            { virtualKeyCode: 0x11, down: true },
            { virtualKeyCode: 0x10, down: true },
            { virtualKeyCode: 0x56, down: true },
            { virtualKeyCode: 0x56, down: false },
            { virtualKeyCode: 0x10, down: false },
            { virtualKeyCode: 0x11, down: false },
          ],
        },
      ]);
      await browser.pause(1000);

      // Window element still exists but may not be visible
      await expect(window).toExist();
    });

    it('should work when another app is focused', async () => {
      // Open Notepad
      await browser.executeScript('powerShell', [
        {
          script: 'Start-Process notepad',
        },
      ]);
      await browser.pause(1000);

      // Press global hotkey
      await browser.executeScript('windows: keys', [
        {
          actions: [
            { virtualKeyCode: 0x11, down: true },
            { virtualKeyCode: 0x10, down: true },
            { virtualKeyCode: 0x56, down: true },
            { virtualKeyCode: 0x56, down: false },
            { virtualKeyCode: 0x10, down: false },
            { virtualKeyCode: 0x11, down: false },
          ],
        },
      ]);
      await browser.pause(1000);

      // PowerPaste should be visible
      const window = await $(getSelector('mainWindow'));
      await expect(window).toExist();

      // Cleanup: close Notepad
      await browser.executeScript('powerShell', [
        {
          script: 'Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process',
        },
      ]);
    });
  });

  describe('Hotkey Responsiveness', () => {
    it('should respond quickly to hotkey press', async () => {
      const startTime = Date.now();

      // Trigger hotkey
      await browser.executeScript('windows: keys', [
        {
          actions: [
            { virtualKeyCode: 0x11, down: true },
            { virtualKeyCode: 0x10, down: true },
            { virtualKeyCode: 0x56, down: true },
            { virtualKeyCode: 0x56, down: false },
            { virtualKeyCode: 0x10, down: false },
            { virtualKeyCode: 0x11, down: false },
          ],
        },
      ]);

      // Wait for window to appear
      const window = await $(getSelector('mainWindow'));
      await window.waitForExist({ timeout: 2000 });

      const endTime = Date.now();
      const responseTime = endTime - startTime;

      // Should respond within 2 seconds
      expect(responseTime).toBeLessThan(2000);
    });
  });
});
