<script lang="ts">
  import { videoPath, videoElement, currentTime, transcript, isTranscribing, transcribeProgress, errorMsg, settings, sessions, activeSessionId } from "../stores/app";
  import { lang, t } from "../i18n";
  import { get } from "svelte/store";
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import type { Transcript, Session } from "../types";
  import { convertChineseScript } from "../utils/chinese-convert";
  import { onMount, onDestroy } from "svelte";

  let videoEl: HTMLVideoElement;
  let isDragOver = false;
  let unlisteners: Array<() => void> = [];

  const VIDEO_EXTS = new Set(["mp4", "mov", "mkv", "avi", "webm", "m4v", "mp3", "m4a", "wav", "flac", "ogg"]);

  function isVideoFile(path: string) {
    const ext = path.split(".").pop()?.toLowerCase() ?? "";
    return VIDEO_EXTS.has(ext);
  }

  onMount(async () => {
    // Progress events from Rust
    const unlistenProgress = await listen<number>("transcribe_progress", (event) => {
      transcribeProgress.set(event.payload);
    });
    unlisteners.push(unlistenProgress);

    // Tauri 2 native drag-drop — this is the only reliable way to get file paths
    const unlistenDrop = await getCurrentWindow().onDragDropEvent((event) => {
      if (event.payload.type === "over") {
        isDragOver = true;
      } else if (event.payload.type === "cancel") {
        isDragOver = false;
      } else if (event.payload.type === "drop") {
        isDragOver = false;
        const paths: string[] = event.payload.paths ?? [];
        const first = paths.find(isVideoFile) ?? paths[0];
        if (first) loadVideo(first);
      }
    });
    unlisteners.push(unlistenDrop);
  });

  onDestroy(() => {
    for (const fn of unlisteners) fn();
  });

  $: if (videoEl) {
    videoElement.set(videoEl);
  }

  $: videoSrc = $videoPath ? convertFileSrc($videoPath) : null;

  function onTimeUpdate() {
    currentTime.set(videoEl.currentTime);
  }

  // DOM drag events — only used for visual feedback (CSS hover state).
  // Actual file paths come from getCurrentWindow().onDragDropEvent() above.
  function onDragOver(e: DragEvent) {
    e.preventDefault();
    isDragOver = true;
  }

  function onDragLeave(e: DragEvent) {
    // Only clear when leaving the drop zone entirely
    if (!(e.currentTarget as HTMLElement).contains(e.relatedTarget as Node)) {
      isDragOver = false;
    }
  }

  function onDrop(e: DragEvent) {
    // Prevent browser from navigating; actual handling is done by Tauri event above
    e.preventDefault();
  }

  async function pickFile() {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: $t.video.filePickerName, extensions: ["mp4", "mov", "mkv", "avi", "webm", "m4v", "mp3", "m4a", "wav", "flac"] }],
      });
      if (selected && typeof selected === "string") {
        loadVideo(selected);
      }
    } catch (e) {
      errorMsg.set(String(e));
    }
  }

  function loadVideo(path: string) {
    videoPath.set(path);
    transcript.set(null);
    errorMsg.set(null);
    transcribeProgress.set(0);
  }

  async function startTranscribe() {
    if (!$videoPath) return;
    isTranscribing.set(true);
    errorMsg.set(null);
    transcribeProgress.set(0);
    try {
      const raw: Transcript = await invoke("transcribe", {
        videoPath: $videoPath,
        config: $settings.engine,
        lang: get(lang),
      });
      // Apply Simplified ↔ Traditional conversion based on language selection
      const transcriptLang = $settings.engine.language;
      const result: Transcript = {
        ...raw,
        segments: raw.segments.map((seg) => ({
          ...seg,
          text: convertChineseScript(seg.text, transcriptLang),
        })),
      };
      transcript.set(result);
      saveSession(result);
    } catch (e) {
      errorMsg.set(String(e));
    } finally {
      isTranscribing.set(false);
    }
  }

  function saveSession(t: Transcript) {
    if (!$videoPath) return;
    const fileName = $videoPath.split(/[/\\]/).pop() ?? $videoPath;
    const title = fileName.replace(/\.[^.]+$/, "");
    const session: Session = {
      id: crypto.randomUUID(),
      title,
      video_path: $videoPath,
      transcript: t,
      created_at: Date.now(),
    };
    sessions.update(s => [...s, session]);
    activeSessionId.set(session.id);
  }

  function formatDuration(s: number) {
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const sec = Math.floor(s % 60);
    if (h > 0) return `${h}:${String(m).padStart(2, "0")}:${String(sec).padStart(2, "0")}`;
    return `${String(m).padStart(2, "0")}:${String(sec).padStart(2, "0")}`;
  }
</script>

<div class="video-panel">
  {#if !$videoPath}
    <!-- Drop zone -->
    <div
      class="drop-zone"
      class:drag-over={isDragOver}
      role="button"
      tabindex="0"
      ondragover={onDragOver}
      ondragleave={onDragLeave}
      ondrop={onDrop}
      onclick={pickFile}
      onkeydown={(e) => e.key === "Enter" && pickFile()}
    >
      <div class="drop-icon">
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
          <rect x="2" y="3" width="20" height="14" rx="2" ry="2"/>
          <polygon points="10,8 16,11 10,14"/>
          <line x1="8" y1="21" x2="16" y2="21"/>
          <line x1="12" y1="17" x2="12" y2="21"/>
        </svg>
      </div>
      <div class="drop-title">{$t.video.dropTitle}</div>
      <div class="drop-sub">{$t.video.dropSub}</div>
      <div class="drop-formats">{$t.video.dropFormats}</div>
    </div>
  {:else}
    <!-- Video Player -->
    <div class="player-wrapper">
      <!-- svelte-ignore a11y_media_has_caption -->
      <video
        bind:this={videoEl}
        src={videoSrc}
        ontimeupdate={onTimeUpdate}
        class="video-el"
        controls
      ></video>
    </div>

    <!-- Controls bar -->
    <div class="controls-bar">
      <button class="change-file-btn" onclick={pickFile} title={$t.video.changeFile}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
          <polyline points="17 8 12 3 7 8"/>
          <line x1="12" y1="3" x2="12" y2="15"/>
        </svg>
        {$t.video.changeFile}
      </button>

      <div class="file-name" title={$videoPath}>
        {$videoPath?.split(/[/\\]/).pop()}
      </div>

      <!-- Language selector: directly updates settings.engine.language -->
      <div class="lang-wrapper" title="選擇語言，中文請選「中文」以確保準確度">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="flex-shrink:0;opacity:0.6">
          <circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/>
          <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>
        </svg>
        <select
          class="lang-select"
          value={$settings.engine.language ?? ""}
          onchange={(e) => {
            const v = (e.target as HTMLSelectElement).value;
            settings.update(s => ({
              ...s,
              engine: { ...s.engine, language: v || null }
            }));
          }}
        >
          <option value="">{$t.video.autoDetect}</option>
          <option value="zh-TW">{$t.video.langZhTW}</option>
          <option value="zh-CN">{$t.video.langZhCN}</option>
          <option value="yue">{$t.video.langYue}</option>
          <option value="en">{$t.video.langEn}</option>
          <option value="ja">{$t.video.langJa}</option>
          <option value="ko">{$t.video.langKo}</option>
          <option value="fr">{$t.video.langFr}</option>
          <option value="de">{$t.video.langDe}</option>
          <option value="es">{$t.video.langEs}</option>
        </select>
      </div>

      <button
        class="transcribe-btn"
        onclick={startTranscribe}
        disabled={$isTranscribing}
      >
        {#if $isTranscribing}
          <span class="spinner"></span>
          {$t.video.transcribing}{$transcribeProgress > 0 ? ` ${$transcribeProgress}%` : ""}
        {:else}
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z"/>
            <path d="M19 10v2a7 7 0 0 1-14 0v-2"/>
            <line x1="12" y1="19" x2="12" y2="23"/>
            <line x1="8" y1="23" x2="16" y2="23"/>
          </svg>
          {$t.video.transcribeBtn}
        {/if}
      </button>
    </div>

    {#if $errorMsg}
      <div class="error-bar">{$errorMsg}</div>
    {/if}
  {/if}
</div>

<style>
  .video-panel {
    flex: 1;
    width: 100%;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: #1a1a1a;
    overflow: hidden;
  }

  .drop-zone {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    cursor: pointer;
    border: 2px dashed transparent;
    border-radius: 12px;
    margin: 20px;
    transition: border-color 0.2s, background 0.2s;
    color: #8e8ea0;
  }

  .drop-zone:hover,
  .drop-zone.drag-over {
    border-color: #4b4b6a;
    background: #1f1f2e;
    color: #adadc9;
  }

  .drop-icon {
    opacity: 0.5;
    margin-bottom: 8px;
  }

  .drop-title {
    font-size: 18px;
    font-weight: 500;
    color: #c5c5d2;
  }

  .drop-sub {
    font-size: 13px;
  }

  .drop-formats {
    font-size: 11px;
    margin-top: 4px;
    opacity: 0.6;
  }

  .player-wrapper {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #000;
    overflow: hidden;
  }

  .video-el {
    max-width: 100%;
    max-height: 100%;
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  .controls-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    background: #1f1f1f;
    border-top: 1px solid #2f2f2f;
  }

  .change-file-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 6px 10px;
    border: 1px solid #3f3f3f;
    border-radius: 6px;
    background: transparent;
    color: #8e8ea0;
    font-size: 12px;
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.15s, color 0.15s;
  }

  .change-file-btn:hover {
    background: #2f2f2f;
    color: #ececec;
  }

  .file-name {
    flex: 1;
    font-size: 12px;
    color: #8e8ea0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .lang-wrapper {
    display: flex;
    align-items: center;
    gap: 5px;
    flex-shrink: 0;
    color: #8e8ea0;
  }

  .lang-select {
    appearance: none;
    padding: 5px 24px 5px 8px;
    background: #1a1a1a url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='10' viewBox='0 0 24 24' fill='none' stroke='%238e8ea0' stroke-width='2'%3E%3Cpolyline points='6 9 12 15 18 9'/%3E%3C/svg%3E") no-repeat right 6px center;
    border: 1px solid #3f3f3f;
    border-radius: 6px;
    color: #c5c5d2;
    font-size: 12px;
    cursor: pointer;
    outline: none;
    transition: border-color 0.15s;
    min-width: 110px;
  }

  .lang-select:focus {
    border-color: #10a37f;
  }

  .lang-select option {
    background: #2f2f2f;
    color: #ececec;
  }

  .transcribe-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 16px;
    border: none;
    border-radius: 8px;
    background: #10a37f;
    color: #fff;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.15s, opacity 0.15s;
  }

  .transcribe-btn:hover:not(:disabled) {
    background: #0d8a6a;
  }

  .transcribe-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .spinner {
    width: 12px;
    height: 12px;
    border: 2px solid rgba(255,255,255,0.3);
    border-top-color: #fff;
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
    display: inline-block;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .error-bar {
    padding: 8px 14px;
    background: #3d1515;
    color: #f87171;
    font-size: 12px;
    border-top: 1px solid #5a2020;
  }
</style>
