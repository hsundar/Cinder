//! The Pivot style's own screens: Home, a menu of large words, and the Library, whose sections
//! (songs · albums · artists · playlists · books) sit side by side in a strip under a huge heading
//! and are reached by a sideways swipe or a tap on their name. Everything is set in Atkinson
//! Hyperlegible Next (`Family::Legible`), large, with one item per row and almost no chrome.
//!
//! Geometry is one set of pure functions here — the section strip, each section's row pitch, the
//! list's top — read by both the draw and the navigator's tap and scroll code, so a style that
//! draws a row in one place cannot answer taps for it in another (the rule `now_playing::layout`
//! keeps for Now Playing). Sizes follow the UI text scale, so nothing overlaps at 140 %.
//!
//! The heading and the strip run off the right edge on purpose: what is cut off says "there is
//! more this way". They draw inside a horizontal clip band, which `Canvas` does not count as
//! overflow (`tests/ui_overflow.rs`).

use crate::canvas::Canvas;
use crate::library::Tab;
use crate::model::Library;
use crate::text::{self, Family, FontSet, TextStyle, Weight};
use crate::theme::Theme;
use embedded_graphics::pixelcolor::Rgb888;

const W: i32 = crate::canvas::W as i32;
/// The left edge of the heading, the strip and the rows: right of the back chevron.
pub const LEFT: f32 = 22.0;

/// A size at the current UI text scale (the same factor `text::draw` applies to type).
fn px(v: f32) -> i32 {
    text::scaled(v).round() as i32
}

fn st(w: Weight, size: f32, color: Rgb888) -> TextStyle {
    crate::widgets::sty(Family::Legible, w, size, color, 0.0)
}

// ── The Library ─────────────────────────────────────────────────────────────────────────────

/// A section of the Library: one of its list tabs, the audiobooks, or the folder browser.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Section {
    Tab(Tab),
    Books,
    Folders,
}

impl Section {
    pub fn label(self) -> &'static str {
        crate::i18n::text(match self {
            Section::Tab(Tab::Songs) => "songs",
            Section::Tab(Tab::Albums) => "albums",
            Section::Tab(Tab::Artists) => "artists",
            Section::Tab(Tab::Playlists) => "playlists",
            Section::Books => "books",
            Section::Folders => "folders",
        })
    }
}

/// The sections, in strip order: the four list tabs, then books when the library has audiobooks
/// and folders when Settings ▸ Folders tab is on.
pub fn sections(lib: &Library) -> Vec<Section> {
    let mut v = vec![
        Section::Tab(Tab::Songs),
        Section::Tab(Tab::Albums),
        Section::Tab(Tab::Artists),
        Section::Tab(Tab::Playlists),
    ];
    if !lib.books.is_empty() {
        v.push(Section::Books);
    }
    if lib.folders_tab {
        v.push(Section::Folders);
    }
    v
}

/// The section a sideways swipe lands on: `dir < 0` (leftward, the next) or `dir > 0`. The strip
/// wraps, as a carousel of names does.
pub fn neighbour(lib: &Library, at: Section, dir: i32) -> Section {
    let s = sections(lib);
    let i = s.iter().position(|x| *x == at).unwrap_or(0);
    let n = s.len();
    s[if dir < 0 { (i + 1) % n } else { (i + n - 1) % n }]
}

/// The heading's baseline and size.
fn title_base() -> f32 {
    crate::chrome::STATUS_H as f32 + text::scaled(60.0)
}
const TITLE_SIZE: f32 = 58.0;
/// The strip of section names.
const STRIP_SIZE: f32 = 30.0;
pub fn strip_top() -> i32 {
    title_base().round() as i32 + px(6.0)
}
pub fn strip_bottom() -> i32 {
    strip_top() + px(50.0)
}
fn strip_base() -> f32 {
    (strip_top() + px(36.0)) as f32
}
/// Where the rows begin.
pub fn list_top() -> i32 {
    strip_bottom() + px(8.0)
}
pub fn list_bottom() -> i32 {
    crate::library::LIST_BOTTOM
}

/// Where each name in the strip is drawn: `(section, x0, x1)`, the current one first at the left
/// and the rest after it in order, wrapping round; those starting past the edge are left out.
pub fn strip_layout(f: &FontSet, lib: &Library, current: Section) -> Vec<(Section, i32, i32)> {
    let s = sections(lib);
    let start = s.iter().position(|x| *x == current).unwrap_or(0);
    let style = st(Weight::Regular, STRIP_SIZE, Rgb888::new(0, 0, 0));
    let gap = text::scaled(26.0);
    let mut x = LEFT;
    let mut out = Vec::new();
    for k in 0..s.len() {
        let sec = s[(start + k) % s.len()];
        if x as i32 >= W {
            break;
        }
        let w = text::measure(f, sec.label(), &style);
        out.push((sec, x as i32, (x + w) as i32));
        x += w + gap;
    }
    out
}

/// The strip's name under `x`: each name owns the gap half-way to its neighbours, so no pixel of
/// the strip is dead.
pub fn strip_hit(zones: &[(Section, i32, i32)], x: i32) -> Option<Section> {
    zones.iter().enumerate().find_map(|(i, &(sec, x0, x1))| {
        let left = if i == 0 { 0 } else { (zones[i - 1].2 + x0) / 2 };
        let right = zones.get(i + 1).map_or(W, |n| (x1 + n.1) / 2);
        (x >= left && x < right).then_some(sec)
    })
}

/// A row's height in a section.
pub fn row_h(sec: Section) -> i32 {
    match sec {
        Section::Tab(Tab::Artists) => px(64.0),
        _ => px(78.0),
    }
}

/// How many rows a section has.
pub fn count(sec: Section, lib: &Library, lib_sort: usize, album_sort: usize) -> usize {
    match sec {
        Section::Tab(Tab::Songs) => crate::library::song_order(lib, lib_sort).len(),
        Section::Tab(Tab::Albums) => crate::library::album_display_order(lib, album_sort).len(),
        Section::Tab(Tab::Artists) => lib.artists.len(),
        Section::Tab(Tab::Playlists) => lib.playlists.len(),
        Section::Books => lib.books.len(),
        Section::Folders => 0,
    }
}

pub fn max_scroll(sec: Section, lib: &Library, lib_sort: usize, album_sort: usize) -> i32 {
    let content = count(sec, lib, lib_sort, album_sort) as i32 * row_h(sec);
    (content - (list_bottom() - list_top())).max(0)
}

/// The row (rank in the drawn order) under `y`, scrolled `scroll` px.
pub fn row_at(sec: Section, lib: &Library, lib_sort: usize, album_sort: usize, scroll: i32, y: i32) -> Option<usize> {
    if y < list_top() || y >= list_bottom() {
        return None;
    }
    let r = ((y - list_top() + scroll) / row_h(sec)) as usize;
    (r < count(sec, lib, lib_sort, album_sort)).then_some(r)
}

/// One row's two lines: the name, and what is under it.
fn row_text(sec: Section, lib: &Library, lib_sort: usize, album_sort: usize, rank: usize, order: &RowOrder) -> (String, String) {
    match sec {
        Section::Tab(Tab::Songs) => order
            .songs
            .get(rank)
            .and_then(|&i| lib.songs.get(i))
            .map(|s| (s.title.clone(), if s.artist.is_empty() { s.album_artist.clone() } else { s.artist.clone() }))
            .unwrap_or_default(),
        Section::Tab(Tab::Albums) => {
            let flat = lib.albums_flat();
            order
                .albums
                .get(rank)
                .and_then(|&i| flat.get(i))
                .map(|a| {
                    let mut sub = a.artist.clone();
                    if !a.year.is_empty() && a.year != "0" {
                        sub = if sub.is_empty() { a.year.clone() } else { format!("{sub} · {}", a.year) };
                    }
                    (a.name.clone(), sub)
                })
                .unwrap_or_default()
        }
        Section::Tab(Tab::Artists) => lib.artists.get(rank).map(|a| (a.name.clone(), String::new())).unwrap_or_default(),
        Section::Tab(Tab::Playlists) => lib
            .playlists
            .get(rank)
            .map(|p| (p.name.clone(), crate::ui_format!("{} songs", p.tracks)))
            .unwrap_or_default(),
        Section::Books => lib
            .books
            .get(rank)
            .map(|b| {
                let place = if b.finished {
                    crate::i18n::text("finished").to_string()
                } else if b.chapter > 0 || b.pos_ms > 0 {
                    crate::ui_format!("chapter {} of {}", b.chapter + 1, b.chapters.len())
                } else {
                    crate::ui_format!("{} chapters", b.chapters.len())
                };
                (b.title.clone(), if b.author.is_empty() { place } else { format!("{} · {place}", b.author) })
            })
            .unwrap_or_default(),
        Section::Folders => {
            let _ = (lib_sort, album_sort);
            Default::default()
        }
    }
}

/// The drawn orders, worked out once per frame rather than once per row.
struct RowOrder {
    songs: Vec<usize>,
    albums: Vec<usize>,
}

/// The heading of a Pivot screen: the back chevron, then `word` huge and light, running off the
/// right edge when it is long. Returns nothing: [`strip_top`] is where the next thing goes.
pub fn heading(c: &mut Canvas, t: &Theme, f: &FontSet, word: &str) {
    // The chevron stays small and left of the heading's first letter, so the heading lines up
    // with the strip and the rows; its target is the usual top-left one (`chrome::back_hit`).
    crate::icons::back(c, 10.0, 62.0, 12.0 * text::touch_scale(), t.faint);
    c.set_clip_x(0, W);
    text::draw(c, f, LEFT - 2.0, title_base(), word, &st(Weight::Light, TITLE_SIZE, t.ink));
    c.clear_clip_x();
}

/// The Library, or its books, in the Pivot style.
#[allow(clippy::too_many_arguments)]
pub fn library(
    c: &mut Canvas,
    t: &Theme,
    f: &FontSet,
    lib: &Library,
    sec: Section,
    lib_sort: usize,
    album_sort: usize,
    scroll: i32,
    playing_id: Option<i64>,
    sbar_active: bool,
) {
    c.fill(t.bg);
    heading(c, t, f, crate::i18n::text(if sec == Section::Books { "books" } else { "music" }));

    // The strip: the current section bright, the rest dim, the last ones cut by the edge.
    c.set_clip_x(0, W);
    for (s, x0, _) in strip_layout(f, lib, sec) {
        let col = if s == sec { t.ink } else { t.faint };
        text::draw(c, f, x0 as f32, strip_base(), s.label(), &st(Weight::Regular, STRIP_SIZE, col));
    }
    c.clear_clip_x();

    let top = list_top();
    let bottom = list_bottom();
    let rh = row_h(sec);
    let n = count(sec, lib, lib_sort, album_sort);
    let max = max_scroll(sec, lib, lib_sort, album_sort);
    let scroll = scroll.clamp(0, max);
    if n == 0 {
        let msg = match sec {
            Section::Books => "Put each book in its own folder inside LEARNING.",
            _ => "Nothing here yet.",
        };
        text::draw(c, f, LEFT, (top + px(40.0)) as f32, crate::i18n::text(msg), &st(Weight::Regular, 18.0, t.dim));
        return;
    }
    let order = RowOrder {
        songs: if sec == Section::Tab(Tab::Songs) { crate::library::song_order(lib, lib_sort) } else { Vec::new() },
        albums: if sec == Section::Tab(Tab::Albums) { crate::library::album_display_order(lib, album_sort) } else { Vec::new() },
    };
    c.set_clip_y(top, bottom);
    let first = (scroll / rh) as usize;
    let max_w = (W as f32) - LEFT - 22.0;
    let one_line = matches!(sec, Section::Tab(Tab::Artists));
    for rank in first..n {
        let y = top + rank as i32 * rh - scroll;
        if y >= bottom {
            break;
        }
        let (name, sub) = row_text(sec, lib, lib_sort, album_sort, rank, &order);
        let playing = sec == Section::Tab(Tab::Songs)
            && playing_id.is_some()
            && order.songs.get(rank).and_then(|&i| lib.songs.get(i)).map(|s| s.object_id) == playing_id;
        let name_col = if playing { t.acc } else { t.ink };
        if one_line {
            let s = st(Weight::Light, 30.0, name_col);
            let name = crate::widgets::fit(f, &name, &s, max_w);
            text::draw(c, f, LEFT, (y + px(42.0)) as f32, &name, &s);
        } else {
            let s = st(if playing { Weight::SemiBold } else { Weight::Regular }, 25.0, name_col);
            let name = crate::widgets::fit(f, &name, &s, max_w);
            text::draw(c, f, LEFT, (y + px(34.0)) as f32, &name, &s);
            if !sub.is_empty() {
                let s2 = st(Weight::Regular, 17.0, t.dim);
                let sub = crate::widgets::fit(f, &sub, &s2, max_w);
                text::draw(c, f, LEFT, (y + px(60.0)) as f32, &sub, &s2);
            }
        }
    }
    c.clear_clip();
    crate::library::scrollbar(c, t, top, bottom, scroll, n as i32 * rh, sbar_active);
}

// ── Home ────────────────────────────────────────────────────────────────────────────────────

/// What is playing, one line under the status bar; a tap there opens Now Playing.
pub fn home_strip_top() -> i32 {
    crate::chrome::STATUS_H
}
pub fn home_strip_bottom() -> i32 {
    home_strip_top() + px(44.0)
}
/// Where the words begin, and their pitch.
pub fn home_top() -> i32 {
    home_strip_bottom() + px(6.0)
}
pub fn home_row_h() -> i32 {
    px(66.0)
}
/// The START ON label and chips, under the last word.
fn home_label_top(rows: usize) -> i32 {
    home_top() + rows as i32 * home_row_h() + px(18.0)
}
fn home_chips_top(rows: usize) -> i32 {
    home_label_top(rows) + crate::kit::SECTION_H
}

pub fn home_max_scroll(rows: usize) -> i32 {
    let bottom = home_chips_top(rows) + crate::kit::CHIP_H + 16;
    (bottom - crate::H as i32).max(0)
}

/// The word under page-`y` (scrolled), of `rows`.
pub fn home_row_at(y: i32, rows: usize) -> Option<usize> {
    if y < home_top() {
        return None;
    }
    let r = ((y - home_top()) / home_row_h()) as usize;
    (r < rows).then_some(r)
}

/// The START ON chip under page-`(x, y)`.
pub fn home_chip_at(x: i32, y: i32, rows: usize) -> Option<usize> {
    crate::kit::chip_at(crate::menu::HOMES.len(), home_chips_top(rows), x, y)
}

/// Home: what is playing, then the destinations as large lowercase words.
pub fn home(c: &mut Canvas, t: &Theme, f: &FontSet, strip: &str, items: &[crate::menu::MenuItem], home: usize, scroll: i32) {
    c.fill(t.bg);
    let s = st(Weight::Regular, 18.0, t.dim);
    let line = crate::widgets::fit(f, strip, &s, (W as f32) - 2.0 * LEFT);
    text::draw(c, f, LEFT, (home_strip_bottom() - px(14.0)) as f32, &line, &s);
    c.set_clip_y(home_top(), crate::H as i32);
    c.set_clip_x(0, W);
    for (i, m) in items.iter().enumerate() {
        let y = home_top() + i as i32 * home_row_h() - scroll;
        let col = if m.active { t.acc } else { t.ink };
        let word = crate::i18n::text(m.label).to_lowercase();
        text::draw(c, f, LEFT, (y + px(50.0)) as f32, &word, &st(Weight::Light, 46.0, col));
    }
    c.clear_clip_x();
    let rows = items.len();
    crate::kit::section_label(c, t, f, home_label_top(rows) - scroll, crate::i18n::text("START ON"), None);
    let on = home.min(crate::menu::HOMES.len() - 1);
    crate::kit::chips_in(c, t, f, home_chips_top(rows) - scroll, &crate::menu::HOMES, Some(on), crate::menu::HOMES.len());
    c.clear_clip();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_strip_starts_at_the_current_section_and_wraps() {
        let fonts = FontSet::load();
        let lib = Library::sample();
        let z = strip_layout(&fonts, &lib, Section::Tab(Tab::Artists));
        assert_eq!(z[0].0, Section::Tab(Tab::Artists));
        assert_eq!(z[1].0, Section::Tab(Tab::Playlists));
        // Every pixel of the strip belongs to some name.
        for x in 0..W {
            assert!(strip_hit(&z, x).is_some(), "dead strip at {x}");
        }
        assert_eq!(strip_hit(&z, 30), Some(Section::Tab(Tab::Artists)));
    }

    #[test]
    fn a_swipe_steps_through_the_sections_and_wraps() {
        let lib = Library::sample();
        let s = sections(&lib);
        let last = *s.last().unwrap();
        assert_eq!(neighbour(&lib, Section::Tab(Tab::Songs), -1), Section::Tab(Tab::Albums));
        assert_eq!(neighbour(&lib, Section::Tab(Tab::Songs), 1), last);
        assert_eq!(neighbour(&lib, last, -1), Section::Tab(Tab::Songs));
    }

    #[test]
    fn rows_are_found_where_they_are_drawn() {
        let lib = Library::sample();
        let sec = Section::Tab(Tab::Songs);
        let y = list_top() + row_h(sec) + 3;
        assert_eq!(row_at(sec, &lib, 0, 0, 0, y), Some(1));
        assert_eq!(row_at(sec, &lib, 0, 0, row_h(sec), list_top() + 3), Some(1), "scrolled one row");
        assert_eq!(row_at(sec, &lib, 0, 0, 0, list_top() - 1), None, "the strip is not a row");
    }
}
