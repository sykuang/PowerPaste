/**
 * Global teardown for Playwright tests.
 *
 * Runs after all tests to clean up the test environment.
 */

import * as fs from 'fs';
import * as path from 'path';

async function globalTeardown(): Promise<void> {
  console.log('[playwright:teardown] Starting global teardown');

  // Clean up test databases
  const tempDir = process.platform === 'darwin' ? '/tmp' : process.env.TEMP || '/tmp';

  try {
    const files = fs.readdirSync(tempDir);
    for (const file of files) {
      if (file.startsWith('powerpaste-playwright-') || file.startsWith('powerpaste-test-')) {
        const filePath = path.join(tempDir, file);
        try {
          fs.unlinkSync(filePath);
          console.log(`[playwright:teardown] Removed: ${filePath}`);
        } catch {
          // Ignore errors (file may be in use)
        }
      }
    }
  } catch {
    // Ignore errors reading temp directory
  }

  console.log('[playwright:teardown] Global teardown complete');
}

export default globalTeardown;
