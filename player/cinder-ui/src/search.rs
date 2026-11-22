//! Library search — find a song by any part of its title, artist or album name, and play it; or
//! an artist, album or audiobook by its name, and open it.
//!
//! Results come in that order: artists, albums, audiobooks, then songs. A row that opens a page
//! wears a chevron; a song row wears the play mark and swipes to the queue like every other song.
//!
//! Scopes (redesign 1a, R3): a line of chips under the band — all · songs · albums · artists —
//! keeps only that kind of result. "All" is the old search, audiobooks included.
//!
//! An opt-in component (`search` in `components.conf`): the Library header draws the button only
//! when the shell says it is installed. The layout is the "Add tracks" picker's — a fixed search band
//! over a plain list — because that screen already taught the idiom, and its geometry and hit tests
//! are shared rather than copied, so the two cannot drift.

use crate::playlist_pick::{content_h, BOTTOM, LIST_TOP, ROW_H, SEARCH_H, TOP};

/// The scopes, in chip order. Index = `App`'s search scope.
pub const SCOPES: [&str; 4] = ["All", "Songs", "Albums", "Artists"];
pub const SCOPE_ALL: usize = 0;
pub const SCOPE_SONGS: usize = 1;
pub const SCOPE_ALBUMS: usize = 2;
pub const SCOPE_ARTISTS: usize = 3;

/// Does scope `scope` keep `hit`?
pub fn in_scope(scope: usize, hit: Hit) -> bool {
    match scope {
        SCOPE_SONGS => matches!(hit, Hit::Song(_)),
        SCOPE_ALBUMS => matches!(hit, Hit::Album(_)),
        SCOPE_ARTISTS => matches!(hit, Hit::Artist(_)),
        _ => true,
    }
}

/// The scope chips' top, straight under the band, and the results' top under them.
pub const SCOPES_TOP: i32 = LIST_TOP + 6;
pub const RESULTS_TOP: i32 = SCOPES_TOP + crate::kit::CHIP_H + 6;

/// The scope chip under `(x, y)`.
pub fn scope_at(x: i32, y: i32) -> Option<usize> {
    crate::kit::chip_at(SCOPES.len(), SCOPES_TOP, x, y)
}

/// The result row under `y`, scrolled `scroll_px`. None on the band, the chips or empty space.
pub fn row_at(rows: usize, scroll_px: i32, y: i32) -> Option<usize> {
    if !(RESULTS_TOP..BOTTOM).contains(&y) {
        return None;
    }
    let i = ((y - RESULTS_TOP + scroll_px.max(0)) / ROW_H) as usize;
    (i < rows).then_some(i)
}

pub fn max_scroll(rows: usize) -> i32 {
    (content_h(rows) - (BOTTOM - RESULTS_TOP)).max(0)
}
use crate::text::{self, Family, FontSet, Weight};
use crate::theme::Theme;
use crate::widgets::{fill_rect, fit, hline, sty};
use crate::Canvas;

/// One result: an index into `lib.artists`, `lib.albums_flat()`, `lib.books` or `lib.songs`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hit {
    Artist(usize),
    Album(usize),
    Book(usize),
    Song(usize),
}

/// What a result row shows. `opens` = it leads to a page (chevron) rather than playing (play mark).
pub struct HitRow {
    pub title: String,
    pub sub: String,
    pub opens: bool,
}

pub fn render(
    c: &mut Canvas,
    t: &Theme,
    f: &FontSet,
    songs: &[HitRow],
    query: &str,
    total: usize,
    scroll_px: i32,
    sbar_active: bool,
    scope: usize,
) {
    c.fill(t.bg);
    let has = !query.trim().is_empty();
    let caption = if has {
        crate::ui_format!("{} FOUND", songs.len())
    } else {
        crate::ui_format!("{total} SONGS", total = total)
    };
    crate::chrome::header(c, t, f, crate::i18n::text("Search"), Some(&caption));

    // ── the search band: shows the query, and a tap on it edits it ────────────────────────────
    // Pivot: a field is an underline in the accent, not a band.
    let pv = crate::pivot::on(t);
    if pv {
        fill_rect(c, 22, TOP + SEARCH_H - 10, 436, 2, t.acc);
    } else {
        fill_rect(c, 0, TOP, crate::canvas::W as i32, SEARCH_H, t.panel);
        hline(c, TOP + SEARCH_H - 1, t.line);
    }
    let qs = sty(
        Family::Sans,
        Weight::Regular,
        if pv { 20.0 } else { 17.0 },
        if has { t.ink } else { t.faint },
        0.0,
    );
    let shown = if has {
        query
    } else {
        crate::i18n::text("Title, artist or album")
    };
    text::draw(
        c,
        f,
        26.0,
        (TOP + SEARCH_H / 2 + 5) as f32,
        &fit(f, shown, &qs, 420.0),
        &qs,
    );

    crate::kit::chips_in(
        c,
        t,
        f,
        SCOPES_TOP,
        &SCOPES,
        Some(scope.min(SCOPES.len() - 1)),
        SCOPES.len(),
    );

    if songs.is_empty() {
        // Say which of the two empty states this is: nothing asked, or nothing found.
        let st = sty(Family::Sans, Weight::Regular, 17.0, t.dim, 0.0);
        let note = if has {
            crate::i18n::text("Nothing matches.")
        } else {
            crate::i18n::text("Tap above and start typing.")
        };
        text::draw(c, f, 26.0, (RESULTS_TOP + 36) as f32, note, &st);
        return;
    }

    c.set_clip_y(RESULTS_TOP, BOTTOM);
    let first = (scroll_px.max(0) / ROW_H) as usize;
    let mut y = RESULTS_TOP - (scroll_px.max(0) % ROW_H);
    let title_style = sty(
        Family::Sans,
        Weight::SemiBold,
        crate::scale::ROW,
        t.ink,
        0.0,
    );
    let artist_style = sty(Family::Sans, Weight::Regular, 14.0, t.dim, 0.0);
    for song in songs.iter().skip(first) {
        if y >= BOTTOM {
            break;
        }
        let cy = y + ROW_H / 2;
        text::draw(
            c,
            f,
            26.0,
            (cy - 2) as f32,
            &fit(f, &song.title, &title_style, 380.0),
            &title_style,
        );
        text::draw(
            c,
            f,
            26.0,
            // 19, not the picker's 17: a CJK second line ("歌手") is taller than a Latin artist
            // name and touched the title at the largest text size (tests/ui_overflow.rs).
            (cy + 19) as f32,
            &fit(f, &song.sub, &artist_style, 380.0),
            &artist_style,
        );
        if pv {
            // No chevrons or play marks in Pivot: the row is the word.
        } else if song.opens {
            crate::icons::chevron(c, 440.0, cy as f32, 14.0, t.faint);
        } else {
            crate::icons::play(c, 440.0, cy as f32, 14.0, t.faint);
        }
        hline(c, y + ROW_H, t.line);
        y += ROW_H;
    }
    c.clear_clip();
    // The same track bounds the "Add tracks" list uses, so `nav::sbar_metrics` can share them.
    crate::library::scrollbar(
        c,
        t,
        TOP,
        BOTTOM,
        scroll_px,
        content_h(songs.len()),
        sbar_active,
    );
}
