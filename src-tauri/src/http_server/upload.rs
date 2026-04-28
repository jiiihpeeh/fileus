use crate::crypto::common::{decrypt_api_data, encrypt_api_binary_response};
use crate::http_server::api_error::ApiError;
use crate::http_server::responses;
use std::fs;
use std::io::Write;
use std::path::Path;

const CHUNK_SIZE: usize = 2 * 1024 * 1024;

pub fn handle_upload_binary(body: &str, stream: &mut std::net::TcpStream) -> Option<String> {
    let payload = serde_json::from_str::<ApiPayload>(body).ok()?;
    let decrypted = decrypt_api_data(&payload.data)?;
    let parsed = serde_json::from_str::<serde_json::Value>(&decrypted).ok()?;

    let file_path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if file_path.contains("..") {
        return Some(ApiError::Forbidden.to_response("403"));
    }

    let p = Path::new(file_path);
    if !p.exists() || p.is_dir() {
        return Some(ApiError::NotFound.to_response("404"));
    }

    let bytes = fs::read(file_path).map_err(|_| ApiError::IoError).ok()?;
    let file_size = bytes.len();
    let chunk_index = parsed
        .get("chunk_index")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;
    let total_chunks = parsed
        .get("total_chunks")
        .and_then(|v| v.as_u64())
        .unwrap_or(1) as usize;

    let start = chunk_index * CHUNK_SIZE;
    if start >= file_size {
        return Some(
            ApiError::BadRequest("Chunk index out of range".to_string()).to_response("400"),
        );
    }

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

    let encrypted = encrypt_api_binary_response(&metadata, &chunk_data)?;
    let resp = responses::ok_octet_stream(&encrypted);
    let _ = stream.write(&resp);
    let _ = stream.flush();
    None
}

#[derive(serde::Deserialize)]
struct ApiPayload {
    data: String,
}
