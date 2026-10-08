# Changelog

The section for each version becomes its GitHub release notes and the text of
the in-app "Update available" dialog. Keep entries short and user-facing.

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
