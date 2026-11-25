//! What Pivot's Now Playing shows in its picture square: the cover, or one of three moving
//! pictures (Settings ▸ Now Playing ▸ Picture): a pair of analog VU meters, a record turning on a
//! platter, or a cassette winding. The Cover choice falls back to the record when a song has no
//! cover, which is where a moving picture earns its place most.
//!
//! Everything is drawn from the theme's colours in Pivot's thin-line way, and every one fits a
//! `size` square at (`x`, `y`), so the layout around it never changes with the choice. Motion
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

/// Draw `p` for `np` in the `size` square at (`x`, `y`).
pub fn draw(c: &mut Canvas, t: &Theme, f: &FontSet, np: &NowPlaying, p: NpPicture, x: i32, y: i32, size: i32) {
    match p {
        NpPicture::Cover => {}
        NpPicture::Meters => meters(c, t, f, np, x, y, size),
        NpPicture::Record => record(c, t, np, x, y, size),
        NpPicture::Tape => tape(c, t, f, np, x, y, size),
    }
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

fn meters(c: &mut Canvas, t: &Theme, f: &FontSet, np: &NowPlaying, x: i32, y: i32, size: i32) {
    let (l, r) = if np.playing { vu_levels(np) } else { (-30.0, -30.0) };
    let gap = 12;
    let h = (size - gap) / 2;
    for (k, (level, name)) in [(l, "L"), (r, "R")].into_iter().enumerate() {
        meter(c, t, f, x, y + k as i32 * (h + gap), size, h, level, name);
    }
}

#[allow(clippy::too_many_arguments)]
fn meter(c: &mut Canvas, t: &Theme, f: &FontSet, x: i32, y: i32, w: i32, h: i32, vu: f32, name: &str) {
    // The face: a slightly warm panel, as the backlit meters were.
    fill_rect(c, x, y, w, h, mix(t.bg, t.panel, 0.9));
    stroke_rect(c, x, y, w, h, t.line, 1);
    // The needle pivots below the face, so only the top of its swing shows.
    let (cx, cy) = (x as f32 + w as f32 / 2.0, y as f32 + h as f32 * 1.22);
    let rad = h as f32 * 0.98;
    let angle = |p: f32| PI / 2.0 + VU_SWEEP - 2.0 * VU_SWEEP * p;
    // The scale: the arc, the red zone from 0 to +3, and a tick per mark with its number.
    arc(c, cx, cy, rad, angle(1.0), angle(0.0), t.dim, 1);
    arc(c, cx, cy, rad + 3.0, angle(1.0), angle(vu_pos(0.0)), t.acc, 4);
    let nums = st(11.0, Weight::Regular, t.dim);
    for m in VU_MARKS {
        let a = angle(vu_pos(m));
        let major = m == -20.0 || m == -10.0 || m == 0.0 || m == 3.0 || m == -5.0;
        let len = if major { 10.0 } else { 6.0 };
        let col = if m > 0.0 { t.acc } else { t.dim };
        line(c, (cx + rad * a.cos(), cy - rad * a.sin()), (cx + (rad + len) * a.cos(), cy - (rad + len) * a.sin()), col, if major { 2 } else { 1 });
        let label = if m > 0.0 { format!("+{}", m as i32) } else { format!("{}", m as i32) };
        let (lx, ly) = (cx + (rad + 22.0) * a.cos(), cy - (rad + 22.0) * a.sin());
        widgets::center(c, f, lx, ly + 4.0, &label, &TextStyle { color: col, ..nums });
    }
    // The channel at the bottom left, "VU" at the bottom right: clear of the needle's swing.
    text::draw(c, f, x as f32 + 10.0, (y + h - 10) as f32, name, &st(13.0, Weight::Regular, t.dim));
    widgets::right(c, f, (x + w - 10) as f32, (y + h - 10) as f32, "VU", &st(15.0, Weight::Regular, t.ink));
    // The needle, from inside the face up past the scale; clipped to the face.
    let a = angle(vu_pos(vu));
    c.set_clip_y(y + 1, y + h - 1);
    line(c, (cx + rad * 0.42 * a.cos(), cy - rad * 0.42 * a.sin()), (cx + (rad + 8.0) * a.cos(), cy - (rad + 8.0) * a.sin()), t.ink, 2);
    c.clear_clip();
}

// ── Record ───────────────────────────────────────────────────────────────────────────────────

/// Turns a second: `viz_seed` advances 3.6 a second while playing, so one radian of it a radian of
/// the record is very nearly 33⅓ rpm (3.49 rad/s).
fn spin(np: &NowPlaying) -> f32 {
    if np.playing { np.viz_seed } else { 0.0 }
}

fn record(c: &mut Canvas, t: &Theme, np: &NowPlaying, x: i32, y: i32, size: i32) {
    let (cx, cy) = (x as f32 + size as f32 / 2.0, y as f32 + size as f32 / 2.0);
    let r = size as f32 / 2.0 - 4.0;
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
    // The sheen stays where the light is while the record turns under it.
    for (a0, a1) in [(0.6f32, 0.95f32), (0.6 + PI, 0.95 + PI)] {
        for k in 0..6 {
            arc(c, cx, cy, label + 16.0 + k as f32 * (r - label - 24.0) / 6.0, a0, a1, Rgb888::new(70, 70, 76), 1);
        }
    }
    // The label: the cover, turning; or the accent with a mark that shows it turning.
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
    // The tonearm: from its pivot at the top right, the stylus on the groove the song has reached
    // (the outer edge at the start, the run-out at the end).
    let (px, py) = (x as f32 + size as f32 - 10.0, y as f32 + 18.0);
    let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
    let arm = d * 0.92;
    let p = np.progress.clamp(0.0, 1.0);
    let rs = (r - 10.0) + (label + 14.0 - (r - 10.0)) * p;
    // The stylus is where a circle of radius `rs` about the centre meets one of `arm` about the
    // pivot: the meeting on the near side, below and left of the pivot.
    let a = (arm * arm - rs * rs + d * d) / (2.0 * d);
    let hh = (arm * arm - a * a).max(0.0).sqrt();
    let (ux, uy) = ((cx - px) / d, (cy - py) / d);
    let (mx, my) = (px + a * ux, py + a * uy);
    let stylus = (mx + hh * uy, my - hh * ux);
    let arm_col = mix(t.dim, t.ink, 0.4);
    line(c, (px, py), stylus, arm_col, 3);
    widgets::disc(c, stylus.0 as i32, stylus.1 as i32, 9, arm_col);
    widgets::disc(c, px as i32, py as i32, 18, t.line);
    widgets::disc(c, px as i32, py as i32, 8, arm_col);
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

// ── Tape ─────────────────────────────────────────────────────────────────────────────────────

fn tape(c: &mut Canvas, t: &Theme, f: &FontSet, np: &NowPlaying, x: i32, y: i32, size: i32) {
    let w = size;
    let h = (size as f32 * 0.64) as i32;
    let (bx, by) = (x, y + (size - h) / 2);
    let k = |v: f32| (v * w as f32) as i32; // proportions of the shell's width
    fill_rect(c, bx, by, w, h, t.panel);
    stroke_rect(c, bx, by, w, h, t.line, 1);
    // The label: an accent stripe, the title and the album.
    let (lx, ly, lw, lh) = (bx + k(0.05), by + k(0.04), w - 2 * k(0.05), k(0.2));
    fill_rect(c, lx, ly, lw, lh, t.bg);
    stroke_rect(c, lx, ly, lw, lh, t.line, 1);
    fill_rect(c, lx, ly + 6, lw, 2, t.acc);
    let tw = (lw - 20) as f32;
    let title = st(17.0, Weight::Regular, t.ink);
    text::draw(c, f, (lx + 10) as f32, (ly + 32) as f32, &widgets::fit(f, np.title, &title, tw), &title);
    let sub = st(12.0, Weight::Regular, t.dim);
    let under = if np.album.is_empty() { np.artist } else { np.album };
    text::draw(c, f, (lx + 10) as f32, (ly + 50) as f32, &widgets::fit(f, under, &sub, tw), &sub);
    // The window, the tape between its rollers, and the two packs: all on the left at the start
    // of the song, all on the right at its end. The spokes turn while it plays.
    let (wx, wy, ww, wh) = (bx + k(0.17), ly + lh + k(0.04), w - 2 * k(0.17), k(0.24));
    fill_rect(c, wx, wy, ww, wh, t.bg);
    stroke_rect(c, wx, wy, ww, wh, t.line, 1);
    let p = np.progress.clamp(0.0, 1.0);
    let (rmin, rmax) = (wh as f32 * 0.2, wh as f32 * 0.44);
    let cy = (wy + wh / 2) as f32;
    let ty = wy + wh - 5;
    fill_rect(c, wx + 8, ty, ww - 16, 1, t.dim);
    let turn = spin(np) * 2.0;
    for (cx, packed) in [(wx as f32 + ww as f32 * 0.24, 1.0 - p), (wx as f32 + ww as f32 * 0.76, p)] {
        let r = rmin + (rmax - rmin) * packed;
        widgets::disc(c, cx as i32, cy as i32, (2.0 * r) as u32, t.dim);
        widgets::disc(c, cx as i32, cy as i32, (2.0 * rmin) as u32 - 2, t.panel);
        widgets::disc(c, cx as i32, cy as i32, (rmin * 0.8) as u32, t.bg);
        for s in 0..6 {
            let a = turn + s as f32 * PI / 3.0;
            let (sx, sy) = (cx + rmin * 0.62 * a.cos(), cy + rmin * 0.62 * a.sin());
            widgets::disc(c, sx.round() as i32, sy.round() as i32, 4, t.faint);
        }
    }
    // The head opening at the bottom edge.
    let (hx, hw, hh) = (bx + k(0.3), w - 2 * k(0.3), k(0.09));
    stroke_rect(c, hx, by + h - hh, hw, hh, t.line, 1);
    for gx in [hx + hw / 5, hx + hw - hw / 5] {
        widgets::disc(c, gx, by + h - hh / 2, 8, t.line);
    }
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
