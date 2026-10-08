import { invoke, convertFileSrc } from "@tauri-apps/api/core";

export { invoke };

/** URL of a capture served by the Rust side (`frozen-<monitor>-<gen>` or `shot-<id>`). */
export const imageUrl = (name: string) => convertFileSrc(name, "ks");

/** Tells Rust the page has painted, so it can show the (initially hidden) window. */
export const ready = () => invoke("window_ready");

export const closeWindow = () => invoke("close_window");

export const param = (key: string) => new URLSearchParams(location.search).get(key);

export const isMac = navigator.userAgent.includes("Mac");

export function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    // The capture comes from another origin (ks://); without CORS the
    // canvas would be tainted and the editor couldn't export it.
    img.crossOrigin = "anonymous";
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error(`failed to load ${src}`));
    img.src = src;
  });
}
