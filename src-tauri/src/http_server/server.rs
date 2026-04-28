use crate::crypto::common::{SERVER_PORT, SERVER_RUNNING};
use std::net::TcpListener;
use std::sync::atomic::Ordering;
use std::thread;

pub fn spawn_http_server(port: u16) -> Result<(), String> {
    SERVER_PORT.store(port, Ordering::SeqCst);
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
        .map_err(|e| format!("Failed to bind to port {}: {}", port, e))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("Failed to set nonblocking: {}", e))?;
    SERVER_RUNNING.store(true, Ordering::SeqCst);
    thread::spawn(move || {
        println!("Rust HTTP server listening on http://127.0.0.1:{}", port);
        loop {
            if !SERVER_RUNNING.load(Ordering::SeqCst) {
                break;
            }
            match listener.accept() {
                Ok((s, _)) => super::handler::handle_request(s),
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => eprintln!("Connection failed: {}", e),
            }
        }
        println!("Rust HTTP server stopped");
    });
    Ok(())
}

pub fn start_http_server(port: u16) -> Result<(), String> {
    if SERVER_RUNNING.load(Ordering::SeqCst) {
        return Err("Server already running".to_string());
    }
    spawn_http_server(port)?;
    Ok(())
}

#[tauri::command]
pub fn toggle_http_server(port: Option<u16>) -> Result<String, String> {
    if SERVER_RUNNING.load(Ordering::SeqCst) {
        SERVER_RUNNING.store(false, Ordering::SeqCst);
        Ok("Stopped".to_string())
    } else {
        let port = port.unwrap_or_else(|| SERVER_PORT.load(Ordering::SeqCst));
        match spawn_http_server(port) {
            Ok(()) => {
                SERVER_PORT.store(port, Ordering::SeqCst);
                Ok("Running".to_string())
            }
            Err(e) => Err(e),
        }
    }
}

#[tauri::command]
pub fn stop_http_server() -> Result<bool, String> {
    if SERVER_RUNNING.load(Ordering::SeqCst) {
        SERVER_RUNNING.store(false, Ordering::SeqCst);
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
pub fn is_server_running() -> bool {
    SERVER_RUNNING.load(Ordering::SeqCst)
}

#[tauri::command]
pub fn get_server_port() -> u16 {
    SERVER_PORT.load(Ordering::SeqCst)
}

#[tauri::command]
pub fn set_server_port(port: u16) -> Result<(), String> {
    let running = SERVER_RUNNING.load(Ordering::SeqCst);
    if running {
        return Err("Stop server first before changing port".to_string());
    }
    SERVER_PORT.store(port, Ordering::SeqCst);
    Ok(())
}
