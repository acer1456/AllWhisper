import { writable, derived } from "svelte/store";
import type { Transcript, Session, AppSettings } from "../types";

// Currently loaded video file path
export const videoPath = writable<string | null>(null);

// Reference to the HTMLVideoElement for seeking
export const videoElement = writable<HTMLVideoElement | null>(null);

// Current playback time (seconds), updated via timeupdate
export const currentTime = writable<number>(0);

// Transcription result for the current video
export const transcript = writable<Transcript | null>(null);

// Whether transcription is running
export const isTranscribing = writable<boolean>(false);

// Progress 0–100 from backend events
export const transcribeProgress = writable<number>(0);

// Error message
export const errorMsg = writable<string | null>(null);

// Session history (persisted via tauri-plugin-store)
export const sessions = writable<Session[]>([]);

// Currently selected session id (null = new unsaved session)
export const activeSessionId = writable<string | null>(null);

// App-wide settings
const defaultSettings: AppSettings = {
  engine: {
    type: "api",
    base_url: "https://api.openai.com",
    api_key: "",
    model: "whisper-1",
    language: null,
  },
  translation: {
    service: "openai",
    configs: {},
  },
};
export const settings = writable<AppSettings>(defaultSettings);

// Translation state
export const isTranslating = writable<boolean>(false);
export const translationTargetLang = writable<string>("zh-TW");

// Settings modal open state
export const settingsOpen = writable<boolean>(false);

// Derived: the active transcript segment index during playback
export const activeSegmentIndex = derived(
  [transcript, currentTime],
  ([$transcript, $currentTime]) => {
    if (!$transcript) return -1;
    for (let i = $transcript.segments.length - 1; i >= 0; i--) {
      if ($currentTime >= $transcript.segments[i].start) return i;
    }
    return -1;
  }
);
