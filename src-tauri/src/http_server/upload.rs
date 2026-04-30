use crate::crypto::common::{decrypt_api_data_raw, encrypt_api_binary_response};
use crate::http_server::api_error::ApiError;
use crate::http_server::responses;
use crate::utilities;
use rmpv::Value;
use std::fs;
use std::io::Write;
use std::path::Path;

const CHUNK_SIZE: usize = 2 * 1024 * 1024;

fn get_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.as_map().and_then(|m| {
        m.iter().find_map(|(k, v)| {
            if k.as_str() == Some(key) {
                v.as_str()
            } else {
                None
            }
        })
    })
}

fn get_u64(value: &Value, key: &str) -> Option<u64> {
    value.as_map().and_then(|m| {
        m.iter().find_map(|(k, v)| {
            if k.as_str() == Some(key) {
                v.as_u64()
            } else {
                None
            }
        })
    })
}

pub fn handle_upload_binary(body: &[u8], stream: &mut std::net::TcpStream) -> Option<Vec<u8>> {
    // Parse MessagePack body: {"data": <binary>}
    let payload: Value = match rmp_serde::from_slice(body) {
        Ok(p) => p,
        Err(_) => return Some(utilities::error_response("Invalid request", "400")),
    };
    let data = match payload.as_map().and_then(|m| {
        m.iter().find_map(|(k, v)| {
            if k.as_str() == Some("data") {
                v.as_array()
            } else {
                None
            }
        })
    }) {
        Some(arr) => {
            let bytes: Vec<u8> = arr
                .iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect();
            bytes
        }
        None => return Some(utilities::error_response("Invalid request", "400")),
    };

    let decrypted = decrypt_api_data_raw(&data)?;
    let parsed: Value = rmp_serde::from_slice(&decrypted).ok()?;

    let file_path = get_str(&parsed, "path").unwrap_or("");
    if file_path.contains("..") {
        return Some(ApiError::Forbidden.to_response("403"));
    }

    let p = Path::new(file_path);
    if !p.exists() || p.is_dir() {
        return Some(ApiError::NotFound.to_response("404"));
    }

    let bytes = fs::read(file_path).map_err(|_| ApiError::IoError).ok()?;
    let file_size = bytes.len();
    let chunk_index = get_u64(&parsed, "chunk_index").unwrap_or(0) as usize;
    let total_chunks = get_u64(&parsed, "total_chunks").unwrap_or(1) as usize;

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

    let metadata = Value::Map(vec![
        (
            Value::String("filename".into()),
            Value::String(filename.into()),
        ),
        (
            Value::String("chunk_index".into()),
            Value::Integer((chunk_index as i64).into()),
        ),
        (
            Value::String("total_chunks".into()),
            Value::Integer((total_chunks as i64).into()),
        ),
        (
            Value::String("file_size".into()),
            Value::Integer((file_size as i64).into()),
        ),
        (
            Value::String("chunk_size".into()),
            Value::Integer(((end - start) as i64).into()),
        ),
    ]);

    let encrypted = encrypt_api_binary_response(&metadata, &chunk_data)?;
    let resp = responses::ok_octet_stream(&encrypted);
    let _ = stream.write(&resp);
    let _ = stream.flush();
    None
}
