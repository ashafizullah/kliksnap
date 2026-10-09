//! Stitches frames of a region the user scrolls into one tall image. Each
//! new frame is matched against the previous one by comparing row hashes:
//! the shift that lines the rows up tells how far the content moved, and
//! the rows that came into view are appended. Rows that never move (a
//! sticky header or footer) are kept once.

use std::hash::{Hash, Hasher};

use xcap::image::RgbaImage;

/// The tallest result, in pixels.
pub const MAX_HEIGHT: usize = 30_000;

#[derive(Debug, PartialEq)]
pub enum Step {
    /// The content moved and this many rows were added.
    Added(usize),
    /// Nothing moved.
    Same,
    /// No match: scrolled too far between frames, or upwards.
    Lost,
    /// The image reached `MAX_HEIGHT`.
    Full,
}

pub struct Stitcher {
    w: usize,
    h: usize,
    /// The image so far, without the footer.
    out: Vec<u8>,
    prev: Vec<u8>,
    prev_rows: Vec<Row>,
    /// Header and footer heights, fixed at the first scroll.
    bands: Option<(usize, usize)>,
}

#[derive(Clone, Copy, PartialEq)]
struct Row {
    hash: u64,
    /// One color across: matches anywhere, so it says nothing about the shift.
    plain: bool,
}

impl Stitcher {
    pub fn new(first: &RgbaImage) -> Self {
        let (w, h) = (first.width() as usize, first.height() as usize);
        Self {
            w,
            h,
            out: first.as_raw().clone(),
            prev: first.as_raw().clone(),
            prev_rows: rows(first.as_raw(), w, h),
            bands: None,
        }
    }

    /// The height of the result so far.
    pub fn height(&self) -> usize {
        self.out.len() / (self.w * 4) + self.bands.map_or(0, |(_, bottom)| bottom)
    }

    pub fn push(&mut self, frame: &RgbaImage) -> Step {
        if (frame.width() as usize, frame.height() as usize) != (self.w, self.h) {
            return Step::Lost;
        }
        if self.height() >= MAX_HEIGHT {
            return Step::Full;
        }
        let cur = rows(frame.as_raw(), self.w, self.h);
        if cur == self.prev_rows {
            return Step::Same;
        }
        let (top, bottom) = self.bands.unwrap_or_else(|| bands(&self.prev_rows, &cur));
        let Some(dy) = shift(&self.prev_rows, &cur, top, self.h - bottom) else {
            return Step::Lost;
        };
        let row = self.w * 4;
        if self.bands.is_none() {
            // The first frame's footer ends `out`; the last frame's goes
            // under everything when done.
            self.bands = Some((top, bottom));
            self.out.truncate(self.out.len() - bottom * row);
        }
        let end = self.h - bottom;
        let added = dy.min(MAX_HEIGHT.saturating_sub(self.height()));
        self.out
            .extend_from_slice(&frame.as_raw()[(end - dy) * row..(end - dy + added) * row]);
        self.prev = frame.as_raw().clone();
        self.prev_rows = cur;
        Step::Added(added)
    }

    pub fn finish(mut self) -> RgbaImage {
        let bottom = self.bands.map_or(0, |(_, bottom)| bottom);
        let row = self.w * 4;
        self.out
            .extend_from_slice(&self.prev[(self.h - bottom) * row..]);
        let h = self.out.len() / (self.w * 4);
        RgbaImage::from_raw(self.w as u32, h as u32, self.out).expect("whole rows")
    }
}

/// Row hashes, leaving out the right edge, where a scroll bar comes and goes.
fn rows(px: &[u8], w: usize, h: usize) -> Vec<Row> {
    let margin = (w / 20).min(40);
    let used = (w - margin).max(1) * 4;
    (0..h)
        .map(|y| {
            let line = &px[y * w * 4..][..used];
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            line.hash(&mut hasher);
            Row {
                hash: hasher.finish(),
                plain: line.as_chunks::<4>().0.iter().all(|p| p[..] == line[..4]),
            }
        })
        .collect()
}

/// Rows at the top and bottom that stayed put: a sticky header and footer.
fn bands(prev: &[Row], cur: &[Row]) -> (usize, usize) {
    let h = cur.len();
    let same = |y: usize| prev[y] == cur[y];
    let top = (0..h).take_while(|&y| same(y)).count();
    let bottom = (0..h).rev().take_while(|&y| same(y)).count();
    // Keep at least a third of the region scrolling.
    if top + bottom > h * 2 / 3 {
        (0, 0)
    } else {
        (top, bottom)
    }
}

/// How far the rows in `start..end` moved up from `prev` to `cur`.
fn shift(prev: &[Row], cur: &[Row], start: usize, end: usize) -> Option<usize> {
    let n = end.saturating_sub(start);
    let min_overlap = (n / 5).max(8);
    let mut best: Option<(f64, usize)> = None;
    for dy in 1..n.saturating_sub(min_overlap) {
        let (mut matched, mut counted) = (0usize, 0usize);
        for i in start..end - dy {
            let (c, p) = (cur[i], prev[i + dy]);
            if c.plain && p.plain {
                continue;
            }
            counted += 1;
            matched += (c == p) as usize;
        }
        if counted < 4 {
            continue;
        }
        let score = matched as f64 / counted as f64;
        // Smallest shift wins ties: between frames taken moments apart,
        // a short scroll is the likelier one.
        if score >= 0.9 && best.is_none_or(|(s, _)| score > s + 0.01) {
            best = Some((score, dy));
        }
    }
    best.map(|(_, dy)| dy)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tall page of distinct rows, some of them blank like real pages.
    fn page(w: u32, h: u32) -> RgbaImage {
        let mut seed = 7u32;
        let mut img = RgbaImage::new(w, h);
        for y in 0..h {
            let blank = y % 37 < 6;
            for x in 0..w {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                let v = if blank { 240 } else { (seed >> 16) as u8 };
                img.put_pixel(x, y, [v, v / 2, 255 - v, 255].into());
            }
        }
        img
    }

    /// The window `h` rows tall at `y`, with a fixed header and footer drawn over it.
    fn view(page: &RgbaImage, y: u32, h: u32, header: u32, footer: u32) -> RgbaImage {
        let mut v = xcap::image::imageops::crop_imm(page, 0, y, page.width(), h).to_image();
        for yy in (0..header).chain(h - footer..h) {
            for x in 0..page.width() {
                v.put_pixel(x, yy, [10, (yy % 255) as u8, 200, 255].into());
            }
        }
        v
    }

    #[test]
    fn rebuilds_the_page() {
        let p = page(60, 1000);
        let mut s = Stitcher::new(&view(&p, 0, 200, 0, 0));
        for y in [37, 90, 90, 150, 260, 400, 520, 640, 780, 800] {
            s.push(&view(&p, y, 200, 0, 0));
        }
        let out = s.finish();
        assert_eq!(out.height(), 1000);
        assert_eq!(out.as_raw(), p.as_raw());
    }

    #[test]
    fn keeps_sticky_header_and_footer_once() {
        let p = page(60, 1000);
        let (header, footer) = (20, 15);
        let mut s = Stitcher::new(&view(&p, 0, 200, header, footer));
        assert_eq!(s.push(&view(&p, 0, 200, header, footer)), Step::Same);
        for y in [50, 120, 200, 330] {
            assert!(matches!(
                s.push(&view(&p, y, 200, header, footer)),
                Step::Added(_)
            ));
        }
        let out = s.finish();
        // Header, the page from under the header to the last frame's footer, footer.
        assert_eq!(out.height(), 330 + 200);
        let expected = view(&p, 0, 200, header, footer);
        assert_eq!(out.get_pixel(5, 0), expected.get_pixel(5, 0));
        assert_eq!(
            out.get_pixel(5, out.height() - 1),
            expected.get_pixel(5, 199)
        );
        // A row from the middle of the page lands where it belongs.
        assert_eq!(out.get_pixel(9, 300), p.get_pixel(9, 300));
    }

    #[test]
    fn ignores_jumps_too_far_and_scrolling_up() {
        let p = page(60, 1000);
        let mut s = Stitcher::new(&view(&p, 300, 200, 0, 0));
        assert_eq!(s.push(&view(&p, 600, 200, 0, 0)), Step::Lost);
        assert_eq!(s.push(&view(&p, 250, 200, 0, 0)), Step::Lost);
        assert_eq!(s.push(&view(&p, 350, 200, 0, 0)), Step::Added(50));
        assert_eq!(s.height(), 250);
    }

    #[test]
    fn a_scroll_bar_does_not_break_matching() {
        let p = page(200, 1000);
        let mut s = Stitcher::new(&view(&p, 0, 200, 0, 0));
        let mut next = view(&p, 60, 200, 0, 0);
        for y in 0..200 {
            for x in 192..200 {
                next.put_pixel(x, y, [128, 128, 128, 255].into());
            }
        }
        assert_eq!(s.push(&next), Step::Added(60));
    }
}
