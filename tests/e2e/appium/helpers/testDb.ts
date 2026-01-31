/**
 * Test database utilities for E2E test isolation.
 *
 * Provides functions to create, seed, and cleanup isolated test databases
 * so each test file or worker has its own clean state.
 */

import * as fs from 'fs';
import * as path from 'path';
import { fileURLToPath } from 'url';
import { getTestDbPath } from './capabilities.js';
import { config as ciConfig } from './ciConfig.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const FIXTURES_DIR = path.resolve(__dirname, '../../fixtures');
const EMPTY_DB_TEMPLATE = path.join(FIXTURES_DIR, 'test.db');

/**
 * Reset the test database for a worker.
 * Creates an empty database file or copies from template.
 */
export function resetTestDatabase(workerId: number = 0): string {
  const dbPath = getTestDbPath(workerId);

  // Remove existing database and related files
  const filesToRemove = [dbPath, `${dbPath}-shm`, `${dbPath}-wal`];
  for (const file of filesToRemove) {
    try {
      if (fs.existsSync(file)) {
        fs.unlinkSync(file);
      }
    } catch (e) {
      console.warn(`[testDb] Failed to remove ${file}:`, e);
    }
  }

  // Ensure parent directory exists
  const dir = path.dirname(dbPath);
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }

  // Copy template if exists, otherwise create empty file
  if (fs.existsSync(EMPTY_DB_TEMPLATE)) {
    fs.copyFileSync(EMPTY_DB_TEMPLATE, dbPath);
    log(`Reset database from template: ${dbPath}`);
  } else {
    // Create empty file - the app will initialize the schema
    fs.writeFileSync(dbPath, '');
    log(`Created empty database: ${dbPath}`);
  }

  return dbPath;
}

/**
 * Seed the test database with sample clipboard items.
 * This executes sqlite3 commands to insert test data.
 */
export async function seedTestData(workerId: number = 0): Promise<void> {
  const dbPath = getTestDbPath(workerId);

  // Note: The app will create the schema on first run.
  // For seeding, we'd need to either:
  // 1. Launch the app first to create schema
  // 2. Use rusqlite or better-sqlite3 to insert data
  // 3. Pre-create a seeded template database

  // For now, log a placeholder - actual seeding happens when app launches
  log(`Database ready for seeding: ${dbPath}`);
}

/**
 * Cleanup the test database after tests complete.
 */
export function cleanupTestDatabase(workerId: number = 0): void {
  const dbPath = getTestDbPath(workerId);

  const filesToRemove = [dbPath, `${dbPath}-shm`, `${dbPath}-wal`];
  for (const file of filesToRemove) {
    try {
      if (fs.existsSync(file)) {
        fs.unlinkSync(file);
        log(`Removed: ${file}`);
      }
    } catch (e) {
      console.warn(`[testDb] Failed to remove ${file}:`, e);
    }
  }
}

/**
 * Cleanup all test databases (for all workers).
 */
export function cleanupAllTestDatabases(maxWorkers: number = 10): void {
  for (let i = 0; i < maxWorkers; i++) {
    cleanupTestDatabase(i);
  }
  log(`Cleaned up databases for ${maxWorkers} workers`);
}

/**
 * Get the environment variables needed for test isolation.
 */
export function getTestEnvironment(workerId: number = 0): Record<string, string> {
  return {
    POWERPASTE_TEST_DB_PATH: getTestDbPath(workerId),
    POWERPASTE_DEVTOOLS_PORT: String(9222 + workerId),
  };
}

function log(message: string): void {
  if (ciConfig.isCI) {
    console.log(`[testDb] ${message}`);
  }
}
