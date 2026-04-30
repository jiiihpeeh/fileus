use std::fs;
use std::path::PathBuf;

pub fn determine_mime(path: &str) -> &'static str {
    match path
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "html" => "text/html",
        "js" => "application/javascript",
        "mjs" => "application/javascript",
        "css" => "text/css",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "wasm" => "application/wasm",
        "txt" => "text/plain",
        _ => "application/octet-stream",
    }
}

fn find_dist_dir() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let candidates = [
        cwd.join("web-frontend/dist"),
        cwd.join("../web-frontend/dist"),
    ];
    candidates.into_iter().find(|pb| pb.exists())
}

pub fn get_preferred_encoding(accept_encoding: &str) -> &'static str {
    if accept_encoding.is_empty() {
        return "";
    }
    let mut has_br = false;
    let mut has_gzip = false;
    for part in accept_encoding.split(',') {
        let enc = part.trim().split(';').next().unwrap_or("").trim();
        match enc {
            "br" => has_br = true,
            "gzip" | "deflate" => has_gzip = true,
            "*" => {
                if !has_br && !has_gzip {
                    has_br = true;
                }
            }
            _ => {}
        }
    }
    if has_br {
        "br"
    } else if has_gzip {
        "gzip"
    } else {
        ""
    }
}

pub fn serve_file(
    path: &str,
    accept_encoding: &str,
) -> Option<(Vec<u8>, &'static str, Option<&'static str>)> {
    let dist_dir = find_dist_dir()?;

    let fs_path = if path == "/" || path.is_empty() {
        dist_dir.join("index.html")
    } else {
        let safe_path = path.trim_start_matches('/');
        if safe_path.contains("..") {
            return None;
        }
        dist_dir.join(safe_path)
    };

    let encoding = get_preferred_encoding(accept_encoding);

    if !encoding.is_empty() {
        let ext = match encoding {
            "br" => ".br",
            "gzip" => ".gz",
            _ => "",
        };
        let compressed_path = format!("{}{}", fs_path.to_string_lossy(), ext);
        if let Ok(data) = fs::read(&compressed_path) {
            let mime = determine_mime(fs_path.to_str().unwrap_or(""));
            return Some((data, mime, Some(encoding)));
        }
    }

    let data = fs::read(&fs_path).ok()?;
    let mime = determine_mime(fs_path.to_str().unwrap_or(""));
    Some((data, mime, None))
}

pub fn parse_query_params(query: &str) -> std::collections::HashMap<String, String> {
    let mut params = std::collections::HashMap::new();
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            params.insert(k.to_string(), urlencoding_decode(v));
        }
    }
    params
}

pub fn urlencoding_decode(s: &str) -> String {
    s.replace("+", " ")
        .split('%')
        .enumerate()
        .map(|(i, part)| {
            if i == 0 {
                part.to_string()
            } else if part.len() >= 2 {
                let hex = &part[..2];
                let remainder = &part[2..];
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    format!("{}{}", (byte as char).to_string(), remainder)
                } else {
                    format!("%{}", part)
                }
            } else {
                format!("%{}", part)
            }
        })
        .collect()
}

pub fn msgpack_response(data: &[u8], status: &str) -> Vec<u8> {
    let header = format!(
        "HTTP/1.1 {} OK\r\nContent-Type: application/msgpack\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        status,
        data.len()
    );
    let mut response = Vec::with_capacity(header.len() + data.len());
    response.extend_from_slice(header.as_bytes());
    response.extend_from_slice(data);
    response
}

pub fn error_response(msg: &str, status: &str) -> Vec<u8> {
    let body = rmp_serde::to_vec(&rmpv::Value::Map(vec![(
        rmpv::Value::String("error".into()),
        rmpv::Value::String(msg.into()),
    )]))
    .unwrap_or_default();
    let header = format!(
        "HTTP/1.1 {} OK\r\nContent-Type: application/msgpack\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        status,
        body.len()
    );
    let mut response = header.into_bytes();
    response.extend(body);
    response
}

pub fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b = match chunk.len() {
            1 => [chunk[0], 0, 0],
            2 => [chunk[0], chunk[1], 0],
            _ => [chunk[0], chunk[1], chunk[2]],
        };
        result.push(CHARS[(b[0] >> 2) as usize] as char);
        result.push(CHARS[((b[0] & 0x03) << 4 | b[1] >> 4) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((b[1] & 0x0f) << 2 | b[2] >> 6) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(b[2] & 0x3f) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}
