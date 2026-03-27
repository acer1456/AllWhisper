<script lang="ts">
  import {
    transcript, isTranscribing, activeSegmentIndex, videoPath, errorMsg,
    settings, sessions, activeSessionId, isTranslating, translationTargetLang,
  } from "../stores/app";
  import TranscriptItem from "./TranscriptItem.svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { save } from "@tauri-apps/plugin-dialog";
  import { get } from "svelte/store";
  import { tick } from "svelte";
  import { t } from "../i18n";

  // ── Auto-scroll to active segment ────────────────────────────────────────
  let listEl: HTMLDivElement;
  let prevActiveIndex = -1;

  $: if ($activeSegmentIndex !== prevActiveIndex && $activeSegmentIndex >= 0 && listEl) {
    prevActiveIndex = $activeSegmentIndex;
    tick().then(() => {
      const item = listEl.querySelector(`[data-idx="${$activeSegmentIndex}"]`);
      item?.scrollIntoView({ block: "nearest", behavior: "smooth" });
    });
  }

  // ── Check if any translation exists ──────────────────────────────────────
  $: hasTranslation = !!$transcript?.segments.some(s => s.translation);

  // ── Translation ───────────────────────────────────────────────────────────
  const TARGET_LANGS = [
    { value: "zh-TW", label: "正體中文" },
    { value: "zh-CN", label: "简体中文" },
    { value: "en",    label: "English" },
    { value: "ja",    label: "日本語" },
    { value: "ko",    label: "한국어" },
    { value: "fr",    label: "Français" },
    { value: "de",    label: "Deutsch" },
    { value: "es",    label: "Español" },
    { value: "pt",    label: "Português" },
    { value: "ru",    label: "Русский" },
    { value: "ar",    label: "العربية" },
    { value: "vi",    label: "Tiếng Việt" },
    { value: "th",    label: "ภาษาไทย" },
    { value: "id",    label: "Indonesia" },
  ];

  async function translateTranscript() {
    if (!$transcript || $isTranslating) return;
    const cfg = $settings.translation;
    if (!cfg) { errorMsg.set("請先在設定中配置翻譯服務"); return; }

    // Resolve per-service config; also handles legacy flat format gracefully
    const svcCfg = cfg.configs?.[cfg.service] ?? {
      api_key:  (cfg as any).api_key  ?? "",
      endpoint: (cfg as any).endpoint ?? "",
      model:    (cfg as any).model    ?? "",
    };

    isTranslating.set(true);
    try {
      interface TranslatedSegment { id: number; translation: string; }
      const results = await invoke<TranslatedSegment[]>("translate_transcript", {
        req: {
          segments: $transcript.segments.map(s => ({ id: s.id, text: s.text })),
          source_language: $transcript.language || null,
          target_language: $translationTargetLang,
          service:  cfg.service,
          api_key:  svcCfg.api_key  || null,
          endpoint: svcCfg.endpoint || null,
          model:    svcCfg.model    || null,
        },
      });

      const map = new Map(results.map(r => [r.id, r.translation]));

      // Update transcript store
      transcript.update(t => {
        if (!t) return t;
        return {
          ...t,
          segments: t.segments.map(s => ({ ...s, translation: map.get(s.id) ?? s.translation })),
        };
      });

      // Persist translation into sessions store
      const sessionId = get(activeSessionId);
      if (sessionId) {
        sessions.update(list =>
          list.map(sess => {
            if (sess.id !== sessionId) return sess;
            return {
              ...sess,
              transcript: {
                ...sess.transcript,
                segments: sess.transcript.segments.map(s => ({
                  ...s,
                  translation: map.get(s.id) ?? s.translation,
                })),
              },
            };
          })
        );
      }
    } catch (e) {
      errorMsg.set("翻譯失敗：" + String(e));
    } finally {
      isTranslating.set(false);
    }
  }

  // ── Export ────────────────────────────────────────────────────────────────
  type ContentMode = "original" | "translation" | "bilingual";

  interface ExportItem {
    format: string;
    mode: ContentMode;
    disabled?: boolean;
  }

  interface ExportGroup {
    label: string;
    disabled?: boolean;
    items: ExportItem[];
  }

  let exportMenuOpen = false;

  function buildExportGroups(): ExportGroup[] {
    const fmts = ["txt", "srt", "vtt"];
    return [
      {
        label: $t.transcript.original,
        items: fmts.map(fmt => ({ format: fmt, mode: "original" as ContentMode })),
      },
      {
        label: $t.transcript.translation,
        disabled: !hasTranslation,
        items: fmts.map(fmt => ({ format: fmt, mode: "translation" as ContentMode, disabled: !hasTranslation })),
      },
      {
        label: $t.transcript.bilingual,
        disabled: !hasTranslation,
        items: fmts.map(fmt => ({ format: fmt, mode: "bilingual" as ContentMode, disabled: !hasTranslation })),
      },
      {
        label: $t.transcript.jsonFull,
        items: [{ format: "json", mode: "original" as ContentMode }],
      },
    ];
  }

  async function exportAs(format: string, mode: ContentMode) {
    if (!$transcript) return;
    exportMenuOpen = false;
    const ext = format;
    const path = await save({
      defaultPath: `transcript.${ext}`,
      filters: [{ name: format.toUpperCase(), extensions: [ext] }],
    });
    if (!path) return;
    try {
      await invoke("export_transcript", {
        transcript: $transcript,
        format,
        outputPath: path,
        contentMode: mode,
      });
    } catch (e) {
      errorMsg.set(String(e));
    }
  }
</script>

<div class="transcript-panel">
  <div class="panel-header">
    <span class="panel-title">
      {$t.transcript.title}
      {#if $transcript}
        <span class="seg-count">({$transcript.segments.length} {$t.transcript.segments})</span>
      {/if}
    </span>

    {#if $transcript}
      <div class="header-actions">
        <!-- Translation target language selector -->
        <select
          class="lang-select"
          bind:value={$translationTargetLang}
          title={$t.transcript.targetLangTitle}
          disabled={$isTranslating}
        >
          {#each TARGET_LANGS as lang}
            <option value={lang.value}>{lang.label}</option>
          {/each}
        </select>

        <!-- Translate button -->
        <button
          class="translate-btn"
          class:loading={$isTranslating}
          onclick={translateTranscript}
          disabled={$isTranslating}
          title={$t.transcript.translateBtn}
        >
          {#if $isTranslating}
            <span class="spin-icon">⏳</span>
            {$t.transcript.translating}
          {:else}
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M5 8l6 6"/><path d="M4 14l6-6 2-3"/>
              <path d="M2 5h12"/><path d="M7 2h1"/>
              <path d="M22 22l-5-10-5 10"/><path d="M14 18h6"/>
            </svg>
            {$t.transcript.translateBtn}
          {/if}
        </button>

        <!-- Export dropdown -->
        <div class="export-wrapper">
          <button
            class="export-btn"
            onclick={() => exportMenuOpen = !exportMenuOpen}
          >
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
              <polyline points="7 10 12 15 17 10"/>
              <line x1="12" y1="15" x2="12" y2="3"/>
            </svg>
            {$t.transcript.exportBtn}
            <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </button>

          {#if exportMenuOpen}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <div class="backdrop" onclick={() => exportMenuOpen = false}></div>
            <div class="export-menu">
              {#each buildExportGroups() as group, gi}
                {#if gi > 0}
                  <div class="menu-divider"></div>
                {/if}
                <div class="menu-group-label" class:disabled={group.disabled}>
                  {group.label}
                </div>
                {#each group.items as item}
                  <button
                    class:disabled={item.disabled}
                    onclick={() => !item.disabled && exportAs(item.format, item.mode)}
                    title={item.disabled ? $t.transcript.needTranslateHint : ""}
                  >
                    {item.format.toUpperCase()}
                  </button>
                {/each}
              {/each}
            </div>
          {/if}
        </div>
      </div>
    {/if}
  </div>

  <div class="segment-list" bind:this={listEl}>
    {#if $isTranscribing}
      <div class="loading">
        <div class="loader-dots"><span></span><span></span><span></span></div>
        <div>{$t.transcript.loadingTranscribing}</div>
      </div>
    {:else if $isTranslating}
      <div class="loading">
        <div class="loader-dots"><span></span><span></span><span></span></div>
        <div>{$t.transcript.loadingTranslating}</div>
      </div>
    {:else if !$videoPath}
      <div class="empty-state">
        <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2" opacity="0.3">
          <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
          <polyline points="14 2 14 8 20 8"/>
          <line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/>
          <polyline points="10 9 9 9 8 9"/>
        </svg>
        <p class="pre-line">{$t.transcript.emptyNoVideo}</p>
      </div>
    {:else if !$transcript}
      <div class="empty-state"><p class="pre-line">{$t.transcript.emptyNoTranscript}</p></div>
    {:else if $transcript.segments.length === 0}
      <div class="empty-state"><p>{$t.transcript.emptySilence}</p></div>
    {:else}
      {#each $transcript.segments as segment, i (segment.id)}
        <div data-idx={i}>
          <TranscriptItem {segment} active={$activeSegmentIndex === i} />
        </div>
      {/each}
    {/if}
  </div>
</div>

<style>
  .transcript-panel {
    width: 100%;
    height: 100%;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: #212121;
    overflow: hidden;
  }

  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 12px;
    border-bottom: 1px solid #2f2f2f;
    flex-shrink: 0;
    gap: 8px;
    flex-wrap: wrap;
  }

  .panel-title {
    font-size: 13px;
    font-weight: 600;
    color: #ececec;
    flex-shrink: 0;
  }

  .seg-count { font-weight: 400; color: #8e8ea0; margin-left: 4px; }

  /* ── Header action row ───────────────────────────────────────────────── */
  .header-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  /* Language selector */
  .lang-select {
    padding: 5px 8px;
    border: 1px solid #3f3f3f;
    border-radius: 6px;
    background: #1e1e1e;
    color: #c5c5d2;
    font-size: 12px;
    cursor: pointer;
    outline: none;
    transition: border-color 0.15s;
  }
  .lang-select:hover { border-color: #5a5a72; }
  .lang-select:disabled { opacity: 0.5; cursor: not-allowed; }

  /* Translate button */
  .translate-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 5px 10px;
    border: 1px solid #2a5a40;
    border-radius: 6px;
    background: #1a3a2e;
    color: #4ecb94;
    font-size: 12px;
    cursor: pointer;
    transition: all 0.15s;
    white-space: nowrap;
  }
  .translate-btn:hover:not(:disabled) { background: #2a5a40; color: #fff; }
  .translate-btn:disabled { opacity: 0.6; cursor: not-allowed; }
  .translate-btn.loading { opacity: 0.7; }

  @keyframes pulse { 0%,100%{opacity:1} 50%{opacity:0.4} }
  .spin-icon { animation: pulse 1s ease-in-out infinite; }

  /* Export button & menu */
  .export-wrapper { position: relative; }

  .export-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 5px 10px;
    border: 1px solid #3f3f3f;
    border-radius: 6px;
    background: transparent;
    color: #c5c5d2;
    font-size: 12px;
    cursor: pointer;
    transition: background 0.15s;
    white-space: nowrap;
  }
  .export-btn:hover { background: #2f2f2f; }

  .backdrop { position: fixed; inset: 0; z-index: 9; }

  .export-menu {
    position: absolute;
    right: 0;
    top: calc(100% + 4px);
    background: #2a2a2a;
    border: 1px solid #4a4a4a;
    border-radius: 8px;
    overflow: hidden;
    z-index: 10;
    min-width: 130px;
    box-shadow: 0 4px 16px rgba(0,0,0,0.4);
    padding: 4px 0;
  }

  .menu-divider {
    height: 1px;
    background: #3f3f3f;
    margin: 4px 0;
  }

  .menu-group-label {
    padding: 5px 14px 2px;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    color: #6a6a80;
    user-select: none;
  }
  .menu-group-label.disabled { color: #44444e; }

  .export-menu button {
    display: block;
    width: 100%;
    padding: 5px 14px 5px 20px;
    border: none;
    background: transparent;
    color: #c5c5d2;
    font-size: 12px;
    cursor: pointer;
    text-align: left;
    transition: background 0.12s;
  }
  .export-menu button:hover { background: #3a3a3a; color: #ececec; }
  .export-menu button.disabled { color: #484858; cursor: not-allowed; }
  .export-menu button.disabled:hover { background: transparent; color: #484858; }

  /* ── Segment list ─────────────────────────────────────────────────────── */
  .segment-list { flex: 1; overflow-y: auto; padding: 8px 6px; }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 16px;
    color: #5a5a72;
    font-size: 13px;
    text-align: center;
    line-height: 1.7;
  }

  .empty-state .pre-line {
    white-space: pre-line;
  }

  .loading {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 16px;
    color: #8e8ea0;
    font-size: 13px;
  }

  .loader-dots { display: flex; gap: 6px; }
  .loader-dots span {
    width: 8px; height: 8px;
    border-radius: 50%;
    background: #10a37f;
    animation: bounce 1.2s ease-in-out infinite;
  }
  .loader-dots span:nth-child(2) { animation-delay: 0.2s; }
  .loader-dots span:nth-child(3) { animation-delay: 0.4s; }

  @keyframes bounce {
    0%, 80%, 100% { transform: scale(0.6); opacity: 0.4; }
    40% { transform: scale(1); opacity: 1; }
  }
</style>
