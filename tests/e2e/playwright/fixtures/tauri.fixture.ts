/**
 * Tauri app fixture for Playwright tests.
 *
 * Launches the Tauri app with DevTools enabled and provides a Page
 * connected to the WebView via Chrome DevTools Protocol.
 */

import { test as base, chromium, type BrowserContext, type Page } from '@playwright/test';
import { spawn, type ChildProcess } from 'child_process';
import * as path from 'path';
import * as fs from 'fs';
import { fileURLToPath } from 'url';
import { getPort } from 'get-port-please';
import kill from 'tree-kill';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Type for the Tauri fixture
export type TauriFixture = {
  tauriPage: Page;
  tauriContext: BrowserContext;
};

// Store app process for cleanup
let appProcess: ChildProcess | null = null;
let devtoolsPort: number | null = null;

/**
 * Get the path to the built Tauri app.
 */
function getAppPath(): string {
  const projectRoot = path.resolve(__dirname, '../../../../');
  const platform = process.platform;

  if (platform === 'darwin') {
    // macOS: Use the .app bundle for release, or binary for debug
    const releasePath = path.join(
      projectRoot,
      'src-tauri',
      'target',
      'release',
      'bundle',
      'macos',
      'PowerPaste.app',
      'Contents',
      'MacOS',
      'PowerPaste'
    );
    const debugPath = path.join(projectRoot, 'src-tauri', 'target', 'debug', 'powerpaste');

    if (fs.existsSync(releasePath)) {
      return releasePath;
    } else if (fs.existsSync(debugPath)) {
      return debugPath;
    }
    // Fallback to debug binary name
    return debugPath;
  } else if (platform === 'win32') {
    const releasePath = path.join(projectRoot, 'src-tauri', 'target', 'release', 'PowerPaste.exe');
    const debugPath = path.join(projectRoot, 'src-tauri', 'target', 'debug', 'PowerPaste.exe');

    if (fs.existsSync(releasePath)) {
      return releasePath;
    }
    return debugPath;
  } else {
    // Linux
    const releasePath = path.join(projectRoot, 'src-tauri', 'target', 'release', 'powerpaste');
    const debugPath = path.join(projectRoot, 'src-tauri', 'target', 'debug', 'powerpaste');

    if (fs.existsSync(releasePath)) {
      return releasePath;
    }
    return debugPath;
  }
}

/**
 * Get test database path for this worker.
 */
function getTestDbPath(workerId: number): string {
  const tempDir = process.platform === 'darwin' ? '/tmp' : process.env.TEMP || '/tmp';
  return path.join(tempDir, `powerpaste-playwright-${workerId}.db`);
}

/**
 * Launch the Tauri app with DevTools enabled.
 */
async function launchApp(workerId: number): Promise<{ port: number; process: ChildProcess }> {
  const port = await getPort({ port: 9222 + workerId, portRange: [9222, 9322] });
  const appPath = getAppPath();
  const testDbPath = getTestDbPath(workerId);

  // Clean up old test database
  const dbFiles = [testDbPath, `${testDbPath}-shm`, `${testDbPath}-wal`];
  for (const file of dbFiles) {
    try {
      if (fs.existsSync(file)) {
        fs.unlinkSync(file);
      }
    } catch {
      // Ignore cleanup errors
    }
  }

  console.log(`[playwright] Launching app: ${appPath}`);
  console.log(`[playwright] DevTools port: ${port}`);
  console.log(`[playwright] Test DB: ${testDbPath}`);

  const env = {
    ...process.env,
    POWERPASTE_TEST_DB_PATH: testDbPath,
    POWERPASTE_DEVTOOLS_PORT: String(port),
  };

  const proc = spawn(appPath, [], {
    env,
    stdio: ['ignore', 'pipe', 'pipe'],
    detached: false,
  });

  // Log app output for debugging
  proc.stdout?.on('data', (data) => {
    console.log(`[app:stdout] ${data.toString().trim()}`);
  });
  proc.stderr?.on('data', (data) => {
    console.log(`[app:stderr] ${data.toString().trim()}`);
  });

  proc.on('error', (error) => {
    console.error(`[playwright] App launch error:`, error);
  });

  proc.on('exit', (code) => {
    console.log(`[playwright] App exited with code: ${code}`);
  });

  // Wait for app to start
  await new Promise((resolve) => setTimeout(resolve, 3000));

  return { port, process: proc };
}

/**
 * Connect to the app's WebView via CDP.
 */
async function connectToWebView(port: number): Promise<{ context: BrowserContext; page: Page }> {
  const cdpUrl = `http://localhost:${port}`;

  console.log(`[playwright] Connecting to CDP: ${cdpUrl}`);

  // Try to connect with retries
  let browser;
  let retries = 5;
  while (retries > 0) {
    try {
      browser = await chromium.connectOverCDP(cdpUrl);
      break;
    } catch (e) {
      console.log(`[playwright] CDP connection failed, retrying... (${retries} left)`);
      retries--;
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
  }

  if (!browser) {
    throw new Error(`Failed to connect to CDP at ${cdpUrl}`);
  }

  // Get the default context (the Tauri WebView)
  const contexts = browser.contexts();
  const context = contexts[0] || (await browser.newContext());

  // Get the page
  const pages = context.pages();
  const page = pages[0] || (await context.newPage());

  console.log(`[playwright] Connected to WebView`);

  return { context, page };
}

/**
 * Extended test with Tauri fixtures.
 */
export const test = base.extend<TauriFixture>({
  tauriContext: async ({}, use, testInfo) => {
    const workerId = testInfo.parallelIndex;

    // Launch the app
    const { port, process } = await launchApp(workerId);
    appProcess = process;
    devtoolsPort = port;

    // Connect to WebView
    const { context } = await connectToWebView(port);

    // Use the context in tests
    await use(context);

    // Cleanup: close browser connection (app stays running for next test in file)
    await context.close();
  },

  tauriPage: async ({ tauriContext }, use) => {
    const pages = tauriContext.pages();
    const page = pages[0] || (await tauriContext.newPage());

    await use(page);
  },
});

export { expect } from '@playwright/test';

/**
 * Cleanup function for global teardown.
 */
export async function cleanupApp(): Promise<void> {
  if (appProcess && appProcess.pid) {
    console.log(`[playwright] Killing app process: ${appProcess.pid}`);
    await new Promise<void>((resolve) => {
      kill(appProcess!.pid!, 'SIGTERM', (err) => {
        if (err) {
          console.warn(`[playwright] Kill error:`, err);
        }
        resolve();
      });
    });
    appProcess = null;
  }
}
