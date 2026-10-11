//! Stitches frames of a region the user scrolls into one tall image. Each
//! new frame is matched against the previous one by comparing row hashes:
//! the shift that lines the rows up tells how far the content moved, and
//! the rows that came into view are appended. Rows that never move (a
//! sticky header or footer) are kept once. New rows are taken from above
//! the bottom of the frame, where floating buttons and banners sit, so they
//! appear once, at the end, rather than in every step.

use std::hash::{Hash, Hasher};

use xcap::image::RgbaImage;

/// The tallest result, in pixels.
pub const MAX_HEIGHT: usize = 30_000;

#[derive(Debug, PartialEq)]
pub enum Step {
    /// The content moved and this many rows were added.
    Added(usize),
    /// Nothing moved: the end of the content, or not scrolled yet. Something
    /// small may have changed in place, like an animation.
    Same,
    /// No match: scrolled too far between frames, or upwards.
    Lost,
    /// The image reached `MAX_HEIGHT`.
    Full,
}

pub struct Stitcher {
    w: usize,
    h: usize,
    /// The image so far, without the frame's bottom zone (see `keep`).
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

    /// The height of the captured region.
    pub fn region_height(&self) -> usize {
        self.h
    }

    /// The height of the result so far.
    pub fn height(&self) -> usize {
        self.out.len() / (self.w * 4) + self.bands.map_or(0, |_| self.keep())
    }

    /// Rows at the bottom of each frame that new content is not taken from:
    /// the sticky footer, or a floating "back to top" button, chat bubble or
    /// cookie banner that a footer check can't see because it comes and goes
    /// or shows the page through it. The last frame's go at the very end.
    fn keep(&self) -> usize {
        let bottom = self.bands.map_or(0, |(_, bottom)| bottom);
        bottom.max(self.h * 15 / 100)
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
        // Failing that, leave out the bottom zone, where things float over
        // the page (see `keep`): it allows shorter jumps only.
        let keep = bottom.max(self.h * 15 / 100);
        let Some(dy) = shift(&self.prev_rows, &cur, top, self.h - bottom)
            .or_else(|| shift(&self.prev_rows, &cur, top, self.h - keep))
        else {
            return Step::Lost;
        };
        if dy == 0 {
            return Step::Same;
        }
        let row = self.w * 4;
        if self.bands.is_none() {
            // The first frame's bottom zone ends `out`; the last frame's goes
            // under everything when done.
            self.bands = Some((top, bottom));
            self.out.truncate(self.out.len() - self.keep() * row);
        }
        let end = self.h - self.keep();
        let dy = dy.min(end);
        let added = dy.min(MAX_HEIGHT.saturating_sub(self.height()));
        self.out
            .extend_from_slice(&frame.as_raw()[(end - dy) * row..(end - dy + added) * row]);
        self.prev = frame.as_raw().clone();
        self.prev_rows = cur;
        Step::Added(added)
    }

    pub fn finish(mut self) -> RgbaImage {
        let keep = self.bands.map_or(0, |_| self.keep());
        let row = self.w * 4;
        self.out
            .extend_from_slice(&self.prev[(self.h - keep) * row..]);
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

/// How far the rows in `start..end` moved up from `prev` to `cur`. The best
/// shift needn't match every row: a see-through sticky header changes with
/// the page behind it, and animations play as content comes into view. It
/// must match most of them and clearly beat every other shift.
fn shift(prev: &[Row], cur: &[Row], start: usize, end: usize) -> Option<usize> {
    let n = end.saturating_sub(start);
    let min_overlap = (n / 5).max(8);
    let mut scores = Vec::new();
    // From 0: a frame that didn't move but changed a little in place (an
    // animation, a caret) must read as still, not as lost.
    for dy in 0..n.saturating_sub(min_overlap) {
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
        scores.push((matched as f64 / counted as f64, dy));
    }
    // Smallest shift wins ties: between frames taken moments apart, a short
    // scroll is the likelier one.
    let (score, dy) =
        scores
            .iter()
            .copied()
            .fold(None, |best: Option<(f64, usize)>, (s, dy)| match best {
                Some((b, _)) if s <= b + 0.01 => best,
                _ => Some((s, dy)),
            })?;
    // The runner-up away from the best, whose neighbors share its rows.
    let runner_up = scores
        .iter()
        .filter(|(_, d)| d.abs_diff(dy) > 2)
        .map(|(s, _)| *s)
        .fold(0.0, f64::max);
    (score >= 0.9 || (score >= 0.5 && runner_up < score - 0.3)).then_some(dy)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A tall page of distinct rows, some of them blank like real pages.
    pub(crate) fn page(w: u32, h: u32) -> RgbaImage {
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
    pub(crate) fn view(page: &RgbaImage, y: u32, h: u32, header: u32, footer: u32) -> RgbaImage {
        let mut v = xcap::image::imageops::crop_imm(page, 0, y, page.width(), h).to_image();
        for yy in (0..header).chain(h - footer..h) {
            for x in 0..page.width() {
                v.put_pixel(x, yy, [10, (yy % 255) as u8, 200, 255].into());
            }
        }
        v
    }

    /// A view under a see-through header that tints the page behind it, with
    /// a "back to top" button floating at the bottom once scrolled.
    fn modern_view(page: &RgbaImage, y: u32, h: u32) -> RgbaImage {
        let mut v = view(page, y, h, 0, 0);
        for yy in 0..h / 10 {
            for x in 0..page.width() {
                let p = v.get_pixel(x, yy).0;
                v.put_pixel(
                    x,
                    yy,
                    [p[0] / 4 + 190, p[1] / 4 + 190, p[2] / 4 + 190, 255].into(),
                );
            }
        }
        if y > 0 {
            for yy in h - h / 8..h - h / 20 {
                for x in page.width() / 2..page.width() * 3 / 4 {
                    v.put_pixel(x, yy, [200, 60, 60, 255].into());
                }
            }
        }
        v
    }

    #[test]
    fn copes_with_a_see_through_header_and_a_floating_button() {
        let p = page(80, 1600);
        let h = 300;
        let mut s = Stitcher::new(&modern_view(&p, 0, h));
        let mut y = 0;
        while y + h < p.height() {
            y = (y + 140).min(p.height() - h);
            assert!(
                matches!(s.push(&modern_view(&p, y, h)), Step::Added(_)),
                "lost at {y}"
            );
        }
        let out = s.finish();
        assert_eq!(out.height(), p.height());
        // Below the header and above the last frame's bottom zone, it's the
        // page itself: the button shows once, at the very end.
        let row = p.width() as usize * 4;
        let (from, to) = (h as usize / 10, p.height() as usize - h as usize * 15 / 100);
        assert_eq!(
            out.as_raw()[from * row..to * row],
            p.as_raw()[from * row..to * row]
        );
    }

    #[test]
    fn an_animation_in_place_reads_as_still() {
        let p = page(80, 900);
        let mut s = Stitcher::new(&view(&p, 0, 300, 0, 0));
        assert!(matches!(
            s.push(&view(&p, 120, 300, 0, 0)),
            Step::Added(120)
        ));
        // At the end of the page, something blinks: still, not lost.
        let mut blink = view(&p, 120, 300, 0, 0);
        for y in 140..160 {
            for x in 10..30 {
                blink.put_pixel(x, y, [255, 0, 0, 255].into());
            }
        }
        assert_eq!(s.push(&blink), Step::Same);
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
