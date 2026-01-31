/**
 * macOS App Launch Tests
 *
 * Tests that PowerPaste launches correctly on macOS.
 * PowerPaste is a menu bar app that uses an NSPanel overlay which is not
 * visible until triggered via hotkey or tray icon.
 */

// Helper to run AppleScript (single-line only for Mac2 driver compatibility)
async function runAppleScript(script: string): Promise<string> {
  const result = await browser.executeScript('macos: appleScript', [{ command: script }]);
  return (result as string).trim();
}

// Helper to check if an app process is running
async function isAppRunning(): Promise<boolean> {
  const script = `tell application "System Events" to return (name of processes) contains "PowerPaste"`;
  const result = await runAppleScript(script);
  return result === 'true';
}

describe('PowerPaste App Launch (macOS)', () => {
  it('should have the app process running', async () => {
    // PowerPaste is a menu bar app - verify it's running
    const running = await isAppRunning();
    expect(running).toBe(true);
  });

  it('should be able to activate the app', async () => {
    // Activate should not throw
    await runAppleScript('tell application "PowerPaste" to activate');
    // The app should still be running after activation
    const running = await isAppRunning();
    expect(running).toBe(true);
  });

  it('should respond to app commands', async () => {
    // Verify we can query the app
    const frontmost = await runAppleScript('tell application "System Events" to return frontmost of application process "PowerPaste"');
    // The value doesn't matter - just verifying the app responds
    expect(['true', 'false']).toContain(frontmost);
  });
});
