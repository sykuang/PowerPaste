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

- `npm test` / `npm run test:run` runs UI unit tests (Vitest + JSDOM). These do **not** execute the native Tauri runtime, so they can't catch platform-specific window/tray behavior (e.g. show/hide lifecycle on macOS).
- To cover native behavior, add an end-to-end/integration test layer that launches the built app and drives it (for example via a WebDriver-based approach or Playwright + a Tauri runner).

## Sync model (MVP)

- Local clipboard history is stored in SQLite under the app data directory.
- If folder sync is enabled, PowerPaste periodically imports then exports an encrypted sync file.
- The passphrase is stored in the OS keychain (macOS Keychain / Windows Credential Manager).
