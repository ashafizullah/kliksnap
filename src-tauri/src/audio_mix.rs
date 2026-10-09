//! Turns sound from devices with any rate and channel count into one
//! stream of 48 kHz stereo 16-bit PCM, as the Windows encoder wants it:
//! each source is resampled into a queue, and the queues are summed.

use std::collections::VecDeque;

pub const RATE: u32 = 48_000;
/// Queued sound beyond this many frames is dropped, so a source running a
/// little fast never drifts far from the video.
const MAX_QUEUE: usize = RATE as usize / 5;

/// Linear resampling of stereo frames to `RATE`.
pub struct Resampler {
    step: f64,
    pos: f64,
    prev: [f32; 2],
}

impl Resampler {
    pub fn new(input_rate: u32) -> Self {
        Self {
            step: input_rate.max(1) as f64 / RATE as f64,
            pos: 0.0,
            prev: [0.0; 2],
        }
    }

    /// Adds one input frame; appends the output frames it completes.
    pub fn push(&mut self, cur: [f32; 2], out: &mut Vec<[f32; 2]>) {
        while self.pos < 1.0 {
            let t = self.pos as f32;
            out.push([
                self.prev[0] + (cur[0] - self.prev[0]) * t,
                self.prev[1] + (cur[1] - self.prev[1]) * t,
            ]);
            self.pos += self.step;
        }
        self.pos -= 1.0;
        self.prev = cur;
    }
}

/// Interleaved samples with `channels` per frame, as stereo frames: mono is
/// doubled, and channels past the second are left out.
pub fn stereo(samples: impl Iterator<Item = f32>, channels: usize, mut f: impl FnMut([f32; 2])) {
    let channels = channels.max(1);
    let mut frame = [0.0f32; 2];
    for (i, s) in samples.enumerate() {
        let c = i % channels;
        if c < 2 {
            frame[c] = s;
        }
        if c == channels - 1 {
            if channels == 1 {
                frame[1] = frame[0];
            }
            f(frame);
        }
    }
}

/// Appends frames to a source's queue, dropping the oldest past the limit.
pub fn enqueue(queue: &mut VecDeque<[f32; 2]>, frames: &[[f32; 2]]) {
    queue.extend(frames);
    let excess = queue.len().saturating_sub(MAX_QUEUE);
    queue.drain(..excess);
}

/// `n` frames of the sources summed, as little-endian 16-bit PCM. A source
/// with too little queued plays silence for the rest.
pub fn mix(queues: &mut [&mut VecDeque<[f32; 2]>], n: usize) -> Vec<u8> {
    let mut pcm = Vec::with_capacity(n * 4);
    for _ in 0..n {
        let mut frame = [0.0f32; 2];
        for q in queues.iter_mut() {
            if let Some([l, r]) = q.pop_front() {
                frame[0] += l;
                frame[1] += r;
            }
        }
        for s in frame {
            let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            pcm.extend_from_slice(&v.to_le_bytes());
        }
    }
    pcm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resampling_keeps_the_duration() {
        for rate in [44_100, 48_000, 16_000, 96_000] {
            let mut r = Resampler::new(rate);
            let mut out = Vec::new();
            for i in 0..rate {
                let v = (i as f32 / rate as f32).sin();
                r.push([v, v], &mut out);
            }
            // One second in, one second out, give or take a frame.
            assert!(
                (out.len() as i64 - RATE as i64).abs() <= 1,
                "{rate}: {}",
                out.len()
            );
        }
    }

    #[test]
    fn stereo_handles_mono_and_surround() {
        let mut frames = Vec::new();
        stereo([0.1, 0.2].into_iter(), 1, |f| frames.push(f));
        assert_eq!(frames, [[0.1, 0.1], [0.2, 0.2]]);
        frames.clear();
        stereo(
            [0.1, 0.2, 0.9, 0.9, 0.3, 0.4, 0.9, 0.9].into_iter(),
            4,
            |f| frames.push(f),
        );
        assert_eq!(frames, [[0.1, 0.2], [0.3, 0.4]]);
    }

    #[test]
    fn mix_sums_clamps_and_pads_with_silence() {
        let mut a: VecDeque<_> = [[0.5, -0.5], [0.8, 0.0]].into();
        let mut b: VecDeque<_> = [[0.75, -0.75]].into();
        let pcm = mix(&mut [&mut a, &mut b], 3);
        let samples: Vec<i16> = pcm
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&c| i16::from_le_bytes(c))
            .collect();
        assert_eq!(samples, [i16::MAX, -i16::MAX, 26213, 0, 0, 0]);
    }

    #[test]
    fn queue_is_capped() {
        let mut q = VecDeque::new();
        enqueue(&mut q, &vec![[0.0, 0.0]; MAX_QUEUE + 10]);
        assert_eq!(q.len(), MAX_QUEUE);
    }
}
