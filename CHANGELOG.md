# Changelog

The section for each version becomes its GitHub release notes and the text of
the in-app "Update available" dialog. Keep entries short and user-facing.

## 0.14.8

### Changed

- Settings → General has a GitHub button, next to Open-source licenses, which now shows and hides the license text itself
- In Indonesian, Open-source licenses now reads "Lisensi open source", as in the other Klik apps

## 0.14.7

### Changed

- Third-party notices now include every license and notice text for packages under more than one license, and a source link for the MPL-2.0 crates
- The macOS DMG itself is now notarized and stapled, not only the app inside it

## 0.14.6

### New

- Third-party license notices, shown under Settings → General → Open-source licenses

### Changed

- The notices give the full license text for crates published without a license file (one, libwayshot-xcap, still links to its license)
- The app's copyright line names Adam Suchi Hafizullah

## 0.14.5

### Fixed

- Copy Text on macOS no longer keeps failing with "CRImageReaderError error 1" until KlikSnap is restarted: when the Neural Engine stops reading text, it reads on the CPU instead

## 0.14.4

### Changed

- Data saved under the old `com.kliksnap.app` identifier is no longer moved over. Updating straight from 0.14.2 or earlier: settings, capture history and editor preferences start fresh

## 0.14.3

### Changed

- App identifier changed from `com.kliksnap.app` to `com.ashafizullah.kliksnap`, matching the other Klik apps. Settings, capture history and editor preferences move over on first launch. On macOS, allow Screen Recording (and Accessibility, for auto-scroll) again when asked

## 0.14.2

### Fixed

- Windows: screenshots keep the tray's hidden-icons flyout (the up arrow) open in the picture instead of closing it when you start selecting

## 0.14.1

### Fixed

- Screenshots of an open menu (a tray, menu bar or right-click menu) keep the menu in the picture instead of closing it

## 0.14.0

### New

- Show your webcam in recordings: turn it on in Settings → Recording → Webcam (macOS and Windows). It is recorded with the screen and can be dragged anywhere while recording. Pick the camera, where it starts (a corner, or where you left it last time), its shape (circle, rounded or square), size, border thickness and color, mirroring and a shadow, with a live preview in Settings
- While recording, the tray icon turns into a red dot: click it to stop. On macOS the recording time shows beside it

### Improved

- The recording controls move out of the webcam's way, and can be dragged elsewhere
- Settings → Recording is split into Recording, Screen, Sound and Webcam sections
- Double-click a slider in Settings to reset it

## 0.13.0

### New

- Highlight mouse clicks in recordings: each click shows a ripple, and a dot follows the mouse while the button is held. Turn it on in Settings → Recording (macOS and Windows)
- Hide the mouse pointer in recordings: Settings → Recording → Show mouse pointer

### Improved

- The recording controls are see-through and show the record shortcut, which stops the recording too
- The countdown before a recording has rounded corners

## 0.12.0

### New

- Scrolling capture scrolls by itself: select an area and it scrolls to the bottom of the page and stops there. On macOS it needs Accessibility permission (System Settings → Privacy & Security → Accessibility); without it, scroll by hand as before
- Color picker: press C while selecting, the cursor becomes an eyedropper, click to copy the color. C or Esc goes back

### Improved

- Scrolling capture copes with see-through headers, floating buttons and small animations, and no longer stops short of the bottom
- Click the preview to open it in the editor
- Space switches to window mode more reliably, also across several displays

## 0.11.0

### New

- Ask AI about a screenshot: set up any OpenAI-compatible model with your own API key in Settings → AI (OpenAI, OpenRouter, Groq, Ollama and others; the model must read images). Press Explain in the preview or the editor, then Translate, Summarize, turn a table into CSV, or ask follow-up questions. Add your own actions, keep several AI profiles and pick the language the AI answers in
- Copy Text can read with AI instead of on-device OCR: Settings → Capture → Read text with. Better with handwriting, tables and mixed languages; if the AI fails, on-device OCR takes over
- Sound effects: a camera shutter for screenshots and a soft pop when text is copied. Turn them off in Settings → Capture

### Improved

- Settings, History, the editor and the AI window stay on top, so they don't vanish behind the app you switch to
- The preview and notices are rounded cards with a border that shows on light and dark screens

## 0.10.1

### Improved

- macOS: KlikSnap is now signed and notarized by Apple, so the DMG opens without a Gatekeeper warning and permissions stay granted across future updates. After this update, macOS asks once more for Screen Recording (and Accessibility or Microphone if you use them): allow them again in System Settings → Privacy & Security

## 0.10.0

### New

- Linux (beta): `.deb`, `.rpm` and AppImage for x86_64. Screenshots, the editor, pins, history, QR codes and scrolling capture work; Copy Text uses Tesseract and recording uses ffmpeg when they're installed. On Wayland, global shortcuts and recording don't work yet: bind `kliksnap --capture area` (or `window`, `screen`, `text`) to a shortcut in your desktop settings
- `kliksnap --capture <mode>` starts a capture from the command line or a desktop shortcut, on every platform

## 0.9.6

### New

- Device mockups: put a screenshot in a MacBook, desktop monitor, phone, app window or browser window, on any background. In the editor, open Background → Frame. With a Transparent background you get the mockup alone

## 0.9.5

### New

- Name recordings from a template, like screenshots: Settings → Recording → File name, for example `rec-%Y%m%d-%H%M%S`
- Copy Text (OCR): hold `Shift` as you let go of the selection to copy only the text, skipping any QR code beside it

## 0.9.4

### Improved

- Easier install on macOS: run `curl -fsSL https://raw.githubusercontent.com/ashafizullah/kliksnap/main/install.sh | bash` in Terminal to install or reinstall KlikSnap without the Gatekeeper warning. The download instructions also now cover macOS 15, where right-click → Open no longer works (use System Settings → Privacy & Security → Open Anyway)

## 0.9.3

### Fixed

- Recording with the microphone on macOS: KlikSnap now asks for microphone access and shows up under System Settings → Privacy & Security → Microphone. Before, macOS refused it silently and the app was missing from that list

## 0.9.2

### Improved

- Update offers open in their own window instead of a system dialog, so long release notes are no longer cut off: *Read more* shows them all, with a link to the full notes on GitHub, and installing shows the download's progress

## 0.9.1

### Fixes

- Recording no longer fails when the microphone isn't allowed: KlikSnap asks for it the first time, and records without it (saying so) if it's refused. Sound that can't be captured is left out instead of stopping the recording
- When macOS refuses screen recording, the notice says how to fix it and opens the right System Settings pane

## 0.9.0

### New

- Scrolling capture: select an area, scroll it, and get one tall image. Sticky headers and footers appear once. Tray → *Scrolling Capture*, or set a shortcut
- Record GIF: tray → *Record GIF* records an area straight to an animated GIF
- Sound in recordings: the computer's audio and the microphone (Settings → Recording). macOS needs 13 or later for computer sound and 15 for the microphone
- History: your recent captures in one window, searchable by the text in them, to copy, edit, pin or save again. Tray → *History…*; set how many to keep in Settings → Capture
- Auto-redact in the editor: one click pixelates email addresses, phone and card numbers, IP addresses, API keys and passwords found by OCR
- Pins: fade them (right-click → Opacity, or `⌥`/`Alt` + scroll), let clicks pass through to the windows below, and pin the image on the clipboard from the tray
- Bahasa Indonesia: menus, notices and every window, following the system language or Settings → General → Language

## 0.8.0

### New

- Color picker: while selecting an area, the loupe shows the color under the crosshair. Press `C` to copy it as a hex code (`#RRGGBB`)
- Editor background: put a screenshot on a gradient or solid backdrop, with padding, rounded corners and a shadow. Pick it from the background button in the toolbar; your last choice carries over to the next screenshot
- Editor: freehand pen (`D`) and blur (`B`) tools
- Save screenshots as PNG or JPG (with a quality setting), and name files from a template such as `shot-%H%M%S`: Settings → Capture → File. Save As follows the extension you type

## 0.7.0

### New

- Screen recording to MP4: tray → *Record Area* or *Record Screen*. A 3-second countdown, then a small timer with a Stop button that stays out of the video. Stop from it, the tray, or the record shortcut. The video goes to your save folder; click the notice to show it. Uses the hardware encoder built into macOS and Windows, no audio yet
- Pin a screenshot to the screen: it floats above other windows where you took it. Drag to move, scroll to zoom, right-click for Copy, Save and Annotate, double-click or `Esc` to close. Pin from the preview, or from the editor with `⌘P` / `Ctrl+P`
- Capture after a delay: tray → *Capture After Delay* takes an area or the whole screen after 3, 5 or 10 seconds. Click the countdown to cancel
- Capture Last Area takes the same area again without selecting it
- Resolution settings for screenshots and recordings: Max, Medium (75%) or Low (50%) for smaller files. Copy Text (OCR) always reads the full resolution
- Editor: numbered steps (`N`), a size slider for every tool (double-click resets it), text fonts and bold, Clear All (`⌘⌫` / `Ctrl+Backspace`), and Share through the system share menu (AirDrop, Messages, Mail… / Windows Share)
- Settings are split into tabs: General, Capture, Recording, Shortcuts
- macOS: the menu bar icon is a monochrome KS that turns white or black with the menu bar

### Fixes

- Check Now in Settings shows the result in Settings, and Settings stays open

## 0.5.0

### New

- Live selection: the screen keeps moving while you select, and the shot is taken when you let go. Turn it off in Settings → Selection to freeze the screen when the shortcut fires instead
- Scroll while selecting to bring the part you want into view
- The loupe magnifies the live screen as you select

### Fixes

- Preview buttons now show on hover right away, without a click first
- Guide lines missing when the mouse was moving as the shortcut fired

## 0.4.1

### Fixes

- macOS: screenshots now work after granting Screen Recording permission

## 0.4.0

### New

- Windows: the tray icon can be hidden (Settings → General); KlikSnap keeps running on its shortcuts, and opening it again brings up Settings
- Opening KlikSnap while it is already running opens Settings instead of starting a second copy

## 0.3.0

### New

- Windows: Print Screen can capture an area instead of opening the Snipping Tool (Settings → Shortcuts)

## 0.2.1

### Fixes

- Annotate not responding when clicked in the preview on Windows
- The update dialog now shows what's new instead of install instructions

## 0.2.0

### New

- QR code scanning: in Copy Text mode, a selection holding a QR code copies its content
- Previews stack in the bottom-right corner instead of replacing each other, and stay there when you switch desktops
- The preview shows Annotate, Copy and Save as soon as you hover it
- Save & Copy in the preview and the editor; Copy and Save now close them
- The tray icon now matches the app icon

### Fixes

- OCR failing with "Text recognition failed" on macOS 27
- Black flash when the capture overlay opens
- Crosshair cursor not showing until the first click on macOS

## 0.1.0

- First release: area, window and screen capture, annotation, OCR, auto-update
