// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod crypto;
mod http_server;
mod shared;
mod utilities;

// crypto imports used in other modules
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct GreetResponse {
    message: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct FileItem {
    name: String,
    path: String,
    is_dir: bool,
    size: u64,
    modified: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct SystemInfo {
    os: String,
    arch: String,
    hostname: String,
    cpu_count: usize,
    total_memory: u64,
    free_memory: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ProcessItem {
    pid: u32,
    name: String,
    cpu: f32,
    memory: u64,
}

#[tauri::command]
fn get_binary_file(path: String) -> Result<Vec<u8>, String> {
    if path.contains("..") {
        return Err("Forbidden".to_string());
    }
    fs::read(&path).map_err(|e| format!("Cannot read file: {}", e))
}

#[tauri::command]
fn get_binary_mime(path: String) -> String {
    if path.ends_with(".png") {
        "image/png".to_string()
    } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        "image/jpeg".to_string()
    } else if path.ends_with(".gif") {
        "image/gif".to_string()
    } else if path.ends_with(".webp") {
        "image/webp".to_string()
    } else if path.ends_with(".svg") {
        "image/svg+xml".to_string()
    } else if path.ends_with(".bmp") {
        "image/bmp".to_string()
    } else if path.ends_with(".avif") {
        "image/avif".to_string()
    } else {
        "application/octet-stream".to_string()
    }
}

#[tauri::command]
fn greet_json(name: &str) -> GreetResponse {
    GreetResponse {
        message: format!("Hello, {}! (from Tauri Rust backend)", name),
    }
}

// ===== FILE BROWSER COMMANDS =====

#[tauri::command]
fn list_directory(dir: Option<String>) -> Result<Vec<FileItem>, String> {
    let path = dir.unwrap_or_else(|| "/".to_string());
    let base = Path::new(&path);

    // Security: prevent directory traversal outside allowed roots
    if path.contains("..") {
        return Err("Invalid path".to_string());
    }

    let mut items = Vec::new();

    // Add parent directory if not root
    if path != "/" && base.parent().is_some() {
        if let Some(parent) = base.parent() {
            items.push(FileItem {
                name: "..".to_string(),
                path: parent.to_string_lossy().to_string(),
                is_dir: true,
                size: 0,
                modified: None,
            });
        }
    }

    match fs::read_dir(&base) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                let metadata = entry.metadata().ok();
                let file_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                items.push(FileItem {
                    name: file_name.clone(),
                    path: path.to_string_lossy().to_string(),
                    is_dir: metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false),
                    size: metadata.as_ref().map(|m| m.len()).unwrap_or(0),
                    modified: metadata
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs() as i64),
                });
            }
        }
        Err(e) => return Err(format!("Cannot read directory: {}", e)),
    }

    // Sort: directories first, then by name
    items.sort_by(|a, b| {
        if a.is_dir != b.is_dir {
            b.is_dir.cmp(&a.is_dir)
        } else {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        }
    });

    Ok(items)
}

#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    if path.contains("..") {
        return Err("Invalid path".to_string());
    }
    fs::read_to_string(&path).map_err(|e| format!("Cannot read file: {}", e))
}

#[tauri::command]
fn write_text_file(path: String, content: String) -> Result<(), String> {
    if path.contains("..") {
        return Err("Invalid path".to_string());
    }
    fs::write(&path, content).map_err(|e| format!("Cannot write file: {}", e))
}

#[tauri::command]
fn create_directory(dir_path: String) -> Result<(), String> {
    if dir_path.contains("..") {
        return Err("Invalid path".to_string());
    }
    fs::create_dir_all(&dir_path).map_err(|e| format!("Cannot create directory: {}", e))
}

#[tauri::command]
fn delete_file_or_dir(path: String) -> Result<(), String> {
    if path.contains("..") || path == "/" {
        return Err("Invalid path".to_string());
    }
    let p = Path::new(&path);
    if p.is_dir() {
        fs::remove_dir_all(p)
    } else {
        fs::remove_file(p)
    }
    .map_err(|e| format!("Cannot delete: {}", e))
}

#[tauri::command]
fn rename_file_or_dir(old_path: String, new_path: String) -> Result<(), String> {
    if old_path.contains("..") || new_path.contains("..") {
        return Err("Invalid path".to_string());
    }
    fs::rename(&old_path, &new_path).map_err(|e| format!("Cannot rename: {}", e))
}

#[tauri::command]
fn get_home_dir() -> Result<String, String> {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "Cannot determine home directory".to_string())
}

#[tauri::command]
fn get_temp_dir() -> Result<String, String> {
    std::env::temp_dir()
        .to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Cannot determine temp directory".to_string())
}

#[tauri::command]
fn get_downloads_dir() -> Result<String, String> {
    dirs::download_dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "Downloads directory not found".to_string())
}

#[tauri::command]
fn copy_file(source: String, destination: String) -> Result<(), String> {
    if source.contains("..") || destination.contains("..") {
        return Err("Invalid path".to_string());
    }
    fs::copy(&source, &destination)
        .map(|_| ())
        .map_err(|e| format!("Copy failed: {}", e))
}

#[tauri::command]
fn move_file(source: String, destination: String) -> Result<(), String> {
    if source.contains("..") || destination.contains("..") {
        return Err("Invalid path".to_string());
    }
    fs::rename(&source, &destination).map_err(|e| format!("Move failed: {}", e))
}

#[tauri::command]
fn copy_file_to_clipboard(path: String) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::process::Command;
        Command::new("xclip")
            .args(&["-selection", "clipboard", "-i", &path])
            .output()
            .map_err(|e| format!("Copy to clipboard failed: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
fn get_file_info(path: String) -> Result<FileItem, String> {
    if path.contains("..") {
        return Err("Invalid path".to_string());
    }
    let p = Path::new(&path);
    let metadata = fs::metadata(&p).map_err(|e| format!("Cannot get file info: {}", e))?;
    let file_name = p
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    Ok(FileItem {
        name: file_name,
        path: path.clone(),
        is_dir: metadata.is_dir(),
        size: metadata.len(),
        modified: metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64),
    })
}

#[tauri::command]
fn file_exists(path: String) -> Result<bool, String> {
    if path.contains("..") {
        return Err("Invalid path".to_string());
    }
    Ok(Path::new(&path).exists())
}

#[tauri::command]
fn search_files(dir: String, pattern: String) -> Result<Vec<FileItem>, String> {
    if dir.contains("..") {
        return Err("Invalid directory".to_string());
    }
    let base = Path::new(&dir);
    let mut results = Vec::new();
    let pattern_lower = pattern.to_lowercase();

    fn walk_dir(dir: &Path, pattern: &str, results: &mut Vec<FileItem>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.to_lowercase().contains(pattern) {
                        let metadata = entry.metadata().ok();
                        results.push(FileItem {
                            name: name.to_string(),
                            path: path.to_string_lossy().to_string(),
                            is_dir: metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false),
                            size: metadata.as_ref().map(|m| m.len()).unwrap_or(0),
                            modified: metadata
                                .and_then(|m| m.modified().ok())
                                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|d| d.as_secs() as i64),
                        });
                    }
                    if path.is_dir() {
                        walk_dir(&path, pattern, results);
                    }
                }
            }
        }
    }

    walk_dir(base, &pattern_lower, &mut results);
    Ok(results)
}

#[tauri::command]
fn get_drives() -> Result<Vec<FileItem>, String> {
    let mut items = Vec::new();

    #[cfg(unix)]
    {
        items.push(FileItem {
            name: "/".to_string(),
            path: "/".to_string(),
            is_dir: true,
            size: 0,
            modified: None,
        });
        if let Some(home) = dirs::home_dir() {
            items.push(FileItem {
                name: "Home".to_string(),
                path: home.to_string_lossy().to_string(),
                is_dir: true,
                size: 0,
                modified: None,
            });
        }
    }

    #[cfg(windows)]
    {
        for letter in b'A'..=b'Z' {
            let drive = format!("{}:\\", letter as char);
            let p = Path::new(&drive);
            if p.exists() {
                items.push(FileItem {
                    name: format!("Drive {}", letter as char),
                    path: drive,
                    is_dir: true,
                    size: 0,
                    modified: None,
                });
            }
        }
    }

    Ok(items)
}

#[tauri::command]
fn execute_terminal_command(cmd: String) -> Result<String, String> {
    use std::process::Command;

    #[cfg(unix)]
    {
        let output = Command::new("sh")
            .args(&["-c", &cmd])
            .output()
            .map_err(|e| format!("Command failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if !stderr.is_empty() {
            Ok(format!("{}{}", stdout, stderr))
        } else {
            Ok(stdout)
        }
    }

    #[cfg(windows)]
    {
        let output = Command::new("cmd")
            .args(&["/C", &cmd])
            .output()
            .map_err(|e| format!("Command failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if !stderr.is_empty() {
            Ok(format!("{}{}", stdout, stderr))
        } else {
            Ok(stdout)
        }
    }
}

#[tauri::command]
fn get_system_info() -> Result<SystemInfo, String> {
    let hostname = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown".to_string());

    let sys = sysinfo::System::new_all();

    Ok(SystemInfo {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        hostname,
        cpu_count: sys.cpus().len(),
        total_memory: sys.total_memory(),
        free_memory: sys.free_memory(),
    })
}

#[tauri::command]
fn get_process_list() -> Result<Vec<ProcessItem>, String> {
    let sys = sysinfo::System::new_all();
    let mut processes = Vec::new();

    for (pid, process) in sys.processes() {
        processes.push(ProcessItem {
            pid: pid.as_u32(),
            name: process.name().to_string(),
            cpu: process.cpu_usage(),
            memory: process.memory(),
        });
    }

    processes.sort_by(|a, b| {
        b.cpu
            .partial_cmp(&a.cpu)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(processes.into_iter().take(100).collect()) // Top 100
}

#[tauri::command]
fn execute_command(cmd: String, args: Vec<String>) -> Result<(i32, String, String), String> {
    use std::process::Command;

    let output = Command::new(&cmd)
        .args(&args)
        .output()
        .map_err(|e| format!("Command execution failed: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    Ok((output.status.code().unwrap_or(-1), stdout, stderr))
}

#[tauri::command]
fn kill_process(pid: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;
        kill(Pid::from_raw(pid as i32), Signal::SIGTERM)
            .map_err(|e| format!("Cannot kill process: {}", e))?;
    }
    #[cfg(windows)]
    {
        use std::process::Command;
        Command::new("taskkill")
            .args(&["/PID", &pid.to_string(), "/F"])
            .output()
            .map_err(|e| format!("Cannot kill process: {}", e))?;
    }
    Ok(())
}

// ===== TLS & SERVER COMMANDS =====

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(e) = crypto::certificates::generate_local_certs(None) {
        eprintln!("Warning: Failed to auto-generate TLS certificates: {}", e);
    }

    http_server::server::start_http_server(8080).ok();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet_json,
            crypto::certificates::generate_tls_certificates,
            crypto::certificates::generate_local_certs,
            http_server::server::toggle_http_server,
            http_server::server::stop_http_server,
            http_server::server::is_server_running,
            http_server::server::set_server_port,
            http_server::server::get_server_port,
            // File operations
            list_directory,
            read_text_file,
            write_text_file,
            create_directory,
            delete_file_or_dir,
            rename_file_or_dir,
            get_home_dir,
            get_temp_dir,
            get_downloads_dir,
            copy_file,
            move_file,
            copy_file_to_clipboard,
            get_file_info,
            file_exists,
            search_files,
            get_drives,
            // System info
            get_system_info,
            get_process_list,
            execute_command,
            kill_process,
            execute_terminal_command,
            // Binary file
            get_binary_file,
            get_binary_mime,
            crypto::common::get_shared_key,
            crypto::common::set_random_shared_alphanumeric_key,
            crypto::common::get_session_new_key,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
