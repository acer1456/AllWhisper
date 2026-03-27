use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub id: u32,
    pub start: f64,
    pub end: f64,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub translation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcript {
    pub segments: Vec<Segment>,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Engine {
    Local {
        model_path: String,
        language: Option<String>,
    },
    Api {
        base_url: String,
        api_key: String,
        model: String,
        language: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscribeConfig {
    #[serde(flatten)]
    pub engine: Engine,
}
