use aes_gcm::{Aes256Gcm, Nonce};
use aes_gcm::aead::{Aead, KeyInit};
// Change 'Mac' to 'KeyInit as MacKeyInit' to avoid naming conflicts if necessary, 
// but fully qualifying the call below is cleaner.
use hmac::{Hmac, Mac}; 
use sha2::Sha256;
use rand::Rng;

type HmacSha256 = Hmac<Sha256>;

pub fn encrypt_chunk(key: &[u8], plaintext: &[u8]) -> Vec<u8> {
    let cipher = Aes256Gcm::new_from_slice(key).unwrap();
    let mut rng = rand::thread_rng();
    let nonce_bytes: [u8; 12] = rng.gen();
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    // Encrypt using AES-256-GCM
    let ciphertext = cipher.encrypt(nonce, plaintext).expect("Encryption failure");

    // Fix: Use fully qualified syntax to tell Rust to use the Hmac implementation
    let mut mac = <HmacSha256 as hmac::KeyInit>::new_from_slice(key)
        .expect("HMAC key error");
    
    mac.update(&ciphertext);
    let tag = mac.finalize().into_bytes();

    // Layout: [Nonce (12)] + [Ciphertext (Varies)] + [HMAC Tag (32)]
    let mut result = Vec::with_capacity(12 + ciphertext.len() + 32);
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);
    result.extend_from_slice(&tag);
    result
}

pub fn decrypt_chunk(key: &[u8], data: &[u8]) -> Result<Vec<u8>, &'static str> {
    // 12 (nonce) + 32 (tag) = 44 minimum
    if data.len() < 44 { return Err("Chunk too small"); }

    let nonce_bytes = &data[..12];
    let tag_start = data.len() - 32;
    let ciphertext = &data[12..tag_start];
    let tag = &data[tag_start..];

    // Fix: Use fully qualified syntax here as well
    let mut mac = <HmacSha256 as hmac::KeyInit>::new_from_slice(key)
        .map_err(|_| "Invalid HMAC key")?;
    
    mac.update(ciphertext);
    mac.verify_slice(tag).map_err(|_| "HMAC verification failed")?;

    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| "Invalid AES key")?;
    let nonce = Nonce::from_slice(nonce_bytes);

    cipher.decrypt(nonce, ciphertext).map_err(|_| "Decryption failed")
}
