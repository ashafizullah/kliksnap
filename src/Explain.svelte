<script lang="ts">
  import { onMount, tick } from "svelte";
  import { closeWindow, imageUrl, invoke, isMac, param, ready } from "./lib/api";
  import { tr } from "./lib/i18n";
  import { LANGUAGES } from "./lib/languages";
  import Markdown from "./lib/Markdown.svelte";

  type Turn = { role: "user" | "assistant"; content: string };
  type Action = { name: string; prompt: string };

  const id = Number(param("id"));
  // Switching profile here asks another one without changing the one in use.
  let profiles = $state<string[]>([]);
  let profile = $state(0);
  let actions = $state<Action[]>([]);
  let action = $state(0);
  // What `{language}` in a prompt becomes: the reply language from Settings,
  // until you pick another here for this window.
  let language = $state("English");
  const usesLanguage = $derived(actions[action]?.prompt.includes("{language}") ?? false);
  // One conversation per action, kept while you switch between them; an
  // action runs by itself only the first time it's opened. The first turn is
  // the action's prompt, shown as its chip rather than as a message.
  let chats = $state<Turn[][]>([]);
  let errors = $state<string[]>([]);
  let busy = $state<boolean[]>([]);
  const turns = $derived(chats[action] ?? []);
  const error = $derived(errors[action] ?? "");
  const asking = $derived(busy[action] ?? false);
  const anyBusy = $derived(busy.some(Boolean));
  let draft = $state("");
  let copied = $state(false);
  let chatEl = $state<HTMLElement>();

  const lastAnswer = $derived([...turns].reverse().find((t) => t.role === "assistant")?.content ?? "");

  async function scrollToEnd() {
    await tick();
    chatEl?.scrollTo({ top: chatEl.scrollHeight });
  }

  /** Sends action `i`'s conversation and adds the reply to it. */
  async function send(i: number) {
    busy[i] = true;
    errors[i] = "";
    scrollToEnd();
    try {
      const reply = await invoke<string>("ai_chat", { id, profile, turns: $state.snapshot(chats[i]) });
      chats[i].push({ role: "assistant", content: reply });
    } catch (e) {
      errors[i] = String(e);
    } finally {
      busy[i] = false;
      if (i === action) scrollToEnd();
    }
  }

  /** Starts action `i`'s conversation over. */
  function start(i: number) {
    action = i;
    chats[i] = [{ role: "user", content: actions[i].prompt.replaceAll("{language}", language) }];
    send(i);
  }

  /** Shows action `i`: what it already answered, or its first run. */
  function open(i: number) {
    action = i;
    if (chats[i]) scrollToEnd();
    else start(i);
  }

  /** Another model answers differently: forget every answer, run this one again. */
  function pickProfile() {
    chats = [];
    errors = [];
    start(action);
  }

  function pickLanguage() {
    start(action);
  }

  function followUp() {
    const text = draft.trim();
    const chat = chats[action];
    if (!text || asking || !chat) return;
    // A question whose answer failed is replaced: models expect turns to alternate.
    if (chat.length > 1 && chat[chat.length - 1].role === "user") chat.pop();
    chat.push({ role: "user", content: text });
    draft = "";
    send(action);
  }

  /** Asks the last question again, in place of its answer. */
  function askAgain() {
    const chat = chats[action];
    if (!chat) return start(action);
    if (chat[chat.length - 1]?.role === "assistant") chat.pop();
    send(action);
  }

  /** Copies the last answer; just the inside when it's one code block, like a CSV. */
  async function copy() {
    const fenced = lastAnswer.trim().match(/^```[^\n]*\n([\s\S]*?)\n?```$/);
    await navigator.clipboard.writeText(fenced ? fenced[1] : lastAnswer);
    copied = true;
    setTimeout(() => (copied = false), 1200);
  }

  function onInputKey(e: KeyboardEvent) {
    // Enter sends, Shift+Enter breaks the line; not while an IME is composing.
    if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      followUp();
    }
  }

  function onKeyDown(e: KeyboardEvent) {
    // Esc while an IME is composing cancels the composition, not the window.
    if (e.isComposing) return;
    const cmd = isMac ? e.metaKey : e.ctrlKey;
    if (e.key === "Escape" || (cmd && e.key.toLowerCase() === "w")) closeWindow();
  }

  onMount(async () => {
    ready();
    const info = await invoke<{ profiles: string[]; active: number; actions: Action[]; language: string }>("ai_info");
    profiles = info.profiles;
    profile = info.active;
    actions = info.actions;
    language = info.language;
    start(0);
  });
</script>

<svelte:window onkeydown={onKeyDown} />

<main>
  <header>
    <img src={imageUrl(`shot-${id}`)} alt={tr("Screenshot")} draggable="false" />
    <h1>{tr("Ask AI")}</h1>
    {#if profiles.length > 1}
      <select bind:value={profile} onchange={pickProfile} disabled={anyBusy} aria-label={tr("Profile")}>
        {#each profiles as name, i (i)}
          <option value={i}>{name.trim() || tr("Untitled")}</option>
        {/each}
      </select>
    {/if}
  </header>

  <div class="actions" role="group" aria-label={tr("Actions")}>
    {#each actions as a, i (i)}
      <button class="chip" class:active={action === i} aria-pressed={action === i} title={a.prompt} onclick={() => open(i)}>
        {tr(a.name.trim() || "Untitled")}
      </button>
    {/each}
    {#if usesLanguage}
      <label class="into">
        <span>{tr("into")}</span>
        <select bind:value={language} onchange={pickLanguage} disabled={asking} aria-label={tr("Language")}>
          {#each LANGUAGES as l (l.en)}
            <option value={l.en}>{l.native}</option>
          {/each}
          {#if !LANGUAGES.some((l) => l.en === language)}
            <option value={language}>{language}</option>
          {/if}
        </select>
      </label>
    {/if}
  </div>

  <section class="chat" bind:this={chatEl} aria-live="polite" aria-busy={asking}>
    {#each turns.slice(1) as t, i (i)}
      {#if t.role === "user"}
        <p class="question">{t.content}</p>
      {:else}
        <div class="answer"><Markdown text={t.content} /></div>
      {/if}
    {/each}
    {#if asking}
      <p class="muted thinking">{tr("Thinking…")}</p>
    {:else if error}
      <p class="error" role="alert">{error}</p>
    {/if}
  </section>

  <form
    class="ask"
    onsubmit={(e) => {
      e.preventDefault();
      followUp();
    }}
  >
    <textarea rows="1" placeholder={tr("Ask a follow-up…")} bind:value={draft} onkeydown={onInputKey} aria-label={tr("Ask a follow-up…")}
    ></textarea>
    <button class="primary" type="submit" disabled={asking || !draft.trim() || !lastAnswer}>{tr("Send")}</button>
  </form>

  <footer>
    {#if error}
      <button class="secondary" onclick={() => invoke("open_ai_settings")}>{tr("AI Settings")}</button>
    {/if}
    <button class="secondary" disabled={asking || turns.length === 0} onclick={askAgain}>{tr("Ask Again")}</button>
    <button class="secondary" disabled={asking || !lastAnswer} onclick={copy}>
      {copied ? tr("Copied") : tr("Copy")}
    </button>
  </footer>
</main>

<style>
  main {
    display: flex;
    flex-direction: column;
    gap: 10px;
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
  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .into {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-left: 4px;
    color: var(--muted);
  }
  .chip {
    height: 26px;
    padding: 0 12px;
    border: 1px solid var(--border);
    border-radius: 13px;
    background: var(--panel);
  }
  .chip:hover:not(:disabled) {
    background: var(--hover);
  }
  .chip.active {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--accent-text);
  }
  .chat {
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
  .question {
    margin: 12px 0 4px auto;
    width: fit-content;
    max-width: 85%;
    padding: 6px 10px;
    border-radius: 10px;
    background: var(--hover);
    white-space: pre-wrap;
  }
  .muted {
    color: var(--muted);
  }
  .thinking {
    margin: 10px 0;
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
    margin: 10px 0;
    color: var(--danger);
  }
  .ask {
    display: flex;
    gap: 8px;
  }
  textarea {
    flex: 1;
    min-height: 30px;
    max-height: 96px;
    padding: 6px 10px;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--panel);
    color: inherit;
    font: inherit;
    resize: none;
    field-sizing: content;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  footer button,
  .ask button {
    height: 30px;
    padding: 0 14px;
    border-radius: 7px;
    font-weight: 500;
  }
  footer button:disabled,
  .ask button:disabled {
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
