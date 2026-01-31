/**
 * Global setup for Playwright tests.
 *
 * Runs before all tests to prepare the test environment.
 */

import * as fs from 'fs';
import * as path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

async function globalSetup(): Promise<void> {
  console.log('[playwright:setup] Starting global setup');

  // Create reports directory
  const reportsDir = path.resolve(__dirname, '../../../reports/playwright');
  if (!fs.existsSync(reportsDir)) {
    fs.mkdirSync(reportsDir, { recursive: true });
    console.log(`[playwright:setup] Created reports directory: ${reportsDir}`);
  }

  // Verify the app binary exists
  const projectRoot = path.resolve(__dirname, '../../../../');
  const platform = process.platform;

  let appPath: string;
  if (platform === 'darwin') {
    appPath = path.join(
      projectRoot,
      'src-tauri',
      'target',
      'release',
      'bundle',
      'macos',
      'PowerPaste.app'
    );
    const debugPath = path.join(projectRoot, 'src-tauri', 'target', 'debug', 'powerpaste');

    if (!fs.existsSync(appPath) && !fs.existsSync(debugPath)) {
      console.warn(
        '[playwright:setup] Warning: App binary not found. Run `npm run tauri build` first.'
      );
    }
  } else if (platform === 'win32') {
    appPath = path.join(projectRoot, 'src-tauri', 'target', 'release', 'PowerPaste.exe');
    const debugPath = path.join(projectRoot, 'src-tauri', 'target', 'debug', 'PowerPaste.exe');

    if (!fs.existsSync(appPath) && !fs.existsSync(debugPath)) {
      console.warn(
        '[playwright:setup] Warning: App binary not found. Run `npm run tauri build` first.'
      );
    }
  }

  console.log('[playwright:setup] Global setup complete');
}

export default globalSetup;
