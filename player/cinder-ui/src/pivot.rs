//! The Pivot style's own screens: Home, the hub, a menu of large words; two hubs under it, music
//! (songs · albums · artists · playlists) and the spoken word (podcasts · books), whose sections
//! sit side by side in a strip under a huge heading and are reached by a sideways swipe or a tap
//! on their name; and the pages a row
//! opens (an album, an artist, a playlist, a podcast show). Everything is set in Atkinson
//! Hyperlegible Next (`Family::Legible`), large, with one item per row and almost no chrome.
//!
//! Geometry is one set of pure functions here — the section strip, each section's row pitch, the
//! list's top — read by both the draw and the navigator's tap and scroll code, so a style that
//! draws a row in one place cannot answer taps for it in another (the rule `now_playing::layout`
//! keeps for Now Playing). Sizes follow the UI text scale, so nothing overlaps at 140 %.
//!
//! The heading and the strip run off the right edge on purpose: what is cut off says "there is
//! more this way". They draw after `Canvas::bleed_x`, so what runs off is not counted as overflow;
//! everything else (rows, action words) is held to the margin by `tests/ui_overflow.rs`.

use crate::canvas::Canvas;
use crate::library::Tab;
use crate::model::Library;
use crate::text::{self, Family, FontSet, TextStyle, Weight};
use crate::theme::Theme;
use embedded_graphics::pixelcolor::Rgb888;

const W: i32 = crate::canvas::W as i32;
/// The left edge of the heading, the strip and the rows: right of the back chevron.
pub const LEFT: f32 = 22.0;

/// Is `t` drawn the Pivot way? The shared header (`chrome::header`), the kit (`kit.rs`), the status
/// bar and Up Next ask this, so every screen built from them takes the style's face and its quiet
/// furniture without a Pivot arm of its own. Only the drawing changes: every box a finger can land
/// on is where the Cinder look puts it.
pub fn on(t: &Theme) -> bool {
    t.style == crate::style::Style::Pivot
}

/// A label said the Pivot way: one written in capitals ("SAVE", "7 LEFT", "OFF") in lower case. A
/// label with any lower-case letter in it is left as it is: it is a name (a device, a codec's
/// "kbps") or already a sentence.
pub fn quiet(s: &str) -> std::borrow::Cow<'_, str> {
    if s.chars().any(char::is_lowercase) {
        s.into()
    } else {
        s.to_lowercase().into()
    }
}

/// The face at `size` and `weight` in `col`, as the Pivot style draws its furniture.
pub fn face(weight: Weight, size: f32, col: Rgb888) -> TextStyle {
    crate::widgets::sty(Family::Legible, weight, size, col, 0.0)
}

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
    /// The podcast shows: books that `BookRow::is_podcast` says are shows.
    Podcasts,
    Folders,
}

impl Section {
    pub fn label(self) -> &'static str {
        // Each word a literal in its own call, so the locale tool finds it.
        match self {
            Section::Tab(Tab::Songs) => crate::i18n::text("songs"),
            Section::Tab(Tab::Albums) => crate::i18n::text("albums"),
            Section::Tab(Tab::Artists) => crate::i18n::text("artists"),
            Section::Tab(Tab::Playlists) => crate::i18n::text("playlists"),
            Section::Books => crate::i18n::text("books"),
            Section::Podcasts => crate::i18n::text("podcasts"),
            Section::Folders => crate::i18n::text("folders"),
        }
    }
}

/// The sections that pivot beside `at`, in strip order. Music is the four list tabs, then folders
/// when Settings ▸ Folders tab is on. The spoken word is a hub of its own, opened from Home as
/// podcasts or audiobooks: those two, always both, so either Home word lands on its own list
/// (empty, with where to put the files) even when the drive has none.
pub fn sections(lib: &Library, at: Section) -> Vec<Section> {
    if matches!(at, Section::Books | Section::Podcasts) {
        return vec![Section::Podcasts, Section::Books];
    }
    let mut v = vec![
        Section::Tab(Tab::Songs),
        Section::Tab(Tab::Albums),
        Section::Tab(Tab::Artists),
        Section::Tab(Tab::Playlists),
    ];
    if lib.folders_tab {
        v.push(Section::Folders);
    }
    v
}

/// The section a sideways swipe lands on: `dir < 0` (leftward, the next) or `dir > 0`. The strip
/// wraps, as a carousel of names does.
pub fn neighbour(lib: &Library, at: Section, dir: i32) -> Section {
    let s = sections(lib, at);
    let i = s.iter().position(|x| *x == at).unwrap_or(0);
    let n = s.len();
    s[if dir < 0 {
        (i + 1) % n
    } else {
        (i + n - 1) % n
    }]
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
    let s = sections(lib, current);
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

/// The books (or, `podcasts`, the shows) on the shelf, as indices into `Library::books`.
pub fn shelf(lib: &Library, podcasts: bool) -> Vec<usize> {
    (0..lib.books.len())
        .filter(|&i| lib.books[i].is_podcast() == podcasts)
        .collect()
}

/// How many rows a section has.
pub fn count(sec: Section, lib: &Library, lib_sort: usize, album_sort: usize) -> usize {
    match sec {
        Section::Tab(Tab::Songs) => crate::library::song_order(lib, lib_sort).len(),
        Section::Tab(Tab::Albums) => crate::library::album_display_order(lib, album_sort).len(),
        Section::Tab(Tab::Artists) => lib.artists.len(),
        Section::Tab(Tab::Playlists) => lib.playlists.len(),
        Section::Books => shelf(lib, false).len(),
        Section::Podcasts => shelf(lib, true).len(),
        Section::Folders => 0,
    }
}

pub fn max_scroll(sec: Section, lib: &Library, lib_sort: usize, album_sort: usize) -> i32 {
    let content = count(sec, lib, lib_sort, album_sort) as i32 * row_h(sec);
    (content - (list_bottom() - list_top())).max(0)
}

/// The row (rank in the drawn order) under `y`, scrolled `scroll` px.
pub fn row_at(
    sec: Section,
    lib: &Library,
    lib_sort: usize,
    album_sort: usize,
    scroll: i32,
    y: i32,
) -> Option<usize> {
    if y < list_top() || y >= list_bottom() {
        return None;
    }
    let r = ((y - list_top() + scroll) / row_h(sec)) as usize;
    (r < count(sec, lib, lib_sort, album_sort)).then_some(r)
}

/// One row's two lines: the name, and what is under it.
fn row_text(
    sec: Section,
    lib: &Library,
    lib_sort: usize,
    album_sort: usize,
    rank: usize,
    order: &RowOrder,
) -> (String, String) {
    match sec {
        Section::Tab(Tab::Songs) => order
            .songs
            .get(rank)
            .and_then(|&i| lib.songs.get(i))
            .map(|s| {
                (
                    s.title.clone(),
                    if s.artist.is_empty() {
                        s.album_artist.clone()
                    } else {
                        s.artist.clone()
                    },
                )
            })
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
                        sub = if sub.is_empty() {
                            a.year.clone()
                        } else {
                            format!("{sub} · {}", a.year)
                        };
                    }
                    (a.name.clone(), sub)
                })
                .unwrap_or_default()
        }
        Section::Tab(Tab::Artists) => lib
            .artists
            .get(rank)
            .map(|a| (a.name.clone(), String::new()))
            .unwrap_or_default(),
        Section::Tab(Tab::Playlists) => lib
            .playlists
            .get(rank)
            .map(|p| (p.name.clone(), crate::ui_format!("{} songs", p.tracks)))
            .unwrap_or_default(),
        Section::Podcasts => order
            .books
            .get(rank)
            .and_then(|&i| lib.books.get(i))
            .map(|b| {
                let n = b.chapters.len();
                let new = unheard(b);
                let mut sub = if new > 0 {
                    crate::ui_format!("{} new of {}", new, n)
                } else {
                    crate::ui_format!("{} episodes", n)
                };
                if let Some(d) = b.chapter_date(n.saturating_sub(1)) {
                    sub = format!("{sub} · {d}");
                }
                (b.title.clone(), sub)
            })
            .unwrap_or_default(),
        Section::Books => order
            .books
            .get(rank)
            .and_then(|&i| lib.books.get(i))
            .map(|b| {
                let place = if b.finished {
                    crate::i18n::text("finished").to_string()
                } else if b.chapter > 0 || b.pos_ms > 0 {
                    crate::ui_format!("chapter {} of {}", b.chapter + 1, b.chapters.len())
                } else {
                    crate::ui_format!("{} chapters", b.chapters.len())
                };
                (
                    b.title.clone(),
                    if b.author.is_empty() {
                        place
                    } else {
                        format!("{} · {place}", b.author)
                    },
                )
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
    books: Vec<usize>,
}

/// How many of a show's episodes have not been heard: those after the one the listener is in
/// (episodes play oldest first, so everything before it was heard), and that one if not begun.
pub fn unheard(b: &crate::model::BookRow) -> usize {
    if b.finished {
        return 0;
    }
    b.chapters
        .len()
        .saturating_sub(b.chapter + usize::from(b.pos_ms > 0))
}

/// The heading of a Pivot screen: the back chevron, then `word` huge and light, running off the
/// right edge when it is long. Returns nothing: [`strip_top`] is where the next thing goes.
pub fn heading(c: &mut Canvas, t: &Theme, f: &FontSet, word: &str) {
    // The chevron stays small and left of the heading's first letter, so the heading lines up
    // with the strip and the rows; its target is the usual top-left one (`chrome::back_hit`).
    crate::icons::back(c, 10.0, 62.0, 12.0 * text::touch_scale(), t.faint);
    c.bleed_x();
    text::draw(
        c,
        f,
        LEFT - 2.0,
        title_base(),
        word,
        &st(Weight::Light, TITLE_SIZE, t.ink),
    );
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
    heading(
        c,
        t,
        f,
        match sec {
            Section::Books | Section::Podcasts => crate::i18n::text("spoken word"),
            _ => crate::i18n::text("music"),
        },
    );

    // The strip: the current section bright, the rest dim, the last ones cut by the edge.
    c.bleed_x();
    for (s, x0, _) in strip_layout(f, lib, sec) {
        let col = if s == sec { t.ink } else { t.faint };
        text::draw(
            c,
            f,
            x0 as f32,
            strip_base(),
            s.label(),
            &st(Weight::Regular, STRIP_SIZE, col),
        );
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
            Section::Books => crate::i18n::text("Put each book in its own folder inside LEARNING."),
            Section::Podcasts => {
                crate::i18n::text("Put each show in its own folder inside LEARNING/Podcasts.")
            }
            _ => crate::i18n::text("Nothing here yet."),
        };
        text::draw(
            c,
            f,
            LEFT,
            (top + px(40.0)) as f32,
            msg,
            &st(Weight::Regular, 18.0, t.dim),
        );
        return;
    }
    let order = RowOrder {
        songs: if sec == Section::Tab(Tab::Songs) {
            crate::library::song_order(lib, lib_sort)
        } else {
            Vec::new()
        },
        albums: if sec == Section::Tab(Tab::Albums) {
            crate::library::album_display_order(lib, album_sort)
        } else {
            Vec::new()
        },
        books: match sec {
            Section::Books => shelf(lib, false),
            Section::Podcasts => shelf(lib, true),
            _ => Vec::new(),
        },
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
            && order
                .songs
                .get(rank)
                .and_then(|&i| lib.songs.get(i))
                .map(|s| s.object_id)
                == playing_id;
        let name_col = if playing { t.acc } else { t.ink };
        if one_line {
            let s = st(Weight::Light, 30.0, name_col);
            let name = crate::widgets::fit(f, &name, &s, max_w);
            text::draw(c, f, LEFT, (y + px(42.0)) as f32, &name, &s);
        } else {
            let s = st(
                if playing {
                    Weight::SemiBold
                } else {
                    Weight::Regular
                },
                25.0,
                name_col,
            );
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

// ── Pages: an album, an artist, a playlist ──────────────────────────────────────────────────
//
// Each page is the heading (whose page it is, huge), a block or a strip under it, a line of action
// words in the accent ("play  shuffle  add"), then the rows. The words' places are measured when
// drawn and recorded by the navigator ([`ActionZone`]); everything vertical is pure, so the list's
// top does not depend on what the words measure.

/// An action word's target: `(index, x0, y0, x1, y1)`.
pub type ActionZone = (usize, i32, i32, i32, i32);

const ACTION_SIZE: f32 = 23.0;
/// Action words per line; more wrap onto a second.
const ACTIONS_PER_LINE: usize = 3;
fn action_line_h() -> i32 {
    px(46.0)
}
fn action_lines(n: usize) -> i32 {
    n.div_ceil(ACTIONS_PER_LINE).max(1) as i32
}

/// The cover block on the album and playlist pages: a 96 px cover, the words beside it.
pub const PAGE_COVER: i32 = crate::library::COVER_PX;
pub fn block_top() -> i32 {
    title_base().round() as i32 + px(20.0)
}
fn block_h() -> i32 {
    px(PAGE_COVER as f32).max(PAGE_COVER)
}
/// Where the action words begin on a page with the cover block.
pub fn block_actions_top() -> i32 {
    block_top() + block_h() + px(10.0)
}
/// Where the action words begin on the artist page, under its strip.
pub fn strip_actions_top() -> i32 {
    strip_bottom() + px(2.0)
}
/// Where a page's rows begin, under `n` action words starting at `actions_top`.
pub fn page_list_top(actions_top: i32, n: usize) -> i32 {
    actions_top + action_lines(n) * action_line_h() + px(4.0)
}

/// Lay out `labels` as action words from `top`, three to a line.
pub fn actions_layout(f: &FontSet, labels: &[&str], top: i32) -> Vec<ActionZone> {
    let style = st(Weight::Regular, ACTION_SIZE, Rgb888::new(0, 0, 0));
    let gap = text::scaled(28.0);
    let lh = action_line_h();
    let mut out = Vec::new();
    for (line, chunk) in labels.chunks(ACTIONS_PER_LINE).enumerate() {
        let y0 = top + line as i32 * lh;
        let mut x = LEFT;
        for (k, l) in chunk.iter().enumerate() {
            let w = text::measure(f, l, &style);
            out.push((
                line * ACTIONS_PER_LINE + k,
                x as i32,
                y0,
                (x + w) as i32,
                y0 + lh,
            ));
            x += w + gap;
        }
    }
    out
}

/// The action word under `(x, y)`: each owns the gap half-way to its neighbour on its line, and
/// the last on a line owns the rest of it.
pub fn action_hit(zones: &[ActionZone], x: i32, y: i32) -> Option<usize> {
    let line: Vec<&ActionZone> = zones.iter().filter(|z| (z.2..z.4).contains(&y)).collect();
    line.iter()
        .enumerate()
        .find_map(|(i, &&(idx, x0, _, x1, _))| {
            let left = if i == 0 { 0 } else { (line[i - 1].3 + x0) / 2 };
            let right = line.get(i + 1).map_or(W, |n| (x1 + n.1) / 2);
            (x >= left && x < right).then_some(idx)
        })
}

fn draw_actions(c: &mut Canvas, t: &Theme, f: &FontSet, zones: &[ActionZone], labels: &[&str]) {
    // Not allowed to run off: an action cut at the edge would be a control nobody can read.
    let s = st(Weight::Regular, ACTION_SIZE, t.acc);
    for &(i, x0, y0, _, _) in zones {
        if let Some(l) = labels.get(i) {
            text::draw(c, f, x0 as f32, (y0 + px(32.0)) as f32, l, &s);
        }
    }
}

/// The words beside a cover: a name in up to two lines, and a line under it.
fn block_words(c: &mut Canvas, t: &Theme, f: &FontSet, name: &str, meta: &str) {
    let x = (LEFT as i32 + PAGE_COVER + 16) as f32;
    let right = W as f32 - 18.0;
    let top = block_top();
    let ns = st(Weight::Regular, 24.0, t.ink);
    let lines = crate::widgets::draw_wrap2(
        c,
        f,
        x,
        (top + px(24.0)) as f32,
        name,
        &ns,
        right,
        text::scaled(28.0),
    );
    let ms = st(Weight::Regular, 16.0, t.dim);
    let my = top + px(24.0) + (lines - 1) * px(28.0) + px(28.0);
    text::draw(
        c,
        f,
        x,
        my as f32,
        &crate::widgets::fit(f, meta, &ms, right - x),
        &ms,
    );
}

/// The rows of a page, from `top`, with the scrollbar: `row(c, y, i)` draws row `i` at `y`.
fn page_rows(
    c: &mut Canvas,
    t: &Theme,
    top: i32,
    rh: i32,
    n: usize,
    scroll: i32,
    sbar_active: bool,
    mut row: impl FnMut(&mut Canvas, i32, usize),
) {
    let bottom = list_bottom();
    let max = (n as i32 * rh - (bottom - top)).max(0);
    let scroll = scroll.clamp(0, max);
    c.set_clip_y(top, bottom);
    for i in (scroll / rh) as usize..n {
        let y = top + i as i32 * rh - scroll;
        if y >= bottom {
            break;
        }
        row(c, y, i);
    }
    c.clear_clip();
    crate::library::scrollbar(c, t, top, bottom, scroll, n as i32 * rh, sbar_active);
}

/// A row's name and the line under it.
fn two_lines(
    c: &mut Canvas,
    t: &Theme,
    f: &FontSet,
    x: f32,
    y: i32,
    name: &str,
    sub: &str,
    now: bool,
) {
    let max_w = W as f32 - x - 22.0;
    let col = if now { t.acc } else { t.ink };
    let s = st(
        if now {
            Weight::SemiBold
        } else {
            Weight::Regular
        },
        25.0,
        col,
    );
    text::draw(
        c,
        f,
        x,
        (y + px(34.0)) as f32,
        &crate::widgets::fit(f, name, &s, max_w),
        &s,
    );
    if !sub.is_empty() {
        let s2 = st(Weight::Regular, 17.0, t.dim);
        text::draw(
            c,
            f,
            x,
            (y + px(60.0)) as f32,
            &crate::widgets::fit(f, sub, &s2, max_w),
            &s2,
        );
    }
}

/// Max scroll of `n` rows of `rh` from `top`.
pub fn rows_max_scroll(top: i32, rh: i32, n: usize) -> i32 {
    (n as i32 * rh - (list_bottom() - top)).max(0)
}

/// The row under `y` of `n` rows of `rh` from `top`, scrolled `scroll`.
pub fn rows_at(top: i32, rh: i32, n: usize, scroll: i32, y: i32) -> Option<usize> {
    if y < top || y >= list_bottom() {
        return None;
    }
    let r = ((y - top + scroll.max(0)) / rh) as usize;
    (r < n).then_some(r)
}

// The album page.

/// The album page's action words: play, shuffle, add — and delete when Settings allows it.
pub fn album_actions(delete: bool) -> Vec<&'static str> {
    let mut v = vec![
        crate::i18n::text("play"),
        crate::i18n::text("shuffle"),
        crate::i18n::text("add"),
    ];
    if delete {
        v.push(crate::i18n::text("delete"));
    }
    v
}
pub fn album_row_h() -> i32 {
    px(62.0)
}
pub fn album_list_top(delete: bool) -> i32 {
    page_list_top(block_actions_top(), album_actions(delete).len())
}
fn album_rows(al: &crate::model::AlbumRow) -> usize {
    if al.cue.is_empty() {
        al.track_list.len()
    } else {
        al.cue.len()
    }
}
pub fn album_max_scroll(al: &crate::model::AlbumRow, delete: bool) -> i32 {
    rows_max_scroll(album_list_top(delete), album_row_h(), album_rows(al))
}
pub fn album_row_at(
    al: &crate::model::AlbumRow,
    delete: bool,
    scroll: i32,
    y: i32,
) -> Option<usize> {
    rows_at(
        album_list_top(delete),
        album_row_h(),
        album_rows(al),
        scroll,
        y,
    )
}

/// The album page: the artist as the heading, the cover and the album's name, then its tracks.
#[allow(clippy::too_many_arguments)]
pub fn album(
    c: &mut Canvas,
    t: &Theme,
    f: &FontSet,
    al: &crate::model::AlbumRow,
    playing: Option<usize>,
    scroll: i32,
    cover: Option<&crate::art::Image>,
    delete: bool,
    sbar_active: bool,
) -> Vec<ActionZone> {
    c.fill(t.bg);
    let artist = if al.artist.is_empty() {
        crate::i18n::text("album")
    } else {
        al.artist.as_str()
    };
    heading(c, t, f, artist);
    let top = block_top();
    match cover {
        Some(img) if img.w == PAGE_COVER as usize && img.h == PAGE_COVER as usize => {
            crate::art::draw_image(c, t, LEFT as i32, top, img, 1.0)
        }
        _ => crate::art::block_cached(c, t, LEFT as i32, top, PAGE_COVER, PAGE_COVER, &al.art, 1.0),
    }
    let n = if al.cue.is_empty() {
        al.tracks
    } else {
        al.cue.len() as u32
    };
    let songs = crate::ui_format!("{} songs", n);
    let meta = if al.year.is_empty() || al.year == "0" {
        songs
    } else {
        format!("{} · {songs}", al.year)
    };
    block_words(c, t, f, &al.name, &meta);
    let labels = album_actions(delete);
    let zones = actions_layout(f, &labels, block_actions_top());
    draw_actions(c, t, f, &zones, &labels);
    let rh = album_row_h();
    page_rows(
        c,
        t,
        album_list_top(delete),
        rh,
        album_rows(al),
        scroll,
        sbar_active,
        |c, y, i| {
            let (title, dur) = match al.cue.get(i) {
                Some(cr) => (cr.title.as_str(), cr.dur.as_str()),
                None => (
                    al.track_list[i].title.as_str(),
                    al.track_list[i].dur.as_str(),
                ),
            };
            let now = playing == Some(i);
            let base = (y + px(40.0)) as f32;
            let ns = st(Weight::Regular, 17.0, if now { t.acc } else { t.faint });
            text::draw(c, f, LEFT, base, &format!("{}", i + 1), &ns);
            let ds = st(Weight::Regular, 16.0, t.dim);
            let dw = text::measure(f, dur, &ds);
            crate::widgets::right(c, f, W as f32 - 22.0, base, dur, &ds);
            let x = LEFT + text::scaled(38.0);
            let ts = st(
                if now {
                    Weight::SemiBold
                } else {
                    Weight::Regular
                },
                23.0,
                if now { t.acc } else { t.ink },
            );
            let room = W as f32 - 22.0 - dw - 14.0 - x;
            text::draw(
                c,
                f,
                x,
                base,
                &crate::widgets::fit(f, title, &ts, room),
                &ts,
            );
        },
    );
    zones
}

// The artist page.

/// The artist page's sections: their albums, and all their songs.
pub fn artist_sections(albums: usize) -> Vec<&'static str> {
    if albums == 0 {
        vec![crate::i18n::text("songs")]
    } else {
        vec![crate::i18n::text("albums"), crate::i18n::text("songs")]
    }
}
/// The artist page's action words: shuffle, any album (with two or more), add.
pub fn artist_actions(albums: usize) -> Vec<&'static str> {
    let mut v = vec![crate::i18n::text("shuffle")];
    if albums > 1 {
        v.push(crate::i18n::text("any album"));
    }
    v.push(crate::i18n::text("add"));
    v
}
pub fn artist_list_top(albums: usize) -> i32 {
    page_list_top(strip_actions_top(), artist_actions(albums).len())
}
pub fn artist_album_h() -> i32 {
    px(72.0)
}
/// Whether the artist page shows its songs (else its albums): an artist without albums has only
/// songs.
pub fn artist_on_songs(page: &crate::library::ArtistPage) -> bool {
    page.songs_open || page.albums.is_empty()
}
fn artist_rows(page: &crate::library::ArtistPage) -> (i32, usize) {
    if artist_on_songs(page) {
        (row_h(Section::Tab(Tab::Songs)), page.tracks.len())
    } else {
        (artist_album_h(), page.albums.len())
    }
}
pub fn artist_max_scroll(page: &crate::library::ArtistPage) -> i32 {
    let (rh, n) = artist_rows(page);
    rows_max_scroll(artist_list_top(page.albums.len()), rh, n)
}
/// The row under `y`: an index into the page's albums or its tracks, whichever is showing.
pub fn artist_row_at(page: &crate::library::ArtistPage, scroll: i32, y: i32) -> Option<usize> {
    let (rh, n) = artist_rows(page);
    rows_at(artist_list_top(page.albums.len()), rh, n, scroll, y)
}

/// The artist page: the name as the heading, a strip (albums · songs), the action words, the rows.
/// Returns the strip's and the actions' targets.
#[allow(clippy::too_many_arguments)]
pub fn artist(
    c: &mut Canvas,
    t: &Theme,
    f: &FontSet,
    lib: &Library,
    page: &crate::library::ArtistPage,
    scroll: i32,
    playing_id: Option<i64>,
    sbar_active: bool,
) -> (Vec<(usize, i32, i32)>, Vec<ActionZone>) {
    c.fill(t.bg);
    heading(c, t, f, page.name);
    let words = artist_sections(page.albums.len());
    let on = usize::from(artist_on_songs(page) && !page.albums.is_empty());
    let strip = words_layout(f, &words, on);
    c.bleed_x();
    for &(i, x0, _) in &strip {
        let col = if i == on { t.ink } else { t.faint };
        text::draw(
            c,
            f,
            x0 as f32,
            strip_base(),
            words[i],
            &st(Weight::Regular, STRIP_SIZE, col),
        );
    }
    c.clear_clip_x();
    let labels = artist_actions(page.albums.len());
    let zones = actions_layout(f, &labels, strip_actions_top());
    draw_actions(c, t, f, &zones, &labels);
    let top = artist_list_top(page.albums.len());
    let (rh, n) = artist_rows(page);
    if artist_on_songs(page) {
        page_rows(c, t, top, rh, n, scroll, sbar_active, |c, y, i| {
            let tr = &page.tracks[i];
            let now = playing_id == Some(tr.song.object_id);
            two_lines(c, t, f, LEFT, y, &tr.song.title, tr.album, now);
        });
    } else {
        let tp = crate::library::THUMB_PX;
        page_rows(c, t, top, rh, n, scroll, sbar_active, |c, y, i| {
            let al = page.albums[i].1;
            crate::library::thumb(
                c,
                t,
                lib,
                al.album_id,
                &al.art,
                LEFT as i32,
                y + (rh - tp) / 2,
                tp,
                1.0,
            );
            let x = LEFT + tp as f32 + 14.0;
            let songs = crate::ui_format!("{} songs", al.tracks);
            let sub = if al.year.is_empty() || al.year == "0" {
                songs
            } else {
                format!("{} · {songs}", al.year)
            };
            let max_w = W as f32 - x - 22.0;
            let s = st(Weight::Regular, 23.0, t.ink);
            text::draw(
                c,
                f,
                x,
                (y + rh / 2 - px(2.0)) as f32,
                &crate::widgets::fit(f, &al.name, &s, max_w),
                &s,
            );
            let s2 = st(Weight::Regular, 16.0, t.dim);
            text::draw(
                c,
                f,
                x,
                (y + rh / 2 + px(22.0)) as f32,
                &crate::widgets::fit(f, &sub, &s2, max_w),
                &s2,
            );
        });
    }
    (strip, zones)
}

/// `words` as a strip from the current one, the way [`strip_layout`] lays out the sections:
/// `(index, x0, x1)`.
pub fn words_layout(f: &FontSet, words: &[&str], current: usize) -> Vec<(usize, i32, i32)> {
    let style = st(Weight::Regular, STRIP_SIZE, Rgb888::new(0, 0, 0));
    let gap = text::scaled(26.0);
    let mut x = LEFT;
    let mut out = Vec::new();
    for k in 0..words.len() {
        let i = (current + k) % words.len();
        if x as i32 >= W {
            break;
        }
        let w = text::measure(f, words[i], &style);
        out.push((i, x as i32, (x + w) as i32));
        x += w + gap;
    }
    out
}

/// The strip word under `x` (see [`strip_hit`]).
pub fn words_hit(zones: &[(usize, i32, i32)], x: i32) -> Option<usize> {
    zones.iter().enumerate().find_map(|(k, &(i, x0, x1))| {
        let left = if k == 0 { 0 } else { (zones[k - 1].2 + x0) / 2 };
        let right = zones.get(k + 1).map_or(W, |n| (x1 + n.1) / 2);
        (x >= left && x < right).then_some(i)
    })
}

// The playlist page.

/// What a playlist's action word does.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PlaylistDo {
    Play,
    Shuffle,
    Edit,
    AddSongs,
    Rename,
    Delete,
}

/// A playlist's action words: play and shuffle; edit for one Cinder owns or a smart one; and for
/// one Cinder owns, add songs, rename and delete.
pub fn playlist_actions(pl: &crate::model::PlaylistRow) -> Vec<(PlaylistDo, &'static str)> {
    let mut v = vec![
        (PlaylistDo::Play, crate::i18n::text("play")),
        (PlaylistDo::Shuffle, crate::i18n::text("shuffle")),
    ];
    if pl.user || pl.smart {
        v.push((PlaylistDo::Edit, crate::i18n::text("edit")));
    }
    if pl.user {
        v.push((PlaylistDo::AddSongs, crate::i18n::text("add songs")));
        v.push((PlaylistDo::Rename, crate::i18n::text("rename")));
        v.push((PlaylistDo::Delete, crate::i18n::text("delete")));
    }
    v
}
pub fn playlist_list_top(pl: &crate::model::PlaylistRow) -> i32 {
    page_list_top(block_actions_top(), playlist_actions(pl).len())
}
pub fn playlist_max_scroll(pl: &crate::model::PlaylistRow) -> i32 {
    rows_max_scroll(
        playlist_list_top(pl),
        row_h(Section::Tab(Tab::Songs)),
        pl.track_list.len(),
    )
}
pub fn playlist_row_at(pl: &crate::model::PlaylistRow, scroll: i32, y: i32) -> Option<usize> {
    rows_at(
        playlist_list_top(pl),
        row_h(Section::Tab(Tab::Songs)),
        pl.track_list.len(),
        scroll,
        y,
    )
}
/// The cover on the playlist page (a button on one Cinder owns: it picks the picture).
pub fn hit_cover(x: i32, y: i32) -> bool {
    let (x0, y0) = (LEFT as i32, block_top());
    (x0..x0 + PAGE_COVER).contains(&x) && (y0..y0 + PAGE_COVER).contains(&y)
}

/// The playlist page: its name as the heading, the cover and count, the action words, the songs.
pub fn playlist(
    c: &mut Canvas,
    t: &Theme,
    f: &FontSet,
    lib: &Library,
    pl: &crate::model::PlaylistRow,
    scroll: i32,
    playing: Option<usize>,
    sbar_active: bool,
) -> Vec<ActionZone> {
    c.fill(t.bg);
    heading(c, t, f, &pl.name);
    crate::library::playlist_thumb(c, t, lib, pl, LEFT as i32, block_top(), PAGE_COVER, 1.0);
    let kind = if pl.smart {
        crate::i18n::text("smart playlist")
    } else if pl.user {
        crate::i18n::text("your playlist")
    } else {
        crate::i18n::text("playlist")
    };
    block_words(
        c,
        t,
        f,
        &crate::ui_format!("{} songs", pl.track_list.len()),
        kind,
    );
    let acts = playlist_actions(pl);
    let labels: Vec<&str> = acts.iter().map(|a| a.1).collect();
    let zones = actions_layout(f, &labels, block_actions_top());
    draw_actions(c, t, f, &zones, &labels);
    let rh = row_h(Section::Tab(Tab::Songs));
    page_rows(
        c,
        t,
        playlist_list_top(pl),
        rh,
        pl.track_list.len(),
        scroll,
        sbar_active,
        |c, y, i| {
            let s = &pl.track_list[i];
            two_lines(c, t, f, LEFT, y, &s.title, &s.artist, playing == Some(i));
        },
    );
    zones
}

// The podcast show page.

/// The show page's action words: the newest episode, and carry on where the listener is.
pub fn show_actions() -> Vec<&'static str> {
    vec![crate::i18n::text("newest"), crate::i18n::text("continue")]
}
pub fn show_list_top() -> i32 {
    page_list_top(
        strip_actions_top() - (strip_bottom() - strip_top()),
        show_actions().len(),
    )
}
pub fn show_max_scroll(b: &crate::model::BookRow) -> i32 {
    rows_max_scroll(show_list_top(), row_h(Section::Podcasts), b.chapters.len())
}
/// The episode (chapter index) under `y`: the rows run newest first.
pub fn show_row_at(b: &crate::model::BookRow, scroll: i32, y: i32) -> Option<usize> {
    let n = b.chapters.len();
    rows_at(show_list_top(), row_h(Section::Podcasts), n, scroll, y).map(|r| n - 1 - r)
}

/// One show: its name, the action words, its episodes newest first, each with its date and
/// whether it is new, part-heard or heard.
pub fn show(
    c: &mut Canvas,
    t: &Theme,
    f: &FontSet,
    b: &crate::model::BookRow,
    scroll: i32,
    playing_id: Option<i64>,
    sbar_active: bool,
) -> Vec<ActionZone> {
    c.fill(t.bg);
    heading(c, t, f, &b.title);
    let labels = show_actions();
    let zones = actions_layout(
        f,
        &labels,
        strip_actions_top() - (strip_bottom() - strip_top()),
    );
    draw_actions(c, t, f, &zones, &labels);
    let n = b.chapters.len();
    let rh = row_h(Section::Podcasts);
    page_rows(
        c,
        t,
        show_list_top(),
        rh,
        n,
        scroll,
        sbar_active,
        |c, y, r| {
            let i = n - 1 - r;
            let ep = &b.chapters[i];
            let state = if b.finished || i < b.chapter {
                crate::i18n::text("heard").to_string()
            } else if i == b.chapter && b.pos_ms > 0 {
                crate::ui_format!("{} min in", b.pos_ms / 60_000)
            } else {
                crate::i18n::text("new").to_string()
            };
            let sub = match b.chapter_date(i) {
                Some(d) => format!("{d} · {state}"),
                None => state,
            };
            let now = playing_id == Some(ep.object_id);
            let heard = b.finished || i < b.chapter;
            let max_w = W as f32 - LEFT - 22.0;
            let col = if now {
                t.acc
            } else if heard {
                t.dim
            } else {
                t.ink
            };
            let s1 = st(
                if now {
                    Weight::SemiBold
                } else {
                    Weight::Regular
                },
                25.0,
                col,
            );
            text::draw(
                c,
                f,
                LEFT,
                (y + px(34.0)) as f32,
                &crate::widgets::fit(f, &ep.title, &s1, max_w),
                &s1,
            );
            let s2 = st(Weight::Regular, 17.0, if heard { t.faint } else { t.dim });
            text::draw(
                c,
                f,
                LEFT,
                (y + px(60.0)) as f32,
                &crate::widgets::fit(f, &sub, &s2, max_w),
                &s2,
            );
        },
    );
    zones
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

/// What Home's top line says: the song and its artist, how far in, and whether it is playing.
/// `None` when nothing is loaded.
pub struct HomeNow<'a> {
    pub title: &'a str,
    pub artist: &'a str,
    /// 0..1.
    pub progress: f32,
    pub playing: bool,
}

/// Home's live word beside a destination: the podcasts' new episodes, the book you are in.
/// Empty for none. Counted from the library alone, so it is right the moment Home draws.
pub fn home_hint(lib: &Library, podcasts: bool) -> String {
    if podcasts {
        let new: usize = shelf(lib, true).iter().map(|&i| unheard(&lib.books[i])).sum();
        if new == 0 {
            String::new()
        } else {
            crate::ui_format!("{} new", new)
        }
    } else {
        let started: Vec<&crate::model::BookRow> = shelf(lib, false)
            .into_iter()
            .map(|i| &lib.books[i])
            .filter(|b| b.in_progress())
            .collect();
        match started.as_slice() {
            [] => String::new(),
            [b] => b.title.clone(),
            many => crate::ui_format!("{} started", many.len()),
        }
    }
}

/// Home: what is playing, then the destinations as large lowercase words, each with its live
/// hint (`MenuItem::sub`, empty for none) in the accent after it when there is room.
pub fn home(
    c: &mut Canvas,
    t: &Theme,
    f: &FontSet,
    now: Option<&HomeNow>,
    items: &[crate::menu::MenuItem],
    home: usize,
    scroll: i32,
) {
    c.fill(t.bg);
    let room = (W as f32) - 2.0 * LEFT;
    let base = (home_strip_bottom() - px(16.0)) as f32;
    match now {
        Some(n) => {
            // "now playing" (or "paused") small in the accent, the song in ink, the artist dim,
            // and a hairline of progress under it: the whole band is the tap to Now Playing.
            let lead = st(Weight::Regular, 15.0, t.acc);
            let word = if n.playing {
                crate::i18n::text("now playing")
            } else {
                crate::i18n::text("paused")
            };
            let lw = text::measure(f, word, &lead);
            text::draw(c, f, LEFT, base, word, &lead);
            let x = LEFT + lw + text::scaled(10.0);
            let song = st(Weight::Regular, 18.0, t.ink);
            let title = crate::widgets::fit(f, n.title, &song, room - (x - LEFT));
            let tw = text::measure(f, &title, &song);
            text::draw(c, f, x, base, &title, &song);
            if !n.artist.is_empty() {
                let by = st(Weight::Regular, 18.0, t.dim);
                let gap = text::scaled(8.0);
                let left = room - (x - LEFT) - tw - gap;
                if left > text::scaled(40.0) {
                    let a = crate::widgets::fit(f, n.artist, &by, left);
                    text::draw(c, f, x + tw + gap, base, &a, &by);
                }
            }
            let y = home_strip_bottom() - px(6.0);
            let w = room as i32;
            crate::widgets::fill_rect(c, LEFT as i32, y, w, 2, t.line);
            let done = (w as f32 * n.progress.clamp(0.0, 1.0)) as i32;
            crate::widgets::fill_rect(c, LEFT as i32, y, done, 2, t.acc);
        }
        None => {
            let s = st(Weight::Regular, 18.0, t.dim);
            text::draw(c, f, LEFT, base, crate::i18n::text("nothing playing"), &s);
        }
    }
    c.set_clip_y(home_top(), crate::H as i32);
    c.bleed_x();
    let hint = st(Weight::Regular, 18.0, t.acc);
    for (i, m) in items.iter().enumerate() {
        let y = home_top() + i as i32 * home_row_h() - scroll;
        let col = if m.active { t.acc } else { t.ink };
        let word = crate::i18n::text(m.label).to_lowercase();
        let big = st(Weight::Light, 46.0, col);
        let base = (y + px(50.0)) as f32;
        text::draw(c, f, LEFT, base, &word, &big);
        if !m.sub.is_empty() {
            let x = LEFT + text::measure(f, &word, &big) + text::scaled(12.0);
            let left = W as f32 - LEFT - x;
            if left >= text::scaled(48.0) {
                let h = crate::widgets::fit(f, m.sub, &hint, left);
                text::draw(c, f, x, base, &h, &hint);
            }
        }
    }
    c.clear_clip_x();
    let rows = items.len();
    crate::kit::section_label(
        c,
        t,
        f,
        home_label_top(rows) - scroll,
        crate::i18n::text("START ON"),
        None,
    );
    let on = home.min(crate::menu::HOMES.len() - 1);
    crate::kit::chips_in(
        c,
        t,
        f,
        home_chips_top(rows) - scroll,
        &crate::menu::HOMES,
        Some(on),
        crate::menu::HOMES.len(),
    );
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
        let s = sections(&lib, Section::Tab(Tab::Songs));
        assert!(
            !s.contains(&Section::Books) && !s.contains(&Section::Podcasts),
            "music only"
        );
        let last = *s.last().unwrap();
        assert_eq!(
            neighbour(&lib, Section::Tab(Tab::Songs), -1),
            Section::Tab(Tab::Albums)
        );
        assert_eq!(neighbour(&lib, Section::Tab(Tab::Songs), 1), last);
        assert_eq!(neighbour(&lib, last, -1), Section::Tab(Tab::Songs));
        assert_eq!(neighbour(&lib, Section::Podcasts, -1), Section::Books);
        assert_eq!(
            neighbour(&lib, Section::Books, -1),
            Section::Podcasts,
            "the spoken pair wraps"
        );
    }

    #[test]
    fn a_show_is_a_podcasts_folder_or_dated_episodes() {
        use crate::model::BookRow;
        let book = |dir: &str, author: &str, files: &[&str]| BookRow {
            author: author.into(),
            dir: dir.into(),
            paths: files.iter().map(|f| format!("{dir}/{f}")).collect(),
            ..Default::default()
        };
        let l = "/contents/LEARNING";
        assert!(book(&format!("{l}/Podcasts/Show"), "Podcasts", &["a.mp3"]).is_podcast());
        assert!(book(
            &format!("{l}/Show"),
            "",
            &["2026-11-03 One.mp3", "2026-11-10 Two.mp3"]
        )
        .is_podcast());
        assert!(!book(
            &format!("{l}/Le Guin/Earthsea"),
            "Le Guin",
            &["01.mp3", "02.mp3"]
        )
        .is_podcast());
        assert!(
            !book(&format!("{l}/Mixed"), "", &["2026-11-03 One.mp3", "02.mp3"]).is_podcast(),
            "one undated file and it is a book"
        );
        let b = BookRow {
            chapters: vec![Default::default(); 4],
            chapter: 2,
            pos_ms: 1,
            ..Default::default()
        };
        assert_eq!(unheard(&b), 1, "after the one in progress");
        assert_eq!(
            unheard(&BookRow {
                pos_ms: 0,
                ..b.clone()
            }),
            2,
            "it, and the one after"
        );
        assert_eq!(
            unheard(&BookRow {
                finished: true,
                ..b
            }),
            0
        );
    }

    #[test]
    fn home_hints_count_new_episodes_and_name_the_book_you_are_in() {
        use crate::model::BookRow;
        let mut lib = Library::sample();
        let show = |n: usize, chapter: usize| BookRow {
            author: "Podcasts".into(),
            dir: "/contents/LEARNING/Podcasts/S".into(),
            chapters: vec![Default::default(); n],
            chapter,
            ..Default::default()
        };
        let book = |title: &str, chapter: usize, finished: bool| BookRow {
            title: title.into(),
            dir: format!("/contents/LEARNING/{title}"),
            paths: vec!["01.mp3".into()],
            chapters: vec![Default::default(); 3],
            chapter,
            finished,
            ..Default::default()
        };
        lib.books = vec![show(4, 1), show(2, 0), book("Earthsea", 0, false)];
        assert_eq!(home_hint(&lib, true), "5 new");
        assert_eq!(home_hint(&lib, false), "", "a book not begun is not one you are in");
        lib.books.push(book("Dune", 2, false));
        lib.books.push(book("Done", 2, true));
        assert_eq!(home_hint(&lib, false), "Dune");
        lib.books[2].pos_ms = 5;
        assert_eq!(home_hint(&lib, false), "2 started");
        lib.books.retain(|b| !b.is_podcast());
        assert_eq!(home_hint(&lib, true), "", "no shows, no hint");
    }

    #[test]
    fn rows_are_found_where_they_are_drawn() {
        let lib = Library::sample();
        let sec = Section::Tab(Tab::Songs);
        let y = list_top() + row_h(sec) + 3;
        assert_eq!(row_at(sec, &lib, 0, 0, 0, y), Some(1));
        assert_eq!(
            row_at(sec, &lib, 0, 0, row_h(sec), list_top() + 3),
            Some(1),
            "scrolled one row"
        );
        assert_eq!(
            row_at(sec, &lib, 0, 0, 0, list_top() - 1),
            None,
            "the strip is not a row"
        );
    }
}
