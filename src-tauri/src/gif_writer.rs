//! Writes recorded frames to an animated GIF as they arrive. Each frame
//! keeps only the rectangle that changed, quantized to 256 colors on worker
//! threads, and shows until the next one came in, so a still screen costs
//! nothing.
//!
//! Linux records GIFs with ffmpeg and only uses `fit` and `FPS` from here.
#![cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]

use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// Frames per second kept; more makes big files for little gain.
pub const FPS: u32 = 15;
/// The longest side of a GIF, in pixels.
pub const MAX_SIDE: u32 = 960;
/// Lower is better looking and slower (1–30).
const SPEED: i32 = 12;

struct Raw {
    seq: u64,
    /// The changed rectangle's pixels.
    rgba: Vec<u8>,
    rect: (u16, u16, u16, u16),
    t: Duration,
}

struct Quantized {
    seq: u64,
    frame: gif::Frame<'static>,
    t: Duration,
}

pub struct GifWriter {
    width: u16,
    height: u16,
    tx: Option<SyncSender<Raw>>,
    workers: Vec<JoinHandle<()>>,
    writer: Option<JoinHandle<Result<(), String>>>,
    end_tx: Option<mpsc::Sender<Duration>>,
    seq: u64,
    last: Option<Duration>,
    /// The last frame sent, to find what changed.
    prev: Option<Vec<u8>>,
}

impl GifWriter {
    pub fn new(path: &Path, width: u32, height: u32) -> Result<Self, String> {
        let (w, h) = (
            width.min(u16::MAX as u32) as u16,
            height.min(u16::MAX as u32) as u16,
        );
        let file = File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut encoder =
            gif::Encoder::new(BufWriter::new(file), w, h, &[]).map_err(|e| e.to_string())?;
        encoder
            .set_repeat(gif::Repeat::Infinite)
            .map_err(|e| e.to_string())?;

        // A short queue: when quantizing falls behind, frames are dropped
        // and the ones kept simply show longer.
        let (tx, rx) = mpsc::sync_channel::<Raw>(8);
        let rx = Arc::new(Mutex::new(rx));
        let (qtx, qrx) = mpsc::channel::<Quantized>();
        let threads = std::thread::available_parallelism()
            .map_or(2, |n| n.get().saturating_sub(1))
            .clamp(1, 4);
        let workers = (0..threads)
            .map(|_| {
                let rx = rx.clone();
                let qtx = qtx.clone();
                std::thread::spawn(move || quantize(&rx, &qtx))
            })
            .collect();
        drop(qtx);
        let (end_tx, end_rx) = mpsc::channel();
        let writer = std::thread::spawn(move || write(encoder, qrx, end_rx));
        Ok(Self {
            width: w,
            height: h,
            tx: Some(tx),
            workers,
            writer: Some(writer),
            end_tx: Some(end_tx),
            seq: 0,
            last: None,
            prev: None,
        })
    }

    /// Adds a frame of BGRA pixels, `stride` bytes per row, taken at `t`.
    /// Rows may come bottom first. Frames sooner than 1/FPS after the
    /// previous one kept are skipped.
    pub fn push(&mut self, bgra: &[u8], stride: usize, bottom_up: bool, t: Duration) {
        if self.last.is_some_and(|last| {
            t.saturating_sub(last) < Duration::from_millis(1000 / FPS as u64 - 5)
        }) {
            return;
        }
        let (w, h) = (self.width as usize, self.height as usize);
        if stride < w * 4 || bgra.len() < stride * (h - 1) + w * 4 {
            return;
        }
        let mut rgba = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            let row = if bottom_up { h - 1 - y } else { y };
            for px in bgra[row * stride..][..w * 4].as_chunks::<4>().0 {
                rgba.extend_from_slice(&[px[2], px[1], px[0], 255]);
            }
        }
        let rect = match &self.prev {
            Some(prev) => match changed(prev, &rgba, w, h) {
                Some(r) => r,
                None => return,
            },
            None => (0, 0, w, h),
        };
        let part = crop(&rgba, w, rect);
        let Some(tx) = &self.tx else { return };
        let (x, y, cw, ch) = rect;
        match tx.try_send(Raw {
            seq: self.seq,
            rgba: part,
            rect: (x as u16, y as u16, cw as u16, ch as u16),
            t,
        }) {
            Ok(()) => {
                self.seq += 1;
                self.last = Some(t);
                self.prev = Some(rgba);
            }
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {}
        }
    }

    /// Writes the frames still queued, the last one lasting until `end`.
    pub fn finish(mut self, end: Duration) -> Result<(), String> {
        if self.seq == 0 {
            return Err("nothing was recorded".into());
        }
        if let Some(end_tx) = self.end_tx.take() {
            let _ = end_tx.send(end);
        }
        drop(self.tx.take());
        for w in self.workers.drain(..) {
            let _ = w.join();
        }
        self.writer.take().map_or(Ok(()), |w| {
            w.join().unwrap_or(Err("GIF writer crashed".into()))
        })
    }
}

/// The bounding box (`x, y, w, h`) of the pixels that differ, if any do.
fn changed(a: &[u8], b: &[u8], w: usize, h: usize) -> Option<(usize, usize, usize, usize)> {
    let row = w * 4;
    let differs = |y: usize| a[y * row..][..row] != b[y * row..][..row];
    let top = (0..h).find(|&y| differs(y))?;
    let bottom = (top..h).rev().find(|&y| differs(y)).unwrap_or(top);
    let (mut left, mut right) = (w, 0);
    for y in top..=bottom {
        let (ra, rb) = (&a[y * row..][..row], &b[y * row..][..row]);
        if let Some(x) = (0..left).find(|&x| ra[x * 4..x * 4 + 4] != rb[x * 4..x * 4 + 4]) {
            left = x;
        }
        if let Some(x) = (right..w)
            .rev()
            .find(|&x| ra[x * 4..x * 4 + 4] != rb[x * 4..x * 4 + 4])
        {
            right = x;
        }
    }
    let right = right.max(left);
    Some((left, top, right - left + 1, bottom - top + 1))
}

fn crop(rgba: &[u8], w: usize, (x, y, cw, ch): (usize, usize, usize, usize)) -> Vec<u8> {
    let mut out = Vec::with_capacity(cw * ch * 4);
    for row in y..y + ch {
        out.extend_from_slice(&rgba[(row * w + x) * 4..][..cw * 4]);
    }
    out
}

fn quantize(rx: &Mutex<Receiver<Raw>>, tx: &mpsc::Sender<Quantized>) {
    loop {
        // Hold the lock only to take a frame, not while quantizing it.
        let next = rx.lock().unwrap().recv();
        let Ok(mut raw) = next else { return };
        let (x, y, w, h) = raw.rect;
        let mut frame = gif::Frame::from_rgba_speed(w, h, &mut raw.rgba, SPEED);
        frame.left = x;
        frame.top = y;
        // Later frames draw over this one.
        frame.dispose = gif::DisposalMethod::Keep;
        if tx
            .send(Quantized {
                seq: raw.seq,
                frame,
                t: raw.t,
            })
            .is_err()
        {
            return;
        }
    }
}

/// Writes frames in order; each one's delay is the time until the next.
fn write(
    mut encoder: gif::Encoder<BufWriter<File>>,
    rx: Receiver<Quantized>,
    end: Receiver<Duration>,
) -> Result<(), String> {
    let mut waiting = BTreeMap::new();
    let mut next = 0u64;
    let mut held: Option<Quantized> = None;
    let mut emit = |held: &mut Option<Quantized>, until: Duration| -> Result<(), String> {
        if let Some(mut q) = held.take() {
            q.frame.delay = centis(until.saturating_sub(q.t));
            encoder.write_frame(&q.frame).map_err(|e| e.to_string())?;
        }
        Ok(())
    };
    for q in rx {
        waiting.insert(q.seq, q);
        while let Some(q) = waiting.remove(&next) {
            next += 1;
            emit(&mut held, q.t)?;
            held = Some(q);
        }
    }
    // Frames lost to a crashed worker leave gaps: write what's left in order.
    for (_, q) in std::mem::take(&mut waiting) {
        emit(&mut held, q.t)?;
        held = Some(q);
    }
    let end = end.recv().unwrap_or_default();
    let last_t = held.as_ref().map(|q| q.t).unwrap_or_default();
    emit(&mut held, end.max(last_t + Duration::from_millis(500)))
}

/// A GIF delay in hundredths of a second; browsers treat under 2 as 10.
fn centis(d: Duration) -> u16 {
    (d.as_millis() / 10).clamp(2, u16::MAX as u128) as u16
}

/// Fits `w`×`h` inside `MAX_SIDE`, keeping the aspect ratio.
pub fn fit(w: u32, h: u32) -> (u32, u32) {
    let k = (MAX_SIDE as f64 / w.max(h) as f64).min(1.0);
    (
        ((w as f64 * k).round() as u32).max(2) & !1,
        ((h as f64 * k).round() as u32).max(2) & !1,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_keeps_small_sizes_and_shrinks_large_ones() {
        assert_eq!(fit(640, 480), (640, 480));
        assert_eq!(fit(1920, 1080), (960, 540));
        assert_eq!(fit(1000, 3000), (320, 960));
    }

    #[test]
    fn changed_finds_the_box() {
        let (w, h) = (4, 3);
        let a = vec![0u8; w * h * 4];
        assert_eq!(changed(&a, &a, w, h), None);
        let mut b = a.clone();
        b[(w + 2) * 4] = 9;
        b[(2 * w + 1) * 4 + 1] = 9;
        assert_eq!(changed(&a, &b, w, h), Some((1, 1, 2, 2)));
        assert_eq!(crop(&b, w, (2, 1, 1, 1)), [9, 0, 0, 0]);
    }

    #[test]
    fn writes_frames_with_their_timing() {
        let path = std::env::temp_dir().join(format!("kliksnap-{}.gif", std::process::id()));
        let (w, h) = (32u32, 16u32);
        let mut g = GifWriter::new(&path, w, h).unwrap();
        for i in 0..5u8 {
            let px = [i * 50, 0, 255 - i * 50, 255];
            let frame: Vec<u8> = px
                .iter()
                .copied()
                .cycle()
                .take((w * h * 4) as usize)
                .collect();
            g.push(
                &frame,
                w as usize * 4,
                false,
                Duration::from_millis(i as u64 * 200),
            );
            // Too soon after the previous frame: dropped.
            g.push(
                &frame,
                w as usize * 4,
                false,
                Duration::from_millis(i as u64 * 200 + 10),
            );
        }
        g.finish(Duration::from_millis(2000)).unwrap();

        let mut options = gif::DecodeOptions::new();
        options.set_color_output(gif::ColorOutput::RGBA);
        let mut decoder = options.read_info(File::open(&path).unwrap()).unwrap();
        let mut delays = Vec::new();
        while let Some(frame) = decoder.read_next_frame().unwrap() {
            assert_eq!((frame.width, frame.height), (32, 16));
            delays.push(frame.delay);
        }
        assert_eq!(delays, [20, 20, 20, 20, 120]);
        std::fs::remove_file(&path).unwrap();
    }
}
