use crate::crypto::common::{
    decrypt_api_data, encrypt_api_binary_response_simple, encrypt_api_response,
};
use crate::http_server::api_error::ApiError;
use crate::http_server::responses;
use crate::utilities;
use std::fs;
use std::io::Write;
use std::path::Path;

#[derive(serde::Deserialize)]
struct ApiPayload {
    data: String,
}

pub fn validate_path(path: &str) -> Result<(), ApiError> {
    if path.contains("..") {
        Err(ApiError::Forbidden)
    } else {
        Ok(())
    }
}

pub fn search_files(dir: &Path, pattern: &str) -> Vec<serde_json::Value> {
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

pub fn add_folder_to_zip(
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

pub fn handle_files_list(body: &str) -> String {
    handle_encrypted_api(body, |parsed| {
        let dir = parsed.get("dir").and_then(|v| v.as_str()).unwrap_or("/");
        validate_path(dir).map_err(ApiError::from)?;
        let entries = fs::read_dir(Path::new(dir)).map_err(|_| ApiError::IoError)?;
        let mut items: Vec<serde_json::Value> = entries
            .flatten()
            .filter_map(|entry: std::fs::DirEntry| {
                let path = entry.path();
                let metadata = entry.metadata().ok()?;
                let name = path.file_name()?.to_str()?.to_string();
                let size = metadata.len();
                let modified = metadata
                    .modified()
                    .ok()
                    .and_then(|t: std::time::SystemTime| {
                        t.duration_since(std::time::UNIX_EPOCH).ok()
                    })
                    .map(|d: std::time::Duration| d.as_secs() as i64);
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
        items.sort_by(|a: &serde_json::Value, b: &serde_json::Value| {
            let a_dir = a
                .get("is_dir")
                .and_then(|v: &serde_json::Value| v.as_bool())
                .unwrap_or(false);
            let b_dir = b
                .get("is_dir")
                .and_then(|v: &serde_json::Value| v.as_bool())
                .unwrap_or(false);
            if a_dir != b_dir {
                return b_dir.cmp(&a_dir);
            }
            let a_name = a
                .get("name")
                .and_then(|v: &serde_json::Value| v.as_str())
                .unwrap_or("");
            let b_name = b
                .get("name")
                .and_then(|v: &serde_json::Value| v.as_str())
                .unwrap_or("");
            a_name.to_lowercase().cmp(&b_name.to_lowercase())
        });
        serde_json::to_string(&items)
            .map_err(|_| ApiError::BadRequest("Serialization failed".to_string()))
    })
}

pub fn handle_files_info(body: &str) -> String {
    handle_encrypted_api(body, |parsed| {
        let file_path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
        validate_path(file_path).map_err(ApiError::from)?;
        let metadata = fs::metadata(file_path).map_err(|_| ApiError::IoError)?;
        let name = Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        let size = metadata.len();
        let modified = metadata
            .modified()
            .ok()
            .and_then(|t: std::time::SystemTime| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d: std::time::Duration| d.as_secs() as i64);
        let is_dir = metadata.is_dir();
        serde_json::to_string(&serde_json::json!({
            "name": name, "path": file_path, "is_dir": is_dir, "size": size, "modified": modified
        }))
        .map_err(|_| ApiError::BadRequest("Serialization failed".to_string()))
    })
}

pub fn handle_files_search(body: &str) -> String {
    handle_encrypted_api(body, |parsed| {
        let dir = parsed.get("dir").and_then(|v| v.as_str()).unwrap_or("/");
        let pattern = parsed.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
        validate_path(dir).map_err(ApiError::from)?;
        let results = search_files(Path::new(dir), pattern);
        serde_json::to_string(&results)
            .map_err(|_| ApiError::BadRequest("Serialization failed".to_string()))
    })
}

pub fn handle_files_read(body: &str) -> String {
    handle_encrypted_api(body, |parsed| {
        let file_path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
        validate_path(file_path).map_err(ApiError::from)?;
        let bytes = fs::read(file_path).map_err(|_| ApiError::IoError)?;
        let mime = utilities::determine_mime(file_path);
        let b64 = utilities::base64_encode(&bytes);
        serde_json::to_string(&serde_json::json!({"content": b64, "mime": mime, "binary": true}))
            .map_err(|_| ApiError::BadRequest("Serialization failed".to_string()))
    })
}

pub fn handle_files_binary(body: &str, stream: &mut std::net::TcpStream) -> Option<String> {
    let payload = serde_json::from_str::<ApiPayload>(body).ok()?;
    let decrypted = decrypt_api_data(&payload.data)?;
    let parsed = serde_json::from_str::<serde_json::Value>(&decrypted).ok()?;

    let file_path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if file_path.contains("..") {
        return Some(ApiError::Forbidden.to_response("403"));
    }

    let bytes = fs::read(file_path).map_err(|_| ApiError::IoError).ok()?;
    let encrypted = encrypt_api_binary_response_simple(&bytes)?;

    let resp = responses::ok_octet_stream(&encrypted);
    let _ = stream.write(&resp);
    let _ = stream.flush();
    None
}

pub fn handle_files_delete(body: &str) -> String {
    handle_encrypted_api(body, |parsed| {
        let file_path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
        validate_path(file_path).map_err(ApiError::from)?;
        if file_path == "/" {
            return Err(ApiError::Forbidden);
        }
        let p = Path::new(file_path);
        let result = if p.is_dir() {
            fs::remove_dir_all(p)
        } else {
            fs::remove_file(p)
        };
        result.map_err(|_| ApiError::IoError)?;
        Ok(r#"{"success":true}"#.to_string())
    })
}

pub fn handle_files_write(body: &str) -> String {
    handle_encrypted_api(body, |parsed| {
        let file_path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let content = parsed.get("content").and_then(|v| v.as_str()).unwrap_or("");
        validate_path(file_path).map_err(ApiError::from)?;
        fs::write(file_path, content).map_err(|_| ApiError::IoError)?;
        Ok(r#"{"success":true}"#.to_string())
    })
}

pub fn handle_files_create_dir(body: &str) -> String {
    handle_encrypted_api(body, |parsed| {
        let dir_path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
        validate_path(dir_path).map_err(ApiError::from)?;
        fs::create_dir_all(dir_path).map_err(|_| ApiError::IoError)?;
        Ok(r#"{"success":true}"#.to_string())
    })
}

pub fn handle_files_rename(body: &str) -> String {
    handle_encrypted_api(body, |parsed| {
        let old_path = parsed
            .get("old_path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let new_path = parsed
            .get("new_path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        validate_path(old_path).map_err(ApiError::from)?;
        validate_path(new_path).map_err(ApiError::from)?;
        fs::rename(old_path, new_path).map_err(|_| ApiError::IoError)?;
        Ok(r#"{"success":true}"#.to_string())
    })
}

pub fn handle_files_copy(body: &str) -> String {
    handle_encrypted_api(body, |parsed| {
        let source = parsed.get("source").and_then(|v| v.as_str()).unwrap_or("");
        let dest = parsed.get("dest").and_then(|v| v.as_str()).unwrap_or("");
        validate_path(source).map_err(ApiError::from)?;
        validate_path(dest).map_err(ApiError::from)?;
        fs::copy(source, dest).map_err(|_| ApiError::IoError)?;
        Ok(r#"{"success":true}"#.to_string())
    })
}

pub fn handle_files_move(body: &str) -> String {
    handle_encrypted_api(body, |parsed| {
        let source = parsed.get("source").and_then(|v| v.as_str()).unwrap_or("");
        let dest = parsed.get("dest").and_then(|v| v.as_str()).unwrap_or("");
        validate_path(source).map_err(ApiError::from)?;
        validate_path(dest).map_err(ApiError::from)?;
        fs::rename(source, dest).map_err(|_| ApiError::IoError)?;
        Ok(r#"{"success":true}"#.to_string())
    })
}

pub fn handle_files_download(body: &str, stream: &mut std::net::TcpStream) -> Option<String> {
    let payload = serde_json::from_str::<ApiPayload>(body).ok()?;
    let decrypted = decrypt_api_data(&payload.data)?;
    let parsed = serde_json::from_str::<serde_json::Value>(&decrypted).ok()?;

    let file_path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if file_path.contains("..") {
        return Some(ApiError::Forbidden.to_response("403"));
    }

    let p = Path::new(file_path);
    if !p.exists() {
        return Some(ApiError::NotFound.to_response("404"));
    }

    if p.is_dir() {
        let mut buffer = Vec::new();
        {
            let mut zip_writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buffer));
            let options = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            add_folder_to_zip(p, "", &mut zip_writer, options);
            let _ = zip_writer.finish();
        }
        let filename = p.file_name().unwrap_or_default().to_string_lossy();
        let resp = responses::ok_zip(&buffer, &filename);
        let _ = stream.write(&resp);
        let _ = stream.flush();
        return None;
    }

    let bytes = fs::read(file_path).map_err(|_| ApiError::IoError).ok()?;
    let mime = utilities::determine_mime(file_path);
    let resp = responses::ok_binary_with_body(&bytes, &mime);
    let _ = stream.write(&resp);
    let _ = stream.flush();
    None
}

pub fn handle_encrypted_api<F>(body: &str, handler: F) -> String
where
    F: FnOnce(serde_json::Value) -> Result<String, ApiError>,
{
    let payload = match serde_json::from_str::<ApiPayload>(body) {
        Ok(p) => p,
        Err(_) => return utilities::error_response("Invalid request", "400"),
    };
    let decrypted = match decrypt_api_data(&payload.data) {
        Some(d) => d,
        None => return ApiError::DecryptionFailed.to_response("400"),
    };
    let parsed = match serde_json::from_str::<serde_json::Value>(&decrypted) {
        Ok(p) => p,
        Err(_) => return ApiError::InvalidDecryptedData.to_response("400"),
    };
    match handler(parsed) {
        Ok(body_str) => encrypt_api_response(&body_str).map_or_else(
            || ApiError::EncryptionError.to_response("500"),
            |enc| utilities::json_response(&enc, "200"),
        ),
        Err(e) => ApiError::from(e).to_response("400"),
    }
}
