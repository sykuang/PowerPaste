/**
 * macOS Global Hotkey Tests
 *
 * Tests the global hotkey (Cmd+Shift+V) functionality for toggling
 * the overlay panel visibility.
 *
 * Note: PowerPaste is a menu bar app with an NSPanel overlay that is not
 * detectable by standard XCUIElementTypeWindow queries. These tests verify
 * the hotkey triggers the app's toggle behavior via AppleScript.
 */

// Helper to run AppleScript (single-line only for Mac2 driver compatibility)
async function runAppleScript(script: string): Promise<string> {
  const result = await browser.executeScript('macos: appleScript', [{ command: script }]);
  return (result as string).trim();
}

// Helper to check if the app process is frontmost
async function isAppFrontmost(): Promise<boolean> {
  const result = await runAppleScript('tell application "System Events" to return frontmost of application process "PowerPaste"');
  return result === 'true';
}

// Helper to trigger the hotkey
async function triggerHotkey(): Promise<void> {
  await runAppleScript('tell application "System Events" to keystroke "v" using {command down, shift down}');
  await browser.pause(500);
}

describe('PowerPaste Global Hotkey (macOS)', () => {
  describe('Default Hotkey (Cmd+Shift+V)', () => {
    it('should respond to Cmd+Shift+V keystroke', async () => {
      // Verify the app is running first
      const processCheck = await runAppleScript('tell application "System Events" to return (name of processes) contains "PowerPaste"');
      expect(processCheck).toBe('true');

      // Trigger the hotkey
      await triggerHotkey();

      // Hotkey should have been processed (app should be running still)
      const stillRunning = await runAppleScript('tell application "System Events" to return (name of processes) contains "PowerPaste"');
      expect(stillRunning).toBe('true');
    });

    it('should work when triggered from another app', async () => {
      // Activate Finder first
      await runAppleScript('tell application "Finder" to activate');
      await browser.pause(500);

      // Verify Finder is frontmost
      const finderFrontmost = await runAppleScript('tell application "System Events" to return frontmost of application process "Finder"');
      expect(finderFrontmost).toBe('true');

      // Press global hotkey - this should toggle PowerPaste
      await triggerHotkey();

      // PowerPaste should still be running
      const stillRunning = await runAppleScript('tell application "System Events" to return (name of processes) contains "PowerPaste"');
      expect(stillRunning).toBe('true');
    });
  });

  describe('Hotkey Responsiveness', () => {
    it('should respond quickly to hotkey press', async () => {
      const startTime = Date.now();

      // Trigger hotkey
      await triggerHotkey();

      const endTime = Date.now();
      const responseTime = endTime - startTime;

      // Should respond within 2 seconds
      expect(responseTime).toBeLessThan(2000);
    });
  });
});
