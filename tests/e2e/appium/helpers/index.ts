/**
 * Re-export all helpers for convenient imports.
 */

export { config } from './ciConfig.js';
export {
  APP_BUNDLE_ID,
  APP_NAME,
  getAppPath,
  getTestDbPath,
  getMacCapabilities,
  getWindowsCapabilities,
  getCapabilities,
  getAppiumPort,
} from './capabilities.js';
export {
  allocatePort,
  getStaticPort,
  releasePort,
  releaseAllPorts,
  allocateWorkerPorts,
} from './portManager.js';
export {
  selectors,
  getSelector,
  getSelectorForPlatform,
  accessibilityId,
  dynamicSelector,
  macPredicate,
  macClassChain,
} from './selectors.js';
export {
  resetTestDatabase,
  seedTestData,
  cleanupTestDatabase,
  cleanupAllTestDatabases,
  getTestEnvironment,
} from './testDb.js';
