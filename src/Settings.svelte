<script lang="ts">
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { invoke, isLinux, isMac, isWindows, param, ready } from "./lib/api";
  import { tr } from "./lib/i18n";

  type Settings = {
    hotkey_area: string;
    hotkey_window: string;
    hotkey_screen: string;
    hotkey_text: string;
    hotkey_last_area: string;
    hotkey_record: string;
    hotkey_scroll: string;
    print_screen: boolean;
    save_dir: string;
    auto_copy: boolean;
    auto_save: boolean;
    capture_scale: number;
    image_format: "png" | "jpg";
    jpg_quality: number;
    file_template: string;
    record_template: string;
    record_countdown: number;
    record_scale: number;
    record_system_audio: boolean;
    record_mic: boolean;
    preview_secs: number;
    history_limit: number;
    live_selection: boolean;
    launch_at_login: boolean;
    check_updates: boolean;
    show_tray: boolean;
    language: "auto" | "en" | "id";
    ai_profiles: AiProfile[];
    ai_profile: number;
  };
  type AiProfile = { name: string; base_url: string; api_key: string; model: string };
  type HotkeyField = "hotkey_area" | "hotkey_window" | "hotkey_screen" | "hotkey_text" | "hotkey_last_area" | "hotkey_record" | "hotkey_scroll";

  const HOTKEYS: { field: HotkeyField; label: string }[] = [
    { field: "hotkey_area", label: "Capture area" },
    { field: "hotkey_window", label: "Capture window" },
    { field: "hotkey_screen", label: "Capture screen" },
    { field: "hotkey_text", label: "Copy text (OCR)" },
    { field: "hotkey_last_area", label: "Capture last area" },
    { field: "hotkey_record", label: "Record area (again to stop)" },
    { field: "hotkey_scroll", label: "Scrolling capture" },
  ];

  type Tab = "general" | "capture" | "recording" | "shortcuts" | "ai";
  const TABS: { id: Tab; label: string }[] = [
    { id: "general", label: "General" },
    { id: "capture", label: "Capture" },
    { id: "recording", label: "Recording" },
    { id: "shortcuts", label: "Shortcuts" },
    { id: "ai", label: "AI" },
  ];
  const isTab = (id: string | null): id is Tab => TABS.some((t) => t.id === id);
  const initialTab = param("tab");
  let tab = $state<Tab>(isTab(initialTab) ? initialTab : "general");

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

  let testing = $state(false);
  let aiStatus = $state<{ text: string; failed: boolean } | null>(null);

  const ai = $derived(s?.ai_profiles[s.ai_profile]);
  const aiReady = $derived(!!ai?.base_url.trim() && !!ai?.model.trim());

  function selectProfile() {
    aiStatus = null;
    save();
  }

  function addProfile() {
    if (!s) return;
    s.ai_profiles.push({
      name: tr("Profile {n}", { n: s.ai_profiles.length + 1 }),
      base_url: "https://api.openai.com/v1",
      api_key: "",
      model: "",
    });
    s.ai_profile = s.ai_profiles.length - 1;
    selectProfile();
  }

  function deleteProfile() {
    if (!s || s.ai_profiles.length < 2) return;
    s.ai_profiles.splice(s.ai_profile, 1);
    s.ai_profile = Math.min(s.ai_profile, s.ai_profiles.length - 1);
    selectProfile();
  }

  async function testAi() {
    if (!ai) return;
    testing = true;
    aiStatus = null;
    try {
      const text = await invoke<string>("test_ai", { profile: $state.snapshot(ai) });
      aiStatus = { text, failed: false };
    } catch (e) {
      aiStatus = { text: String(e), failed: true };
    } finally {
      testing = false;
    }
  }

  // What the next screenshot will be called, filled in by the Rust side.
  let example = $state("");
  $effect(() => {
    if (!s) return;
    invoke<string>("file_name_example", { template: s.file_template, format: s.image_format }).then((name) => (example = name));
  });
  let recordExample = $state("");
  $effect(() => {
    if (!s) return;
    invoke<string>("file_name_example", { template: s.record_template, format: "mp4" }).then((name) => (recordExample = name));
  });

  // Every window shows its language from when it opened: reload this one.
  async function setLanguage() {
    await save();
    location.reload();
  }

  // Text that may be pasted and the window closed at once, before a change
  // event: an API key, say. Saved shortly after typing stops instead.
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  function saveSoon() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(save, 400);
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
    if (!accelerator) return tr("Not set");
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

  onMount(() => {
    // Sent when something asks for a tab while Settings is already open.
    const unlisten = listen<string>("settings:tab", (e) => {
      if (isTab(e.payload)) selectTab(e.payload);
    });
    (async () => {
      [s, version] = await Promise.all([invoke<Settings>("get_settings"), invoke<string>("app_version")]);
      await tick();
      ready();
    })();
    return () => unlisten.then((off) => off());
  });
</script>

<svelte:window onkeydown={onRecordKey} onkeyup={onRecordKeyUp} />

{#if s}
  <main>
    <header>
      <h1>KlikSnap</h1>
      <p>
        {isMac
          ? tr("Free, open-source screenshots. Lives in your menu bar.")
          : tr("Free, open-source screenshots. Lives in your system tray.")}
      </p>
    </header>

    <div class="tabs" role="tablist" aria-label={tr("Settings")}>
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
          {tr(t.label)}
        </button>
      {/each}
    </div>

    <div class="panel" id="panel" role="tabpanel" aria-labelledby="tab-{tab}">
      {#if tab === "general"}
        <section>
          <h2>{tr("General")}</h2>
          <label class="row">
            <span>{tr("Language")}</span>
            <select bind:value={s.language} onchange={setLanguage}>
              <option value="auto">{tr("System ({lang})", { lang: navigator.language })}</option>
              <option value="en">English</option>
              <option value="id">Bahasa Indonesia</option>
            </select>
          </label>
          <label class="row">
            <span>{tr("Launch at login")}</span>
            <input type="checkbox" bind:checked={s.launch_at_login} onchange={save} />
          </label>
          {#if isWindows}
            <label class="row">
              <span>{tr("Show tray icon")}</span>
              <input type="checkbox" bind:checked={s.show_tray} onchange={save} />
            </label>
            {#if !s.show_tray}
              <p class="hint">{tr("KlikSnap keeps running on its shortcuts. Open it again from the Start menu to get back here.")}</p>
              <div class="row">
                <span>{tr("Stop KlikSnap until you open it again")}</span>
                <button class="secondary" onclick={() => invoke("quit")}>{tr("Quit")}</button>
              </div>
            {/if}
          {/if}
          <label class="row">
            <span>{tr("Check for updates automatically")}</span>
            <input type="checkbox" bind:checked={s.check_updates} onchange={save} />
          </label>
          <div class="row">
            <span>{tr("Version {v}", { v: version })}</span>
            <button class="secondary" disabled={checking} onclick={checkNow}>
              {checking ? tr("Checking…") : tr("Check Now")}
            </button>
          </div>
          {#if updateStatus}
            <p class="hint" class:error={updateStatus.failed} role="status">{updateStatus.text}</p>
          {/if}
        </section>

        <section>
          <h2>{tr("Save location")}</h2>
          <div class="row">
            <span class="path" title={s.save_dir}>{s.save_dir}</span>
            <button class="secondary" onclick={chooseFolder}>{tr("Change…")}</button>
          </div>
        </section>
      {:else if tab === "capture"}
        <section>
          <h2>{tr("Selection")}</h2>
          <label class="row">
            <span>{tr("Live screen while selecting")}</span>
            <input type="checkbox" bind:checked={s.live_selection} onchange={save} />
          </label>
          <p class="hint">
            {s.live_selection
              ? tr("Videos keep playing; the shot is taken when you finish selecting. Open menus may close first.")
              : tr("The screen freezes when you press the shortcut, so open menus and tooltips stay in the shot.")}
          </p>
        </section>

        <section>
          <h2>{tr("After capture")}</h2>
          <label class="row">
            <span>{tr("Copy to clipboard")}</span>
            <input type="checkbox" bind:checked={s.auto_copy} onchange={save} />
          </label>
          <label class="row">
            <span>{tr("Save to folder")}</span>
            <input type="checkbox" bind:checked={s.auto_save} onchange={save} />
          </label>
          <label class="row">
            <span>{tr("Resolution")}</span>
            <select bind:value={s.capture_scale} onchange={save}>
              <option value={100}>{tr("Max (100%)")}</option>
              <option value={75}>{tr("Medium (75%)")}</option>
              <option value={50}>{tr("Low (50%)")}</option>
            </select>
          </label>
          {#if s.capture_scale < 100}
            <p class="hint">
              {tr("Smaller files, less detail.")}
              {isMac ? tr("On a Retina display, Low matches the size things appear on screen.") : ""}
              {tr("Copy Text (OCR) always reads the full resolution.")}
            </p>
          {/if}
          <label class="row">
            <span>{tr("Hide preview after")}</span>
            <select bind:value={s.preview_secs} onchange={save}>
              <option value={3}>{tr("{n} seconds", { n: 3 })}</option>
              <option value={6}>{tr("{n} seconds", { n: 6 })}</option>
              <option value={10}>{tr("{n} seconds", { n: 10 })}</option>
              <option value={0}>{tr("Never")}</option>
            </select>
          </label>
        </section>

        <section>
          <h2>{tr("History")}</h2>
          <label class="row">
            <span>{tr("Keep recent captures")}</span>
            <select bind:value={s.history_limit} onchange={save}>
              <option value={0}>{tr("Off")}</option>
              <option value={20}>20</option>
              <option value={50}>50</option>
              <option value={100}>100</option>
              <option value={200}>200</option>
            </select>
          </label>
          <div class="row">
            <span>{tr("Find, copy or edit past captures")}</span>
            <button class="secondary" onclick={() => invoke("open_history")}>{tr("Open History")}</button>
          </div>
          <p class="hint">
            {tr("Stored only on this {device}, and searchable by the text in them.", { device: isMac ? "Mac" : "PC" })}
          </p>
        </section>

        <section>
          <h2>{tr("File")}</h2>
          <label class="row">
            <span>{tr("Format")}</span>
            <select bind:value={s.image_format} onchange={save}>
              <option value="png">{tr("PNG (lossless)")}</option>
              <option value="jpg">{tr("JPG (smaller)")}</option>
            </select>
          </label>
          {#if s.image_format === "jpg"}
            <label class="row">
              <span>{tr("JPG quality")}</span>
              <span class="range">
                <input type="range" min="40" max="100" step="5" bind:value={s.jpg_quality} onchange={save} />
                <span class="value">{s.jpg_quality}</span>
              </span>
            </label>
          {/if}
          <label class="row">
            <span>{tr("File name")}</span>
            <input class="template" type="text" spellcheck="false" bind:value={s.file_template} onchange={save} />
          </label>
          <p class="hint">
            {example}<br />
            {tr("Date fields: %Y year, %m month, %d day, %H hour, %M minute, %S second.")}
          </p>
        </section>
      {:else if tab === "recording"}
        <section>
          <h2>{tr("Recording")}</h2>
          <label class="row">
            <span>{tr("Countdown before recording")}</span>
            <select bind:value={s.record_countdown} onchange={save}>
              <option value={0}>{tr("Off")}</option>
              <option value={3}>{tr("{n} seconds", { n: 3 })}</option>
              <option value={5}>{tr("{n} seconds", { n: 5 })}</option>
              <option value={10}>{tr("{n} seconds", { n: 10 })}</option>
            </select>
          </label>
          <label class="row">
            <span>{tr("Resolution")}</span>
            <select bind:value={s.record_scale} onchange={save}>
              <option value={100}>{tr("Max (100%)")}</option>
              <option value={75}>{tr("Medium (75%)")}</option>
              <option value={50}>{tr("Low (50%)")}</option>
            </select>
          </label>
          <p class="hint">
            {s.record_scale < 100
              ? `${tr("Smaller videos, less detail.")}${isMac ? " " + tr("On a Retina display, Low records at the size things appear on screen.") : ""}`
              : tr("Full detail; the largest files.")}
          </p>
          {#if isLinux}
            <p class="hint">{tr("Recording needs ffmpeg and an X11 session; it doesn't work on Wayland yet.")}</p>
          {/if}
          <label class="row">
            <span>{tr("File name")}</span>
            <input class="template" type="text" spellcheck="false" bind:value={s.record_template} onchange={save} />
          </label>
          <p class="hint">
            {recordExample}<br />
            {tr("Date fields: %Y year, %m month, %d day, %H hour, %M minute, %S second.")}
          </p>
        </section>

        <section>
          <h2>{tr("Sound")}</h2>
          <label class="row">
            <span>{tr("Record computer sound")}</span>
            <input type="checkbox" bind:checked={s.record_system_audio} onchange={save} />
          </label>
          <label class="row">
            <span>{tr("Record microphone")}</span>
            <input type="checkbox" bind:checked={s.record_mic} onchange={save} />
          </label>
          <p class="hint">
            {isMac
              ? tr(
                  "Computer sound needs macOS 13 or later, the microphone macOS 15. Each goes on its own track. GIFs have no sound.",
                )
              : tr("Both are mixed into one track. GIFs have no sound.")}
          </p>
        </section>
      {:else if tab === "ai"}
        <section>
          <h2>{tr("AI model")}</h2>
          <p class="hint">{tr("Any OpenAI-compatible API, with your own key: OpenAI, OpenRouter, Groq, Ollama and others.")}</p>
          <div class="row">
            <span>{tr("Profile")}</span>
            <div class="controls">
              <select bind:value={s.ai_profile} onchange={selectProfile} aria-label={tr("Profile")}>
                {#each s.ai_profiles as p, i (i)}
                  <option value={i}>{p.name.trim() || tr("Untitled")}</option>
                {/each}
              </select>
              <button class="secondary icon" title={tr("Add profile")} aria-label={tr("Add profile")} onclick={addProfile}>+</button>
              <button
                class="secondary icon"
                title={tr("Delete profile")}
                aria-label={tr("Delete profile")}
                disabled={s.ai_profiles.length < 2}
                onclick={deleteProfile}>−</button
              >
            </div>
          </div>
          {#if ai}
            <label class="row">
              <span>{tr("Name")}</span>
              <input class="template" type="text" spellcheck="false" bind:value={ai.name} oninput={saveSoon} />
            </label>
            <label class="row">
              <span>{tr("Base URL")}</span>
              <input class="template" type="url" spellcheck="false" placeholder="https://api.openai.com/v1" bind:value={ai.base_url} oninput={saveSoon} />
            </label>
            <label class="row">
              <span>{tr("API key")}</span>
              <input class="template" type="password" spellcheck="false" autocomplete="off" placeholder="sk-…" bind:value={ai.api_key} oninput={saveSoon} />
            </label>
            <label class="row">
              <span>{tr("Model")}</span>
              <input class="template" type="text" spellcheck="false" placeholder="gpt-4o-mini" bind:value={ai.model} oninput={saveSoon} />
            </label>
          {/if}
          <p class="hint">{tr("Make sure the model can read images (vision); KlikSnap sends it your screenshots.")}</p>
          <div class="row">
            <span>{tr("Check the model reads an image")}</span>
            <button class="secondary" disabled={testing || !aiReady} onclick={testAi}>
              {testing ? tr("Testing…") : tr("Test")}
            </button>
          </div>
          {#if aiStatus}
            <p class="hint" class:error={aiStatus.failed} role="status">{aiStatus.text}</p>
          {/if}
        </section>
      {:else}
        <section>
          <h2>{tr("Shortcuts")}</h2>
          {#each HOTKEYS as h (h.field)}
            <div class="row">
              <span>{tr(h.label)}</span>
              <button
                class="hotkey"
                class:recording={recording === h.field}
                onclick={() => (recording = recording === h.field ? null : h.field)}
              >
                {recording === h.field ? tr("Press keys…") : pretty(s[h.field])}
              </button>
            </div>
          {/each}
          <p class="hint">{tr("Click a shortcut, then press the new keys. Backspace clears it, Esc cancels.")}</p>
          {#if isWindows}
            <label class="row">
              <span>{tr("Print Screen captures an area")}</span>
              <input type="checkbox" bind:checked={s.print_screen} onchange={save} />
            </label>
            <p class="hint">{tr("Replaces the Snipping Tool on the Print Screen key.")}</p>
          {/if}
          {#if isLinux}
            <p class="hint">
              {tr("On Wayland these shortcuts don't work. Add one in your system's keyboard settings that runs:")}
              <code>kliksnap --capture area</code>
              {tr("(or window, screen, text, record).")}
            </p>
          {/if}
        </section>
      {/if}
    </div>

    <footer>
      {#if error}
        <span class="error" role="alert">{error}</span>
      {:else if saved}
        <span class="ok">{tr("Saved")}</span>
      {:else}
        <span>{tr("Free & open source · MIT License")}</span>
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
  .controls {
    display: flex;
    gap: 6px;
  }
  .secondary.icon {
    min-width: 26px;
    padding: 0;
  }
  .secondary:disabled {
    opacity: 0.5;
    cursor: default;
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
  .range {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .range input {
    width: 120px;
    accent-color: var(--accent);
  }
  .value {
    min-width: 3ch;
    font-variant-numeric: tabular-nums;
  }
  .template {
    width: 220px;
    height: 26px;
    padding: 0 10px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
    color: inherit;
    font: inherit;
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
