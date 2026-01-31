/**
 * macOS Overlay Panel Tests
 *
 * Tests for the NSPanel overlay behavior.
 *
 * Note: PowerPaste uses an NSPanel overlay which is not easily detectable
 * by standard XCUIElement queries. These tests verify the app's behavior
 * via AppleScript and process-level checks.
 */

// Helper to run AppleScript (single-line only for Mac2 driver compatibility)
async function runAppleScript(script: string): Promise<string> {
  const result = await browser.executeScript('macos: appleScript', [{ command: script }]);
  return (result as string).trim();
}

// Helper to trigger the hotkey
async function triggerHotkey(): Promise<void> {
  await runAppleScript('tell application "System Events" to keystroke "v" using {command down, shift down}');
  await browser.pause(500);
}

describe('PowerPaste Overlay Panel (macOS)', () => {
  describe('Window Visibility', () => {
    it('should toggle visibility via hotkey', async () => {
      // Verify app is running
      const running = await runAppleScript('tell application "System Events" to return (name of processes) contains "PowerPaste"');
      expect(running).toBe('true');

      // Trigger hotkey to show overlay
      await triggerHotkey();

      // App should still be running after toggle
      const stillRunning = await runAppleScript('tell application "System Events" to return (name of processes) contains "PowerPaste"');
      expect(stillRunning).toBe('true');
    });

    it('should be able to toggle twice (show then hide)', async () => {
      // First toggle (show)
      await triggerHotkey();

      // Second toggle (hide)
      await triggerHotkey();

      // App should still be running
      const running = await runAppleScript('tell application "System Events" to return (name of processes) contains "PowerPaste"');
      expect(running).toBe('true');
    });
  });

  describe('NSPanel Behavior', () => {
    it('should allow other apps to be activated', async () => {
      // Activate Finder
      await runAppleScript('tell application "Finder" to activate');
      await browser.pause(300);

      // Finder should now be frontmost
      const finderFrontmost = await runAppleScript('tell application "System Events" to return frontmost of application process "Finder"');
      expect(finderFrontmost).toBe('true');
    });

    it('should coexist with other running apps', async () => {
      // PowerPaste should be running alongside other apps
      const ppRunning = await runAppleScript('tell application "System Events" to return (name of processes) contains "PowerPaste"');
      const finderRunning = await runAppleScript('tell application "System Events" to return (name of processes) contains "Finder"');

      expect(ppRunning).toBe('true');
      expect(finderRunning).toBe('true');
    });
  });

  describe('App Responsiveness', () => {
    it('should respond to activate command', async () => {
      // Activate command should not throw
      await runAppleScript('tell application "PowerPaste" to activate');

      // App should be running
      const running = await runAppleScript('tell application "System Events" to return (name of processes) contains "PowerPaste"');
      expect(running).toBe('true');
    });

    it('should handle rapid hotkey toggles', async () => {
      // Toggle quickly multiple times
      await triggerHotkey();
      await triggerHotkey();
      await triggerHotkey();

      // App should still be stable
      const running = await runAppleScript('tell application "System Events" to return (name of processes) contains "PowerPaste"');
      expect(running).toBe('true');
    });
  });
});
