//! Small OS hooks that Tauri and xcap don't cover.

pub use imp::*;

#[cfg(target_os = "macos")]
mod imp {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicPtr, Ordering};

    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject};
    use tauri::WebviewWindow;
    use xcap::image::RgbaImage;

    #[repr(C)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    #[repr(C)]
    struct CGRect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
        fn CGEventCreate(source: *const c_void) -> *mut c_void;
        fn CGEventGetLocation(event: *const c_void) -> CGPoint;
        fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
        fn CGWindowListCreateImage(
            rect: CGRect,
            option: u32,
            window: u32,
            image_option: u32,
        ) -> *const c_void;
        fn CGImageGetWidth(image: *const c_void) -> usize;
        fn CGImageGetHeight(image: *const c_void) -> usize;
        fn CGImageGetBytesPerRow(image: *const c_void) -> usize;
        fn CGImageGetBitsPerPixel(image: *const c_void) -> usize;
        fn CGImageGetDataProvider(image: *const c_void) -> *const c_void;
        fn CGDataProviderCopyData(provider: *const c_void) -> *const c_void;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(cf: *const c_void);
        fn CFDataGetBytePtr(data: *const c_void) -> *const u8;
        fn CFDataGetLength(data: *const c_void) -> isize;
    }

    /// Readies a live-selection overlay for `capture_below`; returns its window number.
    pub fn prepare_live_overlay(win: &WebviewWindow) -> u32 {
        let Ok(ptr) = win.ns_window() else { return 0 };
        let number: isize = unsafe { msg_send![&*(ptr as *const AnyObject), windowNumber] };
        number as u32
    }

    /// What is on screen in `(x, y, w, h)` (global points) below `window`, so
    /// the overlay itself is left out.
    pub fn capture_below(window: u32, (x, y, w, h): (f64, f64, f64, f64)) -> Option<RgbaImage> {
        const ON_SCREEN_BELOW_WINDOW: u32 = 1 << 2;
        if window == 0 {
            return None;
        }
        unsafe {
            let image =
                CGWindowListCreateImage(CGRect { x, y, w, h }, ON_SCREEN_BELOW_WINDOW, window, 0);
            if image.is_null() {
                return None;
            }
            let result = (|| {
                if CGImageGetBitsPerPixel(image) != 32 {
                    return None;
                }
                let (width, height) = (CGImageGetWidth(image), CGImageGetHeight(image));
                let row = CGImageGetBytesPerRow(image);
                let data = CGDataProviderCopyData(CGImageGetDataProvider(image));
                if data.is_null() {
                    return None;
                }
                let bytes = std::slice::from_raw_parts(
                    CFDataGetBytePtr(data),
                    CFDataGetLength(data) as usize,
                );
                let mut rgba = Vec::with_capacity(width * height * 4);
                for line in bytes.chunks_exact(row).take(height) {
                    for bgra in line[..width * 4].as_chunks::<4>().0 {
                        rgba.extend_from_slice(&[bgra[2], bgra[1], bgra[0], 255]);
                    }
                }
                CFRelease(data);
                RgbaImage::from_raw(width as u32, height as u32, rgba)
            })();
            CFRelease(image);
            result
        }
    }

    /// Cursor position in global points (xcap's macOS coordinate space).
    pub fn cursor_pos() -> (i32, i32) {
        unsafe {
            let event = CGEventCreate(std::ptr::null());
            if event.is_null() {
                return (0, 0);
            }
            let p = CGEventGetLocation(event);
            CFRelease(event);
            (p.x as i32, p.y as i32)
        }
    }

    pub fn has_screen_permission() -> bool {
        unsafe { CGPreflightScreenCaptureAccess() }
    }

    /// Seconds since the last scroll-wheel or trackpad scroll event, momentum included.
    pub fn secs_since_scroll() -> Option<f64> {
        const COMBINED_SESSION_STATE: i32 = 0;
        const SCROLL_WHEEL: u32 = 22;
        Some(unsafe {
            CGEventSourceSecondsSinceLastEventType(COMBINED_SESSION_STATE, SCROLL_WHEEL)
        })
    }

    pub fn request_screen_permission() {
        unsafe { CGRequestScreenCaptureAccess() };
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
            .spawn();
    }

    /// Lifts the overlay above the menu bar and Dock, and onto every Space.
    pub fn raise_overlay(win: &WebviewWindow) {
        let win = win.clone();
        let _ = win.clone().run_on_main_thread(move || {
            let Ok(ptr) = win.ns_window() else { return };
            let ns_window = unsafe { &*(ptr as *const AnyObject) };
            const SCREEN_SAVER_LEVEL: isize = 1000;
            // canJoinAllSpaces | stationary | ignoresCycle | fullScreenAuxiliary
            const BEHAVIOR: usize = 1 << 0 | 1 << 4 | 1 << 6 | 1 << 8;
            unsafe {
                let _: () = msg_send![ns_window, setLevel: SCREEN_SAVER_LEVEL];
                let _: () = msg_send![ns_window, setCollectionBehavior: BEHAVIOR];
            }
        });
    }

    /// Makes KlikSnap the active app and shows the crosshair right away. The
    /// focus tao gives the overlay uses `activateIgnoringOtherApps:`, which
    /// macOS 14+ may ignore; until the app is active the webview's CSS cursor
    /// doesn't apply and the pointer stays an arrow.
    pub fn activate_with_crosshair() {
        unsafe {
            let Some(app_class) = AnyClass::get(c"NSApplication") else {
                return;
            };
            let app: *mut AnyObject = msg_send![app_class, sharedApplication];
            let can_activate: bool = msg_send![app, respondsToSelector: objc2::sel!(activate)];
            if can_activate {
                let _: () = msg_send![app, activate];
            }
        }
        set_crosshair();
    }

    /// Sets the crosshair once KlikSnap is the active app. Before that, the
    /// app being left still owns the cursor and resets it while the mouse
    /// moves, and WebKit won't set it again until the CSS cursor changes.
    /// Returns whether it was set.
    pub fn set_crosshair_if_active() -> bool {
        let active: bool = unsafe {
            let Some(app_class) = AnyClass::get(c"NSApplication") else {
                return false;
            };
            let app: *mut AnyObject = msg_send![app_class, sharedApplication];
            msg_send![app, isActive]
        };
        if active {
            set_crosshair();
        }
        active
    }

    fn set_crosshair() {
        unsafe {
            if let Some(cursor_class) = AnyClass::get(c"NSCursor") {
                let cursor: *mut AnyObject = msg_send![cursor_class, crosshairCursor];
                let _: () = msg_send![cursor, set];
            }
        }
    }

    /// Shows a window without stealing focus from the app the user is in,
    /// and keeps it in the corner of every Space, full-screen apps included,
    /// so switching desktops doesn't leave it behind.
    pub fn show_inactive(win: &WebviewWindow) {
        let win = win.clone();
        let _ = win.clone().run_on_main_thread(move || {
            let Ok(ptr) = win.ns_window() else { return };
            let ns_window = unsafe { &*(ptr as *const AnyObject) };
            const STATUS_LEVEL: isize = 25;
            // canJoinAllSpaces | stationary | ignoresCycle | fullScreenAuxiliary
            const BEHAVIOR: usize = 1 << 0 | 1 << 4 | 1 << 6 | 1 << 8;
            unsafe {
                let _: () = msg_send![ns_window, setLevel: STATUS_LEVEL];
                let _: () = msg_send![ns_window, setCollectionBehavior: BEHAVIOR];
                let _: () = msg_send![ns_window, orderFrontRegardless];
            }
        });
    }

    pub fn set_print_screen_opens_snipping(_enabled: bool) -> Result<(), String> {
        Ok(())
    }

    static FRONTMOST: AtomicPtr<AnyObject> = AtomicPtr::new(std::ptr::null_mut());

    /// Remembers the active app so focus can go back to it after the overlay
    /// closes; otherwise ⌘V would land in KlikSnap instead of the user's app.
    pub fn remember_frontmost() {
        let app = unsafe {
            let Some(cls) = AnyClass::get(c"NSWorkspace") else {
                return;
            };
            let workspace: *mut AnyObject = msg_send![cls, sharedWorkspace];
            let app: *mut AnyObject = msg_send![workspace, frontmostApplication];
            Retained::retain(app)
        };
        let app = app.filter(|app| {
            let pid: i32 = unsafe { msg_send![&**app, processIdentifier] };
            pid as u32 != std::process::id()
        });
        let new = app.map_or(std::ptr::null_mut(), Retained::into_raw);
        let old = FRONTMOST.swap(new, Ordering::SeqCst);
        if !old.is_null() {
            drop(unsafe { Retained::from_raw(old) });
        }
    }

    pub fn restore_frontmost() {
        let ptr = FRONTMOST.swap(std::ptr::null_mut(), Ordering::SeqCst);
        if let Some(app) = unsafe { Retained::from_raw(ptr) } {
            const ACTIVATE_IGNORING_OTHER_APPS: usize = 1 << 1;
            let _: bool =
                unsafe { msg_send![&*app, activateWithOptions: ACTIVATE_IGNORING_OTHER_APPS] };
        }
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use tauri::WebviewWindow;

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetCursorPos(point: *mut Point) -> i32;
        fn SetWindowDisplayAffinity(hwnd: *mut std::ffi::c_void, affinity: u32) -> i32;
    }

    /// Keeps a live-selection overlay out of screen captures (Windows 10
    /// 2004+), so the loupe sees what's below it. There is no window number.
    pub fn prepare_live_overlay(win: &WebviewWindow) -> u32 {
        const WDA_EXCLUDEFROMCAPTURE: u32 = 0x11;
        if let Ok(hwnd) = win.hwnd() {
            unsafe { SetWindowDisplayAffinity(hwnd.0, WDA_EXCLUDEFROMCAPTURE) };
        }
        0
    }

    /// Cursor position in physical pixels (xcap's Windows coordinate space).
    pub fn cursor_pos() -> (i32, i32) {
        let mut p = Point { x: 0, y: 0 };
        unsafe { GetCursorPos(&mut p) };
        (p.x, p.y)
    }

    pub fn has_screen_permission() -> bool {
        true
    }
    pub fn request_screen_permission() {}
    pub fn raise_overlay(_win: &WebviewWindow) {}
    pub fn activate_with_crosshair() {}
    pub fn set_crosshair_if_active() -> bool {
        true
    }
    /// Turns Windows 11's "Use the Print screen key to open screen capture"
    /// on or off, so the key is free for KlikSnap.
    pub fn set_print_screen_opens_snipping(enabled: bool) -> Result<(), String> {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let status = std::process::Command::new("reg")
            .args([
                "add",
                r"HKCU\Control Panel\Keyboard",
                "/v",
                "PrintScreenKeyForSnippingEnabled",
                "/t",
                "REG_DWORD",
                "/d",
                if enabled { "1" } else { "0" },
                "/f",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("couldn't change the Print Screen setting".into())
        }
    }
    pub fn show_inactive(win: &WebviewWindow) {
        let _ = win.show();
    }
    pub fn remember_frontmost() {}
    pub fn restore_frontmost() {}
    pub fn secs_since_scroll() -> Option<f64> {
        None
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod imp {
    use tauri::WebviewWindow;

    pub fn cursor_pos() -> (i32, i32) {
        (0, 0)
    }
    pub fn prepare_live_overlay(_win: &WebviewWindow) -> u32 {
        0
    }
    pub fn has_screen_permission() -> bool {
        true
    }
    pub fn request_screen_permission() {}
    pub fn raise_overlay(_win: &WebviewWindow) {}
    pub fn activate_with_crosshair() {}
    pub fn set_crosshair_if_active() -> bool {
        true
    }
    pub fn set_print_screen_opens_snipping(_enabled: bool) -> Result<(), String> {
        Ok(())
    }
    pub fn show_inactive(win: &WebviewWindow) {
        let _ = win.show();
    }
    pub fn remember_frontmost() {}
    pub fn restore_frontmost() {}
    pub fn secs_since_scroll() -> Option<f64> {
        None
    }
}
