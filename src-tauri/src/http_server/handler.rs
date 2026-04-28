use crate::crypto::common::{
    decrypt_api_data, encrypt_api_binary_response, encrypt_api_binary_response_simple,
    encrypt_api_response,
};
use crate::shared::{SESSION_NEW_KEY, SHARED_KEY};
use crate::utilities;
use ring::digest::{digest, SHA256};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use time::OffsetDateTime;

#[derive(serde::Deserialize)]
struct ApiPayload {
    data: String,
}

fn decrypt_aes_gcm(key: &str, data: &str) -> Result<Vec<String>, String> {
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data)
        .map_err(|_| "Invalid base64".to_string())?;
    const NONCE_SIZE: usize = 12;
    if bytes.len() < NONCE_SIZE + 16 {
        return Err("Invalid ciphertext".to_string());
    }
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Nonce,
    };
    let nonce = Nonce::from_slice(&bytes[..NONCE_SIZE]);
    let ciphertext = &bytes[NONCE_SIZE..];
    let key_hash = digest(&SHA256, key.as_bytes());
    let cipher = Aes256Gcm::new_from_slice(key_hash.as_ref())
        .map_err(|_| "Failed to initialize cipher".to_string())?;
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "Decryption failed".to_string())?;
    rmp_serde::from_slice::<Vec<String>>(&plaintext).map_err(|_| "Invalid payload".to_string())
}

fn search_files(dir: &Path, pattern: &str) -> Vec<serde_json::Value> {
    let mut results = Vec::new();
    let pattern_lower = pattern.to_lowercase();
    fn walk_dir(dir: &Path, pat: &str, results: &mut Vec<serde_json::Value>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.to_lowercase().contains(pat) {
                        let metadata = entry.metadata().ok();
                        let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
                        let modified = metadata
                            .clone()
                            .and_then(|m| m.modified().ok())
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_secs() as i64);
                        let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                        results.push(serde_json::json!({
                            "name": name,
                            "path": path.to_string_lossy().to_string(),
                            "is_dir": is_dir,
                            "size": size,
                            "modified": modified
                        }));
                    }
                    if path.is_dir() {
                        walk_dir(&path, pat, results);
                    }
                }
            }
        }
    }
    walk_dir(dir, &pattern_lower, &mut results);
    results
}

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
                path.file_name().unwrap().to_string_lossy().to_string()
            } else {
                format!("{}/{}", prefix, path.file_name().unwrap().to_string_lossy())
            };
            if path.is_dir() {
                add_folder_to_zip(&path, &name, zip, opts);
            } else if zip.start_file(name.as_str(), opts.clone()).is_ok() {
                if let Ok(data) = fs::read(&path) {
                    let _ = zip.write_all(&data);
                }
            }
        }
    }
}

pub fn handle_request(mut stream: TcpStream) {
    if let Ok(addr) = stream.peer_addr() {
        let ip = addr.ip();
        let is_localhost = ip.is_loopback();
        let is_192168 = match ip {
            std::net::IpAddr::V4(v4) => v4.octets()[0] == 192 && v4.octets()[1] == 168,
            _ => false,
        };
        if !is_localhost && !is_192168 {
            let _ = stream.write_all(
                b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
            return;
        }
    }
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

    let params = utilities::parse_query_params(query);
    let response;

    if method == "OPTIONS" {
        response = "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, DELETE, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nConnection: close\r\n\r\n".to_string();
    } else if method == "GET" && clean_path == "/api/health" {
        let ts = OffsetDateTime::now_utc().unix_timestamp();
        let body =
            serde_json::to_string(&serde_json::json!({"status": "ok", "timestamp": ts})).unwrap();
        response = utilities::json_response(&body, "200");
    } else if method == "POST" && clean_path == "/api/session/verify" {
        let key = SHARED_KEY.lock().unwrap();
        if key.is_empty() {
            response = utilities::error_response("No shared key set", "400");
        } else if let Ok(payload) = serde_json::from_str::<ApiPayload>(&body) {
            match decrypt_aes_gcm(&key, &payload.data) {
                Ok(vec) if vec.len() >= 2 => {
                    *SESSION_NEW_KEY.lock().unwrap() = vec[1].clone();
                    response = utilities::json_response(r#"{"valid":true}"#, "200");
                }
                Ok(_) => response = utilities::error_response("Invalid payload", "400"),
                Err(e) => response = utilities::error_response(&e, "400"),
            }
        } else {
            response = utilities::error_response("Invalid request body", "400");
        }
    } else if method == "POST" && clean_path == "/api/session/decrypt" {
        let new_key = SESSION_NEW_KEY.lock().unwrap();
        if new_key.is_empty() {
            response = utilities::error_response("No session key", "400");
        } else if let Ok(payload) = serde_json::from_str::<ApiPayload>(&body) {
            match decrypt_aes_gcm(&new_key, &payload.data) {
                Ok(vec) if vec.len() >= 2 => {
                    let body =
                        serde_json::to_string(&serde_json::json!({"payload": vec[1]})).unwrap();
                    response = utilities::json_response(&body, "200");
                }
                Ok(_) => response = utilities::error_response("Invalid payload", "400"),
                Err(e) => response = utilities::error_response(&e, "400"),
            }
        } else {
            response = utilities::error_response("Invalid request body", "400");
        }
    } else if method == "POST" && clean_path == "/api/system/home" {
        if let Ok(payload) = serde_json::from_str::<ApiPayload>(&body) {
            if decrypt_api_data(&payload.data).is_some() {
                let home = dirs::home_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| "/".to_string());
                let body_str = serde_json::to_string(&serde_json::json!({"path": home})).unwrap();
                response = encrypt_api_response(&body_str).map_or_else(
                    || utilities::error_response("Encryption failed", "500"),
                    |enc| utilities::json_response(&enc, "200"),
                );
            } else {
                response = utilities::error_response("Decryption failed", "400");
            }
        } else {
            response = utilities::error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/system/drives" {
        if let Ok(payload) = serde_json::from_str::<ApiPayload>(&body) {
            if decrypt_api_data(&payload.data).is_some() {
                let mut items = Vec::new();
                #[cfg(unix)]
                {
                    items.push(serde_json::json!({"name": "/", "path": "/", "is_dir": true, "size": 0, "modified": null}));
                    if let Some(home) = dirs::home_dir() {
                        items.push(serde_json::json!({"name": "Home", "path": home.to_string_lossy().to_string(), "is_dir": true, "size": 0, "modified": null}));
                    }
                }
                let body_str = serde_json::to_string(&items).unwrap();
                response = encrypt_api_response(&body_str).map_or_else(
                    || utilities::error_response("Encryption failed", "500"),
                    |enc| utilities::json_response(&enc, "200"),
                );
            } else {
                response = utilities::error_response("Decryption failed", "400");
            }
        } else {
            response = utilities::error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/system/processes" {
        if let Ok(payload) = serde_json::from_str::<ApiPayload>(&body) {
            if decrypt_api_data(&payload.data).is_some() {
                let sys = sysinfo::System::new_all();
                let mut processes: Vec<serde_json::Value> = sys
                    .processes()
                    .iter()
                    .map(|(pid, process)| {
                        serde_json::json!({
                            "pid": pid.as_u32(),
                            "name": process.name().to_string(),
                            "cpu": process.cpu_usage(),
                            "memory": process.memory()
                        })
                    })
                    .collect();
                processes.sort_by(|a, b| {
                    let a_cpu = a.get("cpu").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let b_cpu = b.get("cpu").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    b_cpu
                        .partial_cmp(&a_cpu)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                let body_str = serde_json::to_string(&processes).unwrap();
                response = encrypt_api_response(&body_str).map_or_else(
                    || utilities::error_response("Encryption failed", "500"),
                    |enc| utilities::json_response(&enc, "200"),
                );
            } else {
                response = utilities::error_response("Decryption failed", "400");
            }
        } else {
            response = utilities::error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/files/list" {
        if let Ok(payload) = serde_json::from_str::<ApiPayload>(&body) {
            if let Some(decrypted) = decrypt_api_data(&payload.data) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&decrypted) {
                    let dir = parsed.get("dir").and_then(|v| v.as_str()).unwrap_or("/");
                    if dir.contains("..") {
                        response = utilities::error_response("Forbidden", "403");
                    } else if let Ok(entries) = fs::read_dir(Path::new(dir)) {
                        let mut items: Vec<_> = entries
                            .flatten()
                            .filter_map(|entry| {
                                let path = entry.path();
                                let metadata = entry.metadata().ok()?;
                                let name = path.file_name()?.to_str()?.to_string();
                                let size = metadata.len();
                                let modified = metadata
                                    .modified()
                                    .ok()
                                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                    .map(|d| d.as_secs() as i64);
                                let is_dir = metadata.is_dir();
                                Some(serde_json::json!({
                                    "name": name,
                                    "path": path.to_string_lossy().to_string(),
                                    "is_dir": is_dir,
                                    "size": size,
                                    "modified": modified
                                }))
                            })
                            .collect();
                        items.sort_by(|a, b| {
                            let a_dir = a.get("is_dir").and_then(|v| v.as_bool()).unwrap_or(false);
                            let b_dir = b.get("is_dir").and_then(|v| v.as_bool()).unwrap_or(false);
                            if a_dir != b_dir {
                                return b_dir.cmp(&a_dir);
                            }
                            let a_name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
                            let b_name = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
                            a_name.to_lowercase().cmp(&b_name.to_lowercase())
                        });
                        let body_str = serde_json::to_string(&items).unwrap();
                        response = encrypt_api_response(&body_str).map_or_else(
                            || utilities::error_response("Encryption failed", "500"),
                            |enc| utilities::json_response(&enc, "200"),
                        );
                    } else {
                        response = utilities::error_response("Cannot read directory", "500");
                    }
                } else {
                    response = utilities::error_response("Invalid decrypted data", "400");
                }
            } else {
                response = utilities::error_response("Decryption failed", "400");
            }
        } else {
            response = utilities::error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/files/info" {
        if let Ok(payload) = serde_json::from_str::<ApiPayload>(&body) {
            if let Some(decrypted) = decrypt_api_data(&payload.data) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&decrypted) {
                    let file_path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
                    if file_path.contains("..") {
                        response = utilities::error_response("Forbidden", "403");
                    } else if let Ok(metadata) = fs::metadata(file_path) {
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
                        response = encrypt_api_response(&body_str).map_or_else(
                            || utilities::error_response("Encryption failed", "500"),
                            |enc| utilities::json_response(&enc, "200"),
                        );
                    } else {
                        response = utilities::error_response("Cannot get info", "500");
                    }
                } else {
                    response = utilities::error_response("Invalid decrypted data", "400");
                }
            } else {
                response = utilities::error_response("Decryption failed", "400");
            }
        } else {
            response = utilities::error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/files/search" {
        if let Ok(payload) = serde_json::from_str::<ApiPayload>(&body) {
            if let Some(decrypted) = decrypt_api_data(&payload.data) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&decrypted) {
                    let dir = parsed.get("dir").and_then(|v| v.as_str()).unwrap_or("/");
                    let pattern = parsed.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
                    if dir.contains("..") {
                        response = utilities::error_response("Forbidden", "403");
                    } else {
                        let results = search_files(Path::new(dir), pattern);
                        let body_str = serde_json::to_string(&results).unwrap();
                        response = encrypt_api_response(&body_str).map_or_else(
                            || utilities::error_response("Encryption failed", "500"),
                            |enc| utilities::json_response(&enc, "200"),
                        );
                    }
                } else {
                    response = utilities::error_response("Invalid decrypted data", "400");
                }
            } else {
                response = utilities::error_response("Decryption failed", "400");
            }
        } else {
            response = utilities::error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/files/read" {
        if let Ok(payload) = serde_json::from_str::<ApiPayload>(&body) {
            if let Some(decrypted) = decrypt_api_data(&payload.data) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&decrypted) {
                    let file_path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
                    if file_path.contains("..") {
                        response = utilities::error_response("Forbidden", "403");
                    } else if let Ok(bytes) = fs::read(file_path) {
                        let mime = utilities::determine_mime(file_path);
                        let b64 = utilities::base64_encode(&bytes);
                        let body_str = serde_json::to_string(
                            &serde_json::json!({"content": b64, "mime": mime, "binary": true}),
                        )
                        .unwrap();
                        response = encrypt_api_response(&body_str).map_or_else(
                            || utilities::error_response("Encryption failed", "500"),
                            |enc| utilities::json_response(&enc, "200"),
                        );
                    } else {
                        response = utilities::error_response("Cannot read file", "500");
                    }
                } else {
                    response = utilities::error_response("Invalid decrypted data", "400");
                }
            } else {
                response = utilities::error_response("Decryption failed", "400");
            }
        } else {
            response = utilities::error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/files/binary" {
        if let Ok(payload) = serde_json::from_str::<ApiPayload>(&body) {
            if let Some(decrypted) = decrypt_api_data(&payload.data) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&decrypted) {
                    let file_path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
                    if file_path.contains("..") {
                        response = utilities::error_response("Forbidden", "403");
                    } else if let Ok(bytes) = fs::read(file_path) {
                        if let Some(encrypted) = encrypt_api_binary_response_simple(&bytes) {
                            let resp = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
                                encrypted.len()
                            );
                            let mut final_response = resp.into_bytes();
                            final_response.extend(encrypted);
                            let _ = stream.write(&final_response);
                            let _ = stream.flush();
                            return;
                        } else {
                            response = utilities::error_response("Encryption failed", "500");
                        }
                    } else {
                        response = utilities::error_response("Cannot read file", "500");
                    }
                } else {
                    response = utilities::error_response("Invalid decrypted data", "400");
                }
            } else {
                response = utilities::error_response("Decryption failed", "400");
            }
        } else {
            response = utilities::error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/files/delete" {
        if let Ok(payload) = serde_json::from_str::<ApiPayload>(&body) {
            if let Some(decrypted) = decrypt_api_data(&payload.data) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&decrypted) {
                    let file_path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
                    if file_path.contains("..") || file_path == "/" {
                        response = utilities::error_response("Forbidden", "403");
                    } else {
                        let p = Path::new(file_path);
                        let result = if p.is_dir() {
                            fs::remove_dir_all(p)
                        } else {
                            fs::remove_file(p)
                        };
                        if result.is_ok() {
                            response = encrypt_api_response(r#"{"success":true}"#).map_or_else(
                                || utilities::error_response("Encryption failed", "500"),
                                |enc| utilities::json_response(&enc, "200"),
                            );
                        } else {
                            response = utilities::error_response("Cannot delete", "500");
                        }
                    }
                } else {
                    response = utilities::error_response("Invalid decrypted data", "400");
                }
            } else {
                response = utilities::error_response("Decryption failed", "400");
            }
        } else {
            response = utilities::error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/files/write" {
        if let Ok(payload) = serde_json::from_str::<ApiPayload>(&body) {
            if let Some(decrypted) = decrypt_api_data(&payload.data) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&decrypted) {
                    let file_path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
                    let content = parsed.get("content").and_then(|v| v.as_str()).unwrap_or("");
                    if file_path.contains("..") {
                        response = utilities::error_response("Forbidden", "403");
                    } else if fs::write(file_path, content).is_ok() {
                        response = encrypt_api_response(r#"{"success":true}"#).map_or_else(
                            || utilities::error_response("Encryption failed", "500"),
                            |enc| utilities::json_response(&enc, "200"),
                        );
                    } else {
                        response = utilities::error_response("Cannot write file", "500");
                    }
                } else {
                    response = utilities::error_response("Invalid decrypted data", "400");
                }
            } else {
                response = utilities::error_response("Decryption failed", "400");
            }
        } else {
            response = utilities::error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/files/create-dir" {
        if let Ok(payload) = serde_json::from_str::<ApiPayload>(&body) {
            if let Some(decrypted) = decrypt_api_data(&payload.data) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&decrypted) {
                    let dir_path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
                    if dir_path.contains("..") {
                        response = utilities::error_response("Forbidden", "403");
                    } else if fs::create_dir_all(dir_path).is_ok() {
                        response = encrypt_api_response(r#"{"success":true}"#).map_or_else(
                            || utilities::error_response("Encryption failed", "500"),
                            |enc| utilities::json_response(&enc, "200"),
                        );
                    } else {
                        response = utilities::error_response("Cannot create directory", "500");
                    }
                } else {
                    response = utilities::error_response("Invalid decrypted data", "400");
                }
            } else {
                response = utilities::error_response("Decryption failed", "400");
            }
        } else {
            response = utilities::error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/files/rename" {
        if let Ok(payload) = serde_json::from_str::<ApiPayload>(&body) {
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
                        response = utilities::error_response("Forbidden", "403");
                    } else if fs::rename(old_path, new_path).is_ok() {
                        response = encrypt_api_response(r#"{"success":true}"#).map_or_else(
                            || utilities::error_response("Encryption failed", "500"),
                            |enc| utilities::json_response(&enc, "200"),
                        );
                    } else {
                        response = utilities::error_response("Cannot rename", "500");
                    }
                } else {
                    response = utilities::error_response("Invalid decrypted data", "400");
                }
            } else {
                response = utilities::error_response("Decryption failed", "400");
            }
        } else {
            response = utilities::error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/files/copy" {
        if let Ok(payload) = serde_json::from_str::<ApiPayload>(&body) {
            if let Some(decrypted) = decrypt_api_data(&payload.data) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&decrypted) {
                    let source = parsed.get("source").and_then(|v| v.as_str()).unwrap_or("");
                    let dest = parsed.get("dest").and_then(|v| v.as_str()).unwrap_or("");
                    if source.contains("..") || dest.contains("..") {
                        response = utilities::error_response("Forbidden", "403");
                    } else if fs::copy(source, dest).is_ok() {
                        response = encrypt_api_response(r#"{"success":true}"#).map_or_else(
                            || utilities::error_response("Encryption failed", "500"),
                            |enc| utilities::json_response(&enc, "200"),
                        );
                    } else {
                        response = utilities::error_response("Cannot copy", "500");
                    }
                } else {
                    response = utilities::error_response("Invalid decrypted data", "400");
                }
            } else {
                response = utilities::error_response("Decryption failed", "400");
            }
        } else {
            response = utilities::error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/files/move" {
        if let Ok(payload) = serde_json::from_str::<ApiPayload>(&body) {
            if let Some(decrypted) = decrypt_api_data(&payload.data) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&decrypted) {
                    let source = parsed.get("source").and_then(|v| v.as_str()).unwrap_or("");
                    let dest = parsed.get("dest").and_then(|v| v.as_str()).unwrap_or("");
                    if source.contains("..") || dest.contains("..") {
                        response = utilities::error_response("Forbidden", "403");
                    } else if fs::rename(source, dest).is_ok() {
                        response = encrypt_api_response(r#"{"success":true}"#).map_or_else(
                            || utilities::error_response("Encryption failed", "500"),
                            |enc| utilities::json_response(&enc, "200"),
                        );
                    } else {
                        response = utilities::error_response("Cannot move", "500");
                    }
                } else {
                    response = utilities::error_response("Invalid decrypted data", "400");
                }
            } else {
                response = utilities::error_response("Decryption failed", "400");
            }
        } else {
            response = utilities::error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/files/download" {
        if let Ok(payload) = serde_json::from_str::<ApiPayload>(&body) {
            if let Some(decrypted) = decrypt_api_data(&payload.data) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&decrypted) {
                    let file_path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
                    if file_path.contains("..") {
                        response = utilities::error_response("Forbidden", "403");
                    } else {
                        let p = Path::new(file_path);
                        if !p.exists() {
                            response = utilities::error_response("File not found", "404");
                        } else if p.is_dir() {
                            let mut buffer = Vec::new();
                            {
                                let mut zip_writer =
                                    zip::ZipWriter::new(std::io::Cursor::new(&mut buffer));
                                let options = zip::write::FileOptions::default()
                                    .compression_method(zip::CompressionMethod::Deflated);
                                add_folder_to_zip(p, "", &mut zip_writer, options);
                                let _ = zip_writer.finish();
                            }
                            let resp = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/zip\r\nContent-Disposition: attachment; filename=\"{}.zip\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                p.file_name().unwrap_or_default().to_string_lossy(),
                                buffer.len()
                            );
                            let mut final_response = resp.into_bytes();
                            final_response.extend(buffer);
                            let _ = stream.write(&final_response);
                            let _ = stream.flush();
                            return;
                        } else if let Ok(bytes) = fs::read(file_path) {
                            let mime = utilities::determine_mime(file_path);
                            let resp = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
                                mime, bytes.len()
                            );
                            let mut final_response = resp.into_bytes();
                            final_response.extend(bytes);
                            let _ = stream.write(&final_response);
                            let _ = stream.flush();
                            return;
                        } else {
                            response = utilities::error_response("Cannot read file", "500");
                        }
                    }
                } else {
                    response = utilities::error_response("Invalid decrypted data", "400");
                }
            } else {
                response = utilities::error_response("Decryption failed", "400");
            }
        } else {
            response = utilities::error_response("Invalid request", "400");
        }
    } else if method == "POST" && clean_path == "/api/fileupload/binary" {
        const CHUNK_SIZE: usize = 2 * 1024 * 1024;
        if let Ok(payload) = serde_json::from_str::<ApiPayload>(&body) {
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
                        response = utilities::error_response("Forbidden", "403");
                    } else {
                        let p = Path::new(file_path);
                        if !p.exists() || p.is_dir() {
                            response = utilities::error_response("File not found", "404");
                        } else if let Ok(bytes) = fs::read(file_path) {
                            let file_size = bytes.len();
                            let start = chunk_index * CHUNK_SIZE;
                            if start >= file_size {
                                response =
                                    utilities::error_response("Chunk index out of range", "400");
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
                                    let resp = format!(
                                        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                        encrypted.len()
                                    );
                                    let mut final_response = resp.into_bytes();
                                    final_response.extend(encrypted);
                                    let _ = stream.write(&final_response);
                                    let _ = stream.flush();
                                    return;
                                } else {
                                    response =
                                        utilities::error_response("Encryption failed", "500");
                                }
                            }
                        } else {
                            response = utilities::error_response("Cannot read file", "500");
                        }
                    }
                } else {
                    response = utilities::error_response("Invalid decrypted data", "400");
                }
            } else {
                response = utilities::error_response("Decryption failed", "400");
            }
        } else {
            response = utilities::error_response("Invalid request", "400");
        }
    } else if method == "GET" && req_path.starts_with("/api/greet") {
        let name = params.get("name").map(|s| s.as_str()).unwrap_or("World");
        let body = serde_json::to_string(
            &serde_json::json!({"message": format!("Hello, {}! (from Rust HTTP API)", name)}),
        )
        .unwrap();
        response = utilities::json_response(&body, "200");
    } else if method == "GET" {
        if let Some(content) = utilities::serve_file(req_path) {
            let fs_name = if req_path == "/" || req_path.ends_with('/') {
                "index.html"
            } else {
                req_path
            };
            let mime = utilities::determine_mime(fs_name);
            response = format!("HTTP/1.1 200 OK\r\nContent-Type: {}; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", mime, content.len(), content);
        } else {
            if let Some(content) = utilities::serve_file("/") {
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
