use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::sync::RwLock;
use zeroize::Zeroizing;

#[derive(Error, Debug)]
pub enum VaultError {
    #[error("PIN must be at least 6 characters")]
    PinTooShort,
    #[error("Vault already initialized")]
    AlreadyInitialized,
    #[error("Vault not initialized")]
    NotInitialized,
    #[error("Invalid PIN")]
    InvalidPin,
    #[error("Vault is locked")]
    Locked,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Crypto error: {0}")]
    Crypto(String),
    #[error("Terlalu banyak percobaan PIN. Coba lagi dalam {0} detik.")]
    TooManyAttempts(u64),
}

/// Consecutive wrong PINs before logins are refused for a while.
pub const MAX_FAILED_ATTEMPTS: u32 = 5;
const FIRST_LOCKOUT_SECS: u64 = 30;
const MAX_LOCKOUT_SECS: u64 = 15 * 60;

fn lockout_secs(failed: u32) -> u64 {
    let doublings = failed.saturating_sub(MAX_FAILED_ATTEMPTS).min(16);
    (FIRST_LOCKOUT_SECS << doublings).min(MAX_LOCKOUT_SECS)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VaultData {
    pub pin_hash: String,
    pub salt_b64: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AuthStatus {
    pub unlocked: bool,
    pub session_valid: bool,
    pub initialized: bool,
    pub must_change_pin: bool,
}

pub struct VaultInner {
    pub path: PathBuf,
    pub data: Option<VaultData>,
    pub kek: Option<Zeroizing<[u8; 32]>>,
    pub sessions: HashSet<String>,
    pub failed_attempts: u32,
    pub locked_until: Option<Instant>,
}

#[derive(Clone)]
pub struct Vault {
    inner: Arc<RwLock<VaultInner>>,
    /// One PIN check at a time, so parallel guesses cannot slip past the lockout.
    login_gate: Arc<tokio::sync::Mutex<()>>,
}

impl Vault {
    pub async fn new(path: &Path) -> Result<Self, VaultError> {
        let data = if path.exists() {
            let content = tokio::fs::read_to_string(path).await?;
            Some(serde_json::from_str::<VaultData>(&content)?)
        } else {
            None
        };

        Ok(Self {
            inner: Arc::new(RwLock::new(VaultInner {
                path: path.to_path_buf(),
                data,
                kek: None,
                sessions: HashSet::new(),
                failed_attempts: 0,
                locked_until: None,
            })),
            login_gate: Arc::new(tokio::sync::Mutex::new(())),
        })
    }

    pub async fn status(&self, session_token: &str) -> AuthStatus {
        let guard = self.inner.read().await;
        let initialized = guard.data.is_some();
        let unlocked = guard.kek.is_some();
        let session_valid = !session_token.is_empty() && guard.sessions.contains(session_token);

        AuthStatus {
            unlocked,
            session_valid,
            initialized,
            must_change_pin: false,
        }
    }

    pub async fn setup_pin(&self, pin: &str) -> Result<String, VaultError> {
        if pin.len() < 6 {
            return Err(VaultError::PinTooShort);
        }

        let mut guard = self.inner.write().await;
        if guard.data.is_some() {
            return Err(VaultError::AlreadyInitialized);
        }

        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let pin_hash = argon2
            .hash_password(pin.as_bytes(), &salt)
            .map_err(|e| VaultError::Crypto(e.to_string()))?
            .to_string();

        let mut kek_salt = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut kek_salt);
        let salt_b64 = BASE64.encode(kek_salt);

        let data = VaultData {
            pin_hash,
            salt_b64,
        };

        if let Some(parent) = guard.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let json = serde_json::to_string_pretty(&data)?;
        tokio::fs::write(&guard.path, json).await?;

        let kek = Self::derive_kek(pin, &kek_salt)?;
        guard.kek = Some(Zeroizing::new(kek));
        guard.data = Some(data);

        let mut token_bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut token_bytes);
        let session = token_bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>();
        guard.sessions.insert(session.clone());

        Ok(session)
    }

    pub async fn login(&self, pin: &str) -> Result<String, VaultError> {
        self.login_at(pin, Instant::now()).await
    }

    /// `login` with an explicit clock, for the lockout rules.
    pub async fn login_at(&self, pin: &str, now: Instant) -> Result<String, VaultError> {
        let _gate = self.login_gate.lock().await;
        let data = {
            let guard = self.inner.read().await;
            if let Some(until) = guard.locked_until {
                if now < until {
                    let left = until.duration_since(now);
                    let secs = left.as_secs() + u64::from(left.subsec_nanos() > 0);
                    return Err(VaultError::TooManyAttempts(secs.max(1)));
                }
            }
            guard.data.clone().ok_or(VaultError::NotInitialized)?
        };

        // Argon2 is slow on purpose: run it off the async workers and without holding the
        // vault lock, so other requests keep flowing while a PIN is checked.
        let pin_owned = Zeroizing::new(pin.to_string());
        let kek = tokio::task::spawn_blocking(move || -> Result<Option<[u8; 32]>, VaultError> {
            let parsed_hash =
                PasswordHash::new(&data.pin_hash).map_err(|e| VaultError::Crypto(e.to_string()))?;
            if Argon2::default()
                .verify_password(pin_owned.as_bytes(), &parsed_hash)
                .is_err()
            {
                return Ok(None);
            }
            let kek_salt = BASE64
                .decode(&data.salt_b64)
                .map_err(|e| VaultError::Crypto(e.to_string()))?;
            Self::derive_kek(&pin_owned, &kek_salt).map(Some)
        })
        .await
        .map_err(|e| VaultError::Crypto(e.to_string()))??;

        let mut guard = self.inner.write().await;
        let Some(kek) = kek else {
            guard.failed_attempts += 1;
            if guard.failed_attempts >= MAX_FAILED_ATTEMPTS {
                let secs = lockout_secs(guard.failed_attempts);
                guard.locked_until = Some(now + Duration::from_secs(secs));
            }
            return Err(VaultError::InvalidPin);
        };
        guard.failed_attempts = 0;
        guard.locked_until = None;
        guard.kek = Some(Zeroizing::new(kek));

        let mut token_bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut token_bytes);
        let session = token_bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>();
        guard.sessions.insert(session.clone());
        Ok(session)
    }

    pub async fn change_pin(
        &self,
        old_pin: &str,
        new_pin: &str,
        db: &crate::models::db::DbStore,
    ) -> Result<(), VaultError> {
        if new_pin.len() < 6 {
            return Err(VaultError::PinTooShort);
        }

        let mut guard = self.inner.write().await;
        let data = guard.data.as_ref().ok_or(VaultError::NotInitialized)?;

        let parsed_hash = PasswordHash::new(&data.pin_hash)
            .map_err(|e| VaultError::Crypto(e.to_string()))?;

        Argon2::default()
            .verify_password(old_pin.as_bytes(), &parsed_hash)
            .map_err(|_| VaultError::InvalidPin)?;

        let old_kek_salt = BASE64
            .decode(&data.salt_b64)
            .map_err(|e| VaultError::Crypto(e.to_string()))?;
        let old_kek = Self::derive_kek(old_pin, &old_kek_salt)?;

        let mut new_kek_salt = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut new_kek_salt);
        let new_salt_b64 = BASE64.encode(new_kek_salt);
        let new_kek = Self::derive_kek(new_pin, &new_kek_salt)?;

        // Rotate DB credentials with atomic transaction
        db.rotate_kek(&old_kek, &new_kek)
            .map_err(|e| VaultError::Crypto(e.to_string()))?;

        let salt = SaltString::generate(&mut OsRng);
        let pin_hash = Argon2::default()
            .hash_password(new_pin.as_bytes(), &salt)
            .map_err(|e| VaultError::Crypto(e.to_string()))?
            .to_string();

        let new_data = VaultData {
            pin_hash,
            salt_b64: new_salt_b64,
        };

        let json = serde_json::to_string_pretty(&new_data)?;
        tokio::fs::write(&guard.path, json).await?;

        guard.data = Some(new_data);
        guard.kek = Some(Zeroizing::new(new_kek));

        Ok(())
    }

    pub async fn lock(&self) {
        let mut guard = self.inner.write().await;
        guard.kek = None;
        guard.sessions.clear();
    }

    pub async fn get_kek(&self) -> Result<Zeroizing<[u8; 32]>, VaultError> {
        let guard = self.inner.read().await;
        guard.kek.clone().ok_or(VaultError::Locked)
    }

    pub async fn is_session_valid(&self, token: &str) -> bool {
        let guard = self.inner.read().await;
        !token.is_empty() && guard.sessions.contains(token)
    }

    fn derive_kek(pin: &str, salt: &[u8]) -> Result<[u8; 32], VaultError> {
        let mut output = [0u8; 32];
        Argon2::default()
            .hash_password_into(pin.as_bytes(), salt, &mut output)
            .map_err(|e| VaultError::Crypto(e.to_string()))?;
        Ok(output)
    }
}
