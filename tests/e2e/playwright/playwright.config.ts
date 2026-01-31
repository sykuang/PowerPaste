/**
 * Playwright configuration for WebView content testing.
 *
 * Uses Chrome DevTools Protocol to connect to the Tauri app's WebView.
 * Requires the app to be running with POWERPASTE_DEVTOOLS_PORT set.
 */

import { defineConfig } from '@playwright/test';

// CI-aware configuration (inline to avoid ESM import issues)
const isCI = !!process.env.CI;
const envWorkers = process.env.POWERPASTE_TEST_WORKERS
  ? parseInt(process.env.POWERPASTE_TEST_WORKERS, 10)
  : null;
const envTimeout = process.env.POWERPASTE_TEST_TIMEOUT
  ? parseInt(process.env.POWERPASTE_TEST_TIMEOUT, 10)
  : null;
const envRetries = process.env.POWERPASTE_TEST_RETRIES
  ? parseInt(process.env.POWERPASTE_TEST_RETRIES, 10)
  : null;

const ciConfig = {
  isCI,
  playwrightWorkers: envWorkers ?? (isCI ? 2 : 4),
  testTimeout: envTimeout ?? (isCI ? 90000 : 60000),
  retries: envRetries ?? (isCI ? 3 : 1),
};

export default defineConfig({
  testDir: './specs',
  testMatch: '**/*.spec.ts',

  // Parallel execution
  fullyParallel: true,
  workers: ciConfig.playwrightWorkers,

  // Timeouts and retries
  timeout: ciConfig.testTimeout,
  retries: ciConfig.retries,

  // Reporting
  reporter: ciConfig.isCI
    ? [['github'], ['html', { outputFolder: '../../reports/playwright', open: 'never' }]]
    : [['list']],

  // Global setup/teardown
  globalSetup: './fixtures/global-setup.ts',
  globalTeardown: './fixtures/global-teardown.ts',

  use: {
    // Each test gets its own browser context
    trace: ciConfig.isCI ? 'retain-on-failure' : 'off',
    screenshot: ciConfig.isCI ? 'only-on-failure' : 'off',
    video: ciConfig.isCI ? 'retain-on-failure' : 'off',

    // Base URL not needed since we connect via CDP
    baseURL: undefined,

    // Viewport size matching the overlay dimensions
    viewport: { width: 980, height: 240 },
  },

  // Projects for different scenarios
  projects: [
    {
      name: 'webview',
      use: {
        // Connect to Tauri app via CDP
        // The actual connection is handled in the fixture
      },
    },
  ],

  // Output folder for test artifacts
  outputDir: '../../reports/playwright/test-results',

  // Don't run webServer - we launch the app separately
  webServer: undefined,
});
