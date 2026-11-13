<p align="center"><a href="README.tr.md" lang="tr">Türkçe</a> · <strong>English</strong></p>

<p align="center">
  <img src=".github/readme/images/cover.png" alt="Authly — desktop verification codes and PIN lock" width="100%">
</p>

<div align="center">
  <h1>Authly</h1>
  <p><strong>Your codes. Your device.</strong></p>
  <p>An offline desktop authenticator for Windows and Linux.<br>Generate verification codes locally, without a cloud account.</p>
  <p><code>Windows</code> &nbsp; <code>Linux</code> &nbsp; <code>TOTP</code> &nbsp; <code>Tauri v2</code> &nbsp; <code>MIT</code></p>
  <p>
    <a href="https://github.com/ardakrt/Authly-Rust/releases/latest">Download</a> ·
    <a href="#a-closer-look">Screenshots</a> ·
    <a href="#built-for-everyday-sign-ins">Features</a> ·
    <a href=".github/readme/DEVELOPMENT.md">Build from source</a>
  </p>
</div>

---

## Keep your sign-ins close

Authly brings your time-based verification codes into one desktop app built with **Tauri v2 (Rust backend + React UI)**. Add an account, find its code, and copy it when you need it. TOTP generation runs on your device; account secrets are stored locally using your operating system's keyring and AES-256-GCM encryption.

1. **Add your account.** Enter a secret, paste an `otpauth://` URI, or import a QR image.
2. **Get your code.** See the current code and its countdown, then copy it in one click.
3. **Keep a recovery copy.** Export a password-encrypted backup before you need to move devices.

## A closer look

### Clear codes. Light or dark.

A focused account grid, a live countdown, and a compact navigation bar. Switch between dark, light, and system appearance; use the interface in English or Turkish.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset=".github/readme/images/accounts-dark.png">
  <source media="(prefers-color-scheme: light)" srcset=".github/readme/images/accounts-light.png">
  <img src=".github/readme/images/accounts-dark.png" alt="Authly account grid with example verification codes and countdown bars" width="100%">
</picture>

<p align="center"><sub><a href=".github/readme/images/accounts-dark.png">Dark appearance</a> · <a href=".github/readme/images/accounts-light.png">Light appearance</a></sub></p>

### Add an account. Lock your workspace.

Manual entry, authenticator links, and QR image import share the same account workflow. A master PIN adds an application access lock.

<p align="center">
  <a href=".github/readme/images/add-account.png"><img src=".github/readme/images/add-account.png" alt="Authly account setup with manual entry and QR import" width="49%"></a>
  <a href=".github/readme/images/pin-lock.png"><img src=".github/readme/images/pin-lock.png" alt="Authly PIN lock screen" width="49%"></a>
</p>

<sub>These captures use the current renderer with synthetic accounts and fixed example codes. They contain no real secrets or usable authentication codes and are not a security audit.</sub>

## Built for everyday sign-ins

- **Offline TOTP.** RFC 6238 code generation with configurable algorithm, digit count, and period.
- **Several ways to add accounts.** Secret entry, `otpauth://` parsing, and QR image decoding (including bounded Google Authenticator migration QR import).
- **Find and copy.** Account search, editable account details, and one-click code copying. The copied code is cleared after 15 seconds if it is still the clipboard's current content.
- **Local secret storage.** `KeyringAesGcmVault` stores a 32-byte master key in the OS keyring (Windows Credential Manager or Linux Secret Service via GNOME Keyring / KWallet) and encrypts each secret individually with AES-256-GCM.
- **Automatic legacy migration.** On Windows, if the Tauri vault is empty and a legacy Electron installation exists under `%APPDATA%\authapp`, Authly performs an automatic one-time migration by unprotecting the legacy DPAPI key and re-encrypting secrets into `dev.authapp.desktop`.
- **PIN access lock.** Create, change, or remove a master PIN (4–12 characters) or lock immediately from Settings, with 5-minute inactivity auto-lock and exponential backoff after repeated failed attempts.
- **Encrypted backups.** Password-protected export and import using PBKDF2-SHA256 (600,000 iterations) and AES-256-GCM.
- **Desktop controls.** System-tray behavior, English/Turkish language settings, and light/dark/system appearance.
- **Release update checks.** Packaged builds query the GitHub Releases API (`releases/latest`) and open the verified GitHub release page in your external browser so you can download updates.

## Make it yours

Appearance, language, PIN lock settings, tray behavior, encrypted backups, and update checks live in one settings view.

<p align="center">
  <a href=".github/readme/images/settings.png"><img src=".github/readme/images/settings.png" alt="Authly settings for theme, language, PIN lock, tray behavior, encrypted backups, and updates" width="100%"></a>
</p>

## Install

**[Get the latest release →](https://github.com/ardakrt/Authly-Rust/releases/latest)**

Download the Windows NSIS installer or Linux `.deb` / `.AppImage` packages from the release assets, or follow the [build instructions](.github/readme/DEVELOPMENT.md#packaging).

- **Windows:** Windows 10/11 x64 (Microsoft Edge WebView2).
- **Linux:** x64 distribution with an installed, unlocked Secret Service keyring (`GNOME Keyring` or `KWallet`).

On Linux, Authly requires an available, unlocked OS keyring (`sync-secret-service`). If the keyring is unavailable or locked, `KeyringAesGcmVault` refuses to store secrets rather than falling back to plaintext storage.

## Local by design

Account data and encrypted secrets live in your local per-user application-data directory (`dev.authapp.desktop`). A cloud account is not required for code generation or account management. Portable recovery uses a separate password-encrypted backup file.

<details>
<summary><strong>Storage, encryption, and network behavior</strong></summary>

- **Storage:** SQLite (`authapp.db`) and AES-256-GCM encrypted secrets are stored under `%APPDATA%\dev.authapp.desktop` on Windows and `~/.local/share/dev.authapp.desktop` on Linux. On Windows, a one-time startup migration imports accounts, groups, and settings from legacy Electron `%APPDATA%\authapp` when the target vault is empty. Runtime databases and personal backups do not belong in Git.
- **Backups:** The current export format (`authapp-backup-v1`, payload version 2) derives a 256-bit key with PBKDF2-SHA256 and 600,000 iterations, then encrypts the backup with AES-256-GCM. Keep the backup password separately from the backup file.
- **PIN boundary:** The PIN gates application access, sensitive operations, and settings writes. OS keyring + AES-256-GCM protects stored account secrets; the PIN is not an independent encryption key for the SQLite vault.
- **Process boundary:** The React UI inside the Tauri WebView accesses account operations only through typed, Zod-validated Tauri commands (`src/renderer/src/bridge/tauriBridge.ts` → Rust `#[tauri::command]` handlers in `src-tauri/`) rather than direct filesystem or SQLite access.
- **Network:** TOTP generation and UI fonts (`@fontsource/*` bundled assets) are offline. Update checks query `https://api.github.com/repos/ardakrt/Authly-Rust/releases/latest`, and installing an update opens the GitHub release URL in your default external browser.

</details>

## Develop

**Tauri v2 · Rust · React · TypeScript · Vite · SQLite**

With Node.js 20.19+, npm 11+, and the stable Rust toolchain (`rustc` / `cargo`) installed (plus Linux WebKitGTK/keyring system libraries on Ubuntu/Debian):

```bash
git clone https://github.com/ardakrt/Authly-Rust.git
cd Authly-Rust
npm ci
npm run dev
```

Run the project's verification pipeline and Tauri smoke check:

```bash
npm run verify
npm run smoke
```

`npm run verify` runs formatting, ESLint, TypeScript checks, Vitest web tests, Rust Cargo tests, and the Vite renderer build. `npm run smoke` verifies the renderer bundle and runs Rust tests under `AUTHAPP_SMOKE_TEST=1`. **[Development and packaging guide →](.github/readme/DEVELOPMENT.md)**

## License

[MIT](LICENSE). See the license file for attribution and terms.

---

<p align="center">
  <img src="resources/icon.png" alt="" width="36" height="36"><br>
  <sub><strong>Authly</strong> · Your codes. Your device.</sub>
</p>
