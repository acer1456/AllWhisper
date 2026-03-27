use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub filename: String,
    pub url: String,
    pub size_mb: u32,
    pub description: String,   // kept for backwards-compat; frontend now uses i18n desc
    pub downloaded: bool,
    pub path: Option<String>,
}

/// Return a localised string based on the lang code.
/// Defaults to zh-TW for any unrecognised value.
fn loc<'a>(lang: &str, zh_tw: &'a str, zh_cn: &'a str, en: &'a str) -> &'a str {
    match lang {
        "zh-CN" => zh_cn,
        "en"    => en,
        _       => zh_tw,
    }
}

fn model_catalog() -> Vec<ModelInfo> {
    let base = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";
    vec![
        ModelInfo {
            name: "tiny".into(),
            filename: "ggml-tiny.bin".into(),
            url: format!("{base}/ggml-tiny.bin"),
            size_mb: 75,
            description: "tiny".into(), // frontend i18n key
            downloaded: false,
            path: None,
        },
        ModelInfo {
            name: "base".into(),
            filename: "ggml-base.bin".into(),
            url: format!("{base}/ggml-base.bin"),
            size_mb: 142,
            description: "base".into(),
            downloaded: false,
            path: None,
        },
        ModelInfo {
            name: "small".into(),
            filename: "ggml-small.bin".into(),
            url: format!("{base}/ggml-small.bin"),
            size_mb: 466,
            description: "small".into(),
            downloaded: false,
            path: None,
        },
        ModelInfo {
            name: "medium".into(),
            filename: "ggml-medium.bin".into(),
            url: format!("{base}/ggml-medium.bin"),
            size_mb: 1_500,
            description: "medium".into(),
            downloaded: false,
            path: None,
        },
        ModelInfo {
            name: "large-v3".into(),
            filename: "ggml-large-v3.bin".into(),
            url: format!("{base}/ggml-large-v3.bin"),
            size_mb: 2_900,
            description: "large-v3".into(),
            downloaded: false,
            path: None,
        },
        ModelInfo {
            name: "large-v3-turbo".into(),
            filename: "ggml-large-v3-turbo.bin".into(),
            url: format!("{base}/ggml-large-v3-turbo.bin"),
            size_mb: 809,
            description: "large-v3-turbo".into(),
            downloaded: false,
            path: None,
        },
    ]
}

fn models_dir(app: &AppHandle, lang: &str) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("{}: {e}", loc(lang,
            "無法取得 App 資料目錄",
            "无法获取 App 数据目录",
            "Cannot locate app data directory")))?
        .join("models");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", loc(lang,
        "無法建立 models 目錄",
        "无法创建 models 目录",
        "Cannot create models directory")))?;
    Ok(dir)
}

#[tauri::command]
pub fn get_models_dir(app: AppHandle) -> Result<String, String> {
    models_dir(&app, "zh-TW").map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
pub fn list_models(app: AppHandle, lang: Option<String>) -> Result<Vec<ModelInfo>, String> {
    let l = lang.as_deref().unwrap_or("zh-TW");
    let dir = models_dir(&app, l)?;
    let mut catalog = model_catalog();
    for m in &mut catalog {
        let path = dir.join(&m.filename);
        if path.exists() {
            m.downloaded = true;
            m.path = Some(path.to_string_lossy().to_string());
        }
    }
    Ok(catalog)
}

#[tauri::command]
pub async fn download_model(
    app: AppHandle,
    name: String,
    lang: Option<String>,
) -> Result<String, String> {
    use reqwest::Client;
    use tokio::io::AsyncWriteExt;

    let l = lang.as_deref().unwrap_or("zh-TW");
    let dir = models_dir(&app, l)?;
    let catalog = model_catalog();
    let model = catalog
        .iter()
        .find(|m| m.name == name)
        .ok_or_else(|| format!("{}: {name}", loc(l,
            "找不到模型",
            "找不到模型",
            "Model not found")))?;

    let dest = dir.join(&model.filename);
    if dest.exists() {
        return Ok(dest.to_string_lossy().to_string());
    }

    let temp = dest.with_extension("tmp");
    let _ = std::fs::remove_file(&temp);

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(3600))
        .build()
        .map_err(|e| format!("{}: {e}", loc(l,
            "建立 HTTP client 失敗",
            "创建 HTTP client 失败",
            "Failed to create HTTP client")))?;

    let mut resp = client
        .get(&model.url)
        .send()
        .await
        .map_err(|e| format!("{}: {e}", loc(l,
            "下載請求失敗",
            "下载请求失败",
            "Download request failed")))?;

    if !resp.status().is_success() {
        return Err(format!("{}: HTTP {}", loc(l,
            "下載失敗",
            "下载失败",
            "Download failed"),
            resp.status()));
    }

    let total = resp.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    let mut file = tokio::fs::File::create(&temp)
        .await
        .map_err(|e| format!("{}: {e}", loc(l,
            "建立暫存檔失敗",
            "创建临时文件失败",
            "Failed to create temp file")))?;

    let _ = app.emit(
        "model_download_progress",
        serde_json::json!({ "name": name, "progress": 0.0, "downloaded": 0, "total": total }),
    );

    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| format!("{}: {e}", loc(l,
            "讀取資料失敗",
            "读取数据失败",
            "Failed to read data")))?
    {
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("{}: {e}", loc(l,
                "寫入失敗",
                "写入失败",
                "Write failed")))?;
        downloaded += chunk.len() as u64;

        let pct = if total > 0 {
            (downloaded as f64 / total as f64 * 100.0).min(100.0)
        } else {
            0.0
        };

        let _ = app.emit(
            "model_download_progress",
            serde_json::json!({
                "name": name,
                "progress": pct,
                "downloaded": downloaded,
                "total": total,
            }),
        );
    }

    file.flush().await.map_err(|e| format!("{}: {e}", loc(l,
        "刷新失敗",
        "刷新失败",
        "Flush failed")))?;
    drop(file);

    tokio::fs::rename(&temp, &dest)
        .await
        .map_err(|e| format!("{}: {e}", loc(l,
            "重命名失敗",
            "重命名失败",
            "Rename failed")))?;

    let _ = app.emit(
        "model_download_progress",
        serde_json::json!({ "name": name, "progress": 100.0, "downloaded": total, "total": total }),
    );

    Ok(dest.to_string_lossy().to_string())
}

#[tauri::command]
pub fn delete_model(
    app: AppHandle,
    name: String,
    lang: Option<String>,
) -> Result<(), String> {
    let l = lang.as_deref().unwrap_or("zh-TW");
    let dir = models_dir(&app, l)?;
    let catalog = model_catalog();
    let model = catalog
        .iter()
        .find(|m| m.name == name)
        .ok_or_else(|| format!("{}: {name}", loc(l,
            "找不到模型",
            "找不到模型",
            "Model not found")))?;
    let path = dir.join(&model.filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("{}: {e}", loc(l,
            "刪除失敗",
            "删除失败",
            "Delete failed")))?;
    }
    Ok(())
}
