/**
 * Platform-specific Appium capabilities for PowerPaste.
 *
 * Provides factory functions to generate capabilities for macOS (Mac2 driver)
 * and Windows (Windows driver) with proper port allocation and test isolation.
 */

import * as path from 'path';
import { fileURLToPath } from 'url';
import { config as ciConfig } from './ciConfig.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// App identifiers from tauri.conf.json
export const APP_BUNDLE_ID = 'com.kenkuang.powerpaste';
export const APP_NAME = 'PowerPaste';

// Base ports for parallel workers
const APPIUM_BASE_PORT = 4723;
const MAC_SYSTEM_BASE_PORT = 10100;
const MAC_WDA_BASE_PORT = 8100;
const WINDOWS_SYSTEM_BASE_PORT = 4724;

/**
 * Get the built app path based on platform and build type.
 */
export function getAppPath(
  platform: 'mac' | 'windows',
  buildType: 'debug' | 'release' = 'release'
): string {
  const projectRoot = path.resolve(__dirname, '../../../../');
  const target = buildType === 'debug' ? 'debug' : 'release';

  if (platform === 'mac') {
    return path.join(
      projectRoot,
      'src-tauri',
      'target',
      target,
      'bundle',
      'macos',
      `${APP_NAME}.app`
    );
  } else {
    return path.join(
      projectRoot,
      'src-tauri',
      'target',
      target,
      `${APP_NAME}.exe`
    );
  }
}

/**
 * Get test database path for worker isolation.
 */
export function getTestDbPath(workerId: number): string {
  const tempDir = process.platform === 'darwin' ? '/tmp' : process.env.TEMP || '/tmp';
  return path.join(tempDir, `powerpaste-test-${workerId}.db`);
}

/**
 * Check if WebDriverAgentMac is pre-started and available.
 */
function isWdaPreStarted(port: number): boolean {
  // Check environment variable to see if we should use pre-started WDA
  return process.env.POWERPASTE_WDA_PRESTARTED === 'true';
}

/**
 * Generate macOS capabilities for Mac2 driver.
 */
export function getMacCapabilities(workerId: number = 0) {
  const testDbPath = getTestDbPath(workerId);
  const systemPort = MAC_SYSTEM_BASE_PORT + workerId;
  const usePreStartedWda = isWdaPreStarted(systemPort);

  const caps: Record<string, unknown> = {
    platformName: 'mac',
    'appium:automationName': 'mac2',
    'appium:bundleId': APP_BUNDLE_ID,
    'appium:appPath': getAppPath('mac'),

    // Port allocation for parallel workers
    'appium:systemPort': systemPort,
    'appium:wdaLocalPort': MAC_WDA_BASE_PORT + workerId,

    // Timeouts - prevent session from being killed during tests
    'appium:serverStartupTimeout': ciConfig.appLaunchTimeout,
    'appium:commandTimeout': ciConfig.testTimeout,
    'appium:newCommandTimeout': 300, // 5 minutes to allow for long test runs

    // Environment variables for test isolation
    'appium:environment': {
      POWERPASTE_TEST_DB_PATH: testDbPath,
      POWERPASTE_DEVTOOLS_PORT: '9222',
    },

    // Logging - enable in CI for debugging
    'appium:showServerLogs': ciConfig.isCI,

    // Don't reset app state between tests in the same file
    'appium:noReset': true,
    'appium:skipAppKill': false,
  };

  // If WDA is pre-started, connect to it instead of starting new
  if (usePreStartedWda) {
    caps['appium:webDriverAgentMacUrl'] = `http://127.0.0.1:${systemPort}`;
    console.log(`[capabilities] Using pre-started WDA at http://127.0.0.1:${systemPort}`);
  }

  return caps;
}

/**
 * Generate Windows capabilities for Windows driver.
 */
export function getWindowsCapabilities(workerId: number = 0) {
  const testDbPath = getTestDbPath(workerId);

  return {
    platformName: 'windows',
    'appium:automationName': 'windows',
    'appium:app': getAppPath('windows'),

    // Port allocation for parallel workers
    'appium:systemPort': WINDOWS_SYSTEM_BASE_PORT + workerId,

    // Timeouts
    'appium:createSessionTimeout': ciConfig.appLaunchTimeout,
    'ms:waitForAppLaunch': Math.floor(ciConfig.appLaunchTimeout / 1000),

    // Environment variables for test isolation
    'appium:appArguments': '',
    'appium:appWorkingDir': path.dirname(getAppPath('windows')),

    // Note: Windows driver doesn't support environment injection directly.
    // We'll need to set these before launching the test runner.
  };
}

/**
 * Get platform-appropriate capabilities.
 */
export function getCapabilities(workerId: number = 0) {
  return process.platform === 'darwin'
    ? getMacCapabilities(workerId)
    : getWindowsCapabilities(workerId);
}

/**
 * Get Appium server port for a worker.
 */
export function getAppiumPort(workerId: number = 0): number {
  return APPIUM_BASE_PORT + workerId;
}
