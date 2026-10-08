<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke, isMac, ready } from "./lib/api";

  type Settings = {
    hotkey_area: string;
    hotkey_window: string;
    hotkey_screen: string;
    hotkey_text: string;
    hotkey_last_area: string;
    hotkey_record: string;
    print_screen: boolean;
    save_dir: string;
    auto_copy: boolean;
    auto_save: boolean;
    capture_scale: number;
    record_countdown: number;
    record_scale: number;
    preview_secs: number;
    live_selection: boolean;
    launch_at_login: boolean;
    check_updates: boolean;
    show_tray: boolean;
  };
  type HotkeyField = "hotkey_area" | "hotkey_window" | "hotkey_screen" | "hotkey_text" | "hotkey_last_area" | "hotkey_record";

  const HOTKEYS: { field: HotkeyField; label: string }[] = [
    { field: "hotkey_area", label: "Capture area" },
    { field: "hotkey_window", label: "Capture window" },
    { field: "hotkey_screen", label: "Capture screen" },
    { field: "hotkey_text", label: "Copy text (OCR)" },
    { field: "hotkey_last_area", label: "Capture last area" },
    { field: "hotkey_record", label: "Record area (again to stop)" },
  ];

  type Tab = "general" | "capture" | "recording" | "shortcuts";
  const TABS: { id: Tab; label: string }[] = [
    { id: "general", label: "General" },
    { id: "capture", label: "Capture" },
    { id: "recording", label: "Recording" },
    { id: "shortcuts", label: "Shortcuts" },
  ];
  let tab = $state<Tab>("general");

  function selectTab(id: Tab) {
    recording = null;
    tab = id;
  }

  // Arrow keys move between tabs, as in native tab bars.
  function onTabKey(e: KeyboardEvent) {
    const step = { ArrowRight: 1, ArrowLeft: -1 }[e.key];
    if (!step) return;
    e.preventDefault();
    const i = TABS.findIndex((t) => t.id === tab);
    const next = TABS[(i + step + TABS.length) % TABS.length].id;
    selectTab(next);
    document.getElementById(`tab-${next}`)?.focus();
  }

  let s = $state<Settings | null>(null);
  let recording = $state<HotkeyField | null>(null);
  let error = $state("");
  let saved = $state(false);
  let version = $state("");
  let checking = $state(false);
  let updateStatus = $state<{ text: string; failed: boolean } | null>(null);

  async function checkNow() {
    checking = true;
    updateStatus = null;
    try {
      const text = await invoke<string>("check_updates");
      updateStatus = text ? { text, failed: false } : null;
    } catch (e) {
      updateStatus = { text: String(e), failed: true };
    } finally {
      checking = false;
    }
  }

  async function save() {
    if (!s) return;
    try {
      await invoke("save_settings", { settings: s });
      error = "";
      saved = true;
      setTimeout(() => (saved = false), 1200);
    } catch (e) {
      error = String(e);
    }
  }

  const MAC_SYMBOLS: Record<string, string> = { Command: "⌘", Control: "⌃", Alt: "⌥", Shift: "⇧", CommandOrControl: "⌘" };

  function pretty(accelerator: string) {
    if (!accelerator) return "Not set";
    const parts = accelerator.split("+");
    if (!isMac) return parts.map((p) => (p === "CommandOrControl" ? "Ctrl" : p)).join(" + ");
    return parts.map((p) => MAC_SYMBOLS[p] ?? p).join("");
  }

  /** Converts a key event into a Tauri accelerator such as "Alt+Shift+4". */
  function accelerator(e: KeyboardEvent): string | null {
    const mods = [e.metaKey && "Command", e.ctrlKey && "Control", e.altKey && "Alt", e.shiftKey && "Shift"].filter(
      Boolean,
    ) as string[];
    const code = e.code;
    if (/^(Meta|Control|Alt|Shift|OS)/.test(code)) return null;
    const key = code.replace(/^Key/, "").replace(/^Digit/, "");
    const standalone = /^F\d+$/.test(key) || key === "PrintScreen";
    if (!mods.length && !standalone) return null;
    return [...mods, key].join("+");
  }

  // Windows sends no keydown for Print Screen, only keyup.
  function onRecordKeyUp(e: KeyboardEvent) {
    if (e.code === "PrintScreen") onRecordKey(e);
  }

  function onRecordKey(e: KeyboardEvent) {
    if (!recording || !s) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") {
      recording = null;
      return;
    }
    if ((e.key === "Backspace" || e.key === "Delete") && !e.metaKey && !e.ctrlKey && !e.altKey) {
      s[recording] = "";
    } else {
      const acc = accelerator(e);
      if (!acc) return;
      s[recording] = acc;
    }
    recording = null;
    save();
  }

  async function chooseFolder() {
    const dir = await invoke<string | null>("pick_folder");
    if (dir && s) {
      s.save_dir = dir;
      save();
    }
  }

  onMount(async () => {
    [s, version] = await Promise.all([invoke<Settings>("get_settings"), invoke<string>("app_version")]);
    await tick();
    ready();
  });
</script>

<svelte:window onkeydown={onRecordKey} onkeyup={onRecordKeyUp} />

{#if s}
  <main>
    <header>
      <h1>KlikSnap</h1>
      <p>Free, open-source screenshots. Lives in your {isMac ? "menu bar" : "system tray"}.</p>
    </header>

    <div class="tabs" role="tablist" aria-label="Settings">
      {#each TABS as t (t.id)}
        <button
          role="tab"
          id="tab-{t.id}"
          aria-selected={tab === t.id}
          aria-controls="panel"
          tabindex={tab === t.id ? 0 : -1}
          class:active={tab === t.id}
          onclick={() => selectTab(t.id)}
          onkeydown={onTabKey}
        >
          {t.label}
        </button>
      {/each}
    </div>

    <div class="panel" id="panel" role="tabpanel" aria-labelledby="tab-{tab}">
      {#if tab === "general"}
        <section>
          <h2>General</h2>
          <label class="row">
            <span>Launch at login</span>
            <input type="checkbox" bind:checked={s.launch_at_login} onchange={save} />
          </label>
          {#if !isMac}
            <label class="row">
              <span>Show tray icon</span>
              <input type="checkbox" bind:checked={s.show_tray} onchange={save} />
            </label>
            {#if !s.show_tray}
              <p class="hint">KlikSnap keeps running on its shortcuts. Open it again from the Start menu to get back here.</p>
              <div class="row">
                <span>Stop KlikSnap until you open it again</span>
                <button class="secondary" onclick={() => invoke("quit")}>Quit</button>
              </div>
            {/if}
          {/if}
          <label class="row">
            <span>Check for updates automatically</span>
            <input type="checkbox" bind:checked={s.check_updates} onchange={save} />
          </label>
          <div class="row">
            <span>Version {version}</span>
            <button class="secondary" disabled={checking} onclick={checkNow}>
              {checking ? "Checking…" : "Check Now"}
            </button>
          </div>
          {#if updateStatus}
            <p class="hint" class:error={updateStatus.failed} role="status">{updateStatus.text}</p>
          {/if}
        </section>

        <section>
          <h2>Save location</h2>
          <div class="row">
            <span class="path" title={s.save_dir}>{s.save_dir}</span>
            <button class="secondary" onclick={chooseFolder}>Change…</button>
          </div>
        </section>
      {:else if tab === "capture"}
        <section>
          <h2>Selection</h2>
          <label class="row">
            <span>Live screen while selecting</span>
            <input type="checkbox" bind:checked={s.live_selection} onchange={save} />
          </label>
          <p class="hint">
            {s.live_selection
              ? "Videos keep playing; the shot is taken when you finish selecting. Open menus may close first."
              : "The screen freezes when you press the shortcut, so open menus and tooltips stay in the shot."}
          </p>
        </section>

        <section>
          <h2>After capture</h2>
          <label class="row">
            <span>Copy to clipboard</span>
            <input type="checkbox" bind:checked={s.auto_copy} onchange={save} />
          </label>
          <label class="row">
            <span>Save to folder</span>
            <input type="checkbox" bind:checked={s.auto_save} onchange={save} />
          </label>
          <label class="row">
            <span>Resolution</span>
            <select bind:value={s.capture_scale} onchange={save}>
              <option value={100}>Max (100%)</option>
              <option value={75}>Medium (75%)</option>
              <option value={50}>Low (50%)</option>
            </select>
          </label>
          {#if s.capture_scale < 100}
            <p class="hint">
              Smaller files, less detail. {isMac ? "On a Retina display, Low matches the size things appear on screen." : ""}
              Copy Text (OCR) always reads the full resolution.
            </p>
          {/if}
          <label class="row">
            <span>Hide preview after</span>
            <select bind:value={s.preview_secs} onchange={save}>
              <option value={3}>3 seconds</option>
              <option value={6}>6 seconds</option>
              <option value={10}>10 seconds</option>
              <option value={0}>Never</option>
            </select>
          </label>
        </section>
      {:else if tab === "recording"}
        <section>
          <h2>Recording</h2>
          <label class="row">
            <span>Countdown before recording</span>
            <select bind:value={s.record_countdown} onchange={save}>
              <option value={0}>Off</option>
              <option value={3}>3 seconds</option>
              <option value={5}>5 seconds</option>
              <option value={10}>10 seconds</option>
            </select>
          </label>
          <label class="row">
            <span>Resolution</span>
            <select bind:value={s.record_scale} onchange={save}>
              <option value={100}>Max (100%)</option>
              <option value={75}>Medium (75%)</option>
              <option value={50}>Low (50%)</option>
            </select>
          </label>
          <p class="hint">
            {s.record_scale < 100
              ? `Smaller videos, less detail.${isMac ? " On a Retina display, Low records at the size things appear on screen." : ""}`
              : "Full detail; the largest files."}
          </p>
        </section>
      {:else}
        <section>
          <h2>Shortcuts</h2>
          {#each HOTKEYS as h (h.field)}
            <div class="row">
              <span>{h.label}</span>
              <button
                class="hotkey"
                class:recording={recording === h.field}
                onclick={() => (recording = recording === h.field ? null : h.field)}
              >
                {recording === h.field ? "Press keys…" : pretty(s[h.field])}
              </button>
            </div>
          {/each}
          <p class="hint">Click a shortcut, then press the new keys. Backspace clears it, Esc cancels.</p>
          {#if !isMac}
            <label class="row">
              <span>Print Screen captures an area</span>
              <input type="checkbox" bind:checked={s.print_screen} onchange={save} />
            </label>
            <p class="hint">Replaces the Snipping Tool on the Print Screen key.</p>
          {/if}
        </section>
      {/if}
    </div>

    <footer>
      {#if error}
        <span class="error" role="alert">{error}</span>
      {:else if saved}
        <span class="ok">Saved</span>
      {:else}
        <span>Free & open source · MIT License</span>
      {/if}
    </footer>
  </main>
{/if}

<style>
  main {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 20px;
    height: 100vh;
    overflow-y: auto;
  }
  header h1 {
    margin: 0;
    font-size: 18px;
  }
  header p,
  .hint,
  footer {
    margin: 2px 0 0;
    color: var(--muted);
  }
  .tabs {
    display: flex;
    gap: 2px;
    padding: 3px;
    border-radius: 9px;
    background: var(--hover);
  }
  .tabs button {
    flex: 1;
    height: 28px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-weight: 500;
  }
  .tabs button:hover {
    color: var(--text);
  }
  .tabs button.active {
    background: var(--panel);
    color: var(--text);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.15);
  }
  .tabs button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .panel {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  section {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 4px 14px 10px;
  }
  h2 {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
    margin: 10px 0 4px;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 34px;
  }
  .hint {
    font-size: 12px;
  }
  .hotkey,
  .secondary,
  select {
    min-width: 110px;
    height: 26px;
    padding: 0 10px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
    font: inherit;
    color: inherit;
  }
  .hotkey {
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.05em;
  }
  .hotkey.recording {
    border-color: var(--accent);
    color: var(--accent);
  }
  .path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    -webkit-user-select: text;
    user-select: text;
  }
  input[type="checkbox"] {
    width: 16px;
    height: 16px;
    accent-color: var(--accent);
  }
  footer {
    margin-top: auto;
    font-size: 12px;
  }
  .error {
    color: var(--danger);
  }
  .ok {
    color: var(--accent);
  }
</style>
