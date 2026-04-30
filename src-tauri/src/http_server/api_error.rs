use crate::utilities;

#[derive(Debug, Clone)]
pub enum ApiError {
    BadRequest(String),
    Forbidden,
    NotFound,
    IoError,
    EncryptionError,
    DecryptionFailed,
    InvalidDecryptedData,
}

impl From<ApiError> for String {
    fn from(err: ApiError) -> Self {
        match err {
            ApiError::BadRequest(msg) => msg,
            ApiError::Forbidden => "Forbidden".to_string(),
            ApiError::NotFound => "File not found".to_string(),
            ApiError::IoError => "IO error".to_string(),
            ApiError::EncryptionError => "Encryption failed".to_string(),
            ApiError::DecryptionFailed => "Decryption failed".to_string(),
            ApiError::InvalidDecryptedData => "Invalid decrypted data".to_string(),
        }
    }
}

impl ApiError {
    pub fn to_response(&self, code: &str) -> Vec<u8> {
        let err: ApiError = self.clone();
        let msg: String = err.into();
        utilities::error_response(&msg, code)
    }
}
