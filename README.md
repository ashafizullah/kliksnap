<p align="center">
  <img src="design/icon.png" width="128" alt="KlikSnap icon">
</p>

<h1 align="center">KlikSnap</h1>

<p align="center">
  A free, open-source screenshot tool for macOS and Windows, with a Linux beta. Small and fast, with no account and no cloud upload.
</p>

<p align="center">
  <a href="https://github.com/ashafizullah/kliksnap/actions/workflows/ci.yml"><img src="https://github.com/ashafizullah/kliksnap/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/ashafizullah/kliksnap/releases/latest"><img src="https://img.shields.io/github/v/release/ashafizullah/kliksnap?include_prereleases&label=release" alt="Release"></a>
  <a href="https://github.com/ashafizullah/kliksnap/releases"><img src="https://img.shields.io/github/downloads/ashafizullah/kliksnap/total?color=2563eb" alt="Downloads"></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/ashafizullah/kliksnap?color=blue" alt="MIT License"></a>
  <br>
  <img src="https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white" alt="Tauri 2">
  <img src="https://img.shields.io/badge/Svelte-5-FF3E00?logo=svelte&logoColor=white" alt="Svelte 5">
  <img src="https://img.shields.io/badge/Rust-stable-000000?logo=rust" alt="Rust">
  <img src="https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux%20(beta)-lightgrey" alt="macOS | Windows | Linux (beta)">
</p>

<p align="center">
  <a href="https://trakteer.id/adamshafizullah/tip"><img src="https://img.shields.io/badge/Buy%20me%20an%20AI%20token-Trakteer-C02E2E?style=for-the-badge" alt="Buy me an AI token on Trakteer"></a>
</p>

## Features

- **Capture** an area, a window, or the whole screen from a global shortcut or the menu bar / tray icon
- **Floating preview** after each capture: copy, save, pin, or open the editor
- **Share** from the editor through the system share menu (AirDrop, Messages, Mail… / Windows Share)
- **Scrolling capture**: select an area, scroll it, and get one tall stitched image (sticky headers kept once)
- **Screen recording** to MP4 (area or full screen) with the OS's built-in hardware encoder, with the computer's sound and the microphone if you want them
- **Record GIF**: an area straight to an animated GIF
- **Pin to screen**: keep a screenshot floating above your windows as a reference; fade it, let clicks pass through, or pin the image on the clipboard
- **Capture after a delay** (3, 5 or 10 s) and **capture the last area again**
- **Annotate**: arrow, line, rectangle, ellipse, pen, text, numbered steps, highlighter, blur, pixelate, crop, undo/redo
- **Auto-redact**: one click hides email addresses, phone and card numbers, IP addresses, API keys and passwords that OCR finds
- **Background**: place a screenshot on a gradient or solid backdrop with padding, rounded corners and a shadow
- **Color picker**: press `C` while selecting to copy the color under the crosshair as hex
- **Copy text (OCR)** from anything on screen, using the OCR engine built into the OS (Apple Vision / Windows.Media.Ocr; Tesseract on Linux). If the selection holds a QR code, its content is copied instead
- Copies to the clipboard automatically; optional auto-save to a folder as PNG or JPG, with a file name template
- **History**: recent captures, searchable by the text in them, to copy, edit, pin or save again
- **Capture resolution**: Max, Medium (75%) or Low (50%) for smaller files
- **English and Bahasa Indonesia**, following the system language or your choice in Settings
- **Auto-update** from GitHub Releases (signed packages; checked daily, or via tray → *Check for Updates…*)

The macOS app is about 4 MB per architecture (a 5 MB universal DMG) and uses about 15 MB of memory while idle.

## Download

Get the latest version from **[Releases](https://github.com/ashafizullah/kliksnap/releases/latest)**:

| Platform | File |
|---|---|
| macOS 12.3+ (Apple Silicon & Intel) | `KlikSnap_x.y.z_universal.dmg` |
| Windows 10/11 (x64) | `KlikSnap_x.y.z_x64-setup.exe` or `.msi` |
| Linux x86_64 (beta) | `KlikSnap_x.y.z_amd64.deb`, `KlikSnap-x.y.z-1.x86_64.rpm` or `KlikSnap_x.y.z_amd64.AppImage` |

**macOS:** open the DMG and drag KlikSnap into Applications. KlikSnap is signed and notarized by Apple, so it opens without a Gatekeeper warning. Or use the install script, which downloads the latest DMG, copies KlikSnap into Applications and opens it (run it again later to reinstall; updates otherwise arrive in the app):

```sh
curl -fsSL https://raw.githubusercontent.com/ashafizullah/kliksnap/main/install.sh | bash
```

Either way, allow **Screen Recording** when asked (System Settings → Privacy & Security → Screen Recording).

**Windows:** SmartScreen may warn about an unsigned app. Click **More info → Run anyway**.

**Linux (beta):** install the `.deb` (Ubuntu 24.04+, Debian 13+) or `.rpm` (Fedora), or `chmod +x` the AppImage and run it. Updating from inside the app is only sure to work for the AppImage; with the `.deb` or `.rpm`, install new versions from Releases. Install `tesseract-ocr` for Copy Text and `ffmpeg` for recording (the packages recommend both). What works depends on the session:

- **X11:** everything except Share; recording uses ffmpeg, with sound from PulseAudio or PipeWire.
- **Wayland** (the default on Ubuntu and Fedora): global shortcuts and recording don't work yet, and the selection overlay may misbehave. Add a shortcut in your desktop's keyboard settings that runs `kliksnap --capture area` (or `window`, `screen`, `text`, `scroll`). Logging in with an Xorg session gets you everything.

## Default shortcuts

| Action | Shortcut |
|---|---|
| Capture area | `⌥⇧4` / `Alt+Shift+4` |
| Capture window | `⌥⇧5` / `Alt+Shift+5` |
| Capture screen | `⌥⇧3` / `Alt+Shift+3` |
| Copy text (OCR) | `⌥⇧2` / `Alt+Shift+2` |
| Capture last area | not set (Settings → Shortcuts) |
| Record area / stop recording | not set (Settings → Shortcuts) |
| Scrolling capture | not set (Settings → Shortcuts) |

On Windows you can also hide the tray icon in Settings; KlikSnap keeps running on its shortcuts, and opening it again from the Start menu brings up Settings.

On Windows, turn on *Print Screen captures an area* in Settings to use `PrtScn` instead of the Snipping Tool.

In area mode, press `Space` to switch to window mode, `C` to copy the color under the crosshair and `Esc` to cancel. In the editor, use `A L R O D T N H B P C` to pick a tool, `1 2 3` or `[ ]` to set the size, `⌘⌫` to clear everything, `⌘C` to copy, `⌘S` to save, `⌘⇧S` for Save As and `⌘P` to pin.

A pin moves when dragged and zooms with the scroll wheel (`0` resets to 100%); hold `⌥` / `Alt` while scrolling to fade it. Right-click it for Copy, Save, Annotate, Opacity and Click Through; double-click or press `Esc` to close it. A click-through pin ignores the mouse until you choose *Make Pins Clickable Again* in the menu bar / tray menu.

For a scrolling capture, pick *Scrolling Capture* in the menu, select the part of the window that scrolls, then scroll down slowly and press *Done*.

## Development

Requires Node 22+ and stable Rust.

```sh
npm install
npm run tauri dev      # run
npm run tauri build    # bundle (.app/.dmg on macOS, .msi/.exe on Windows, .deb/.rpm/.AppImage on Linux)
```

On Linux, install the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/#linux) plus `libxcb1-dev libxrandr-dev libdbus-1-dev libpipewire-0.3-dev libwayland-dev libegl1-mesa-dev libgbm-dev clang` (PipeWire 1.0+, so Ubuntu 24.04 or later).

CI runs type checks, `cargo fmt`, `clippy` and tests on macOS, Windows and Linux for every push and pull request. Pushing a `v*` tag builds the universal macOS DMG, the Windows installers and the Linux packages, signs the update packages and attaches everything (including `latest.json` for the in-app updater) to a draft GitHub release. Publish the draft to roll it out: installed apps only see published releases.

Releases need the `TAURI_SIGNING_PRIVATE_KEY` repository secret, plus `APPLE_CERTIFICATE` (base64 of the Developer ID Application `.p12`), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD` (an app-specific password) and `APPLE_TEAM_ID` to sign and notarize the macOS build. The matching public key is in `tauri.conf.json`. Bump `version` in `tauri.conf.json`, `Cargo.toml` and `package.json` before tagging.

```sh
git tag v0.1.0 && git push origin v0.1.0
```

### How it stays light

- Tauri 2 uses the system webview (WKWebView / WebView2), so no Chromium is bundled
- Svelte 5 compiles away; the whole UI is ~75 KB of JS/CSS
- Windows are created on demand and destroyed when closed; idle KlikSnap is only a tray icon
- Captures go to the webview through a custom `ks://` protocol as uncompressed BMP, so nothing is base64-encoded through JSON
- OCR uses the OS engine instead of bundling Tesseract

### Layout

```
src-tauri/src/
  lib.rs        app state, capture flow, ks:// protocol
  capture.rs    screen/window capture (xcap), cropping
  ocr.rs        Apple Vision / Windows.Media.Ocr / Tesseract, QR codes
  record.rs     screen recording (ScreenCaptureKit / Windows Graphics Capture → MP4; ffmpeg on Linux)
  ui.rs         window creation and placement
  commands.rs   commands called from the webviews
  output.rs     clipboard, PNG saving
  platform.rs   macOS/Windows specifics (cursor, permissions, window level)
  hotkeys.rs, tray.rs, settings.rs
src/
  Overlay.svelte   area/window selection
  Preview.svelte   floating preview
  Editor.svelte    annotation editor (lib/editor/*)
  Settings.svelte, Toast.svelte
```

## Support

KlikSnap is free and always will be. If it saves you time, **[buy me an AI token on Trakteer](https://trakteer.id/adamshafizullah/tip)** ☕🤖. It keeps this project being developed.

<p align="center">
  <a href="https://trakteer.id/adamshafizullah/tip"><img src="design/trakteer-qr.png" width="200" alt="QR code: trakteer.id/adamshafizullah/tip"></a>
  <br>
  <sub>Scan to support on Trakteer</sub>
</p>

## Contributing

Issues and pull requests are welcome. `main` is protected: every change goes through a pull request and must pass CI (type checks, `cargo fmt`, `clippy` and tests on macOS, Windows and Linux). Please run `npm run check` and `cargo clippy` locally before opening a PR.

## License

[MIT](LICENSE)
