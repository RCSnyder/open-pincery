//! AC-38 (v7): AES-256-GCM vault round-trip, tamper, and key-separation
//! tests. Pure unit tests — no DB, no tokio required.

use base64::Engine;
use open_pincery::runtime::vault::{SealedCredential, Vault, VaultError};
use uuid::Uuid;

fn fresh_key_b64() -> String {
    use aes_gcm::aead::rand_core::RngCore;
    use aes_gcm::aead::OsRng;
    let mut k = [0u8; 32];
    OsRng.fill_bytes(&mut k);
    base64::engine::general_purpose::STANDARD.encode(k)
}

#[test]
fn roundtrip_100x_random_values() {
    let key = fresh_key_b64();
    let v = Vault::from_base64(&key).expect("load key");
    let ws = Uuid::new_v4();

    for i in 0..100 {
        let name = format!("cred_{i:03}");
        let plaintext = format!("secret-value-{i}-{}", Uuid::new_v4());
        let sealed = v
            .seal(ws, &name, plaintext.as_bytes())
            .expect("seal must succeed");
        // Every seal produces a fresh 12-byte nonce.
        assert_eq!(sealed.nonce.len(), 12);
        // AES-GCM tag is 16 bytes; plaintext is nonempty so ciphertext > 16.
        assert!(sealed.ciphertext.len() > 16);

        let opened = v.open(ws, &name, &sealed).expect("open must succeed");
        assert_eq!(opened, plaintext.as_bytes());
    }
}

#[test]
fn fresh_nonce_per_seal_same_plaintext() {
    let v = Vault::from_base64(&fresh_key_b64()).unwrap();
    let ws = Uuid::new_v4();
    let a = v.seal(ws, "api_key", b"same-value").unwrap();
    let b = v.seal(ws, "api_key", b"same-value").unwrap();
    // Distinct nonces ⇒ distinct ciphertexts even for identical plaintext.
    assert_ne!(a.nonce, b.nonce, "nonce must be freshly generated per seal");
    assert_ne!(
        a.ciphertext, b.ciphertext,
        "ciphertext must differ when nonces differ"
    );
}

#[test]
fn tampered_ciphertext_fails_authentication() {
    let v = Vault::from_base64(&fresh_key_b64()).unwrap();
    let ws = Uuid::new_v4();
    let mut sealed = v.seal(ws, "token", b"hunter2").unwrap();
    // Flip a bit in the ciphertext.
    sealed.ciphertext[0] ^= 0x01;
    let err = v.open(ws, "token", &sealed).unwrap_err();
    assert!(matches!(err, VaultError::Authentication));
}

#[test]
fn tampered_nonce_fails_authentication() {
    let v = Vault::from_base64(&fresh_key_b64()).unwrap();
    let ws = Uuid::new_v4();
    let mut sealed = v.seal(ws, "token", b"hunter2").unwrap();
    sealed.nonce[0] ^= 0x01;
    let err = v.open(ws, "token", &sealed).unwrap_err();
    assert!(matches!(err, VaultError::Authentication));
}

#[test]
fn wrong_workspace_id_fails_authentication() {
    let v = Vault::from_base64(&fresh_key_b64()).unwrap();
    let ws_a = Uuid::new_v4();
    let ws_b = Uuid::new_v4();
    let sealed = v.seal(ws_a, "shared_name", b"secret").unwrap();
    // Same name, different workspace — AAD mismatch.
    let err = v.open(ws_b, "shared_name", &sealed).unwrap_err();
    assert!(matches!(err, VaultError::Authentication));
}

#[test]
fn wrong_name_fails_authentication() {
    let v = Vault::from_base64(&fresh_key_b64()).unwrap();
    let ws = Uuid::new_v4();
    let sealed = v.seal(ws, "stripe", b"sk_live_xxx").unwrap();
    let err = v.open(ws, "stripe_other", &sealed).unwrap_err();
    assert!(matches!(err, VaultError::Authentication));
}

#[test]
fn wrong_key_fails_authentication() {
    let v1 = Vault::from_base64(&fresh_key_b64()).unwrap();
    let v2 = Vault::from_base64(&fresh_key_b64()).unwrap();
    let ws = Uuid::new_v4();
    let sealed = v1.seal(ws, "api", b"value").unwrap();
    let err = v2.open(ws, "api", &sealed).unwrap_err();
    assert!(matches!(err, VaultError::Authentication));
}

#[test]
fn invalid_base64_rejected() {
    let err = Vault::from_base64("!!!not-base64!!!").unwrap_err();
    assert!(matches!(err, VaultError::InvalidKey(_)));
}

#[test]
fn wrong_length_key_rejected() {
    // 16 bytes instead of 32.
    let short = base64::engine::general_purpose::STANDARD.encode([0u8; 16]);
    let err = Vault::from_base64(&short).unwrap_err();
    match err {
        VaultError::InvalidKey(msg) => assert!(msg.contains("32")),
        other => panic!("expected InvalidKey, got {other:?}"),
    }

    // 64 bytes instead of 32.
    let long = base64::engine::general_purpose::STANDARD.encode([0u8; 64]);
    let err = Vault::from_base64(&long).unwrap_err();
    assert!(matches!(err, VaultError::InvalidKey(_)));
}

#[test]
fn empty_plaintext_is_sealable_and_roundtrips() {
    let v = Vault::from_base64(&fresh_key_b64()).unwrap();
    let ws = Uuid::new_v4();
    let sealed: SealedCredential = v.seal(ws, "empty", b"").unwrap();
    // Even with empty plaintext the 16-byte tag is present.
    assert_eq!(sealed.ciphertext.len(), 16);
    let opened = v.open(ws, "empty", &sealed).unwrap();
    assert_eq!(opened, b"");
}
