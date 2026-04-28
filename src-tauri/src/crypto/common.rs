use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use ring::digest::{digest, SHA256};
use ring::rand::{SecureRandom, SystemRandom};
use std::sync::atomic::{AtomicBool, AtomicU16};
use std::sync::Mutex;

use serde_json;

pub static SHARED_KEY: Mutex<String> = Mutex::new(String::new());
pub static SESSION_NEW_KEY: Mutex<String> = Mutex::new(String::new());
pub static SERVER_RUNNING: AtomicBool = AtomicBool::new(false);
pub static SERVER_PORT: AtomicU16 = AtomicU16::new(8080);

pub fn decrypt_api_data(data: &str) -> Option<String> {
    let new_key = SESSION_NEW_KEY.lock().unwrap();
    if new_key.is_empty() {
        return None;
    }
    let key = &*new_key;

    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data).ok()?;
    const NONCE_SIZE: usize = 12;
    if bytes.len() < NONCE_SIZE + 16 {
        return None;
    }

    let nonce = Nonce::from_slice(&bytes[..NONCE_SIZE]);
    let ciphertext = &bytes[NONCE_SIZE..];
    let key_hash = digest(&SHA256, key.as_bytes());
    let cipher = Aes256Gcm::new_from_slice(key_hash.as_ref()).ok()?;

    let plaintext = cipher.decrypt(nonce, ciphertext).ok()?;

    let decoded: Vec<String> = rmp_serde::from_slice(&plaintext).ok()?;
    if decoded.len() >= 2 {
        Some(decoded[1].clone())
    } else {
        None
    }
}

pub fn encrypt_api_response(response_data: &str) -> Option<String> {
    let new_key = SESSION_NEW_KEY.lock().unwrap();
    if new_key.is_empty() {
        return None;
    }
    let key = &*new_key;

    let key_hash = digest(&SHA256, key.as_bytes());

    let cipher = Aes256Gcm::new_from_slice(key_hash.as_ref()).ok()?;

    let rng = SystemRandom::new();
    let mut nonce_bytes = [0u8; 12];
    rng.fill(&mut nonce_bytes).ok()?;

    let salt = generate_random_string(random_len(49, 87));

    let payload_buf = rmp_serde::to_vec(&(salt, response_data)).ok()?;

    let nonce_iv = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce_iv, payload_buf.as_ref()).ok()?;

    let mut combined = Vec::with_capacity(12 + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);

    let encrypted = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &combined);

    let response_json = serde_json::to_string(&serde_json::json!({"data": encrypted})).unwrap();
    Some(response_json)
}

pub fn encrypt_api_binary_response_simple(binary_data: &[u8]) -> Option<Vec<u8>> {
    let new_key = SESSION_NEW_KEY.lock().unwrap();
    if new_key.is_empty() {
        return None;
    }
    let key = &*new_key;

    let key_hash = digest(&SHA256, key.as_bytes());

    let cipher = Aes256Gcm::new_from_slice(key_hash.as_ref()).ok()?;

    let rng = SystemRandom::new();
    let mut nonce_bytes = [0u8; 12];
    rng.fill(&mut nonce_bytes).ok()?;

    let nonce_iv = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce_iv, binary_data).ok()?;

    let mut combined = Vec::with_capacity(12 + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);

    Some(combined)
}

pub fn encrypt_api_binary_response(metadata_json: &str, binary_data: &[u8]) -> Option<Vec<u8>> {
    let new_key = SESSION_NEW_KEY.lock().unwrap();
    if new_key.is_empty() {
        return None;
    }
    let key = &*new_key;

    let key_hash = digest(&SHA256, key.as_bytes());

    let cipher = Aes256Gcm::new_from_slice(key_hash.as_ref()).ok()?;

    let rng = SystemRandom::new();
    let mut nonce_bytes = [0u8; 12];
    rng.fill(&mut nonce_bytes).ok()?;

    let salt = generate_random_string(random_len(49, 87));

    let payload_buf = rmp_serde::to_vec(&(salt, metadata_json, binary_data)).ok()?;

    let nonce_iv = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce_iv, payload_buf.as_ref()).ok()?;

    let mut combined = Vec::with_capacity(12 + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);

    Some(combined)
}

pub fn generate_random_string(len: usize) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut result = String::with_capacity(len);
    let rng = SystemRandom::new();
    for _ in 0..len {
        let mut buf = [0u8; 1];
        rng.fill(&mut buf).unwrap();
        let idx = buf[0] as usize % CHARSET.len();
        result.push(CHARSET[idx] as char);
    }
    result
}

pub fn random_len(min: usize, max: usize) -> usize {
    let rng = SystemRandom::new();
    let mut buf = [0u8; 1];
    rng.fill(&mut buf).unwrap();
    min + (buf[0] as usize % (max - min + 1))
}

#[tauri::command]
pub fn get_shared_key() -> Option<String> {
    let key = SHARED_KEY.lock().unwrap();
    if key.is_empty() {
        None
    } else {
        Some(key.clone())
    }
}

#[tauri::command]
pub fn get_session_new_key() -> Option<String> {
    let key = SESSION_NEW_KEY.lock().unwrap();
    if key.is_empty() {
        None
    } else {
        Some(key.clone())
    }
}

#[tauri::command]
pub fn set_random_shared_alphanumeric_key() -> String {
    const CHARS: &[u8] = b"ABCDEFGHJKLMNOPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz0123456789";
    let mut key = String::with_capacity(10);
    let rng = SystemRandom::new();
    for _ in 0..10 {
        let idx = {
            let mut buf = [0u8; 1];
            rng.fill(&mut buf).unwrap();
            buf[0] as usize % CHARS.len()
        };
        key.push(CHARS[idx] as char);
    }
    eprintln!("DEBUG get_shared_key: accessed, value = {}", key);
    *SHARED_KEY.lock().unwrap() = key.clone();
    eprintln!(
        "DEBUG SHARED_KEY now contains: {}",
        *SHARED_KEY.lock().unwrap()
    );
    key
}
