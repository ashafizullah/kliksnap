import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  build: {
    target: ["safari15", "chrome105"],
    rollupOptions: {
      input: {
        overlay: "overlay.html",
        preview: "preview.html",
        editor: "editor.html",
        settings: "settings.html",
        history: "history.html",
        update: "update.html",
        toast: "toast.html",
        pin: "pin.html",
        countdown: "countdown.html",
        recording: "recording.html",
        scroll: "scroll.html",
      },
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    host: host || "127.0.0.1",
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
});
