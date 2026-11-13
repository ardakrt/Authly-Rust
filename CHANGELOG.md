# Changelog

## 0.3.12

- Fix account search dialog (`SearchDialog`) to load registered accounts and live TOTP codes instead of showing an empty-accounts message.
- Filter and rank accounts across issuer and account name with Turkish and default locale case matching, keyboard navigation (`ArrowUp`/`ArrowDown`/`Enter`/`Escape`), one-click TOTP code copy, and jump-to-card highlighting.
- Refresh TOTP codes on period rollover while the search dialog is open and reset search state when closed or locked.

## 0.3.11

- Validate and normalize Base32 account keys before storage; isolate unreadable legacy accounts so healthy codes remain available.
- Reject malformed migration QR data and unsupported HOTP accounts; save selected QR accounts atomically.
- Reload accounts after PIN unlock and clear sensitive views when the application locks.
- Enforce inactivity locking in main, lock on system lock/suspend, and revoke pending sensitive operations across lock sessions.
- Restore grouped accounts and backups atomically; continue reading legacy backups and report missing group metadata without skipping accounts.
- Add regression coverage and real Electron account/lock/unlock smoke checks.
