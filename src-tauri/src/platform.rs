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

    #[repr(C)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
        fn CGEventCreate(source: *const c_void) -> *mut c_void;
        fn CGEventGetLocation(event: *const c_void) -> CGPoint;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(cf: *const c_void);
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

    /// Shows a window without stealing focus from the app the user is in.
    pub fn show_inactive(win: &WebviewWindow) {
        let win = win.clone();
        let _ = win.clone().run_on_main_thread(move || {
            let Ok(ptr) = win.ns_window() else { return };
            let ns_window = unsafe { &*(ptr as *const AnyObject) };
            let _: () = unsafe { msg_send![ns_window, orderFrontRegardless] };
        });
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
    pub fn show_inactive(win: &WebviewWindow) {
        let _ = win.show();
    }
    pub fn remember_frontmost() {}
    pub fn restore_frontmost() {}
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod imp {
    use tauri::WebviewWindow;

    pub fn cursor_pos() -> (i32, i32) {
        (0, 0)
    }
    pub fn has_screen_permission() -> bool {
        true
    }
    pub fn request_screen_permission() {}
    pub fn raise_overlay(_win: &WebviewWindow) {}
    pub fn show_inactive(win: &WebviewWindow) {
        let _ = win.show();
    }
    pub fn remember_frontmost() {}
    pub fn restore_frontmost() {}
}
