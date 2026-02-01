use std::process::Command;
use std::path::PathBuf;
use std::fs;
use serde::{Deserialize, Serialize};
use tauri::Manager;

#[derive(Debug, Serialize, Deserialize)]
pub struct TranscriptionResult {
    pub text: String,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelStatus {
    pub installed: bool,
    pub path: Option<String>,
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

fn get_model_path(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    let app_data = app_handle.path().app_data_dir()
        .map_err(|e| format!("Не удалось получить директорию данных приложения: {}", e))?;
    
    let models_dir = app_data.join("models");
    let model_path = models_dir.join("ggml-tiny.bin");
    
    Ok(model_path)
}

#[tauri::command]
async fn check_model_installed(app_handle: tauri::AppHandle) -> Result<ModelStatus, String> {
    let model_path = get_model_path(&app_handle)?;
    
    let installed = model_path.exists();
    
    Ok(ModelStatus {
        installed,
        path: if installed { 
            Some(model_path.to_string_lossy().to_string()) 
        } else { 
            None 
        },
    })
}

#[tauri::command]
async fn download_model(app_handle: tauri::AppHandle) -> Result<String, String> {
    let model_path = get_model_path(&app_handle)?;
    let models_dir = model_path.parent()
        .ok_or("Не удалось получить родительскую директорию")?;
    
    // Создаём директорию если её нет
    fs::create_dir_all(models_dir)
        .map_err(|e| format!("Не удалось создать директорию для моделей: {}", e))?;
    
    // URL модели tiny от OpenAI Whisper
    let model_url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin";
    
    // Скачиваем модель
    let response = reqwest::get(model_url)
        .await
        .map_err(|e| format!("Не удалось скачать модель: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("Ошибка при скачивании модели: {}", response.status()));
    }
    
    let bytes = response.bytes()
        .await
        .map_err(|e| format!("Не удалось прочитать данные: {}", e))?;
    
    // Сохраняем модель
    fs::write(&model_path, bytes)
        .map_err(|e| format!("Не удалось сохранить модель: {}", e))?;
    
    Ok(model_path.to_string_lossy().to_string())
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

    // Проверяем наличие модели
    let model_path = get_model_path(&app_handle)?;
    if !model_path.exists() {
        return Ok(TranscriptionResult {
            text: String::new(),
            success: false,
            error: Some("Модель не установлена. Пожалуйста, установите модель.".to_string()),
        });
    }

    // Используем whisper-cli
    let whisper_cli_name = if cfg!(target_os = "windows") {
        "whisper-cli.exe"
    } else if cfg!(target_os = "macos") {
        "whisper-cli"
    } else {
        "whisper-cli"
    };
    
    let whisper_cli_path = if cfg!(debug_assertions) {
        // В dev режиме берём из whisher директории проекта
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("whisher").join(whisper_cli_name)
    } else {
        if cfg!(target_os = "windows") {
            // В Windows берём из resource директории
            let resource_dir = app_handle.path().resource_dir()
                .map_err(|e| format!("Не удалось найти директорию ресурсов: {}", e))?;
            resource_dir.join("whisher").join(whisper_cli_name)
        } else {
            // В Linux/Mac сначала пробуем из resource директории
            let resource_dir = app_handle.path().resource_dir()
                .map_err(|e| format!("Не удалось найти директорию ресурсов: {}", e))?;
            let resource_path = resource_dir.join("whisher").join(whisper_cli_name);
            
            // Если нет в resources, ищем в системе
            if resource_path.exists() {
                resource_path
            } else {
                // Пробуем найти в PATH
                which::which("main")
                    .or_else(|_| which::which("whisper-cli"))
                    .unwrap_or_else(|_| PathBuf::from("main"))
            }
        }
    };
    
    if !whisper_cli_path.exists() {
        return Ok(TranscriptionResult {
            text: String::new(),
            success: false,
            error: Some(format!("Whisper CLI не найден по пути: {:?}", whisper_cli_path)),
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
                            error: Some(format!("Не удалось прочитать результат из файла: {}. STDERR: {}. STDOUT: {}", e, stderr, stdout)),
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
        .invoke_handler(tauri::generate_handler![greet, transcribe_audio, check_model_installed, download_model])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
