use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// ── Request / Response types ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SegmentInput {
    pub id: u32,
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct TranslateRequest {
    pub segments: Vec<SegmentInput>,
    pub source_language: Option<String>,
    pub target_language: String,
    pub service: String,
    pub api_key: Option<String>,
    pub endpoint: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TranslatedSegment {
    pub id: u32,
    pub translation: String,
}

// ── Main Tauri command ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn translate_transcript(req: TranslateRequest) -> Result<Vec<TranslatedSegment>, String> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("建立 HTTP 客戶端失敗：{e}"))?;

    match req.service.as_str() {
        "free_google"    => translate_free_google(&req).await,
        "google"         => translate_google(&client, &req).await,
        "bing"           => translate_bing(&client, &req).await,
        "libretranslate" => translate_libre(&client, &req).await,
        "openai" | "grok" | "openrouter" => translate_openai_compat(&client, &req).await,
        "gemini"         => translate_gemini(&client, &req).await,
        "claude"         => translate_claude(&client, &req).await,
        other => Err(format!("不支援的翻譯服務：{other}")),
    }
}

// ── AI helpers ─────────────────────────────────────────────────────────────

fn build_ai_prompt(segments: &[SegmentInput], target_language: &str) -> String {
    let input: Vec<Value> = segments
        .iter()
        .map(|s| json!({"id": s.id, "text": s.text}))
        .collect();
    let input_json = serde_json::to_string(&input).unwrap_or_default();

    format!(
        "Translate the following subtitle segments to {target_language}. \
         Return ONLY a valid JSON array in this exact format (no markdown, no explanation): \
         [{{\"id\": <number>, \"translation\": \"<translated text>\"}}]\n\n\
         Input:\n{input_json}"
    )
}

fn parse_ai_response(content: &str) -> Result<Vec<TranslatedSegment>, String> {
    // Strip potential markdown code fences
    let start = content.find('[').unwrap_or(0);
    let end = content.rfind(']').map(|i| i + 1).unwrap_or(content.len());
    let json_str = &content[start..end];

    serde_json::from_str::<Vec<TranslatedSegment>>(json_str)
        .map_err(|e| format!("解析翻譯結果失敗：{e}\n原始回應：{content}"))
}

// ── OpenAI-compatible (OpenAI / Grok / OpenRouter) ─────────────────────────

async fn translate_openai_compat(
    client: &Client,
    req: &TranslateRequest,
) -> Result<Vec<TranslatedSegment>, String> {
    let key = req.api_key.as_deref().unwrap_or("");
    let base = match req.service.as_str() {
        "grok"       => "https://api.x.ai/v1".to_string(),
        "openrouter" => req.endpoint.clone()
            .unwrap_or_else(|| "https://openrouter.ai/api/v1".to_string()),
        _            => "https://api.openai.com/v1".to_string(),
    };
    let model = req.model.as_deref().unwrap_or("gpt-4o-mini");
    let prompt = build_ai_prompt(&req.segments, &req.target_language);

    let resp = client
        .post(format!("{base}/chat/completions"))
        .header("Authorization", format!("Bearer {key}"))
        .json(&json!({
            "model": model,
            "temperature": 0.1,
            "messages": [
                {"role": "system", "content": "You are a professional subtitle translator. Return only valid JSON."},
                {"role": "user", "content": prompt}
            ]
        }))
        .send()
        .await
        .map_err(|e| format!("翻譯請求失敗：{e}"))?;

    let body: Value = resp.json().await.map_err(|e| format!("解析回應失敗：{e}"))?;

    // Detect API-level errors (e.g. {"error": {"message": "..."}})
    if let Some(err) = body.get("error") {
        let msg = err["message"].as_str().unwrap_or_else(|| err.as_str().unwrap_or("未知錯誤"));
        return Err(format!("API 錯誤（{}）：{msg}", req.service));
    }

    let content = body["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| format!("無效的回應格式：{body}"))?;

    parse_ai_response(content)
}

// ── Claude (Anthropic) ──────────────────────────────────────────────────────

async fn translate_claude(
    client: &Client,
    req: &TranslateRequest,
) -> Result<Vec<TranslatedSegment>, String> {
    let key = req.api_key.as_deref().unwrap_or("");
    let model = req.model.as_deref().unwrap_or("claude-3-haiku-20240307");
    let prompt = build_ai_prompt(&req.segments, &req.target_language);

    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .json(&json!({
            "model": model,
            "max_tokens": 8192,
            "messages": [{"role": "user", "content": prompt}]
        }))
        .send()
        .await
        .map_err(|e| format!("翻譯請求失敗：{e}"))?;

    let body: Value = resp.json().await.map_err(|e| format!("解析回應失敗：{e}"))?;

    if let Some(err) = body.get("error") {
        let msg = err["message"].as_str().unwrap_or("未知錯誤");
        return Err(format!("API 錯誤（Claude）：{msg}"));
    }

    let content = body["content"][0]["text"]
        .as_str()
        .ok_or_else(|| format!("無效的回應格式：{body}"))?;

    parse_ai_response(content)
}

// ── Gemini (Google AI) ──────────────────────────────────────────────────────

async fn translate_gemini(
    client: &Client,
    req: &TranslateRequest,
) -> Result<Vec<TranslatedSegment>, String> {
    let key = req.api_key.as_deref().unwrap_or("");
    let model = req.model.as_deref().unwrap_or("gemini-1.5-flash");
    let prompt = build_ai_prompt(&req.segments, &req.target_language);

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={key}"
    );

    let resp = client
        .post(&url)
        .json(&json!({
            "contents": [{"parts": [{"text": prompt}]}],
            "generationConfig": {"temperature": 0.1}
        }))
        .send()
        .await
        .map_err(|e| format!("翻譯請求失敗：{e}"))?;

    let body: Value = resp.json().await.map_err(|e| format!("解析回應失敗：{e}"))?;

    if let Some(err) = body.get("error") {
        let msg = err["message"].as_str().unwrap_or("未知錯誤");
        return Err(format!("API 錯誤（Gemini）：{msg}"));
    }

    let content = body["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .ok_or_else(|| format!("無效的回應格式：{body}"))?;

    parse_ai_response(content)
}

// ── Google Translate (Cloud Translation API v2) ─────────────────────────────

async fn translate_google(
    client: &Client,
    req: &TranslateRequest,
) -> Result<Vec<TranslatedSegment>, String> {
    let key = req.api_key.as_deref().unwrap_or("");
    // Google API uses language codes without region subtag (e.g. "zh-TW" → "zh-TW" is ok,
    // but "zh" means Simplified; we pass the full tag and let Google handle it)
    let target = &req.target_language;

    let texts: Vec<&str> = req.segments.iter().map(|s| s.text.as_str()).collect();

    let resp = client
        .post(format!(
            "https://translation.googleapis.com/language/translate/v2?key={key}"
        ))
        .json(&json!({ "q": texts, "target": target, "format": "text" }))
        .send()
        .await
        .map_err(|e| format!("翻譯請求失敗：{e}"))?;

    let body: Value = resp.json().await.map_err(|e| format!("解析回應失敗：{e}"))?;
    let translations = body["data"]["translations"]
        .as_array()
        .ok_or_else(|| format!("無效的回應格式：{body}"))?;

    Ok(req
        .segments
        .iter()
        .zip(translations.iter())
        .map(|(seg, t)| TranslatedSegment {
            id: seg.id,
            translation: t["translatedText"].as_str().unwrap_or("").to_string(),
        })
        .collect())
}

// ── Bing Translator (Azure Cognitive Services) ─────────────────────────────

async fn translate_bing(
    client: &Client,
    req: &TranslateRequest,
) -> Result<Vec<TranslatedSegment>, String> {
    let key = req.api_key.as_deref().unwrap_or("");
    let target = &req.target_language;
    let texts: Vec<Value> = req
        .segments
        .iter()
        .map(|s| json!({"Text": s.text}))
        .collect();

    let resp = client
        .post(format!(
            "https://api.cognitive.microsofttranslator.com/translate?api-version=3.0&to={target}"
        ))
        .header("Ocp-Apim-Subscription-Key", key)
        .header("Ocp-Apim-Subscription-Region", "global")
        .json(&texts)
        .send()
        .await
        .map_err(|e| format!("翻譯請求失敗：{e}"))?;

    let body: Value = resp.json().await.map_err(|e| format!("解析回應失敗：{e}"))?;
    let arr = body.as_array().ok_or_else(|| format!("無效的回應格式：{body}"))?;

    Ok(req
        .segments
        .iter()
        .zip(arr.iter())
        .map(|(seg, item)| TranslatedSegment {
            id: seg.id,
            translation: item["translations"][0]["text"]
                .as_str()
                .unwrap_or("")
                .to_string(),
        })
        .collect())
}

// ── LibreTranslate ─────────────────────────────────────────────────────────

async fn translate_libre(
    client: &Client,
    req: &TranslateRequest,
) -> Result<Vec<TranslatedSegment>, String> {
    let endpoint = req
        .endpoint
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("https://libretranslate.com");
    let key = req.api_key.as_deref().unwrap_or("");
    // LibreTranslate uses two-letter language codes
    let target = req
        .target_language
        .split('-')
        .next()
        .unwrap_or(&req.target_language);

    let mut results = Vec::with_capacity(req.segments.len());

    for seg in &req.segments {
        let mut body = json!({
            "q": seg.text,
            "source": "auto",
            "target": target,
            "format": "text"
        });
        if !key.is_empty() {
            body["api_key"] = json!(key);
        }

        let resp = client
            .post(format!("{endpoint}/translate"))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("翻譯請求失敗：{e}"))?;

        let json: Value = resp.json().await.map_err(|e| format!("解析回應失敗：{e}"))?;
        results.push(TranslatedSegment {
            id: seg.id,
            translation: json["translatedText"].as_str().unwrap_or("").to_string(),
        });
    }

    Ok(results)
}

// ── Free Google Translate (no API key, using translators crate) ────────────

async fn translate_free_google(req: &TranslateRequest) -> Result<Vec<TranslatedSegment>, String> {
    use translators::{GoogleTranslator, Translator};
    use tokio::time::{sleep, Duration};

    let translator = GoogleTranslator::default();
    // Use the transcription source language; fall back to "zh-CN" only if unknown.
    let source = req
        .source_language
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("zh-CN");
    let target = req.target_language.as_str();
    let mut results = Vec::with_capacity(req.segments.len());

    for (i, seg) in req.segments.iter().enumerate() {
        // Delay between requests to avoid hitting rate limits.
        // Skip delay before the very first request.
        if i > 0 {
            sleep(Duration::from_millis(100)).await;
        }

        let translated = translator
            .translate_async(&seg.text, source, target)
            .await
            .map_err(|e| format!("Free Google 翻譯失敗（第 {} 段）：{}", i + 1, e))?;

        results.push(TranslatedSegment {
            id: seg.id,
            translation: translated,
        });
    }

    Ok(results)
}
