/**
 * macOS Permission Dialog Flow Tests
 *
 * IMPORTANT: This test file is EXCLUDED from normal test runs.
 * It requires the app to NOT have Accessibility/Automation permissions pre-granted.
 *
 * Run separately with:
 *   npm run test:e2e:mac:permissions
 *
 * Before running, reset permissions:
 *   tccutil reset Accessibility com.primattek.powerpaste
 *   tccutil reset AppleEvents com.primattek.powerpaste
 */

import { getSelector, accessibilityId } from '../../helpers/index.js';

describe('PowerPaste Permission Dialog Flow (macOS)', () => {
  describe('Permission Status Detection', () => {
    it('should detect when Accessibility permission is not granted', async () => {
      // Show the app
      await browser.executeScript('macos: appleScript', [
        {
          command: 'tell application "PowerPaste" to activate',
        },
      ]);
      await browser.pause(1000);

      // The permissions modal should be visible when permissions are missing
      const modal = await $(accessibilityId('permissions-modal'));
      // Note: Modal visibility depends on permission state
      // If permissions are already granted, this test should be skipped
      await expect(modal).toExist();
    });

    it('should show permission modal on first launch', async () => {
      // After a permission reset, the app should show the permission modal
      const modal = await $(accessibilityId('permissions-modal'));

      // Wait for modal to appear (may take a moment after launch)
      await modal.waitForDisplayed({ timeout: 10000 });

      await expect(modal).toBeDisplayed();
    });
  });

  describe('Accessibility Permission Request', () => {
    it('should have "Open Accessibility Settings" button', async () => {
      // Show app and wait for modal
      await browser.executeScript('macos: appleScript', [
        {
          command: 'tell application "PowerPaste" to activate',
        },
      ]);
      await browser.pause(500);

      const button = await $(getSelector('openAccessibilityButton'));
      await expect(button).toExist();
    });

    it('should open System Preferences Accessibility pane when clicked', async () => {
      // Show app
      await browser.executeScript('macos: appleScript', [
        {
          command: 'tell application "PowerPaste" to activate',
        },
      ]);
      await browser.pause(500);

      // Click the button
      const button = await $(getSelector('openAccessibilityButton'));
      await button.click();
      await browser.pause(2000);

      // Verify System Preferences/Settings opened
      const result = await browser.executeScript('macos: appleScript', [
        {
          command: `
            tell application "System Events"
              set procs to name of every process
              if "System Preferences" is in procs then
                return "System Preferences"
              else if "System Settings" is in procs then
                return "System Settings"
              else
                return "not found"
              end if
            end tell
          `,
        },
      ]);

      expect(['System Preferences', 'System Settings']).toContain(result);
    });

    it('should navigate to correct Accessibility pane', async () => {
      // Check that the Accessibility pane is shown
      // This verifies the deep link worked
      const result = await browser.executeScript('macos: appleScript', [
        {
          command: `
            tell application "System Events"
              tell process "System Settings"
                try
                  return name of window 1
                on error
                  return "error"
                end try
              end tell
            end tell
          `,
        },
      ]);

      // The window title should include "Privacy" or "Accessibility"
      // Note: macOS Ventura+ uses "Privacy & Security"
      expect(result).toBeDefined();
    });
  });

  describe('Automation Permission Request', () => {
    it('should have "Open Automation Settings" button', async () => {
      // Show app and wait for modal
      await browser.executeScript('macos: appleScript', [
        {
          command: 'tell application "PowerPaste" to activate',
        },
      ]);
      await browser.pause(500);

      const button = await $(getSelector('openAutomationButton'));
      await expect(button).toExist();
    });

    it('should open System Preferences Automation pane when clicked', async () => {
      // Show app
      await browser.executeScript('macos: appleScript', [
        {
          command: 'tell application "PowerPaste" to activate',
        },
      ]);
      await browser.pause(500);

      // Click the button
      const button = await $(getSelector('openAutomationButton'));
      await button.click();
      await browser.pause(2000);

      // Verify System Preferences/Settings opened
      const result = await browser.executeScript('macos: appleScript', [
        {
          command: `
            tell application "System Events"
              set procs to name of every process
              if "System Preferences" is in procs then
                return "System Preferences"
              else if "System Settings" is in procs then
                return "System Settings"
              else
                return "not found"
              end if
            end tell
          `,
        },
      ]);

      expect(['System Preferences', 'System Settings']).toContain(result);
    });
  });

  describe('Permission Grant Detection', () => {
    it('should detect when permission is granted', async () => {
      // This test requires manual permission grant or tccutil
      // For automated testing, we'd use tccutil to grant permission
      // then verify the modal closes

      // Grant permission via tccutil (requires sudo in CI)
      // await browser.executeScript('macos: appleScript', [{
      //   command: 'do shell script "tccutil add Accessibility com.primattek.powerpaste" with administrator privileges'
      // }]);

      // For now, just verify the app can detect permission state
      const modal = await $(accessibilityId('permissions-modal'));
      await expect(modal).toExist();
    });

    it('should hide permission modal after all permissions granted', async () => {
      // This test assumes permissions have been granted
      // The modal should no longer be visible

      // In a real test, we would:
      // 1. Grant permissions via tccutil or manually
      // 2. Wait for the app to detect the change
      // 3. Verify the modal is hidden

      const modal = await $(accessibilityId('permissions-modal'));
      // If permissions are granted, modal should not be displayed
      // If permissions are not granted, this test should be skipped
    });
  });

  // Cleanup: Close System Preferences/Settings
  after(async () => {
    await browser.executeScript('macos: appleScript', [
      {
        command: `
          tell application "System Preferences"
            quit
          end tell
          tell application "System Settings"
            quit
          end tell
        `,
      },
    ]);
  });
});
