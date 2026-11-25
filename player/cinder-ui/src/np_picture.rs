//! What Pivot's Now Playing shows in its picture block: the cover, or one of three moving pictures
//! (Settings ▸ Now Playing ▸ Picture): a pair of backlit analog VU meters across the block's width,
//! a record turning, or a cassette that takes the whole panel on its side, the controls left to
//! the player's buttons and the tape's own taps. The Cover choice falls back to the record when a
//! song has no cover, which is where a moving picture earns its place most.
//!
//! The meters and the record sit in the block above the title, so the layout around them never
//! changes with the choice. Motion
//! comes from `NowPlaying::viz_seed`, which the shell advances in real time while the song plays
//! (`App::np_picture_turns`); the meters read the PCM tap's levels (`App::np_picture_meters`).

use std::f32::consts::PI;

use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Circle, Line, PrimitiveStyle};

use crate::canvas::Canvas;
use crate::now_playing::NowPlaying;
use crate::text::{self, Family, FontSet, TextStyle, Weight};
use crate::theme::Theme;
use crate::widgets::{self, fill_rect, stroke_rect};

/// The choice, as the settings file and the Settings row name it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NpPicture {
    /// The album cover; a record when the song has none.
    #[default]
    Cover,
    Meters,
    Record,
    Tape,
}

impl NpPicture {
    pub const ALL: [NpPicture; 4] = [NpPicture::Cover, NpPicture::Meters, NpPicture::Record, NpPicture::Tape];

    /// The settings-file word.
    pub fn token(self) -> &'static str {
        match self {
            NpPicture::Cover => "cover",
            NpPicture::Meters => "meters",
            NpPicture::Record => "record",
            NpPicture::Tape => "tape",
        }
    }

    pub fn from_token(s: &str) -> NpPicture {
        crate::from_token(&Self::ALL, Self::token, s)
    }

    pub fn next(self) -> NpPicture {
        crate::next_in(&Self::ALL, self)
    }

    /// The Settings row's value.
    pub fn label(self) -> &'static str {
        match self {
            NpPicture::Cover => crate::i18n::text("COVER"),
            NpPicture::Meters => crate::i18n::text("VU METERS"),
            NpPicture::Record => crate::i18n::text("RECORD"),
            NpPicture::Tape => crate::i18n::text("TAPE"),
        }
    }

    /// What is drawn for a song with (`has_cover`) or without a cover.
    pub fn shown(self, has_cover: bool) -> NpPicture {
        match self {
            NpPicture::Cover if !has_cover => NpPicture::Record,
            p => p,
        }
    }

    /// Does it move with the music on the shell's clock (the record and the tape)?
    pub fn turns(self) -> bool {
        matches!(self, NpPicture::Record | NpPicture::Tape)
    }
}

fn st(size: f32, w: Weight, col: Rgb888) -> TextStyle {
    widgets::sty(Family::Legible, w, size, col, 0.0)
}

fn line(c: &mut Canvas, a: (f32, f32), b: (f32, f32), col: Rgb888, w: u32) {
    Line::new(Point::new(a.0.round() as i32, a.1.round() as i32), Point::new(b.0.round() as i32, b.1.round() as i32))
        .into_styled(PrimitiveStyle::with_stroke(col, w))
        .draw(c)
        .ok();
}

fn ring(c: &mut Canvas, cx: f32, cy: f32, r: f32, col: Rgb888, w: u32) {
    Circle::with_center(Point::new(cx.round() as i32, cy.round() as i32), (2.0 * r).round() as u32)
        .into_styled(PrimitiveStyle::with_stroke(col, w))
        .draw(c)
        .ok();
}

/// An arc of radius `r` about (`cx`, `cy`) from angle `a0` to `a1` (radians, 0 = right, counter-
/// clockwise positive, screen y down), as short segments.
fn arc(c: &mut Canvas, cx: f32, cy: f32, r: f32, a0: f32, a1: f32, col: Rgb888, w: u32) {
    let steps = ((a1 - a0).abs() * r / 6.0).ceil().max(2.0) as usize;
    let at = |a: f32| (cx + r * a.cos(), cy - r * a.sin());
    for k in 0..steps {
        let (p, q) = (a0 + (a1 - a0) * k as f32 / steps as f32, a0 + (a1 - a0) * (k + 1) as f32 / steps as f32);
        line(c, at(p), at(q), col, w);
    }
}

/// `a` → `b` by `t` (0..1), per channel.
fn mix(a: Rgb888, b: Rgb888, t: f32) -> Rgb888 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Rgb888::new(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

// ── VU meters ────────────────────────────────────────────────────────────────────────────────

/// The scale's marks, in VU.
const VU_MARKS: [f32; 10] = [-20.0, -10.0, -7.0, -5.0, -3.0, -1.0, 0.0, 1.0, 2.0, 3.0];
/// How far either side of upright the scale sweeps.
const VU_SWEEP: f32 = 48.0 * PI / 180.0;
/// dBFS that reads 0 VU: the usual studio reference.
const VU_REF_DBFS: f32 = -18.0;

/// Where `vu` sits along the scale, 0 (−20) to 1 (+3). A VU meter's scale is linear in voltage,
/// not in dB: that is what crowds −20…−7 into the left third and spreads 0…+3 across the red.
pub fn vu_pos(vu: f32) -> f32 {
    let v = |db: f32| 10f32.powf(db / 20.0);
    ((v(vu.clamp(-20.0, 3.5)) - v(-20.0)) / (v(3.0) - v(-20.0))).clamp(0.0, 1.04)
}

/// The two needles' levels in VU: the PCM tap's RMS, left and right; else the analyzer's band
/// average on both; else at rest.
pub fn vu_levels(np: &NowPlaying) -> (f32, f32) {
    let from = |m: f32| -60.0 + 60.0 * m.clamp(0.0, 1.0) - VU_REF_DBFS;
    if let Some(s) = np.viz_sig.filter(|s| s.meter.iter().any(|m| *m > 0.0)) {
        return (from(s.meter[2]), from(s.meter[3]));
    }
    if let Some(l) = np.viz_levels.filter(|l| !l.is_empty()) {
        let mean = l.iter().sum::<f32>() / l.len() as f32;
        return (from(mean), from(mean));
    }
    (-30.0, -30.0)
}

/// `col` at the theme's night dimming, as the covers are.
fn lit(t: &Theme, col: Rgb888) -> Rgb888 {
    let d = t.dim_pct.min(100);
    let k = |v: u8| (v as u32 * d / 100) as u8;
    Rgb888::new(k(col.r()), k(col.g()), k(col.b()))
}

/// The meters across the block's full width (`x0`..`x1`), stacked, in `y`..`y + h`.
pub fn meters_wide(c: &mut Canvas, t: &Theme, f: &FontSet, np: &NowPlaying, x0: i32, x1: i32, y: i32, h: i32) {
    let (l, r) = if np.playing { vu_levels(np) } else { (-30.0, -30.0) };
    let gap = 12;
    let mh = (h - gap) / 2;
    for (k, (level, name)) in [(l, "L"), (r, "R")].into_iter().enumerate() {
        meter(c, t, f, x0, y + k as i32 * (mh + gap), x1 - x0, mh, level, name);
    }
}

/// A warm backlit face: brightest low in the middle, where the lamp sits, falling to amber at the
/// edges. Written straight into the buffer (a meter is ~70k pixels, twice a frame).
fn backlight(c: &mut Canvas, t: &Theme, x: i32, y: i32, w: i32, h: i32) {
    let glow = lit(t, Rgb888::new(255, 214, 140));
    let edge = lit(t, Rgb888::new(196, 116, 38));
    let (cx, cy) = (x as f32 + w as f32 / 2.0, y as f32 + h as f32 * 0.9);
    let cw = crate::canvas::W;
    for py in y.max(0)..(y + h).min(crate::canvas::H as i32) {
        let ny = (py as f32 - cy) / h as f32;
        for px in x.max(0)..(x + w).min(cw as i32) {
            let nx = (px as f32 - cx) / (w as f32 / 2.0);
            let k = (1.0 - 0.55 * nx * nx - 0.9 * ny * ny).clamp(0.0, 1.0);
            c.buf[py as usize * cw + px as usize] = crate::canvas::to_u32(mix(edge, glow, k));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn meter(c: &mut Canvas, t: &Theme, f: &FontSet, x: i32, y: i32, w: i32, h: i32, vu: f32, name: &str) {
    // The bezel, then the lit face inside it.
    fill_rect(c, x, y, w, h, lit(t, Rgb888::new(28, 22, 16)));
    let b = 5;
    backlight(c, t, x + b, y + b, w - 2 * b, h - 2 * b);
    let ink = lit(t, Rgb888::new(58, 34, 14));
    let red = lit(t, Rgb888::new(176, 38, 24));
    let needle = lit(t, Rgb888::new(26, 18, 10));
    // The needle pivots below the face, so only the top of its swing shows.
    let (cx, cy) = (x as f32 + w as f32 / 2.0, y as f32 + h as f32 * 1.25);
    let rad = h as f32 * 1.0;
    let angle = |p: f32| PI / 2.0 + VU_SWEEP - 2.0 * VU_SWEEP * p;
    // The scale: the arc, the red zone from 0 to +3, and a tick per mark with its number.
    arc(c, cx, cy, rad, angle(1.0), angle(0.0), ink, 2);
    arc(c, cx, cy, rad + 4.0, angle(1.0), angle(vu_pos(0.0)), red, 6);
    let nums = st(13.0, Weight::Regular, ink);
    for m in VU_MARKS {
        let a = angle(vu_pos(m));
        let major = m == -20.0 || m == -10.0 || m == 0.0 || m == 3.0 || m == -5.0;
        let len = if major { 12.0 } else { 7.0 };
        let col = if m > 0.0 { red } else { ink };
        line(c, (cx + rad * a.cos(), cy - rad * a.sin()), (cx + (rad + len) * a.cos(), cy - (rad + len) * a.sin()), col, 2);
        let label = if m > 0.0 { format!("+{}", m as i32) } else { format!("{}", m as i32) };
        let (lx, ly) = (cx + (rad + 26.0) * a.cos(), cy - (rad + 26.0) * a.sin());
        widgets::center(c, f, lx, ly + 5.0, &label, &TextStyle { color: col, ..nums });
    }
    // The channel at the bottom left, "VU" at the bottom right: clear of the needle's swing.
    text::draw(c, f, (x + b + 12) as f32, (y + h - b - 10) as f32, name, &st(15.0, Weight::Regular, ink));
    widgets::right(c, f, (x + w - b - 12) as f32, (y + h - b - 10) as f32, "VU", &st(18.0, Weight::SemiBold, ink));
    // The needle, from inside the face up past the scale; clipped to the face.
    let a = angle(vu_pos(vu));
    c.set_clip_y(y + b, y + h - b);
    line(c, (cx + rad * 0.45 * a.cos(), cy - rad * 0.45 * a.sin()), (cx + (rad + 10.0) * a.cos(), cy - (rad + 10.0) * a.sin()), needle, 2);
    c.clear_clip();
}

// ── Record ───────────────────────────────────────────────────────────────────────────────────

/// Turns a second: `viz_seed` advances 3.6 a second while playing, so one radian of it a radian of
/// the record is very nearly 33⅓ rpm (3.49 rad/s).
fn spin(np: &NowPlaying) -> f32 {
    if np.playing { np.viz_seed } else { 0.0 }
}

/// The record, radius `r` about (`cx`, `cy`): the disc and its grooves, a sheen that stays where
/// the light is, and the label turning (the cover, or the accent with a mark).
pub fn record(c: &mut Canvas, t: &Theme, np: &NowPlaying, cx: f32, cy: f32, r: f32) {
    // Vinyl is black in either theme; the grooves a shade off it.
    let vinyl = Rgb888::new(22, 22, 24);
    let groove = Rgb888::new(44, 44, 48);
    widgets::disc(c, cx as i32, cy as i32, (2.0 * r) as u32, vinyl);
    let label = r * 0.36;
    let mut g = label + 10.0;
    while g < r - 3.0 {
        ring(c, cx, cy, g, groove, 1);
        g += if ((g as i32) / 23) % 3 == 0 { 7.0 } else { 3.0 }; // track gaps, wider
    }
    for (a0, a1) in [(0.6f32, 0.95f32), (0.6 + PI, 0.95 + PI)] {
        for k in 0..6 {
            arc(c, cx, cy, label + 16.0 + k as f32 * (r - label - 24.0) / 6.0, a0, a1, Rgb888::new(70, 70, 76), 1);
        }
    }
    let turn = spin(np);
    match np.art_full.or(np.art_thumb) {
        Some(img) => label_image(c, img, cx, cy, label, turn),
        None => {
            widgets::disc(c, cx as i32, cy as i32, (2.0 * label) as u32, t.acc);
            let (s, co) = (turn.sin(), turn.cos());
            line(c, (cx + label * 0.45 * co, cy + label * 0.45 * s), (cx + label * 0.85 * co, cy + label * 0.85 * s), mix(t.acc, t.bg, 0.6), 3);
        }
    }
    widgets::disc(c, cx as i32, cy as i32, 8, t.bg);
}

/// `img` as a round label of radius `r` at (`cx`, `cy`), turned by `turn` radians: the centre
/// square of the cover, sampled nearest, every pixel of the circle.
fn label_image(c: &mut Canvas, img: &crate::art::Image, cx: f32, cy: f32, r: f32, turn: f32) {
    let side = img.w.min(img.h) as f32;
    let (ox, oy) = ((img.w as f32 - side) / 2.0, (img.h as f32 - side) / 2.0);
    let scale = side / (2.0 * r);
    let (s, co) = (turn.sin(), turn.cos());
    let ri = r.ceil() as i32;
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            let (fx, fy) = (dx as f32, dy as f32);
            if fx * fx + fy * fy > r * r {
                continue;
            }
            // Turn the sample point back by `turn`, so the picture turns forward.
            let (sx, sy) = (fx * co + fy * s, -fx * s + fy * co);
            let ix = (ox + (sx + r) * scale) as usize;
            let iy = (oy + (sy + r) * scale) as usize;
            if ix >= img.w || iy >= img.h {
                continue;
            }
            let k = (iy * img.w + ix) * 3;
            let col = Rgb888::new(img.rgb[k], img.rgb[k + 1], img.rgb[k + 2]);
            Pixel(Point::new(cx as i32 + dx, cy as i32 + dy), col).draw(c).ok();
        }
    }
}

// ── Tape: the whole panel, on its side ───────────────────────────────────────────────────────
//
// The player turned sideways is a cassette: 800 × 480, near enough a real one's 1.6:1. Drawn in
// the tape's landscape pixels (`crate::tape::LW` × `LH`, the player's top on the left, as the
// Cassette style's tapes are) and mapped to the panel by `land_*`. Nothing on it is a control: a
// tap is the tape's own (`tape::tap_at`: ◁ ▶/‖ ▷ by thirds, the Menu at the top right).

/// A landscape rectangle, filled on the panel.
fn land_rect(c: &mut Canvas, u: i32, v: i32, w: i32, h: i32, col: Rgb888) {
    fill_rect(c, crate::canvas::W as i32 - v - h, u, h, w, col);
}

fn land_stroke(c: &mut Canvas, u: i32, v: i32, w: i32, h: i32, col: Rgb888, t: u32) {
    stroke_rect(c, crate::canvas::W as i32 - v - h, u, h, w, col, t);
}

fn land_disc(c: &mut Canvas, u: f32, v: f32, d: u32, col: Rgb888) {
    widgets::disc(c, (crate::canvas::W as f32 - 1.0 - v).round() as i32, u.round() as i32, d, col);
}

fn land_ring(c: &mut Canvas, u: f32, v: f32, r: f32, col: Rgb888, w: u32) {
    ring(c, crate::canvas::W as f32 - 1.0 - v, u, r, col, w);
}

fn land_text(c: &mut Canvas, f: &FontSet, u: f32, base: f32, s: &str, st: &TextStyle, max_w: f32) {
    text::draw_land(c, f, (u, base), &widgets::fit(f, s, st, max_w), st, max_w);
}

/// The cassette, filling the panel.
pub fn tape_land(c: &mut Canvas, t: &Theme, f: &FontSet, np: &NowPlaying) {
    let (lw, lh) = (crate::tape::LW, crate::tape::LH);
    let deck = lit(t, Rgb888::new(12, 12, 14));
    let shell = lit(t, Rgb888::new(38, 38, 44));
    let edge = lit(t, Rgb888::new(64, 64, 72));
    let paper = lit(t, Rgb888::new(236, 228, 208));
    let ink = lit(t, Rgb888::new(34, 30, 26));
    let soft = lit(t, Rgb888::new(110, 100, 88));
    let tape_col = lit(t, Rgb888::new(70, 46, 30));
    let stripe = lit(t, t.acc);
    c.fill(deck);
    // The shell, its screws.
    land_rect(c, 10, 10, lw - 20, lh - 20, shell);
    land_stroke(c, 10, 10, lw - 20, lh - 20, edge, 2);
    for (u, v) in [(38, 38), (lw - 38, 38), (38, lh - 38), (lw - 38, lh - 38), (lw / 2, lh - 26)] {
        land_disc(c, u as f32, v as f32, 16, edge);
        land_disc(c, u as f32, v as f32, 6, shell);
    }
    // The label: a big A, the song, a stripe, the time.
    let (lu, lv, luw, lvh) = (64, 34, lw - 128, 268);
    land_rect(c, lu, lv, luw, lvh, paper);
    land_rect(c, lu, lv + 186, luw, 14, stripe);
    land_rect(c, lu, lv + 204, luw, 5, mix(stripe, paper, 0.45));
    land_stroke(c, lu + 14, lv + 14, 58, 58, ink, 2);
    let big = |size: f32, w: Weight, col: Rgb888| widgets::sty(Family::Legible, w, size, col, 0.0);
    text::draw_land(c, f, ((lu + 30) as f32, (lv + 58) as f32), "A", &big(36.0, Weight::SemiBold, ink), 40.0);
    let tx = (lu + 92) as f32;
    let tw = (luw - 92 - 24) as f32;
    let quiet = np.title.is_empty() && np.artist.is_empty();
    if quiet {
        land_text(c, f, tx, (lv + 56) as f32, crate::i18n::text("No tape"), &big(32.0, Weight::Regular, soft), tw);
    } else {
        land_text(c, f, tx, (lv + 52) as f32, np.title, &big(32.0, Weight::Regular, ink), tw);
        land_text(c, f, tx, (lv + 92) as f32, np.artist, &big(24.0, Weight::Regular, ink), tw);
        if !np.album.is_empty() {
            land_text(c, f, tx, (lv + 124) as f32, np.album, &big(17.0, Weight::Regular, soft), tw);
        }
        let time = format!("{}   {}", np.elapsed, np.remaining);
        let ts = big(15.0, Weight::Regular, soft);
        let w = text::measure(f, &time, &ts);
        text::draw_land(c, f, ((lu + luw - 20) as f32 - w, (lv + lvh - 18) as f32), &time, &ts, w + 2.0);
    }
    // The window through the label: the tape packed on each reel (all on the left at the start
    // of the song, all on the right at its end), the hubs and their teeth turning.
    let (wu, wv, ww, wh) = (lw / 2 - 170, lv + 214 - 64, 340, 104);
    land_rect(c, wu, wv, ww, wh, lit(t, Rgb888::new(18, 18, 20)));
    land_stroke(c, wu, wv, ww, wh, ink, 2);
    let p = np.progress.clamp(0.0, 1.0);
    let cv = (wv + wh / 2) as f32;
    let (rmin, rmax) = (24.0f32, 48.0f32);
    let turn = spin(np) * 2.0;
    for (cu, packed) in [((wu + 82) as f32, 1.0 - p), ((wu + ww - 82) as f32, p)] {
        let r = rmin + (rmax - rmin) * packed;
        land_disc(c, cu, cv, (2.0 * r) as u32, tape_col);
        land_disc(c, cu, cv, 2 * rmin as u32 - 2, lit(t, Rgb888::new(225, 222, 214)));
        land_disc(c, cu, cv, 18, lit(t, Rgb888::new(18, 18, 20)));
        // The hub's teeth, dark on the pale hub, so its turning shows.
        for k in 0..6 {
            let a = turn + k as f32 * PI / 3.0;
            land_disc(c, cu + 14.0 * a.cos(), cv + 14.0 * a.sin(), 8, lit(t, Rgb888::new(18, 18, 20)));
        }
        land_ring(c, cu, cv, rmin, edge, 1);
    }
    // The head opening along the bottom edge, the tape across it, the rollers.
    let (hv, hb) = (lh - 120, lh - 12);
    for v in hv..hb {
        let k = (v - hv) as f32 / (hb - hv) as f32;
        // Bevelled: narrowing toward the edge, as a cassette's is.
        let inset = (30.0 + 40.0 * k) as i32;
        land_rect(c, 190 + inset, v, lw - 380 - 2 * inset, 1, lit(t, Rgb888::new(26, 26, 30)));
    }
    land_rect(c, 160, hb - 14, lw - 320, 3, tape_col);
    for u in [250.0, (lw - 250) as f32] {
        land_disc(c, u, (hb - 40) as f32, 30, edge);
        land_disc(c, u, (hb - 40) as f32, 14, deck);
    }
    land_rect(c, lw / 2 - 30, hb - 34, 60, 22, edge);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_vu_scale_is_spaced_like_a_real_one() {
        assert!(vu_pos(-20.0) < 0.01 && (vu_pos(3.0) - 1.0).abs() < 1e-4);
        // 0 VU sits about 70 % of the way along, and −10 inside the left quarter.
        assert!((0.65..0.75).contains(&vu_pos(0.0)), "{}", vu_pos(0.0));
        assert!(vu_pos(-10.0) < 0.25, "{}", vu_pos(-10.0));
        assert!(vu_pos(-40.0) == vu_pos(-20.0), "below the scale rests on its end");
    }

    #[test]
    fn the_picture_choice_round_trips_and_falls_back_without_a_cover() {
        for p in NpPicture::ALL {
            assert_eq!(NpPicture::from_token(p.token()), p);
        }
        assert_eq!(NpPicture::from_token("nonsense"), NpPicture::Cover);
        assert_eq!(NpPicture::Cover.shown(false), NpPicture::Record);
        assert_eq!(NpPicture::Cover.shown(true), NpPicture::Cover);
        assert_eq!(NpPicture::Tape.shown(true), NpPicture::Tape);
        assert!(NpPicture::Record.turns() && NpPicture::Tape.turns() && !NpPicture::Meters.turns());
    }
}
