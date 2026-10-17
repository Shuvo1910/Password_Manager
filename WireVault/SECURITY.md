# Security Notes

This project has **not** undergone an independent security audit. It must not be described as audited, certified, or perfectly secure.

## Intended protections

- Vault payload and wrapped DEK are protected by XChaCha20-Poly1305 AEAD.
- A random 256-bit DEK encrypts the payload; the master password is processed with Argon2id and only wraps the DEK.
- A separate random recovery key wraps the same DEK.
- KDF parameters and header lengths are bounded before expensive derivation or deserialization.
- Header version, KDF parameters, salt, and both key wrappers are authenticated as payload associated data.
- Data is serialized in memory and encrypted before being written to disk.
- No password export to plaintext files is implemented.

## Format compatibility

The authenticated-header format uses version 2 (`CVAULT02`). Earlier prototype version 1 files are rejected; there is no automatic migration because no released stable version 1 format was established.

## Important limitations

- A copied vault can be attacked offline; a weak master password remains vulnerable to guessing.
- Malware, keyloggers, memory capture, an attacker controlling the operating system, clipboard history, swap/hibernation, crash dumps, and compromised dependencies are outside the application's complete control.
- `zeroize` provides best-effort clearing only; Rust strings, GUI buffers, allocator behavior, and OS memory management can leave copies.
- Atomic replacement and crash behavior differ by filesystem and operating system; the implementation needs platform-specific testing and abandoned-transaction recovery. Durability after a sudden power loss has not been verified.
- Clipboard auto-clear (30 s, only if unchanged) is best-effort and cannot defeat clipboard managers or history. Comprehensive offline-network verification has not been performed.
- Secure deletion cannot be guaranteed on SSDs, copy-on-write filesystems, snapshots, or backups.

## User recommendations

Use a strong unique master password, store the recovery key separately, keep encrypted backups under your own control, and enable full-disk encryption on the host. Do not run the application on a device you suspect is compromised.
