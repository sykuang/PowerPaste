/**
 * CI-aware configuration for E2E tests.
 *
 * Automatically detects CI environment and adjusts parallelization, timeouts,
 * and retry settings. Can be overridden via environment variables for custom
 * CI setups (e.g., self-hosted runners with more resources).
 *
 * Environment Variables:
 * - POWERPASTE_TEST_WORKERS: Override parallel worker count
 * - POWERPASTE_TEST_TIMEOUT: Override test timeout in milliseconds
 * - POWERPASTE_TEST_RETRIES: Override retry count
 */

import ci from 'ci-info';
import os from 'os';

// Parse environment variable as integer, returning null if not set or invalid
function parseEnvInt(name: string): number | null {
  const value = process.env[name];
  if (!value) return null;
  const parsed = parseInt(value, 10);
  return isNaN(parsed) ? null : parsed;
}

// Environment variable overrides (for self-hosted runners, etc.)
const envWorkers = parseEnvInt('POWERPASTE_TEST_WORKERS');
const envTimeout = parseEnvInt('POWERPASTE_TEST_TIMEOUT');
const envRetries = parseEnvInt('POWERPASTE_TEST_RETRIES');

// Auto-detect based on environment
const autoWorkers = ci.isCI
  ? 2 // Conservative for GitHub-hosted runners (3 CPU cores)
  : Math.min(os.cpus().length - 1, 4);

const autoTimeout = ci.isCI ? 90_000 : 60_000;
const autoRetries = ci.isCI ? 3 : 1;

export const config = {
  // CI detection
  isCI: ci.isCI,
  ciName: ci.name ?? 'local',

  // Parallelization - POWERPASTE_TEST_WORKERS overrides auto-detection
  workers: envWorkers ?? autoWorkers,

  // Derived settings for different test runners
  get appiumMaxInstances() {
    return this.workers;
  },
  get playwrightWorkers() {
    return this.workers;
  },

  // Timeouts (longer in CI due to cold starts)
  testTimeout: envTimeout ?? autoTimeout,
  // WebDriverAgentMac can take a while to build/start the first time
  appLaunchTimeout: ci.isCI ? 180_000 : 120_000,

  // Retries (more in CI for flakiness)
  retries: envRetries ?? autoRetries,

  // Reporting - capture artifacts on failure in CI
  video: ci.isCI ? 'retain-on-failure' : ('off' as const),
  screenshot: ci.isCI ? 'only-on-failure' : ('off' as const),
  trace: ci.isCI ? 'retain-on-failure' : ('off' as const),

  // Reporter configuration
  get reporters() {
    return ci.isCI
      ? [['spec', { showPreface: false }], ['html', { outputDir: './reports/appium' }]]
      : [['spec', { showPreface: false }]];
  },

  // Debug info for logging
  debugInfo() {
    return {
      isCI: this.isCI,
      ciName: this.ciName,
      workers: this.workers,
      workerSource: envWorkers ? 'POWERPASTE_TEST_WORKERS' : 'auto-detected',
      timeout: this.testTimeout,
      timeoutSource: envTimeout ? 'POWERPASTE_TEST_TIMEOUT' : 'auto-detected',
      retries: this.retries,
      retriesSource: envRetries ? 'POWERPASTE_TEST_RETRIES' : 'auto-detected',
      cpuCount: os.cpus().length,
      platform: process.platform,
    };
  },
} as const;

// Log config on module load for debugging
console.log('[e2e] Test configuration:', config.debugInfo());
