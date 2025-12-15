use aes_gcm::{Aes256Gcm, Nonce};
use aes_gcm::aead::{Aead, KeyInit};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use rand::Rng;

type HmacSha256 = Hmac<Sha256>;

pub fn encrypt_chunk(key: &[u8], plaintext: &[u8]) -> Vec<u8> {
    let cipher = Aes256Gcm::new_from_slice(key).unwrap();
    let nonce_bytes: [u8; 12] = rand::thread_rng().gen();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, plaintext).unwrap();

    // HMAC
    let mut mac = HmacSha256::new_from_slice(key).unwrap();
    mac.update(&ciphertext);
    let tag = mac.finalize().into_bytes();

    [nonce_bytes.to_vec(), ciphertext, tag.to_vec()].concat()
}

pub fn decrypt_chunk(key: &[u8], data: &[u8]) -> Result<Vec<u8>, &'static str> {
    if data.len() < 12 + 32 { return Err("Chunk too small"); }

    let cipher = Aes256Gcm::new_from_slice(key).unwrap();
    let nonce = Nonce::from_slice(&data[..12]);
    let ciphertext = &data[12..data.len()-32];
    let tag = &data[data.len()-32..];

    // Verify HMAC
    let mut mac = HmacSha256::new_from_slice(key).unwrap();
    mac.update(ciphertext);
    mac.verify_slice(tag).map_err(|_| "HMAC verification failed")?;

    cipher.decrypt(nonce, ciphertext).map_err(|_| "Decryption failed")
}
