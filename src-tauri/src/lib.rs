mod api;
mod config;

use api::TinyPngClient;
use base64::Engine;
use config::{load_config, save_config, Config};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, State};

const SUPPORTED_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "avif"];

struct AppState {
    config: Arc<Mutex<Config>>,
}

#[derive(Debug, Serialize, Clone)]
struct ConfigResponse {
    has_api_key: bool,
    output_dir: Option<String>,
    compression_count: u64,
}

#[derive(Debug, Serialize, Clone)]
struct ValidateKeyResponse {
    valid: bool,
    compression_count: u64,
}

#[derive(Debug, Serialize, Clone)]
struct FileProgress {
    file_name: String,
    status: String, // "compressing", "done", "error"
    original_size: Option<u64>,
    compressed_size: Option<u64>,
    saved_to: Option<String>,
    error: Option<String>,
    compression_count: Option<u64>,
    index: usize,
    total: usize,
}

fn is_supported_file(path: &str) -> bool {
    let p = Path::new(path);
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| SUPPORTED_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn get_output_dir(config: &Config, file_path: &str) -> PathBuf {
    if let Some(ref dir) = config.output_dir {
        PathBuf::from(dir)
    } else {
        // Default: "compressed" subdirectory relative to the source file
        let parent = Path::new(file_path)
            .parent()
            .unwrap_or_else(|| Path::new("."));
        parent.join("compressed")
    }
}

#[tauri::command]
fn get_config(state: State<AppState>) -> ConfigResponse {
    let config = state.config.lock().unwrap();
    ConfigResponse {
        has_api_key: config.api_key.is_some(),
        output_dir: config.output_dir.clone(),
        compression_count: config.compression_count,
    }
}

#[tauri::command]
fn set_api_key(key: String, state: State<AppState>) -> Result<ValidateKeyResponse, String> {
    let client = TinyPngClient::new(&key);
    let (valid, count) = client.validate_key()?;

    if valid {
        let mut config = state.config.lock().unwrap();
        config.api_key = Some(key);
        config.compression_count = count;
        save_config(&config)?;
        Ok(ValidateKeyResponse {
            valid: true,
            compression_count: count,
        })
    } else {
        Err("Invalid API key".to_string())
    }
}

#[tauri::command]
fn set_output_dir(dir: String, state: State<AppState>) -> Result<(), String> {
    let mut config = state.config.lock().unwrap();
    if dir.is_empty() {
        config.output_dir = None;
    } else {
        config.output_dir = Some(dir);
    }
    save_config(&config)?;
    Ok(())
}

#[tauri::command]
fn compress_files(paths: Vec<String>, app: tauri::AppHandle, state: State<AppState>) -> Result<(), String> {
    let config = state.config.lock().unwrap().clone();
    let api_key = config
        .api_key
        .as_ref()
        .ok_or_else(|| "No API key configured".to_string())?
        .clone();

    let total = paths.len();
    let config_clone = config.clone();
    let config_mutex = state.config.clone();

    // Spawn compression in a background thread to not block the UI
    std::thread::spawn(move || {
        let client = TinyPngClient::new(&api_key);

        for (index, file_path) in paths.iter().enumerate() {
            if !is_supported_file(file_path) {
                let _ = app.emit(
                    "compress-progress",
                    FileProgress {
                        file_name: Path::new(file_path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown")
                            .to_string(),
                        status: "error".to_string(),
                        original_size: None,
                        compressed_size: None,
                        saved_to: None,
                        error: Some("Unsupported file type".to_string()),
                        compression_count: None,
                        index,
                        total,
                    },
                );
                continue;
            }

            // Emit "compressing" status
            let file_name = Path::new(file_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            let _ = app.emit(
                "compress-progress",
                FileProgress {
                    file_name: file_name.clone(),
                    status: "compressing".to_string(),
                    original_size: None,
                    compressed_size: None,
                    saved_to: None,
                    error: None,
                    compression_count: None,
                    index,
                    total,
                },
            );

            let output_dir = get_output_dir(&config_clone, file_path);
            match client.compress_file(Path::new(file_path), &output_dir) {
                Ok(result) => {
                    // Persist the compression count
                    if let Ok(mut cfg) = config_mutex.lock() {
                        cfg.compression_count = result.compression_count;
                        let _ = save_config(&cfg);
                    }

                    let _ = app.emit(
                        "compress-progress",
                        FileProgress {
                            file_name: result.file_name,
                            status: "done".to_string(),
                            original_size: Some(result.original_size),
                            compressed_size: Some(result.compressed_size),
                            saved_to: Some(result.saved_to),
                            error: None,
                            compression_count: Some(result.compression_count),
                            index,
                            total,
                        },
                    );
                }
                Err(err) => {
                    let _ = app.emit(
                        "compress-progress",
                        FileProgress {
                            file_name,
                            status: "error".to_string(),
                            original_size: None,
                            compressed_size: None,
                            saved_to: None,
                            error: Some(err),
                            compression_count: None,
                            index,
                            total,
                        },
                    );
                }
            }
        }
    });

    Ok(())
}

#[tauri::command]
fn install_send_to_shortcut() -> Result<String, String> {
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get exe path: {}", e))?;

    // Get the SendTo folder: %APPDATA%\Microsoft\Windows\SendTo
    let appdata = dirs::config_dir()
        .ok_or_else(|| "Failed to get AppData path".to_string())?;
    let send_to_dir = appdata.join("Microsoft").join("Windows").join("SendTo");

    if !send_to_dir.exists() {
        return Err("SendTo folder not found".to_string());
    }

    let shortcut_path = send_to_dir.join("TinyPNG Compressor.lnk");

    // Use PowerShell to create a .lnk shortcut
    let ps_script = format!(
        r#"$ws = New-Object -ComObject WScript.Shell; $s = $ws.CreateShortcut('{}'); $s.TargetPath = '{}'; $s.Description = 'Compress images with TinyPNG'; $s.Save()"#,
        shortcut_path.to_string_lossy().replace('\'', "''"),
        exe_path.to_string_lossy().replace('\'', "''")
    );

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_script])
        .output()
        .map_err(|e| format!("Failed to run PowerShell: {}", e))?;

    if output.status.success() {
        Ok(format!(
            "Shortcut created at {}",
            shortcut_path.to_string_lossy()
        ))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Failed to create shortcut: {}", stderr))
    }
}

#[tauri::command]
fn read_image_thumbnail(path: String) -> Result<String, String> {
    let file_path = Path::new(&path);
    let data = std::fs::read(file_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let mime = match file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
    {
        Some(ref e) if e == "png" => "image/png",
        Some(ref e) if e == "jpg" || e == "jpeg" => "image/jpeg",
        Some(ref e) if e == "webp" => "image/webp",
        Some(ref e) if e == "avif" => "image/avif",
        _ => "application/octet-stream",
    };

    let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
    Ok(format!("data:{};base64,{}", mime, b64))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config = load_config();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            config: Arc::new(Mutex::new(config)),
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            set_api_key,
            set_output_dir,
            compress_files,
            install_send_to_shortcut,
            read_image_thumbnail,
        ])
        .setup(|app| {
            // Check for CLI arguments (for "Send To" integration)
            let args: Vec<String> = std::env::args().skip(1).collect();
            let file_args: Vec<String> = args
                .into_iter()
                .filter(|a| !a.starts_with('-') && is_supported_file(a))
                .collect();

            if !file_args.is_empty() {
                let handle = app.handle().clone();
                // Small delay for the window to be ready
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    let _ = handle.emit("files-from-cli", file_args);
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
