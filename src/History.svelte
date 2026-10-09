<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { closeWindow, imageUrl, invoke, isMac, ready } from "./lib/api";
  import { tr } from "./lib/i18n";

  type Entry = { id: string; created: number; width: number; height: number; scale: number; text: string };

  let entries = $state<Entry[]>([]);
  let loaded = $state(false);
  let historyOn = $state(true);
  let query = $state("");
  let selected = $state<string | null>(null);
  let confirmClear = $state(false);
  let status = $state("");
  let statusTimer: ReturnType<typeof setTimeout> | undefined;

  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return entries;
    return entries.filter((e) => e.text.toLowerCase().includes(q));
  });

  function flash(text: string) {
    status = text;
    clearTimeout(statusTimer);
    statusTimer = setTimeout(() => (status = ""), 2000);
  }

  async function refresh() {
    entries = await invoke<Entry[]>("history_list");
    if (selected && !entries.some((e) => e.id === selected)) selected = null;
    loaded = true;
  }

  const DONE: Record<string, string> = { copy: "Copied", save: "Saved to your folder" };


  async function act(id: string, action: "copy" | "edit" | "pin" | "save" | "delete") {
    try {
      await invoke("history_action", { id, action });
      if (action === "delete") {
        entries = entries.filter((e) => e.id !== id);
        if (selected === id) selected = null;
      } else if (DONE[action]) {
        flash(tr(DONE[action]));
      }
    } catch (e) {
      flash(String(e));
    }
  }

  async function clearAll() {
    if (!confirmClear) {
      confirmClear = true;
      setTimeout(() => (confirmClear = false), 3000);
      return;
    }
    confirmClear = false;
    await invoke("history_clear");
    await refresh();
  }

  const timeFormat = new Intl.DateTimeFormat(undefined, { hour: "2-digit", minute: "2-digit" });
  const dayFormat = new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric" });

  function when(ms: number) {
    const d = new Date(ms);
    const today = new Date();
    const yesterday = new Date(today.getTime() - 86_400_000);
    const same = (a: Date, b: Date) => a.toDateString() === b.toDateString();
    const day = same(d, today) ? tr("Today") : same(d, yesterday) ? tr("Yesterday") : dayFormat.format(d);
    return `${day}, ${timeFormat.format(d)}`;
  }

  function onKeyDown(e: KeyboardEvent) {
    const cmd = isMac ? e.metaKey : e.ctrlKey;
    const typing = e.target instanceof HTMLInputElement;
    if (cmd && e.key.toLowerCase() === "w") {
      e.preventDefault();
      closeWindow();
    } else if (cmd && e.key.toLowerCase() === "f") {
      e.preventDefault();
      document.querySelector<HTMLInputElement>("input[type=search]")?.focus();
    } else if (e.key === "Escape" && !typing) {
      selected = null;
    } else if (selected && !typing) {
      if (e.key === "Enter") act(selected, "edit");
      else if (e.key === "Backspace" || e.key === "Delete") act(selected, "delete");
      else if (cmd && e.key.toLowerCase() === "c") act(selected, "copy");
    }
  }

  onMount(() => {
    const unlisten = listen("history:changed", refresh);
    Promise.all([
      refresh(),
      invoke<{ history_limit: number }>("get_settings").then((s) => (historyOn = s.history_limit > 0)),
    ]).finally(ready);
    return () => unlisten.then((off) => off());
  });
</script>

<svelte:window onkeydown={onKeyDown} onfocus={refresh} />

<div class="app">
  <header>
    <input type="search" placeholder={tr("Search text in captures")} aria-label={tr("Search")} bind:value={query} spellcheck="false" />
    <span class="count">{tr("{shown} of {total}", { shown: shown.length, total: entries.length })}</span>
    <button class="danger" class:confirm={confirmClear} disabled={!entries.length} onclick={clearAll}>
      {confirmClear ? tr("Click again to delete all") : tr("Clear All")}
    </button>
  </header>

  <main>
    {#if loaded && !entries.length}
      <div class="empty">
        {#if historyOn}
          <p><strong>{tr("No captures yet")}</strong></p>
          <p>{tr("Screenshots you take show up here, newest first.")}</p>
        {:else}
          <p><strong>{tr("History is off")}</strong></p>
          <p>{tr("Turn it on in Settings → Capture.")}</p>
        {/if}
      </div>
    {:else if loaded && !shown.length}
      <div class="empty"><p>{tr("No capture contains “{query}”.", { query: query.trim() })}</p></div>
    {:else}
      <ul class="grid" aria-label={tr("Captures")}>
        {#each shown as e (e.id)}
          <li>
            <div
              class="card"
              class:selected={selected === e.id}
              role="button"
              tabindex="0"
              aria-label={tr("Capture from {when}", { when: when(e.created) })}
              onclick={() => (selected = e.id)}
              ondblclick={() => act(e.id, "edit")}
              onkeydown={(k) => k.key === " " && (selected = e.id)}
            >
              <div class="thumb">
                <img src={imageUrl(`hist-${e.id}`)} alt="" loading="lazy" draggable="false" />
              </div>
              <div class="meta">
                <span>{when(e.created)}</span>
                <span class="muted">{e.width} × {e.height}</span>
              </div>
              <div class="actions">
                <button onclick={(ev) => (ev.stopPropagation(), act(e.id, "copy"))}>{tr("Copy")}</button>
                <button onclick={(ev) => (ev.stopPropagation(), act(e.id, "edit"))}>{tr("Edit")}</button>
                <button onclick={(ev) => (ev.stopPropagation(), act(e.id, "pin"))}>{tr("Pin")}</button>
                <button onclick={(ev) => (ev.stopPropagation(), act(e.id, "save"))}>{tr("Save")}</button>
                <button
                  class="icon"
                  title={tr("Delete")}
                  aria-label={tr("Delete")}
                  onclick={(ev) => (ev.stopPropagation(), act(e.id, "delete"))}
                >
                  <svg viewBox="0 0 18 18"><path d="M4 5h10M7.5 5V3.5h3V5M5.5 5l.7 9.5h5.6l.7-9.5" /></svg>
                </button>
              </div>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
    {#if status}
      <div class="status" role="status">{status}</div>
    {/if}
  </main>
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
  }
  input[type="search"] {
    flex: 1;
    min-width: 0;
    height: 30px;
    padding: 0 10px;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--bg);
    color: inherit;
    font: inherit;
    -webkit-user-select: text;
    user-select: text;
  }
  .count {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  header button {
    height: 30px;
    padding: 0 12px;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--panel);
    white-space: nowrap;
  }
  header button:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .danger {
    color: var(--danger);
  }
  .danger.confirm {
    background: var(--danger);
    border-color: var(--danger);
    color: #fff;
  }
  main {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 14px;
  }
  .grid {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 12px;
  }
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--panel);
    overflow: hidden;
    cursor: default;
  }
  .card.selected {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent);
  }
  .thumb {
    aspect-ratio: 16 / 10;
    display: grid;
    place-items: center;
    background: var(--bg);
    background-image: repeating-conic-gradient(rgba(127, 127, 127, 0.12) 0 25%, transparent 0 50%);
    background-size: 12px 12px;
  }
  .thumb img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    display: block;
  }
  .meta {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    padding: 6px 10px 8px;
    font-size: 12px;
  }
  .muted {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .actions {
    position: absolute;
    left: 6px;
    right: 6px;
    top: 6px;
    display: flex;
    gap: 4px;
    opacity: 0;
    transition: opacity 0.12s;
  }
  .card:hover .actions,
  .card:focus-within .actions,
  .card.selected .actions {
    opacity: 1;
  }
  .actions button {
    height: 26px;
    padding: 0 8px;
    border: 0;
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.72);
    color: #fff;
    font-size: 12px;
  }
  .actions button:hover {
    background: rgba(0, 0, 0, 0.88);
  }
  .actions .icon {
    margin-left: auto;
    width: 26px;
    padding: 0;
    display: grid;
    place-items: center;
  }
  .actions svg {
    width: 15px;
    height: 15px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .empty {
    height: 100%;
    display: grid;
    place-content: center;
    text-align: center;
    color: var(--muted);
  }
  .empty p {
    margin: 2px 0;
  }
  .empty strong {
    color: var(--text);
    font-size: 15px;
  }
  .status {
    position: fixed;
    bottom: 14px;
    left: 50%;
    transform: translateX(-50%);
    padding: 6px 12px;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.8);
    color: #fff;
  }
  @media (prefers-reduced-motion: reduce) {
    .actions {
      transition: none;
    }
  }
</style>
