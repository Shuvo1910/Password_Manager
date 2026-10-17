# Password Manager

Password Manager is an offline-first Rust desktop password-manager prototype. It uses a native `egui/eframe` interface and an encrypted, portable `.vault` file. It does not implement network requests, telemetry, cloud sync, or remote asset loading.

## Implementation status

This is an early, unaudited implementation. The source provides native create/open/unlock UI; a random DEK, Argon2id key wrapping, XChaCha20-Poly1305 payload encryption; offline recovery key and rewrapping; local CRUD/search/favorites; password generation; clipboard copy; embedded normalized PNG icons; user-selected encrypted backup export; and a 5-minute activity lock. Header metadata and wrapped keys are bound to payload authentication. The app does not implement network features. Format version 2 is incompatible with the incomplete version 1 prototype files; the prototype never had a stable release format, and version 1 files are rejected rather than migrated.

Build, test, clippy, dependency audit, release, and offline-isolation checks have not been run in the current environment. No cross-platform compatibility or production readiness is claimed. Clipboard auto-clear (30 s, only if the clipboard still holds the copied value) is implemented but untested on all platforms. Known remaining work includes suspend/resume lock behavior, robust transaction recovery, password strength feedback, fuzzing and migration coverage, platform validation, and independent review. Keep separate encrypted backups and do not rely on this app as the sole copy of critical credentials until it has been validated.

## Build

Install a current stable Rust toolchain and native GUI build prerequisites, then run:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo run --release
```

The first build may download crates. Runtime application behavior is designed to be offline. Generate and commit `Cargo.lock` from a Rust-enabled build environment.

## Portable layout

```text
Password Manager/
  password-manager.exe (Windows) / password-manager (Linux) / Password Manager.app (macOS)
  PasswordManager.vault (created by the user)
```

The app asks the user to choose the vault path. It does not silently fall back to a profile directory. On macOS, keep the vault beside the `.app` bundle, not inside it.

## Recovery warning

The recovery key is the only fallback when the master password is forgotten. Store it separately in a secure offline location. If both are lost, the vault cannot be recovered. Copying the recovery key may expose it to clipboard history.
