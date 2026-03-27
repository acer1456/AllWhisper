use std::fmt::Write as FmtWrite;
use std::fs;

use crate::types::Transcript;

// ── Time formatters ─────────────────────────────────────────────────────────

fn format_srt_time(secs: f64) -> String {
    let h = (secs / 3600.0) as u32;
    let m = ((secs % 3600.0) / 60.0) as u32;
    let s = (secs % 60.0) as u32;
    let ms = ((secs % 1.0) * 1000.0) as u32;
    format!("{:02}:{:02}:{:02},{:03}", h, m, s, ms)
}

fn format_vtt_time(secs: f64) -> String {
    let h = (secs / 3600.0) as u32;
    let m = ((secs % 3600.0) / 60.0) as u32;
    let s = (secs % 60.0) as u32;
    let ms = ((secs % 1.0) * 1000.0) as u32;
    format!("{:02}:{:02}:{:02}.{:03}", h, m, s, ms)
}

// ── Content helpers ─────────────────────────────────────────────────────────

/// Returns the text to display for a segment given the content mode.
/// mode: "original" | "translation" | "bilingual"
fn seg_text(text: &str, translation: Option<&str>, mode: &str) -> String {
    match mode {
        "translation" => translation.unwrap_or(text).to_string(),
        "bilingual" => {
            if let Some(tr) = translation {
                format!("{}\n{}", text.trim(), tr.trim())
            } else {
                text.to_string()
            }
        }
        _ => text.to_string(), // "original" and anything else
    }
}

// ── Format writers ──────────────────────────────────────────────────────────

fn to_txt(t: &Transcript, mode: &str) -> String {
    t.segments
        .iter()
        .map(|s| seg_text(s.text.trim(), s.translation.as_deref(), mode))
        .collect::<Vec<_>>()
        .join("\n")
}

fn to_srt(t: &Transcript, mode: &str) -> String {
    let mut out = String::new();
    for (i, seg) in t.segments.iter().enumerate() {
        writeln!(out, "{}", i + 1).unwrap();
        writeln!(
            out,
            "{} --> {}",
            format_srt_time(seg.start),
            format_srt_time(seg.end)
        )
        .unwrap();
        writeln!(out, "{}", seg_text(seg.text.trim(), seg.translation.as_deref(), mode)).unwrap();
        writeln!(out).unwrap();
    }
    out
}

fn to_vtt(t: &Transcript, mode: &str) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for (i, seg) in t.segments.iter().enumerate() {
        writeln!(out, "{}", i + 1).unwrap();
        writeln!(
            out,
            "{} --> {}",
            format_vtt_time(seg.start),
            format_vtt_time(seg.end)
        )
        .unwrap();
        writeln!(out, "{}", seg_text(seg.text.trim(), seg.translation.as_deref(), mode)).unwrap();
        writeln!(out).unwrap();
    }
    out
}

fn to_json(t: &Transcript) -> Result<String, String> {
    serde_json::to_string_pretty(t).map_err(|e| format!("JSON 序列化失敗：{e}"))
}

// ── Tauri command ────────────────────────────────────────────────────────────

/// format:       "txt" | "srt" | "vtt" | "json"
/// content_mode: "original" | "translation" | "bilingual"  (default: "original")
#[tauri::command]
pub fn export_transcript(
    transcript: Transcript,
    format: String,
    output_path: String,
    content_mode: Option<String>,
) -> Result<(), String> {
    let mode = content_mode.as_deref().unwrap_or("original");

    let content = match format.as_str() {
        "txt"  => to_txt(&transcript, mode),
        "srt"  => to_srt(&transcript, mode),
        "vtt"  => to_vtt(&transcript, mode),
        "json" => to_json(&transcript)?,
        other  => return Err(format!("不支援的格式：{other}")),
    };

    fs::write(&output_path, content.as_bytes())
        .map_err(|e| format!("寫入檔案失敗：{e}"))?;

    Ok(())
}
