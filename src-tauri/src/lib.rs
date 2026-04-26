// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rcgen::{Certificate, CertificateParams, DistinguishedName, IsCa, KeyPair};
use ring::digest::{digest, SHA256};
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::Mutex;
use std::thread;
use time::OffsetDateTime;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct GreetResponse {
    message: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct TlsCertificates {
    ca_cert: String,
    ca_key: String,
    domain: String,
    domain_cert: String,
    domain_key: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct FileItem {
    name: String,
    path: String,
    is_dir: bool,
    size: u64,
    modified: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct SystemInfo {
    os: String,
    arch: String,
    hostname: String,
    cpu_count: usize,
    total_memory: u64,
    free_memory: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ProcessItem {
    pid: u32,
    name: String,
    cpu: f32,
    memory: u64,
}

static SERVER_RUNNING: AtomicBool = AtomicBool::new(false);
static SERVER_PORT: AtomicU16 = AtomicU16::new(8080);
static SHARED_KEY: Mutex<String> = Mutex::new(String::new());
static SESSION_NEW_KEY: Mutex<String> = Mutex::new(String::new());

#[tauri::command]
fn get_binary_file(path: String) -> Result<Vec<u8>, String> {
    if path.contains("..") {
        return Err("Forbidden".to_string());
    }
    fs::read(&path).map_err(|e| format!("Cannot read file: {}", e))
}

#[tauri::command]
fn get_binary_mime(path: String) -> String {
    if path.ends_with(".png") {
        "image/png".to_string()
    } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        "image/jpeg".to_string()
    } else if path.ends_with(".gif") {
        "image/gif".to_string()
    } else if path.ends_with(".webp") {
        "image/webp".to_string()
    } else if path.ends_with(".svg") {
        "image/svg+xml".to_string()
    } else if path.ends_with(".bmp") {
        "image/bmp".to_string()
    } else if path.ends_with(".avif") {
        "image/avif".to_string()
    } else {
        "application/octet-stream".to_string()
    }
}

#[tauri::command]
fn greet_json(name: &str) -> GreetResponse {
    GreetResponse {
        message: format!("Hello, {}! (from Tauri Rust backend)", name),
    }
}

fn generate_ca() -> Result<(Certificate, String, String), String> {
    let mut params = CertificateParams::default();
    params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "Local Rust CA");
    params.not_before = OffsetDateTime::now_utc();
    params.not_after = params.not_before + time::Duration::days(365);

    let key_pair = KeyPair::generate().map_err(|e| e.to_string())?;
    let ca_cert = params.self_signed(&key_pair).map_err(|e| e.to_string())?;
    let ca_pem = ca_cert.pem();
    let ca_key = key_pair.serialize_pem();

    Ok((ca_cert, ca_pem, ca_key))
}

fn generate_domain_cert(
    ca_cert: &Certificate,
    ca_key: &KeyPair,
    domain: &str,
) -> Result<(String, String), String> {
    let mut params = CertificateParams::new(vec![domain.to_string()]).map_err(|e| e.to_string())?;
    params.distinguished_name = DistinguishedName::new();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, domain);
    params.not_before = OffsetDateTime::now_utc();
    params.not_after = params.not_before + time::Duration::days(365);

    let key_pair = KeyPair::generate().map_err(|e| e.to_string())?;
    let cert = params
        .signed_by(&key_pair, ca_cert, ca_key)
        .map_err(|e| e.to_string())?;
    let pem = cert.pem();
    let key = key_pair.serialize_pem();

    Ok((pem, key))
}

fn determine_mime(path: &str) -> &'static str {
    match path
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "html" => "text/html",
        "js" => "application/javascript",
        "mjs" => "application/javascript",
        "css" => "text/css",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "wasm" => "application/wasm",
        "txt" => "text/plain",
        _ => "application/octet-stream",
    }
}

fn decrypt_api_data(data: &str) -> Option<String> {
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

fn encrypt_api_response(response_data: &str) -> Option<String> {
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

fn encrypt_api_binary_response_simple(binary_data: &[u8]) -> Option<Vec<u8>> {
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

fn encrypt_api_binary_response(metadata_json: &str, binary_data: &[u8]) -> Option<Vec<u8>> {
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

fn generate_random_string(len: usize) -> String {
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

fn random_len(min: usize, max: usize) -> usize {
    let rng = SystemRandom::new();
    let mut buf = [0u8; 1];
    rng.fill(&mut buf).unwrap();
    min + (buf[0] as usize % (max - min + 1))
}

fn serve_file(path: &str) -> Option<String> {
    let web_frontend_dist = std::env::current_dir().ok().and_then(|cwd| {
        let candidates = [
            cwd.join("web-frontend/dist"),
            cwd.join("../web-frontend/dist"),
        ];
        candidates.into_iter().find(|pb| pb.exists())
    })?;

    let fs_path = if path == "/" || path.is_empty() {
        web_frontend_dist.join("index.html")
    } else {
        let safe_path = path.trim_start_matches('/');
        if safe_path.contains("..") {
            return None;
        }
        web_frontend_dist.join(safe_path)
    };

    fs::read_to_string(&fs_path).ok()
}

fn handle_request(mut stream: TcpStream) {
    let mut buffer = [0; 8192];
    let n = match stream.read(&mut buffer) {
        Ok(n) if n > 0 => n,
        _ => return,
    };

    let request = String::from_utf8_lossy(&buffer[..n]);
    let lines: Vec<&str> = request.lines().collect();
    let (method, path, _version) = if lines.len() >= 1 {
        let parts: Vec<&str> = lines[0].split_whitespace().collect();
        (
            *parts.get(0).unwrap_or(&""),
            *parts.get(1).unwrap_or(&""),
            *parts.get(2).unwrap_or(&""),
        )
    } else {
        ("", "", "")
    };

    let req_path: &str = path;
    let body_start = request.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
    let body = if body_start < n {
        String::from_utf8_lossy(&buffer[body_start..n]).to_string()
    } else {
        String::new()
    };

    let query = req_path.split('?').nth(1).unwrap_or("");
    let clean_path = req_path.split('?').next().unwrap_or("/");

    fn parse_query_params(query: &str) -> std::collections::HashMap<String, String> {
        let mut params = std::collections::HashMap::new();
        for pair in query.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                params.insert(k.to_string(), urlencoding_decode(v));
            }
        }
        params
    }

    fn urlencoding_decode(s: &str) -> String {
        s.replace("+", " ")
            .split('%')
            .enumerate()
            .map(|(i, part)| {
                if i == 0 {
                    part.to_string()
                } else if part.len() >= 2 {
                    let hex = &part[..2];
                    let remainder = &part[2..];
                    if let Ok(byte) = u8::from_str_radix(hex, 16) {
                        format!("{}{}", (byte as char).to_string(), remainder)
                    } else {
                        format!("%{}", part)
                    }
                } else {
                    format!("%{}", part)
                }
            })
            .collect()
    }

    fn json_response(data: &str, status: &str) -> String {
        format!("HTTP/1.1 {} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{}", status, data.len(), data)
    }

    fn error_response(msg: &str, status: &str) -> String {
        let body = format!("{{\"error\":\"{}\"}}", msg);
        format!("HTTP/1.1 {} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{}", status, body.len(), body)
    }

    fn base64_encode(data: &[u8]) -> String {
        const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut result = String::new();
        for chunk in data.chunks(3) {
            let b = match chunk.len() {
                1 => [chunk[0], 0, 0],
                2 => [chunk[0], chunk[1], 0],
                _ => [chunk[0], chunk[1], chunk[2]],
            };
            result.push(CHARS[(b[0] >> 2) as usize] as char);
            result.push(CHARS[((b[0] & 0x03) << 4 | b[1] >> 4) as usize] as char);
            if chunk.len() > 1 {
                result.push(CHARS[((b[1] & 0x0f) << 2 | b[2] >> 6) as usize] as char);
            } else {
                result.push('=');
            }
            if chunk.len() > 2 {
                result.push(CHARS[(b[2] & 0x3f) as usize] as char);
            } else {
                result.push('=');
            }
        }
        result
    }

    let params = parse_query_params(query);
    let mut response = String::new();

    if method == "OPTIONS" {
        response = "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, DELETE, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nConnection: close\r\n\r\n".to_string();
    } else if method == "GET" && clean_path == "/api/health" {
        let ts = OffsetDateTime::now_utc().unix_timestamp();
        let body =
            serde_json::to_string(&serde_json::json!({"status": "ok", "timestamp": ts})).unwrap();
        response = json_response(&body, "200");
    } else if method == "POST" && clean_path == "/api/session/verify" {
        let key = SHARED_KEY.lock().unwrap();
        if key.is_empty() {
            response = error_response("No shared key set", "400");
        } else {
            #[derive(Deserialize)]
            struct SessionPayload {
                data: String,
            }
            if let Ok(payload) = serde_json::from_str::<SessionPayload>(&body) {
                match base64::Engine::decode(
                    &base64::engine::general_purpose::STANDARD,
                    &payload.data,
                ) {
                    Ok(bytes) => {
                        const NONCE_SIZE: usize = 12;
                        if bytes.len() < NONCE_SIZE + 16 {
                            response = error_response("Invalid ciphertext", "400");
                        } else {
                            use aes_gcm::{
                                aead::{Aead, KeyInit},
                                Aes256Gcm, Nonce,
                            };

                            let nonce = Nonce::from_slice(&bytes[..NONCE_SIZE]);
                            let ciphertext = &bytes[NONCE_SIZE..];
                            let key_hash = digest(&SHA256, key.as_bytes());
                            let cipher = Aes256Gcm::new_from_slice(key_hash.as_ref()).unwrap();

                            match cipher.decrypt(nonce, ciphertext) {
                                Ok(plaintext) => {
                                    let decoded = rmp_serde::from_slice::<Vec<String>>(&plaintext);
                                    match decoded {
                                        Ok(vec) if vec.len() >= 2 => {
                                            let new_key = vec[1].clone();
                                            *SESSION_NEW_KEY.lock().unwrap() = new_key;
                                            response = json_response(r#"{"valid":true}"#, "200");
                                        }
                                        _ => {
                                            response = error_response("Invalid payload", "400");
                                        }
                                    }
                                }
                                Err(_) => {
                                    eprintln!("DEBUG: Decryption failed. Expected key: {}", key);
                                    response = error_response("Decryption failed", "400");
                                }
                            }
                        }
                    }
                    Err(_) => response = error_response("Invalid base64", "400"),
                }
            } else {
                response = error_response("Invalid request body", "400");
            }
        }
    } else if method == "POST" && clean_path == "/api/session/decrypt" {
        let new_key = SESSION_NEW_KEY.lock().unwrap();
        if new_key.is_empty() {
            response = error_response("No session key", "400");
        } else {
            #[derive(Deserialize)]
            struct DecryptPayload {
                data: String,
            }
            if let Ok(payload) = serde_json::from_str::<DecryptPayload>(&body) {
                match base64::Engine::decode(
                    &base64::engine::general_purpose::STANDARD,
                    &payload.data,
                ) {
                    Ok(bytes) => {
                        const NONCE_SIZE: usize = 12;
                        if bytes.len() < NONCE_SIZE + 16 {
                            response = error_response("Invalid ciphertext", "400");
                        } else {
                            use aes_gcm::{
                                aead::{Aead, KeyInit},
                                Aes256Gcm, Nonce,
                            };

                            let nonce = Nonce::from_slice(&bytes[..NONCE_SIZE]);
                            let ciphertext = &bytes[NONCE_SIZE..];
                            let key_hash = digest(&SHA256, new_key.as_bytes());
                            let cipher = Aes256Gcm::new_from_slice(key_hash.as_ref()).unwrap();

                            match cipher.decrypt(nonce, ciphertext) {
                                Ok(plaintext) => {
                                    let decoded = rmp_serde::from_slice::<Vec<String>>(&plaintext);
                                    match decoded {
                                        Ok(vec) if vec.len() >= 2 => {
                                            let body = serde_json::to_string(
                                                &serde_json::json!({"payload": vec[1]}),
                                            )
                                            .unwrap();
                                            response = json_response(&body, "200");
                                        }
                                        _ => {
                                            response = error_response("Invalid payload", "400");
                                        }
                                    }
                                }
                                Err(_) => response = error_response("Decryption failed", "400"),
                            }
                        }
                    }
                    Err(_) => response = error_response("Invalid base64", "400"),
                }
            } else {
                response = error_response("Invalid request body", "400");
            }
        }
    } else if method == "POST" && clean_path == "/api/system/home" {
        #[derive(Deserialize)]
        struct HomePayload {
            data: String,
        }
        if let Ok(payload) = serde_json::from_str::<HomePayload>(&body) {
            if let Some(_decrypted) = decrypt_api_data(&payload.data) {
                let home = dirs::home_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| "/".to_string());
                let body_str = serde_json::to_string(&serde_json::json!({"path": home})).unwrap();
                if let Some(encrypted) = encrypt_api_response(&body_str) {
                    response = json_response(&encrypted, "200");
                } else {
                    response = error_response("Encryption failed", "500");
                }
            } else {
                response = error_response("Decryption failed", "400");
            }
        } else {
            response = error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/system/drives" {
        #[derive(Deserialize)]
        struct DrivesPayload {
            data: String,
        }
        if let Ok(payload) = serde_json::from_str::<DrivesPayload>(&body) {
            if let Some(_decrypted) = decrypt_api_data(&payload.data) {
                let mut items = Vec::new();
                #[cfg(unix)]
                {
                    items.push(serde_json::json!({"name": "/", "path": "/", "is_dir": true, "size": 0, "modified": null}));
                    if let Some(home) = dirs::home_dir() {
                        items.push(serde_json::json!({"name": "Home", "path": home.to_string_lossy().to_string(), "is_dir": true, "size": 0, "modified": null}));
                    }
                }
                let body_str = serde_json::to_string(&items).unwrap();
                if let Some(encrypted) = encrypt_api_response(&body_str) {
                    response = json_response(&encrypted, "200");
                } else {
                    response = error_response("Encryption failed", "500");
                }
            } else {
                response = error_response("Decryption failed", "400");
            }
        } else {
            response = error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/system/processes" {
        #[derive(Deserialize)]
        struct ProcessesPayload {
            data: String,
        }
        if let Ok(payload) = serde_json::from_str::<ProcessesPayload>(&body) {
            if let Some(_decrypted) = decrypt_api_data(&payload.data) {
                let sys = sysinfo::System::new_all();
                let mut processes = Vec::new();
                for (pid, process) in sys.processes() {
                    processes.push(serde_json::json!({
                        "pid": pid.as_u32(),
                        "name": process.name().to_string(),
                        "cpu": process.cpu_usage(),
                        "memory": process.memory()
                    }));
                }
                processes.sort_by(|a, b| {
                    let a_cpu = a.get("cpu").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let b_cpu = b.get("cpu").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    b_cpu
                        .partial_cmp(&a_cpu)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                let body_str = serde_json::to_string(&processes).unwrap();
                if let Some(encrypted) = encrypt_api_response(&body_str) {
                    response = json_response(&encrypted, "200");
                } else {
                    response = error_response("Encryption failed", "500");
                }
            } else {
                response = error_response("Decryption failed", "400");
            }
        } else {
            response = error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/files/list" {
        #[derive(Deserialize)]
        struct ListPayload {
            data: String,
        }
        if let Ok(payload) = serde_json::from_str::<ListPayload>(&body) {
            if let Some(decrypted) = decrypt_api_data(&payload.data) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&decrypted) {
                    let dir = parsed.get("dir").and_then(|v| v.as_str()).unwrap_or("/");
                    if dir.contains("..") {
                        response = error_response("Forbidden", "403");
                    } else {
                        match fs::read_dir(Path::new(dir)) {
                            Ok(entries) => {
                                let mut items = Vec::new();
                                for entry in entries.flatten() {
                                    let path = entry.path();
                                    let metadata = entry.metadata().ok();
                                    let name = path
                                        .file_name()
                                        .and_then(|n| n.to_str())
                                        .unwrap_or("unknown")
                                        .to_string();
                                    let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
                                    let modified = metadata
                                        .clone()
                                        .and_then(|m| m.modified().ok())
                                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                        .map(|d| d.as_secs() as i64);
                                    let is_dir =
                                        metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                                    items.push(serde_json::json!({"name": name, "path": path.to_string_lossy().to_string(), "is_dir": is_dir, "size": size, "modified": modified}));
                                }
                                items.sort_by(|a, b| {
                                    let a_dir =
                                        a.get("is_dir").and_then(|v| v.as_bool()).unwrap_or(false);
                                    let b_dir =
                                        b.get("is_dir").and_then(|v| v.as_bool()).unwrap_or(false);
                                    if a_dir != b_dir {
                                        return b_dir.cmp(&a_dir);
                                    }
                                    let a_name =
                                        a.get("name").and_then(|v| v.as_str()).unwrap_or("");
                                    let b_name =
                                        b.get("name").and_then(|v| v.as_str()).unwrap_or("");
                                    a_name.to_lowercase().cmp(&b_name.to_lowercase())
                                });
                                let body_str = serde_json::to_string(&items).unwrap();
                                if let Some(encrypted) = encrypt_api_response(&body_str) {
                                    response = json_response(&encrypted, "200");
                                } else {
                                    response = error_response("Encryption failed", "500");
                                }
                            }
                            Err(e) => {
                                response =
                                    error_response(&format!("Cannot read directory: {}", e), "500");
                            }
                        }
                    }
                } else {
                    response = error_response("Invalid decrypted data", "400");
                }
            } else {
                response = error_response("Decryption failed", "400");
            }
        } else {
            response = error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/files/info" {
        #[derive(Deserialize)]
        struct InfoPayload {
            data: String,
        }
        if let Ok(payload) = serde_json::from_str::<InfoPayload>(&body) {
            if let Some(decrypted) = decrypt_api_data(&payload.data) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&decrypted) {
                    let file_path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
                    if file_path.contains("..") {
                        response = error_response("Forbidden", "403");
                    } else {
                        match fs::metadata(file_path) {
                            Ok(metadata) => {
                                let name = Path::new(file_path)
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                let size = metadata.len();
                                let modified = metadata
                                    .modified()
                                    .ok()
                                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                    .map(|d| d.as_secs() as i64);
                                let is_dir = metadata.is_dir();
                                let body_str = serde_json::to_string(&serde_json::json!({"name": name, "path": file_path, "is_dir": is_dir, "size": size, "modified": modified})).unwrap();
                                if let Some(encrypted) = encrypt_api_response(&body_str) {
                                    response = json_response(&encrypted, "200");
                                } else {
                                    response = error_response("Encryption failed", "500");
                                }
                            }
                            Err(e) => {
                                response = error_response(&format!("Cannot get info: {}", e), "500")
                            }
                        }
                    }
                } else {
                    response = error_response("Invalid decrypted data", "400");
                }
            } else {
                response = error_response("Decryption failed", "400");
            }
        } else {
            response = error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/files/search" {
        #[derive(Deserialize)]
        struct SearchPayload {
            data: String,
        }
        if let Ok(payload) = serde_json::from_str::<SearchPayload>(&body) {
            if let Some(decrypted) = decrypt_api_data(&payload.data) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&decrypted) {
                    let dir = parsed.get("dir").and_then(|v| v.as_str()).unwrap_or("/");
                    let pattern = parsed.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
                    if dir.contains("..") {
                        response = error_response("Forbidden", "403");
                    } else {
                        let mut results = Vec::new();
                        let pattern_lower = pattern.to_lowercase();
                        fn walk_dir(dir: &Path, pat: &str, results: &mut Vec<serde_json::Value>) {
                            if let Ok(entries) = fs::read_dir(dir) {
                                for entry in entries.flatten() {
                                    let path = entry.path();
                                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                                        if name.to_lowercase().contains(pat) {
                                            let metadata = entry.metadata().ok();
                                            let size =
                                                metadata.as_ref().map(|m| m.len()).unwrap_or(0);
                                            let modified = metadata
                                                .clone()
                                                .and_then(|m| m.modified().ok())
                                                .and_then(|t| {
                                                    t.duration_since(std::time::UNIX_EPOCH).ok()
                                                })
                                                .map(|d| d.as_secs() as i64);
                                            let is_dir = metadata
                                                .as_ref()
                                                .map(|m| m.is_dir())
                                                .unwrap_or(false);
                                            results.push(serde_json::json!({"name": name, "path": path.to_string_lossy().to_string(), "is_dir": is_dir, "size": size, "modified": modified}));
                                        }
                                        if path.is_dir() {
                                            walk_dir(&path, pat, results);
                                        }
                                    }
                                }
                            }
                        }
                        walk_dir(Path::new(dir), &pattern_lower, &mut results);
                        let body_str = serde_json::to_string(&results).unwrap();
                        if let Some(encrypted) = encrypt_api_response(&body_str) {
                            response = json_response(&encrypted, "200");
                        } else {
                            response = error_response("Encryption failed", "500");
                        }
                    }
                } else {
                    response = error_response("Invalid decrypted data", "400");
                }
            } else {
                response = error_response("Decryption failed", "400");
            }
        } else {
            response = error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/files/read" {
        #[derive(Deserialize)]
        struct ReadPayload {
            data: String,
        }
        if let Ok(payload) = serde_json::from_str::<ReadPayload>(&body) {
            if let Some(decrypted) = decrypt_api_data(&payload.data) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&decrypted) {
                    let file_path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
                    if file_path.contains("..") {
                        response = error_response("Forbidden", "403");
                    } else {
                        match fs::read(file_path) {
                            Ok(bytes) => {
                                let mime = if file_path.ends_with(".png") {
                                    "image/png"
                                } else if file_path.ends_with(".jpg")
                                    || file_path.ends_with(".jpeg")
                                {
                                    "image/jpeg"
                                } else if file_path.ends_with(".gif") {
                                    "image/gif"
                                } else if file_path.ends_with(".webp") {
                                    "image/webp"
                                } else if file_path.ends_with(".svg") {
                                    "image/svg+xml"
                                } else if file_path.ends_with(".bmp") {
                                    "image/bmp"
                                } else if file_path.ends_with(".avif") {
                                    "image/avif"
                                } else {
                                    "application/octet-stream"
                                };
                                let b64 = base64_encode(&bytes);
                                let body_str = serde_json::to_string(
                                    &serde_json::json!({"content": b64, "mime": mime, "binary": true}),
                                )
                                .unwrap();
                                if let Some(encrypted) = encrypt_api_response(&body_str) {
                                    response = json_response(&encrypted, "200");
                                } else {
                                    response = error_response("Encryption failed", "500");
                                }
                            }
                            Err(e) => {
                                response =
                                    error_response(&format!("Cannot read file: {}", e), "500")
                            }
                        }
                    }
                } else {
                    response = error_response("Invalid decrypted data", "400");
                }
            } else {
                response = error_response("Decryption failed", "400");
            }
        } else {
            response = error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/files/binary" {
        #[derive(Deserialize)]
        struct BinaryPayload {
            data: String,
        }
        if let Ok(payload) = serde_json::from_str::<BinaryPayload>(&body) {
            if let Some(decrypted) = decrypt_api_data(&payload.data) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&decrypted) {
                    let file_path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
                    if file_path.contains("..") {
                        response = error_response("Forbidden", "403");
                    } else {
                        match fs::read(file_path) {
                            Ok(bytes) => {
                                if let Some(encrypted) = encrypt_api_binary_response_simple(&bytes)
                                {
                                    response = format!(
                                        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
                                        encrypted.len()
                                    );
                                    let mut final_response = response.into_bytes();
                                    final_response.extend(encrypted);
                                    let _ = stream.write(&final_response);
                                    let _ = stream.flush();
                                    return;
                                } else {
                                    response = error_response("Encryption failed", "500");
                                }
                            }
                            Err(e) => {
                                response =
                                    error_response(&format!("Cannot read file: {}", e), "500")
                            }
                        }
                    }
                } else {
                    response = error_response("Invalid decrypted data", "400");
                }
            } else {
                response = error_response("Decryption failed", "400");
            }
        } else {
            response = error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/files/delete" {
        #[derive(Deserialize)]
        struct DeletePayload {
            data: String,
        }
        if let Ok(payload) = serde_json::from_str::<DeletePayload>(&body) {
            if let Some(decrypted) = decrypt_api_data(&payload.data) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&decrypted) {
                    let file_path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
                    if file_path.contains("..") || file_path == "/" {
                        response = error_response("Forbidden", "403");
                    } else {
                        let p = Path::new(file_path);
                        let result = if p.is_dir() {
                            fs::remove_dir_all(p)
                        } else {
                            fs::remove_file(p)
                        };
                        match result {
                            Ok(_) => {
                                let body_str = "{\"success\":true}".to_string();
                                if let Some(encrypted) = encrypt_api_response(&body_str) {
                                    response = json_response(&encrypted, "200");
                                } else {
                                    response = error_response("Encryption failed", "500");
                                }
                            }
                            Err(e) => {
                                response = error_response(&format!("Cannot delete: {}", e), "500")
                            }
                        }
                    }
                } else {
                    response = error_response("Invalid decrypted data", "400");
                }
            } else {
                response = error_response("Decryption failed", "400");
            }
        } else {
            response = error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/files/write" {
        #[derive(Deserialize)]
        struct WritePayload {
            data: String,
        }
        if let Ok(payload) = serde_json::from_str::<WritePayload>(&body) {
            if let Some(decrypted) = decrypt_api_data(&payload.data) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&decrypted) {
                    let file_path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
                    let content = parsed.get("content").and_then(|v| v.as_str()).unwrap_or("");
                    if file_path.contains("..") {
                        response = error_response("Forbidden", "403");
                    } else {
                        match fs::write(file_path, content) {
                            Ok(_) => {
                                let body_str = "{\"success\":true}".to_string();
                                if let Some(encrypted) = encrypt_api_response(&body_str) {
                                    response = json_response(&encrypted, "200");
                                } else {
                                    response = error_response("Encryption failed", "500");
                                }
                            }
                            Err(e) => {
                                response =
                                    error_response(&format!("Cannot write file: {}", e), "500")
                            }
                        }
                    }
                } else {
                    response = error_response("Invalid decrypted data", "400");
                }
            } else {
                response = error_response("Decryption failed", "400");
            }
        } else {
            response = error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/files/create-dir" {
        #[derive(Deserialize)]
        struct CreateDirPayload {
            data: String,
        }
        if let Ok(payload) = serde_json::from_str::<CreateDirPayload>(&body) {
            if let Some(decrypted) = decrypt_api_data(&payload.data) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&decrypted) {
                    let dir_path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
                    if dir_path.contains("..") {
                        response = error_response("Forbidden", "403");
                    } else {
                        match fs::create_dir_all(dir_path) {
                            Ok(_) => {
                                let body_str = "{\"success\":true}".to_string();
                                if let Some(encrypted) = encrypt_api_response(&body_str) {
                                    response = json_response(&encrypted, "200");
                                } else {
                                    response = error_response("Encryption failed", "500");
                                }
                            }
                            Err(e) => {
                                response = error_response(
                                    &format!("Cannot create directory: {}", e),
                                    "500",
                                )
                            }
                        }
                    }
                } else {
                    response = error_response("Invalid decrypted data", "400");
                }
            } else {
                response = error_response("Decryption failed", "400");
            }
        } else {
            response = error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/files/rename" {
        #[derive(Deserialize)]
        struct RenamePayload {
            data: String,
        }
        if let Ok(payload) = serde_json::from_str::<RenamePayload>(&body) {
            if let Some(decrypted) = decrypt_api_data(&payload.data) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&decrypted) {
                    let old_path = parsed
                        .get("old_path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let new_path = parsed
                        .get("new_path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if old_path.contains("..") || new_path.contains("..") {
                        response = error_response("Forbidden", "403");
                    } else {
                        match fs::rename(old_path, new_path) {
                            Ok(_) => {
                                let body_str = "{\"success\":true}".to_string();
                                if let Some(encrypted) = encrypt_api_response(&body_str) {
                                    response = json_response(&encrypted, "200");
                                } else {
                                    response = error_response("Encryption failed", "500");
                                }
                            }
                            Err(e) => {
                                response = error_response(&format!("Cannot rename: {}", e), "500")
                            }
                        }
                    }
                } else {
                    response = error_response("Invalid decrypted data", "400");
                }
            } else {
                response = error_response("Decryption failed", "400");
            }
        } else {
            response = error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/files/copy" {
        #[derive(Deserialize)]
        struct CopyPayload {
            data: String,
        }
        if let Ok(payload) = serde_json::from_str::<CopyPayload>(&body) {
            if let Some(decrypted) = decrypt_api_data(&payload.data) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&decrypted) {
                    let source = parsed.get("source").and_then(|v| v.as_str()).unwrap_or("");
                    let dest = parsed.get("dest").and_then(|v| v.as_str()).unwrap_or("");
                    if source.contains("..") || dest.contains("..") {
                        response = error_response("Forbidden", "403");
                    } else {
                        match fs::copy(source, dest) {
                            Ok(_) => {
                                let body_str = "{\"success\":true}".to_string();
                                if let Some(encrypted) = encrypt_api_response(&body_str) {
                                    response = json_response(&encrypted, "200");
                                } else {
                                    response = error_response("Encryption failed", "500");
                                }
                            }
                            Err(e) => {
                                response = error_response(&format!("Cannot copy: {}", e), "500")
                            }
                        }
                    }
                } else {
                    response = error_response("Invalid decrypted data", "400");
                }
            } else {
                response = error_response("Decryption failed", "400");
            }
        } else {
            response = error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/files/move" {
        #[derive(Deserialize)]
        struct MovePayload {
            data: String,
        }
        if let Ok(payload) = serde_json::from_str::<MovePayload>(&body) {
            if let Some(decrypted) = decrypt_api_data(&payload.data) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&decrypted) {
                    let source = parsed.get("source").and_then(|v| v.as_str()).unwrap_or("");
                    let dest = parsed.get("dest").and_then(|v| v.as_str()).unwrap_or("");
                    if source.contains("..") || dest.contains("..") {
                        response = error_response("Forbidden", "403");
                    } else {
                        match fs::rename(source, dest) {
                            Ok(_) => {
                                let body_str = "{\"success\":true}".to_string();
                                if let Some(encrypted) = encrypt_api_response(&body_str) {
                                    response = json_response(&encrypted, "200");
                                } else {
                                    response = error_response("Encryption failed", "500");
                                }
                            }
                            Err(e) => {
                                response = error_response(&format!("Cannot move: {}", e), "500")
                            }
                        }
                    }
                } else {
                    response = error_response("Invalid decrypted data", "400");
                }
            } else {
                response = error_response("Decryption failed", "400");
            }
        } else {
            response = error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/files/download" {
        #[derive(Deserialize)]
        struct DownloadPayload {
            data: String,
        }
        if let Ok(payload) = serde_json::from_str::<DownloadPayload>(&body) {
            if let Some(decrypted) = decrypt_api_data(&payload.data) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&decrypted) {
                    let file_path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
                    if file_path.contains("..") {
                        response = error_response("Forbidden", "403");
                    } else {
                        let p = Path::new(file_path);
                        if !p.exists() {
                            response = error_response("File not found", "404");
                        } else if p.is_dir() {
                            use std::io::Write;
                            let mut buffer = Vec::new();
                            {
                                let mut zip_writer =
                                    zip::ZipWriter::new(std::io::Cursor::new(&mut buffer));
                                let options = zip::write::FileOptions::default()
                                    .compression_method(zip::CompressionMethod::Deflated);
                                fn add_folder_to_zip(
                                    dir: &Path,
                                    prefix: &str,
                                    zip: &mut zip::ZipWriter<std::io::Cursor<&mut Vec<u8>>>,
                                    opts: zip::write::FileOptions,
                                ) {
                                    if let Ok(entries) = fs::read_dir(dir) {
                                        for entry in entries.flatten() {
                                            let path = entry.path();
                                            let name = if prefix.is_empty() {
                                                path.file_name()
                                                    .unwrap()
                                                    .to_string_lossy()
                                                    .to_string()
                                            } else {
                                                format!(
                                                    "{}/{}",
                                                    prefix,
                                                    path.file_name().unwrap().to_string_lossy()
                                                )
                                            };
                                            if path.is_dir() {
                                                add_folder_to_zip(&path, &name, zip, opts);
                                            } else {
                                                if let Ok(_) =
                                                    zip.start_file(name.as_str(), opts.clone())
                                                {
                                                    if let Ok(data) = fs::read(&path) {
                                                        let _ = zip.write_all(&data);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                add_folder_to_zip(p, "", &mut zip_writer, options);
                                let _ = zip_writer.finish();
                            }
                            let zip_data = buffer;
                            response = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/zip\r\nContent-Disposition: attachment; filename=\"{}.zip\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                p.file_name().unwrap_or_default().to_string_lossy(),
                                zip_data.len()
                            );
                            let mut final_response = response.into_bytes();
                            final_response.extend(zip_data);
                            let _ = stream.write(&final_response);
                            let _ = stream.flush();
                            return;
                        } else {
                            match fs::read(file_path) {
                                Ok(bytes) => {
                                    let mime = if file_path.ends_with(".png") {
                                        "image/png"
                                    } else if file_path.ends_with(".jpg")
                                        || file_path.ends_with(".jpeg")
                                    {
                                        "image/jpeg"
                                    } else if file_path.ends_with(".gif") {
                                        "image/gif"
                                    } else if file_path.ends_with(".webp") {
                                        "image/webp"
                                    } else if file_path.ends_with(".svg") {
                                        "image/svg+xml"
                                    } else if file_path.ends_with(".bmp") {
                                        "image/bmp"
                                    } else if file_path.ends_with(".avif") {
                                        "image/avif"
                                    } else {
                                        "application/octet-stream"
                                    };
                                    response = format!(
                                        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
                                        mime, bytes.len()
                                    );
                                    let mut final_response = response.into_bytes();
                                    final_response.extend(bytes);
                                    let _ = stream.write(&final_response);
                                    let _ = stream.flush();
                                    return;
                                }
                                Err(e) => {
                                    response =
                                        error_response(&format!("Cannot read file: {}", e), "500")
                                }
                            }
                        }
                    }
                } else {
                    response = error_response("Invalid decrypted data", "400");
                }
            } else {
                response = error_response("Decryption failed", "400");
            }
        } else {
            response = error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/fileupload/binary" {
        #[derive(Deserialize)]
        struct ChunkPayload {
            data: String,
        }
        const CHUNK_SIZE: usize = 2 * 1024 * 1024;
        if let Ok(payload) = serde_json::from_str::<ChunkPayload>(&body) {
            if let Some(decrypted) = decrypt_api_data(&payload.data) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&decrypted) {
                    let file_path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
                    let chunk_index = parsed
                        .get("chunk_index")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as usize;
                    let total_chunks = parsed
                        .get("total_chunks")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(1) as usize;
                    if file_path.contains("..") {
                        response = error_response("Forbidden", "403");
                    } else {
                        let p = Path::new(file_path);
                        if !p.exists() || p.is_dir() {
                            response = error_response("File not found", "404");
                        } else {
                            match fs::read(file_path) {
                                Ok(bytes) => {
                                    let file_size = bytes.len();
                                    let start = chunk_index * CHUNK_SIZE;
                                    if start >= file_size {
                                        response =
                                            error_response("Chunk index out of range", "400");
                                    } else {
                                        let end = std::cmp::min(start + CHUNK_SIZE, file_size);
                                        let chunk_data = bytes[start..end].to_vec();
                                        let filename = Path::new(file_path)
                                            .file_name()
                                            .and_then(|n| n.to_str())
                                            .unwrap_or("unknown")
                                            .to_string();
                                        let metadata = serde_json::json!({
                                            "filename": filename,
                                            "chunk_index": chunk_index,
                                            "total_chunks": total_chunks,
                                            "file_size": file_size,
                                            "chunk_size": end - start,
                                        })
                                        .to_string();
                                        if let Some(encrypted) =
                                            encrypt_api_binary_response(&metadata, &chunk_data)
                                        {
                                            response = format!(
                                                "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                                encrypted.len()
                                            );
                                            let mut final_response = response.into_bytes();
                                            final_response.extend(encrypted);
                                            let _ = stream.write(&final_response);
                                            let _ = stream.flush();
                                            return;
                                        } else {
                                            response = error_response("Encryption failed", "500");
                                        }
                                    }
                                }
                                Err(e) => {
                                    response =
                                        error_response(&format!("Cannot read file: {}", e), "500")
                                }
                            }
                        }
                    }
                } else {
                    response = error_response("Invalid decrypted data", "400");
                }
            } else {
                response = error_response("Decryption failed", "400");
            }
        } else {
            response = error_response("Invalid request", "400");
        }
    } else if method == "GET" && req_path.starts_with("/api/greet") {
        let name = params.get("name").map(|s| s.as_str()).unwrap_or("World");
        let body = serde_json::to_string(
            &serde_json::json!({"message": format!("Hello, {}! (from Rust HTTP API)", name)}),
        )
        .unwrap();
        response = json_response(&body, "200");
    } else if method == "GET" {
        if let Some(content) = serve_file(req_path) {
            let fs_name = if req_path == "/" || req_path.ends_with('/') {
                "index.html"
            } else {
                req_path
            };
            let mime = determine_mime(fs_name);
            response = format!("HTTP/1.1 200 OK\r\nContent-Type: {}; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", mime, content.len(), content);
        } else {
            if let Some(content) = serve_file("/") {
                response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", content.len(), content);
            } else {
                let body = "<h1>404 Not Found</h1>";
                response = format!("HTTP/1.1 404 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
            }
        }
    } else {
        response =
            "HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                .to_string();
    }

    let _ = stream.write(response.as_bytes());
    let _ = stream.flush();
}

fn spawn_http_server(port: u16) {
    SERVER_PORT.store(port, Ordering::SeqCst);
    SERVER_RUNNING.store(true, Ordering::SeqCst);
    thread::spawn(move || {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port));
        match listener {
            Ok(l) => {
                println!("Rust HTTP server listening on http://127.0.0.1:{}", port);
                for stream in l.incoming() {
                    if !SERVER_RUNNING.load(Ordering::SeqCst) {
                        break;
                    }
                    match stream {
                        Ok(s) => handle_request(s),
                        Err(e) => eprintln!("Connection failed: {}", e),
                    }
                }
                println!("Rust HTTP server stopped");
            }
            Err(e) => eprintln!("Failed to bind to port {}: {}", port, e),
        }
    });
}

// ===== FILE BROWSER COMMANDS =====

#[tauri::command]
fn list_directory(dir: Option<String>) -> Result<Vec<FileItem>, String> {
    let path = dir.unwrap_or_else(|| "/".to_string());
    let base = Path::new(&path);

    // Security: prevent directory traversal outside allowed roots
    if path.contains("..") {
        return Err("Invalid path".to_string());
    }

    let mut items = Vec::new();

    // Add parent directory if not root
    if path != "/" && base.parent().is_some() {
        if let Some(parent) = base.parent() {
            items.push(FileItem {
                name: "..".to_string(),
                path: parent.to_string_lossy().to_string(),
                is_dir: true,
                size: 0,
                modified: None,
            });
        }
    }

    match fs::read_dir(&base) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                let metadata = entry.metadata().ok();
                let file_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                items.push(FileItem {
                    name: file_name.clone(),
                    path: path.to_string_lossy().to_string(),
                    is_dir: metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false),
                    size: metadata.as_ref().map(|m| m.len()).unwrap_or(0),
                    modified: metadata
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs() as i64),
                });
            }
        }
        Err(e) => return Err(format!("Cannot read directory: {}", e)),
    }

    // Sort: directories first, then by name
    items.sort_by(|a, b| {
        if a.is_dir != b.is_dir {
            b.is_dir.cmp(&a.is_dir)
        } else {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        }
    });

    Ok(items)
}

#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    if path.contains("..") {
        return Err("Invalid path".to_string());
    }
    fs::read_to_string(&path).map_err(|e| format!("Cannot read file: {}", e))
}

#[tauri::command]
fn write_text_file(path: String, content: String) -> Result<(), String> {
    if path.contains("..") {
        return Err("Invalid path".to_string());
    }
    fs::write(&path, content).map_err(|e| format!("Cannot write file: {}", e))
}

#[tauri::command]
fn create_directory(dir_path: String) -> Result<(), String> {
    if dir_path.contains("..") {
        return Err("Invalid path".to_string());
    }
    fs::create_dir_all(&dir_path).map_err(|e| format!("Cannot create directory: {}", e))
}

#[tauri::command]
fn delete_file_or_dir(path: String) -> Result<(), String> {
    if path.contains("..") || path == "/" {
        return Err("Invalid path".to_string());
    }
    let p = Path::new(&path);
    if p.is_dir() {
        fs::remove_dir_all(p)
    } else {
        fs::remove_file(p)
    }
    .map_err(|e| format!("Cannot delete: {}", e))
}

#[tauri::command]
fn rename_file_or_dir(old_path: String, new_path: String) -> Result<(), String> {
    if old_path.contains("..") || new_path.contains("..") {
        return Err("Invalid path".to_string());
    }
    fs::rename(&old_path, &new_path).map_err(|e| format!("Cannot rename: {}", e))
}

#[tauri::command]
fn get_home_dir() -> Result<String, String> {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "Cannot determine home directory".to_string())
}

#[tauri::command]
fn get_temp_dir() -> Result<String, String> {
    std::env::temp_dir()
        .to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Cannot determine temp directory".to_string())
}

#[tauri::command]
fn get_downloads_dir() -> Result<String, String> {
    dirs::download_dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "Downloads directory not found".to_string())
}

#[tauri::command]
fn copy_file(source: String, destination: String) -> Result<(), String> {
    if source.contains("..") || destination.contains("..") {
        return Err("Invalid path".to_string());
    }
    fs::copy(&source, &destination)
        .map(|_| ())
        .map_err(|e| format!("Copy failed: {}", e))
}

#[tauri::command]
fn move_file(source: String, destination: String) -> Result<(), String> {
    if source.contains("..") || destination.contains("..") {
        return Err("Invalid path".to_string());
    }
    fs::rename(&source, &destination).map_err(|e| format!("Move failed: {}", e))
}

#[tauri::command]
fn copy_file_to_clipboard(path: String) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::process::Command;
        Command::new("xclip")
            .args(&["-selection", "clipboard", "-i", &path])
            .output()
            .map_err(|e| format!("Copy to clipboard failed: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
fn get_file_info(path: String) -> Result<FileItem, String> {
    if path.contains("..") {
        return Err("Invalid path".to_string());
    }
    let p = Path::new(&path);
    let metadata = fs::metadata(&p).map_err(|e| format!("Cannot get file info: {}", e))?;
    let file_name = p
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    Ok(FileItem {
        name: file_name,
        path: path.clone(),
        is_dir: metadata.is_dir(),
        size: metadata.len(),
        modified: metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64),
    })
}

#[tauri::command]
fn file_exists(path: String) -> Result<bool, String> {
    if path.contains("..") {
        return Err("Invalid path".to_string());
    }
    Ok(Path::new(&path).exists())
}

#[tauri::command]
fn search_files(dir: String, pattern: String) -> Result<Vec<FileItem>, String> {
    if dir.contains("..") {
        return Err("Invalid directory".to_string());
    }
    let base = Path::new(&dir);
    let mut results = Vec::new();
    let pattern_lower = pattern.to_lowercase();

    fn walk_dir(dir: &Path, pattern: &str, results: &mut Vec<FileItem>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.to_lowercase().contains(pattern) {
                        let metadata = entry.metadata().ok();
                        results.push(FileItem {
                            name: name.to_string(),
                            path: path.to_string_lossy().to_string(),
                            is_dir: metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false),
                            size: metadata.as_ref().map(|m| m.len()).unwrap_or(0),
                            modified: metadata
                                .and_then(|m| m.modified().ok())
                                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|d| d.as_secs() as i64),
                        });
                    }
                    if path.is_dir() {
                        walk_dir(&path, pattern, results);
                    }
                }
            }
        }
    }

    walk_dir(base, &pattern_lower, &mut results);
    Ok(results)
}

#[tauri::command]
fn get_drives() -> Result<Vec<FileItem>, String> {
    let mut items = Vec::new();

    #[cfg(unix)]
    {
        items.push(FileItem {
            name: "/".to_string(),
            path: "/".to_string(),
            is_dir: true,
            size: 0,
            modified: None,
        });
        if let Some(home) = dirs::home_dir() {
            items.push(FileItem {
                name: "Home".to_string(),
                path: home.to_string_lossy().to_string(),
                is_dir: true,
                size: 0,
                modified: None,
            });
        }
    }

    #[cfg(windows)]
    {
        for letter in b'A'..=b'Z' {
            let drive = format!("{}:\\", letter as char);
            let p = Path::new(&drive);
            if p.exists() {
                items.push(FileItem {
                    name: format!("Drive {}", letter as char),
                    path: drive,
                    is_dir: true,
                    size: 0,
                    modified: None,
                });
            }
        }
    }

    Ok(items)
}

#[tauri::command]
fn execute_terminal_command(cmd: String) -> Result<String, String> {
    use std::process::Command;

    #[cfg(unix)]
    {
        let output = Command::new("sh")
            .args(&["-c", &cmd])
            .output()
            .map_err(|e| format!("Command failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if !stderr.is_empty() {
            Ok(format!("{}{}", stdout, stderr))
        } else {
            Ok(stdout)
        }
    }

    #[cfg(windows)]
    {
        let output = Command::new("cmd")
            .args(&["/C", &cmd])
            .output()
            .map_err(|e| format!("Command failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if !stderr.is_empty() {
            Ok(format!("{}{}", stdout, stderr))
        } else {
            Ok(stdout)
        }
    }
}

#[tauri::command]
fn get_system_info() -> Result<SystemInfo, String> {
    let hostname = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown".to_string());

    let sys = sysinfo::System::new_all();

    Ok(SystemInfo {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        hostname,
        cpu_count: sys.cpus().len(),
        total_memory: sys.total_memory(),
        free_memory: sys.free_memory(),
    })
}

#[tauri::command]
fn get_process_list() -> Result<Vec<ProcessItem>, String> {
    let sys = sysinfo::System::new_all();
    let mut processes = Vec::new();

    for (pid, process) in sys.processes() {
        processes.push(ProcessItem {
            pid: pid.as_u32(),
            name: process.name().to_string(),
            cpu: process.cpu_usage(),
            memory: process.memory(),
        });
    }

    processes.sort_by(|a, b| {
        b.cpu
            .partial_cmp(&a.cpu)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(processes.into_iter().take(100).collect()) // Top 100
}

#[tauri::command]
fn execute_command(cmd: String, args: Vec<String>) -> Result<(i32, String, String), String> {
    use std::process::Command;

    let output = Command::new(&cmd)
        .args(&args)
        .output()
        .map_err(|e| format!("Command execution failed: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    Ok((output.status.code().unwrap_or(-1), stdout, stderr))
}

#[tauri::command]
fn kill_process(pid: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;
        kill(Pid::from_raw(pid as i32), Signal::SIGTERM)
            .map_err(|e| format!("Cannot kill process: {}", e))?;
    }
    #[cfg(windows)]
    {
        use std::process::Command;
        Command::new("taskkill")
            .args(&["/PID", &pid.to_string(), "/F"])
            .output()
            .map_err(|e| format!("Cannot kill process: {}", e))?;
    }
    Ok(())
}

// ===== TLS & SERVER COMMANDS =====

#[tauri::command]
fn generate_local_certs(domain: Option<String>) -> Result<TlsCertificates, String> {
    let domain = domain.unwrap_or_else(|| "localhost".to_string());
    let (ca_cert, ca_pem, ca_key) = generate_ca()?;
    let key_pair = KeyPair::from_pem(&ca_key).map_err(|e| e.to_string())?;
    let (domain_cert, domain_key) = generate_domain_cert(&ca_cert, &key_pair, &domain)?;

    let mut cert_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    cert_dir.push("certs");
    fs::create_dir_all(&cert_dir).map_err(|e| format!("Failed to create certs dir: {}", e))?;

    fs::write(cert_dir.join("ca_cert.pem"), &ca_pem)
        .map_err(|e| format!("Failed to write ca_cert.pem: {}", e))?;
    fs::write(cert_dir.join("ca_key.pem"), &ca_key)
        .map_err(|e| format!("Failed to write ca_key.pem: {}", e))?;
    fs::write(cert_dir.join(format!("{}.pem", domain)), &domain_cert)
        .map_err(|e| format!("Failed to write domain cert: {}", e))?;
    fs::write(cert_dir.join(format!("{}-key.pem", domain)), &domain_key)
        .map_err(|e| format!("Failed to write domain key: {}", e))?;

    Ok(TlsCertificates {
        ca_cert: ca_pem,
        ca_key,
        domain,
        domain_cert,
        domain_key,
    })
}

#[tauri::command]
fn generate_tls_certificates(domain: Option<String>) -> Result<TlsCertificates, String> {
    let domain = domain.unwrap_or_else(|| "localhost".to_string());
    let (ca_cert, ca_pem, ca_key) = generate_ca()?;
    let key_pair = KeyPair::from_pem(&ca_key).map_err(|e| e.to_string())?;
    let (domain_cert, domain_key) = generate_domain_cert(&ca_cert, &key_pair, &domain)?;
    Ok(TlsCertificates {
        ca_cert: ca_pem,
        ca_key,
        domain,
        domain_cert,
        domain_key,
    })
}

fn start_http_server(port: u16) -> Result<(), String> {
    if SERVER_RUNNING.load(Ordering::SeqCst) {
        return Err("Server already running".to_string());
    }
    spawn_http_server(port);
    Ok(())
}

#[tauri::command]
fn start_http_server_cmd(port: Option<u16>) -> Result<(), String> {
    let port = port.unwrap_or_else(|| SERVER_PORT.load(Ordering::SeqCst));
    start_http_server(port)
}

#[tauri::command]
fn stop_http_server() -> Result<bool, String> {
    if SERVER_RUNNING.load(Ordering::SeqCst) {
        SERVER_RUNNING.store(false, Ordering::SeqCst);
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
fn is_server_running() -> bool {
    SERVER_RUNNING.load(Ordering::SeqCst)
}

#[tauri::command]
fn get_server_port() -> u16 {
    SERVER_PORT.load(Ordering::SeqCst)
}

#[tauri::command]
fn set_server_port(port: u16) -> Result<(), String> {
    let running = SERVER_RUNNING.load(Ordering::SeqCst);
    if running {
        return Err("Stop server first before changing port".to_string());
    }
    SERVER_PORT.store(port, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
fn set_random_shared_alphanumeric_key() -> String {
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

#[tauri::command]
fn get_shared_key() -> Option<String> {
    let key = SHARED_KEY.lock().unwrap();
    let key_str: &str = key.as_ref();
    eprintln!("DEBUG get_shared_key: accessed, value = {}", key_str);
    if key.is_empty() {
        None
    } else {
        Some(key.clone())
    }
}

#[tauri::command]
fn get_session_new_key() -> Option<String> {
    let key = SESSION_NEW_KEY.lock().unwrap();
    let key_str: &str = key.as_ref();
    eprintln!("DEBUG get_session_new_key: accessed, value = {}", key_str);
    if key.is_empty() {
        None
    } else {
        Some(key.clone())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(e) = generate_local_certs(None) {
        eprintln!("Warning: Failed to auto-generate TLS certificates: {}", e);
    }

    start_http_server(8080).ok();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet_json,
            generate_tls_certificates,
            generate_local_certs,
            start_http_server_cmd,
            stop_http_server,
            is_server_running,
            set_server_port,
            get_server_port,
            // File operations
            list_directory,
            read_text_file,
            write_text_file,
            create_directory,
            delete_file_or_dir,
            rename_file_or_dir,
            get_home_dir,
            get_temp_dir,
            get_downloads_dir,
            copy_file,
            move_file,
            copy_file_to_clipboard,
            get_file_info,
            file_exists,
            search_files,
            get_drives,
            // System info
            get_system_info,
            get_process_list,
            execute_command,
            kill_process,
            execute_terminal_command,
            // Binary file
            get_binary_file,
            get_binary_mime,
            get_shared_key,
            set_random_shared_alphanumeric_key,
            get_session_new_key,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
