<script lang="ts">
  import { onMount } from "svelte";
  import { closeWindow, imageUrl, invoke, isMac, param, ready } from "./lib/api";
  import { tr } from "./lib/i18n";
  import { parse } from "./lib/markdown";

  const id = Number(param("id"));
  let answer = $state("");
  let error = $state("");
  let asking = $state(false);
  let copied = $state(false);
  // Switching here asks another profile without changing the one in use.
  let profiles = $state<string[]>([]);
  let profile = $state(0);

  const blocks = $derived(parse(answer));

  async function ask() {
    asking = true;
    error = "";
    answer = "";
    try {
      answer = await invoke<string>("explain", { id, profile });
    } catch (e) {
      error = String(e);
    } finally {
      asking = false;
    }
  }

  async function copy() {
    await navigator.clipboard.writeText(answer);
    copied = true;
    setTimeout(() => (copied = false), 1200);
  }

  function onKeyDown(e: KeyboardEvent) {
    const cmd = isMac ? e.metaKey : e.ctrlKey;
    if (e.key === "Escape" || (cmd && e.key.toLowerCase() === "w")) closeWindow();
  }

  onMount(async () => {
    ready();
    const p = await invoke<{ names: string[]; active: number }>("ai_profiles");
    profiles = p.names;
    profile = p.active;
    ask();
  });
</script>

<svelte:window onkeydown={onKeyDown} />

<main>
  <header>
    <img src={imageUrl(`shot-${id}`)} alt={tr("Screenshot")} draggable="false" />
    <h1>{tr("Explain with AI")}</h1>
    {#if profiles.length > 1}
      <select bind:value={profile} onchange={ask} disabled={asking} aria-label={tr("Profile")}>
        {#each profiles as name, i (i)}
          <option value={i}>{name.trim() || tr("Untitled")}</option>
        {/each}
      </select>
    {/if}
  </header>

  <section class="answer" aria-live="polite" aria-busy={asking}>
    {#if asking}
      <p class="muted thinking">{tr("Thinking…")}</p>
    {:else if error}
      <p class="error" role="alert">{error}</p>
    {:else}
      {#each blocks as b, i (i)}
        <svelte:element this={b.kind === "h" ? "h2" : b.kind}>
          {#each b.parts as part, j (j)}
            {#if part.code}<code>{part.text}</code>{:else if part.bold}<strong>{part.text}</strong
              >{:else if part.em}<em>{part.text}</em>{:else}{part.text}{/if}
          {/each}
        </svelte:element>
      {/each}
    {/if}
  </section>

  <footer>
    {#if error}
      <button class="secondary" onclick={() => invoke("open_ai_settings")}>{tr("AI Settings")}</button>
    {/if}
    <button class="secondary" disabled={asking} onclick={ask}>{tr("Ask Again")}</button>
    <button class="primary" disabled={asking || !answer} onclick={copy}>
      {copied ? tr("Copied") : tr("Copy")}
    </button>
  </footer>
</main>

<style>
  main {
    display: flex;
    flex-direction: column;
    gap: 12px;
    height: 100vh;
    padding: 16px 20px;
  }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  header img {
    width: 72px;
    height: 48px;
    object-fit: cover;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--panel);
  }
  h1 {
    flex: 1;
    margin: 0;
    font-size: 16px;
  }
  select {
    max-width: 160px;
    height: 26px;
    padding: 0 8px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--panel);
    color: inherit;
    font: inherit;
  }
  .answer {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 4px 14px 10px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--panel);
    -webkit-user-select: text;
    user-select: text;
  }
  .answer h2 {
    margin: 12px 0 4px;
    font-size: 13px;
  }
  .answer li {
    margin: 4px 0 4px 16px;
  }
  .answer p {
    margin: 8px 0;
  }
  code {
    padding: 0 4px;
    border-radius: 4px;
    background: var(--hover);
    font: 12px ui-monospace, Menlo, Consolas, monospace;
  }
  pre {
    margin: 8px 0;
    padding: 8px 10px;
    border-radius: 6px;
    background: var(--hover);
    overflow-x: auto;
  }
  pre code {
    padding: 0;
    background: none;
  }
  .muted {
    color: var(--muted);
  }
  .thinking {
    animation: pulse 1.2s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.4;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .thinking {
      animation: none;
    }
  }
  .error {
    color: var(--danger);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  footer button {
    height: 30px;
    padding: 0 14px;
    border-radius: 7px;
    font-weight: 500;
  }
  footer button:disabled {
    opacity: 0.5;
    cursor: default;
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
  .primary:hover:not(:disabled) {
    filter: brightness(1.1);
  }
</style>
