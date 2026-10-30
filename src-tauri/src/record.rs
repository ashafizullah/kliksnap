//! Screen recording to MP4 with the encoders built into the OS:
//! ScreenCaptureKit + AVAssetWriter on macOS, Windows Graphics Capture +
//! Media Foundation on Windows. A `.gif` path records an animated GIF
//! instead, from the same frames. MP4s can carry the system's sound and the
//! microphone. Linux has no such encoder to rely on: there it runs ffmpeg,
//! which can grab X11 but not yet Wayland.

use std::path::Path;
use std::sync::Arc;

use crate::capture::Bounds;
use crate::zoom::Zoom;

pub use imp::Recorder;

/// Frames per second recorded.
pub const FPS: u32 = 30;

/// Which sound an MP4 recording carries.
#[derive(Clone, Copy, Debug, Default)]
pub struct Audio {
    /// What the computer plays.
    pub system: bool,
    pub mic: bool,
}

/// The start error when macOS refused screen capture (Screen Recording off,
/// or granted to an older build of KlikSnap).
pub const DECLINED: &str = "screen recording isn't allowed";

/// Why a recording runs without the sound asked for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SoundWarning {
    /// The microphone isn't allowed (macOS privacy settings).
    MicDenied,
    /// The sound couldn't be captured; the video is recorded without it.
    NoSound,
}

/// Starts recording `rect` (fractions of monitor `b`) into an MP4 at `path`,
/// or a GIF if the path ends in `.gif`, at `percent` of the screen's
/// resolution, with the mouse pointer drawn in if `cursor`. KlikSnap's own
/// windows are left out, except those in `keep` (macOS window numbers;
/// Windows and Linux record every window anyway). Frames follow `zoom`'s
/// view (not on Linux). Sound that can't be captured is left out rather
/// than failing the recording; the warning says so.
#[allow(clippy::too_many_arguments)]
pub fn start(
    b: &Bounds,
    rect: [f64; 4],
    percent: u32,
    path: &Path,
    audio: Audio,
    cursor: bool,
    keep: &[u32],
    zoom: Option<Arc<Zoom>>,
) -> Result<(Recorder, Option<SoundWarning>), String> {
    let mut r = region(b, rect, percent);
    let audio = if is_gif(path) {
        let (w, h) = crate::gif_writer::fit(r.px_w, r.px_h);
        r.scaled |= (w, h) != (r.px_w, r.px_h);
        (r.px_w, r.px_h) = (w, h);
        Audio::default()
    } else {
        audio
    };
    let mut warning = None;
    let mut audio = audio;
    if audio.mic && !imp::microphone_allowed() {
        audio.mic = false;
        warning = Some(SoundWarning::MicDenied);
    }
    match imp::Recorder::start(b, r, path, audio, cursor, keep, zoom.clone()) {
        Ok(rec) => Ok((rec, warning)),
        Err(e) if e != DECLINED && (audio.system || audio.mic) => {
            eprintln!("recording with sound: {e}");
            let _ = std::fs::remove_file(path);
            let rec =
                imp::Recorder::start(b, r, path, Audio::default(), cursor, keep, zoom.clone())?;
            Ok((rec, Some(warning.unwrap_or(SoundWarning::NoSound))))
        }
        // macOS also declines the whole stream when only the microphone is
        // refused: try once more without sound before blaming the screen.
        Err(e) if audio.system || audio.mic => {
            let _ = std::fs::remove_file(path);
            match imp::Recorder::start(b, r, path, Audio::default(), cursor, keep, zoom.clone()) {
                Ok(rec) => {
                    let w = if audio.mic {
                        SoundWarning::MicDenied
                    } else {
                        SoundWarning::NoSound
                    };
                    Ok((rec, Some(w)))
                }
                Err(_) => Err(e),
            }
        }
        Err(e) => Err(e),
    }
}

pub fn is_gif(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("gif"))
}

/// A region of a monitor: `x, y, w, h` in its points (macOS) or pixels
/// (Windows), relative to its top-left corner, and the output size in
/// pixels, even as H.264 requires. `scaled` when the output is smaller
/// than the screen's pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Region {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub px_w: u32,
    pub px_h: u32,
    pub scaled: bool,
}

pub fn region(b: &Bounds, [fx, fy, fw, fh]: [f64; 4], percent: u32) -> Region {
    let (bw, bh) = (b.w as f64, b.h as f64);
    let x = (fx * bw).round().clamp(0.0, bw);
    let y = (fy * bh).round().clamp(0.0, bh);
    let w = (fw * bw).round().clamp(2.0, bw - x);
    let h = (fh * bh).round().clamp(2.0, bh - y);
    // xcap space is points on macOS, pixels elsewhere.
    let k = percent.clamp(10, 100) as f64 / 100.0;
    let px = if cfg!(target_os = "macos") {
        b.scale
    } else {
        1.0
    } * k;
    let even = |v: f64| (((v * px).round() as u32) & !1).max(2);
    Region {
        x,
        y,
        w,
        h,
        px_w: even(w),
        px_h: even(h),
        scaled: k < 1.0,
    }
}

/// Scales BGRA `src` (`sw`×`sh`, `stride` bytes per row) to `dw`×`dh` with
/// bilinear filtering, bottom row first as the Windows encoder wants it.
#[cfg(any(target_os = "windows", test))]
pub fn resample_flipped(
    src: &[u8],
    stride: usize,
    sw: usize,
    sh: usize,
    dw: usize,
    dh: usize,
) -> Vec<u8> {
    let mut out = vec![0u8; dw * dh * 4];
    if sw == 0 || sh == 0 {
        return out;
    }
    // 16.16 fixed point, sampling at pixel centers.
    let step_x = ((sw as u64) << 16) / dw as u64;
    let step_y = ((sh as u64) << 16) / dh as u64;
    let cols: Vec<(usize, usize, u32)> = (0..dw)
        .map(|x| {
            let fx = ((x as u64 * step_x) + step_x / 2).saturating_sub(1 << 15);
            let x0 = ((fx >> 16) as usize).min(sw - 1);
            ((x0 * 4), ((x0 + 1).min(sw - 1) * 4), ((fx & 0xffff) as u32))
        })
        .collect();
    for y in 0..dh {
        let fy = ((y as u64 * step_y) + step_y / 2).saturating_sub(1 << 15);
        let y0 = ((fy >> 16) as usize).min(sh - 1);
        let y1 = (y0 + 1).min(sh - 1);
        let wy = (fy & 0xffff) as u32;
        let (r0, r1) = (&src[y0 * stride..][..sw * 4], &src[y1 * stride..][..sw * 4]);
        let dst = &mut out[(dh - 1 - y) * dw * 4..][..dw * 4];
        for (x, &(a, b, wx)) in cols.iter().enumerate() {
            for c in 0..4 {
                let top = r0[a + c] as u32 * (0x10000 - wx) + r0[b + c] as u32 * wx;
                let bot = r1[a + c] as u32 * (0x10000 - wx) + r1[b + c] as u32 * wx;
                let v = ((top >> 8) as u64 * (0x10000 - wy) as u64 + (bot >> 8) as u64 * wy as u64)
                    >> 24;
                dst[x * 4 + c] = v as u8;
            }
        }
    }
    out
}

#[cfg(target_os = "macos")]
mod imp {
    use std::ffi::c_void;
    use std::path::Path;
    use std::ptr::NonNull;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{mpsc, Arc, Mutex};
    use std::thread::JoinHandle;
    use std::time::Duration;

    use block2::RcBlock;
    use dispatch2::DispatchQueue;
    use objc2::rc::Allocated;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject, Bool, ProtocolObject, Sel};
    use objc2::{define_class, msg_send, sel, AllocAnyThread, DefinedClass};
    use objc2_av_foundation::{
        AVAssetWriter, AVAssetWriterInput, AVAssetWriterInputPixelBufferAdaptor,
        AVAssetWriterStatus, AVFileTypeMPEG4, AVMediaTypeAudio, AVMediaTypeVideo, AVVideoCodecKey,
        AVVideoCodecTypeH264, AVVideoHeightKey, AVVideoWidthKey,
    };
    use objc2_core_foundation::CFRetained;
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_core_media::{CMClock, CMSampleBuffer, CMTime};
    use objc2_core_video::{
        CVPixelBuffer, CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow,
        CVPixelBufferGetHeight, CVPixelBufferGetWidth, CVPixelBufferLockBaseAddress,
        CVPixelBufferLockFlags, CVPixelBufferPool, CVPixelBufferUnlockBaseAddress,
    };
    use objc2_foundation::{
        NSArray, NSDictionary, NSError, NSNumber, NSObject, NSObjectProtocol, NSString, NSURL,
    };
    use objc2_screen_capture_kit::{
        SCContentFilter, SCShareableContent, SCStream, SCStreamConfiguration, SCStreamOutput,
        SCStreamOutputType,
    };

    use super::{Region, FPS};
    use crate::capture::Bounds;
    use crate::gif_writer::GifWriter;
    use crate::zoom::{View, Zoom};

    const WAIT: Duration = Duration::from_secs(10);

    pub enum Writer {
        Mp4(Mp4),
        Gif(Mutex<Option<GifWriter>>),
    }

    pub struct Mp4 {
        writer: Retained<AVAssetWriter>,
        input: Retained<AVAssetWriterInput>,
        /// Takes the frames: the captured ones, and zoomed ones from its pool.
        adaptor: Retained<AVAssetWriterInputPixelBufferAdaptor>,
        /// One audio track each, for the system's sound and the microphone.
        system: Option<Retained<AVAssetWriterInput>>,
        mic: Option<Retained<AVAssetWriterInput>>,
        started: AtomicBool,
        /// The newest frame, appended again when recording stops.
        last: Mutex<Option<CFRetained<CVPixelBuffer>>>,
    }

    pub struct Ivars {
        writer: Writer,
        zoom: Option<Arc<Zoom>>,
        frames: Mutex<Frames>,
    }

    struct Frames {
        /// The newest frame captured, zoomed again while the screen is still.
        source: Option<CFRetained<CVPixelBuffer>>,
        /// When the newest frame was written, and with which view.
        written: Option<Duration>,
        shown: View,
    }

    fn seconds(t: CMTime) -> Duration {
        if t.timescale <= 0 || t.value < 0 {
            return Duration::ZERO;
        }
        Duration::from_secs_f64(t.value as f64 / t.timescale as f64)
    }

    /// Runs `f` on the frame's pixels: base address, bytes per row, width
    /// and height.
    unsafe fn with_pixels<R>(
        image: &CVPixelBuffer,
        flags: CVPixelBufferLockFlags,
        f: impl FnOnce(*mut u8, usize, usize, usize) -> R,
    ) -> Option<R> {
        unsafe {
            if CVPixelBufferLockBaseAddress(image, flags) != 0 {
                return None;
            }
            let base = CVPixelBufferGetBaseAddress(image) as *mut u8;
            let r = (!base.is_null()).then(|| {
                f(
                    base,
                    CVPixelBufferGetBytesPerRow(image),
                    CVPixelBufferGetWidth(image),
                    CVPixelBufferGetHeight(image),
                )
            });
            CVPixelBufferUnlockBaseAddress(image, flags);
            r
        }
    }

    #[repr(C)]
    struct VImageBuffer {
        data: *mut c_void,
        height: usize,
        width: usize,
        row_bytes: usize,
    }

    #[link(name = "Accelerate", kind = "framework")]
    extern "C" {
        fn vImageScale_ARGB8888(
            src: *const VImageBuffer,
            dest: *const VImageBuffer,
            temp: *mut c_void,
            flags: u32,
        ) -> isize;
    }

    /// Scales the part of `src` that `view` shows to fill `dw`×`dh` BGRA
    /// pixels at `dst`.
    unsafe fn zoom_into(
        src: &CVPixelBuffer,
        view: View,
        dst: *mut u8,
        dst_stride: usize,
        dw: usize,
        dh: usize,
    ) -> bool {
        unsafe {
            with_pixels(
                src,
                CVPixelBufferLockFlags::ReadOnly,
                |base, stride, w, h| {
                    let (x, y, cw, ch) = view.crop(w, h);
                    let from = VImageBuffer {
                        data: base.add(y * stride + x * 4) as *mut c_void,
                        height: ch,
                        width: cw,
                        row_bytes: stride,
                    };
                    let to = VImageBuffer {
                        data: dst as *mut c_void,
                        height: dh,
                        width: dw,
                        row_bytes: dst_stride,
                    };
                    vImageScale_ARGB8888(&from, &to, std::ptr::null_mut(), 0) == 0
                },
            )
            .unwrap_or(false)
        }
    }

    /// The part of `src` that `view` shows, as a full-size frame from `pool`.
    unsafe fn zoomed(
        src: &CVPixelBuffer,
        view: View,
        pool: &CVPixelBufferPool,
    ) -> Option<CFRetained<CVPixelBuffer>> {
        unsafe {
            let mut out: *mut CVPixelBuffer = std::ptr::null_mut();
            if CVPixelBufferPool::create_pixel_buffer(None, pool, NonNull::from(&mut out)) != 0 {
                return None;
            }
            let out = CFRetained::from_raw(NonNull::new(out)?);
            let ok = with_pixels(
                &out,
                CVPixelBufferLockFlags::empty(),
                |base, stride, w, h| zoom_into(src, view, base, stride, w, h),
            )?;
            // The color space, so zoomed frames match the others.
            src.propagate_attachments(&out);
            ok.then_some(out)
        }
    }

    /// Hands the frame's pixels to the GIF writer.
    unsafe fn push_gif(
        gif: &Mutex<Option<GifWriter>>,
        image: &CVPixelBuffer,
        view: View,
        t: Duration,
    ) {
        let mut gif = gif.lock().unwrap();
        let Some(g) = gif.as_mut() else { return };
        unsafe {
            with_pixels(
                image,
                CVPixelBufferLockFlags::ReadOnly,
                |base, stride, w, h| {
                    if view.is_full() {
                        g.push(
                            std::slice::from_raw_parts(base, stride * h),
                            stride,
                            false,
                            t,
                        );
                        return;
                    }
                    let mut px = vec![0u8; w * h * 4];
                    if zoom_into(image, view, px.as_mut_ptr(), w * 4, w, h) {
                        g.push(&px, w * 4, false, t);
                    }
                },
            );
        }
    }

    impl Ivars {
        /// Writes a frame taken at `t`, zoomed to the view.
        unsafe fn write(&self, image: &CVPixelBuffer, t: CMTime, frames: &mut Frames) {
            let view = self.zoom.as_ref().map_or(View::FULL, |z| z.view());
            let at = seconds(t);
            // A frame older than the last one written fails the whole file.
            if frames.written.is_some_and(|w| at <= w) {
                return;
            }
            frames.written = Some(at);
            frames.shown = view;
            let w = match &self.writer {
                Writer::Mp4(w) => w,
                Writer::Gif(g) => return unsafe { push_gif(g, image, view, at) },
            };
            unsafe {
                let zoomed = match view.is_full() {
                    true => None,
                    false => match w.adaptor.pixelBufferPool() {
                        Some(pool) => match zoomed(image, view, &pool) {
                            Some(z) => Some(z),
                            None => return,
                        },
                        None => return,
                    },
                };
                let frame = zoomed.as_deref().unwrap_or(image);
                if !w.started.swap(true, Ordering::SeqCst) {
                    w.writer.startSessionAtSourceTime(t);
                }
                if w.input.isReadyForMoreMediaData()
                    && w.adaptor.appendPixelBuffer_withPresentationTime(frame, t)
                {
                    *w.last.lock().unwrap() = Some(CFRetained::retain(NonNull::from(frame)));
                }
            }
        }

        /// While zoomed, writes the newest frame again if the screen hasn't
        /// sent one lately, so the zoom moves on a still screen too.
        unsafe fn tick(&self) {
            let Some(zoom) = &self.zoom else { return };
            let mut frames = self.frames.lock().unwrap();
            if zoom.view().is_full() && frames.shown.is_full() {
                return;
            }
            let now = CMClock::host_time_clock().time();
            let due = Duration::from_millis(800 / FPS as u64);
            if frames
                .written
                .is_some_and(|w| seconds(now).saturating_sub(w) < due)
            {
                return;
            }
            if let Some(source) = frames.source.clone() {
                unsafe { self.write(&source, now, &mut frames) };
            }
        }
    }

    define_class!(
        // Receives the frames on the recording's serial queue.
        #[unsafe(super(NSObject))]
        #[name = "KlikSnapRecordOutput"]
        #[ivars = Ivars]
        struct Output;

        unsafe impl NSObjectProtocol for Output {}

        unsafe impl SCStreamOutput for Output {
            #[unsafe(method(stream:didOutputSampleBuffer:ofType:))]
            unsafe fn stream_did_output(
                &self,
                _stream: &SCStream,
                buffer: &CMSampleBuffer,
                kind: SCStreamOutputType,
            ) {
                if kind != SCStreamOutputType::Screen {
                    if let Writer::Mp4(w) = &self.ivars().writer {
                        let input = if kind == SCStreamOutputType::Audio {
                            w.system.as_ref()
                        } else if kind == SCStreamOutputType::Microphone {
                            w.mic.as_ref()
                        } else {
                            None
                        };
                        // Sound before the first frame would precede the session.
                        if let Some(input) = input.filter(|_| w.started.load(Ordering::SeqCst)) {
                            unsafe {
                                if buffer.is_valid() && input.isReadyForMoreMediaData() {
                                    input.appendSampleBuffer(buffer);
                                }
                            }
                        }
                    }
                    return;
                }
                unsafe {
                    // Frames where nothing changed carry no image.
                    if !buffer.is_valid() {
                        return;
                    }
                    let Some(image) = buffer.image_buffer() else {
                        return;
                    };
                    let ivars = self.ivars();
                    let mut frames = ivars.frames.lock().unwrap();
                    ivars.write(&image, buffer.presentation_time_stamp(), &mut frames);
                    if ivars.zoom.is_some() {
                        frames.source = Some(image);
                    }
                }
            }
        }
    );

    #[link(name = "CoreVideo", kind = "framework")]
    extern "C" {
        fn CVPixelBufferGetIOSurface(buffer: &CVPixelBuffer) -> *mut AnyObject;
    }

    // CALayer and CATransaction.
    #[link(name = "QuartzCore", kind = "framework")]
    extern "C" {}

    pub struct LiveIvars {
        layer: Retained<AnyObject>,
        /// The frame on screen, kept until the next one replaces it.
        current: Mutex<Option<CFRetained<CVPixelBuffer>>>,
    }

    define_class!(
        // Puts the live capture's frames on the zoom window's layer.
        #[unsafe(super(NSObject))]
        #[name = "KlikSnapZoomOutput"]
        #[ivars = LiveIvars]
        struct LiveOutput;

        unsafe impl NSObjectProtocol for LiveOutput {}

        unsafe impl SCStreamOutput for LiveOutput {
            #[unsafe(method(stream:didOutputSampleBuffer:ofType:))]
            unsafe fn stream_did_output(
                &self,
                _stream: &SCStream,
                buffer: &CMSampleBuffer,
                kind: SCStreamOutputType,
            ) {
                unsafe {
                    if kind != SCStreamOutputType::Screen || !buffer.is_valid() {
                        return;
                    }
                    let Some(image) = buffer.image_buffer() else {
                        return;
                    };
                    let surface = CVPixelBufferGetIOSurface(&image);
                    if surface.is_null() {
                        return;
                    }
                    let ivars = self.ivars();
                    transaction(|| {
                        let _: () = msg_send![&*ivars.layer, setContents: surface];
                    });
                    *ivars.current.lock().unwrap() = Some(image);
                }
            }
        }
    );

    /// Changes layers at once, without Core Animation's implicit fades; on
    /// any thread.
    fn transaction(f: impl FnOnce()) {
        let Some(cls) = AnyClass::get(c"CATransaction") else {
            return;
        };
        unsafe {
            let _: () = msg_send![cls, begin];
            let _: () = msg_send![cls, setDisableActions: true];
            f();
            let _: () = msg_send![cls, commit];
        }
    }

    /// Runs `f` on the main thread, where AppKit wants windows made; None
    /// when the main thread doesn't get to it soon (or ever, as in tests).
    fn on_main<R: Send + 'static>(f: impl FnOnce() -> R + Send + 'static) -> Option<R> {
        let main = AnyClass::get(c"NSThread").is_some_and(|c| unsafe {
            let main: bool = msg_send![c, isMainThread];
            main
        });
        if main {
            return Some(f());
        }
        let (tx, rx) = mpsc::channel();
        let late = Arc::new(AtomicBool::new(false));
        let skip = late.clone();
        // What comes too late is dropped, which closes a window.
        DispatchQueue::main().exec_async(move || {
            if !skip.load(Ordering::SeqCst) {
                let _ = tx.send(f());
            }
        });
        let out = rx.recv_timeout(Duration::from_secs(2)).ok();
        late.store(true, Ordering::SeqCst);
        out
    }

    /// A window over the region recorded that ignores the mouse, with a
    /// hidden layer filling it; closed when dropped.
    struct Overlay {
        window: Retained<AnyObject>,
        layer: Retained<AnyObject>,
    }

    impl Overlay {
        fn open(b: &Bounds, r: Region) -> Option<Self> {
            let b = *b;
            on_main(move || unsafe { overlay(&b, r) }.map(AnyThread))
                .flatten()
                .map(|o| o.0)
        }
    }

    impl Drop for Overlay {
        fn drop(&mut self) {
            let window = AnyThread(self.window.clone());
            DispatchQueue::main().exec_async(move || unsafe {
                let _: () = msg_send![&**window.get(), close];
            });
        }
    }

    /// Opens the overlay for region `r` of monitor `b`. Main thread only.
    unsafe fn overlay(b: &Bounds, r: Region) -> Option<Overlay> {
        let window_cls = AnyClass::get(c"NSWindow")?;
        let screen_cls = AnyClass::get(c"NSScreen")?;
        let color_cls = AnyClass::get(c"NSColor")?;
        let layer_cls = AnyClass::get(c"CALayer")?;
        unsafe {
            // AppKit counts y up from the bottom of the main screen.
            let screens: Option<Retained<AnyObject>> = msg_send![screen_cls, screens];
            let main: Option<Retained<AnyObject>> = msg_send![&*screens?, firstObject];
            let main: CGRect = msg_send![&*main?, frame];
            let frame = CGRect::new(
                CGPoint::new(
                    b.x as f64 + r.x,
                    main.size.height - (b.y as f64 + r.y + r.h),
                ),
                CGSize::new(r.w, r.h),
            );
            const BORDERLESS: usize = 0;
            const BUFFERED: usize = 2;
            let window: Allocated<AnyObject> = msg_send![window_cls, alloc];
            let window: Option<Retained<AnyObject>> = msg_send![
                window,
                initWithContentRect: frame,
                styleMask: BORDERLESS,
                backing: BUFFERED,
                defer: false
            ];
            let window = window?;
            let clear: Option<Retained<AnyObject>> = msg_send![color_cls, clearColor];
            let _: () = msg_send![&*window, setReleasedWhenClosed: false];
            let _: () = msg_send![&*window, setOpaque: false];
            let _: () = msg_send![&*window, setBackgroundColor: clear.as_deref()];
            let _: () = msg_send![&*window, setHasShadow: false];
            let _: () = msg_send![&*window, setIgnoresMouseEvents: true];
            // Above the menu bar and the Dock, below menus.
            let _: () = msg_send![&*window, setLevel: 100isize];
            // On every space, full-screen apps' too, and out of Cmd-`.
            const BEHAVIOR: usize = 1 | 16 | 64 | 256;
            let _: () = msg_send![&*window, setCollectionBehavior: BEHAVIOR];
            let view: Option<Retained<AnyObject>> = msg_send![&*window, contentView];
            let view = view?;
            let _: () = msg_send![&*view, setWantsLayer: true];
            let root: Option<Retained<AnyObject>> = msg_send![&*view, layer];
            let layer: Retained<AnyObject> = msg_send![layer_cls, new];
            let bounds: CGRect = msg_send![&*view, bounds];
            const FILL: u32 = 2 | 16;
            let _: () = msg_send![&*layer, setFrame: bounds];
            let _: () = msg_send![&*layer, setAutoresizingMask: FILL];
            let _: () = msg_send![&*layer, setHidden: true];
            let _: () = msg_send![&*root?, addSublayer: &*layer];
            let _: () = msg_send![&*window, orderFrontRegardless];
            Some(Overlay { window, layer })
        }
    }

    /// Shows the zoom live: a window over the region showing a second capture
    /// of it cropped to the view. Like KlikSnap's other windows it is left
    /// out of both captures, and that capture leaves the pointer out, as
    /// macOS draws the real one on top.
    struct Live {
        stream: Retained<SCStream>,
        _output: Retained<LiveOutput>,
        _queue: dispatch2::DispatchRetained<DispatchQueue>,
        _overlay: Overlay,
        /// Moves the layer with the view, and its stop flag.
        thread: JoinHandle<()>,
        stop: Arc<AtomicBool>,
    }

    impl Live {
        unsafe fn start(
            b: &Bounds,
            r: Region,
            filter: &SCContentFilter,
            zoom: Arc<Zoom>,
            overlay: Overlay,
        ) -> Result<Self, String> {
            let even = |v: f64| (((v * b.scale).round() as usize) & !1).max(2);
            let (stream, output, queue) = unsafe {
                let config = SCStreamConfiguration::new();
                config.setWidth(even(r.w));
                config.setHeight(even(r.h));
                config.setSourceRect(CGRect::new(CGPoint::new(r.x, r.y), CGSize::new(r.w, r.h)));
                config.setMinimumFrameInterval(CMTime::new(1, 60));
                config.setShowsCursor(false);
                config.setPixelFormat(u32::from_be_bytes(*b"BGRA"));
                config.setQueueDepth(4);
                let output = LiveOutput::alloc().set_ivars(LiveIvars {
                    layer: overlay.layer.clone(),
                    current: Mutex::new(None),
                });
                let output: Retained<LiveOutput> = msg_send![super(output), init];
                let queue = DispatchQueue::new("app.kliksnap.zoom", None);
                let stream = SCStream::initWithFilter_configuration_delegate(
                    SCStream::alloc(),
                    filter,
                    &config,
                    None,
                );
                stream
                    .addStreamOutput_type_sampleHandlerQueue_error(
                        ProtocolObject::from_ref(&*output),
                        SCStreamOutputType::Screen,
                        Some(&queue),
                    )
                    .map_err(|e| e.localizedDescription().to_string())?;
                start_stream(&stream)?;
                (stream, output, queue)
            };
            let stop = Arc::new(AtomicBool::new(false));
            let (flag, layer) = (stop.clone(), AnyThread(overlay.layer.clone()));
            let thread = std::thread::spawn(move || {
                let layer = layer.get();
                let mut shown = View::FULL;
                while !flag.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(8));
                    let view = zoom.view();
                    if view == shown {
                        continue;
                    }
                    shown = view;
                    let [x, y, w, h] = view.rect();
                    // Layers count y up from the bottom.
                    let rect = CGRect::new(CGPoint::new(x, 1.0 - y - h), CGSize::new(w, h));
                    transaction(|| unsafe {
                        let _: () = msg_send![&**layer, setContentsRect: rect];
                        let _: () = msg_send![&**layer, setHidden: view.is_full()];
                    });
                }
            });
            Ok(Self {
                stream,
                _output: output,
                _queue: queue,
                _overlay: overlay,
                thread,
                stop,
            })
        }

        fn stop(self) {
            self.stop.store(true, Ordering::SeqCst);
            let _ = self.thread.join();
            if let Err(e) = stop_stream(&self.stream) {
                eprintln!("stopping the live zoom: {e}");
            }
        }
    }

    fn start_stream(stream: &SCStream) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        let done = RcBlock::new(move |e: *mut NSError| {
            let _ = tx.send(ns_error(e));
        });
        unsafe { stream.startCaptureWithCompletionHandler(Some(&done)) };
        match rx.recv_timeout(WAIT) {
            Ok(None) => Ok(()),
            Ok(Some(e)) => Err(e),
            Err(_) => Err("recording didn't start".into()),
        }
    }

    fn stop_stream(stream: &SCStream) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        let done = RcBlock::new(move |e: *mut NSError| {
            let _ = tx.send(ns_error(e));
        });
        unsafe { stream.stopCaptureWithCompletionHandler(Some(&done)) };
        match rx.recv_timeout(WAIT) {
            Ok(Some(e)) => Err(e),
            _ => Ok(()),
        }
    }

    pub struct Recorder {
        stream: Retained<SCStream>,
        output: Retained<Output>,
        queue: dispatch2::DispatchRetained<DispatchQueue>,
        /// Keeps the zoom moving on a still screen, and its stop flag.
        ticker: Option<(JoinHandle<()>, Arc<AtomicBool>)>,
        live: Option<Live>,
    }

    // The stream and writer are only touched on their queue or after the
    // stream has stopped; ScreenCaptureKit and AVFoundation allow that from
    // any thread.
    unsafe impl Send for Recorder {}

    struct AnyThread<T>(T);
    unsafe impl<T> Send for AnyThread<T> {}

    impl<T> AnyThread<T> {
        // A method, so closures capture the wrapper and not its field.
        fn get(&self) -> &T {
            &self.0
        }
    }

    fn ns_error(e: *mut NSError) -> Option<String> {
        unsafe { e.as_ref() }.map(|e| {
            // SCStreamErrorUserDeclined: macOS refused the capture.
            if e.code() == -3801 {
                super::DECLINED.to_string()
            } else {
                e.localizedDescription().to_string()
            }
        })
    }

    /// Whether KlikSnap may use the microphone, asking the first time.
    /// Blocks until the user answers.
    pub fn microphone_allowed() -> bool {
        use objc2::runtime::AnyClass;
        const NOT_DETERMINED: isize = 0;
        const AUTHORIZED: isize = 3;
        let (Some(class), Some(media)) = (AnyClass::get(c"AVCaptureDevice"), unsafe {
            AVMediaTypeAudio
        }) else {
            return false;
        };
        let status: isize = unsafe { msg_send![class, authorizationStatusForMediaType: media] };
        if status != NOT_DETERMINED {
            return status == AUTHORIZED;
        }
        let (tx, rx) = mpsc::channel();
        let done = RcBlock::new(move |granted: Bool| {
            let _ = tx.send(granted.as_bool());
        });
        let _: () = unsafe {
            msg_send![class, requestAccessForMediaType: media, completionHandler: &*done]
        };
        rx.recv_timeout(Duration::from_secs(120)).unwrap_or(false)
    }

    impl Recorder {
        pub fn start(
            b: &Bounds,
            r: Region,
            path: &Path,
            audio: super::Audio,
            cursor: bool,
            keep: &[u32],
            zoom: Option<Arc<Zoom>>,
        ) -> Result<Self, String> {
            // Before listing what's on screen: macOS only leaves out the
            // windows of apps it lists, and an app without windows isn't.
            let overlay = zoom.as_ref().and_then(|_| Overlay::open(b, r));
            let content = shareable_content()?;
            let display = unsafe { content.displays() }
                .iter()
                .find(|d| {
                    let f = unsafe { d.frame() };
                    (f.origin.x - b.x as f64).abs() < 1.0 && (f.origin.y - b.y as f64).abs() < 1.0
                })
                .ok_or("monitor disconnected")?;
            // Leave KlikSnap's own windows (the recording controls) out.
            let own: Vec<_> = unsafe { content.applications() }
                .iter()
                .filter(|a| unsafe { a.processID() } as u32 == std::process::id())
                .collect();
            let own = NSArray::from_retained_slice(&own);
            let kept: Vec<_> = unsafe { content.windows() }
                .iter()
                .filter(|w| keep.contains(&unsafe { w.windowID() }))
                .collect();
            let filter = unsafe {
                SCContentFilter::initWithDisplay_excludingApplications_exceptingWindows(
                    SCContentFilter::alloc(),
                    &display,
                    &own,
                    &NSArray::from_retained_slice(&kept),
                )
            };
            let config = unsafe { SCStreamConfiguration::new() };
            unsafe {
                config.setWidth(r.px_w as usize);
                config.setHeight(r.px_h as usize);
                config.setSourceRect(CGRect::new(CGPoint::new(r.x, r.y), CGSize::new(r.w, r.h)));
                config.setMinimumFrameInterval(CMTime::new(1, FPS as i32));
                config.setShowsCursor(cursor);
                config.setPixelFormat(u32::from_be_bytes(*b"BGRA"));
                config.setQueueDepth(6);
            }
            let responds = |selector: Sel| -> bool {
                let r: Bool = unsafe { msg_send![&*config, respondsToSelector: selector] };
                r.as_bool()
            };
            if audio.system {
                if !responds(sel!(setCapturesAudio:)) {
                    return Err("recording sound needs macOS 13 or later".into());
                }
                unsafe {
                    config.setCapturesAudio(true);
                    config.setExcludesCurrentProcessAudio(true);
                    config.setSampleRate(48_000);
                    config.setChannelCount(2);
                }
            }
            if audio.mic {
                if !responds(sel!(setCaptureMicrophone:)) {
                    return Err("recording the microphone needs macOS 15 or later".into());
                }
                unsafe { config.setCaptureMicrophone(true) };
            }

            let writer = if super::is_gif(path) {
                Writer::Gif(Mutex::new(Some(GifWriter::new(path, r.px_w, r.px_h)?)))
            } else {
                Writer::Mp4(new_writer(path, r, audio)?)
            };
            let live_zoom = zoom.clone();
            let ticks = zoom.is_some();
            let output = Output::alloc().set_ivars(Ivars {
                writer,
                zoom,
                frames: Mutex::new(Frames {
                    source: None,
                    written: None,
                    shown: View::FULL,
                }),
            });
            let output: Retained<Output> = unsafe { msg_send![super(output), init] };
            let queue = DispatchQueue::new("app.kliksnap.record", None);
            let stream = unsafe {
                SCStream::initWithFilter_configuration_delegate(
                    SCStream::alloc(),
                    &filter,
                    &config,
                    None,
                )
            };
            let mut kinds = vec![SCStreamOutputType::Screen];
            if audio.system {
                kinds.push(SCStreamOutputType::Audio);
            }
            if audio.mic {
                kinds.push(SCStreamOutputType::Microphone);
            }
            for kind in kinds {
                unsafe {
                    stream.addStreamOutput_type_sampleHandlerQueue_error(
                        ProtocolObject::from_ref(&*output),
                        kind,
                        Some(&queue),
                    )
                }
                .map_err(|e| e.localizedDescription().to_string())?;
            }
            start_stream(&stream)?;
            // Without it the zoom is still recorded, just not seen.
            let live = live_zoom.zip(overlay).and_then(|(zoom, overlay)| {
                unsafe { Live::start(b, r, &filter, zoom, overlay) }
                    .map_err(|e| eprintln!("live zoom: {e}"))
                    .ok()
            });
            let ticker = ticks.then(|| {
                let stop = Arc::new(AtomicBool::new(false));
                let (flag, output, queue) = (
                    stop.clone(),
                    AnyThread(output.clone()),
                    AnyThread(queue.clone()),
                );
                let thread = std::thread::spawn(move || {
                    while !flag.load(Ordering::SeqCst) {
                        std::thread::sleep(Duration::from_millis(1000 / FPS as u64));
                        let output = AnyThread(output.get().clone());
                        queue
                            .get()
                            .exec_async(move || unsafe { output.get().ivars().tick() });
                    }
                });
                (thread, stop)
            });
            Ok(Self {
                stream,
                output,
                queue,
                ticker,
                live,
            })
        }

        /// Stops recording and finishes the file.
        pub fn stop(self) -> Result<(), String> {
            if let Some(live) = self.live {
                live.stop();
            }
            if let Some((thread, stop)) = self.ticker {
                stop.store(true, Ordering::SeqCst);
                let _ = thread.join();
            }
            if let Err(e) = stop_stream(&self.stream) {
                eprintln!("stopping the recording: {e}");
            }
            // Finish on the frames' queue, after any frame still in flight.
            let (tx, rx) = mpsc::channel();
            let output = AnyThread(self.output.clone());
            self.queue.exec_async(move || {
                let w = &output.get().ivars().writer;
                let _ = tx.send(unsafe { finish(w) });
            });
            rx.recv_timeout(WAIT)
                .map_err(|_| "saving the recording timed out".to_string())?
        }
    }

    unsafe fn finish(w: &Writer) -> Result<(), String> {
        let w = match w {
            Writer::Mp4(w) => w,
            Writer::Gif(g) => {
                let end = seconds(CMClock::host_time_clock().time());
                return g
                    .lock()
                    .unwrap()
                    .take()
                    .ok_or("already stopped")?
                    .finish(end);
            }
        };
        unsafe {
            if !w.started.load(Ordering::SeqCst) {
                w.writer.cancelWriting();
                return Err("nothing was recorded".into());
            }
            // Frames only come when the screen changes: repeat the last one
            // at the stop time, or a still ending would be cut off.
            let end = CMClock::host_time_clock().time();
            if let Some(last) = w.last.lock().unwrap().take() {
                if w.input.isReadyForMoreMediaData() {
                    w.adaptor.appendPixelBuffer_withPresentationTime(&last, end);
                }
            }
            w.input.markAsFinished();
            for input in [&w.system, &w.mic].into_iter().flatten() {
                input.markAsFinished();
            }
            w.writer.endSessionAtSourceTime(end);
            let (tx, rx) = mpsc::channel();
            let done = RcBlock::new(move || {
                let _ = tx.send(());
            });
            w.writer.finishWritingWithCompletionHandler(&done);
            rx.recv_timeout(WAIT)
                .map_err(|_| "saving the recording timed out")?;
            if w.writer.status() == AVAssetWriterStatus::Completed {
                Ok(())
            } else {
                Err(w
                    .writer
                    .error()
                    .map_or("saving the recording failed".into(), |e| {
                        e.localizedDescription().to_string()
                    }))
            }
        }
    }

    fn shareable_content() -> Result<Retained<SCShareableContent>, String> {
        let (tx, rx) = mpsc::channel();
        let done = RcBlock::new(move |content: *mut SCShareableContent, e: *mut NSError| {
            let content = unsafe { Retained::retain(content) };
            let _ = tx.send(content.ok_or_else(|| {
                ns_error(e).unwrap_or_else(|| "screen recording isn't allowed".into())
            }));
        });
        unsafe { SCShareableContent::getShareableContentWithCompletionHandler(&done) };
        rx.recv_timeout(WAIT)
            .map_err(|_| "screen recording isn't allowed".to_string())?
    }

    /// An AAC track at 48 kHz; the writer converts whatever comes in.
    fn audio_input(channels: u32) -> Result<Retained<AVAssetWriterInput>, String> {
        let media = unsafe { AVMediaTypeAudio }.ok_or("AVFoundation constants missing")?;
        let keys = [
            NSString::from_str("AVFormatIDKey"),
            NSString::from_str("AVSampleRateKey"),
            NSString::from_str("AVNumberOfChannelsKey"),
            NSString::from_str("AVEncoderBitRateKey"),
        ];
        let values = [
            NSNumber::new_u32(u32::from_be_bytes(*b"aac ")),
            NSNumber::new_u32(48_000),
            NSNumber::new_u32(channels),
            NSNumber::new_u32(if channels > 1 { 160_000 } else { 96_000 }),
        ];
        let keys: Vec<&NSString> = keys.iter().map(|k| &**k).collect();
        let values: Vec<&AnyObject> = values.iter().map(|v| &***v as &AnyObject).collect();
        let settings = NSDictionary::from_slices(&keys, &values);
        let input = unsafe {
            AVAssetWriterInput::assetWriterInputWithMediaType_outputSettings(media, Some(&settings))
        };
        unsafe { input.setExpectsMediaDataInRealTime(true) };
        Ok(input)
    }

    fn new_writer(path: &Path, r: Region, audio: super::Audio) -> Result<Mp4, String> {
        let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
        let missing = || "AVFoundation constants missing".to_string();
        unsafe {
            let file_type = AVFileTypeMPEG4.ok_or_else(missing)?;
            let writer = AVAssetWriter::assetWriterWithURL_fileType_error(&url, file_type)
                .map_err(|e| e.localizedDescription().to_string())?;
            let keys = [
                AVVideoCodecKey.ok_or_else(missing)?,
                AVVideoWidthKey.ok_or_else(missing)?,
                AVVideoHeightKey.ok_or_else(missing)?,
            ];
            let codec = AVVideoCodecTypeH264.ok_or_else(missing)?;
            let (width, height) = (NSNumber::new_u32(r.px_w), NSNumber::new_u32(r.px_h));
            let values: [&AnyObject; 3] = [codec, &width, &height];
            let settings = NSDictionary::from_slices(&keys, &values);
            let input = AVAssetWriterInput::assetWriterInputWithMediaType_outputSettings(
                AVMediaTypeVideo.ok_or_else(missing)?,
                Some(&settings),
            );
            input.setExpectsMediaDataInRealTime(true);
            writer.addInput(&input);
            let keys = [
                NSString::from_str("PixelFormatType"),
                NSString::from_str("Width"),
                NSString::from_str("Height"),
            ];
            let values = [
                NSNumber::new_u32(u32::from_be_bytes(*b"BGRA")),
                NSNumber::new_u32(r.px_w),
                NSNumber::new_u32(r.px_h),
            ];
            let keys: Vec<&NSString> = keys.iter().map(|k| &**k).collect();
            let values: Vec<&AnyObject> = values.iter().map(|v| &***v as &AnyObject).collect();
            let adaptor = AVAssetWriterInputPixelBufferAdaptor::assetWriterInputPixelBufferAdaptorWithAssetWriterInput_sourcePixelBufferAttributes(
                &input,
                Some(&NSDictionary::from_slices(&keys, &values)),
            );
            let system = audio.system.then(|| audio_input(2)).transpose()?;
            let mic = audio.mic.then(|| audio_input(1)).transpose()?;
            for a in [&system, &mic].into_iter().flatten() {
                if !writer.canAddInput(a) {
                    return Err("couldn't add a sound track".into());
                }
                writer.addInput(a);
            }
            if !writer.startWriting() {
                return Err(writer
                    .error()
                    .map_or("couldn't start the recording".into(), |e| {
                        e.localizedDescription().to_string()
                    }));
            }
            Ok(Mp4 {
                writer,
                input,
                adaptor,
                system,
                mic,
                started: AtomicBool::new(false),
                last: Mutex::new(None),
            })
        }
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread::JoinHandle;
    use std::time::Duration;

    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTONEAREST};
    use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
    use windows_capture::capture::{CaptureControl, Context, GraphicsCaptureApiHandler};
    use windows_capture::encoder::{
        AudioSettingsBuilder, ContainerSettingsBuilder, VideoEncoder, VideoSettingsBuilder,
        VideoSettingsSubType,
    };
    use windows_capture::frame::Frame;
    use windows_capture::graphics_capture_api::InternalCaptureControl;
    use windows_capture::monitor::Monitor;
    use windows_capture::settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    };

    use super::{Region, FPS};
    use crate::capture::Bounds;
    use crate::gif_writer::GifWriter;
    use crate::zoom::{View, Zoom};

    type Error = Box<dyn std::error::Error + Send + Sync>;

    pub struct Handler {
        region: Region,
        encoder: Option<VideoEncoder>,
        gif: Option<GifWriter>,
        scratch: Vec<u8>,
        /// The newest frame (bottom-up rows), sent again when recording stops.
        last: Option<Vec<u8>>,
        zoom: Option<Arc<Zoom>>,
        /// The newest region captured (top-down rows), its width and height,
        /// zoomed again while the screen is still.
        source: Option<(Vec<u8>, usize, usize)>,
        /// When the newest frame was written (100 ns units), and with which view.
        written: Option<i64>,
        shown: View,
    }

    impl Handler {
        fn new_with(region: Region, zoom: Option<Arc<Zoom>>) -> Self {
            Self {
                region,
                encoder: None,
                gif: None,
                scratch: Vec::new(),
                last: None,
                zoom,
                source: None,
                written: None,
                shown: View::FULL,
            }
        }

        /// Writes the region's `sw`×`sh` pixels taken at `t`, zoomed to the view.
        fn write(&mut self, pixels: &[u8], sw: usize, sh: usize, t: i64) -> Result<(), Error> {
            let r = self.region;
            let view = self.zoom.as_ref().map_or(View::FULL, |z| z.view());
            // A frame older than the last one written would fail the file.
            if self.written.is_some_and(|w| t <= w) {
                return Ok(());
            }
            self.written = Some(t);
            self.shown = view;
            let (dw, dh) = (r.px_w as usize, r.px_h as usize);
            let stride = sw * 4;
            // The encoder wants the exact output size, bottom row first.
            let flipped = if view.is_full() && !r.scaled {
                let mut flipped = vec![0u8; dw * 4 * dh];
                let n = stride.min(dw * 4);
                for (i, line) in pixels.chunks_exact(stride).take(dh).enumerate() {
                    let dst = (dh - 1 - i) * dw * 4;
                    flipped[dst..dst + n].copy_from_slice(&line[..n]);
                }
                flipped
            } else {
                let (x, y, cw, ch) = view.crop(sw, sh);
                super::resample_flipped(&pixels[y * stride + x * 4..], stride, cw, ch, dw, dh)
            };
            if let Some(encoder) = self.encoder.as_mut() {
                encoder.send_frame_buffer(&flipped, t)?;
            }
            if let Some(gif) = self.gif.as_mut() {
                let t = Duration::from_nanos(t.max(0) as u64 * 100);
                gif.push(&flipped, dw * 4, true, t);
                return Ok(());
            }
            self.last = Some(flipped);
            Ok(())
        }

        /// While zoomed, writes the newest frame again if the screen hasn't
        /// sent one lately, so the zoom moves on a still screen too.
        fn tick(&mut self) {
            let Some(zoom) = &self.zoom else { return };
            if zoom.view().is_full() && self.shown.is_full() {
                return;
            }
            let t = now();
            if self.written.is_some_and(|w| t - w < 8_000_000 / FPS as i64) {
                return;
            }
            if let Some((px, w, h)) = self.source.take() {
                let _ = self.write(&px, w, h, t);
                self.source = Some((px, w, h));
            }
        }
    }

    impl GraphicsCaptureApiHandler for Handler {
        /// The region, the file, whether it gets a sound track, and the zoom.
        type Flags = (Region, PathBuf, bool, Option<Arc<Zoom>>);
        type Error = Error;

        fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
            let (region, path, sound, zoom) = ctx.flags;
            if super::is_gif(&path) {
                return Ok(Self {
                    gif: Some(GifWriter::new(&path, region.px_w, region.px_h)?),
                    ..Self::new_with(region, zoom)
                });
            }
            let bitrate = (region.px_w * region.px_h * 4).clamp(2_000_000, 15_000_000);
            let encoder = VideoEncoder::new(
                VideoSettingsBuilder::new(region.px_w, region.px_h)
                    .sub_type(VideoSettingsSubType::H264)
                    .frame_rate(FPS)
                    .bitrate(bitrate),
                // 48 kHz stereo 16-bit, what `audio_mix` produces.
                AudioSettingsBuilder::new().disabled(!sound),
                ContainerSettingsBuilder::default(),
                &path,
            )?;
            Ok(Self {
                encoder: Some(encoder),
                ..Self::new_with(region, zoom)
            })
        }

        fn on_frame_arrived(
            &mut self,
            frame: &mut Frame,
            _control: InternalCaptureControl,
        ) -> Result<(), Self::Error> {
            let r = self.region;
            let timestamp = frame.timestamp()?.Duration;
            let (x, y) = (r.x as u32, r.y as u32);
            // At full size, crop to the (even) output size; scaled or zoomed,
            // take the whole region and resample it.
            let (cw, ch) = if r.scaled || self.zoom.is_some() {
                (r.w as u32, r.h as u32)
            } else {
                (r.px_w, r.px_h)
            };
            let (x1, y1) = ((x + cw).min(frame.width()), (y + ch).min(frame.height()));
            let buffer = frame.buffer_crop(x, y, x1, y1)?;
            let mut scratch = std::mem::take(&mut self.scratch);
            let pixels = buffer.as_nopadding_buffer(&mut scratch);
            let (sw, sh) = ((x1 - x) as usize, (y1 - y) as usize);
            let written = self.write(pixels, sw, sh, timestamp);
            if self.zoom.is_some() {
                let (px, w, h) = self.source.get_or_insert_with(Default::default);
                px.clear();
                px.extend_from_slice(pixels);
                (*w, *h) = (sw, sh);
            }
            self.scratch = scratch;
            written
        }
    }

    pub struct Recorder {
        control: CaptureControl<Handler, Error>,
        sound: Option<sound::Sound>,
        /// Keeps the zoom moving on a still screen, and its stop flag.
        ticker: Option<(JoinHandle<()>, Arc<AtomicBool>)>,
        live: Option<live::Live>,
    }

    /// Windows asks for nothing up front; a blocked microphone fails to open.
    pub fn microphone_allowed() -> bool {
        true
    }

    /// Captures the system's sound (WASAPI loopback) and the microphone,
    /// mixed into one track.
    mod sound {
        use std::collections::VecDeque;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::{mpsc, Arc, Mutex};
        use std::thread::JoinHandle;
        use std::time::{Duration, Instant};

        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        use crate::audio_mix::{self, Resampler, RATE};

        type Queue = Arc<Mutex<VecDeque<[f32; 2]>>>;

        pub struct Sound {
            stop: Arc<AtomicBool>,
            thread: Option<JoinHandle<()>>,
        }

        impl Sound {
            /// Calls `send` every few milliseconds with the sound since the
            /// last call, as 48 kHz stereo 16-bit PCM; silence when there's none.
            pub fn start(
                system: bool,
                mic: bool,
                mut send: impl FnMut(&[u8]) + Send + 'static,
            ) -> Result<Self, String> {
                let stop = Arc::new(AtomicBool::new(false));
                let flag = stop.clone();
                let (ready_tx, ready_rx) = mpsc::channel();
                // cpal streams can't move between threads: open, run and
                // close them all on this one.
                let thread = std::thread::spawn(move || {
                    let host = cpal::default_host();
                    let mut sources = Vec::new();
                    let mut opened: Result<(), String> = Ok(());
                    if system {
                        opened = opened.and_then(|_| {
                            sources.push(open(host.default_output_device(), true)?);
                            Ok(())
                        });
                    }
                    if mic {
                        opened = opened.and_then(|_| {
                            sources.push(open(host.default_input_device(), false)?);
                            Ok(())
                        });
                    }
                    let failed = opened.is_err();
                    let _ = ready_tx.send(opened);
                    if failed {
                        return;
                    }
                    let queues: Vec<Queue> = sources.iter().map(|(_, q)| q.clone()).collect();
                    let start = Instant::now();
                    let mut sent = 0u64;
                    while !flag.load(Ordering::SeqCst) {
                        std::thread::sleep(Duration::from_millis(10));
                        let due = (start.elapsed().as_secs_f64() * RATE as f64) as u64;
                        let n = due.saturating_sub(sent) as usize;
                        if n == 0 {
                            continue;
                        }
                        let pcm = {
                            let mut guards: Vec<_> =
                                queues.iter().map(|q| q.lock().unwrap()).collect();
                            let mut refs: Vec<&mut VecDeque<[f32; 2]>> =
                                guards.iter_mut().map(|g| &mut **g).collect();
                            audio_mix::mix(&mut refs, n)
                        };
                        send(&pcm);
                        sent += n as u64;
                    }
                    drop(sources);
                });
                ready_rx
                    .recv()
                    .map_err(|_| "the sound thread stopped".to_string())??;
                Ok(Self {
                    stop,
                    thread: Some(thread),
                })
            }

            pub fn stop(mut self) {
                self.stop.store(true, Ordering::SeqCst);
                if let Some(t) = self.thread.take() {
                    let _ = t.join();
                }
            }
        }

        /// Starts capturing from a device: an output device records what it
        /// plays (loopback), an input device what it hears.
        fn open(
            device: Option<cpal::Device>,
            loopback: bool,
        ) -> Result<(cpal::Stream, Queue), String> {
            let device = device.ok_or(if loopback {
                "no speakers or headphones found"
            } else {
                "no microphone found"
            })?;
            let config = if loopback {
                device.default_output_config()
            } else {
                device.default_input_config()
            }
            .map_err(|e| e.to_string())?;
            let channels = config.channels() as usize;
            let rate = config.sample_rate().0;
            let format = config.sample_format();
            let stream_config: cpal::StreamConfig = config.into();
            let queue: Queue = Arc::default();
            let q = queue.clone();
            let mut resampler = Resampler::new(rate);
            let mut feed = move |samples: &mut dyn Iterator<Item = f32>| {
                let mut out = Vec::new();
                audio_mix::stereo(samples, channels, |f| resampler.push(f, &mut out));
                audio_mix::enqueue(&mut q.lock().unwrap(), &out);
            };
            let on_error = |e: cpal::StreamError| eprintln!("sound: {e}");
            let stream = match format {
                cpal::SampleFormat::F32 => device.build_input_stream(
                    &stream_config,
                    move |data: &[f32], _: &_| feed(&mut data.iter().copied()),
                    on_error,
                    None,
                ),
                cpal::SampleFormat::I16 => device.build_input_stream(
                    &stream_config,
                    move |data: &[i16], _: &_| feed(&mut data.iter().map(|&s| s as f32 / 32_768.0)),
                    on_error,
                    None,
                ),
                cpal::SampleFormat::I32 => device.build_input_stream(
                    &stream_config,
                    move |data: &[i32], _: &_| {
                        feed(&mut data.iter().map(|&s| s as f32 / 2_147_483_648.0))
                    },
                    on_error,
                    None,
                ),
                cpal::SampleFormat::U16 => device.build_input_stream(
                    &stream_config,
                    move |data: &[u16], _: &_| {
                        feed(&mut data.iter().map(|&s| (s as f32 - 32_768.0) / 32_768.0))
                    },
                    on_error,
                    None,
                ),
                other => return Err(format!("unsupported sound format {other:?}")),
            }
            .map_err(|e| e.to_string())?;
            stream.play().map_err(|e| e.to_string())?;
            Ok((stream, queue))
        }
    }

    /// Shows the zoom live: a window over the region that ignores the mouse,
    /// holding a magnifier control (the Magnification API) set to the view.
    /// It is left out of the recording and of its own source, and draws no
    /// pointer, as Windows draws the real one on top.
    mod live {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::{mpsc, Arc};
        use std::thread::JoinHandle;
        use std::time::Duration;

        use windows::core::w;
        use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
        use windows::Win32::Graphics::Gdi::InvalidateRect;
        use windows::Win32::System::LibraryLoader::GetModuleHandleW;
        use windows::Win32::UI::Magnification::{
            MagInitialize, MagSetWindowFilterList, MagSetWindowSource, MagSetWindowTransform,
            MagUninitialize, MAGTRANSFORM, MW_FILTERMODE_EXCLUDE, WC_MAGNIFIER,
        };
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, PeekMessageW,
            RegisterClassW, SetLayeredWindowAttributes, SetWindowDisplayAffinity, ShowWindow,
            LWA_ALPHA, MSG, PM_REMOVE, SW_HIDE, SW_SHOWNOACTIVATE, WDA_EXCLUDEFROMCAPTURE,
            WINDOW_EX_STYLE, WNDCLASSW, WS_CHILD, WS_EX_LAYERED, WS_EX_NOACTIVATE,
            WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP, WS_VISIBLE,
        };

        use super::super::Region;
        use crate::capture::Bounds;
        use crate::zoom::Zoom;

        pub struct Live {
            thread: JoinHandle<()>,
            stop: Arc<AtomicBool>,
        }

        impl Live {
            /// Opens the window over region `r` of monitor `b`, following `zoom`.
            pub fn start(b: &Bounds, r: Region, zoom: Arc<Zoom>) -> Result<Self, String> {
                let (x, y) = (b.x + r.x as i32, b.y + r.y as i32);
                let screen = RECT {
                    left: x,
                    top: y,
                    right: x + r.w as i32,
                    bottom: y + r.h as i32,
                };
                let stop = Arc::new(AtomicBool::new(false));
                let flag = stop.clone();
                let (tx, rx) = mpsc::channel();
                // Windows belong to the thread that made them: open, run and
                // close them all on this one.
                let thread = std::thread::spawn(move || unsafe { run(screen, &zoom, &flag, tx) });
                rx.recv()
                    .map_err(|_| "the live zoom thread stopped".to_string())??;
                Ok(Self { thread, stop })
            }

            pub fn stop(self) {
                self.stop.store(true, Ordering::SeqCst);
                let _ = self.thread.join();
            }
        }

        unsafe fn run(
            screen: RECT,
            zoom: &Zoom,
            stop: &AtomicBool,
            ready: mpsc::Sender<Result<(), String>>,
        ) {
            if !unsafe { MagInitialize() }.as_bool() {
                let _ = ready.send(Err("the magnifier didn't start".into()));
                return;
            }
            match unsafe { open(screen) } {
                Ok((host, mag)) => {
                    let _ = ready.send(Ok(()));
                    unsafe { follow(screen, host, mag, zoom, stop) };
                    let _ = unsafe { DestroyWindow(host) };
                }
                Err(e) => {
                    let _ = ready.send(Err(e));
                }
            }
            let _ = unsafe { MagUninitialize() };
        }

        unsafe extern "system" fn host_proc(
            hwnd: HWND,
            msg: u32,
            wparam: WPARAM,
            lparam: LPARAM,
        ) -> LRESULT {
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        /// Opens the hidden window over `s` (screen pixels) and the magnifier
        /// filling it.
        unsafe fn open(s: RECT) -> Result<(HWND, HWND), String> {
            unsafe {
                let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
                let class = w!("KlikSnapLiveZoom");
                let wc = WNDCLASSW {
                    lpfnWndProc: Some(host_proc),
                    hInstance: instance.into(),
                    lpszClassName: class,
                    ..Default::default()
                };
                // Fails harmlessly when an earlier recording registered it.
                RegisterClassW(&wc);
                let (w, h) = (s.right - s.left, s.bottom - s.top);
                // Layered and transparent, so clicks go through to what's below.
                let host = CreateWindowExW(
                    WS_EX_TOPMOST
                        | WS_EX_LAYERED
                        | WS_EX_TRANSPARENT
                        | WS_EX_TOOLWINDOW
                        | WS_EX_NOACTIVATE,
                    class,
                    w!(""),
                    WS_POPUP,
                    s.left,
                    s.top,
                    w,
                    h,
                    None,
                    None,
                    Some(instance.into()),
                    None,
                )
                .map_err(|e| e.to_string())?;
                let _ = SetLayeredWindowAttributes(host, COLORREF(0), 255, LWA_ALPHA);
                // Out of the recording, which would zoom it in again.
                let _ = SetWindowDisplayAffinity(host, WDA_EXCLUDEFROMCAPTURE);
                let mag = match CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    WC_MAGNIFIER,
                    w!(""),
                    WS_CHILD | WS_VISIBLE,
                    0,
                    0,
                    w,
                    h,
                    Some(host),
                    None,
                    Some(instance.into()),
                    None,
                ) {
                    Ok(mag) => mag,
                    Err(e) => {
                        let _ = DestroyWindow(host);
                        return Err(e.to_string());
                    }
                };
                // Out of its own source, which would show it inside itself.
                let mut own = host;
                let _ = MagSetWindowFilterList(mag, MW_FILTERMODE_EXCLUDE, 1, &mut own);
                Ok((host, mag))
            }
        }

        /// Sets the magnifier to the view until stopped, showing the window
        /// only while zoomed. The magnifier only redraws when asked, so it is
        /// asked every few milliseconds.
        unsafe fn follow(s: RECT, host: HWND, mag: HWND, zoom: &Zoom, stop: &AtomicBool) {
            let (w, h) = ((s.right - s.left) as usize, (s.bottom - s.top) as usize);
            let mut shown = false;
            let mut msg = MSG::default();
            while !stop.load(Ordering::SeqCst) {
                unsafe {
                    while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                        DispatchMessageW(&msg);
                    }
                    let view = zoom.view();
                    if view.is_full() {
                        if shown {
                            let _ = ShowWindow(host, SW_HIDE);
                            shown = false;
                        }
                    } else {
                        let (x, y, cw, ch) = view.crop(w, h);
                        let source = RECT {
                            left: s.left + x as i32,
                            top: s.top + y as i32,
                            right: s.left + (x + cw) as i32,
                            bottom: s.top + (y + ch) as i32,
                        };
                        let (sx, sy) = (w as f32 / cw as f32, h as f32 / ch as f32);
                        let mut transform = MAGTRANSFORM {
                            v: [sx, 0.0, 0.0, 0.0, sy, 0.0, 0.0, 0.0, 1.0],
                        };
                        let _ = MagSetWindowTransform(mag, &mut transform);
                        let _ = MagSetWindowSource(mag, source);
                        let _ = InvalidateRect(Some(mag), None, true);
                        if !shown {
                            let _ = ShowWindow(host, SW_SHOWNOACTIVATE);
                            shown = true;
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(8));
            }
        }
    }

    /// Now on the capture clock (QPC, in 100 ns units).
    fn now() -> i64 {
        let (mut count, mut freq) = (0i64, 0i64);
        unsafe {
            let _ = QueryPerformanceCounter(&mut count);
            let _ = QueryPerformanceFrequency(&mut freq);
        }
        if freq == 0 {
            return 0;
        }
        (count as i128 * 10_000_000 / freq as i128) as i64
    }

    impl Recorder {
        pub fn start(
            b: &Bounds,
            r: Region,
            path: &Path,
            audio: super::Audio,
            cursor: bool,
            _keep: &[u32],
            zoom: Option<Arc<Zoom>>,
        ) -> Result<Self, String> {
            let with_sound = audio.system || audio.mic;
            let live_zoom = zoom.clone();
            let ticks = zoom.is_some();
            let center = POINT {
                x: b.x + b.w as i32 / 2,
                y: b.y + b.h as i32 / 2,
            };
            let hmonitor = unsafe { MonitorFromPoint(center, MONITOR_DEFAULTTONEAREST) };
            let monitor = Monitor::from_raw_hmonitor(hmonitor.0);
            let settings = Settings::new(
                monitor,
                if cursor {
                    CursorCaptureSettings::WithCursor
                } else {
                    CursorCaptureSettings::WithoutCursor
                },
                DrawBorderSettings::WithoutBorder,
                SecondaryWindowSettings::Default,
                MinimumUpdateIntervalSettings::Custom(Duration::from_millis(1000 / FPS as u64)),
                DirtyRegionSettings::Default,
                ColorFormat::Bgra8,
                (r, path.to_path_buf(), with_sound, zoom),
            );
            let control = Handler::start_free_threaded(settings).map_err(|e| e.to_string())?;
            let ticker = ticks.then(|| {
                let stop = Arc::new(AtomicBool::new(false));
                let (flag, handler) = (stop.clone(), control.callback());
                let thread = std::thread::spawn(move || {
                    while !flag.load(Ordering::SeqCst) {
                        std::thread::sleep(Duration::from_millis(1000 / FPS as u64));
                        handler.lock().tick();
                    }
                });
                (thread, stop)
            });
            // Without it the zoom is still recorded, just not seen.
            let live = live_zoom.and_then(|zoom| {
                live::Live::start(b, r, zoom)
                    .map_err(|e| eprintln!("live zoom: {e}"))
                    .ok()
            });
            if !with_sound {
                return Ok(Self {
                    control,
                    sound: None,
                    ticker,
                    live,
                });
            }
            let handler = control.callback();
            let sound = sound::Sound::start(audio.system, audio.mic, move |pcm| {
                if let Some(encoder) = handler.lock().encoder.as_mut() {
                    let _ = encoder.send_audio_buffer(pcm, 0);
                }
            });
            match sound {
                Ok(sound) => Ok(Self {
                    control,
                    sound: Some(sound),
                    ticker,
                    live,
                }),
                Err(e) => {
                    if let Some(live) = live {
                        live.stop();
                    }
                    if let Some((thread, stop)) = ticker {
                        stop.store(true, Ordering::SeqCst);
                        let _ = thread.join();
                    }
                    let _ = control.stop();
                    Err(e)
                }
            }
        }

        pub fn stop(mut self) -> Result<(), String> {
            if let Some(live) = self.live.take() {
                live.stop();
            }
            if let Some((thread, stop)) = self.ticker.take() {
                stop.store(true, Ordering::SeqCst);
                let _ = thread.join();
            }
            if let Some(sound) = self.sound.take() {
                sound.stop();
            }
            let handler = self.control.callback();
            self.control.stop().map_err(|e| e.to_string())?;
            let mut h = handler.lock();
            if let Some(gif) = h.gif.take() {
                return gif.finish(Duration::from_nanos(now().max(0) as u64 * 100));
            }
            let mut encoder = h.encoder.take().ok_or("nothing was recorded")?;
            // Frames only come when the screen changes: repeat the last one
            // now, or a still ending would be cut off.
            if let Some(last) = h.last.take() {
                let _ = encoder.send_frame_buffer(&last, now());
            }
            encoder.finish().map_err(|e| e.to_string())
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod imp {
    use std::io::{Read, Write};
    use std::path::Path;
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    use super::{Audio, Region};
    use crate::capture::Bounds;
    use crate::zoom::Zoom;

    pub struct Recorder {
        child: Child,
    }

    pub fn microphone_allowed() -> bool {
        true
    }

    impl Recorder {
        pub fn start(
            b: &Bounds,
            r: Region,
            path: &Path,
            audio: Audio,
            cursor: bool,
            _keep: &[u32],
            _zoom: Option<std::sync::Arc<Zoom>>,
        ) -> Result<Self, String> {
            if std::env::var("XDG_SESSION_TYPE").is_ok_and(|t| t == "wayland") {
                return Err("recording on Wayland isn't supported yet: log in with an X11 (Xorg) session to record".into());
            }
            let display = std::env::var("DISPLAY").unwrap_or_else(|_| ":0".into());
            let mut cmd = Command::new("ffmpeg");
            cmd.args(args(b, r, path, audio, cursor, &display))
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped());
            let mut child = cmd.spawn().map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => {
                    "Recording needs ffmpeg: install the ffmpeg package".to_string()
                }
                _ => e.to_string(),
            })?;
            // A bad display, region or sound device ends ffmpeg right away.
            std::thread::sleep(Duration::from_millis(600));
            if let Ok(Some(_)) = child.try_wait() {
                let mut err = String::new();
                if let Some(mut s) = child.stderr.take() {
                    let _ = s.read_to_string(&mut err);
                }
                return Err(format!("ffmpeg: {}", err.trim()));
            }
            Ok(Recorder { child })
        }

        pub fn stop(mut self) -> Result<(), String> {
            // `q` makes ffmpeg finish the file properly.
            if let Some(mut stdin) = self.child.stdin.take() {
                let _ = stdin.write_all(b"q");
            }
            let deadline = Instant::now() + Duration::from_secs(15);
            while Instant::now() < deadline {
                match self.child.try_wait() {
                    Ok(Some(status)) if status.success() => return Ok(()),
                    Ok(Some(status)) => return Err(format!("ffmpeg ended with {status}")),
                    Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                    Err(e) => return Err(e.to_string()),
                }
            }
            let _ = self.child.kill();
            Err("ffmpeg didn't finish the recording".into())
        }
    }

    /// ffmpeg's arguments: X11 grab of the region (pixels on Linux), scaled
    /// to the output size; sound from PulseAudio or PipeWire's Pulse server,
    /// mixed into one track as on Windows.
    pub(super) fn args(
        b: &Bounds,
        r: Region,
        path: &Path,
        audio: Audio,
        cursor: bool,
        display: &str,
    ) -> Vec<String> {
        let mut a: Vec<String> = ["-hide_banner", "-loglevel", "error", "-y"]
            .map(String::from)
            .to_vec();
        let (x, y) = (b.x + r.x as i32, b.y + r.y as i32);
        a.extend([
            "-f".into(),
            "x11grab".into(),
            "-framerate".into(),
            super::FPS.to_string(),
            "-video_size".into(),
            format!("{}x{}", r.w as u32, r.h as u32),
            "-draw_mouse".into(),
            u8::from(cursor).to_string(),
            "-i".into(),
            format!("{display}+{x},{y}"),
        ]);
        let (w, h) = (r.px_w, r.px_h);
        if super::is_gif(path) {
            a.extend([
                "-filter_complex".into(),
                format!(
                    "[0:v]fps={},scale={w}:{h}:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse",
                    crate::gif_writer::FPS
                ),
                "-an".into(),
            ]);
        } else {
            let mut sources = Vec::new();
            if audio.system {
                sources.push("@DEFAULT_MONITOR@");
            }
            if audio.mic {
                sources.push("default");
            }
            for s in &sources {
                a.extend(["-f", "pulse", "-i", s].map(String::from));
            }
            let mut graph = format!("[0:v]scale={w}:{h}[v]");
            match sources.len() {
                0 => {}
                1 => graph.push_str(";[1:a]anull[a]"),
                _ => graph.push_str(";[1:a][2:a]amix=inputs=2[a]"),
            }
            a.extend(["-filter_complex".into(), graph, "-map".into(), "[v]".into()]);
            if !sources.is_empty() {
                a.extend(["-map", "[a]", "-c:a", "aac", "-b:a", "160k"].map(String::from));
            }
            a.extend(
                [
                    "-c:v",
                    "libx264",
                    "-preset",
                    "veryfast",
                    "-crf",
                    "23",
                    "-pix_fmt",
                    "yuv420p",
                    "-movflags",
                    "+faststart",
                ]
                .map(String::from),
            );
        }
        a.push(path.display().to_string());
        a
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn ffmpeg_args_grab_the_region() {
            let b = Bounds {
                x: 1920,
                y: 0,
                w: 1920,
                h: 1080,
                scale: 1.0,
            };
            let r = crate::record::region(&b, [0.5, 0.5, 0.25, 0.25], 50);
            let audio = Audio {
                system: true,
                mic: true,
            };
            let a = args(&b, r, Path::new("/tmp/a.mp4"), audio, false, ":1").join(" ");
            assert!(
                a.contains("-video_size 480x270 -draw_mouse 0 -i :1+2880,540"),
                "{a}"
            );
            let graph = format!(
                "[0:v]scale={}:{}[v];[1:a][2:a]amix=inputs=2[a]",
                r.px_w, r.px_h
            );
            assert!(a.contains(&graph), "{a}");
            assert!(a.ends_with("/tmp/a.mp4"));
            let gif = args(&b, r, Path::new("/tmp/a.gif"), Audio::default(), true, ":1").join(" ");
            assert!(gif.contains("palettegen") && gif.contains("-an"), "{gif}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(scale: f64) -> Bounds {
        Bounds {
            x: 0,
            y: 0,
            w: 1000,
            h: 600,
            scale,
        }
    }

    #[test]
    fn region_is_even_and_inside_the_monitor() {
        let r = region(&bounds(1.0), [0.1, 0.1, 0.2555, 0.5], 100);
        assert_eq!((r.x, r.y), (100.0, 60.0));
        assert_eq!(r.px_w % 2, 0);
        assert_eq!(r.px_h % 2, 0);
        let r = region(&bounds(1.0), [0.9, 0.9, 0.5, 0.5], 100);
        assert!(r.x + r.w <= 1000.0 && r.y + r.h <= 600.0);
    }

    #[test]
    fn region_scales_the_output_only() {
        let full = region(&bounds(1.0), [0.0, 0.0, 0.5, 0.5], 100);
        let half = region(&bounds(1.0), [0.0, 0.0, 0.5, 0.5], 50);
        assert_eq!((half.w, half.h), (full.w, full.h));
        assert_eq!((half.px_w, half.px_h), (250, 150));
        assert!(half.scaled && !full.scaled);
    }

    #[test]
    fn resample_flips_and_averages() {
        // 2×2 → 1×1 averages; rows come out bottom first.
        let px = |v: u8| [v, v, v, 255];
        let src: Vec<u8> = [px(0), px(100), px(200), px(100)].concat();
        assert_eq!(resample_flipped(&src, 8, 2, 2, 1, 1)[..3], [100, 100, 100]);
        let tall: Vec<u8> = [px(10), px(250)].concat();
        let out = resample_flipped(&tall, 4, 1, 2, 1, 2);
        assert_eq!((out[0], out[4]), (250, 10));
        // A stride wider than the row skips what's past it.
        let wide: Vec<u8> = [px(10), px(99), px(250), px(99)].concat();
        let out = resample_flipped(&wide, 8, 1, 2, 1, 2);
        assert_eq!((out[0], out[4]), (250, 10));
    }

    /// Records two seconds of the main display; needs Screen Recording
    /// permission for the terminal. Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn records_an_mp4() {
        let b = crate::capture::bounds_at((10, 10)).unwrap();
        let path = std::env::temp_dir().join("kliksnap-record-test.mp4");
        for percent in [100, 50] {
            let _ = std::fs::remove_file(&path);
            let (rec, _) = start(
                &b,
                [0.0, 0.0, 0.5, 0.5],
                percent,
                &path,
                Audio::default(),
                true,
                &[],
                None,
            )
            .unwrap();
            std::thread::sleep(std::time::Duration::from_secs(2));
            rec.stop().unwrap();
            let len = std::fs::metadata(&path).unwrap().len();
            assert!(len > 1000, "{percent}%: file is only {len} bytes");
        }
    }

    /// Records with the system's sound; needs the same permission.
    #[test]
    #[ignore]
    fn records_with_sound() {
        let b = crate::capture::bounds_at((10, 10)).unwrap();
        let path = std::env::temp_dir().join("kliksnap-record-sound.mp4");
        let _ = std::fs::remove_file(&path);
        let audio = Audio {
            system: true,
            mic: false,
        };
        let (rec, warning) =
            start(&b, [0.0, 0.0, 0.5, 0.5], 50, &path, audio, true, &[], None).unwrap();
        assert_eq!(warning, None);
        std::thread::sleep(std::time::Duration::from_secs(2));
        rec.stop().unwrap();
        assert!(std::fs::metadata(&path).unwrap().len() > 1000);
    }

    /// Records zoomed in, on a still screen too, as MP4 and GIF; needs the
    /// same permission.
    #[test]
    #[ignore]
    fn records_zoomed() {
        let b = crate::capture::bounds_at((10, 10)).unwrap();
        let view = crate::zoom::View {
            scale: 2.0,
            x: 0.3,
            y: 0.3,
        };
        for ext in ["mp4", "gif"] {
            let path = std::env::temp_dir().join(format!("kliksnap-record-zoom.{ext}"));
            let _ = std::fs::remove_file(&path);
            let zoom = Some(crate::zoom::Zoom::fixed(view));
            let (rec, _) = start(
                &b,
                [0.0, 0.0, 0.5, 0.5],
                100,
                &path,
                Audio::default(),
                true,
                &[],
                zoom,
            )
            .unwrap();
            std::thread::sleep(std::time::Duration::from_secs(2));
            rec.stop().unwrap();
            let len = std::fs::metadata(&path).unwrap().len();
            assert!(len > 1000, "{ext}: file is only {len} bytes");
        }
    }

    /// Like `records_an_mp4`, for a GIF. Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn records_a_gif() {
        let b = crate::capture::bounds_at((10, 10)).unwrap();
        let path = std::env::temp_dir().join("kliksnap-record-test.gif");
        let _ = std::fs::remove_file(&path);
        let (rec, _) = start(
            &b,
            [0.0, 0.0, 0.5, 0.5],
            100,
            &path,
            Audio::default(),
            true,
            &[],
            None,
        )
        .unwrap();
        std::thread::sleep(std::time::Duration::from_secs(2));
        rec.stop().unwrap();
        let len = std::fs::metadata(&path).unwrap().len();
        assert!(len > 1000, "file is only {len} bytes");
    }
}
