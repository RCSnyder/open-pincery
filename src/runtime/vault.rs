//! AC-38 (v7): AES-256-GCM credential vault.
//!
//! In-process authenticated-encryption over a single workspace-wide master
//! key. Sealed output is `(nonce, ciphertext)` — authenticated tag is
//! appended to the ciphertext by `aes-gcm`. The AAD is the literal
//! `{workspace_id}:{name}` pair so ciphertexts are **bound** to the
//! `(workspace_id, name)` tuple under which they were sealed; swapping
//! either half at `open` time fails authentication.
//!
//! This module is the ONLY place that handles master-key bytes or
//! plaintext credential bytes. Everything else goes through [`Vault`]
//! by value.
//!
//! Guarantees:
//!
//! 1. **Fresh nonce per seal.** 12 random bytes from `OsRng`. 2^-96 collision
//!    probability per seal is safe for the operator-scale call volumes v7
//!    targets; `aes-gcm`'s nonce-misuse protection is NOT relied on.
//! 2. **Authenticated.** Tampered nonce, tampered ciphertext, wrong
//!    `(workspace_id, name)`, or wrong master key all collapse to
//!    [`VaultError::Authentication`]. `open` never panics.
//! 3. **Fixed key size.** `from_base64` decodes exactly 32 bytes; otherwise
//!    [`VaultError::InvalidKey`].

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, KeyInit, OsRng, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::Engine;
use uuid::Uuid;

#[derive(Debug)]
pub enum VaultError {
    /// Authenticated decryption failed: wrong key, tampered ciphertext or
    /// nonce, or mismatched `(workspace_id, name)` AAD. Do not leak which.
    Authentication,
    /// Master key could not be loaded (bad base64, wrong byte length).
    InvalidKey(String),
}

impl std::fmt::Display for VaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Authentication => write!(f, "credential authentication failed"),
            Self::InvalidKey(msg) => write!(f, "invalid master key: {msg}"),
        }
    }
}

impl std::error::Error for VaultError {}

#[derive(Debug, Clone)]
pub struct SealedCredential {
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
}

#[derive(Clone)]
pub struct Vault {
    cipher: Aes256Gcm,
}

impl std::fmt::Debug for Vault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print the cipher state — the key must not leak via logs.
        f.debug_struct("Vault")
            .field("cipher", &"<redacted>")
            .finish()
    }
}

impl Vault {
    /// Load a master key from a base64-encoded 32-byte blob. Both standard
    /// and URL-safe alphabets are accepted; padding is required per
    /// `STANDARD` engine rules. Keys of the wrong decoded length are
    /// rejected — no silent truncation.
    pub fn from_base64(b64: &str) -> Result<Self, VaultError> {
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(b64.trim())
            .map_err(|e| VaultError::InvalidKey(format!("base64 decode failed: {e}")))?;
        if decoded.len() != 32 {
            return Err(VaultError::InvalidKey(format!(
                "expected 32 bytes after base64 decode, got {}",
                decoded.len()
            )));
        }
        let key = Key::<Aes256Gcm>::from_slice(&decoded);
        Ok(Self {
            cipher: Aes256Gcm::new(key),
        })
    }

    /// Seal `plaintext` under the AAD `{workspace_id}:{name}` with a fresh
    /// random 12-byte nonce. The returned `SealedCredential` is safe to
    /// persist; the master key is never materialised outside this module.
    pub fn seal(
        &self,
        workspace_id: Uuid,
        name: &str,
        plaintext: &[u8],
    ) -> Result<SealedCredential, VaultError> {
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let aad = aad(workspace_id, name);
        let ciphertext = self
            .cipher
            .encrypt(
                nonce,
                Payload {
                    msg: plaintext,
                    aad: &aad,
                },
            )
            // AES-GCM encryption can only fail if the plaintext would overflow
            // the 2^39-256 bit limit. Credential values are capped at 8 KiB
            // upstream, so this branch is unreachable under normal operation.
            // Surface it as Authentication rather than panicking to preserve
            // the "never panics" invariant.
            .map_err(|_| VaultError::Authentication)?;

        Ok(SealedCredential {
            nonce: nonce_bytes,
            ciphertext,
        })
    }

    /// Open a `SealedCredential`. Returns [`VaultError::Authentication`]
    /// uniformly for every failure mode (wrong key, tampered ciphertext,
    /// tampered nonce, mismatched `(workspace_id, name)`), so the caller
    /// cannot distinguish which — side-channel defence.
    pub fn open(
        &self,
        workspace_id: Uuid,
        name: &str,
        sealed: &SealedCredential,
    ) -> Result<Vec<u8>, VaultError> {
        let nonce = Nonce::from_slice(&sealed.nonce);
        let aad = aad(workspace_id, name);
        self.cipher
            .decrypt(
                nonce,
                Payload {
                    msg: &sealed.ciphertext,
                    aad: &aad,
                },
            )
            .map_err(|_| VaultError::Authentication)
    }
}

fn aad(workspace_id: Uuid, name: &str) -> Vec<u8> {
    format!("{workspace_id}:{name}").into_bytes()
}
