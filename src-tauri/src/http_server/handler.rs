use crate::http_server::files;
use crate::http_server::responses;
use crate::http_server::session;
use crate::http_server::system;
use crate::http_server::upload;
use crate::utilities;
use std::io::{Read, Write};
use std::net::TcpStream;

pub fn handle_request(mut stream: TcpStream) {
    if let Ok(addr) = stream.peer_addr() {
        let ip = addr.ip();
        let is_localhost = ip.is_loopback();
        let is_192168 = match ip {
            std::net::IpAddr::V4(v4) => v4.octets()[0] == 192 && v4.octets()[1] == 168,
            _ => false,
        };
        if !is_localhost && !is_192168 {
            let _ = stream.write_all(responses::forbidden().as_bytes());
            return;
        }
    }
    let mut buffer = [0; 8192];
    let n = match stream.read(&mut buffer) {
        Ok(n) if n > 0 => n,
        _ => return,
    };

    let request = String::from_utf8_lossy(&buffer[..n]);
    let lines: Vec<&str> = request.lines().collect();
    let (method, path, _version) = if lines.len() >= 1 {
        let parts: Vec<&str> = lines[0].split_whitespace().collect();
        (
            *parts.get(0).unwrap_or(&""),
            *parts.get(1).unwrap_or(&""),
            *parts.get(2).unwrap_or(&""),
        )
    } else {
        ("", "", "")
    };

    let mut accept_encoding = "";
    for line in &lines[1..] {
        let l = line.trim();
        if let Some(rest) = l
            .strip_prefix("Accept-Encoding:")
            .or_else(|| l.strip_prefix("accept-encoding:"))
        {
            accept_encoding = rest.trim();
            break;
        }
    }

    let req_path: &str = path;
    let body_start = request.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
    let body = if body_start < n {
        &buffer[body_start..n]
    } else {
        &[]
    };

    let query = req_path.split('?').nth(1).unwrap_or("");
    let clean_path = req_path.split('?').next().unwrap_or("/");

    let params = utilities::parse_query_params(query);
    let response: Vec<u8>;

    if method == "OPTIONS" {
        response = responses::no_content().into_bytes();
    } else {
        match (method, clean_path) {
            ("POST", "/api/session/verify") => {
                response = session::handle_session_verify(body);
            }
            ("POST", "/api/session/decrypt") => {
                response = session::handle_session_decrypt(body);
            }

            ("POST", "/api/system/home") => {
                response = system::handle_system_home(body);
            }
            ("POST", "/api/system/drives") => {
                response = system::handle_system_drives(body);
            }
            ("POST", "/api/system/processes") => {
                response = system::handle_system_processes(body);
            }

            ("POST", "/api/files/list") => {
                response = files::handle_files_list(body);
            }
            ("POST", "/api/files/info") => {
                response = files::handle_files_info(body);
            }
            ("POST", "/api/files/search") => {
                response = files::handle_files_search(body);
            }
            ("POST", "/api/files/read") => {
                response = files::handle_files_read(body);
            }
            ("POST", "/api/files/binary") => {
                if let Some(resp) = files::handle_files_binary(body, &mut stream) {
                    response = resp;
                } else {
                    return;
                }
            }
            ("POST", "/api/files/delete") => {
                response = files::handle_files_delete(body);
            }
            ("POST", "/api/files/write") => {
                response = files::handle_files_write(body);
            }
            ("POST", "/api/files/create-dir") => {
                response = files::handle_files_create_dir(body);
            }
            ("POST", "/api/files/rename") => {
                response = files::handle_files_rename(body);
            }
            ("POST", "/api/files/copy") => {
                response = files::handle_files_copy(body);
            }
            ("POST", "/api/files/move") => {
                response = files::handle_files_move(body);
            }
            ("POST", "/api/files/download") => {
                if let Some(resp) = files::handle_files_download(body, &mut stream) {
                    response = resp;
                } else {
                    return;
                }
            }
            ("POST", "/api/fileupload/binary") => {
                if let Some(resp) = upload::handle_upload_binary(body, &mut stream) {
                    response = resp;
                } else {
                    return;
                }
            }

            ("GET", _) if req_path.starts_with("/api/greet") => {
                let name = params.get("name").map(|s| s.as_str()).unwrap_or("World");
                let body = rmp_serde::to_vec(&rmpv::Value::Map(vec![(
                    rmpv::Value::String("message".into()),
                    rmpv::Value::String(format!("Hello, {}! (from Rust HTTP API)", name).into()),
                )]))
                .unwrap();
                response = utilities::msgpack_response(&body, "200");
            }
            #[cfg(all(feature = "expose_shared_key_api", debug_assertions))]
            ("GET", "/api/shared-key") => {
                let body = rmp_serde::to_vec(&rmpv::Value::Map(vec![(
                    rmpv::Value::String("shared_key".into()),
                    rmpv::Value::String(crate::shared::SHARED_KEY.lock().unwrap().clone().into()),
                )]))
                .unwrap();
                response = utilities::msgpack_response(&body, "200");
            }
            ("GET", _) => {
                if let Some((data, mime, encoding)) =
                    utilities::serve_file(req_path, accept_encoding)
                {
                    if let Some(enc) = encoding {
                        response = responses::ok_compressed(&data, mime, enc);
                    } else {
                        response = responses::ok_binary_with_body(&data, mime);
                    }
                } else if let Some((data, mime, encoding)) =
                    utilities::serve_file("/", accept_encoding)
                {
                    if let Some(enc) = encoding {
                        response = responses::ok_compressed(&data, mime, enc);
                    } else {
                        response = responses::ok_binary_with_body(&data, mime);
                    }
                } else {
                    response = responses::not_found().into_bytes();
                }
            }

            _ => {
                response = responses::method_not_allowed().into_bytes();
            }
        }
    }

    let _ = stream.write(&response);
    let _ = stream.flush();
}
