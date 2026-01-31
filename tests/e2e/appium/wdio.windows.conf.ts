/**
 * WebdriverIO configuration for Windows E2E tests with Appium Windows driver.
 *
 * Run with: npm run test:e2e:windows
 */

import type { Options } from '@wdio/types';
import path from 'path';
import { fileURLToPath } from 'url';
import { config as ciConfig, getWindowsCapabilities, getAppiumPort, resetTestDatabase } from './helpers/index.js';

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
  specs: [
    path.resolve(__dirname, './specs/shared/**/*.spec.ts'),
    path.resolve(__dirname, './specs/windows/**/*.spec.ts'),
  ],
  exclude: [],

  //
  // ============
  // Capabilities
  // ============
  maxInstances: ciConfig.appiumMaxInstances,
  capabilities: [getWindowsCapabilities(workerId)],

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
    console.log('[wdio:windows] Starting Windows E2E tests');
    console.log('[wdio:windows] Config:', ciConfig.debugInfo());
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
        console.log(`[wdio:windows] Screenshot saved: ${screenshotPath}`);
      } catch (e) {
        console.warn('[wdio:windows] Failed to capture screenshot:', e);
      }
    }
  },

  /**
   * Gets executed after all tests are done.
   */
  onComplete: function () {
    console.log('[wdio:windows] All Windows E2E tests completed');
  },
};
