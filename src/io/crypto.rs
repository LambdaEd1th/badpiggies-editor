//! Decryption for Bad Piggies save files (Progress.dat, .contraption, Achievements.xml).
//!
//! Uses PBKDF2-HMAC-SHA1 key derivation + AES-256-CBC, matching the game's CryptoUtility.

use cbc::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit, block_padding::Pkcs7};
use pbkdf2::pbkdf2_hmac;
use sha1::{Digest, Sha1};

use crate::diagnostics::error::{AppError, AppResult};
use crate::i18n::locale::I18n;

type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;
type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;

/// Fixed salt used by the game's `Rfc2898DeriveBytes`.
const SALT: &[u8] = &[82, 166, 66, 87, 146, 51, 179, 108, 242, 110, 98, 237, 124];

/// PBKDF2 iteration count — .NET `Rfc2898DeriveBytes` defaults to 1000.
const PBKDF2_ITERATIONS: u32 = 1000;

/// Derive AES-256 key (32 bytes) and IV (16 bytes) from a password using PBKDF2.
fn derive_key_iv(password: &str) -> AppResult<([u8; 32], [u8; 16])> {
    // .NET Rfc2898DeriveBytes uses HMAC-SHA1 and produces a continuous stream.
    // Two consecutive GetBytes(32) and GetBytes(16) calls produce 48 bytes total.
    let mut derived = [0u8; 48];
    pbkdf2_hmac::<Sha1>(password.as_bytes(), SALT, PBKDF2_ITERATIONS, &mut derived);
    let mut key = [0u8; 32];
    let mut iv = [0u8; 16];
    key.copy_from_slice(&derived[..32]);
    iv.copy_from_slice(&derived[32..48]);
    Ok((key, iv))
}

/// Encrypt data with AES-256-CBC.
fn aes_encrypt(key: &[u8; 32], iv: &[u8; 16], plaintext: &[u8]) -> AppResult<Vec<u8>> {
    Ok(Aes256CbcEnc::new(key.into(), iv.into()).encrypt_padded_vec::<Pkcs7>(plaintext))
}

/// Decrypt AES-256-CBC data.
fn aes_decrypt(key: &[u8; 32], iv: &[u8; 16], ciphertext: &[u8]) -> AppResult<Vec<u8>> {
    Aes256CbcDec::new(key.into(), iv.into())
        .decrypt_padded_vec::<Pkcs7>(ciphertext)
        .map_err(|error| AppError::crypto_key1("error_aes_decrypt_failed", error.to_string()))
}

/// Verify SHA1 hash: first 20 bytes of file must match SHA1 of the rest.
fn verify_sha1(data: &[u8]) -> AppResult<&[u8]> {
    if data.len() < 20 {
        return Err(AppError::invalid_data_key("error_file_too_short_sha1"));
    }
    let (hash_bytes, payload) = data.split_at(20);
    let computed = Sha1::digest(payload);
    if computed.as_slice() != hash_bytes {
        return Err(AppError::invalid_data_key("error_sha1_mismatch"));
    }
    Ok(payload)
}

/// Known file types with their passwords and whether they have a SHA1 prefix.
#[derive(Clone)]
pub enum SaveFileType {
    /// `Progress.dat` — game progress
    Progress,
    /// `*.contraption` — player-built contraptions
    Contraption,
    /// `Achievements.xml` — achievement data
    Achievements,
}

impl SaveFileType {
    fn password(&self) -> &'static str {
        match self {
            Self::Progress => "56SA%FG42Dv5#4aG67f2",
            Self::Contraption => "3b91A049Ca7HvSjhxT35",
            Self::Achievements => "fHHg5#%3RRfnJi78&%lP?65",
        }
    }

    fn has_sha1_prefix(&self) -> bool {
        match self {
            Self::Progress | Self::Achievements => true,
            Self::Contraption => false,
        }
    }

    /// Detect file type from file name.
    pub fn detect(filename: &str) -> Option<Self> {
        let lower = filename.to_ascii_lowercase();
        if lower.contains("progress") && lower.ends_with(".dat") {
            Some(Self::Progress)
        } else if lower.ends_with(".contraption") {
            Some(Self::Contraption)
        } else if lower.contains("achievement") && lower.ends_with(".xml") {
            Some(Self::Achievements)
        } else {
            None
        }
    }

    pub fn localized_label(&self, i18n: &I18n) -> String {
        let key = match self {
            Self::Progress => "save_file_type_progress",
            Self::Contraption => "save_file_type_contraption",
            Self::Achievements => "save_file_type_achievements",
        };
        i18n.get(key)
    }
}

/// Compute SHA1 hash and prepend it to the data.
fn prepend_sha1(payload: &[u8]) -> Vec<u8> {
    let hash = Sha1::digest(payload);
    let mut result = Vec::with_capacity(20 + payload.len());
    result.extend_from_slice(&hash);
    result.extend_from_slice(payload);
    result
}

/// Encrypt XML bytes back into the save file format.
pub fn encrypt_save_file(file_type: &SaveFileType, xml_bytes: &[u8]) -> AppResult<Vec<u8>> {
    let (key, iv) = derive_key_iv(file_type.password())?;
    let ciphertext = aes_encrypt(&key, &iv, xml_bytes)?;
    if file_type.has_sha1_prefix() {
        Ok(prepend_sha1(&ciphertext))
    } else {
        Ok(ciphertext)
    }
}

/// Decrypt a save file and return the raw XML bytes.
pub fn decrypt_save_file(file_type: &SaveFileType, data: &[u8]) -> AppResult<Vec<u8>> {
    let (key, iv) = derive_key_iv(file_type.password())?;
    let ciphertext = if file_type.has_sha1_prefix() {
        verify_sha1(data)?
    } else {
        data
    };
    aes_decrypt(&key, &iv, ciphertext)
}
