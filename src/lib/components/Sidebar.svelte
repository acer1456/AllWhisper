<script lang="ts">
  import { sessions, activeSessionId, videoPath, transcript, errorMsg, settingsOpen } from "../stores/app";
  import type { Session } from "../types";
  import { getVersion } from "@tauri-apps/api/app";
  import { onMount } from "svelte";
  import { t, lang } from "../i18n";

  let appVersion = $state("…");

  onMount(async () => {
    try { appVersion = await getVersion(); } catch { appVersion = "0.1.0"; }
  });

  async function newSession() {
    activeSessionId.set(null);
    videoPath.set(null);
    transcript.set(null);
    errorMsg.set(null);
  }

  async function loadSession(session: Session) {
    activeSessionId.set(session.id);
    videoPath.set(session.video_path);
    transcript.set(session.transcript);
    errorMsg.set(null);
  }

  async function deleteSession(id: string, e: MouseEvent) {
    e.stopPropagation();
    sessions.update(s => s.filter(x => x.id !== id));
    if ($activeSessionId === id) {
      activeSessionId.set(null);
      videoPath.set(null);
      transcript.set(null);
    }
  }

  function formatDate(ts: number) {
    const locale = $lang === "en" ? "en-US" : $lang;
    return new Date(ts).toLocaleDateString(locale, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
  }
</script>

<aside class="sidebar">
  <div class="sidebar-header">
    <span class="logo">
      <img src="/app-icon.png" alt="AllWhisper" class="logo-icon" />
      AllWhisper
    </span>
    <button class="new-btn" onclick={newSession} title={$t.sidebar.newTitle}>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
      </svg>
    </button>
  </div>

  <div class="session-list">
    {#if $sessions.length === 0}
      <div class="empty-hint">{$t.sidebar.emptyHint}</div>
    {/if}
    {#each [...$sessions].reverse() as session (session.id)}
      <!-- svelte-ignore a11y_interactive_supports_focus -->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div
        class="session-item"
        class:active={$activeSessionId === session.id}
        role="button"
        onclick={() => loadSession(session)}
      >
        <div class="session-icon">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polygon points="23 7 16 12 23 17 23 7"/><rect x="1" y="5" width="15" height="14" rx="2" ry="2"/>
          </svg>
        </div>
        <div class="session-info">
          <div class="session-title">{session.title}</div>
          <div class="session-date">{formatDate(session.created_at)}</div>
        </div>
        <button class="delete-btn" onclick={(e) => deleteSession(session.id, e)} title={$t.sidebar.deleteTitle}>
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
          </svg>
        </button>
      </div>
    {/each}
  </div>

  <!-- Bottom bar: version + settings -->
  <div class="sidebar-bottom">
    <span class="version-tag">v{appVersion}</span>
    <button
      class="settings-btn"
      onclick={() => settingsOpen.set(true)}
      title={$t.sidebar.settings}
    >
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="12" cy="12" r="3"/>
        <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
      </svg>
      {$t.sidebar.settings}
    </button>
  </div>
</aside>

<style>
  .sidebar {
    width: 100%;
    height: 100%;
    background: #171717;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .sidebar-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 14px;
    border-bottom: 1px solid #2f2f2f;
  }

  .logo {
    display: flex;
    align-items: center;
    gap: 7px;
    font-weight: 600;
    font-size: 15px;
    color: #ececec;
    letter-spacing: -0.3px;
  }

  .logo-icon {
    width: 20px;
    height: 20px;
    border-radius: 5px;
    flex-shrink: 0;
  }

  .new-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: #8e8ea0;
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
  }

  .new-btn:hover {
    background: #2f2f2f;
    color: #ececec;
  }

  .session-list {
    flex: 1;
    overflow-y: auto;
    padding: 8px 6px;
  }

  .empty-hint {
    color: #5a5a72;
    font-size: 12px;
    text-align: center;
    padding: 32px 16px;
    line-height: 1.6;
    white-space: pre-line;
  }

  .session-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 8px 10px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: #c5c5d2;
    cursor: pointer;
    text-align: left;
    transition: background 0.15s;
    position: relative;
  }

  .session-item:hover {
    background: #2f2f2f;
  }

  .session-item.active {
    background: #343541;
    color: #ececec;
  }

  .session-icon {
    flex-shrink: 0;
    opacity: 0.6;
  }

  .session-info {
    flex: 1;
    min-width: 0;
  }

  .session-title {
    font-size: 13px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .session-date {
    font-size: 11px;
    color: #5a5a72;
    margin-top: 2px;
  }

  .delete-btn {
    flex-shrink: 0;
    display: none;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border: none;
    background: transparent;
    color: #8e8ea0;
    cursor: pointer;
    border-radius: 4px;
    padding: 0;
  }

  .session-item:hover .delete-btn {
    display: flex;
  }

  .delete-btn:hover {
    background: #4a4a5a;
    color: #ececec;
  }

  /* ── Bottom bar ── */
  .sidebar-bottom {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 10px 0 14px;
    height: 44px;
    border-top: 1px solid #2f2f2f;
    background: #141414;
  }

  .version-tag {
    font-size: 11px;
    color: #454560;
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.2px;
    user-select: none;
  }

  .settings-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 5px 9px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: #8e8ea0;
    font-size: 12px;
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
  }

  .settings-btn:hover {
    background: #2a2a2a;
    color: #ececec;
  }
</style>
