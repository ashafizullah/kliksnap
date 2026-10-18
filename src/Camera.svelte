<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { invoke, param, ready } from "./lib/api";
  import { RADIUS, openCamera } from "./lib/camera";

  // The webcam over the region being recorded, so it is recorded too, framed
  // as Settings say. Drag it anywhere; it closes itself without a camera.
  const radius = RADIUS[param("shape") ?? ""] ?? RADIUS.circle;
  const border = Math.max(0, Number(param("border")) || 0);
  const color = param("color") ?? "#ffffff";
  const mirror = param("mirror") !== "0";
  // Room for the shadow around the webcam, in pixels; 0 casts none.
  const shadow = Math.max(0, Number(param("shadow")) || 0);
  let video: HTMLVideoElement;
  let live = $state(false);

  onMount(() => {
    let stream: MediaStream | undefined;
    let closed = false;
    ready();
    openCamera(param("device") ?? "", param("label") ?? "")
      .then((s) => {
        if (closed) return s.getTracks().forEach((t) => t.stop());
        stream = s;
        video.srcObject = s;
      })
      .catch((e: DOMException) => invoke("camera_failed", { denied: e?.name === "NotAllowedError" }));
    return () => {
      closed = true;
      stream?.getTracks().forEach((t) => t.stop());
    };
  });

  function drag(e: MouseEvent) {
    if (e.button === 0) getCurrentWindow().startDragging();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="bubble"
  class:shadow={shadow > 0}
  style:inset="{shadow}px"
  style:border-radius={radius}
  style:border="{border}px solid {color}"
  onmousedown={drag}
>
  <video bind:this={video} class:live class:mirror autoplay muted playsinline onplaying={() => (live = true)}></video>
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent;
    margin: 0;
    overflow: hidden;
  }
  .bubble {
    position: fixed;
    inset: 0;
    box-sizing: border-box;
    overflow: hidden;
    background: #1c1c1e;
    cursor: grab;
  }
  .bubble.shadow {
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.45);
  }
  video {
    width: 100%;
    height: 100%;
    object-fit: cover;
    opacity: 0;
    transition: opacity 0.2s;
    pointer-events: none;
  }
  video.live {
    opacity: 1;
  }
  /* As people expect to see themselves. */
  video.mirror {
    transform: scaleX(-1);
  }
</style>
