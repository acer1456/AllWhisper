<script lang="ts">
  import type { Segment } from "../types";
  import { videoElement, transcript, sessions, activeSessionId } from "../stores/app";
  import { get } from "svelte/store";
  import { tick } from "svelte";
  import { t } from "../i18n";

  let { segment, active = false }: { segment: Segment; active?: boolean } = $props();

  let editing = $state(false);
  let editText = $state("");
  let textareaEl = $state<HTMLTextAreaElement | undefined>(undefined);

  // Delay single-click to distinguish from double-click
  let clickTimer: ReturnType<typeof setTimeout> | null = null;

  function handleClick() {
    if (editing) return;
    if (clickTimer) {
      // Second click within 300ms → enter edit mode
      clearTimeout(clickTimer);
      clickTimer = null;
      startEdit();
    } else {
      clickTimer = setTimeout(() => {
        clickTimer = null;
        seekTo();
      }, 300);
    }
  }

  function seekTo() {
    const el = $videoElement;
    if (!el) return;
    el.currentTime = segment.start;
    el.play().catch(() => {});
  }

  async function startEdit() {
    editText = segment.text;
    editing = true;
    await tick();
    textareaEl?.focus();
    // Select all text for easy replacement
    textareaEl?.select();
    autoResize();
  }

  function saveEdit() {
    if (!editing) return;
    editing = false;
    const trimmed = editText.trim();
    if (trimmed === segment.text) return;

    // 1. 更新記憶體中的 transcript store
    transcript.update(t => {
      if (!t) return t;
      return {
        ...t,
        segments: t.segments.map(s =>
          s.id === segment.id ? { ...s, text: trimmed } : s
        ),
      };
    });

    // 2. 同步更新 sessions store 中的對應 session，確保持久化寫入 JSON
    const sessionId = get(activeSessionId);
    if (sessionId) {
      sessions.update(list =>
        list.map(sess => {
          if (sess.id !== sessionId) return sess;
          return {
            ...sess,
            transcript: {
              ...sess.transcript,
              segments: sess.transcript.segments.map(s =>
                s.id === segment.id ? { ...s, text: trimmed } : s
              ),
            },
          };
        })
      );
    }
  }

  function cancelEdit() {
    editing = false;
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      saveEdit();
    } else if (e.key === "Escape") {
      e.preventDefault();
      cancelEdit();
    }
  }

  function autoResize() {
    if (!textareaEl) return;
    textareaEl.style.height = "auto";
    textareaEl.style.height = textareaEl.scrollHeight + "px";
  }

  function formatTime(s: number) {
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const sec = Math.floor(s % 60);
    const ms = Math.floor((s % 1) * 1000);
    if (h > 0) return `${h}:${String(m).padStart(2, "0")}:${String(sec).padStart(2, "0")}`;
    return `${String(m).padStart(2, "0")}:${String(sec).padStart(2, "0")}.${String(ms).padStart(3, "0")}`;
  }
</script>

{#if editing}
  <!-- Edit mode: edit the original text only; translation shown as read-only hint -->
  <div class="segment editing" class:active>
    <span class="timestamp">{formatTime(segment.start)}</span>
    <textarea
      bind:this={textareaEl}
      bind:value={editText}
      class="edit-textarea"
      rows={1}
      oninput={autoResize}
      onblur={saveEdit}
      onkeydown={onKeydown}
    ></textarea>
    <div class="edit-hint">{$t.item.editHint}</div>
  </div>
{:else}
  <!-- View mode: single click → seek, double-click (two fast clicks) → edit -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_interactive_supports_focus -->
  <div
    class="segment"
    class:active
    role="button"
    tabindex="0"
    onclick={handleClick}
    onkeydown={(e) => e.key === "Enter" && seekTo()}
    title={$t.item.clickHint.replace("{time}", formatTime(segment.start))}
  >
    <span class="timestamp">{formatTime(segment.start)}</span>
    <span class="text-block">
      <span class="text">{segment.text}</span>
      {#if segment.translation}
        <span class="translation">{segment.translation}</span>
      {/if}
    </span>
    <span class="edit-icon" aria-hidden="true">✎</span>
  </div>
{/if}

<style>
  .segment {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    width: 100%;
    padding: 8px 12px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: #c5c5d2;
    cursor: pointer;
    text-align: left;
    transition: background 0.12s;
    line-height: 1.5;
    position: relative;
  }

  .segment:hover {
    background: #2a2a2a;
  }

  .segment:hover .edit-icon {
    opacity: 0.4;
  }

  .segment.active {
    background: #1a3a2e;
    color: #ececec;
  }

  .segment.editing {
    background: #1e2433;
    border: 1px solid #3a5080;
    cursor: default;
    flex-wrap: wrap;
    gap: 6px;
  }

  .timestamp {
    flex-shrink: 0;
    font-size: 11px;
    font-family: "SF Mono", "Fira Code", monospace;
    color: #10a37f;
    opacity: 0.9;
    padding-top: 2px;
    min-width: 72px;
  }

  .segment.active .timestamp {
    opacity: 1;
  }

  .text-block {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .text {
    font-size: 13px;
    line-height: 1.6;
  }

  .translation {
    display: block;
    font-size: 12px;
    line-height: 1.5;
    color: #7ec8a4;
    margin-top: 2px;
    font-style: italic;
  }

  .segment.active .translation { color: #a8dfc0; }

  .edit-icon {
    position: absolute;
    right: 10px;
    top: 10px;
    font-size: 12px;
    color: #8e8ea0;
    opacity: 0;
    transition: opacity 0.15s;
    pointer-events: none;
    user-select: none;
  }

  /* ── Edit mode ── */
  .edit-textarea {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: #ececec;
    font-size: 13px;
    font-family: inherit;
    line-height: 1.6;
    resize: none;
    overflow: hidden;
    min-height: 1.6em;
    padding: 0;
    width: 0; /* let flex handle width */
  }

  .edit-hint {
    width: 100%;
    font-size: 10px;
    color: #4a6080;
    padding-left: calc(72px + 10px); /* align with text column */
    margin-top: -2px;
    pointer-events: none;
  }
</style>
