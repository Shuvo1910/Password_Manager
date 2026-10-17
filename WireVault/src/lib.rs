//! password manager cryptographic and storage core.
//! This crate is an early implementation; see README.md for unverified items.

pub mod crypto;
pub mod vault;

pub const FORMAT_VERSION: u16 = 1;
pub const MAX_VAULT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_ICON_BYTES: usize = 2 * 1024 * 1024;
