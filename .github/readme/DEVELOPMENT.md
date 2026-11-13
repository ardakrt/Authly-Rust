# Development and packaging

[← Back to Authly](../../README.md)

## Requirements

- Node.js 20.19 or newer (`>=20.19.0`).
- npm 11 or newer; the repository declares its package-manager version in `package.json`.
- Rust stable toolchain (`rustc` and `cargo`, e.g. via `rustup`).
- Windows 10/11 x64 (with Microsoft Edge WebView2), or a modern x64 Linux distribution.
- On Linux:
  - Runtime: an installed and unlocked Secret Service keyring (`GNOME Keyring` or `KWallet`).
  - Build dependencies (Debian / Ubuntu):
    ```bash
    sudo apt-get update && sudo apt-get install -y \
      libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libdbus-1-dev pkg-config
    ```

## Start locally

```bash
git clone https://github.com/ardakrt/Authly-Rust.git
cd Authly-Rust
npm ci
npm run dev
```

`npm run dev` invokes `tauri dev`, which starts the Vite renderer dev server (`npm run dev:web` on port `5173`) and compiles/launches the Rust Tauri v2 desktop application together. (`npm run dev:web` alone starts the browser preview mode.)

## Verification

```bash
npm run verify
npm run smoke
```

`npm run verify` runs formatting checks, ESLint, strict TypeScript checks, Vitest web tests, Rust Cargo tests, and the Vite renderer build (`dist-renderer/`). Individual commands:

```bash
npm run format:check
npm run lint
npm run typecheck
npm run test:web
npm run test:rust
npm run test
npm run build
npm run smoke
```

- `npm run test` runs both `npm run test:web` (Vitest) and `npm run test:rust` (`cargo test --manifest-path src-tauri/Cargo.toml`).
- `npm run smoke` runs `scripts/smoke.mjs`, ensuring `dist-renderer/index.html` exists and is non-empty (running `npm run build:web` if missing) and executing `cargo test --manifest-path src-tauri/Cargo.toml --quiet` with `AUTHAPP_SMOKE_TEST=1` (printing `AUTHAPP_SMOKE_READY` on success).
- Rust tests exercise SQLite persistence, AES-256-GCM vault encryption, legacy Electron `v10` migration, TOTP RFC 6238 generation, PBKDF2 PIN locking/epoch invalidation, encrypted backup round-trips, and semantic version update comparison using isolated in-memory storage (`InMemoryAesGcmVault` and `LocalDatabase::open_in_memory()`). They never access the system clipboard, keyring, or user vault.

### Account input and import boundaries

- Creation, secret updates, `otpauth://` parsing, and backup payloads share canonical Base32 validation (`normalize_base32_secret`) and character-count bounds (`issuer` <= 120 chars, `account_name` <= 240 chars). Spaces, hyphens, case, and valid padding are normalized; invalid characters, padding, and trailing bits are rejected before storage.
- A legacy or unreadable account remains visible in account listings with an unavailable-code state. Its failure does not prevent healthy accounts from generating or copying codes.
- Google Authenticator migration QR input (`src/renderer/src/utils/otpMigrationParser.ts`) is bounded to 128 KiB and 100 accounts (`MAX_ACCOUNT_IMPORT_SIZE`). Truncated protobuf data, overflowing lengths, unknown algorithms/digit counts, and non-TOTP accounts are rejected before the confirmation preview.
- `import_accounts` validates Zod schemas on the renderer bridge (`src/renderer/src/bridge/tauriBridge.ts`), deserializes strict structs (`#[serde(deny_unknown_fields)]`) in Rust, captures a `LockService` session epoch, encrypts all selected accounts with `VaultService`, rechecks the session epoch, and inserts the batch inside one SQLite transaction (`create_many`).

### Lock lifecycle, backup recovery, and updates

- **PIN Lock:** `LockService` (`src-tauri/src/lock_service.rs`) owns a five-minute inactivity deadline (`autoLockTimeout = 5` by default), failed-attempt rate limiting (30s base backoff doubling after every 5 failures up to 15m), and a session `epoch`. A 1-second background thread in `src-tauri/src/lib.rs` checks auto-lock (incorporating OS idle time via `GetLastInputInfo` on Windows) and emits `lock:status-changed`. Sensitive commands (`list_accounts`, writes, TOTP generation, clipboard copy, backups, `update_settings`, `install_update`) enforce lock checks and epoch verification; `get_settings` and `get_lock_status` remain readable while locked. Users can set, change, remove, or immediately trigger the Master PIN from Settings (`SettingsPage`).
- **Backups:** Backup payload version 2 includes groups and restores all groups and accounts in one SQLite transaction (`restore_groups_and_accounts`) after encryption/session checks succeed. Version 1 remains supported; missing legacy group metadata produces an explicit ungrouped-account notice. Export files use format `authapp-backup-v1` with PBKDF2-SHA256 (600,000 iterations) and AES-256-GCM.
- **Updates:** In packaged builds (`!cfg!(debug_assertions)`), `check_update` queries `https://api.github.com/repos/ardakrt/Authly-Rust/releases/latest` via `reqwest` (`src-tauri/src/update_service.rs`). Triggering `install_update` checks the lock guard, confirms a newer release is available, verifies the release URL via `is_trusted_release_url` (`https://github.com/ardakrt/Authly-Rust/releases`), and opens it in the user's external default browser (`rundll32 url.dll,FileProtocolHandler` on Windows, `xdg-open` on Linux, `open` on macOS).

## Packaging

Run `npm run verify` (and `npm run smoke`) before building release artifacts.

Windows NSIS installer:

```bash
npm run tauri:build
```

Outputs are written to `src-tauri/target/release/bundle/nsis/*.exe`.

Linux Debian package and AppImage (run on a Linux build host with Tauri system dependencies installed):

```bash
npx tauri build --bundles deb,appimage
```

Outputs are written to `src-tauri/target/release/bundle/deb/*.deb` and `src-tauri/target/release/bundle/appimage/*.AppImage`. Published binaries are listed on [GitHub Releases](https://github.com/ardakrt/Authly-Rust/releases).

If an AppImage or Debian package is available:

```bash
chmod +x Authly*.AppImage
./Authly*.AppImage

# Debian / Ubuntu
sudo apt install ./Authly*.deb
```

## Source layout

```text
src-tauri/              Tauri v2 Rust backend, SQLite database, OS keyring vault, TOTP, backup, lock & update services
src/renderer/           React interface, Tauri bridge, themes, and translations
src/shared/             Shared TypeScript types, Zod schemas, and IPC channel/event names
tests/                  Automated Vitest unit and renderer tests
scripts/                Runtime smoke checks
resources/              Application and tray icon sources
.github/readme/         Documentation assets and screenshot capture
```

## Refresh the README screenshots

Install development dependencies and make Playwright and Chrome available. The capture script starts a local renderer-only Vite server with `@shared` and `@renderer` aliases, injects synthetic `window.authapp` responses (reporting version `0.3.12`), and saves PNGs in `.github/readme/images/`:

```bash
node .github/readme/capture.mjs
```

If Playwright is installed outside this project, set `PLAYWRIGHT_MODULE` to its module path before running the script. Real account databases, master PINs, backup files, and TOTP secrets are never needed for this capture. External renderer requests are blocked. Screenshots demonstrate UI states; they do not test the Tauri/Rust encryption or backup implementation.

## Working with local data

The application keeps runtime data in Tauri's app-data directory for identifier `dev.authapp.desktop`:

- **Windows:** `%APPDATA%\dev.authapp.desktop` (`authapp.db`)
- **Linux:** `~/.local/share/dev.authapp.desktop` (`authapp.db`)

The vault master key is stored in the OS keyring under service `dev.authapp.desktop` and user `vault-master-key` via `KeyringAesGcmVault`. On Windows, when the target database has zero accounts and `%APPDATA%\authapp` contains legacy Electron `authapp.db` and `Local State`, Authly automatically performs a one-time migration from the legacy DPAPI-protected vault.

Use synthetic accounts when developing or taking screenshots. Keep local databases, backups, PIN hashes, and account secrets out of the repository.
