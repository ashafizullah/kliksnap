<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { closeWindow, imageUrl, invoke, param, ready } from "./lib/api";

  const secs = Number(param("t") ?? 6);

  const id = Number(param("id"));
  let status = $state("");
  let hovering = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let shown = false;

  function schedule() {
    clearTimeout(timer);
    if (secs > 0 && !hovering) timer = setTimeout(closeWindow, secs * 1000);
  }

  function flash(text: string) {
    status = text;
    setTimeout(() => (status = ""), 1600);
  }

  async function run(action: () => Promise<unknown>, done: string) {
    try {
      await action();
      flash(done);
    } catch (e) {
      flash(String(e));
    }
  }

  const copy = () => run(() => invoke("copy_shot", { id }), "Copied");
  const save = () => run(() => invoke("save_shot", { id }), "Saved");
  const edit = () => invoke("edit_shot", { id });

  function setHover(on: boolean) {
    if (on === hovering) return;
    hovering = on;
    if (on) clearTimeout(timer);
    else schedule();
  }

  onMount(() => {
    // Sent by Rust: the webview gets no mouse-move events while unfocused.
    const unlistenHover = listen<boolean>("preview:hover", (e) => setHover(e.payload));
    schedule();
    return () => {
      clearTimeout(timer);
      unlistenHover.then((f) => f());
    };
  });

  function onLoad() {
    if (shown) return;
    shown = true;
    ready();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="card"
  class:hover={hovering}
  onmouseenter={() => setHover(true)}
  onmouseleave={() => setHover(false)}
>
  <img src={imageUrl(`shot-${id}`)} alt="Screenshot" draggable="false" onload={onLoad} onerror={closeWindow} />

  <div class="actions">
    <button class="close" title="Close" aria-label="Close" onclick={closeWindow}>
      <svg viewBox="0 0 16 16"><path d="M4 4l8 8M12 4l-8 8" /></svg>
    </button>
    <button class="edit" onclick={edit}>Annotate</button>
    <div class="row">
      <button onclick={copy}>Copy</button>
      <button onclick={save}>Save</button>
    </div>
  </div>

  {#if status}
    <div class="status">{status}</div>
  {/if}
</div>

<style>
  :global(body) {
    background: #111;
  }
  .card {
    position: fixed;
    inset: 0;
    overflow: hidden;
    background: #111;
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    display: block;
  }
  .actions {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    background: rgba(0, 0, 0, 0.55);
    opacity: 0;
    transition: opacity 0.12s;
  }
  .card:hover .actions,
  .card.hover .actions {
    opacity: 1;
  }
  button {
    border: 0;
    border-radius: 6px;
    padding: 6px 12px;
    background: rgba(255, 255, 255, 0.92);
    color: #111;
    font-weight: 600;
  }
  button:hover {
    background: #fff;
  }
  .edit {
    background: #2563eb;
    color: #fff;
    min-width: 132px;
  }
  .edit:hover {
    background: #1d4ed8;
  }
  .row {
    display: flex;
    gap: 8px;
  }
  .row button {
    min-width: 62px;
  }
  .close {
    position: absolute;
    top: 6px;
    left: 6px;
    width: 22px;
    height: 22px;
    padding: 0;
    border-radius: 50%;
    display: grid;
    place-items: center;
  }
  .close svg {
    width: 10px;
    height: 10px;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
  }
  .status {
    position: absolute;
    left: 50%;
    bottom: 10px;
    transform: translateX(-50%);
    max-width: 90%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    padding: 4px 10px;
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.8);
    color: #fff;
    font-size: 12px;
    pointer-events: none;
  }
</style>
