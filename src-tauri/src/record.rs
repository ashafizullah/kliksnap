//! Screen recording to MP4 with the encoders built into the OS:
//! ScreenCaptureKit + AVAssetWriter on macOS, Windows Graphics Capture +
//! Media Foundation on Windows. A `.gif` path records an animated GIF
//! instead, from the same frames.

use std::path::Path;

use crate::capture::Bounds;

pub use imp::Recorder;

/// Frames per second recorded.
pub const FPS: u32 = 30;

/// Starts recording `rect` (fractions of monitor `b`) into an MP4 at `path`,
/// or a GIF if the path ends in `.gif`, at `percent` of the screen's resolution.
pub fn start(b: &Bounds, rect: [f64; 4], percent: u32, path: &Path) -> Result<Recorder, String> {
    let mut r = region(b, rect, percent);
    if is_gif(path) {
        let (w, h) = crate::gif_writer::fit(r.px_w, r.px_h);
        r.scaled |= (w, h) != (r.px_w, r.px_h);
        (r.px_w, r.px_h) = (w, h);
    }
    imp::Recorder::start(b, r, path)
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

/// Scales tightly packed BGRA `src` (`sw`×`sh`) to `dw`×`dh` with bilinear
/// filtering, bottom row first as the Windows encoder wants it.
#[cfg(any(target_os = "windows", test))]
pub fn resample_flipped(src: &[u8], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<u8> {
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
        let (r0, r1) = (&src[y0 * sw * 4..][..sw * 4], &src[y1 * sw * 4..][..sw * 4]);
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
    use std::path::Path;
    use std::ptr::NonNull;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc;
    use std::time::Duration;

    use block2::RcBlock;
    use dispatch2::DispatchQueue;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, ProtocolObject};
    use objc2::{define_class, msg_send, AllocAnyThread, DefinedClass};
    use objc2_av_foundation::{
        AVAssetWriter, AVAssetWriterInput, AVAssetWriterStatus, AVFileTypeMPEG4, AVMediaTypeVideo,
        AVVideoCodecKey, AVVideoCodecTypeH264, AVVideoHeightKey, AVVideoWidthKey,
    };
    use objc2_core_foundation::CFRetained;
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_core_media::{kCMTimeInvalid, CMClock, CMSampleBuffer, CMSampleTimingInfo, CMTime};
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

    const WAIT: Duration = Duration::from_secs(10);

    pub enum Writer {
        Mp4(Mp4),
        Gif(std::sync::Mutex<Option<GifWriter>>),
    }

    pub struct Mp4 {
        writer: Retained<AVAssetWriter>,
        input: Retained<AVAssetWriterInput>,
        started: AtomicBool,
        /// The newest frame, appended again when recording stops.
        last: std::sync::Mutex<Option<CFRetained<CMSampleBuffer>>>,
    }

    fn seconds(t: CMTime) -> Duration {
        if t.timescale <= 0 || t.value < 0 {
            return Duration::ZERO;
        }
        Duration::from_secs_f64(t.value as f64 / t.timescale as f64)
    }

    /// Hands the frame's pixels to the GIF writer.
    unsafe fn push_gif(gif: &std::sync::Mutex<Option<GifWriter>>, buffer: &CMSampleBuffer) {
        use objc2_core_video::{
            CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow, CVPixelBufferGetHeight,
            CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress,
        };
        unsafe {
            let Some(image) = buffer.image_buffer() else {
                return;
            };
            if CVPixelBufferLockBaseAddress(&image, CVPixelBufferLockFlags::ReadOnly) != 0 {
                return;
            }
            let base = CVPixelBufferGetBaseAddress(&image);
            let stride = CVPixelBufferGetBytesPerRow(&image);
            let h = CVPixelBufferGetHeight(&image);
            if !base.is_null() {
                let pixels = std::slice::from_raw_parts(base as *const u8, stride * h);
                if let Some(g) = gif.lock().unwrap().as_mut() {
                    g.push(
                        pixels,
                        stride,
                        false,
                        seconds(buffer.presentation_time_stamp()),
                    );
                }
            }
            CVPixelBufferUnlockBaseAddress(&image, CVPixelBufferLockFlags::ReadOnly);
        }
    }

    define_class!(
        // Receives the frames on the recording's serial queue.
        #[unsafe(super(NSObject))]
        #[name = "KlikSnapRecordOutput"]
        #[ivars = Writer]
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
                    return;
                }
                unsafe {
                    // Frames where nothing changed carry no image.
                    if !buffer.is_valid() || buffer.image_buffer().is_none() {
                        return;
                    }
                    let w = match self.ivars() {
                        Writer::Mp4(w) => w,
                        Writer::Gif(g) => return push_gif(g, buffer),
                    };
                    if !w.started.swap(true, Ordering::SeqCst) {
                        w.writer
                            .startSessionAtSourceTime(buffer.presentation_time_stamp());
                    }
                    if w.input.isReadyForMoreMediaData() && w.input.appendSampleBuffer(buffer) {
                        *w.last.lock().unwrap() = Some(CFRetained::retain(NonNull::from(buffer)));
                    }
                }
            }
        }
    );

    pub struct Recorder {
        stream: Retained<SCStream>,
        output: Retained<Output>,
        queue: dispatch2::DispatchRetained<DispatchQueue>,
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
        unsafe { e.as_ref() }.map(|e| e.localizedDescription().to_string())
    }

    impl Recorder {
        pub fn start(b: &Bounds, r: Region, path: &Path) -> Result<Self, String> {
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
            let filter = unsafe {
                SCContentFilter::initWithDisplay_excludingApplications_exceptingWindows(
                    SCContentFilter::alloc(),
                    &display,
                    &own,
                    &NSArray::new(),
                )
            };
            let config = unsafe { SCStreamConfiguration::new() };
            unsafe {
                config.setWidth(r.px_w as usize);
                config.setHeight(r.px_h as usize);
                config.setSourceRect(CGRect::new(CGPoint::new(r.x, r.y), CGSize::new(r.w, r.h)));
                config.setMinimumFrameInterval(CMTime::new(1, FPS as i32));
                config.setShowsCursor(true);
                config.setPixelFormat(u32::from_be_bytes(*b"BGRA"));
                config.setQueueDepth(6);
            }

            let writer = if super::is_gif(path) {
                Writer::Gif(std::sync::Mutex::new(Some(GifWriter::new(
                    path, r.px_w, r.px_h,
                )?)))
            } else {
                Writer::Mp4(new_writer(path, r)?)
            };
            let output = Output::alloc().set_ivars(writer);
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
            unsafe {
                stream.addStreamOutput_type_sampleHandlerQueue_error(
                    ProtocolObject::from_ref(&*output),
                    SCStreamOutputType::Screen,
                    Some(&queue),
                )
            }
            .map_err(|e| e.localizedDescription().to_string())?;
            let (tx, rx) = mpsc::channel();
            let done = RcBlock::new(move |e: *mut NSError| {
                let _ = tx.send(ns_error(e));
            });
            unsafe { stream.startCaptureWithCompletionHandler(Some(&done)) };
            if let Some(e) = rx
                .recv_timeout(WAIT)
                .map_err(|_| "recording didn't start")?
            {
                return Err(e);
            }
            Ok(Self {
                stream,
                output,
                queue,
            })
        }

        /// Stops recording and finishes the file.
        pub fn stop(self) -> Result<(), String> {
            let (tx, rx) = mpsc::channel();
            let done = RcBlock::new(move |e: *mut NSError| {
                let _ = tx.send(ns_error(e));
            });
            unsafe { self.stream.stopCaptureWithCompletionHandler(Some(&done)) };
            if let Ok(Some(e)) = rx.recv_timeout(WAIT) {
                eprintln!("stopping the recording: {e}");
            }
            // Finish on the frames' queue, after any frame still in flight.
            let (tx, rx) = mpsc::channel();
            let output = AnyThread(self.output.clone());
            self.queue.exec_async(move || {
                let w = output.get().ivars();
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
                let timing = CMSampleTimingInfo {
                    duration: kCMTimeInvalid,
                    presentationTimeStamp: end,
                    decodeTimeStamp: kCMTimeInvalid,
                };
                let mut copy: *mut CMSampleBuffer = std::ptr::null_mut();
                let status = CMSampleBuffer::create_copy_with_new_timing(
                    None,
                    &last,
                    1,
                    &timing,
                    NonNull::from(&mut copy),
                );
                if status == 0 {
                    if let Some(copy) = NonNull::new(copy) {
                        let copy = CFRetained::from_raw(copy);
                        if w.input.isReadyForMoreMediaData() {
                            w.input.appendSampleBuffer(&copy);
                        }
                    }
                }
            }
            w.input.markAsFinished();
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

    fn new_writer(path: &Path, r: Region) -> Result<Mp4, String> {
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
                started: AtomicBool::new(false),
                last: std::sync::Mutex::new(None),
            })
        }
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use std::path::{Path, PathBuf};
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

    type Error = Box<dyn std::error::Error + Send + Sync>;

    pub struct Handler {
        region: Region,
        encoder: Option<VideoEncoder>,
        gif: Option<GifWriter>,
        scratch: Vec<u8>,
        /// The newest frame (bottom-up rows), sent again when recording stops.
        last: Option<Vec<u8>>,
    }

    impl GraphicsCaptureApiHandler for Handler {
        type Flags = (Region, PathBuf);
        type Error = Error;

        fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
            let (region, path) = ctx.flags;
            if super::is_gif(&path) {
                return Ok(Self {
                    region,
                    encoder: None,
                    gif: Some(GifWriter::new(&path, region.px_w, region.px_h)?),
                    scratch: Vec::new(),
                    last: None,
                });
            }
            let bitrate = (region.px_w * region.px_h * 4).clamp(2_000_000, 15_000_000);
            let encoder = VideoEncoder::new(
                VideoSettingsBuilder::new(region.px_w, region.px_h)
                    .sub_type(VideoSettingsSubType::H264)
                    .frame_rate(FPS)
                    .bitrate(bitrate),
                AudioSettingsBuilder::default().disabled(true),
                ContainerSettingsBuilder::default(),
                &path,
            )?;
            Ok(Self {
                region,
                encoder: Some(encoder),
                gif: None,
                scratch: Vec::new(),
                last: None,
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
            // At full size, crop to the (even) output size; scaled, take the
            // whole region and resample it.
            let (cw, ch) = if r.scaled {
                (r.w as u32, r.h as u32)
            } else {
                (r.px_w, r.px_h)
            };
            let (x1, y1) = ((x + cw).min(frame.width()), (y + ch).min(frame.height()));
            let buffer = frame.buffer_crop(x, y, x1, y1)?;
            let pixels = buffer.as_nopadding_buffer(&mut self.scratch);
            let (sw, sh) = ((x1 - x) as usize, (y1 - y) as usize);
            // The encoder wants the exact output size, bottom row first.
            let flipped = if r.scaled {
                super::resample_flipped(pixels, sw, sh, r.px_w as usize, r.px_h as usize)
            } else {
                let out_row = r.px_w as usize * 4;
                let mut flipped = vec![0u8; out_row * r.px_h as usize];
                for (i, line) in pixels.chunks_exact(sw * 4).enumerate() {
                    let dst = (r.px_h as usize - 1 - i) * out_row;
                    flipped[dst..dst + sw * 4].copy_from_slice(line);
                }
                flipped
            };
            if let Some(encoder) = self.encoder.as_mut() {
                encoder.send_frame_buffer(&flipped, timestamp)?;
            }
            if let Some(gif) = self.gif.as_mut() {
                let t = Duration::from_nanos(timestamp.max(0) as u64 * 100);
                gif.push(&flipped, r.px_w as usize * 4, true, t);
                return Ok(());
            }
            self.last = Some(flipped);
            Ok(())
        }
    }

    pub struct Recorder {
        control: CaptureControl<Handler, Error>,
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
        pub fn start(b: &Bounds, r: Region, path: &Path) -> Result<Self, String> {
            let center = POINT {
                x: b.x + b.w as i32 / 2,
                y: b.y + b.h as i32 / 2,
            };
            let hmonitor = unsafe { MonitorFromPoint(center, MONITOR_DEFAULTTONEAREST) };
            let monitor = Monitor::from_raw_hmonitor(hmonitor.0);
            let settings = Settings::new(
                monitor,
                CursorCaptureSettings::WithCursor,
                DrawBorderSettings::WithoutBorder,
                SecondaryWindowSettings::Default,
                MinimumUpdateIntervalSettings::Custom(Duration::from_millis(1000 / FPS as u64)),
                DirtyRegionSettings::Default,
                ColorFormat::Bgra8,
                (r, path.to_path_buf()),
            );
            let control = Handler::start_free_threaded(settings).map_err(|e| e.to_string())?;
            Ok(Self { control })
        }

        pub fn stop(self) -> Result<(), String> {
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
    use super::Region;
    use crate::capture::Bounds;

    pub struct Recorder;

    impl Recorder {
        pub fn start(_b: &Bounds, _r: Region, _path: &std::path::Path) -> Result<Self, String> {
            Err("screen recording isn't supported here".into())
        }
        pub fn stop(self) -> Result<(), String> {
            Ok(())
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
        assert_eq!(resample_flipped(&src, 2, 2, 1, 1)[..3], [100, 100, 100]);
        let tall: Vec<u8> = [px(10), px(250)].concat();
        let out = resample_flipped(&tall, 1, 2, 1, 2);
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
            let rec = start(&b, [0.0, 0.0, 0.5, 0.5], percent, &path).unwrap();
            std::thread::sleep(std::time::Duration::from_secs(2));
            rec.stop().unwrap();
            let len = std::fs::metadata(&path).unwrap().len();
            assert!(len > 1000, "{percent}%: file is only {len} bytes");
        }
    }

    /// Like `records_an_mp4`, for a GIF. Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn records_a_gif() {
        let b = crate::capture::bounds_at((10, 10)).unwrap();
        let path = std::env::temp_dir().join("kliksnap-record-test.gif");
        let _ = std::fs::remove_file(&path);
        let rec = start(&b, [0.0, 0.0, 0.5, 0.5], 100, &path).unwrap();
        std::thread::sleep(std::time::Duration::from_secs(2));
        rec.stop().unwrap();
        let len = std::fs::metadata(&path).unwrap().len();
        assert!(len > 1000, "file is only {len} bytes");
    }
}
