/**
 * Windows App Launch Tests
 *
 * Tests that PowerPaste launches correctly on Windows and the main window
 * is created with expected properties.
 */

import { getSelector } from '../../helpers/index.js';

describe('PowerPaste App Launch (Windows)', () => {
  it('should launch the app successfully', async () => {
    // Verify the app window exists
    const window = await $(getSelector('mainWindow'));
    await expect(window).toExist();
  });

  it('should have the correct window title', async () => {
    const window = await $(getSelector('mainWindow'));
    const title = await window.getAttribute('Name');
    expect(title).toBe('PowerPaste');
  });

  it('should start with reasonable dimensions', async () => {
    const window = await $(getSelector('mainWindow'));
    const size = await window.getSize();

    // Check size is within expected bounds
    expect(size.width).toBeGreaterThan(400);
    expect(size.width).toBeLessThan(2000);
    expect(size.height).toBeGreaterThan(100);
    expect(size.height).toBeLessThan(500);
  });

  it('should be positioned on screen', async () => {
    const window = await $(getSelector('mainWindow'));
    const location = await window.getLocation();

    // Window should be at a reasonable position (not off-screen)
    expect(location.x).toBeGreaterThanOrEqual(0);
    expect(location.y).toBeGreaterThanOrEqual(0);
  });
});
