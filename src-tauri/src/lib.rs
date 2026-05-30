use std::process::{Command, Stdio};
use std::path::PathBuf;
use std::io::{BufRead, BufReader};
use std::fs;
use serde::{Deserialize, Serialize};
use tauri::{Manager, Emitter};
use futures_util::StreamExt;

/// Скрывает чёрное окно консоли при запуске дочерних процессов на Windows.
/// На остальных ОС ничего не делает.
fn hidden(mut cmd: Command) -> Command {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

// ============================================================================
// Структуры данных
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct TranscriptionResult {
    pub text: String,
    /// Содержимое субтитров в формате SRT (для экспорта)
    pub srt: String,
    pub success: bool,
    pub error: Option<String>,
    /// Был ли применён шумоподавитель (ffmpeg)
    pub denoised: bool,
}

/// Прогресс расшифровки, отправляется через событие "transcribe-progress".
#[derive(Debug, Clone, Serialize)]
pub struct TranscribeProgress {
    pub percent: u32,
    /// Этап: "denoise" | "transcribe"
    pub stage: String,
}

/// Описание одной модели Whisper, доступной для установки.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Уникальный идентификатор (имя .bin файла без расширения), напр. "ggml-base"
    pub id: String,
    /// Человекочитаемое название
    pub name: String,
    /// Имя файла на диске
    pub file_name: String,
    /// URL для скачивания
    pub url: String,
    /// Приблизительный размер для отображения
    pub size_label: String,
    /// Описание (скорость / точность)
    pub description: String,
    /// Установлена ли модель локально
    pub installed: bool,
}

/// Прогресс скачивания, отправляется во фронтенд через событие "download-progress".
#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    pub model_id: String,
    pub downloaded: u64,
    pub total: u64,
    pub percent: f64,
}

// ============================================================================
// Каталог моделей
// ============================================================================

/// Базовый URL репозитория whisper.cpp на Hugging Face.
const HF_BASE: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

/// Возвращает каталог всех поддерживаемых моделей (без поля installed).
fn model_catalog() -> Vec<ModelInfo> {
    let mk = |id: &str, name: &str, size: &str, desc: &str| {
        let file_name = format!("{}.bin", id);
        ModelInfo {
            id: id.to_string(),
            name: name.to_string(),
            file_name: file_name.clone(),
            url: format!("{}/{}", HF_BASE, file_name),
            size_label: size.to_string(),
            description: desc.to_string(),
            installed: false,
        }
    };

    vec![
        mk("ggml-tiny", "Tiny", "75 MB", "Самая быстрая, низкая точность"),
        mk("ggml-base", "Base", "142 MB", "Быстрая, средняя точность"),
        mk("ggml-small", "Small", "466 MB", "Баланс скорости и точности"),
        mk("ggml-medium", "Medium", "1.5 GB", "Высокая точность, медленнее"),
        mk("ggml-large-v3", "Large v3", "2.9 GB", "Максимальная точность, требует ресурсов"),
        mk("ggml-tiny.en", "Tiny (English)", "75 MB", "Только английский, быстрая"),
        mk("ggml-base.en", "Base (English)", "142 MB", "Только английский, средняя"),
    ]
}

/// Директория, где хранятся скачанные модели.
fn models_dir(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    let app_data = app_handle.path().app_data_dir()
        .map_err(|e| format!("Не удалось получить директорию данных приложения: {}", e))?;
    Ok(app_data.join("models"))
}

/// Полный путь к файлу конкретной модели.
fn model_file_path(app_handle: &tauri::AppHandle, model_id: &str) -> Result<PathBuf, String> {
    Ok(models_dir(app_handle)?.join(format!("{}.bin", model_id)))
}

// ============================================================================
// Команды: управление моделями
// ============================================================================

/// Возвращает список всех моделей с актуальным статусом установки.
#[tauri::command]
async fn list_models(app_handle: tauri::AppHandle) -> Result<Vec<ModelInfo>, String> {
    let dir = models_dir(&app_handle)?;
    let mut models = model_catalog();
    for m in models.iter_mut() {
        m.installed = dir.join(&m.file_name).exists();
    }
    Ok(models)
}

/// Скачивает выбранную модель, отправляя прогресс через событие "download-progress".
#[tauri::command]
async fn download_model(app_handle: tauri::AppHandle, model_id: String) -> Result<String, String> {
    let model = model_catalog()
        .into_iter()
        .find(|m| m.id == model_id)
        .ok_or_else(|| format!("Неизвестная модель: {}", model_id))?;

    let dir = models_dir(&app_handle)?;
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Не удалось создать директорию для моделей: {}", e))?;

    let target = dir.join(&model.file_name);
    // Скачиваем во временный файл, затем переименовываем — чтобы прерванная
    // загрузка не оставила «битую» модель.
    let tmp = dir.join(format!("{}.part", model.file_name));

    let response = reqwest::get(&model.url)
        .await
        .map_err(|e| format!("Не удалось начать скачивание: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Ошибка при скачивании модели: {}", response.status()));
    }

    let total = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;
    let mut file = fs::File::create(&tmp)
        .map_err(|e| format!("Не удалось создать файл модели: {}", e))?;

    let mut stream = response.bytes_stream();
    use std::io::Write;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Ошибка при чтении данных: {}", e))?;
        file.write_all(&chunk)
            .map_err(|e| format!("Ошибка при записи файла: {}", e))?;
        downloaded += chunk.len() as u64;

        let percent = if total > 0 {
            (downloaded as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        let _ = app_handle.emit("download-progress", DownloadProgress {
            model_id: model_id.clone(),
            downloaded,
            total,
            percent,
        });
    }
    file.flush().map_err(|e| format!("Ошибка при сохранении файла: {}", e))?;
    drop(file);

    fs::rename(&tmp, &target)
        .map_err(|e| format!("Не удалось сохранить модель: {}", e))?;

    Ok(target.to_string_lossy().to_string())
}

/// Удаляет установленную модель.
#[tauri::command]
async fn delete_model(app_handle: tauri::AppHandle, model_id: String) -> Result<(), String> {
    let path = model_file_path(&app_handle, &model_id)?;
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|e| format!("Не удалось удалить модель: {}", e))?;
    }
    Ok(())
}

// ============================================================================
// Поиск бинарника whisper-cli и ffmpeg
// ============================================================================

fn whisper_cli_path(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    let cli_name = if cfg!(target_os = "windows") {
        "whisper-cli.exe"
    } else {
        "whisper-cli"
    };

    if cfg!(debug_assertions) {
        return Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("whisher").join(cli_name));
    }

    let resource_dir = app_handle.path().resource_dir()
        .map_err(|e| format!("Не удалось найти директорию ресурсов: {}", e))?;
    let resource_path = resource_dir.join("whisher").join(cli_name);
    if resource_path.exists() {
        Ok(resource_path)
    } else {
        // Fallback: ищем в системном PATH
        which::which("whisper-cli")
            .or_else(|_| which::which("main"))
            .map_err(|_| "Whisper CLI не найден ни в ресурсах, ни в PATH".to_string())
    }
}

/// Ищет ffmpeg: сначала бандлёный (в ресурсах/dev-папке whisher), затем системный.
/// Возвращает None, если нигде не найден.
fn find_ffmpeg(app_handle: &tauri::AppHandle) -> Option<PathBuf> {
    let bin = if cfg!(target_os = "windows") { "ffmpeg.exe" } else { "ffmpeg" };

    // 1. Бандлёный бинарник
    let bundled = if cfg!(debug_assertions) {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("whisher").join(bin)
    } else {
        app_handle.path().resource_dir().ok()?.join("whisher").join(bin)
    };
    if bundled.exists() {
        return Some(bundled);
    }

    // 2. Системный ffmpeg
    which::which("ffmpeg").ok()
}

/// Прогоняет аудио через ffmpeg с фильтрами шумоподавления и нормализации.
/// Возвращает путь к обработанному WAV-файлу (16 kHz mono — оптимально для whisper).
/// При любой ошибке возвращает None — вызывающий код использует исходный файл.
fn denoise_audio(ffmpeg: &PathBuf, input: &str) -> Option<PathBuf> {
    let out = std::env::temp_dir().join(format!(
        "stt_denoised_{}.wav",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_millis()
    ));

    // afftdn  — адаптивное подавление шума в частотной области
    // highpass=80 / lowpass=8000 — срез гула и высокочастотного шипения
    // loudnorm — нормализация громкости (EBU R128)
    let filter = "afftdn=nf=-25,highpass=f=80,lowpass=f=8000,loudnorm";

    let mut cmd = hidden(Command::new(ffmpeg));
    let status = cmd
        .args([
            "-y",
            "-i", input,
            "-af", filter,
            "-ar", "16000",
            "-ac", "1",
            "-c:a", "pcm_s16le",
        ])
        .arg(&out)
        .output();

    match status {
        Ok(r) if r.status.success() && out.exists() => Some(out),
        _ => None,
    }
}

// ============================================================================
// Команда: транскрибация
// ============================================================================

#[tauri::command]
async fn transcribe_audio(
    app_handle: tauri::AppHandle,
    file_path: String,
    model_id: String,
    language: String,
    denoise: bool,
) -> Result<TranscriptionResult, String> {
    let path = PathBuf::from(&file_path);
    if !path.exists() {
        return Ok(TranscriptionResult {
            text: String::new(), srt: String::new(), success: false, denoised: false,
            error: Some("Файл не найден".to_string()),
        });
    }

    let model_path = model_file_path(&app_handle, &model_id)?;
    if !model_path.exists() {
        return Ok(TranscriptionResult {
            text: String::new(), srt: String::new(), success: false, denoised: false,
            error: Some("Выбранная модель не установлена.".to_string()),
        });
    }

    let cli = match whisper_cli_path(&app_handle) {
        Ok(p) if p.exists() => p,
        Ok(p) => return Ok(TranscriptionResult {
            text: String::new(), srt: String::new(), success: false, denoised: false,
            error: Some(format!("Whisper CLI не найден по пути: {:?}", p)),
        }),
        Err(e) => return Ok(TranscriptionResult {
            text: String::new(), srt: String::new(), success: false, denoised: false, error: Some(e),
        }),
    };

    let emit_progress = |percent: u32, stage: &str| {
        let _ = app_handle.emit("transcribe-progress", TranscribeProgress {
            percent,
            stage: stage.to_string(),
        });
    };

    // ---- Шаг 1: опциональное шумоподавление ----
    emit_progress(0, "transcribe");
    let mut denoised = false;
    let mut tmp_denoised: Option<PathBuf> = None;
    let input_for_whisper = if denoise {
        match find_ffmpeg(&app_handle) {
            Some(ffmpeg) => {
                emit_progress(0, "denoise");
                match denoise_audio(&ffmpeg, &file_path) {
                    Some(out) => {
                        denoised = true;
                        let s = out.to_string_lossy().to_string();
                        tmp_denoised = Some(out);
                        s
                    }
                    None => file_path.clone(), // ffmpeg есть, но фильтр упал — берём оригинал
                }
            }
            None => file_path.clone(), // ffmpeg не найден — graceful fallback
        }
    } else {
        file_path.clone()
    };

    let output_dir = std::env::temp_dir();
    let output_base = output_dir.join(format!(
        "whisper_output_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    ));

    // "auto" => whisper определит язык сам
    let lang = if language.is_empty() { "auto".to_string() } else { language };

    // ---- Шаг 2: запускаем whisper-cli и стримим прогресс из stderr ----
    // Флаг -pp печатает строки вида "...progress = NN%" в stderr.
    let mut cmd = hidden(Command::new(&cli));
    let child = cmd
        .arg("-f").arg(&input_for_whisper)
        .arg("-m").arg(&model_path)
        .arg("-l").arg(&lang)
        .arg("-t").arg("4")
        .arg("-otxt")
        .arg("-osrt")              // также генерируем .srt для экспорта субтитров
        .arg("-of").arg(&output_base)
        .arg("-pp")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            if let Some(tmp) = &tmp_denoised { let _ = fs::remove_file(tmp); }
            return Ok(TranscriptionResult {
                text: String::new(), srt: String::new(), success: false, denoised,
                error: Some(format!("Не удалось запустить Whisper CLI: {}", e)),
            });
        }
    };

    // Читаем stderr построчно, вытаскиваем прогресс
    let mut stderr_buf = String::new();
    if let Some(stderr) = child.stderr.take() {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            if let Some(p) = parse_progress(&line) {
                emit_progress(p, "transcribe");
            }
            stderr_buf.push_str(&line);
            stderr_buf.push('\n');
        }
    }

    let status = child.wait();
    emit_progress(100, "transcribe");

    // Чистим временный денойзный файл
    if let Some(tmp) = &tmp_denoised { let _ = fs::remove_file(tmp); }

    let success_status = matches!(&status, Ok(s) if s.success());

    let txt_file = format!("{}.txt", output_base.to_string_lossy());
    let srt_file = format!("{}.srt", output_base.to_string_lossy());

    let srt = fs::read_to_string(&srt_file).unwrap_or_default();
    let _ = fs::remove_file(&srt_file);

    let text = match fs::read_to_string(&txt_file) {
        Ok(content) => {
            let _ = fs::remove_file(&txt_file);
            content.trim().to_string()
        }
        Err(e) => {
            // Файла нет — пробуем вытащить текст из srt
            let from_srt = srt_to_text(&srt);
            if from_srt.is_empty() {
                return Ok(TranscriptionResult {
                    text: String::new(), srt: String::new(), success: false, denoised,
                    error: Some(format!(
                        "Не удалось прочитать результат: {}. STDERR: {}", e, stderr_buf
                    )),
                });
            }
            from_srt
        }
    };

    if success_status || !text.is_empty() {
        Ok(TranscriptionResult { text, srt, success: true, error: None, denoised })
    } else {
        Ok(TranscriptionResult {
            text: String::new(), srt: String::new(), success: false, denoised,
            error: Some(format!("Whisper завершился с ошибкой. STDERR: {}", stderr_buf)),
        })
    }
}

/// Вытаскивает процент из строки прогресса whisper-cli (напр. "progress =  42%").
fn parse_progress(line: &str) -> Option<u32> {
    let l = line.to_lowercase();
    let idx = l.find("progress")?;
    let rest = &l[idx..];
    let pct = rest.find('%')?;
    // берём цифры перед знаком %
    let digits: String = rest[..pct]
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit() || c.is_whitespace())
        .collect::<String>()
        .chars()
        .rev()
        .filter(|c| c.is_ascii_digit())
        .collect();
    digits.parse::<u32>().ok().map(|v| v.min(100))
}

/// Преобразует SRT в чистый текст (для fallback, если .txt не создался).
fn srt_to_text(srt: &str) -> String {
    srt.lines()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty()
                && t.parse::<u32>().is_err()       // не номер субтитра
                && !t.contains("-->")              // не таймкод
        })
        .map(|l| l.trim())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            list_models,
            download_model,
            delete_model,
            transcribe_audio
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
