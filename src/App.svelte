<script lang="ts">
  import "./app.css";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import VideoPlayer from "./lib/components/VideoPlayer.svelte";
  import TranscriptPanel from "./lib/components/TranscriptPanel.svelte";
  import SettingsModal from "./lib/components/SettingsModal.svelte";
  import { sessions, settings, videoPath, settingsOpen, translationTargetLang } from "./lib/stores/app";
  import { lang } from "./lib/i18n";
  import { onMount } from "svelte";
  import { Store } from "@tauri-apps/plugin-store";

  let store: Store;

  // ── Resizable panel state ──────────────────────────────────────────────────
  let sidebarWidth = $state(220);   // px
  let videoPercent = $state(55);    // % of main-content width

  type DragTarget = "sidebar" | "video" | null;
  let dragType: DragTarget = $state(null);
  let dragStartX = 0;
  let dragStartVal = 0;
  let mainContentEl: HTMLElement;

  function startDrag(type: "sidebar" | "video", e: MouseEvent) {
    dragType = type;
    dragStartX = e.clientX;
    dragStartVal = type === "sidebar" ? sidebarWidth : videoPercent;
    e.preventDefault();
  }

  function onMouseMove(e: MouseEvent) {
    if (!dragType) return;
    const dx = e.clientX - dragStartX;
    if (dragType === "sidebar") {
      sidebarWidth = Math.max(160, Math.min(400, dragStartVal + dx));
    } else {
      const mainW = mainContentEl?.offsetWidth ?? window.innerWidth - sidebarWidth;
      const dPct = (dx / mainW) * 100;
      videoPercent = Math.max(25, Math.min(75, dragStartVal + dPct));
    }
  }

  function stopDrag() { dragType = null; }

  // ── Persistence ────────────────────────────────────────────────────────────
  onMount(async () => {
    store = await Store.load("app-store.json");

    const savedSessions = await store.get<typeof $sessions>("sessions");
    if (savedSessions) sessions.set(savedSessions);

    const savedSettings = await store.get<typeof $settings>("settings");
    if (savedSettings) settings.set(savedSettings);

    const savedTargetLang = await store.get<string>("translationTargetLang");
    if (savedTargetLang) translationTargetLang.set(savedTargetLang);

    const savedLang = await store.get<string>("appLang");
    if (savedLang) lang.set(savedLang as import("./lib/i18n").AppLang);

    sessions.subscribe(async (v) => { await store.set("sessions", v); await store.save(); });
    settings.subscribe(async (v) => { await store.set("settings", v); await store.save(); });
    translationTargetLang.subscribe(async (v) => { await store.set("translationTargetLang", v); await store.save(); });
    lang.subscribe(async (v) => { await store.set("appLang", v); await store.save(); });
  });
</script>

<!-- Global mouse tracking for resize drag -->
<svelte:window onmousemove={onMouseMove} onmouseup={stopDrag} />

<div class="app-layout" class:is-dragging={!!dragType}>

  <!-- ── Sidebar: always visible ─────────────────────────────────────────── -->
  <div class="sidebar-wrapper" style="width:{sidebarWidth}px">
    <Sidebar />
  </div>

  <!-- Sidebar ↔ Video resize handle -->
  <div
    class="resize-handle"
    role="separator"
    aria-orientation="vertical"
    onmousedown={(e) => startDrag("sidebar", e)}
  ></div>

  <!-- ── Main content ─────────────────────────────────────────────────────── -->
  <!-- VideoPlayer is always mounted to keep Tauri drag-drop listeners alive -->
  <main class="main-content" bind:this={mainContentEl}>

    <div
      class="video-wrapper"
      style={$videoPath ? `flex: 0 0 ${videoPercent}%` : "flex: 1"}
    >
      <VideoPlayer />
    </div>

    {#if $videoPath}
      <!-- Video ↔ Transcript resize handle -->
      <div
        class="resize-handle"
        role="separator"
        aria-orientation="vertical"
        onmousedown={(e) => startDrag("video", e)}
      ></div>

      <div class="transcript-wrapper">
        <TranscriptPanel />
      </div>
    {/if}
  </main>

</div>

{#if $settingsOpen}
  <SettingsModal />
{/if}

<style>
  .app-layout {
    display: flex;
    height: 100vh;
    width: 100vw;
    overflow: hidden;
    position: relative;
  }

  /* Prevent text selection while dragging */
  .app-layout.is-dragging {
    user-select: none;
    cursor: col-resize;
  }

  /* ── Sidebar wrapper ── */
  .sidebar-wrapper {
    flex-shrink: 0;
    height: 100%;
    overflow: hidden;
  }

  /* ── Resize handle ── */
  .resize-handle {
    flex-shrink: 0;
    width: 5px;
    height: 100%;
    background: transparent;
    cursor: col-resize;
    position: relative;
    z-index: 10;
    transition: background 0.15s;
  }

  .resize-handle::after {
    content: "";
    position: absolute;
    top: 0;
    left: 2px;
    width: 1px;
    height: 100%;
    background: #2f2f2f;
    transition: background 0.15s;
  }

  .resize-handle:hover::after,
  .app-layout.is-dragging .resize-handle::after {
    background: #10a37f;
  }

  /* ── Main content ── */
  .main-content {
    flex: 1;
    display: flex;
    min-width: 0;
    overflow: hidden;
  }

  .video-wrapper {
    min-width: 0;
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .transcript-wrapper {
    flex: 1;
    min-width: 0;
    height: 100%;
    overflow: hidden;
  }

</style>
