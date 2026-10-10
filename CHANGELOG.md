# Changelog

The section for each version becomes its GitHub release notes and the text of
the in-app "Update available" dialog. Keep entries short and user-facing.

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
