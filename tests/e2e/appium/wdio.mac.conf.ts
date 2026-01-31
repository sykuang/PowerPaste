/**
 * WebdriverIO configuration for macOS E2E tests with Appium Mac2 driver.
 *
 * Run with: npm run test:e2e:mac
 */

import type { Options } from '@wdio/types';
import path from 'path';
import { fileURLToPath } from 'url';
import { config as ciConfig, getMacCapabilities, getAppiumPort, resetTestDatabase } from './helpers/index.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Worker ID from WDIO (0-indexed)
const workerId = parseInt(process.env.WDIO_WORKER_ID || '0', 10);

export const config: Options.Testrunner = {
  //
  // ====================
  // Runner Configuration
  // ====================
  runner: 'local',
  autoCompileOpts: {
    autoCompile: true,
    tsNodeOpts: {
      project: path.resolve(__dirname, '../tsconfig.json'),
      transpileOnly: true,
    },
  },

  //
  // ==================
  // Specify Test Files
  // ==================
  // Note: PowerPaste on macOS is a menu bar app with NSPanel overlay.
  // Standard window queries (XCUIElementTypeWindow) don't work.
  // We only run Mac-specific tests that use AppleScript for interaction.
  specs: [
    path.resolve(__dirname, './specs/mac/**/*.spec.ts'),
  ],
  exclude: [
    // Exclude permission dialog test from normal runs (needs special setup)
    path.resolve(__dirname, './specs/mac/permissions-dialog.spec.ts'),
  ],

  //
  // ============
  // Capabilities
  // ============
  maxInstances: ciConfig.appiumMaxInstances,
  capabilities: [getMacCapabilities(workerId)],

  //
  // ===================
  // Test Configurations
  // ===================
  logLevel: ciConfig.isCI ? 'warn' : 'info',
  bail: 0,
  baseUrl: '',
  waitforTimeout: ciConfig.testTimeout,
  connectionRetryTimeout: 120000,
  connectionRetryCount: 3,

  //
  // ========
  // Services
  // ========
  // Note: Due to Node.js 24 compatibility issues with appium dependencies,
  // start Appium manually before running tests: appium --relaxed-security
  // Or use Node.js 22 LTS which has better compatibility.
  services: [],
  port: getAppiumPort(workerId),

  //
  // =========
  // Framework
  // =========
  framework: 'mocha',
  mochaOpts: {
    ui: 'bdd',
    timeout: ciConfig.testTimeout,
    retries: ciConfig.retries,
  },

  //
  // =========
  // Reporters
  // =========
  reporters: ['spec'] as const,

  //
  // =====
  // Hooks
  // =====

  /**
   * Gets executed before a worker process is spawned.
   */
  onPrepare: function () {
    console.log('[wdio:mac] Starting macOS E2E tests');
    console.log('[wdio:mac] Config:', ciConfig.debugInfo());
  },

  /**
   * Gets executed before test execution begins.
   */
  before: async function () {
    // Reset test database for this worker
    resetTestDatabase(workerId);
  },

  /**
   * Function to be executed after a test.
   */
  afterTest: async function (test, _context, { passed }) {
    // Capture screenshot on failure in CI
    if (!passed && ciConfig.isCI) {
      const screenshotPath = `./reports/screenshots/${test.title.replace(/\s+/g, '_')}.png`;
      try {
        await browser.saveScreenshot(screenshotPath);
        console.log(`[wdio:mac] Screenshot saved: ${screenshotPath}`);
      } catch (e) {
        console.warn('[wdio:mac] Failed to capture screenshot:', e);
      }
    }
  },

  /**
   * Gets executed after all tests are done.
   */
  onComplete: function () {
    console.log('[wdio:mac] All macOS E2E tests completed');
  },
};
