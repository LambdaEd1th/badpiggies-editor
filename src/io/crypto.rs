//! Decryption for Bad Piggies save files (Progress.dat, .contraption, Achievements.xml).
//!
//! Uses PBKDF2-HMAC-SHA1 key derivation + AES-256-CBC, matching the game's CryptoUtility.

use cbc::cipher::{BlockModeDecrypt, KeyIvInit, block_padding::Pkcs7};
use hmac::{Hmac, KeyInit, Mac};
use sha1::{Digest, Sha1};

use crate::error::{AppError, AppResult};
use crate::locale::I18n;

type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;
type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;
type HmacSha1 = Hmac<Sha1>;

/// Fixed salt used by the game's `Rfc2898DeriveBytes`.
const SALT: &[u8] = &[82, 166, 66, 87, 146, 51, 179, 108, 242, 110, 98, 237, 124];

/// PBKDF2 iteration count — .NET `Rfc2898DeriveBytes` defaults to 1000.
const PBKDF2_ITERATIONS: u32 = 1000;

/// Derive AES-256 key (32 bytes) and IV (16 bytes) from a password using PBKDF2.
fn derive_key_iv(password: &str) -> AppResult<([u8; 32], [u8; 16])> {
    // .NET Rfc2898DeriveBytes uses HMAC-SHA1 and produces a continuous stream.
    // Two consecutive GetBytes(32) and GetBytes(16) calls produce 48 bytes total.
    let mut derived = [0u8; 48];
    pbkdf2_hmac_sha1(password.as_bytes(), SALT, PBKDF2_ITERATIONS, &mut derived)?;
    let mut key = [0u8; 32];
    let mut iv = [0u8; 16];
    key.copy_from_slice(&derived[..32]);
    iv.copy_from_slice(&derived[32..48]);
    Ok((key, iv))
}

/// PBKDF2-HMAC-SHA1 (RFC 2898) — implemented directly to avoid digest version conflicts.
fn pbkdf2_hmac_sha1(
    password: &[u8],
    salt: &[u8],
    iterations: u32,
    output: &mut [u8],
) -> AppResult<()> {
    let hlen = 20; // SHA1 output length
    let mut block_num = 1u32;
    let mut offset = 0;
    while offset < output.len() {
        // U1 = PRF(Password, Salt || INT_32_BE(i))
        let mut mac = HmacSha1::new_from_slice(password)
            .map_err(|_| AppError::crypto_key("error_pbkdf2_init"))?;
        mac.update(salt);
        mac.update(&block_num.to_be_bytes());
        let u1 = mac.finalize().into_bytes();

        let mut u_prev: [u8; 20] = u1.into();
        let mut result = u_prev;

        // U2..Uc — XOR all rounds together
        for _ in 1..iterations {
            let mut mac = HmacSha1::new_from_slice(password)
                .map_err(|_| AppError::crypto_key("error_pbkdf2_init"))?;
            mac.update(&u_prev);
            let u_next = mac.finalize().into_bytes();
            u_prev = u_next.into();
            for (r, n) in result.iter_mut().zip(u_prev.iter()) {
                *r ^= n;
            }
        }

        let to_copy = (output.len() - offset).min(hlen);
        output[offset..offset + to_copy].copy_from_slice(&result[..to_copy]);
        offset += to_copy;
        block_num += 1;
    }

    Ok(())
}

/// Encrypt data with AES-256-CBC.
fn aes_encrypt(key: &[u8; 32], iv: &[u8; 16], plaintext: &[u8]) -> AppResult<Vec<u8>> {
    use cbc::cipher::BlockModeEncrypt;
    // cipher 0.5 has no encrypt_padded_vec; allocate buffer manually.
    let block_size = 16;
    let padded_len = (plaintext.len() / block_size + 1) * block_size;
    let mut buf = vec![0u8; padded_len];
    buf[..plaintext.len()].copy_from_slice(plaintext);
    let ct = Aes256CbcEnc::new(key.into(), iv.into())
        .encrypt_padded::<Pkcs7>(&mut buf, plaintext.len())
        .map_err(|_| AppError::crypto_key("error_aes_encrypt_buffer_too_small"))?;
    Ok(ct.to_vec())
}

/// Decrypt AES-256-CBC data.
fn aes_decrypt(key: &[u8; 32], iv: &[u8; 16], ciphertext: &[u8]) -> AppResult<Vec<u8>> {
    let mut buf = ciphertext.to_vec();
    let decrypted = Aes256CbcDec::new(key.into(), iv.into())
        .decrypt_padded::<Pkcs7>(&mut buf)
        .map_err(|error| AppError::crypto_key1("error_aes_decrypt_failed", error.to_string()))?;
    Ok(decrypted.to_vec())
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
