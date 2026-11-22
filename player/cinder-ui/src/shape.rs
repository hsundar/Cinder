//! Complex-script shaping, for Devanagari.
//!
//! The rest of the text engine (`text.rs`) draws one glyph per character. That is right for Latin,
//! CJK, Hangul and (nearly) Thai, and wrong for Devanagari, where what is drawn is not what is
//! stored:
//!
//!   * a consonant + virama + consonant is ONE conjunct glyph (क्ष, श्च, त्त), or a half form;
//!   * the vowel sign ि is stored after its consonant and drawn before it (कि);
//!   * र + virama before a consonant becomes the reph mark above the END of the syllable (र्म);
//!   * marks (anusvara, candrabindu, nukta, the vowel signs above and below) sit at positions the
//!     font gives per pair, not at a fixed offset.
//!
//! So a Devanagari RUN is shaped with `rustybuzz` (a port of HarfBuzz) against a bundled Noto Sans
//! Devanagari, which turns the run into positioned glyph ids, and `text.rs` draws those ids. Text
//! with no Devanagari never reaches this module, so every other string draws exactly as before.
//!
//! Sony's firmware carries no Devanagari face at all (`analysis/RE_sony_fonts.md`), which is why the
//! font is bundled rather than added to the device chain. Noto Sans Devanagari is OFL-1.1
//! (`assets/fonts/NotoSansDevanagari-OFL.txt`); the files are the unhinted TTFs of release
//! NotoSansDevanagari-v2.007 (github.com/notofonts/devanagari), unmodified.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::LazyLock;

use fontdue::{Font, FontSettings};

static REGULAR_BYTES: &[u8] = include_bytes!("../assets/fonts/NotoSansDevanagari-Regular.ttf");
static BOLD_BYTES: &[u8] = include_bytes!("../assets/fonts/NotoSansDevanagari-Bold.ttf");

/// The rasterisers. Parsed on first use: a library with no Devanagari tags never pays for them
/// (about 1 MB each parsed — fontdue reads every outline at load).
static RASTER: [LazyLock<Font>; 2] = [
    LazyLock::new(|| {
        Font::from_bytes(REGULAR_BYTES, FontSettings::default()).expect("Noto Sans Devanagari")
    }),
    LazyLock::new(|| {
        Font::from_bytes(BOLD_BYTES, FontSettings::default()).expect("Noto Sans Devanagari Bold")
    }),
];

/// The shapers' view of the same bytes. `rustybuzz::Face` reads tables lazily, so this is cheap.
static SHAPER: [LazyLock<rustybuzz::Face<'static>>; 2] = [
    LazyLock::new(|| rustybuzz::Face::from_slice(REGULAR_BYTES, 0).expect("Noto Sans Devanagari")),
    LazyLock::new(|| {
        rustybuzz::Face::from_slice(BOLD_BYTES, 0).expect("Noto Sans Devanagari Bold")
    }),
];

/// The rasteriser for weight `bold` (see [`face_for`]).
pub(crate) fn raster(bold: bool) -> &'static Font {
    &RASTER[bold as usize]
}

/// Characters drawn in a shaped run: the Devanagari block, Devanagari Extended, Vedic Extensions,
/// and the joiners (ZWNJ/ZWJ) and dotted circle that only mean something inside one.
pub fn in_run(ch: char) -> bool {
    matches!(ch as u32,
        0x0900..=0x097F | 0xA8E0..=0xA8FF | 0x1CD0..=0x1CFF | 0x200C | 0x200D | 0x25CC)
}

/// Is there anything here this module has to shape? The fast path for every other string.
pub fn needs_shaping(s: &str) -> bool {
    s.chars()
        .any(|c| matches!(c as u32, 0x0900..=0x097F | 0xA8E0..=0xA8FF | 0x1CD0..=0x1CFF))
}

/// Which bundled face a text weight maps to: false = Regular, true = Bold. Two faces, not four:
/// SemiBold reads as bold at the sizes it is used for, and each face is ~180 KB in the binary.
pub fn face_for(w: crate::text::Weight) -> bool {
    use crate::text::Weight;
    matches!(w, Weight::SemiBold | Weight::Bold | Weight::ExtraBold)
}

/// One glyph out of the shaper, in pixels at the size it was shaped for.
#[derive(Clone, Copy, Debug)]
pub struct Glyph {
    pub id: u16,
    pub advance: f32,
    pub x_off: f32,
    /// Up is positive, as in the font; the drawer subtracts it from the baseline.
    pub y_off: f32,
    /// Byte offset in the run of the cluster this glyph belongs to. Glyphs of one cluster share
    /// it, and a cluster is the smallest piece that may be cut off or measured on its own.
    pub cluster: u32,
}

/// A piece of a string as the drawer walks it: a character drawn the old way, or a run to shape.
pub enum Piece<'a> {
    Char(char),
    Run(&'a str),
}

/// Split `s` into single characters and maximal shaped runs, in order.
pub fn pieces(s: &str) -> Vec<Piece<'_>> {
    let mut out = Vec::new();
    let mut run_start: Option<usize> = None;
    for (i, ch) in s.char_indices() {
        if in_run(ch) {
            run_start.get_or_insert(i);
        } else {
            if let Some(a) = run_start.take() {
                out.push(Piece::Run(&s[a..i]));
            }
            out.push(Piece::Char(ch));
        }
    }
    if let Some(a) = run_start {
        out.push(Piece::Run(&s[a..]));
    }
    out
}

const SHAPE_CACHE_MAX: usize = 512;

type ShapeKey = (String, bool, u32);

thread_local! {
    /// Shaping is not free and the UI measures the same title several times a frame (fit, centre,
    /// marquee), so shaped runs are kept by (run, face, exact size). Dropped whole at the cap, like
    /// the glyph cache: a library of distinct Devanagari titles is the only way to reach it.
    static SHAPED: RefCell<HashMap<ShapeKey, Rc<Vec<Glyph>>>> = RefCell::new(HashMap::new());
}

/// Shape one run (from [`pieces`]) at `size` px.
pub fn shape(run: &str, bold: bool, size: f32) -> Rc<Vec<Glyph>> {
    let key = (run.to_string(), bold, size.to_bits());
    if let Some(hit) = SHAPED.with(|c| c.borrow().get(&key).cloned()) {
        return hit;
    }
    let face = &*SHAPER[bold as usize];
    let mut buf = rustybuzz::UnicodeBuffer::new();
    buf.push_str(run);
    // Say so rather than guess: a run may start with a joiner or a mark, which guessing reads as
    // "common" script and shapes without the Devanagari rules.
    buf.set_script(rustybuzz::script::DEVANAGARI);
    buf.set_direction(rustybuzz::Direction::LeftToRight);
    let out = rustybuzz::shape(face, &[], buf);
    let scale = size / face.units_per_em() as f32;
    let glyphs: Vec<Glyph> = out
        .glyph_infos()
        .iter()
        .zip(out.glyph_positions())
        .map(|(info, pos)| Glyph {
            id: info.glyph_id as u16,
            advance: pos.x_advance as f32 * scale,
            x_off: pos.x_offset as f32 * scale,
            y_off: pos.y_offset as f32 * scale,
            cluster: info.cluster,
        })
        .collect();
    let rc = Rc::new(glyphs);
    SHAPED.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() >= SHAPE_CACHE_MAX {
            c.clear();
        }
        c.insert(key, rc.clone());
    });
    rc
}

/// A combining mark of the Devanagari block: never the first character of a cluster.
fn is_mark(ch: char) -> bool {
    matches!(ch as u32,
        0x0900..=0x0903 | 0x093A..=0x093C | 0x093E..=0x094F | 0x0951..=0x0957 | 0x0962..=0x0963
        | 0xA8E0..=0xA8F1 | 0xA8FF | 0x1CD0..=0x1CF9)
}

const VIRAMA: char = '\u{094D}';
const ZWJ: char = '\u{200D}';
const ZWNJ: char = '\u{200C}';

/// The string cut into the pieces it may be truncated or wrapped between. For anything but
/// Devanagari that is one character each, as before. In Devanagari a cluster is an akshara: a
/// consonant with its conjoined consonants (joined by virama), its vowel sign and its marks — so a
/// cut never strands a ि or splits क्ष into क् and ष.
pub fn clusters(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut prev: Option<char> = None;
    for (i, ch) in s.char_indices() {
        let joined = is_mark(ch)
            || ch == ZWJ
            || ch == ZWNJ
            || (prev == Some(VIRAMA) && in_run(ch))
            || prev == Some(ZWJ);
        if i > 0 && !joined {
            out.push(&s[start..i]);
            start = i;
        }
        prev = Some(ch);
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_never_shaped() {
        assert!(!needs_shaping("Vishwa Mohan Bhatt — Meghdoot (Kalidasa)"));
        assert!(!needs_shaping("北京 東京 서울 ภาษาไทย"));
        assert!(needs_shaping("मेघदूत ( कालिदास)"));
    }

    #[test]
    fn a_string_splits_into_characters_and_runs() {
        let p = pieces("Raag यमन (live)");
        let runs: Vec<&str> = p
            .iter()
            .filter_map(|x| {
                if let Piece::Run(r) = x {
                    Some(*r)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(runs, ["यमन"]);
        let chars = p.iter().filter(|x| matches!(x, Piece::Char(_))).count();
        assert_eq!(chars, "Raag  (live)".chars().count());
    }

    #[test]
    fn clusters_keep_conjuncts_and_vowel_signs_whole() {
        // The tags this was written against (Meghadutam, Balwant Bhatt).
        assert_eq!(clusters("कश्चित्"), ["क", "श्चि", "त्"]);
        assert_eq!(clusters("भट्ट"), ["भ", "ट्ट"]);
        assert_eq!(clusters("महत्त्व"), ["म", "ह", "त्त्व"]);
        assert_eq!(clusters("शृङ्गार"), ["शृ", "ङ्गा", "र"]);
        assert_eq!(clusters("मेघदूत"), ["मे", "घ", "दू", "त"]);
        assert_eq!(clusters("Ab"), ["A", "b"]);
    }

    #[test]
    fn a_conjunct_shapes_to_fewer_glyphs_than_it_has_characters() {
        // क + ् + ष is one glyph in Noto Sans Devanagari: the whole point of shaping.
        let g = shape("क्ष", false, 20.0);
        assert!(g.len() < 3, "{g:?}");
        assert!(g.iter().all(|x| x.id != 0), "a .notdef in {g:?}");
    }

    #[test]
    fn the_short_i_sign_is_drawn_before_its_consonant() {
        // कि is stored क, ि. Shaped, the ि glyph comes first, so it draws to the left.
        let g = shape("कि", false, 20.0);
        let font = &*SHAPER[0];
        let ka = font.glyph_index('क').unwrap().0;
        assert_eq!(g.len(), 2);
        assert_ne!(g[0].id, ka, "ि should precede क: {g:?}");
        assert_eq!(g[1].id, ka);
    }

    #[test]
    fn truncation_ends_on_a_whole_akshara_at_every_width() {
        use crate::text::{Family, FontSet, TextStyle, Weight};
        // Measures at 100 %: hold the text size against tests that change it.
        let _g = crate::text::scale_guard();
        let fonts = FontSet::load();
        let st = TextStyle {
            fam: Family::Sans,
            weight: Weight::SemiBold,
            size: 18.0,
            color: embedded_graphics::pixelcolor::Rgb888::new(255, 255, 255),
            tracking: 0.0,
        };
        for t in [
            "कश्चित् कान्ताविरहगुरुणा",
            "राग वाक्यों का महत्त्व",
            "बन्दिषो मे राग ताल शृङ्गार",
        ] {
            let whole = clusters(t);
            for w in (20..300).step_by(7) {
                let fit = crate::widgets::fit(&fonts, t, &st, w as f32);
                let kept = fit.strip_suffix('…').unwrap_or(&fit);
                // A prefix made of whole clusters: re-clustering it gives the same first clusters.
                let got = clusters(kept);
                assert_eq!(
                    got[..],
                    whole[..got.len()],
                    "{t:?} at {w}px cut to {kept:?}"
                );
                assert!(crate::text::measure(&fonts, &fit, &st) <= w as f32 || got.is_empty());
            }
        }
    }

    #[test]
    fn every_tag_in_the_test_albums_shapes_without_a_missing_glyph() {
        for t in [
            "मेघदूत ( कालिदास)",
            "कश्चित् कान्ताविरहगुरुणा",
            "जातं वंशे भुवनविदिते",
            "उज्जैन",
            "बलवन्त राई भट्ट (भावरंग)",
            "रागो की प्रस्तुति",
            "राग वाक्यों का महत्त्व",
            "बन्दिषो मे राग ताल शृङ्गार - १",
        ] {
            for p in pieces(t) {
                if let Piece::Run(r) = p {
                    for bold in [false, true] {
                        let g = shape(r, bold, 18.0);
                        assert!(g.iter().all(|x| x.id != 0), "{r:?} has a .notdef: {g:?}");
                    }
                }
            }
        }
    }
}
