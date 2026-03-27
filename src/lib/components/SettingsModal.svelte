<script lang="ts">
  import { settings, settingsOpen } from "../stores/app";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import type { TranslationServiceType, ServiceConfig } from "../types";
  import { onMount, onDestroy } from "svelte";
  import { lang, t } from "../i18n";

  interface ModelInfo {
    name: string;
    filename: string;
    url: string;
    size_mb: number;
    description: string;
    downloaded: boolean;
    path: string | null;
  }

  interface DownloadProgressEvent {
    name: string;
    progress: number;
    downloaded: number;
    total: number;
  }

  // ── Per-service default configs ───────────────────────────────────────────
  const SERVICE_DEFAULTS: Record<TranslationServiceType, ServiceConfig> = {
    free_google:    { api_key: "", endpoint: "", model: "" },
    google:         { api_key: "", endpoint: "", model: "" },
    bing:           { api_key: "", endpoint: "", model: "" },
    libretranslate: { api_key: "", endpoint: "https://libretranslate.com", model: "" },
    openai:         { api_key: "", endpoint: "", model: "gpt-4o-mini" },
    gemini:         { api_key: "", endpoint: "", model: "gemini-1.5-flash" },
    grok:           { api_key: "", endpoint: "", model: "grok-2-latest" },
    claude:         { api_key: "", endpoint: "", model: "claude-3-haiku-20240307" },
    openrouter:     { api_key: "", endpoint: "https://openrouter.ai/api/v1", model: "openai/gpt-4o-mini" },
  };

  /** Build a complete configs map, merging saved per-service data with defaults.
   *  Also handles migration from the old flat format (api_key at top level). */
  function buildTranslation(saved: any) {
    let service: TranslationServiceType = saved?.service ?? "openai";
    let savedConfigs: Partial<Record<TranslationServiceType, ServiceConfig>> = saved?.configs ?? {};

    // ── Migration: old flat format had api_key/endpoint/model at the top level ──
    if (typeof (saved as any)?.api_key === "string") {
      savedConfigs = {
        ...savedConfigs,
        [service]: {
          api_key: (saved as any).api_key ?? "",
          endpoint: (saved as any).endpoint ?? "",
          model: (saved as any).model ?? "",
        },
      };
    }

    // Ensure every known service has an entry (merge defaults + saved)
    const configs = {} as Record<TranslationServiceType, ServiceConfig>;
    for (const svc of ALL_SERVICES) {
      configs[svc] = { ...SERVICE_DEFAULTS[svc], ...(savedConfigs[svc] ?? {}) };
    }

    return { service, configs };
  }

  const ALL_SERVICES = [
    "free_google", "google", "bing", "libretranslate",
    "openai", "gemini", "grok", "claude", "openrouter",
  ] as TranslationServiceType[];

  // ── Local working copy of settings ───────────────────────────────────────
  let local = $state({
    ...$settings,
    translation: buildTranslation($settings.translation),
  });

  // ── Settings nav tab ──────────────────────────────────────────────────────
  type SettingsTab = "general" | "engine" | "translation";
  let activeTab = $state<SettingsTab>("general");

  // ── Model manager state ───────────────────────────────────────────────────
  let models = $state<ModelInfo[]>([]);
  let modelsDir = $state("");
  let downloading = $state<Record<string, number>>({});
  let downloadErrors = $state<Record<string, string>>({});
  let unlistenProgress: (() => void) | null = null;

  async function loadModels() {
    try {
      [models, modelsDir] = await Promise.all([
        invoke<ModelInfo[]>("list_models", { lang: $lang }),
        invoke<string>("get_models_dir"),
      ]);
    } catch (e) {
      console.error("list_models failed", e);
    }
  }

  onMount(async () => {
    await loadModels();
    unlistenProgress = await listen<DownloadProgressEvent>(
      "model_download_progress",
      (event) => {
        const { name, progress } = event.payload;
        downloading = { ...downloading, [name]: progress };
        if (progress >= 100) {
          setTimeout(async () => {
            await loadModels();
            const { [name]: _, ...rest } = downloading;
            downloading = rest;
          }, 300);
        }
      }
    );
  });

  onDestroy(() => { unlistenProgress?.(); });

  // ── Modal actions ─────────────────────────────────────────────────────────
  function close() { settingsOpen.set(false); }

  function save() {
    // JSON round-trip strips Svelte 5 reactive proxies, ensuring a plain object is stored.
    settings.set(JSON.parse(JSON.stringify(local)));
    close();
  }

  // ── Engine tab helpers ────────────────────────────────────────────────────

  // Cloud API is temporarily disabled due to a known bug.
  // If the persisted setting is "api", silently fall back to "local".
  if (local.engine.type === "api") {
    local.engine = { type: "local", model_path: "", language: local.engine.language };
  }

  function setEngineType(type: "local" | "api") {
    if (type === "api") return; // cloud API temporarily disabled
    if (type === "local" && local.engine.type !== "local") {
      local.engine = { type: "local", model_path: "", language: local.engine.language };
    }
  }

  async function startDownload(model: ModelInfo) {
    downloading = { ...downloading, [model.name]: 0 };
    delete downloadErrors[model.name];
    downloadErrors = { ...downloadErrors };
    try {
      await invoke("download_model", { name: model.name, lang: $lang });
    } catch (e) {
      downloadErrors = { ...downloadErrors, [model.name]: String(e) };
      const { [model.name]: _, ...rest } = downloading;
      downloading = rest;
    }
  }

  function selectModel(model: ModelInfo) {
    if (model.path && local.engine.type === "local") {
      const newEngine = { ...local.engine, model_path: model.path, model_name: model.name };
      local.engine = newEngine;
      settings.update(s => ({ ...s, engine: { ...newEngine } }));
    }
  }

  async function removeModel(model: ModelInfo) {
    try {
      await invoke("delete_model", { name: model.name, lang: $lang });
      if (local.engine.type === "local" &&
          (local.engine.model_path === model.path || local.engine.model_name === model.name)) {
        local.engine = { ...local.engine, model_path: "", model_name: undefined };
        settings.update(s => ({ ...s, engine: { ...local.engine } }));
      }
      await loadModels();
    } catch (e) { console.error(e); }
  }

  function isSelected(model: ModelInfo) {
    if (local.engine.type !== "local") return false;
    if (local.engine.model_name && local.engine.model_name === model.name) return true;
    return model.path !== null && local.engine.model_path === model.path;
  }

  function formatSize(mb: number) {
    return mb >= 1000 ? (mb / 1000).toFixed(1) + " GB" : mb + " MB";
  }

  const RECOMMENDED = new Set(["small", "large-v3-turbo"]);

  // ── Translation tab helpers ───────────────────────────────────────────────
  interface ServiceMeta {
    label: string;
    fields: ("api_key" | "endpoint" | "model")[];
    placeholders: Record<string, string>;
    hints: Record<string, string>;
    defaultModels?: string[];
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") close();
  }

  // Reactive SERVICE_META – rebuilds whenever $t changes (lang switch)
  const SERVICE_META = $derived<Record<TranslationServiceType, ServiceMeta>>({
    free_google: {
      label: $t.settings.freeGoogleLabel,
      fields: [],
      placeholders: {},
      hints: {},
    },
    google: {
      label: $t.settings.svcGoogle,
      fields: ["api_key"],
      placeholders: { api_key: "AIza..." },
      hints: { api_key: $t.settings.hintGoogleApiKey },
    },
    bing: {
      label: $t.settings.svcBing,
      fields: ["api_key"],
      placeholders: { api_key: $t.settings.placeholderBingApiKey },
      hints: { api_key: $t.settings.hintBingApiKey },
    },
    libretranslate: {
      label: "LibreTranslate",
      fields: ["endpoint", "api_key"],
      placeholders: { endpoint: "https://libretranslate.com", api_key: $t.settings.placeholderLibreApiKey },
      hints: { endpoint: $t.settings.hintLibreEndpoint, api_key: $t.settings.hintLibreApiKey },
    },
    openai: {
      label: "OpenAI",
      fields: ["api_key", "model"],
      placeholders: { api_key: "sk-...", model: "gpt-4o-mini" },
      hints: { api_key: $t.settings.hintOpenAiApiKey, model: $t.settings.hintOpenAiModel },
      defaultModels: ["gpt-4o-mini", "gpt-4o", "gpt-4-turbo"],
    },
    gemini: {
      label: "Gemini",
      fields: ["api_key", "model"],
      placeholders: { api_key: "AI Studio API Key", model: "gemini-1.5-flash" },
      hints: { api_key: $t.settings.hintGeminiApiKey, model: $t.settings.hintGeminiModel },
      defaultModels: ["gemini-1.5-flash", "gemini-1.5-pro", "gemini-2.0-flash"],
    },
    grok: {
      label: "Grok (xAI)",
      fields: ["api_key", "model"],
      placeholders: { api_key: "xai-...", model: "grok-2-latest" },
      hints: { api_key: $t.settings.hintGrokApiKey, model: "" },
      defaultModels: ["grok-2-latest", "grok-2-mini"],
    },
    claude: {
      label: "Claude (Anthropic)",
      fields: ["api_key", "model"],
      placeholders: { api_key: "sk-ant-...", model: "claude-3-haiku-20240307" },
      hints: { api_key: $t.settings.hintClaudeApiKey, model: $t.settings.hintClaudeModel },
      defaultModels: ["claude-3-haiku-20240307", "claude-3-5-haiku-20241022", "claude-3-5-sonnet-20241022"],
    },
    openrouter: {
      label: "OpenRouter",
      fields: ["api_key", "model", "endpoint"],
      placeholders: { api_key: "sk-or-...", model: "openai/gpt-4o-mini", endpoint: "https://openrouter.ai/api/v1" },
      hints: { api_key: $t.settings.hintOpenRouterApiKey, model: $t.settings.hintOpenRouterModel, endpoint: $t.settings.hintOpenRouterEndpoint },
      defaultModels: ["openai/gpt-4o-mini", "anthropic/claude-3-haiku", "google/gemini-1.5-flash"],
    },
  });

  // Reactive current translation service metadata (fallback to openai if unknown)
  const currentMeta = $derived(
    SERVICE_META[local.translation?.service as TranslationServiceType] ?? SERVICE_META["openai"]
  );

  // Shorthand: the active service's config object
  const activeSvc = $derived(local.translation.service);
  const activeConfig = $derived(local.translation.configs[activeSvc]!);
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="overlay" onclick={close} onkeydown={handleKeydown} role="dialog" aria-modal="true" tabindex="-1">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal" onclick={(e) => e.stopPropagation()}>

    <!-- ── Header ─────────────────────────────────────────────────────────── -->
    <div class="modal-header">
      <h2>{$t.settings.title}</h2>
      <button class="close-btn" onclick={close} aria-label={$t.settings.closeAriaLabel}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
        </svg>
      </button>
    </div>

    <!-- ── Body: sidebar + panel ─────────────────────────────────────────── -->
    <div class="modal-body">

      <!-- Left nav -->
      <nav class="settings-nav">
        <button
          class="nav-item"
          class:active={activeTab === "general"}
          onclick={() => activeTab = "general"}
        >
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
            <circle cx="12" cy="12" r="4"/>
            <path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M4.93 19.07l1.41-1.41M17.66 6.34l1.41-1.41"/>
          </svg>
          {$t.settings.navGeneral}
        </button>
        <button
          class="nav-item"
          class:active={activeTab === "engine"}
          onclick={() => activeTab = "engine"}
        >
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
            <circle cx="12" cy="12" r="3"/>
            <path d="M19.07 4.93a10 10 0 0 1 0 14.14M4.93 4.93a10 10 0 0 0 0 14.14"/>
            <path d="M12 2v2M12 20v2M2 12h2M20 12h2"/>
          </svg>
          {$t.settings.navEngine}
        </button>
        <button
          class="nav-item"
          class:active={activeTab === "translation"}
          onclick={() => activeTab = "translation"}
        >
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
            <path d="M5 8l6 6"/>
            <path d="M4 14l6-6 2-3"/>
            <path d="M2 5h12"/>
            <path d="M7 2h1"/>
            <path d="M22 22l-5-10-5 10"/>
            <path d="M14 18h6"/>
          </svg>
          {$t.settings.navTranslation}
        </button>
      </nav>

      <!-- Right panel -->
      <div class="panel-content">

        <!-- ── 一般 panel ──────────────────────────────────────────────── -->
        {#if activeTab === "general"}
          <div class="panel-section">
            <div class="field-group">
              <!-- svelte-ignore a11y_label_has_associated_control -->
              <label class="field-label">{$t.settings.interfaceLang}</label>
              <div class="lang-option-group">
                <button
                  class="lang-option-btn"
                  class:active={$lang === "zh-TW"}
                  onclick={() => lang.set("zh-TW")}
                >{$t.settings.langZhTW}</button>
                <button
                  class="lang-option-btn"
                  class:active={$lang === "zh-CN"}
                  onclick={() => lang.set("zh-CN")}
                >{$t.settings.langZhCN}</button>
                <button
                  class="lang-option-btn"
                  class:active={$lang === "en"}
                  onclick={() => lang.set("en")}
                >{$t.settings.langEn}</button>
                <button
                  class="lang-option-btn"
                  class:active={$lang === "ko"}
                  onclick={() => lang.set("ko")}
                >{$t.settings.langKo}</button>
                <button
                  class="lang-option-btn"
                  class:active={$lang === "vi"}
                  onclick={() => lang.set("vi")}
                >{$t.settings.langVi}</button>
                <button
                  class="lang-option-btn"
                  class:active={$lang === "ja"}
                  onclick={() => lang.set("ja")}
                >{$t.settings.langJa}</button>
              </div>
            </div>
          </div>

        <!-- ── 轉錄引擎 panel ──────────────────────────────────────────── -->
        {:else if activeTab === "engine"}
          <div class="panel-section">
            <!-- Engine type toggle -->
            <div class="field-group">
              <!-- svelte-ignore a11y_label_has_associated_control -->
              <label class="field-label">{$t.settings.engineType}</label>
              <div class="toggle-group">
                <button
                  class="toggle-btn api-disabled"
                  disabled
                  title={$t.settings.cloudApiDisabledHint}
                >
                  {$t.settings.cloudApi}
                  <span class="maintenance-badge">{$t.settings.maintenance}</span>
                </button>
                <button
                  class="toggle-btn"
                  class:active={local.engine.type === "local"}
                  onclick={() => setEngineType("local")}
                >{$t.settings.localWhisper}</button>
              </div>
            </div>

            {#if local.engine.type === "api"}
              <div class="field-group">
                <label class="field-label" for="base-url">{$t.settings.apiBaseUrl}</label>
                <input id="base-url" class="field-input" type="text"
                  placeholder="https://api.openai.com"
                  bind:value={local.engine.base_url} />
                <div class="field-hint">{$t.settings.apiBaseUrlHint}</div>
              </div>
              <div class="field-group">
                <label class="field-label" for="api-key">{$t.settings.apiKey}</label>
                <input id="api-key" class="field-input" type="password"
                  placeholder="sk-..." bind:value={local.engine.api_key} />
              </div>
              <div class="field-group">
                <label class="field-label" for="model">{$t.settings.model}</label>
                <input id="model" class="field-input" type="text"
                  placeholder="whisper-1" bind:value={local.engine.model} />
                <div class="field-hint">{$t.settings.modelHint}</div>
              </div>

            {:else}
              <!-- Local Whisper Model Manager -->
              <div class="field-group">
                <!-- svelte-ignore a11y_label_has_associated_control -->
                <label class="field-label">{$t.settings.whisperModel}</label>
                {#if modelsDir}
                  <div class="models-dir-hint">{$t.settings.modelsStoredAt}：{modelsDir}</div>
                {/if}

                <div class="model-list">
                  {#each models as model (model.name)}
                    {@const isDownloading = model.name in downloading}
                    {@const pct = downloading[model.name] ?? 0}
                    {@const selected = isSelected(model)}
                    {@const err = downloadErrors[model.name]}

                    <div class="model-card" class:selected>
                      <div class="model-left">
                        <div class="model-title-row">
                          <span class="model-name">{model.name}</span>
                          {#if RECOMMENDED.has(model.name)}<span class="badge recommend">{$t.settings.recommended}</span>{/if}
                          {#if selected}
                            <span class="badge in-use">{$t.settings.using}</span>
                          {:else if model.downloaded}
                            <span class="badge ready">{$t.settings.downloaded}</span>
                          {/if}
                        </div>
                        <div class="model-desc">{$t.modelDesc[model.name] ?? model.description}　{formatSize(model.size_mb)}</div>
                        {#if isDownloading}
                          <div class="progress-wrap">
                            <div class="progress-bar"><div class="progress-fill" style="width:{pct.toFixed(1)}%"></div></div>
                            <span class="progress-pct">{pct.toFixed(0)}%</span>
                          </div>
                        {/if}
                        {#if err}<div class="model-error">{err}</div>{/if}
                      </div>
                      <div class="model-actions">
                        {#if isDownloading}
                          <!-- downloading -->
                        {:else if model.downloaded}
                          {#if !selected}
                            <button class="action-btn use-btn" onclick={() => selectModel(model)}>{$t.settings.use}</button>
                          {/if}
                          <button class="action-btn del-btn" onclick={() => removeModel(model)} title={$t.settings.delete}>
                            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                              <polyline points="3 6 5 6 21 6"/>
                              <path d="M19 6l-1 14H6L5 6"/>
                              <path d="M10 11v6M14 11v6M9 6V4h6v2"/>
                            </svg>
                          </button>
                        {:else}
                          <button class="action-btn dl-btn" onclick={() => startDownload(model)}>
                            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="margin-right:4px">
                              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
                              <polyline points="7 10 12 15 17 10"/>
                              <line x1="12" y1="15" x2="12" y2="3"/>
                            </svg>
                            {$t.settings.download}
                          </button>
                        {/if}
                      </div>
                    </div>
                  {/each}
                </div>
                <div class="field-hint warn">{$t.settings.ffmpegHint}</div>
              </div>
            {/if}

            <!-- Transcription language selector (shared for both engine types) -->
            <div class="field-group">
              <label class="field-label" for="language">{$t.settings.language}</label>
              <select
                id="language"
                class="field-input lang-select"
                value={local.engine.language ?? ""}
                onchange={(e) => {
                  const v = (e.target as HTMLSelectElement).value;
                  local.engine = { ...local.engine, language: v || null };
                }}
              >
                <option value="">🌐 {$t.settings.autoDetect}</option>
                <optgroup label={$t.settings.langGroupChinese}>
                  <option value="zh-TW">{$t.settings.langZhTWFull}</option>
                  <option value="zh-CN">{$t.settings.langZhCNFull}</option>
                  <option value="yue">{$t.video.langYue}　yue</option>
                </optgroup>
                <optgroup label={$t.settings.langGroupEastAsian}>
                  <option value="ja">{$t.video.langJa}　ja</option>
                  <option value="ko">{$t.video.langKo}　ko</option>
                </optgroup>
                <optgroup label={$t.settings.langGroupWestern}>
                  <option value="en">{$t.video.langEn}　en</option>
                  <option value="fr">{$t.video.langFr}　fr</option>
                  <option value="de">{$t.video.langDe}　de</option>
                  <option value="es">{$t.video.langEs}　es</option>
                  <option value="pt">{$t.video.langPt}　pt</option>
                  <option value="it">{$t.video.langIt}　it</option>
                  <option value="ru">{$t.video.langRu}　ru</option>
                  <option value="nl">{$t.video.langNl}　nl</option>
                  <option value="pl">{$t.video.langPl}　pl</option>
                  <option value="sv">{$t.video.langSv}　sv</option>
                </optgroup>
                <optgroup label={$t.settings.langGroupOther}>
                  <option value="ar">{$t.video.langAr}　ar</option>
                  <option value="hi">{$t.video.langHi}　hi</option>
                  <option value="th">{$t.video.langTh}　th</option>
                  <option value="vi">{$t.video.langVi}　vi</option>
                  <option value="id">{$t.video.langId}　id</option>
                  <option value="tr">{$t.video.langTr}　tr</option>
                  <option value="uk">{$t.video.langUk}　uk</option>
                </optgroup>
              </select>
            </div>
          </div>

        <!-- ── 翻譯服務 panel ──────────────────────────────────────────── -->
        {:else}
          <div class="panel-section">
            <!-- Service selector -->
            <div class="field-group">
              <label class="field-label" for="trans-service">{$t.settings.translationService}</label>
              <select
                id="trans-service"
                class="field-input"
                value={local.translation.service}
                onchange={(e) => {
                  const v = (e.target as HTMLSelectElement).value as TranslationServiceType;
                  local.translation = { ...local.translation, service: v };
                }}
              >
                {#each ALL_SERVICES as svc}
                  <option value={svc}>{SERVICE_META[svc].label}</option>
                {/each}
              </select>
            </div>

            <!-- Free Google: no config needed, just show a notice -->
            {#if local.translation.service === "free_google"}
              <div class="free-google-notice">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="flex-shrink:0;color:#10a37f">
                  <circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/>
                  <line x1="12" y1="16" x2="12.01" y2="16"/>
                </svg>
                <span>{$t.settings.freeGoogleNotice}</span>
              </div>
            {/if}

            <!-- Conditional config fields based on selected service -->
            {#if currentMeta.fields.includes("api_key")}
              <div class="field-group">
                <label class="field-label" for="trans-apikey">{$t.settings.apiKey}</label>
                <input
                  id="trans-apikey"
                  class="field-input"
                  type="password"
                  placeholder={currentMeta.placeholders.api_key ?? ""}
                  bind:value={activeConfig.api_key}
                />
                {#if currentMeta.hints.api_key}
                  <div class="field-hint">{currentMeta.hints.api_key}</div>
                {/if}
              </div>
            {/if}

            {#if currentMeta.fields.includes("endpoint")}
              <div class="field-group">
                <label class="field-label" for="trans-endpoint">Endpoint URL</label>
                <input
                  id="trans-endpoint"
                  class="field-input"
                  type="text"
                  placeholder={currentMeta.placeholders.endpoint ?? ""}
                  bind:value={activeConfig.endpoint}
                />
                {#if currentMeta.hints.endpoint}
                  <div class="field-hint">{currentMeta.hints.endpoint}</div>
                {/if}
              </div>
            {/if}

            {#if currentMeta.fields.includes("model")}
              <div class="field-group">
                <label class="field-label" for="trans-model">{$t.settings.model}</label>
                {#if currentMeta.defaultModels?.length}
                  <div class="model-preset-row">
                    <input
                      id="trans-model"
                      class="field-input"
                      type="text"
                      placeholder={currentMeta.placeholders.model ?? ""}
                      bind:value={activeConfig.model}
                    />
                    <div class="model-presets">
                      {#each currentMeta.defaultModels as preset}
                        <button
                          class="preset-chip"
                          class:active={activeConfig.model === preset}
                          onclick={() => { activeConfig.model = preset; }}
                        >{preset}</button>
                      {/each}
                    </div>
                  </div>
                {:else}
                  <input
                    id="trans-model"
                    class="field-input"
                    type="text"
                    placeholder={currentMeta.placeholders.model ?? ""}
                    bind:value={activeConfig.model}
                  />
                {/if}
                {#if currentMeta.hints.model}
                  <div class="field-hint">{currentMeta.hints.model}</div>
                {/if}
              </div>
            {/if}
          </div>
        {/if}

      </div><!-- end panel-content -->
    </div><!-- end modal-body -->

    <!-- ── Footer ─────────────────────────────────────────────────────────── -->
    <div class="modal-footer">
      <button class="cancel-btn" onclick={close}>{$t.settings.cancel}</button>
      <button class="save-btn" onclick={save}>{$t.settings.save}</button>
    </div>

  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0,0,0,0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .modal {
    background: #2f2f2f;
    border: 1px solid #4a4a4a;
    border-radius: 12px;
    width: 740px;
    max-width: 96vw;
    max-height: 90vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 8px 32px rgba(0,0,0,0.5);
  }

  /* ── Header ──────────────────────────────────────────────────────────── */
  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px 14px;
    border-bottom: 1px solid #3f3f3f;
    flex-shrink: 0;
  }

  .modal-header h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    color: #ececec;
  }

  .close-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: #8e8ea0;
    cursor: pointer;
    transition: background 0.12s, color 0.12s;
  }
  .close-btn:hover { background: #3f3f3f; color: #ececec; }

  /* ── Body layout: sidebar + panel ───────────────────────────────────── */
  .modal-body {
    display: flex;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  /* ── Settings nav sidebar ────────────────────────────────────────────── */
  .settings-nav {
    width: 170px;
    flex-shrink: 0;
    border-right: 1px solid #3f3f3f;
    padding: 12px 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    background: #282828;
    border-bottom-left-radius: 0;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 8px 10px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: #8e8ea0;
    font-size: 13px;
    cursor: pointer;
    text-align: left;
    transition: background 0.12s, color 0.12s;
  }
  .nav-item:hover { background: #333; color: #c5c5d2; }
  .nav-item.active { background: #1a3a2e; color: #10a37f; font-weight: 600; }

  /* ── Panel content ───────────────────────────────────────────────────── */
  .panel-content {
    flex: 1;
    overflow-y: auto;
    min-width: 0;
  }

  .panel-section {
    padding: 18px 20px;
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  /* ── Field groups ────────────────────────────────────────────────────── */
  .field-group {
    margin-bottom: 18px;
  }

  .field-label {
    display: block;
    font-size: 12px;
    font-weight: 600;
    color: #8e8ea0;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin-bottom: 6px;
  }

  .field-input {
    width: 100%;
    padding: 8px 10px;
    background: #1e1e1e;
    border: 1px solid #4a4a4a;
    border-radius: 6px;
    color: #ececec;
    font-size: 13px;
    outline: none;
    transition: border-color 0.15s;
    box-sizing: border-box;
  }
  .field-input:focus { border-color: #10a37f; }

  .field-hint {
    margin-top: 5px;
    font-size: 11px;
    color: #5a5a72;
    line-height: 1.5;
  }
  .field-hint.warn { color: #c0803a; }

  /* ── Engine type toggle ──────────────────────────────────────────────── */
  .toggle-group {
    display: flex;
    gap: 6px;
  }

  .toggle-btn {
    padding: 6px 14px;
    border: 1px solid #4a4a4a;
    border-radius: 6px;
    background: transparent;
    color: #8e8ea0;
    font-size: 13px;
    cursor: pointer;
    transition: all 0.15s;
  }
  .toggle-btn:hover { background: #3f3f3f; color: #c5c5d2; }
  .toggle-btn.active { background: #1a3a2e; border-color: #10a37f; color: #10a37f; font-weight: 600; }

  .toggle-btn.api-disabled,
  .toggle-btn[disabled] {
    opacity: 0.45;
    cursor: not-allowed;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .toggle-btn[disabled]:hover { background: transparent; color: #8e8ea0; }

  .maintenance-badge {
    display: inline-block;
    padding: 1px 6px;
    font-size: 10px;
    border-radius: 4px;
    background: #5a3a10;
    color: #f5a623;
    font-weight: 600;
    letter-spacing: 0.3px;
  }

  /* ── Free Google notice ─────────────────────────────────────────────── */
  .free-google-notice {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 10px 12px;
    background: #1a2a1e;
    border: 1px solid #2a4a30;
    border-radius: 8px;
    color: #9ecfaa;
    font-size: 12px;
    line-height: 1.5;
    margin-bottom: 4px;
  }

  /* ── Model list ──────────────────────────────────────────────────────── */
  .models-dir-hint {
    font-size: 11px;
    color: #5a5a72;
    margin-bottom: 8px;
    word-break: break-all;
  }

  .model-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-bottom: 10px;
  }

  .model-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 12px;
    background: #1e1e1e;
    border: 1px solid #3f3f3f;
    border-radius: 8px;
    transition: border-color 0.15s;
  }
  .model-card.selected { border-color: #10a37f; background: #1a2e24; }

  .model-left { flex: 1; min-width: 0; }

  .model-title-row {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 2px;
  }

  .model-name { font-size: 13px; font-weight: 600; color: #ececec; }

  .badge {
    font-size: 10px;
    padding: 2px 6px;
    border-radius: 4px;
    font-weight: 600;
  }
  .badge.recommend { background: #1a2e44; color: #4a9eff; }
  .badge.in-use    { background: #1a3a2e; color: #10a37f; }
  .badge.ready     { background: #2a2a2a; color: #8e8ea0; }

  .model-desc { font-size: 11px; color: #5a5a72; }

  .model-actions { display: flex; gap: 6px; flex-shrink: 0; }

  .action-btn {
    padding: 5px 12px;
    font-size: 12px;
    border: 1px solid transparent;
    border-radius: 5px;
    cursor: pointer;
    display: flex;
    align-items: center;
    transition: all 0.12s;
  }

  .dl-btn  { background: #1a3a2a; border-color: #2a5a40; color: #4ecb94; }
  .dl-btn:hover  { background: #2a5a40; color: #fff; }
  .use-btn { background: #10a37f; border-color: #10a37f; color: #fff; }
  .use-btn:hover { background: #0d8a6a; }
  .del-btn { background: transparent; border-color: #5a3a3a; color: #8e8ea0; padding: 5px 8px; }
  .del-btn:hover { background: #5a2020; border-color: #e57373; color: #e57373; }

  .progress-wrap {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 6px;
  }
  .progress-bar {
    flex: 1;
    height: 4px;
    background: #3f3f3f;
    border-radius: 2px;
    overflow: hidden;
  }
  .progress-fill {
    height: 100%;
    background: #10a37f;
    border-radius: 2px;
    transition: width 0.3s;
  }
  .progress-pct { font-size: 11px; color: #8e8ea0; min-width: 32px; text-align: right; }
  .model-error  { font-size: 11px; color: #e57373; margin-top: 4px; }

  /* ── Language select ─────────────────────────────────────────────────── */
  .lang-select { background: #1e1e1e; color: #ececec; }

  /* ── General tab – language option buttons ──────────────────────────── */
  .lang-option-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 4px;
  }

  .lang-option-btn {
    display: flex;
    align-items: center;
    padding: 10px 14px;
    border: 1px solid #3f3f3f;
    border-radius: 8px;
    background: transparent;
    color: #c5c5d2;
    font-size: 13px;
    cursor: pointer;
    text-align: left;
    transition: all 0.12s;
  }

  .lang-option-btn:hover {
    background: #3a3a3a;
    border-color: #5a5a5a;
  }

  .lang-option-btn.active {
    background: #1a3a2e;
    border-color: #10a37f;
    color: #10a37f;
    font-weight: 600;
  }

  /* ── Translation service – model preset chips ───────────────────────── */
  .model-preset-row { display: flex; flex-direction: column; gap: 6px; }

  .model-presets { display: flex; flex-wrap: wrap; gap: 4px; }

  .preset-chip {
    padding: 3px 9px;
    font-size: 11px;
    border: 1px solid #4a4a4a;
    border-radius: 4px;
    background: transparent;
    color: #8e8ea0;
    cursor: pointer;
    transition: all 0.12s;
    font-family: "SF Mono", "Fira Code", monospace;
  }
  .preset-chip:hover  { background: #3f3f3f; color: #c5c5d2; }
  .preset-chip.active { background: #1a3a2e; border-color: #10a37f; color: #10a37f; }

  /* ── Footer ──────────────────────────────────────────────────────────── */
  .modal-footer {
    display: flex;
    gap: 10px;
    justify-content: flex-end;
    padding: 14px 20px;
    border-top: 1px solid #3f3f3f;
    flex-shrink: 0;
  }

  .cancel-btn {
    padding: 8px 16px;
    border: 1px solid #4a4a4a;
    border-radius: 6px;
    background: transparent;
    color: #c5c5d2;
    font-size: 13px;
    cursor: pointer;
    transition: background 0.15s;
  }
  .cancel-btn:hover { background: #3f3f3f; }

  .save-btn {
    padding: 8px 20px;
    border: none;
    border-radius: 6px;
    background: #10a37f;
    color: #fff;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s;
  }
  .save-btn:hover { background: #0d8a6a; }
</style>
