<script lang="ts">
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
  import { closeWindow, invoke, isMac, ready } from "./lib/api";
  import { tr } from "./lib/i18n";
  import { parse } from "./lib/markdown";

  type Info = { version: string; current: string; notes: string; url: string };

  let info = $state<Info | null>(null);
  let expanded = $state(false);
  let overflows = $state(false);
  let notesEl = $state<HTMLElement>();
  let installing = $state(false);
  let progress = $state<{ downloaded: number; total: number | null } | null>(null);
  let error = $state("");

  const blocks = $derived(info ? parse(info.notes) : []);
  const percent = $derived(
    progress?.total ? Math.min(100, Math.round((progress.downloaded / progress.total) * 100)) : null,
  );

  async function install() {
    installing = true;
    error = "";
    try {
      // Restarts the app when it succeeds, so this only returns on failure.
      await invoke("update_install");
    } catch (e) {
      error = String(e);
      installing = false;
      progress = null;
    }
  }

  /** Shows all the notes: the window grows to fit them, up to most of the screen. */
  async function toggle() {
    expanded = !expanded;
    if (!expanded || !notesEl) return;
    const extra = notesEl.scrollHeight - notesEl.clientHeight;
    const height = Math.min(innerHeight + extra + 8, Math.round(screen.availHeight * 0.8));
    if (height > innerHeight) await getCurrentWindow().setSize(new LogicalSize(innerWidth, height));
  }

  function onKeyDown(e: KeyboardEvent) {
    const cmd = isMac ? e.metaKey : e.ctrlKey;
    if (!installing && (e.key === "Escape" || (cmd && e.key.toLowerCase() === "w"))) closeWindow();
  }

  onMount(() => {
    const unlisten = listen<{ downloaded: number; total: number | null }>("update:progress", (e) => {
      progress = e.payload;
    });
    invoke<Info | null>("update_info").then(async (i) => {
      if (!i) return closeWindow();
      info = i;
      await tick();
      overflows = !!notesEl && notesEl.scrollHeight > notesEl.clientHeight + 4;
      ready();
    });
    return () => unlisten.then((off) => off());
  });
</script>

<svelte:window onkeydown={onKeyDown} />

{#if info}
  <main>
    <header>
      <h1>{tr("KlikSnap {version} is available", { version: info.version })}</h1>
      <p>{tr("You have {version}.", { version: info.current })}</p>
    </header>

    <section class="notes-wrap" class:expanded>
      <div class="notes" bind:this={notesEl}>
        {#if blocks.length}
          {#each blocks as b, i (i)}
            <svelte:element this={b.kind === "h" ? "h2" : b.kind === "li" ? "li" : "p"}>
              {#each b.parts as part, j (j)}
                {#if part.code}<code>{part.text}</code>{:else if part.bold}<strong>{part.text}</strong
                  >{:else if part.em}<em>{part.text}</em>{:else}{part.text}{/if}
              {/each}
            </svelte:element>
          {/each}
        {:else}
          <p class="muted">{tr("No release notes.")}</p>
        {/if}
      </div>
      {#if overflows && !expanded}
        <div class="fade" aria-hidden="true"></div>
      {/if}
    </section>

    <div class="links">
      {#if overflows}
        <button class="link" onclick={toggle}>
          {expanded ? tr("Show less") : tr("Read more")}
        </button>
      {/if}
      <button class="link" onclick={() => invoke("update_notes")}>{tr("Full release notes on GitHub")}</button>
    </div>

    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}

    <footer>
      {#if installing}
        <div class="progress" role="status">
          <div class="bar"><div class="fill" style:width="{percent ?? 100}%" class:busy={percent === null}></div></div>
          <span>
            {percent === null || percent < 100
              ? tr("Downloading… {percent}", { percent: percent === null ? "" : `${percent}%` })
              : tr("Installing and restarting…")}
          </span>
        </div>
      {:else}
        <button class="secondary" onclick={closeWindow}>{tr("Later")}</button>
        <button class="primary" onclick={install}>{tr("Install and Restart")}</button>
      {/if}
    </footer>
  </main>
{/if}

<style>
  main {
    display: flex;
    flex-direction: column;
    gap: 10px;
    height: 100vh;
    padding: 18px 20px 16px;
  }
  h1 {
    margin: 0;
    font-size: 16px;
  }
  header p {
    margin: 2px 0 0;
    color: var(--muted);
  }
  .notes-wrap {
    position: relative;
    flex: 1;
    min-height: 0;
    max-height: 170px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--panel);
    overflow: hidden;
  }
  .notes-wrap.expanded {
    max-height: none;
  }
  .notes {
    height: 100%;
    max-height: inherit;
    padding: 4px 14px 10px;
    overflow: hidden;
    -webkit-user-select: text;
    user-select: text;
  }
  .expanded .notes {
    overflow-y: auto;
  }
  .notes h2 {
    margin: 10px 0 4px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
  }
  .notes li {
    margin: 4px 0 4px 16px;
  }
  .notes p {
    margin: 6px 0;
  }
  code {
    padding: 0 4px;
    border-radius: 4px;
    background: var(--hover);
    font: 12px ui-monospace, Menlo, Consolas, monospace;
  }
  .fade {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 48px;
    background: linear-gradient(transparent, var(--panel));
    pointer-events: none;
  }
  .links {
    display: flex;
    gap: 16px;
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--accent);
  }
  .link:hover {
    text-decoration: underline;
  }
  .muted {
    color: var(--muted);
  }
  .error {
    margin: 0;
    color: var(--danger);
    -webkit-user-select: text;
    user-select: text;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: auto;
  }
  footer button {
    height: 30px;
    padding: 0 14px;
    border-radius: 7px;
    font-weight: 500;
  }
  .secondary {
    border: 1px solid var(--border);
    background: var(--panel);
  }
  .primary {
    border: 0;
    background: var(--accent);
    color: var(--accent-text);
  }
  .primary:hover {
    filter: brightness(1.1);
  }
  .progress {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--muted);
  }
  .bar {
    flex: 1;
    height: 6px;
    border-radius: 3px;
    background: var(--hover);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--accent);
    transition: width 0.2s;
  }
  .fill.busy {
    animation: pulse 1.2s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.4;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .fill {
      transition: none;
    }
    .fill.busy {
      animation: none;
    }
  }
</style>
