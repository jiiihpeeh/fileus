use crate::crypto::common::{decrypt_api_data, encrypt_api_response};
use crate::http_server::api_error::ApiError;
use crate::utilities;

pub fn handle_system_home() -> String {
    handle_encrypted_api(&String::new(), |_parsed| {
        let home = dirs::home_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "/".to_string());
        serde_json::to_string(&serde_json::json!({"path": home}))
            .map_err(|_| ApiError::BadRequest("Serialization failed".to_string()))
    })
}

pub fn handle_system_drives() -> String {
    handle_encrypted_api(&String::new(), |_parsed| {
        let mut items = Vec::new();
        #[cfg(unix)]
        {
            items.push(serde_json::json!({"name": "/", "path": "/", "is_dir": true, "size": 0, "modified": null}));
            if let Some(home) = dirs::home_dir() {
                items.push(serde_json::json!({"name": "Home", "path": home.to_string_lossy().to_string(), "is_dir": true, "size": 0, "modified": null}));
            }
        }
        serde_json::to_string(&items)
            .map_err(|_| ApiError::BadRequest("Serialization failed".to_string()))
    })
}

pub fn handle_system_processes() -> String {
    handle_encrypted_api(&String::new(), |_parsed| {
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
        serde_json::to_string(&processes)
            .map_err(|_| ApiError::BadRequest("Serialization failed".to_string()))
    })
}

pub fn handle_encrypted_api<F>(body: &str, handler: F) -> String
where
    F: FnOnce(serde_json::Value) -> Result<String, ApiError>,
{
    #[derive(serde::Deserialize)]
    struct ApiPayload {
        data: String,
    }
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
