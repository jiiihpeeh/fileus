use crate::crypto::common::decrypt_aes_gcm;
use crate::http_server::api_error::ApiError;
use crate::shared::{SESSION_NEW_KEY, SHARED_KEY};
use crate::utilities;

#[derive(serde::Deserialize)]
pub struct ApiPayload {
    pub data: String,
}

pub fn handle_session_verify(body: &str) -> String {
    let key = SHARED_KEY.lock().unwrap();
    if key.is_empty() {
        return ApiError::BadRequest("No shared key set".to_string()).to_response("400");
    }
    match serde_json::from_str::<ApiPayload>(body) {
        Ok(payload) => match decrypt_aes_gcm(&key, &payload.data) {
            Ok(vec) if vec.len() >= 2 => {
                *SESSION_NEW_KEY.lock().unwrap() = vec[1].clone();
                utilities::json_response(r#"{"valid":true}"#, "200")
            }
            Ok(_) => ApiError::BadRequest("Invalid payload".to_string()).to_response("400"),
            Err(e) => ApiError::BadRequest(e).to_response("400"),
        },
        Err(_) => utilities::error_response("Invalid request body", "400"),
    }
}

pub fn handle_session_decrypt(body: &str) -> String {
    let new_key = SESSION_NEW_KEY.lock().unwrap();
    if new_key.is_empty() {
        return ApiError::BadRequest("No session key".to_string()).to_response("400");
    }
    match serde_json::from_str::<ApiPayload>(body) {
        Ok(payload) => match decrypt_aes_gcm(&new_key, &payload.data) {
            Ok(vec) if vec.len() >= 2 => {
                let body = serde_json::to_string(&serde_json::json!({"payload": vec[1]})).unwrap();
                utilities::json_response(&body, "200")
            }
            Ok(_) => ApiError::BadRequest("Invalid payload".to_string()).to_response("400"),
            Err(e) => ApiError::BadRequest(e).to_response("400"),
        },
        Err(_) => utilities::error_response("Invalid request body", "400"),
    }
}
