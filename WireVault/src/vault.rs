use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

use crate::{
    crypto::{self, ARGON_ITERATIONS, ARGON_LANES, ARGON_MEMORY_KIB, KEY_LEN, SALT_LEN},
    FORMAT_VERSION, MAX_ICON_BYTES, MAX_VAULT_BYTES,
};

const MAGIC: &[u8; 8] = b"CVAULT02";
const PAYLOAD_DOMAIN: &[u8] = b"CipherVault:payload:v1\0";
const MAX_HEADER_BYTES: usize = 4096;

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("secure random number generation failed")]
    Randomness,
    #[error("cryptographic operation failed")]
    Crypto,
    #[error("authentication failed; check your password or recovery key")]
    Authentication,
    #[error("invalid recovery key")]
    InvalidRecoveryKey,
    #[error("invalid vault header: {0}")]
    InvalidHeader(String),
    #[error("unsupported vault format version")]
    UnsupportedVersion,
    #[error("vault file exceeds size limit")]
    TooLarge,
    #[error("vault data is malformed")]
    Serialization,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Entry {
    pub id: Uuid,
    pub title: String,
    pub username: String,
    pub password: String,
    pub url: String,
    pub notes: String,
    pub tags: Vec<String>,
    pub favorite: bool,
    pub icon_mime: Option<String>,
    pub icon_bytes: Option<Vec<u8>>,
    pub created_unix: u64,
    pub modified_unix: u64,
}
impl Entry {
    pub fn new(title: String, username: String, password: String) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            id: Uuid::new_v4(),
            title,
            username,
            password,
            url: String::new(),
            notes: String::new(),
            tags: vec![],
            favorite: false,
            icon_mime: None,
            icon_bytes: None,
            created_unix: now,
            modified_unix: now,
        }
    }
    pub fn validate(&self) -> Result<(), VaultError> {
        if self.title.trim().is_empty() || self.password.is_empty() {
            return Err(VaultError::InvalidHeader(
                "title and password are required".into(),
            ));
        }
        if self
            .icon_bytes
            .as_ref()
            .is_some_and(|b| b.len() > MAX_ICON_BYTES)
        {
            return Err(VaultError::TooLarge);
        }
        if self.icon_bytes.is_some() && self.icon_mime.as_deref() != Some("image/png") {
            return Err(VaultError::InvalidHeader(
                "custom icons must be normalized PNG images".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct VaultData {
    pub entries: Vec<Entry>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Header {
    version: u16,
    memory_kib: u32,
    iterations: u32,
    lanes: u32,
    salt: [u8; SALT_LEN],
    master_wrapped_dek: Vec<u8>,
    recovery_wrapped_dek: Vec<u8>,
}

pub struct UnlockedVault {
    pub data: VaultData,
    dek: Zeroizing<[u8; KEY_LEN]>,
    header: Header,
    path: Option<std::path::PathBuf>,
}
impl std::fmt::Debug for UnlockedVault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnlockedVault")
            .field("entry_count", &self.data.entries.len())
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}
impl UnlockedVault {
    pub fn create(password: &[u8]) -> Result<(Self, String), VaultError> {
        if password.is_empty() {
            return Err(VaultError::InvalidHeader(
                "master password must not be empty".into(),
            ));
        }
        let dek = Zeroizing::new(crypto::random_bytes::<KEY_LEN>()?);
        let recovery = crypto::random_bytes::<KEY_LEN>()?;
        let salt = crypto::random_bytes::<SALT_LEN>()?;
        let kek = crypto::derive_kek(
            password,
            &salt,
            ARGON_MEMORY_KIB,
            ARGON_ITERATIONS,
            ARGON_LANES,
        )?;
        let mut header = Header {
            version: FORMAT_VERSION,
            memory_kib: ARGON_MEMORY_KIB,
            iterations: ARGON_ITERATIONS,
            lanes: ARGON_LANES,
            salt,
            master_wrapped_dek: Vec::new(),
            recovery_wrapped_dek: Vec::new(),
        };
        header.master_wrapped_dek = crypto::seal(&kek, dek.as_ref(), &master_header_aad(&header))?;
        header.recovery_wrapped_dek =
            crypto::seal(&recovery, dek.as_ref(), &recovery_header_aad(&header))?;
        Ok((
            Self {
                data: VaultData::default(),
                dek,
                header,
                path: None,
            },
            crypto::encode_recovery_key(&recovery),
        ))
    }

    pub fn unlock_password(path: &Path, password: &[u8]) -> Result<Self, VaultError> {
        let (header, payload) = read_file(path)?;
        let kek = crypto::derive_kek(
            password,
            &header.salt,
            header.memory_kib,
            header.iterations,
            header.lanes,
        )?;
        let dek_bytes = Zeroizing::new(
            crypto::open(
                &kek,
                &header.master_wrapped_dek,
                &master_header_aad(&header),
            )
            .map_err(|_| VaultError::Authentication)?,
        );
        let dek_arr: [u8; KEY_LEN] = dek_bytes
            .as_slice()
            .try_into()
            .map_err(|_| VaultError::Authentication)?;
        let dek = Zeroizing::new(dek_arr);
        let plaintext = Zeroizing::new(crypto::open(&dek, &payload, &payload_aad(&header))?);
        let data: VaultData = deserialize_data(&plaintext)?;
        validate_data(&data)?;
        Ok(Self {
            data,
            dek,
            header,
            path: Some(path.to_path_buf()),
        })
    }

    pub fn unlock_recovery(path: &Path, recovery_text: &str) -> Result<Self, VaultError> {
        let recovery = Zeroizing::new(crypto::decode_recovery_key(recovery_text)?);
        let (header, payload) = read_file(path)?;
        let dek_bytes = Zeroizing::new(
            crypto::open(
                &recovery,
                &header.recovery_wrapped_dek,
                &recovery_header_aad(&header),
            )
            .map_err(|_| VaultError::Authentication)?,
        );
        let dek_arr: [u8; KEY_LEN] = dek_bytes
            .as_slice()
            .try_into()
            .map_err(|_| VaultError::Authentication)?;
        let dek = Zeroizing::new(dek_arr);
        let plaintext = Zeroizing::new(crypto::open(&dek, &payload, &payload_aad(&header))?);
        let data: VaultData = deserialize_data(&plaintext)?;
        validate_data(&data)?;
        Ok(Self {
            data,
            dek,
            header,
            path: Some(path.to_path_buf()),
        })
    }

    pub fn save_as(&mut self, path: &Path) -> Result<(), VaultError> {
        self.write_to_path(path, &self.header)?;
        self.path = Some(path.to_path_buf());
        Ok(())
    }

    fn write_to_path(&self, path: &Path, header: &Header) -> Result<(), VaultError> {
        validate_data(&self.data)?;
        use bincode::Options;
        let payload = bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .with_limit(MAX_VAULT_BYTES as u64)
            .serialize(&self.data)
            .map_err(|_| VaultError::Serialization)?;
        if payload.len() > MAX_VAULT_BYTES {
            return Err(VaultError::TooLarge);
        }
        let sealed = crypto::seal(&self.dek, &payload, &payload_aad(header))?;
        let header_bytes = bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .with_limit(MAX_HEADER_BYTES as u64)
            .serialize(header)
            .map_err(|_| VaultError::Serialization)?;
        if header_bytes.len() > MAX_HEADER_BYTES {
            return Err(VaultError::TooLarge);
        }
        let mut bytes = Vec::with_capacity(8 + 4 + header_bytes.len() + sealed.len());
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&(header_bytes.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&header_bytes);
        bytes.extend_from_slice(&sealed);
        if bytes.len() > MAX_VAULT_BYTES {
            return Err(VaultError::TooLarge);
        }
        atomic_write(path, &bytes)
    }

    pub fn export_encrypted_copy(&self, path: &Path) -> Result<(), VaultError> {
        self.write_to_path(path, &self.header)
    }

    pub fn create_encrypted_copy(&self, path: &Path) -> Result<(), VaultError> {
        if path.exists() {
            return Err(VaultError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "destination already exists",
            )));
        }
        self.write_to_path(path, &self.header)
    }

    pub fn save(&mut self) -> Result<(), VaultError> {
        let path = self
            .path
            .clone()
            .ok_or_else(|| VaultError::InvalidHeader("vault has not been saved yet".into()))?;
        self.save_as(&path)
    }

    pub fn change_master_password(&mut self, new_password: &[u8]) -> Result<(), VaultError> {
        if new_password.is_empty() {
            return Err(VaultError::InvalidHeader(
                "master password must not be empty".into(),
            ));
        }
        let salt = crypto::random_bytes::<SALT_LEN>()?;
        let kek = crypto::derive_kek(
            new_password,
            &salt,
            ARGON_MEMORY_KIB,
            ARGON_ITERATIONS,
            ARGON_LANES,
        )?;
        let mut header = self.header.clone();
        header.salt = salt;
        header.memory_kib = ARGON_MEMORY_KIB;
        header.iterations = ARGON_ITERATIONS;
        header.lanes = ARGON_LANES;
        header.master_wrapped_dek =
            crypto::seal(&kek, self.dek.as_ref(), &master_header_aad(&header))?;
        let path = self
            .path
            .as_deref()
            .ok_or_else(|| VaultError::InvalidHeader("vault has not been saved yet".into()))?;
        self.write_to_path(path, &header)?;
        self.header = header;
        Ok(())
    }

    pub fn rotate_recovery_key(&mut self) -> Result<String, VaultError> {
        let recovery = crypto::random_bytes::<KEY_LEN>()?;
        let mut header = self.header.clone();
        header.recovery_wrapped_dek =
            crypto::seal(&recovery, self.dek.as_ref(), &recovery_header_aad(&header))?;
        let path = self
            .path
            .as_deref()
            .ok_or_else(|| VaultError::InvalidHeader("vault has not been saved yet".into()))?;
        self.write_to_path(path, &header)?;
        self.header = header;
        Ok(crypto::encode_recovery_key(&recovery))
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}
impl Drop for UnlockedVault {
    fn drop(&mut self) {
        for entry in &mut self.data.entries {
            entry.title.zeroize();
            entry.username.zeroize();
            entry.password.zeroize();
            entry.url.zeroize();
            entry.notes.zeroize();
            entry.tags.iter_mut().for_each(Zeroize::zeroize);
            if let Some(bytes) = entry.icon_bytes.as_mut() {
                bytes.zeroize();
            }
        }
    }
}

fn master_header_aad(header: &Header) -> Vec<u8> {
    let mut aad = Vec::with_capacity(8 + 2 + 4 + 4 + 4 + SALT_LEN);
    aad.extend_from_slice(b"CV-KEY1\0");
    aad.extend_from_slice(&header.version.to_le_bytes());
    aad.extend_from_slice(&header.memory_kib.to_le_bytes());
    aad.extend_from_slice(&header.iterations.to_le_bytes());
    aad.extend_from_slice(&header.lanes.to_le_bytes());
    aad.extend_from_slice(&header.salt);
    aad
}

fn recovery_header_aad(header: &Header) -> Vec<u8> {
    let mut aad = b"CV-REC1\0".to_vec();
    aad.extend_from_slice(&header.version.to_le_bytes());
    aad
}

fn payload_aad(header: &Header) -> Vec<u8> {
    let mut aad = Vec::with_capacity(PAYLOAD_DOMAIN.len() + 2 + 4 + 4 + 4 + SALT_LEN + 64);
    aad.extend_from_slice(PAYLOAD_DOMAIN);
    aad.extend_from_slice(&header.version.to_le_bytes());
    aad.extend_from_slice(&header.memory_kib.to_le_bytes());
    aad.extend_from_slice(&header.iterations.to_le_bytes());
    aad.extend_from_slice(&header.lanes.to_le_bytes());
    aad.extend_from_slice(&header.salt);
    aad.extend_from_slice(&header.master_wrapped_dek);
    aad.extend_from_slice(&header.recovery_wrapped_dek);
    aad
}

fn validate_data(data: &VaultData) -> Result<(), VaultError> {
    if data.entries.len() > 100_000 {
        return Err(VaultError::TooLarge);
    }
    for e in &data.entries {
        e.validate()?;
    }
    Ok(())
}

fn deserialize_data(bytes: &[u8]) -> Result<VaultData, VaultError> {
    use bincode::Options;
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_limit(MAX_VAULT_BYTES as u64)
        .reject_trailing_bytes()
        .deserialize(bytes)
        .map_err(|_| VaultError::Serialization)
}

fn read_file(path: &Path) -> Result<(Header, Zeroizing<Vec<u8>>), VaultError> {
    let metadata = fs::metadata(path)?;
    if metadata.len() > MAX_VAULT_BYTES as u64 || metadata.len() < 12 {
        return Err(VaultError::TooLarge);
    }
    let file = fs::File::open(path)?;
    let mut bytes = Zeroizing::new(Vec::new());
    use std::io::Read;
    file.take((MAX_VAULT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_VAULT_BYTES {
        return Err(VaultError::TooLarge);
    }
    if bytes.len() < 12 {
        return Err(VaultError::InvalidHeader("truncated vault".into()));
    }
    if &bytes[..8] != MAGIC {
        return Err(VaultError::InvalidHeader("not a CipherVault file".into()));
    }
    let header_len = u32::from_le_bytes(
        bytes[8..12]
            .try_into()
            .map_err(|_| VaultError::InvalidHeader("truncated header".into()))?,
    ) as usize;
    if header_len == 0 || header_len > MAX_HEADER_BYTES || 12 + header_len >= bytes.len() {
        return Err(VaultError::InvalidHeader("invalid header length".into()));
    }
    use bincode::Options;
    let header: Header = bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_limit(MAX_HEADER_BYTES as u64)
        .reject_trailing_bytes()
        .deserialize(&bytes[12..12 + header_len])
        .map_err(|_| VaultError::InvalidHeader("malformed header".into()))?;
    if header.version != FORMAT_VERSION {
        return Err(VaultError::UnsupportedVersion);
    }
    // Reject invalid parameters before invoking Argon2.
    if !(19 * 1024..=256 * 1024).contains(&header.memory_kib)
        || !(2..=10).contains(&header.iterations)
        || !(1..=4).contains(&header.lanes)
    {
        return Err(VaultError::InvalidHeader(
            "KDF parameters outside accepted bounds".into(),
        ));
    }
    if header.master_wrapped_dek.len() != 72 || header.recovery_wrapped_dek.len() != 72 {
        return Err(VaultError::InvalidHeader(
            "invalid wrapped key length".into(),
        ));
    }
    Ok((header, Zeroizing::new(bytes[12 + header_len..].to_vec())))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), VaultError> {
    use std::io::Write;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .ok_or_else(|| VaultError::InvalidHeader("invalid destination path".into()))?
        .to_string_lossy();
    let temp = parent.join(format!(".{name}.{}.ciphervault-tmp", Uuid::new_v4()));
    let result = (|| -> Result<(), VaultError> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        // Never remove the last valid vault before attempting replacement.
        // Windows rename does not replace an existing file, so use the OS API.
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            use windows_sys::Win32::Foundation::GetLastError;
            use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;
            if path.exists() {
                let wide = |p: &Path| {
                    p.as_os_str()
                        .encode_wide()
                        .chain(Some(0))
                        .collect::<Vec<u16>>()
                };
                let target = wide(path);
                let replacement = wide(&temp);
                let ok = unsafe {
                    ReplaceFileW(
                        target.as_ptr(),
                        replacement.as_ptr(),
                        std::ptr::null(),
                        0,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    )
                };
                if ok == 0 {
                    let error = unsafe { GetLastError() } as i32;
                    // Filesystems that do not support ReplaceFileW may still
                    // support MoveFileExW with replace-existing semantics.
                    use windows_sys::Win32::Storage::FileSystem::{
                        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
                    };
                    let moved = unsafe {
                        MoveFileExW(
                            replacement.as_ptr(),
                            target.as_ptr(),
                            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
                        )
                    };
                    if moved == 0 {
                        return Err(std::io::Error::from_raw_os_error(error).into());
                    }
                }
            } else {
                fs::rename(&temp, path)?;
            }
        }
        #[cfg(not(windows))]
        fs::rename(&temp, path)?;
        if let Ok(dir) = fs::File::open(parent) {
            let _ = dir.sync_all();
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vault_create_save_unlock_and_reject_wrong_password() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.vault");
        let (mut vault, recovery) = UnlockedVault::create(b"correct horse battery staple").unwrap();
        vault.data.entries.push(Entry::new(
            "Example".into(),
            "alice".into(),
            "canary-password-934".into(),
        ));
        vault.save_as(&path).unwrap();
        assert!(UnlockedVault::unlock_password(&path, b"wrong").is_err());
        let opened =
            UnlockedVault::unlock_password(&path, b"correct horse battery staple").unwrap();
        assert_eq!(opened.data.entries[0].password, "canary-password-934");
        let recovered = UnlockedVault::unlock_recovery(&path, &recovery).unwrap();
        assert_eq!(recovered.data.entries.len(), 1);
        let raw = fs::read(&path).unwrap();
        assert!(!raw
            .windows(b"canary-password-934".len())
            .any(|w| w == b"canary-password-934"));
    }

    #[test]
    fn password_change_and_recovery_rotate_wrappers_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rotate.vault");
        let (mut vault, old_recovery) = UnlockedVault::create(b"first strong password").unwrap();
        vault.data.entries.push(Entry::new(
            "Private title".into(),
            "private user".into(),
            "private password".into(),
        ));
        vault.save_as(&path).unwrap();
        let new_recovery = vault.rotate_recovery_key().unwrap();
        assert!(UnlockedVault::unlock_recovery(&path, &old_recovery).is_err());
        let mut recovered = UnlockedVault::unlock_recovery(&path, &new_recovery).unwrap();
        recovered
            .change_master_password(b"second strong password")
            .unwrap();
        assert!(UnlockedVault::unlock_password(&path, b"first strong password").is_err());
        assert_eq!(
            UnlockedVault::unlock_password(&path, b"second strong password")
                .unwrap()
                .data
                .entries[0]
                .title,
            "Private title"
        );
        assert_eq!(
            UnlockedVault::unlock_recovery(&path, &new_recovery)
                .unwrap()
                .data
                .entries[0]
                .password,
            "private password"
        );
    }

    #[test]
    fn header_tampering_is_detected_by_payload_authentication() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tamper.vault");
        let (mut vault, _) = UnlockedVault::create(b"strong password").unwrap();
        vault.save_as(&path).unwrap();
        let mut bytes = fs::read(&path).unwrap();
        // Version and salt live in the serialized header. Flip a header byte
        // without changing its framing and ensure neither credential unlocks.
        bytes[12 + 2 + 4 + 4 + 4] ^= 1;
        fs::write(&path, bytes).unwrap();
        assert!(UnlockedVault::unlock_password(&path, b"strong password").is_err());
    }
}
