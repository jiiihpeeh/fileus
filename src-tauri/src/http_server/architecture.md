# HTTP Server Architecture

## Overview

HTTP server module handling encrypted API requests for file operations.

## Directory Structure

```
src/http_server/
├── mod.rs           # Module declarations
├── api_error.rs     # Error types and response helper
├── responses.rs     # HTTP response builders
├── handler.rs       # Request router
├── session.rs       # Session endpoints
├── system.rs        # System endpoints
├── files.rs         # File operation endpoints
├── upload.rs        # File upload/chunk handling
└── server.rs        # TCP server setup
```

## File Descriptions

### mod.rs
Module declarations. Exports all submodules.

### api_error.rs
Centralized error handling:
- `ApiError` enum: `BadRequest`, `Forbidden`, `NotFound`, `IoError`, `EncryptionError`, `DecryptionFailed`, `InvalidDecryptedData`
- `From<ApiError> for String` - converts error to message
- `to_response(&self, code: &str) -> String` - converts error to HTTP error response

### responses.rs
HTTP response builders:
- `ok(content, content_type)` - 200 with string body
- `ok_html(content)` - 200 HTML response
- `ok_binary(content, content_type)` - 200 binary header only (unused)
- `ok_binary_with_body(content, content_type)` - 200 with binary body
- `ok_octet_stream(content)` - 200 application/octet-stream
- `ok_zip(content, filename)` - 200 zip download with Content-Disposition
- `no_content()` - 204 CORS preflight response
- `forbidden()` - 403 response
- `not_found()` - 404 HTML response
- `method_not_allowed()` - 405 response

### handler.rs
Request router. Main entry point `handle_request(stream)`:
- IP whitelist check (localhost, 192.168.x.x)
- Parses HTTP request, extracts method, path, query, body
- Routes to appropriate handler by `(method, path)` match
- Returns responses via `responses::*` or handler return values

Routes:
- `POST /api/session/*` → session module
- `POST /api/system/*` → system module
- `POST /api/files/*` → files module
- `POST /api/fileupload/*` → upload module
- `GET /api/greet` → greet endpoint
- `GET *` → static file serving

### session.rs
Session management endpoints:
- `handle_session_verify(body)` - verifies and sets session key
- `handle_session_decrypt(body)` - decrypts with session key
- Uses `SHARED_KEY` and `SESSION_NEW_KEY` from `crate::shared`

### system.rs
System information endpoints:
- `handle_system_home()` - returns home directory
- `handle_system_drives()` - returns mounted drives
- `handle_system_processes()` - returns system processes
- `handle_encrypted_api()` - helper for encrypted API handlers

### files.rs
File operation endpoints:
- `validate_path(path)` - checks for `..` traversal
- `search_files(dir, pattern)` - recursive file search
- `add_folder_to_zip()` - recursive zip creation
- `handle_files_list(body)` - list directory contents
- `handle_files_info(body)` - file/directory metadata
- `handle_files_search(body)` - search by pattern
- `handle_files_read(body)` - read file as base64
- `handle_files_binary(body, stream)` - raw binary download
- `handle_files_delete(body)` - delete file/directory
- `handle_files_write(body)` - write file content
- `handle_files_create_dir(body)` - create directory
- `handle_files_rename(body)` - rename file
- `handle_files_copy(body)` - copy file
- `handle_files_move(body)` - move file
- `handle_files_download(body, stream)` - file/zip download
- `handle_encrypted_api()` - shared encrypted request handler

### upload.rs
File upload/chunk handling:
- `handle_upload_binary(body, stream)` - serve file chunks for upload
- `CHUNK_SIZE` = 2MB
- `ApiPayload` struct for deserializing request

## Patterns

### Encrypted API Flow
1. Parse `ApiPayload { data: String }` from body
2. `decrypt_api_data()` to get decrypted JSON string
3. Parse JSON to get request parameters
4. Execute handler logic
5. Encrypt response with `encrypt_api_response()`
6. Return via `utilities::json_response()`

### Error Handling
All errors convert to `ApiError` enum, then to HTTP response via `to_response()`:
```rust
Err(e) => ApiError::from(e).to_response("400")
```

### Binary Responses
Some endpoints write directly to `TcpStream` and return `None`:
```rust
pub fn handle_files_binary(...) -> Option<String> {
    // ...
    let _ = stream.write(&resp);
    let _ = stream.flush();
    None  // response already sent
}
```

## Dependencies
- `crate::crypto::common` - encryption/decryption functions
- `crate::shared` - `SHARED_KEY`, `SESSION_NEW_KEY`
- `crate::utilities` - `error_response`, `json_response`, `determine_mime`, `base64_encode`, `parse_query_params`, `serve_file`
