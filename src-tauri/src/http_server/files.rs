use crate::crypto::common::{
    decrypt_api_data_raw, encrypt_api_binary_response_simple, encrypt_api_response_raw,
};
use crate::http_server::api_error::ApiError;
use crate::http_server::responses;
use crate::utilities;
use rmpv::Value;
use std::fs;
use std::io::Write;
use std::path::Path;

// Helper trait to add .get() method to rmpv::Value
pub trait ValueExt {
    fn get(&self, key: &str) -> Option<&Value>;
}

impl ValueExt for Value {
    fn get(&self, key: &str) -> Option<&Value> {
        self.as_map().and_then(|m| {
            m.iter().find_map(|(k, v)| {
                if k.as_str() == Some(key) {
                    Some(v)
                } else {
                    None
                }
            })
        })
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct FileEntry {
    is_dir: bool,
    modified: Option<u64>,
    name: String,
    path: String,
    size: u64,
    owner: Option<String>,
    permissions: Option<String>,
}

pub fn validate_path(path: &str) -> Result<(), ApiError> {
    if path.contains("..") {
        Err(ApiError::Forbidden)
    } else {
        Ok(())
    }
}

pub fn search_files(dir: &Path, pattern: &str) -> Vec<FileEntry> {
    let mut results = Vec::new();
    let pattern_lower = pattern.to_lowercase();
    fn walk_dir(dir: &Path, pat: &str, results: &mut Vec<FileEntry>) {
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
                            .map(|d| d.as_secs());
                        let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                        let (owner, permissions) = metadata
                            .as_ref()
                            .map(|m| get_owner_and_permissions(m))
                            .unwrap_or((None, None));
                        results.push(FileEntry {
                            name: name.to_string(),
                            path: path.to_string_lossy().to_string(),
                            is_dir,
                            size,
                            modified,
                            owner,
                            permissions,
                        });
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

pub fn handle_files_list(body: &[u8]) -> Vec<u8> {
    handle_encrypted_api(body, |parsed| {
        let dir = parsed.get("dir").and_then(|v| v.as_str()).unwrap_or("/");
        validate_path(dir).map_err(ApiError::from)?;
        let entries = fs::read_dir(Path::new(dir)).map_err(|_| ApiError::IoError)?;
        let mut items: Vec<FileEntry> = entries
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
                    .map(|d: std::time::Duration| d.as_secs());
                let is_dir = metadata.is_dir();
                let (owner, permissions) = get_owner_and_permissions(&metadata);
                Some(FileEntry {
                    name,
                    path: path.to_string_lossy().to_string(),
                    is_dir,
                    size,
                    modified,
                    owner,
                    permissions,
                })
            })
            .collect();
        items.sort_by(|a: &FileEntry, b: &FileEntry| {
            if a.is_dir != b.is_dir {
                return b.is_dir.cmp(&a.is_dir);
            }
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        });
        #[derive(serde::Serialize)]
        struct ListResponse {
            items: Vec<FileEntry>,
        }
        rmp_serde::encode::to_vec_named(&ListResponse { items })
            .map_err(|_| ApiError::BadRequest("Serialization failed".to_string()))
    })
}

pub fn handle_files_info(body: &[u8]) -> Vec<u8> {
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
            .map(|d: std::time::Duration| d.as_secs());
        let is_dir = metadata.is_dir();
        let (owner, permissions) = get_owner_and_permissions(&metadata);
        let entry = FileEntry {
            name,
            path: file_path.to_string(),
            is_dir,
            size,
            modified,
            owner,
            permissions,
        };
        rmp_serde::encode::to_vec_named(&entry)
            .map_err(|_| ApiError::BadRequest("Serialization failed".to_string()))
    })
}

fn get_owner_and_permissions(metadata: &fs::Metadata) -> (Option<String>, Option<String>) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let uid = metadata.uid();
        let gid = metadata.gid();
        let mode = metadata.mode();
        let permissions = Some(format!("{:o}", mode & 0o777));
        let owner = Some(format!("{}:{}", uid, gid));
        (owner, permissions)
    }
    #[cfg(not(unix))]
    {
        (None, None)
    }
}

pub fn handle_files_search(body: &[u8]) -> Vec<u8> {
    handle_encrypted_api(body, |parsed| {
        let dir = parsed.get("dir").and_then(|v| v.as_str()).unwrap_or("/");
        let pattern = parsed.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
        validate_path(dir).map_err(ApiError::from)?;
        let results = search_files(Path::new(dir), pattern);
        rmp_serde::encode::to_vec_named(&results)
            .map_err(|_| ApiError::BadRequest("Serialization failed".to_string()))
    })
}

pub fn handle_files_read(body: &[u8]) -> Vec<u8> {
    handle_encrypted_api(body, |parsed| {
        let file_path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
        validate_path(file_path).map_err(ApiError::from)?;
        let bytes = fs::read(file_path).map_err(|_| ApiError::IoError)?;
        let mime = utilities::determine_mime(file_path);
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&bytes).map_err(|_| ApiError::IoError)?;
        let compressed = encoder.finish().map_err(|_| ApiError::IoError)?;
        let response = Value::Map(vec![
            (Value::String("content".into()), Value::Binary(compressed)),
            (Value::String("mime".into()), Value::String(mime.into())),
            (Value::String("binary".into()), Value::Boolean(true)),
            (
                Value::String("compression".into()),
                Value::String("gzip".into()),
            ),
        ]);
        rmp_serde::to_vec(&response)
            .map_err(|_| ApiError::BadRequest("Serialization failed".to_string()))
    })
}

pub fn handle_files_binary(body: &[u8], stream: &mut std::net::TcpStream) -> Option<Vec<u8>> {
    // Parse MessagePack body: {"data": <binary>}
    let payload: Value = rmp_serde::from_slice(body).ok()?;
    let data = payload.get("data")?.as_array()?;
    let encrypted_bytes: Vec<u8> = data
        .iter()
        .filter_map(|v| v.as_u64().map(|n| n as u8))
        .collect();
    let decrypted = decrypt_api_data_raw(&encrypted_bytes)?;
    let parsed: Value = rmp_serde::from_slice(&decrypted).ok()?;

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

pub fn handle_files_delete(body: &[u8]) -> Vec<u8> {
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
        let response = Value::Map(vec![(
            Value::String("success".into()),
            Value::Boolean(true),
        )]);
        rmp_serde::to_vec(&response)
            .map_err(|_| ApiError::BadRequest("Serialization failed".to_string()))
    })
}

pub fn handle_files_write(body: &[u8]) -> Vec<u8> {
    handle_encrypted_api(body, |parsed| {
        let file_path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let content = parsed.get("content").and_then(|v| v.as_str()).unwrap_or("");
        validate_path(file_path).map_err(ApiError::from)?;
        fs::write(file_path, content).map_err(|_| ApiError::IoError)?;
        let response = Value::Map(vec![(
            Value::String("success".into()),
            Value::Boolean(true),
        )]);
        rmp_serde::to_vec(&response)
            .map_err(|_| ApiError::BadRequest("Serialization failed".to_string()))
    })
}

pub fn handle_files_create_dir(body: &[u8]) -> Vec<u8> {
    handle_encrypted_api(body, |parsed| {
        let dir_path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
        validate_path(dir_path).map_err(ApiError::from)?;
        fs::create_dir_all(dir_path).map_err(|_| ApiError::IoError)?;
        let response = Value::Map(vec![(
            Value::String("success".into()),
            Value::Boolean(true),
        )]);
        rmp_serde::to_vec(&response)
            .map_err(|_| ApiError::BadRequest("Serialization failed".to_string()))
    })
}

pub fn handle_files_rename(body: &[u8]) -> Vec<u8> {
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
        let response = Value::Map(vec![(
            Value::String("success".into()),
            Value::Boolean(true),
        )]);
        rmp_serde::to_vec(&response)
            .map_err(|_| ApiError::BadRequest("Serialization failed".to_string()))
    })
}

pub fn handle_files_copy(body: &[u8]) -> Vec<u8> {
    handle_encrypted_api(body, |parsed| {
        let source = parsed.get("source").and_then(|v| v.as_str()).unwrap_or("");
        let dest = parsed.get("dest").and_then(|v| v.as_str()).unwrap_or("");
        validate_path(source).map_err(ApiError::from)?;
        validate_path(dest).map_err(ApiError::from)?;
        fs::copy(source, dest).map_err(|_| ApiError::IoError)?;
        let response = Value::Map(vec![(
            Value::String("success".into()),
            Value::Boolean(true),
        )]);
        rmp_serde::to_vec(&response)
            .map_err(|_| ApiError::BadRequest("Serialization failed".to_string()))
    })
}

pub fn handle_files_move(body: &[u8]) -> Vec<u8> {
    handle_encrypted_api(body, |parsed| {
        let source = parsed.get("source").and_then(|v| v.as_str()).unwrap_or("");
        let dest = parsed.get("dest").and_then(|v| v.as_str()).unwrap_or("");
        validate_path(source).map_err(ApiError::from)?;
        validate_path(dest).map_err(ApiError::from)?;
        fs::rename(source, dest).map_err(|_| ApiError::IoError)?;
        let response = Value::Map(vec![(
            Value::String("success".into()),
            Value::Boolean(true),
        )]);
        rmp_serde::to_vec(&response)
            .map_err(|_| ApiError::BadRequest("Serialization failed".to_string()))
    })
}

pub fn handle_files_download(body: &[u8], stream: &mut std::net::TcpStream) -> Option<Vec<u8>> {
    // Parse MessagePack body: {"data": <binary>}
    let payload: Value = rmp_serde::from_slice(body).ok()?;
    let data = payload.get("data")?.as_array()?;
    let encrypted_bytes: Vec<u8> = data
        .iter()
        .filter_map(|v| v.as_u64().map(|n| n as u8))
        .collect();
    let decrypted = decrypt_api_data_raw(&encrypted_bytes)?;
    let parsed: Value = rmp_serde::from_slice(&decrypted).ok()?;

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

pub fn handle_encrypted_api<F>(body: &[u8], handler: F) -> Vec<u8>
where
    F: FnOnce(Value) -> Result<Vec<u8>, ApiError>,
{
    // Parse MessagePack body: {"data": <binary>}
    let payload: Value = match rmp_serde::from_slice(body) {
        Ok(p) => p,
        Err(_) => return utilities::error_response("Invalid request", "400"),
    };
    let data = match payload.get("data").and_then(|v| v.as_array()) {
        Some(arr) => {
            let bytes: Vec<u8> = arr
                .iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect();
            bytes
        }
        None => return utilities::error_response("Invalid request", "400"),
    };

    let decrypted = match decrypt_api_data_raw(&data) {
        Some(d) => d,
        None => return ApiError::DecryptionFailed.to_response("400"),
    };
    let parsed: Value = match rmp_serde::from_slice(&decrypted) {
        Ok(p) => p,
        Err(_) => return ApiError::InvalidDecryptedData.to_response("400"),
    };
    match handler(parsed) {
        Ok(body_bytes) => encrypt_api_response_raw(&body_bytes).map_or_else(
            || ApiError::EncryptionError.to_response("500"),
            |enc| utilities::msgpack_response(&enc, "200"),
        ),
        Err(e) => ApiError::from(e).to_response("400"),
    }
}
