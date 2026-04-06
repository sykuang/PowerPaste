<p align="center">
  <img src="src/assets/logo.svg" width="128" height="128" alt="PowerPaste logo" />
</p>

<h1 align="center">PowerPaste</h1>

<p align="center">
  Open-source clipboard manager for macOS &amp; Windows — with encrypted cross-device sync, full-text search, and zero cloud accounts required.
</p>

<p align="center">
  Built with <strong>Tauri v2</strong> (Rust) + <strong>React 19</strong> (TypeScript)
</p>

---

## Why PowerPaste?

Most clipboard managers either lack sync, charge a subscription for it, or route your data through someone else's cloud. PowerPaste takes a different approach: **encrypted folder-based sync** that works with any provider you already use (iCloud Drive, OneDrive, Google Drive, or a plain folder). Your data never leaves your machines unencrypted.

## Comparison

| Feature | PowerPaste | Maccy | Paste | CopyClip | Flycut |
|---|---|---|---|---|---|
| **Price** | Free | Free / $10 | $3.99/mo | Free / $10 | Free |
| **Open source** | ✅ Yes | ✅ Yes | ❌ | ❌ | ✅ Yes |
| **Text** | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Images & files** | ✅ | ❌ | ✅ | ❌ | ❌ |
| **Full-text search** | ✅ FTS5 | ✅ | ✅ | ❌ | ❌ |
| **Cross-device sync** | ✅ Encrypted folder | ❌ | ✅ iCloud only | ❌ | ❌ |
| **No cloud account** | ✅ | ✅ | ❌ | ✅ | ✅ |
| **Pinboards** | ✅ Custom w/ icons | ❌ | ✅ | ❌ | ❌ |
| **Source app tracking** | ✅ | ❌ | ✅ | ❌ | ❌ |
| **Windows support** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Themes** | Light / Dark / System | System | Dark | System | System |

## Features

- **Clipboard history** — text, images, and files with automatic content-type detection
- **Full-text search** — SQLite FTS5 with query caching
- **Encrypted sync** — ChaCha20-Poly1305 + Argon2 key derivation; passphrase stored in OS keychain
- **Folder-based sync** — works with iCloud Drive, OneDrive, Google Drive, or any synced folder
- **Pinboards** — organize clips into custom boards with 16 built-in icons
- **Global hotkey** — configurable shortcut (`⌘⇧V` on macOS, `Ctrl+Shift+V` on Windows) with conflict detection
- **Source app tracking** — see which app copied each item
- **Pinned items & trash** — pin important clips; soft-delete with configurable retention
- **Two UI modes** — floating overlay near cursor or fixed bottom strip
- **Native macOS integration** — menu bar app, NSPanel overlay, Touch Bar support
- **Native Windows integration** — frameless floating window, DWM rounded corners, taskbar-hidden popup
- **Light / Dark / System themes** — respects system accent color
- **Launch at startup** — via `tauri-plugin-autostart`

## Platform support

| Platform | Status |
|---|---|
| macOS (Apple Silicon & Intel) | ✅ Fully supported |
| Windows (x64 & ARM64) | ✅ Fully supported |
| Linux | ❌ Not supported |

## Install

Download the latest release from the [Releases](https://github.com/sykuang/PowerPaste/releases) page.

| Platform | Format |
|---|---|
| macOS | `.dmg` |
| Windows | `.msi` or `.exe` |

### macOS Gatekeeper notice

The DMG is not notarized by Apple. macOS will block it by default. To open:

1. **Right-click** the app → **Open** → click **Open** in the dialog
2. Or run: `xattr -cr /Applications/PowerPaste.app`

## Development

### Prerequisites

- Node.js 18+
- Rust toolchain (stable)
- [Tauri prerequisites](https://tauri.app/start/prerequisites/)

### Quick start

```bash
npm install
npm run tauri dev
```

### Build

```bash
npm run tauri build
```

### Recommended IDE

[VS Code](https://code.visualstudio.com/) with [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

### Architecture — platform abstraction

All OS-specific code lives in `src-tauri/src/platform/`:

```
platform/
  mod.rs        # Re-exports from the active platform via #[cfg]
  macos.rs      # macOS: NSPanel, osascript, NSPasteboard
  windows.rs    # Windows: Win32 SendInput, CF_HDROP, DWM, GetForegroundWindow
```

Each file exports the same function signatures (e.g. `perform_paste`, `query_frontmost_app_info`, `check_permissions`). The rest of the codebase calls `platform::*` without any `#[cfg]` blocks.

## Tests

### Unit tests

```bash
npm test            # run once
npm run test:watch  # watch mode
```

Runs UI unit tests (Vitest + JSDOM). These do **not** execute the native Tauri runtime.

### E2E tests

PowerPaste uses a dual-layer E2E strategy:

- **Appium + WebdriverIO** — native UI testing (window behavior, hotkeys, permissions)
- **Playwright** — WebView content testing (React components, interactions)

#### Setup

```bash
# Build the app
npm run tauri build -- --debug

# macOS Appium driver
npm install -g appium && appium driver install mac2

# Windows Appium driver
npm install -g appium && appium driver install windows && appium driver run windows install-wad

# Playwright
npx playwright install chromium
```

#### Run

```bash
npm run test:e2e                   # all E2E tests
npm run test:e2e:mac               # macOS native
npm run test:e2e:windows           # Windows native
npm run test:playwright            # WebView tests
npm run test:e2e:mac:permissions   # macOS permission dialog flow
```

#### Environment variables

| Variable | Description | Default |
|---|---|---|
| `POWERPASTE_TEST_WORKERS` | Parallel worker count | 2 in CI, CPU−1 locally |
| `POWERPASTE_TEST_TIMEOUT` | Test timeout (ms) | 90 s in CI, 60 s locally |
| `POWERPASTE_TEST_RETRIES` | Retry count | 3 in CI, 1 locally |
| `POWERPASTE_TEST_DB_PATH` | Isolated test DB path | Auto per worker |
| `POWERPASTE_DEVTOOLS_PORT` | Playwright WebView port | Disabled |

## Sync model

1. Local clipboard history is stored in **SQLite** (WAL mode) under the app data directory.
2. When folder sync is enabled, PowerPaste periodically exports an **encrypted** `powerpaste.sync.json` into the chosen folder.
3. On other devices the file is imported and merged.
4. Encryption uses **ChaCha20-Poly1305** with an **Argon2**-derived key; the passphrase lives in the OS keychain (macOS Keychain / Windows Credential Manager).

## Logo & branding

See [docs/LOGO.md](docs/LOGO.md) for logo usage and icon regeneration.

## License

MIT
