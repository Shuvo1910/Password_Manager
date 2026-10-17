# Threat Model

## Assets

Master password (in transient UI buffers), DEK, recovery key, account titles, usernames, passwords, URLs, notes, tags, and serialized vault state.

## Adversaries considered

- Someone who copies the executable or encrypted vault file.
- Someone who reads or modifies the vault while it is locked.
- Someone who supplies malformed vault files or altered ciphertext.
- Someone who attempts offline master-password guessing.
- Accidental disclosure through plaintext logs, exports, or persistent sidecar files.

## Intended controls

- Random DEK independent of the master password.
- Argon2id-derived KEK and authenticated DEK wrapping.
- Separate high-entropy recovery key and authenticated wrapper.
- XChaCha20-Poly1305 payload encryption with fresh nonces.
- Versioned magic/header and bounds on KDF parameters and file sizes.
- Payload authentication binds vault version, KDF metadata, salt, and both wrapped-key blobs to prevent undetected header edits.
- Local-only search over decrypted in-memory records.
- Explicit copy actions; no automatic password copying.

## Out of scope / limitations

- Malware/keyloggers or a compromised OS while unlocked.
- Memory capture, swap, hibernation, crash dumps, and GUI/allocator copies.
- Weak master-password guessing after vault theft.
- Theft of both the vault and recovery key.
- Clipboard history or third-party clipboard managers.
- Secure erasure of old copies on SSDs, snapshots, or backups.
- Supply-chain attacks and unreviewed dependencies.

## Residual risk

This source snapshot is a prototype, not a completed production password manager. Clipboard clearing, cross-platform validation, network-isolation testing, comprehensive test execution, and a full independent review remain outstanding.
