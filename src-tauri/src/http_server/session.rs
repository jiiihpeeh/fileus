use crate::crypto::common::decrypt_aes_gcm_raw;
use crate::http_server::api_error::ApiError;
use crate::http_server::files::ValueExt;
use crate::shared::{SESSION_NEW_KEY, SHARED_KEY};
use crate::utilities;
use rmpv::Value;

pub fn handle_session_verify(body: &[u8]) -> Vec<u8> {
    let key = SHARED_KEY.lock().unwrap();
    if key.is_empty() {
        return ApiError::BadRequest("No shared key set".to_string()).to_response("400");
    }

    // Parse MessagePack body: {"data": [byte, byte, ...]} - raw binary
    let payload: Value = match rmp_serde::from_slice(body) {
        Ok(p) => p,
        Err(_) => return utilities::error_response("Invalid request body", "400"),
    };
    let data = match payload.get("data").and_then(|v| v.as_array()) {
        Some(arr) => {
            let bytes: Vec<u8> = arr
                .iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect();
            bytes
        }
        None => return utilities::error_response("Invalid request body", "400"),
    };

    match decrypt_aes_gcm_raw(&key, &data) {
        Ok(vec) if vec.len() >= 2 => {
            *SESSION_NEW_KEY.lock().unwrap() = vec[1].clone();
            let body = rmp_serde::to_vec(&Value::Map(vec![(
                Value::String("valid".into()),
                Value::Boolean(true),
            )]))
            .unwrap_or_default();
            utilities::msgpack_response(&body, "200")
        }
        Ok(_) => ApiError::BadRequest("Invalid payload".to_string()).to_response("400"),
        Err(e) => ApiError::BadRequest(e).to_response("400"),
    }
}

pub fn handle_session_decrypt(body: &[u8]) -> Vec<u8> {
    let new_key = SESSION_NEW_KEY.lock().unwrap();
    if new_key.is_empty() {
        return ApiError::BadRequest("No session key".to_string()).to_response("400");
    }

    // Parse MessagePack body: {"data": [byte, byte, ...]} - raw binary
    let payload: Value = match rmp_serde::from_slice(body) {
        Ok(p) => p,
        Err(_) => return utilities::error_response("Invalid request body", "400"),
    };
    let data = match payload.get("data").and_then(|v| v.as_array()) {
        Some(arr) => {
            let bytes: Vec<u8> = arr
                .iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect();
            bytes
        }
        None => return utilities::error_response("Invalid request body", "400"),
    };

    match decrypt_aes_gcm_raw(&new_key, &data) {
        Ok(vec) if vec.len() >= 2 => {
            let body = rmp_serde::to_vec(&Value::Map(vec![(
                Value::String("payload".into()),
                Value::String(vec[1].clone().into()),
            )]))
            .unwrap_or_default();
            utilities::msgpack_response(&body, "200")
        }
        Ok(_) => ApiError::BadRequest("Invalid payload".to_string()).to_response("400"),
        Err(e) => ApiError::BadRequest(e).to_response("400"),
    }
}
