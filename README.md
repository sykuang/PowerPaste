# PowerPaste

Cross-platform clipboard history (macOS/Windows) with optional encrypted sync via a local folder.

Folder sync is designed to work with providers that sync a folder to your machine (iCloud Drive, OneDrive, Google Drive). You pick the synced folder on each device, and PowerPaste writes an encrypted `powerpaste.sync.json` file into it.

Built with Tauri v2 (Rust backend) + React/TypeScript (UI).

## Prerequisites

- Node.js 18+ (or newer)
- Rust toolchain (stable) + `cargo`
- Tauri system prerequisites: https://tauri.app/start/prerequisites/

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Run

```bash
cd powerpaste
npm install
npm run tauri dev
```

## Tests

### Unit Tests

```bash
npm test          # Run once
npm run test:watch  # Watch mode
```

Runs UI unit tests (Vitest + JSDOM). These do **not** execute the native Tauri runtime.

### E2E Tests

PowerPaste uses a dual-layer E2E testing strategy:

- **Appium + WebdriverIO**: Native UI testing (window behavior, hotkeys, permissions)
- **Playwright**: WebView content testing (React components, interactions)

#### Prerequisites

1. Build the app first:
   ```bash
   npm run tauri build -- --debug
   ```

2. Install Appium drivers:
   ```bash
   # macOS
   npm install -g appium
   appium driver install mac2
   
   # Windows
   npm install -g appium
   appium driver install windows
   appium driver run windows install-wad
   ```

3. Install Playwright browsers:
   ```bash
   npx playwright install chromium
   ```

#### Running E2E Tests

```bash
# Run all E2E tests for current platform
npm run test:e2e

# Appium native tests only
npm run test:e2e:mac        # macOS
npm run test:e2e:windows    # Windows

# Playwright WebView tests only
npm run test:playwright

# macOS permission dialog flow (requires reset permissions)
npm run test:e2e:mac:permissions
```

#### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `POWERPASTE_TEST_WORKERS` | Override parallel worker count | Auto (2 in CI, CPU-1 locally) |
| `POWERPASTE_TEST_TIMEOUT` | Override test timeout (ms) | Auto (90s in CI, 60s locally) |
| `POWERPASTE_TEST_RETRIES` | Override retry count | Auto (3 in CI, 1 locally) |
| `POWERPASTE_TEST_DB_PATH` | Isolated test database path | Auto per worker |
| `POWERPASTE_DEVTOOLS_PORT` | Enable Playwright WebView access | Disabled |

#### Test Structure

```
tests/e2e/
├── appium/                    # Native UI tests
│   ├── wdio.mac.conf.ts       # macOS config
│   ├── wdio.windows.conf.ts   # Windows config
│   ├── helpers/               # Shared utilities
│   └── specs/
│       ├── mac/               # macOS-specific tests
│       ├── windows/           # Windows-specific tests
│       └── shared/            # Cross-platform tests
├── playwright/                # WebView tests
│   ├── playwright.config.ts
│   ├── fixtures/
│   └── specs/
└── fixtures/                  # Test data
```

## Sync model (MVP)

- Local clipboard history is stored in SQLite under the app data directory.
- If folder sync is enabled, PowerPaste periodically imports then exports an encrypted sync file.
- The passphrase is stored in the OS keychain (macOS Keychain / Windows Credential Manager).
