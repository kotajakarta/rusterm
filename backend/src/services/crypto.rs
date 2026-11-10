use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use sha2::Sha256;
use thiserror::Error;

use crate::models::server::{BackupEnvelope, BackupKdf, BackupPayload};

pub const BACKUP_MAGIC: &str = "TERMDECK_BACKUP";
pub const BACKUP_VERSION: u32 = 1;
pub const PBKDF2_ITERATIONS: u32 = 100_000;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Encryption failed: {0}")]
    Encryption(String),
    #[error("Decryption failed: {0}")]
    Decryption(String),
    #[error("Invalid ciphertext format: {0}")]
    InvalidFormat(String),
    #[error("Invalid backup PIN: {0}")]
    InvalidBackupPin(String),
    #[error("Invalid backup format: {0}")]
    InvalidBackupFormat(String),
}

pub struct CryptoService;

impl CryptoService {
    pub fn encrypt(plaintext: &[u8], key: &[u8; 32]) -> Result<String, CryptoError> {
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| CryptoError::Encryption(e.to_string()))?;
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| CryptoError::Encryption(e.to_string()))?;

        let mut payload = Vec::with_capacity(12 + ciphertext.len());
        payload.extend_from_slice(&nonce_bytes);
        payload.extend_from_slice(&ciphertext);

        Ok(BASE64.encode(payload))
    }

    pub fn decrypt(ciphertext_b64: &str, key: &[u8; 32]) -> Result<Vec<u8>, CryptoError> {
        let payload = BASE64
            .decode(ciphertext_b64)
            .map_err(|e| CryptoError::InvalidFormat(e.to_string()))?;

        if payload.len() < 12 {
            return Err(CryptoError::InvalidFormat("Payload too short for nonce".into()));
        }

        let (nonce_bytes, ciphertext) = payload.split_at(12);
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| CryptoError::Decryption(e.to_string()))?;
        let nonce = Nonce::from_slice(nonce_bytes);

        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| CryptoError::Decryption(e.to_string()))
    }

    pub fn derive_backup_key(pin: &str, salt: &[u8]) -> Result<[u8; 32], CryptoError> {
        if pin.is_empty() {
            return Err(CryptoError::InvalidBackupPin("PIN tidak boleh kosong".into()));
        }
        let mut key = [0u8; 32];
        pbkdf2_hmac::<Sha256>(pin.as_bytes(), salt, PBKDF2_ITERATIONS, &mut key);
        Ok(key)
    }

    pub fn encrypt_backup(payload: &BackupPayload, pin: &str) -> Result<BackupEnvelope, CryptoError> {
        let mut salt = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut salt);
        let key = Self::derive_backup_key(pin, &salt)?;

        let mut iv = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut iv);

        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| CryptoError::Encryption(e.to_string()))?;
        let nonce = Nonce::from_slice(&iv);

        let plaintext = serde_json::to_vec(payload)
            .map_err(|e| CryptoError::Encryption(e.to_string()))?;
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_slice())
            .map_err(|e| CryptoError::Encryption(e.to_string()))?;

        Ok(BackupEnvelope {
            magic: BACKUP_MAGIC.to_string(),
            version: BACKUP_VERSION,
            kdf: BackupKdf {
                algorithm: "PBKDF2-HMAC-SHA256".to_string(),
                iterations: PBKDF2_ITERATIONS,
                salt_hex: hex::encode(salt),
            },
            cipher: "AES-256-GCM".to_string(),
            iv_hex: hex::encode(iv),
            ciphertext_hex: hex::encode(ciphertext),
        })
    }

    pub fn decrypt_backup(envelope: &BackupEnvelope, pin: &str) -> Result<BackupPayload, CryptoError> {
        if envelope.magic != BACKUP_MAGIC {
            return Err(CryptoError::InvalidBackupFormat(
                "Format file backup tidak valid (magic header salah)".into(),
            ));
        }
        if envelope.version != BACKUP_VERSION {
            return Err(CryptoError::InvalidBackupFormat(format!(
                "Versi backup {} tidak didukung",
                envelope.version
            )));
        }

        let salt = hex::decode(&envelope.kdf.salt_hex)
            .map_err(|e| CryptoError::InvalidFormat(e.to_string()))?;
        let iv = hex::decode(&envelope.iv_hex)
            .map_err(|e| CryptoError::InvalidFormat(e.to_string()))?;
        let ciphertext = hex::decode(&envelope.ciphertext_hex)
            .map_err(|e| CryptoError::InvalidFormat(e.to_string()))?;

        let key = Self::derive_backup_key(pin, &salt)?;
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| CryptoError::Decryption(e.to_string()))?;
        let nonce = Nonce::from_slice(&iv);

        let plaintext = cipher
            .decrypt(nonce, ciphertext.as_slice())
            .map_err(|_| CryptoError::Decryption("PIN backup salah atau file korup".into()))?;

        let payload: BackupPayload = serde_json::from_slice(&plaintext)
            .map_err(|e| CryptoError::InvalidFormat(e.to_string()))?;

        Ok(payload)
    }
}
