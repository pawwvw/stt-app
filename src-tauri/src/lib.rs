use std::process::Command;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use tauri::Manager;

#[derive(Debug, Serialize, Deserialize)]
pub struct TranscriptionResult {
    pub text: String,
    pub success: bool,
    pub error: Option<String>,
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn transcribe_audio(file_path: String, app_handle: tauri::AppHandle) -> Result<TranscriptionResult, String> {
    let path = PathBuf::from(&file_path);
    if !path.exists() {
        return Ok(TranscriptionResult {
            text: String::new(),
            success: false,
            error: Some("Файл не найден".to_string()),
        });
    }

    let whisper_cli_name = if cfg!(target_os = "windows") {
        "whisper-cli.exe"
    } else {
        "whisper-cli"
    };
    
    let (whisper_cli_path, model_path) = if cfg!(debug_assertions) {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("whisher");
        (base.join(whisper_cli_name), base.join("models").join("ggml-tiny.bin"))
    } else {
        let resource_dir = app_handle.path().resource_dir()
            .map_err(|e| format!("Не удалось найти директорию ресурсов: {}", e))?;
        
        let cli_path = resource_dir.join(whisper_cli_name);
        let model_path_option1 = resource_dir.join("models").join("ggml-tiny.bin");
        let model_path_option2 = resource_dir.join("ggml-tiny.bin");
        
        let final_model = if model_path_option1.exists() {
            model_path_option1
        } else {
            model_path_option2
        };
        
        (cli_path, final_model)
    };
    
    if !whisper_cli_path.exists() {
        return Ok(TranscriptionResult {
            text: String::new(),
            success: false,
            error: Some(format!("Whisper CLI не найден по пути: {:?}", whisper_cli_path)),
        });
    }
    
    if !model_path.exists() {
        return Ok(TranscriptionResult {
            text: String::new(),
            success: false,
            error: Some(format!("Модель не найдена по пути: {:?}", model_path)),
        });
    }

    let output_dir = std::env::temp_dir();
    let output_file_base = output_dir.join(format!("whisper_output_{}", 
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()));
    
    let output = Command::new(&whisper_cli_path)
        .arg("-f")
        .arg(&file_path)
        .arg("-m")
        .arg(&model_path)
        .arg("-l")
        .arg("ru")
        .arg("-t")
        .arg("4")
        .arg("-otxt")
        .arg("-of")
        .arg(&output_file_base)
        .arg("-pp")
        .output();

    match output {
        Ok(result) => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            let stderr = String::from_utf8_lossy(&result.stderr);
            
            let output_txt_file = format!("{}.txt", output_file_base.to_string_lossy());
            let text = match std::fs::read_to_string(&output_txt_file) {
                Ok(content) => {
                    let _ = std::fs::remove_file(&output_txt_file);
                    content.trim().to_string()
                }
                Err(e) => {
                    let text_from_stdout = stdout
                        .lines()
                        .filter(|line| {
                            let line_trim = line.trim();
                            !line_trim.starts_with("whisper_")
                                && !line_trim.starts_with("system_info")
                                && !line_trim.starts_with("main:")
                                && !line_trim.contains("processing")
                                && !line_trim.contains("load time")
                                && !line_trim.contains("mel time")
                                && !line_trim.contains("sample time")
                                && !line_trim.contains("encode time")
                                && !line_trim.contains("decode time")
                                && !line_trim.contains("batchd time")
                                && !line_trim.contains("prompt time")
                                && !line_trim.contains("total time")
                                && !line_trim.contains("fallbacks")
                                && !line_trim.is_empty()
                                && line_trim != "[BLANK_AUDIO]"
                        })
                        .map(|line| line.trim())
                        .collect::<Vec<_>>()
                        .join("\n");
                    
                    if text_from_stdout.is_empty() {
                        return Ok(TranscriptionResult {
                            text: String::new(),
                            success: false,
                            error: Some(format!("Не удалось прочитать результат из файла: {}", e)),
                        });
                    }
                    
                    text_from_stdout
                }
            };
            
            if result.status.success() || !text.is_empty() {
                Ok(TranscriptionResult {
                    text,
                    success: true,
                    error: None,
                })
            } else {
                Ok(TranscriptionResult {
                    text: String::new(),
                    success: false,
                    error: Some(format!("Whisper завершился с ошибкой. STDERR: {}", stderr)),
                })
            }
        }
        Err(e) => {
            Ok(TranscriptionResult {
                text: String::new(),
                success: false,
                error: Some(format!("Не удалось запустить Whisper CLI: {}", e)),
            })
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![greet, transcribe_audio])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
