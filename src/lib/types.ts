export interface Segment {
  id: number;
  start: number; // seconds
  end: number;   // seconds
  text: string;
  translation?: string; // translated text (populated after calling translate_transcript)
}

export interface Transcript {
  segments: Segment[];
  language: string | null;
}

export type ExportFormat = "txt" | "srt" | "vtt" | "json";

export type EngineType = "local" | "api";

export interface LocalEngineConfig {
  type: "local";
  model_path: string;
  model_name?: string;   // model identifier for reliable cross-session matching
  language: string | null;
}

export interface ApiEngineConfig {
  type: "api";
  base_url: string;
  api_key: string;
  model: string;
  language: string | null;
}

export type EngineConfig = LocalEngineConfig | ApiEngineConfig;

export interface Session {
  id: string;
  title: string;
  video_path: string;
  transcript: Transcript;
  created_at: number; // unix ms
}

// ── Translation ────────────────────────────────────────────────────────────────

export type TranslationServiceType =
  | "free_google"
  | "google"
  | "bing"
  | "libretranslate"
  | "openai"
  | "gemini"
  | "grok"
  | "claude"
  | "openrouter";

/** Per-service credentials and config. Each service stores its own independently. */
export interface ServiceConfig {
  api_key: string;
  endpoint: string;
  model: string;
}

export interface TranslationConfig {
  service: TranslationServiceType;
  /** Each service's config is stored under its own key so switching services
   *  never leaks credentials from one service into another. */
  configs: Partial<Record<TranslationServiceType, ServiceConfig>>;
}

export interface AppSettings {
  engine: EngineConfig;
  translation: TranslationConfig;
}
