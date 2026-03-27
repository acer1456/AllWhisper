use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{AppHandle, Emitter};

use crate::types::{Engine, Segment, Transcript, TranscribeConfig};

/// Return a localised string based on the `lang` code.
/// Falls back to Traditional Chinese if the lang is unrecognised.
fn loc(lang: &str, zh_tw: &str, zh_cn: &str, en: &str, ko: &str, vi: &str) -> String {
    match lang {
        "zh-CN" => zh_cn.to_string(),
        "en"    => en.to_string(),
        "ko"    => ko.to_string(),
        "vi"    => vi.to_string(),
        _       => zh_tw.to_string(),
    }
}

/// Locate the ffmpeg binary.
/// macOS .app bundles launch with a minimal PATH that excludes Homebrew,
/// so we probe known install locations in addition to PATH.
fn find_ffmpeg() -> Option<PathBuf> {
    // First: try whatever is in the current PATH
    if let Ok(out) = Command::new("which").arg("ffmpeg").output() {
        if out.status.success() {
            let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !p.is_empty() {
                return Some(PathBuf::from(p));
            }
        }
    }

    // Second: probe common install directories (covers Homebrew Apple Silicon / Intel,
    // MacPorts, nix, Linux distros, and user-local installs)
    let candidates = [
        "/opt/homebrew/bin/ffmpeg",   // Homebrew on Apple Silicon
        "/usr/local/bin/ffmpeg",       // Homebrew on Intel Mac
        "/opt/local/bin/ffmpeg",       // MacPorts
        "/nix/var/nix/profiles/default/bin/ffmpeg",
        "/home/linuxbrew/.linuxbrew/bin/ffmpeg",
        "/usr/bin/ffmpeg",
        "/bin/ffmpeg",
    ];

    for path in &candidates {
        let p = Path::new(path);
        if p.exists() {
            return Some(p.to_path_buf());
        }
    }

    // Third: check common user-level paths from HOME
    if let Ok(home) = std::env::var("HOME") {
        for sub in &[".local/bin/ffmpeg", "bin/ffmpeg"] {
            let p = PathBuf::from(&home).join(sub);
            if p.exists() {
                return Some(p);
            }
        }
    }

    None
}

/// Extract audio to a temp 16kHz mono WAV file using ffmpeg.
/// Returns the temp WAV path.
fn extract_audio(video_path: &str, lang: &str) -> Result<PathBuf, String> {
    let ffmpeg = find_ffmpeg().ok_or_else(|| {
        loc(lang,
            "找不到 ffmpeg 執行檔。\nmacOS：brew install ffmpeg\nWindows：choco install ffmpeg",
            "找不到 ffmpeg 可执行文件。\nmacOS：brew install ffmpeg\nWindows：choco install ffmpeg",
            "ffmpeg not found.\nmacOS: brew install ffmpeg\nWindows: choco install ffmpeg",
            "ffmpeg를 찾을 수 없습니다.\nmacOS: brew install ffmpeg\nWindows: choco install ffmpeg",
            "Không tìm thấy ffmpeg.\nmacOS: brew install ffmpeg\nWindows: choco install ffmpeg",
        )
    })?;

    let input = Path::new(video_path);
    let out = std::env::temp_dir().join(format!(
        "allwhisper_{}.wav",
        input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("audio")
    ));

    let status = Command::new(&ffmpeg)
        .args(["-y", "-i", video_path, "-ar", "16000", "-ac", "1", "-vn", "-f", "wav", out.to_str().unwrap()])
        .output()
        .map_err(|e| format!("{} ({})：{e}",
            loc(lang, "無法執行 ffmpeg", "无法执行 ffmpeg", "Failed to run ffmpeg", "ffmpeg 실행 실패", "Không thể chạy ffmpeg"),
            ffmpeg.display()
        ))?;

    if !status.status.success() {
        let stderr = String::from_utf8_lossy(&status.stderr);
        return Err(format!("{}: {stderr}",
            loc(lang, "ffmpeg 錯誤", "ffmpeg 错误", "ffmpeg error", "ffmpeg 오류", "Lỗi ffmpeg")
        ));
    }

    Ok(out)
}

/// Transcribe using local whisper.cpp via whisper-rs.
#[cfg(feature = "local-whisper")]
fn transcribe_local_impl(
    video_path: &str,
    model_path: &str,
    language: Option<&str>,
    ui_lang: &str,
    app: &AppHandle,
) -> Result<Transcript, String> {
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    let _ = app.emit("transcribe_progress", 5u8);

    let wav_path = extract_audio(video_path, ui_lang)?;

    let _ = app.emit("transcribe_progress", 20u8);

    // Read WAV samples
    let mut reader = hound::WavReader::open(&wav_path)
        .map_err(|e| format!("{}: {e}",
            loc(ui_lang, "無法讀取 WAV", "无法读取 WAV", "Failed to read WAV", "WAV 읽기 실패", "Không thể đọc WAV")
        ))?;
    let samples: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| s.map(|v| v as f32 / i16::MAX as f32))
        .collect::<Result<_, _>>()
        .map_err(|e| format!("{}: {e}",
            loc(ui_lang, "WAV 解碼失敗", "WAV 解码失败", "WAV decode failed", "WAV 디코딩 실패", "Giải mã WAV thất bại")
        ))?;

    let _ = app.emit("transcribe_progress", 30u8);

    let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
        .map_err(|e| format!("{}: {e}",
            loc(ui_lang, "無法載入 Whisper 模型", "无法加载 Whisper 模型", "Failed to load Whisper model", "Whisper 모델 로드 실패", "Không thể tải mô hình Whisper")
        ))?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(true);
    if let Some(lang) = language {
        // Normalize to ISO 639-1 base code (e.g. "zh-TW" → "zh")
        let base_lang = lang.split('-').next().unwrap_or(lang);
        params.set_language(Some(base_lang));
    }

    let mut state = ctx.create_state().map_err(|e| format!("{}: {e}",
        loc(ui_lang, "Whisper state 錯誤", "Whisper state 错误", "Whisper state error", "Whisper state 오류", "Lỗi Whisper state")
    ))?;
    state
        .full(params, &samples)
        .map_err(|e| format!("{}: {e}",
            loc(ui_lang, "Whisper 轉錄失敗", "Whisper 转录失败", "Whisper transcription failed", "Whisper 전사 실패", "Whisper chuyển âm thất bại")
        ))?;

    let _ = app.emit("transcribe_progress", 90u8);

    // whisper-rs 0.16 API: use as_iter() → WhisperSegment
    // segment.start_timestamp() / end_timestamp() → i64 centiseconds
    // segment (Display) → text
    let mut segments: Vec<Segment> = state
        .as_iter()
        .enumerate()
        .map(|(i, seg)| Segment {
            id: i as u32,
            start: seg.start_timestamp() as f64 / 100.0,
            end: seg.end_timestamp() as f64 / 100.0,
            text: seg.to_string().trim().to_string(),
            translation: None,
        })
        .collect();

    // Clean up temp file
    let _ = std::fs::remove_file(&wav_path);

    let _ = app.emit("transcribe_progress", 100u8);

    Ok(Transcript {
        segments,
        language: language.map(str::to_string),
    })
}

#[cfg(not(feature = "local-whisper"))]
fn transcribe_local_impl(
    _video_path: &str,
    _model_path: &str,
    _language: Option<&str>,
    ui_lang: &str,
    _app: &AppHandle,
) -> Result<Transcript, String> {
    Err(loc(ui_lang,
        "本地 Whisper 功能未編譯。請加入 --features local-whisper 重新建置。",
        "本地 Whisper 功能未编译。请加入 --features local-whisper 重新构建。",
        "Local Whisper is not compiled. Rebuild with --features local-whisper.",
        "로컬 Whisper가 컴파일되지 않았습니다. --features local-whisper 로 재빌드하세요.",
        "Local Whisper chưa được biên dịch. Hãy build lại với --features local-whisper.",
    ))
}

/// Transcribe using OpenAI-compatible API.
async fn transcribe_api_impl(
    video_path: &str,
    base_url: &str,
    api_key: &str,
    model: &str,
    language: Option<&str>,
    ui_lang: &str,
    app: &AppHandle,
) -> Result<Transcript, String> {
    use reqwest::{multipart, Client};
    use serde::Deserialize;

    let _ = app.emit("transcribe_progress", 5u8);

    let file_path = Path::new(video_path);
    let file_size = std::fs::metadata(file_path)
        .map_err(|e| format!("{}: {e}",
            loc(ui_lang, "無法讀取檔案", "无法读取文件", "Failed to read file", "파일 읽기 실패", "Không thể đọc file")
        ))?
        .len();

    // If larger than 24 MB, extract compressed audio first
    let send_path = if file_size > 24 * 1024 * 1024 {
        let _ = app.emit("transcribe_progress", 10u8);
        let wav = extract_audio(video_path, ui_lang)?;
        Some(wav)
    } else {
        None
    };

    let actual_path = send_path.as_deref().unwrap_or(file_path);
    let _ = app.emit("transcribe_progress", 20u8);

    let file_bytes = std::fs::read(actual_path).map_err(|e| format!("{}: {e}",
        loc(ui_lang, "讀取音訊失敗", "读取音频失败", "Failed to read audio", "오디오 읽기 실패", "Không thể đọc audio")
    ))?;
    let file_name = actual_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("audio.wav")
        .to_string();

    let _ = app.emit("transcribe_progress", 30u8);

    let client = Client::new();
    let url = format!("{}/v1/audio/transcriptions", base_url.trim_end_matches('/'));

    // gpt-4o-transcribe / gpt-4o-mini-transcribe only support "json" or "text".
    // whisper-1 supports "verbose_json" which includes segment timestamps.
    let is_gpt4o = model.starts_with("gpt-4o");
    let response_format = if is_gpt4o { "json" } else { "verbose_json" };

    let mut form = multipart::Form::new()
        .text("model", model.to_string())
        .text("response_format", response_format)
        .part(
            "file",
            multipart::Part::bytes(file_bytes).file_name(file_name),
        );

    if let Some(lang) = language {
        // Normalize to ISO 639-1 (e.g. "zh-TW" → "zh"); Whisper API only accepts base codes.
        // gpt-4o models support ISO 639-1 and 639-3 codes per the docs.
        let base_lang = lang.split('-').next().unwrap_or(lang);
        form = form.text("language", base_lang.to_string());
    }

    let _ = app.emit("transcribe_progress", 40u8);

    let resp = client
        .post(&url)
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("{}: {e}",
            loc(ui_lang, "API 請求失敗", "API 请求失败", "API request failed", "API 요청 실패", "Yêu cầu API thất bại")
        ))?;

    let _ = app.emit("transcribe_progress", 80u8);

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        // Try to extract the human-readable message from {"error": {"message": "..."}}
        let friendly = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v["error"]["message"].as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| body.chars().take(200).collect());
        let hint = match status.as_u16() {
            401 => loc(ui_lang,
                " （API Key 無效或未授權）",
                " （API Key 无效或未授权）",
                " (Invalid or unauthorized API Key)",
                " （API Key 무효 또는 미인증）",
                " （API Key không hợp lệ hoặc chưa xác thực）",
            ),
            403 => loc(ui_lang,
                " （存取被拒，請確認 API Key 權限）",
                " （访问被拒，请确认 API Key 权限）",
                " (Access denied, check API Key permissions)",
                " （접근 거부, API Key 권한을 확인하세요）",
                " （Truy cập bị từ chối, kiểm tra quyền API Key）",
            ),
            429 => loc(ui_lang,
                " （配額已超限，請檢查帳單或稍後再試）",
                " （配额已超限，请检查账单或稍后再试）",
                " (Quota exceeded, check billing or retry later)",
                " （할당량 초과, 청구서 확인 또는 나중에 재시도）",
                " （Vượt hạn mức, kiểm tra thanh toán hoặc thử lại sau）",
            ),
            500 | 502 | 503 => loc(ui_lang,
                " （服務暫時不可用，請稍後再試）",
                " （服务暂时不可用，请稍后再试）",
                " (Service temporarily unavailable, retry later)",
                " （서비스 일시 불가, 나중에 재시도）",
                " （Dịch vụ tạm thời không khả dụng, thử lại sau）",
            ),
            _ => String::new(),
        };
        return Err(format!("{} {status}{hint}: {friendly}",
            loc(ui_lang, "API 錯誤", "API 错误", "API error", "API 오류", "Lỗi API")
        ));
    }

    let body = resp.text().await.map_err(|e| format!("{}: {e}",
        loc(ui_lang, "讀取回應失敗", "读取响应失败", "Failed to read response", "응답 읽기 실패", "Không thể đọc phản hồi")
    ))?;

    // Parse verbose_json response
    #[derive(Deserialize)]
    struct ApiSegment {
        id: Option<u32>,
        start: f64,
        end: f64,
        text: String,
    }

    #[derive(Deserialize)]
    struct ApiResponse {
        language: Option<String>,
        segments: Option<Vec<ApiSegment>>,
        // Fallback if no segments
        text: Option<String>,
    }

    let parsed: ApiResponse =
        serde_json::from_str(&body).map_err(|e| format!("{}: {e}\n{}: {body}",
            loc(ui_lang, "解析 API 回應失敗", "解析 API 响应失败", "Failed to parse API response", "API 응답 파싱 실패", "Không thể phân tích phản hồi API"),
            loc(ui_lang, "回應", "响应", "Response", "응답", "Phản hồi"),
        ))?;

    let segments = if let Some(segs) = parsed.segments {
        // verbose_json (whisper-1): has per-segment timestamps
        segs.into_iter()
            .enumerate()
            .map(|(i, s)| Segment {
                id: s.id.unwrap_or(i as u32),
                start: s.start,
                end: s.end,
                text: s.text.trim().to_string(),
                translation: None,
            })
            .collect()
    } else {
        // json (gpt-4o-transcribe / gpt-4o-mini-transcribe) or plain text fallback:
        // no segment timestamps, treat whole response as a single segment.
        vec![Segment {
            id: 0,
            start: 0.0,
            end: 0.0,
            text: parsed.text.unwrap_or_default().trim().to_string(),
            translation: None,
        }]
    };

    // Clean up temp file if created
    if let Some(tmp) = send_path {
        let _ = std::fs::remove_file(tmp);
    }

    let _ = app.emit("transcribe_progress", 100u8);

    Ok(Transcript {
        segments,
        language: parsed.language,
    })
}

#[tauri::command]
pub async fn transcribe(
    app: AppHandle,
    video_path: String,
    config: TranscribeConfig,
    lang: Option<String>,
) -> Result<Transcript, String> {
    let ui_lang = lang.as_deref().unwrap_or("zh-TW");
    match &config.engine {
        Engine::Local {
            model_path,
            language,
        } => {
            let model_path = model_path.clone();
            let language = language.clone();
            let video_path = video_path.clone();
            let ui_lang = ui_lang.to_string();
            let ui_lang_err = ui_lang.clone();  // keep a copy for map_err after the move
            tokio::task::spawn_blocking(move || {
                transcribe_local_impl(&video_path, &model_path, language.as_deref(), &ui_lang, &app)
            })
            .await
            .map_err(|e| format!("{}: {e}",
                loc(&ui_lang_err, "執行緒錯誤", "线程错误", "Thread error", "스레드 오류", "Lỗi thread")
            ))?
        }
        Engine::Api {
            base_url,
            api_key,
            model,
            language,
        } => {
            transcribe_api_impl(
                &video_path,
                base_url,
                api_key,
                model,
                language.as_deref(),
                ui_lang,
                &app,
            )
            .await
        }
    }
}
