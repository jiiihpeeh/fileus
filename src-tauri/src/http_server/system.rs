use crate::crypto::common::{decrypt_api_data_raw, encrypt_api_response_raw};
use crate::http_server::api_error::ApiError;
use crate::utilities;
use rmpv::Value;

pub fn handle_system_home(body: &[u8]) -> Vec<u8> {
    handle_encrypted_api(body, |_parsed| {
        let home = dirs::home_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "/".to_string());
        let response = Value::Map(vec![(
            Value::String("path".into()),
            Value::String(home.into()),
        )]);
        rmp_serde::to_vec(&response)
            .map_err(|_| ApiError::BadRequest("Serialization failed".to_string()))
    })
}

pub fn handle_system_drives(body: &[u8]) -> Vec<u8> {
    handle_encrypted_api(body, |_parsed| {
        let mut items = Vec::new();
        #[cfg(unix)]
        {
            items.push(Value::Map(vec![
                (Value::String("name".into()), Value::String("/".into())),
                (Value::String("path".into()), Value::String("/".into())),
                (Value::String("is_dir".into()), Value::Boolean(true)),
                (Value::String("size".into()), Value::Integer(0.into())),
                (Value::String("modified".into()), Value::Nil),
            ]));
            if let Some(home) = dirs::home_dir() {
                items.push(Value::Map(vec![
                    (Value::String("name".into()), Value::String("Home".into())),
                    (
                        Value::String("path".into()),
                        Value::String(home.to_string_lossy().to_string().into()),
                    ),
                    (Value::String("is_dir".into()), Value::Boolean(true)),
                    (Value::String("size".into()), Value::Integer(0.into())),
                    (Value::String("modified".into()), Value::Nil),
                ]));
            }
        }
        rmp_serde::to_vec(&items)
            .map_err(|_| ApiError::BadRequest("Serialization failed".to_string()))
    })
}

pub fn handle_system_processes(body: &[u8]) -> Vec<u8> {
    handle_encrypted_api(body, |_parsed| {
        let sys = sysinfo::System::new_all();
        let mut processes: Vec<Value> = sys
            .processes()
            .iter()
            .map(|(pid, process)| {
                Value::Map(vec![
                    (
                        Value::String("pid".into()),
                        Value::Integer((pid.as_u32() as i64).into()),
                    ),
                    (
                        Value::String("name".into()),
                        Value::String(process.name().to_string().into()),
                    ),
                    (
                        Value::String("cpu".into()),
                        Value::F64(process.cpu_usage() as f64),
                    ),
                    (
                        Value::String("memory".into()),
                        Value::Integer((process.memory() as i64).into()),
                    ),
                ])
            })
            .collect();
        processes.sort_by(|a, b| {
            let a_cpu = a
                .as_map()
                .and_then(|m| {
                    m.iter().find_map(|(k, v)| {
                        if k.as_str() == Some("cpu") {
                            v.as_f64()
                        } else {
                            None
                        }
                    })
                })
                .unwrap_or(0.0);
            let b_cpu = b
                .as_map()
                .and_then(|m| {
                    m.iter().find_map(|(k, v)| {
                        if k.as_str() == Some("cpu") {
                            v.as_f64()
                        } else {
                            None
                        }
                    })
                })
                .unwrap_or(0.0);
            b_cpu
                .partial_cmp(&a_cpu)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        rmp_serde::to_vec(&processes)
            .map_err(|_| ApiError::BadRequest("Serialization failed".to_string()))
    })
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
