# Test Plan

## Automated tests included (execution not verified)

- `crypto::tests::recovery_key_round_trip_and_checksum`: Base32 encoding, decoding, checksum rejection.
- `crypto::tests::aead_authenticates_ciphertext`: encryption/decryption round trip and ciphertext tamper rejection.
- `vault::tests::vault_create_save_unlock_and_reject_wrong_password`: create/save/open, wrong-password rejection, recovery unlock, and plaintext canary absence.
- `vault::tests::password_change_and_recovery_rotate_wrappers_atomically`: old credential invalidation and recovery continuity after wrapper changes.
- `vault::tests::header_tampering_is_detected_by_payload_authentication`: rejects modified authenticated header metadata.

## Execution status

Not run in this environment: `rustc` and `cargo` were not found on PATH. Therefore no compile, test, clippy, dependency audit, GUI run, or offline runtime test is claimed as passed. Generate `Cargo.lock` and execute the commands in `BUILDING.md` in a Rust-enabled environment. `src/lib.rs` previously declared a missing `storage` module; that declaration has been removed, but compilation must still be checked with Cargo.

## Manual acceptance checklist

- [ ] Create a vault with matching master password confirmation.
- [ ] Save and verify the recovery key; create the vault only after confirmation.
- [ ] Close and reopen the app; unlock with the correct master password.
- [ ] Verify wrong password never shows records.
- [ ] Add, edit, search, favorite, copy, and delete entries.
- [ ] Generate a password using the OS CSPRNG.
- [ ] Recover with the recovery key; verify the old master password fails.
- [ ] Change the master password and verify the recovery wrapper remains usable.
- [ ] Modify ciphertext/header and verify fail-closed behavior.
- [ ] Search raw vault bytes for canary title, username, password, URL, and notes.
- [ ] Run under OS-level network denial and verify all supported local workflows.
- [ ] Test atomic saves and abandoned temporary files on each target filesystem.
- [ ] Build and test on Windows, Linux, and macOS separately.
