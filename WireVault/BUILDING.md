# Building Password Manager

## Prerequisites

- Current stable Rust toolchain (`rustup`, `rustc`, `cargo`).
- Rust 1.85 or newer (the manifest declares the minimum compiler version).
- Platform native prerequisites for `eframe`/`winit` and `rfd`.

## Commands

```sh
rustup update stable
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo run --release
```

Create a release binary with `cargo build --release`. The result is under `target/release/` (`password-manager.exe` on Windows).

## Platform notes

- **Windows:** Build on Windows for a native executable. Verify running from a writable portable folder and test Windows file replacement behavior.
- **Linux:** Build on the target distribution or a compatible build environment. Verify required system GUI libraries and permissions.
- **macOS:** Build on macOS for a native app/binary. The vault must remain outside the signed `.app` bundle. Signing/notarization are not configured here.

A binary built for one operating system is not a native binary for the others. Cross-platform releases need native CI/build runners and tests. The vault format is intended to be OS-independent, but cross-build interoperability has not yet been verified.
