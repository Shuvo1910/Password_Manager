use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    aead::{Aead, Payload},
    KeyInit, XChaCha20Poly1305, XNonce,
};
use rand_core::{OsRng, RngCore};
use zeroize::Zeroizing;

use crate::vault::VaultError;

pub const SALT_LEN: usize = 16;
pub const KEY_LEN: usize = 32;
pub const NONCE_LEN: usize = 24;
// OWASP-recommended Argon2id baseline: 19 MiB, 2 iterations, 1 lane.
pub const ARGON_MEMORY_KIB: u32 = 19 * 1024;
pub const ARGON_ITERATIONS: u32 = 2;
pub const ARGON_LANES: u32 = 1;

pub fn random_bytes<const N: usize>() -> Result<[u8; N], VaultError> {
    let mut out = [0u8; N];
    OsRng
        .try_fill_bytes(&mut out)
        .map_err(|_| VaultError::Randomness)?;
    Ok(out)
}

pub fn derive_kek(
    password: &[u8],
    salt: &[u8; SALT_LEN],
    memory_kib: u32,
    iterations: u32,
    lanes: u32,
) -> Result<Zeroizing<[u8; KEY_LEN]>, VaultError> {
    // Validate before doing expensive work; file-supplied values are untrusted.
    if !(19 * 1024..=256 * 1024).contains(&memory_kib)
        || !(2..=10).contains(&iterations)
        || !(1..=4).contains(&lanes)
    {
        return Err(VaultError::InvalidHeader(
            "KDF parameters outside accepted bounds".into(),
        ));
    }
    let params = Params::new(memory_kib, iterations, lanes, Some(KEY_LEN))
        .map_err(|_| VaultError::InvalidHeader("invalid KDF parameters".into()))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    argon
        .hash_password_into(password, salt, key.as_mut())
        .map_err(|_| VaultError::Crypto)?;
    Ok(key)
}

pub fn seal(key: &[u8; KEY_LEN], plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>, VaultError> {
    let nonce = random_bytes::<NONCE_LEN>()?;
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| VaultError::Crypto)?;
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| VaultError::Crypto)?;
    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

pub fn open(key: &[u8; KEY_LEN], sealed: &[u8], aad: &[u8]) -> Result<Vec<u8>, VaultError> {
    if sealed.len() < NONCE_LEN + 16 {
        return Err(VaultError::Crypto);
    }
    let (nonce, ciphertext) = sealed.split_at(NONCE_LEN);
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| VaultError::Crypto)?;
    cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| VaultError::Authentication)
}

/// RFC 4648 Base32 without padding, plus a 24-bit BLAKE3 checksum.
pub fn encode_recovery_key(key: &[u8; KEY_LEN]) -> String {
    const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let digest = blake3::hash(key);
    let mut data = Vec::with_capacity(35);
    data.extend_from_slice(key);
    data.extend_from_slice(&digest.as_bytes()[..3]);
    let mut bits: u32 = 0;
    let mut count = 0;
    let mut out = String::new();
    for byte in data {
        bits = (bits << 8) | byte as u32;
        count += 8;
        while count >= 5 {
            count -= 5;
            out.push(ALPHABET[((bits >> count) & 31) as usize] as char);
        }
    }
    if count > 0 {
        out.push(ALPHABET[((bits << (5 - count)) & 31) as usize] as char);
    }
    // Group for human entry.
    out.as_bytes()
        .chunks(5)
        .map(|c| std::str::from_utf8(c).unwrap_or(""))
        .collect::<Vec<_>>()
        .join("-")
}

pub fn decode_recovery_key(input: &str) -> Result<[u8; KEY_LEN], VaultError> {
    let mut normalized = String::with_capacity(56);
    for ch in input.chars() {
        if ch == '-' || ch.is_ascii_whitespace() {
            continue;
        }
        if !ch.is_ascii_alphanumeric() {
            return Err(VaultError::InvalidRecoveryKey);
        }
        normalized.push(ch.to_ascii_uppercase());
    }
    if normalized.len() != 56 {
        return Err(VaultError::InvalidRecoveryKey);
    }
    let mut bytes = Zeroizing::new(Vec::with_capacity(35));
    let mut bits: u32 = 0;
    let mut count = 0;
    for ch in normalized.bytes() {
        let value = match ch {
            b'A'..=b'Z' => ch - b'A',
            b'2'..=b'7' => ch - b'2' + 26,
            _ => return Err(VaultError::InvalidRecoveryKey),
        };
        bits = (bits << 5) | value as u32;
        count += 5;
        if count >= 8 {
            count -= 8;
            bytes.push(((bits >> count) & 0xff) as u8);
        }
    }
    if bytes.len() != 35 {
        return Err(VaultError::InvalidRecoveryKey);
    }
    let key: [u8; KEY_LEN] = bytes[..KEY_LEN]
        .try_into()
        .map_err(|_| VaultError::InvalidRecoveryKey)?;
    let digest = blake3::hash(&key);
    if !constant_time_eq(&bytes[KEY_LEN..], &digest.as_bytes()[..3]) {
        return Err(VaultError::InvalidRecoveryKey);
    }
    Ok(key)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_key_round_trip_and_checksum() {
        let key = [0x5au8; 32];
        let encoded = encode_recovery_key(&key);
        assert_eq!(decode_recovery_key(&encoded).unwrap(), key);
        let mut bad = encoded.into_bytes();
        bad[0] = if bad[0] == b'A' { b'B' } else { b'A' };
        assert!(decode_recovery_key(std::str::from_utf8(&bad).unwrap()).is_err());
        assert!(decode_recovery_key("AA/AA").is_err());
    }
    #[test]
    fn aead_authenticates_ciphertext() {
        let key = [7u8; 32];
        let sealed = seal(&key, b"secret", b"payload-v1").unwrap();
        assert_eq!(open(&key, &sealed, b"payload-v1").unwrap(), b"secret");
        let mut changed = sealed;
        *changed.last_mut().unwrap() ^= 1;
        assert!(open(&key, &changed, b"payload-v1").is_err());
    }
}
