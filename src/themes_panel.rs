//! themes_panel — Theme picker state (600x366) for dynamite-rust.
//!
//! Layout spec + `draw_themes_panel()` in the same manual-SDF /
//! Argb8888 style as `wallpaper_panel.rs` (no tiny-skia, no
//! cosmic-text, no new dependencies — only `fontdue`, already in
//! Cargo.toml).
//!
//! SOURCES (do not re-derive from memory):
//! - `frames/per-second/s12.png` — video frame, RE-MEASURED FRESH
//!   this run (iter16) with PIL: panel frame origin (220,5) in the
//!   640x360 frame, x3 = panel-relative logical px, +/-3 logical px
//!   granularity. Every layout constant below comes from that pass
//!   (see MEASUREMENTS); nothing is copied from the dead iter8 file.
//! - `iterations/builder-pickers-iter13.md` — the iter13 spec; its
//!   numbers were RE-CHECKED against s12 this run. Most survive
//!   (selected row 66..162, thumb 357..582, rows pitch 48, dots
//!   right-edge on unselected rows). CORRECTIONS found this run:
//!   (a) SIX dots per row, not five (iter13/QS said five — the
//!   640x360 frame plainly shows six in the selected row and six at
//!   the right edge of the ariadne row); (b) the anime palette order
//!   is pink, GREEN, YELLOW, blue, purple — QS has yellow and green
//!   swapped; (c) unselected rows carry a small circular ICON at
//!   the left (dark disc + teal dot), which iter13 did not record
//!   and QS does not have at all; (d) the selected row has a small
//!   white bullet dot before the name; (e) unselected names start
//!   at x ~63 (after the icon), not x 33.
//! - `configs/quickshell/dynamite/shell.qml` ("THEME PICKER") —
//!   the five theme names + dot palettes. The palettes are QS
//!   approximations from the frames, NOT video-sampled per dot:
//!   at 640x360 each dot is ~3x3 frame px of AA blend, so exact
//!   per-dot RGB is unresolvable. Hue order is verified from s12
//!   (anime corrected, see above); the values are kept from QS
//!   and flagged as QS-derived. The 6th dot's colour per theme is
//!   the brightest-pixel-in-patch sample from s12 (also
//!   approximate, flagged). Do not claim video-sampled.
//! - `iterations/critic-pickers-iter13.md` — harsh critic: geometry
//!   PASS for five iterations, content 0% (placeholder word only).
//!   This module is the first real content for the state.
//!
//! WIRING (integrator — `main.rs` is patched separately, this file
//! only defines the panel):
//! ```ignore
//! // main.rs, with the other state-content mods:
//! mod themes_panel;
//! // main.rs, in draw(), before the `other =>` placeholder arm:
//! //   IslandState::Themes => {
//! //       themes_panel::draw_themes_panel(canvas, w, h, font);
//! //   }
//! // No socket-word changes: `themes` already dispatches via
//! // IslandState::from_word. No icon-font change: the panel needs
//! // no icon_font_for entry (procedural magnifier + drawn discs
//! // only) — do NOT add one (avoids the ~19 MB lazy load).
//! ```
//!
//! ITER18 (13 Nov 2026, local-only verification — module NOT rewired,
//! live island NOT touched):
//! - The iter16 module was already complete (1052 lines + real thumb
//!   blob); this run VERIFIED it instead of rewriting it. One real
//!   bug found and fixed: the test loop used `.iter()` so `row_y` was
//!   `&f32` and the `row_y as i32` uses in that loop are E0605 —
//!   the `#[cfg(test)]` module NEVER compiled, so no themes render
//!   test ever ran. Fix: `.into_iter()` (one line). `cargo check`
//!   (non-test) was unaffected by this bug.
//! - GEOMETRY NOTE (critic-pickers-iter16: live placeholder measured
//!   600x364.7, top 16.0 vs spec 366 / 15.3): RESOLVED, no code
//!   change. `PANEL_H` is 366 and `main.rs` `size()` is (600,366)
//!   with `TOP_MARGIN` 15; the critic measured the PLACEHOLDER
//!   capsule (main.rs-owned, this module never fills it), and a
//!   fresh s12 PIL pass this run puts the video panel's own bottom
//!   edge at ~364.5-366 logical at 640x360 granularity (+/-3 px) —
//!   the 1.3 px is AA-threshold noise, not a panel defect. The
//!   highlight sample was re-verified fresh: (25,25,37) = #1A1A25.
//! - Thumb blob re-verified: 86,400 B, sha256 matches the header
//!   doc, mean RGB (143,176,245) bright sky, 92 transparent corner px.
//! - NO Rust toolchain exists on this VM (no cargo/rustc, no
//!   ~/.cargo, no vendored crates) — `cargo check` / `cargo test`
//!   could NOT be run locally; see iterations/builder-themes-iter18.md.
//!   The integrator MUST run both on the Surface scratch copy
//!   (expect 0 errors, 28/28 tests incl. the 2 fixed themes tests).
//!
//! SEARCH — FLAT line, REAL type-to-filter (iter128 B2, same as
//! the wallpaper panel): the video has NO search field, just a
//! magnifier glyph and grey "Search themes..." sitting flat on the
//! black panel. The clean sample at the search-bar centre is
//! (2,2,0) — essentially black, so NO #1A1C1F fill is drawn (the
//! dead file's SEARCH_BG was a QS invention; the video wins).
//! Typing while the view is open filters the 24 library themes
//! live (theme-name substring, case-insensitive); the query renders
//! as plain white text, with NO caret and NO box. The live state is
//! THEME_SEARCH below (`ThemesData::search_text` is only the
//! compiled-in fallback panel's display field).
//!
//! THUMBNAIL — REAL FILE, pre-decoded RGBA blob:
//! The selected row's right side is a REAL sky/cloud photo, not a
//! gradient: the live wallpaper
//! `~/.config/mango/wallpaper/wallpaper.png` (the iter14 fallback
//! sky), cover-fit by `icons-src/extract-themes-thumb.py` (PIL,
//! LANCZOS, rounded-corner alpha baked in, r10) and embedded via
//! `include_bytes!`. No PNG decoder exists in this binary and none
//! is needed. The blob is `&'static [u8]` in .rodata (86,400 B):
//! demand-paged, ~0 PSS until the themes state is first drawn.
//! The video's thumbnail fades from the row highlight into the
//! photo over its left ~110 px (measured horizontal profile at
//! prel y 105: RGB (28,46,90) at x 360 rising to (208,217,233)
//! at x 450); the fade is drawn in Rust as a black overlay with
//! alpha 0.9 at the left edge easing to 0 at x 467, inside the
//! rounded-rect mask — the blob itself is the raw photo.
//!
//! TRUTHFULNESS (hard rules, do not relax):
//! - The counter shows the REAL rendered-theme count `1/5` — NOT
//!   the video's `1/16` (no 16-theme collection exists anywhere in
//!   the replica's sources; QS lists exactly these five themes).
//! - Theme names are exactly the five QS/video names (anime,
//!   ariadne, catppuccin, everblush, everforest) — never invented.
//! - No `wallhaven-4d9....png` string appears anywhere here (the
//!   exact Wallhaven file is proven deleted; see
//!   analysis/wallpaper.md). The thumbnail is the live wallpaper
//!   file, not a Wallhaven reconstruction.
//! - Dot palettes: QS values, hue order verified vs s12 (anime
//!   corrected), 6th dot + icon teals are s12 brightest-patch
//!   samples — all flagged APPROXIMATE in the code, never claimed
//!   as video-sampled.
//!
//! APPROXIMATIONS (honest, flagged — not video-sampled):
//! - Per-dot RGB (dots 1-5): QS values; hue order verified, exact
//!   values unresolvable at 640x360 (each dot ~3x3 frame px).
//! - 6th dot colours + unselected-row icon teals: brightest-pixel
//!   in a small patch around the measured dot centre (s12) —
//!   centre-pixel sampling is +/-1.5 px fragile at this resolution.
//! - Icon disc fill #101218 and dot diameters are from the visible
//!   shape (dark disc ~24 px, teal dot ~11 px), not measured to
//!   the pixel; the icon's exact glyph/semantics are unknown — it
//!   is drawn as what it looks like (disc + dot), nothing more.
//! - Thumbnail fade curve (alpha 0.9 -> 0 over x 357..467) is fit
//!   to the measured horizontal profile; the residual is photo
//!   content (dark cloud at the photo's left edge).
//! - Highlight fill #1A1A25 from the clean sample (25,25,37) at
//!   (300,40); magnifier centre (24,31) +/-3 px.
//!
//! TEXT WEIGHT: `Quicksand-Bold.ttf` IS the text font (main.rs loads
//! it), so the `bold` flag's double-blit is OFF at every call site
//! in this file (the base font is already Bold; double-blitting it
//! would over-thicken the labels). The flag stays in the signature —
//! pass `false`.
//!
//! BACKGROUND: this function does NOT fill the panel — `main.rs`
//! `draw()` already fills the black capsule for this state (unlike
//! ControlCenter/Launcher, which fill their own). The unit test
//! replicates that by pre-filling its canvas opaque black.
//!
//! ---------------------------------------------------------------------------
//! MEASUREMENTS (iter16, s12.png, PIL — panel frame origin (220,5), x3)
//! ---------------------------------------------------------------------------
//!   Panel                       220..420 x 5..127 frame  = 600x366
//!   Search line (flat)          y 14..48, centre 31
//!     magnifier                 centre ~(24,31), ring r6
//!     "Search themes..." 12px   x 45.., glyph band y 21..33
//!     counter "1/16" (video)    right edge ~576, y ~12..27
//!       -> rendered "1/5" (truthful; see TRUTHFULNESS)
//!   Selected row "anime"        x 14..586, y 66..162 (h 96), r10
//!     highlight fill            (25,25,37) clean sample -> #1A1A25
//!     bullet (white dot)        centre (34,100), Ø ~9
//!     name "anime" 13px         x 46, glyph band y 96..108
//!     dots (6)                  cy 132, Ø ~9, centres x
//!                               34, 52, 72, 90, 108, 129
//!     thumbnail (real photo)    x 357..582, y 66..162 (225x96)
//!     thumb fade                left ~110 px (x 357..467)
//!   Unselected rows (pitch 48)  y 168..216 / 216..264 /
//!                               264..312 / 312..360, no fill
//!     icon disc                 centre (36,row_cy), Ø ~24, #101218
//!     icon teal dot             same centre, Ø ~11, per-row teal
//!     name 13px                 x 63, tops row_y+13
//!       glyph bands: ariadne 183..192, catppuccin 228..240,
//!       everblush 276..285, everforest 324..333
//!     dots (6, right edge)      cy row_y+21, Ø ~9, centres x
//!                               508, 523, 535, 552, 565, 580
//!   Row centres: selected 114; unselected 192, 240, 288, 336.
//! ---------------------------------------------------------------------------
//! COLOURS:
//!   PANEL           #000000  (0,0,0)      — filled by main.rs, not here
//!   HIGHLIGHT       #05163C  (5,22,60)   — selected-row fill (iter96 tint; was sampled #1A1A25)
//!   PRIMARY         #FFFFFF                — theme names
//!   SECONDARY       #FFFFFF  (255,255,255)— search placeholder + counter
//!     (iter128: was #9AA0A6, measured peak lum ~143-166 on screen / in
//!     the buffer — too dim at 10-12 px; white peaks ~255 in-buffer.
//!     Hierarchy is kept by SIZE: meta 12 px stays below names 13 px.)
//!   ICON_DISC       #101218  (16,18,24)   — unselected-row icon disc
//!   Dot palettes    see THEMES below     — QS-derived + s12 dot-6
//! ---------------------------------------------------------------------------

use fontdue::Font;
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
pub const PANEL_W: u32 = 600;
pub const PANEL_H: u32 = 366;
// (Panel corner radius 20 is owned by main.rs IslandState::radius;
// this module never fills the capsule, so it defines no radius const.)

/// Search line: flat, no field fill (video wins over QS).
const SEARCH_Y: f32 = 14.0;
const SEARCH_H: f32 = 34.0; // 14..48, centre 31
const MAGNIFIER_CX: f32 = 24.0;
const MAGNIFIER_CY: f32 = SEARCH_Y + SEARCH_H / 2.0; // 31
const SEARCH_TEXT_X: f32 = 45.0;
const SEARCH_TEXT_TOP: f32 = 19.0; // 12 px, glyph band ~21..33 (iter16)
const SEARCH_PX: f32 = 12.0;
const SEARCH_PLACEHOLDER: &str = "Search themes...";
const COUNTER_PX: f32 = 12.0; // iter128: was 10.0 — counter must read at least as tall as the search line (12 px), still below names (13 px)
const COUNTER_RIGHT: f32 = 576.0;
const COUNTER_TOP: f32 = 21.0; // 10 px, centred in the 34 px bar

/// Selected row: x 14..586, y 66..162 (h 96), r 10.
const ROW_X: f32 = 14.0;
const ROW_W: f32 = PANEL_W as f32 - 2.0 * ROW_X; // 572
const ROW_R: f32 = 10.0;
const SEL_ROW_Y: f32 = 66.0;
const SEL_ROW_H: f32 = 96.0;
/// Selected-row name "anime": 13 px, x 46, glyph band y 96..108.
const SEL_NAME_X: f32 = 46.0;
const SEL_NAME_TOP: f32 = 94.0;
/// Selected-row bullet: white dot Ø 9 at (34, 100).
const BULLET_CX: f32 = 34.0;
const BULLET_CY: f32 = 100.0;
const BULLET_D: f32 = 9.0;
/// Selected-row dots: 6 x Ø 9, cy 132, centres x below.
const SEL_DOT_CY: f32 = 132.0;
const SEL_DOT_XS: [f32; 6] = [34.0, 52.0, 72.0, 90.0, 108.0, 129.0];
const DOT_D: f32 = 9.0;

/// Thumbnail: x 357..582, y 66..162 (225x96), r 10; fade over the
/// left 110 px (x 357..467).
pub const THUMB_W: u32 = 225;
pub const THUMB_H: u32 = 96;
const THUMB_X: f32 = 357.0;
const THUMB_Y: f32 = 66.0;
const THUMB_R: f32 = 10.0;
const FADE_X0: f32 = 357.0;
const FADE_X1: f32 = 467.0;
const FADE_MAX: f32 = 0.9;

/// Unselected rows: pitch 48, y 168..216 / 216..264 / 264..312 /
/// 312..360; no fill.
const ROW_PITCH: f32 = 48.0;
const ROW_FIRST_Y: f32 = 168.0;
/// Unselected-row icon: dark disc Ø 24 + teal dot Ø 11, centre
/// (36, row centre).
const ICON_CX: f32 = 36.0;
const ICON_DISC_D: f32 = 24.0;
const ICON_DOT_D: f32 = 11.0;
/// Unselected-row name: 13 px, x 63, top row_y + 13.
const NAME_X: f32 = 63.0;
const NAME_TOP_OFF: f32 = 13.0;
/// Unselected-row dots (right edge): 6 x Ø 9, cy row_y + 21.
const DOT_CY_OFF: f32 = 21.0;
const DOT_XS: [f32; 6] = [508.0, 523.0, 535.0, 552.0, 565.0, 580.0];

const NAME_PX: f32 = 13.0;

const C_HIGHLIGHT: (u8, u8, u8) = (25, 26, 31); // #191A1F, selected row - r1d: reference selection is neutral dark, not navy (was #05163C iter96)
const C_PRIMARY: (u8, u8, u8) = (255, 255, 255);
const C_SECONDARY: (u8, u8, u8) = (255, 255, 255); // #FFFFFF — iter128: meta text white, was #9AA0A6 (154,160,166), peak lum ~143-166, contrast ~6:1. White peaks ~255 in-buffer (~237 live after the 1.5x upscale). Hierarchy kept by size (12 px vs 13 px names), not by dimming.
const C_QUERY: (u8, u8, u8) = (255, 255, 255);
const C_ICON_DISC: (u8, u8, u8) = (16, 18, 24); // #101218, s12 shape
/// r1d: unselected-row chips are rounded SQUARES (35x29 device px,
/// radius 6) at scale 1.5, not discs.
const CHIP_W: f32 = 23.3;
const CHIP_H: f32 = 19.3;
const CHIP_R: f32 = 4.0;
/// r1d: search row divider #1F2127, 1 logical px, and the 2 px caret
/// that sits before an empty placeholder.
const C_DIVIDER: (u8, u8, u8) = (31, 33, 39);
const CARET_W: f32 = 2.0;
const CARET_H: f32 = 14.0;

// ---------------------------------------------------------------------------
// Themes — names + dot palettes (6 dots per row, s12-measured count).
//
// Dots 1-5: QS values verbatim from shell.qml, hue order verified
// against s12 — EXCEPT anime, where the video order is pink, GREEN,
// YELLOW, blue, purple (QS lists yellow before green; the s12 centre
// samples say otherwise: dot2 (194,206,174) green, dot3 (214,198,166)
// yellow). The other four themes keep QS order (dot2/dot3 order
// unverified there — flagged, not fudged).
// Dot 6: s12 brightest-pixel-in-patch sample per theme (APPROX — see
// module docs). Anime dot6 (196,224,230) pale cyan; ariadne
// (137,169,167); catppuccin (220,221,226); everblush (165,175,176);
// everforest (199,199,194).
// ---------------------------------------------------------------------------
pub struct ThemeDef {
    pub name: &'static str,
    pub dots: [(u8, u8, u8); 6],
    /// Unselected-row icon teal dot (s12 brightest-patch sample).
    pub icon_teal: (u8, u8, u8),
}

pub const THEMES: [ThemeDef; 5] = [
    ThemeDef {
        name: "anime",
        dots: [
            (0xF7, 0x76, 0x8E), // #F7768E pink
            (0x9E, 0xCE, 0x6A), // #9ECE6A green (QS order corrected)
            (0xE0, 0xAF, 0x68), // #E0AF68 yellow (QS order corrected)
            (0x7A, 0xA2, 0xF7), // #7AA2F7 blue
            (0xBB, 0x9A, 0xF7), // #BB9AF7 purple
            (196, 224, 230),    // s12 dot6 sample, pale cyan (APPROX)
        ],
        icon_teal: (136, 200, 189), // s12 sample (APPROX)
    },
    ThemeDef {
        name: "ariadne",
        dots: [
            (0xD8, 0xA7, 0xB1), // #D8A7B1
            (0xB6, 0xE2, 0xD3), // #B6E2D3
            (0xFB, 0xE8, 0xA6), // #FBE8A6
            (0x95, 0xB8, 0xD1), // #95B8D1
            (0xC5, 0xA3, 0xCB), // #C5A3CB
            (137, 169, 167),    // s12 dot6 sample (APPROX)
        ],
        icon_teal: (136, 200, 189), // s12 sample (APPROX)
    },
    ThemeDef {
        name: "catppuccin",
        dots: [
            (0xCB, 0xA6, 0xF7), // #CBA6F7
            (0xF5, 0xC2, 0xE7), // #F5C2E7
            (0xA6, 0xE3, 0xA1), // #A6E3A1
            (0x89, 0xB4, 0xFA), // #89B4FA
            (0xFA, 0xB3, 0x87), // #FAB387
            (220, 221, 226),    // s12 dot6 sample (APPROX)
        ],
        icon_teal: (202, 228, 234), // s12 sample (APPROX)
    },
    ThemeDef {
        name: "everblush",
        dots: [
            (0xE0, 0x6C, 0x75), // #E06C75
            (0x98, 0xC3, 0x79), // #98C379
            (0xE5, 0xC0, 0x7B), // #E5C07B
            (0x61, 0xAF, 0xEF), // #61AFEF
            (0xC6, 0x78, 0xDD), // #C678DD
            (165, 175, 176),    // s12 dot6 sample (APPROX)
        ],
        icon_teal: (157, 189, 189), // s12 sample (APPROX)
    },
    ThemeDef {
        name: "everforest",
        dots: [
            (0xA7, 0xC0, 0x80), // #A7C080
            (0xE6, 0x98, 0x75), // #E69875
            (0xDB, 0xBC, 0x7F), // #DBBC7F
            (0x7F, 0xBB, 0xB3), // #7FBBB3
            (0xD6, 0x99, 0xB6), // #D699B6
            (199, 199, 194),    // s12 dot6 sample (APPROX)
        ],
        icon_teal: (170, 198, 185), // s12 sample (APPROX)
    },
];

// ---------------------------------------------------------------------------
// Thumbnail — the REAL live wallpaper, pre-decoded RGBA8 blob.
// Extracted on the Surface by icons-src/extract-themes-thumb.py
// (PIL, cover-fit LANCZOS, rounded-corner alpha baked in, r10).
// Source: ~/.config/mango/wallpaper/wallpaper.png (LIVE, iter14
// fallback sky), src md5 37b6c14019d058b2fbfd8b575da2cc9e.
// ---------------------------------------------------------------------------
/// wallpaper.png (1536x1024) — the LIVE wallpaper, cover-fit 225x96.
/// sha256 f1b4b137dc12fd804f07448efcc0faf342b71c71e1272e3193697264c9dcf3bb
pub const THUMB_THEMES: &[u8; (THUMB_W * THUMB_H * 4) as usize] =
    include_bytes!("themes-thumbs/thumb-themes.rgba");

// ---------------------------------------------------------------------------
// Data
// ---------------------------------------------------------------------------
/// The picker's state. Defaults = the video's settled state: "anime"
/// selected, empty search (placeholder). `total_themes` is the REAL
/// count of rendered themes (5) — the counter reads "1/5", never the
/// video's "1/16" (no 16-theme collection exists; see TRUTHFULNESS).
/// `search_text` is the compiled-in fallback panel's display field
/// only — the live library panel reads the real query from
/// THEME_SEARCH (iter128 B2).
#[derive(Clone, Debug)]
pub struct ThemesData {
    pub selected: usize, // index into THEMES (0 = "anime")
    pub total_themes: usize, // full list size for the counter
    pub search_text: String, // empty => "Search themes..." placeholder
}

impl Default for ThemesData {
    fn default() -> Self {
        Self {
            selected: 0,
            total_themes: THEMES.len(),
            search_text: String::new(),
        }
    }
}

impl ThemesData {
    /// The video frame s12, exactly, except the counter is the
    /// truthful "1/5" (see TRUTHFULNESS): "anime" selected, empty
    /// search (placeholder shown).
    pub fn video_defaults() -> Self {
        Self::default()
    }

    pub fn selected_name(&self) -> &'static str {
        THEMES
            .get(self.selected)
            .map(|t| t.name)
            .unwrap_or(THEMES[0].name)
    }

    /// Truthful counter for the real 5-theme list, e.g. "1/5".
    pub fn counter_text(&self) -> String {
        format!("{}/{}", self.selected.min(THEMES.len() - 1) + 1, self.total_themes)
    }
}

// ---------------------------------------------------------------------------
// Layout — every element rect, computed from the constants above so the
// spec is auditable in one place. The geometry measurer checks THESE
// numbers against s12.png.
// ---------------------------------------------------------------------------
/// Panel-relative rect of unselected row `i` (i = 1..5 in THEMES
/// order; row 0 is the taller selected row).
pub fn row_rect(i: usize) -> (f32, f32, f32, f32) {
    if i == 0 {
        (ROW_X, SEL_ROW_Y, ROW_W, SEL_ROW_H)
    } else {
        let y = ROW_FIRST_Y + ROW_PITCH * (i - 1) as f32;
        (ROW_X, y, ROW_W, ROW_PITCH)
    }
}

/// iter128 (clip guard): a row is drawable only when its whole rect
/// ends at or above the panel's bottom edge. All five shipped rows
/// fit (96 + 4x48 = 288 of content; last row 312..360 of 366, 6 px
/// spare); the guard makes that structural — a future row-count or
/// pitch change can never smear a half-row across the bottom edge,
/// it just drops the overflowing row whole. (The iter127 "5th-row
/// smear" itself was NOT a row clip — the offscreen render proves
/// the panel draws nothing below y 348; the live band is main.rs's
/// frost fill sampling a bright T3 backdrop feature. See
/// builder-themes-text-iter128.md. The guard is defence-in-depth
/// for the layout invariant, not the frost fix.)
pub fn row_fits(i: usize) -> bool {
    let (_, y, _, h) = row_rect(i);
    y + h <= PANEL_H as f32
}

/// Panel-relative rect of the selected row's thumbnail (225x96).
pub fn thumbnail_rect() -> (f32, f32, f32, f32) {
    (THUMB_X, THUMB_Y, THUMB_W as f32, THUMB_H as f32)
}

/// Centre x of the selected row's dots (6, under the name).
pub fn sel_dot_centres() -> [f32; 6] {
    SEL_DOT_XS
}

/// Centre x of an unselected row's dots (6, right edge).
pub fn row_dot_centres() -> [f32; 6] {
    DOT_XS
}

// ---------------------------------------------------------------------------
// Drawing primitives — same SDF/Argb8888 style as main.rs and
// wallpaper_panel.rs. Canvas is BGRA bytes, little-endian
// (Format::Argb8888), premultiplied. (Private helpers duplicated here
// because main.rs is not edited.)
// ---------------------------------------------------------------------------
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl Rect {
    const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
}

fn rounded_rect_alpha(x: f32, y: f32, w: f32, h: f32, r: f32) -> f32 {
    let cx = x - w / 2.0;
    let cy = y - h / 2.0;
    let qx = cx.abs() - (w / 2.0 - r);
    let qy = cy.abs() - (h / 2.0 - r);
    let ax = qx.max(0.0);
    let ay = qy.max(0.0);
    let d = (ax * ax + ay * ay).sqrt() + qx.max(qy).min(0.0) - r;
    (0.5 - d).clamp(0.0, 1.0)
}

/// Source-over blend of one opaque-colour pixel at coverage `cov` (0..1).
/// Destination is opaque inside the panel, so alpha is forced to 255 —
/// same shortcut as main.rs.
fn blend_pixel(canvas: &mut [u8], w: u32, x: i32, y: i32, rgb: (u8, u8, u8), cov: f32) {
    if x < 0 || y < 0 || x >= w as i32 {
        return;
    }
    let i = ((y as u32) * w + x as u32) as usize * 4;
    if i + 3 >= canvas.len() {
        return;
    }
    let inv = 1.0 - cov;
    canvas[i] = (rgb.2 as f32 * cov + canvas[i] as f32 * inv) as u8; // B
    canvas[i + 1] = (rgb.1 as f32 * cov + canvas[i + 1] as f32 * inv) as u8; // G
    canvas[i + 2] = (rgb.0 as f32 * cov + canvas[i + 2] as f32 * inv) as u8; // R
    canvas[i + 3] = 255; // A
}

/// Fill a rounded rect with a solid colour (opaque where the SDF says so).
fn fill_rounded_rect(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    rect: Rect,
    radius: f32,
    rgb: (u8, u8, u8),
) {
    let x0 = rect.x.floor().max(0.0) as i32;
    let y0 = rect.y.floor().max(0.0) as i32;
    let x1 = (rect.x + rect.w).ceil().min(w as f32) as i32;
    let y1 = (rect.y + rect.h).ceil().min(h as f32) as i32;
    for y in y0..y1 {
        for x in x0..x1 {
            let a = rounded_rect_alpha(
                x as f32 + 0.5 - rect.x,
                y as f32 + 0.5 - rect.y,
                rect.w,
                rect.h,
                radius,
            );
            if a <= 0.0 {
                continue;
            }
            blend_pixel(canvas, w, x, y, rgb, a);
        }
    }
}

/// AA ring (stroke circle): centre (cx, cy), mid radius r, thickness t.
fn stroke_circle(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    cx: f32,
    cy: f32,
    r: f32,
    t: f32,
    rgb: (u8, u8, u8),
) {
    let x0 = (cx - r - t).floor() as i32;
    let x1 = (cx + r + t).ceil() as i32;
    let y0 = (cy - r - t).floor() as i32;
    let y1 = (cy + r + t).ceil() as i32;
    for y in y0..y1 {
        for x in x0..x1 {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let d = (dx * dx + dy * dy).sqrt() - r;
            let a = (t / 2.0 + 0.5 - d.abs()).clamp(0.0, 1.0);
            if a <= 0.0 {
                continue;
            }
            blend_pixel(canvas, w, x, y, rgb, a);
        }
    }
}

/// AA line segment, `t` px thick (same as wallpaper_panel.rs).
fn stroke_segment(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    (ax, ay): (f32, f32),
    (bx, by): (f32, f32),
    t: f32,
    rgb: (u8, u8, u8),
) {
    let dx = bx - ax;
    let dy = by - ay;
    let len2 = dx * dx + dy * dy;
    let x0 = ax.min(bx).floor() as i32 - 2;
    let x1 = ax.max(bx).ceil() as i32 + 2;
    let y0 = ay.min(by).floor() as i32 - 2;
    let y1 = ay.max(by).ceil() as i32 + 2;
    for y in y0..y1 {
        for x in x0..x1 {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let tt = if len2 == 0.0 {
                0.0
            } else {
                ((px - ax) * dx + (py - ay) * dy) / len2
            }
            .clamp(0.0, 1.0);
            let cx = ax + tt * dx;
            let cy = ay + tt * dy;
            let d = ((px - cx) * (px - cx) + (py - cy) * (py - cy)).sqrt() - t / 2.0;
            let a = (0.5 - d).clamp(0.0, 1.0);
            if a <= 0.0 {
                continue;
            }
            blend_pixel(canvas, w, x, y, rgb, a);
        }
    }
}

/// Filled circle with AA edge.
fn fill_circle(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    cx: f32,
    cy: f32,
    r: f32,
    rgb: (u8, u8, u8),
) {
    let x0 = (cx - r - 1.0).floor() as i32;
    let x1 = (cx + r + 1.0).ceil() as i32;
    let y0 = (cy - r - 1.0).floor() as i32;
    let y1 = (cy + r + 1.0).ceil() as i32;
    for y in y0..y1 {
        for x in x0..x1 {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let d = (dx * dx + dy * dy).sqrt() - r;
            let a = (0.5 - d).clamp(0.0, 1.0);
            if a <= 0.0 {
                continue;
            }
            blend_pixel(canvas, w, x, y, rgb, a);
        }
    }
}

/// Blit one straight-alpha RGBA thumbnail onto the BGRA premultiplied
/// canvas, top-left at (`x0`, `y0`), 1:1 (no scaling — the blob is
/// extracted at exactly its blit size). Fully transparent source
/// pixels (the baked rounded corners) are skipped; out-of-bounds
/// pixels are clipped, never wrapped. Same math as
/// wallpaper_panel::blit_rgba.
fn blit_rgba(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    x0: i32,
    y0: i32,
    tw: u32,
    th: u32,
    rgba: &[u8],
) {
    debug_assert_eq!(rgba.len(), (tw * th * 4) as usize);
    for sy in 0..th as i32 {
        let y = y0 + sy;
        if y < 0 || y >= h as i32 {
            continue;
        }
        for sx in 0..tw as i32 {
            let x = x0 + sx;
            if x < 0 || x >= w as i32 {
                continue;
            }
            let si = (sy as usize * tw as usize + sx as usize) * 4;
            let sa = rgba[si + 3] as f32 / 255.0;
            if sa <= 0.0 {
                continue;
            }
            let i = ((y as u32) * w + x as u32) as usize * 4;
            if i + 3 >= canvas.len() {
                continue;
            }
            // Source is straight-alpha RGBA; dest is BGRA, opaque.
            let inv = 1.0 - sa;
            canvas[i] = (rgba[si + 2] as f32 * sa + canvas[i] as f32 * inv) as u8; // B
            canvas[i + 1] = (rgba[si + 1] as f32 * sa + canvas[i + 1] as f32 * inv) as u8; // G
            canvas[i + 2] = (rgba[si] as f32 * sa + canvas[i + 2] as f32 * inv) as u8; // R
            canvas[i + 3] = 255; // A
        }
    }
}

/// Darken the thumbnail's left edge so it fades into the row
/// highlight, as in s12: a black overlay whose alpha is FADE_MAX at
/// the thumb's left edge and eases to 0 at FADE_X1, applied only
/// inside the rounded-rect mask (and only where the blob is
/// opaque). `alpha` of the overlay at panel x is
/// `FADE_MAX * (1 - t)^1.5` with `t = (x - FADE_X0)/(FADE_X1-FADE_X0)`.
fn fade_thumb_left(canvas: &mut [u8], w: u32, h: u32) {
    let x0 = THUMB_X.floor() as i32;
    let x1 = (THUMB_X + THUMB_W as f32).ceil() as i32;
    let y0 = THUMB_Y.floor() as i32;
    let y1 = (THUMB_Y + THUMB_H as f32).ceil() as i32;
    for y in y0..y1 {
        for x in x0..x1 {
            let px = x as f32 + 0.5;
            if px >= FADE_X1 {
                continue;
            }
            let t = ((px - FADE_X0) / (FADE_X1 - FADE_X0)).clamp(0.0, 1.0);
            let fade = FADE_MAX * (1.0 - t).powf(1.5);
            if fade <= 0.0 {
                continue;
            }
            let py = y as f32 + 0.5;
            let mask = rounded_rect_alpha(
                px - THUMB_X,
                py - THUMB_Y,
                THUMB_W as f32,
                THUMB_H as f32,
                THUMB_R,
            );
            let a = fade * mask;
            if a <= 0.0 {
                continue;
            }
            // r1d: blend toward the card fill, not pure black — the
            // fade has to terminate exactly at C_HIGHLIGHT or its
            // edge reads as a hard seam against the row.
            blend_pixel(canvas, w, x, y, C_HIGHLIGHT, a);
        }
    }
}

// ---------------------------------------------------------------------------
// Text — fontdue, same pattern as wallpaper_panel.rs / main.rs
// (private helpers duplicated here because main.rs is not edited).
// ---------------------------------------------------------------------------
struct TextRow {
    glyphs: Vec<(fontdue::Metrics, std::sync::Arc<[u8]>)>,
    ascent: f32,
}

fn layout_text(font: &Font, text: &str, px: f32) -> TextRow {
    let mut glyphs = Vec::new();
    for ch in text.chars() {
        let (m, cov) = crate::lock_timer::rasterize_cached(font, ch, px);
        glyphs.push((m, cov));
    }
    let ascent = font
        .horizontal_line_metrics(px)
        .map(|lm| lm.ascent)
        .unwrap_or(px);
    TextRow { glyphs, ascent }
}

fn text_width(font: &Font, text: &str, px: f32) -> f32 {
    text.chars()
        .map(|ch| crate::lock_timer::rasterize_cached(font, ch, px).0.advance_width)
        .sum()
}

/// Draw `text` with its left edge at `x`, top of the line box at `y_top`.
/// `bold` double-blits with a 0.75 px x-offset (fake bold — the base
/// font is already Bold, so pass `false` at every call site).
fn draw_text(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &Font,
    text: &str,
    px: f32,
    x: f32,
    y_top: f32,
    rgb: (u8, u8, u8),
    bold: bool,
) {
    if text.is_empty() {
        return;
    }
    let row = layout_text(font, text, px);
    let baseline = y_top + row.ascent;
    let passes = if bold { 2 } else { 1 };
    for pass in 0..passes {
        let mut pen = x + pass as f32 * 0.75;
        for (m, cov) in &row.glyphs {
            let gx0 = (pen + m.xmin as f32).round() as i32;
            let gy0 = (baseline - m.ymin as f32 - m.height as f32).round() as i32;
            for gy in 0..m.height as i32 {
                for gx in 0..m.width as i32 {
                    let (px_x, px_y) = (gx0 + gx, gy0 + gy);
                    if px_x < 0 || px_y < 0 || px_x >= w as i32 || px_y >= h as i32 {
                        continue;
                    }
                    let c = cov[(gy as usize) * m.width + gx as usize] as f32 / 255.0;
                    if c <= 0.0 {
                        continue;
                    }
                    blend_pixel(canvas, w, px_x, px_y, rgb, c);
                }
            }
            pen += m.advance_width;
        }
    }
}

/// Search magnifier: ring r 6, 2 px stroke, handle to the lower right.
/// Procedural (same as wallpaper_panel.rs): the Material Symbols
/// `search` codepoint was NOT verified on the Surface — do not "fix"
/// this by guessing a codepoint.
fn draw_magnifier(canvas: &mut [u8], w: u32, h: u32, cx: f32, cy: f32, rgb: (u8, u8, u8)) {
    stroke_circle(canvas, w, h, cx - 1.0, cy - 1.0, 6.0, 2.0, rgb);
    stroke_segment(
        canvas, w, h,
        (cx + 3.2, cy + 3.2),
        (cx + 8.0, cy + 8.0),
        2.0,
        rgb,
    );
}

// ---------------------------------------------------------------------------
// Panel
// ---------------------------------------------------------------------------
/// Draw the full theme picker into `canvas` (600x366, Argb8888).
/// `font` is the UI font (Quicksand Bold, loaded by main.rs). The
/// caller (main.rs `draw()`) owns the black capsule fill; this
/// function draws content only. Defaults = the video's settled state
/// ("anime" selected, empty search).
pub fn draw_themes_panel(canvas: &mut [u8], w: u32, h: u32, font: &Font) {
    draw_themes_panel_with_data(canvas, w, h, font, &ThemesData::video_defaults());
}

/// Same as `draw_themes_panel`, with explicit state. `data.selected`
/// is clamped to the visible rows (0..5); out-of-range values fall
/// back to 0. The thumbnail always shows the current live wallpaper:
/// the video only shows it on the selected ("anime") row, and the
/// live wallpaper is the same photo whichever theme is selected.
pub fn draw_themes_panel_with_data(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &Font,
    data: &ThemesData,
) {
    debug_assert_eq!((w, h), (PANEL_W, PANEL_H));
    let selected = data.selected.min(THEMES.len() - 1);
    draw_search_line(canvas, w, h, font, &data.search_text, &data.counter_text());
    for (i, theme) in THEMES.iter().enumerate() {
        // iter128: never draw a row past the panel's bottom edge —
        // an overflowing row is skipped whole, never half-clipped.
        if !row_fits(i) {
            continue;
        }
        if i == selected {
            draw_selected_row(canvas, w, h, font, theme.name, &theme.dots, THUMB_THEMES, THUMB_H);
        } else {
            draw_unselected_row(
                canvas, w, h, font, i, theme.name, &theme.dots, theme.icon_teal,
            );
        }
    }
}

/// Number of themes the picker browses: the library count when
/// deployed, else the 5 compiled-in themes.
pub fn theme_total() -> usize {
    crate::library::manifest()
        .map(|m| m.themes.len())
        .unwrap_or(THEMES.len())
}

// ---------------------------------------------------------------------------
// iter128 B2: REAL type-to-filter search — same model as the
// Wallpaper panel (WALL_SEARCH) / Launcher / Clipboard. The panel
// owns its query + highlight in one Mutex; main.rs owns the key
// dispatch (on_themes_key) and calls theme_* here.
// ---------------------------------------------------------------------------

/// Live search state: the typed query + the highlight position inside
/// the FILTERED theme list (0 = top result).
struct ThemeSearch {
    query: String,
    selected: usize,
}

static THEME_SEARCH: Mutex<ThemeSearch> = Mutex::new(ThemeSearch {
    query: String::new(),
    selected: 0,
});

fn with_theme_search<R>(f: impl FnOnce(&mut ThemeSearch) -> R) -> R {
    let mut guard = THEME_SEARCH.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut guard)
}

/// Current query (untrimmed — trimming happens at match time).
pub fn theme_query() -> String {
    with_theme_search(|s| s.query.clone())
}

/// Theme indices matching the current query (case-insensitive
/// substring on the theme name), in library order. Empty query
/// returns every index (substring "" matches all). No manifest
/// (compiled-in fallback panel) matches against the five
/// compiled-in theme names only.
pub fn filtered_theme_indices() -> Vec<usize> {
    let q = theme_query().trim().to_lowercase();
    if let Some(m) = crate::library::manifest() {
        let n = m.themes.len();
        (0..n)
            .filter(|&i| {
                m.themes
                    .get(i)
                    .map(|t| t.name.to_lowercase().contains(&q))
                    .unwrap_or(false)
            })
            .collect()
    } else {
        (0..THEMES.len())
            .filter(|&i| THEMES[i].name.to_lowercase().contains(&q))
            .collect()
    }
}

/// Global theme index of the highlighted match, if any. Empty query
/// => the island's current selection (`selected`, the top row); a
/// query => filtered[selected], clamped to the last match. None only
/// when the filter matches nothing.
pub fn theme_highlighted(selected: usize) -> Option<usize> {
    if theme_query().trim().is_empty() {
        let n = theme_total().max(1);
        return Some(selected.min(n - 1));
    }
    let f = filtered_theme_indices();
    if f.is_empty() {
        return None;
    }
    let pos = with_theme_search(|s| s.selected).min(f.len() - 1);
    Some(f[pos])
}

/// Fresh search: empty query, highlight back to the top result.
/// Called on Themes entry and on Esc (main.rs); also after an
/// Enter-apply, so the rows recentre on the applied theme.
pub fn theme_reset() {
    with_theme_search(|s| {
        s.query.clear();
        s.selected = 0;
    });
}

/// Append typed text; highlight resets to the top result.
pub fn theme_push_text(text: &str) {
    with_theme_search(|s| {
        s.query.push_str(text);
        s.selected = 0;
    });
}

/// Delete the last query character (char-safe); highlight resets.
pub fn theme_backspace() {
    with_theme_search(|s| {
        s.query.pop();
        s.selected = 0;
    });
}

/// Move the highlight one match (dir -1 up / +1 down), clamped at
/// both ends (no wrap — same as Launcher/Clipboard). No-op when the
/// filter matches nothing.
pub fn theme_move_selection(dir: i32) {
    let n = filtered_theme_indices().len();
    if n == 0 {
        with_theme_search(|s| s.selected = 0);
        return;
    }
    with_theme_search(|s| {
        let (sel, _) = crate::selection::move_by(s.selected, 0, dir, n, 5);
        s.selected = sel;
    });
}

/// Truncate `text` to `max_w` at `px` (char cut + "…" — a query has
/// no word boundaries worth keeping). The full text stays in state;
/// only the display truncates.
fn truncate_to_width(font: &Font, text: &str, px: f32, max_w: f32) -> String {
    if text_width(font, text, px) <= max_w {
        return text.to_string();
    }
    let ellipsis_w = text_width(font, "…", px);
    let mut out = String::new();
    let mut w = 0.0f32;
    for ch in text.chars() {
        let cw = crate::lock_timer::rasterize_cached(font, ch, px).0.advance_width;
        if w + cw + ellipsis_w > max_w {
            break;
        }
        out.push(ch);
        w += cw;
    }
    out.push('…');
    out
}

/// Library theme index shown at row `row` (0..5) when `selected` is
/// the selected row: [sel, sel+1, ..., sel+4] wrapping, so every
/// theme is reachable by walking the rows. iter128 B2: with a query
/// the rows ARE the filtered list — row 0 is the highlighted match,
/// rows 1..4 the next matches wrapping inside it. None when the
/// filter matches nothing (the empty state draws no rows, so a click
/// there must apply nothing).
pub fn row_theme_index(row: usize, selected: usize) -> Option<usize> {
    if theme_query().trim().is_empty() {
        return Some((selected + row) % theme_total().max(1));
    }
    let f = filtered_theme_indices();
    if f.is_empty() {
        return None;
    }
    let highlight = with_theme_search(|s| s.selected).min(f.len() - 1);
    Some(f[(highlight + row) % f.len()])
}

fn draw_search_line(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &Font,
    search_text: &str,
    counter: &str,
) {
    draw_magnifier(canvas, w, h, MAGNIFIER_CX, MAGNIFIER_CY, C_SECONDARY);
    if search_text.is_empty() {
        draw_text(
            canvas, w, h, font, SEARCH_PLACEHOLDER, SEARCH_PX, SEARCH_TEXT_X, SEARCH_TEXT_TOP,
            C_SECONDARY, false,
        );
    } else {
        draw_text(
            canvas, w, h, font, search_text, SEARCH_PX, SEARCH_TEXT_X, SEARCH_TEXT_TOP,
            C_QUERY, false,
        );
    }
    if search_text.is_empty() {
        // r1d: 2 px caret before the empty placeholder.
        fill_rounded_rect(
            canvas,
            w,
            h,
            Rect::new(
                SEARCH_TEXT_X - CARET_W - 4.0,
                SEARCH_TEXT_TOP,
                CARET_W,
                CARET_H,
            ),
            0.5,
            C_PRIMARY,
        );
    }
    let counter_w = text_width(font, counter, COUNTER_PX);
    draw_text(
        canvas, w, h, font, counter, COUNTER_PX, COUNTER_RIGHT - counter_w, COUNTER_TOP,
        C_SECONDARY, false,
    );
    // r1d: 1 px divider on the search row's bottom edge.
    fill_rounded_rect(
        canvas,
        w,
        h,
        Rect::new(ROW_X, SEARCH_Y + SEARCH_H, ROW_W, 1.0),
        0.0,
        C_DIVIDER,
    );
}

fn draw_selected_row(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &Font,
    name: &str,
    dots: &[(u8, u8, u8); 6],
    thumb: &[u8],
    thumb_h: u32,
) {
    // Selected row: #05163C highlight + bullet + name + 6 dots under
    // the name + thumbnail (right).
    let (rx, ry, rw, rh) = row_rect(0);
    fill_rounded_rect(canvas, w, h, Rect::new(rx, ry, rw, rh), ROW_R, C_HIGHLIGHT);
    fill_circle(
        canvas, w, h, BULLET_CX, BULLET_CY, BULLET_D / 2.0, C_PRIMARY,
    );
    draw_text(
        canvas, w, h, font, name, NAME_PX, SEL_NAME_X, SEL_NAME_TOP, C_PRIMARY,
        false,
    );
    for (j, dot) in dots.iter().enumerate() {
        fill_circle(
            canvas, w, h, SEL_DOT_XS[j], SEL_DOT_CY, DOT_D / 2.0, *dot,
        );
    }
    if thumb_h == THUMB_H {
        blit_rgba(
            canvas,
            w,
            h,
            THUMB_X.round() as i32,
            THUMB_Y.round() as i32,
            THUMB_W,
            THUMB_H,
            thumb,
        );
    } else {
        // Library thumbs are 225x138: centre-crop the middle 96 rows
        // into the 225x96 slot (same image, same width).
        blit_rgba_vcrop(
            canvas,
            w,
            h,
            THUMB_X.round() as i32,
            THUMB_Y.round() as i32,
            THUMB_W,
            THUMB_H,
            thumb_h,
            thumb,
        );
    }
    fade_thumb_left(canvas, w, h);
}

/// Blit the middle `th` rows of a taller RGBA blob (same width `tw`,
/// full height `src_h`): the library's 225x138 thumbs centre-cropped
/// to the themes panel's 225x96 slot.
fn blit_rgba_vcrop(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    x0: i32,
    y0: i32,
    tw: u32,
    th: u32,
    src_h: u32,
    rgba: &[u8],
) {
    debug_assert_eq!(rgba.len(), (tw * src_h * 4) as usize);
    let y_off = ((src_h - th) / 2) as i32;
    for sy in 0..th as i32 {
        let y = y0 + sy;
        if y < 0 || y >= h as i32 {
            continue;
        }
        for sx in 0..tw as i32 {
            let x = x0 + sx;
            if x < 0 || x >= w as i32 {
                continue;
            }
            let si = (((sy + y_off) as usize * tw as usize) + sx as usize) * 4;
            let sa = rgba[si + 3] as f32 / 255.0;
            if sa <= 0.0 {
                continue;
            }
            let i = ((y as u32) * w + x as u32) as usize * 4;
            if i + 3 >= canvas.len() {
                continue;
            }
            let inv = 1.0 - sa;
            canvas[i] = (rgba[si + 2] as f32 * sa + canvas[i] as f32 * inv) as u8;
            canvas[i + 1] = (rgba[si + 1] as f32 * sa + canvas[i + 1] as f32 * inv) as u8;
            canvas[i + 2] = (rgba[si] as f32 * sa + canvas[i + 2] as f32 * inv) as u8;
            canvas[i + 3] = 255;
        }
    }
}

fn draw_unselected_row(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &Font,
    row: usize,
    name: &str,
    dots: &[(u8, u8, u8); 6],
    icon_teal: (u8, u8, u8),
) {
    // Unselected row: icon disc + teal dot at the left, name,
    // 6 dots at the right edge. No fill (video wins over QS).
    let (_, ry, _, rh) = row_rect(row);
    let cy = ry + rh / 2.0;
    // r1d: rounded square, not a disc (reference 35x29 device, r 6).
    fill_rounded_rect(
        canvas,
        w,
        h,
        Rect::new(ICON_CX - CHIP_W / 2.0, cy - CHIP_H / 2.0, CHIP_W, CHIP_H),
        CHIP_R,
        C_ICON_DISC,
    );
    fill_circle(
        canvas, w, h, ICON_CX, cy, ICON_DOT_D / 2.0, icon_teal,
    );
    draw_text(
        canvas, w, h, font, name, NAME_PX, NAME_X, ry + NAME_TOP_OFF, C_PRIMARY,
        false,
    );
    let dot_cy = ry + DOT_CY_OFF;
    for (j, dot) in dots.iter().enumerate() {
        fill_circle(canvas, w, h, DOT_XS[j], dot_cy, DOT_D / 2.0, *dot);
    }
}

/// iter36 a6b: LIVE themes panel — rows from the library manifest/// iter36 a6b: LIVE themes panel — rows from the library manifest
/// (24 themes, real palettes). Row 0 is the selected theme; rows 1..5
/// are the next themes in library order (wrapping). The selected row's
/// thumbnail is the theme's first wallpaper; the unselected-row icon
/// teal is the theme's palette slot 4. Falls back to the compiled-in
/// 5-theme panel when the library is not deployed (yet).
/// iter128 B2: with a query the rows ARE the filtered list — row 0 is
/// the highlighted match, rows 1..4 the next matches wrapping inside
/// it; counter reads "<match>/<matches>". Zero matches draws the
/// honest empty state (no stale rows). No query => the old behaviour.
pub fn draw_themes_panel_live(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &Font,
    selected: usize,
) {
    let Some(m) = crate::library::manifest() else {
        draw_themes_panel(canvas, w, h, font);
        return;
    };
    let n = m.themes.len();
    if n == 0 {
        draw_themes_panel(canvas, w, h, font);
        return;
    }
    // Search text + counter + the five row indices. Filtered: the
    // highlighted match is row 0; unfiltered: `selected`.
    let query = theme_query();
    let (search_text, counter, rows): (String, String, Vec<usize>) = if query.trim().is_empty()
    {
        let sel = selected.min(n - 1);
        (
            String::new(),
            format!("{}/{}", sel + 1, n),
            (0..5).map(|row| (sel + row) % n).collect(),
        )
    } else {
        let f = filtered_theme_indices();
        if f.is_empty() {
            draw_search_line(canvas, w, h, font, query.trim(), &format!("0/{}", n));
            let msg = format!(
                "No themes match \"{}\"",
                truncate_to_width(font, query.trim(), SEARCH_PX, 420.0)
            );
            let msg_w = text_width(font, &msg, 12.0);
            draw_text(
                canvas, w, h, font, &msg, 12.0,
                (PANEL_W as f32 - msg_w) / 2.0, 200.0, C_SECONDARY, false,
            );
            return;
        }
        let highlight = with_theme_search(|s| s.selected).min(f.len() - 1);
        (
            truncate_to_width(font, query.trim(), SEARCH_PX, 480.0),
            format!("{}/{}", highlight + 1, f.len()),
            (0..5).map(|row| f[(highlight + row) % f.len()]).collect(),
        )
    };
    draw_search_line(canvas, w, h, font, &search_text, &counter);
    for row in 0..5 {
        // iter128 B3 (integrator hand-merge — B3 hunk #6 rejected:
        // B2 rewrote this loop): never draw a row past the panel's
        // bottom edge — an overflowing row is skipped whole, never
        // half-clipped. Same guard as draw_themes_panel_with_data.
        if !row_fits(row) {
            continue;
        }
        let idx = rows[row];
        let theme = &m.themes[idx];
        if row == 0 {
            // Selected row's thumbnail: the theme's first wallpaper
            // (225x96 RGBA); the compiled-in live-wallpaper thumb when
            // it has not arrived yet.
            let thumb_vec = theme
                .wallpapers
                .first()
                .and_then(|f| f.rsplit_once('.').map(|(s, _)| s.to_string()))
                .and_then(|stem| crate::library::thumb_rgba(&stem, 225));
            let (thumb, thumb_h): (&[u8], u32) = match &thumb_vec {
                Some(v) => (v, 138),
                None => (THUMB_THEMES, THUMB_H),
            };
            draw_selected_row(canvas, w, h, font, &theme.name, &theme.palette, thumb, thumb_h);
        } else {
            draw_unselected_row(
                canvas, w, h, font, row, &theme.name, &theme.palette, theme.palette[3],
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Tests — render the real panel offscreen (no Wayland) and assert the
// content is there: search line + truthful counter, selected-row
// highlight + bullet + name + 6 dots + real thumbnail with fade,
// unselected rows with icon discs, names and right-edge dots.
// Run on the Surface scratch copy: `cargo test --offline`.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    fn load_test_font() -> &'static Font {
        // Vendored asset is embedded: no $HOME dependency, no panic.
        crate::ui_font()
    }

    /// Render the default panel into a fresh buffer. main.rs fills the
    /// black capsule first, so the test pre-fills opaque black too.
    fn render_default() -> (Vec<u8>, &'static Font) {
        let font = load_test_font();
        let mut canvas = vec![0u8; (PANEL_W * PANEL_H * 4) as usize];
        for px in canvas.chunks_exact_mut(4) {
            px[0] = 0;
            px[1] = 0;
            px[2] = 0;
            px[3] = 255;
        }
        draw_themes_panel(&mut canvas, PANEL_W, PANEL_H, &font);
        (canvas, font)
    }

    fn px(canvas: &[u8], x: i32, y: i32) -> (u8, u8, u8) {
        let i = ((y as u32) * PANEL_W + x as u32) as usize * 4;
        (canvas[i + 2], canvas[i + 1], canvas[i]) // (R, G, B)
    }

    fn count_bright(canvas: &[u8], x0: i32, y0: i32, x1: i32, y1: i32, thr: u32) -> usize {
        let mut n = 0;
        for y in y0..y1 {
            for x in x0..x1 {
                let (r, g, b) = px(canvas, x, y);
                if r as u32 + g as u32 + b as u32 > thr {
                    n += 1;
                }
            }
        }
        n
    }

    fn mean_rgb(canvas: &[u8], x0: i32, y0: i32, x1: i32, y1: i32) -> (f32, f32, f32) {
        let (mut sr, mut sg, mut sb, mut n) = (0f32, 0f32, 0f32, 0f32);
        for y in y0..y1 {
            for x in x0..x1 {
                let (r, g, b) = px(canvas, x, y);
                sr += r as f32;
                sg += g as f32;
                sb += b as f32;
                n += 1.0;
            }
        }
        (sr / n, sg / n, sb / n)
    }

    /// iter128: brightest channel value in a rect (0..255).
    fn peak_lum(canvas: &[u8], x0: i32, y0: i32, x1: i32, y1: i32) -> u32 {
        let mut peak = 0u32;
        for y in y0..y1 {
            for x in x0..x1 {
                let (r, g, b) = px(canvas, x, y);
                peak = peak.max(r.max(g).max(b) as u32);
            }
        }
        peak
    }

    #[test]
    fn layout_matches_iter16_spec() {
        // Selected row 14..586 x 66..162 (h 96); rows pitch 48 from 168.
        assert_eq!(row_rect(0), (14.0, 66.0, 572.0, 96.0));
        assert_eq!(row_rect(1), (14.0, 168.0, 572.0, 48.0));
        assert_eq!(row_rect(2), (14.0, 216.0, 572.0, 48.0));
        assert_eq!(row_rect(3), (14.0, 264.0, 572.0, 48.0));
        assert_eq!(row_rect(4), (14.0, 312.0, 572.0, 48.0));
        // Thumbnail rect 357..582 x 66..162 (225x96).
        assert_eq!(thumbnail_rect(), (357.0, 66.0, 225.0, 96.0));
        // Six dots per row (s12-measured count, not QS's five).
        assert_eq!(sel_dot_centres().len(), 6);
        assert_eq!(row_dot_centres().len(), 6);
        assert_eq!(SEL_DOT_XS, [34.0, 52.0, 72.0, 90.0, 108.0, 129.0]);
        assert_eq!(DOT_XS, [508.0, 523.0, 535.0, 552.0, 565.0, 580.0]);
        // Truthfulness: counter derives from the real 5-theme list.
        let data = ThemesData::video_defaults();
        assert_eq!(data.selected, 0);
        assert_eq!(data.selected_name(), "anime");
        assert_eq!(data.counter_text(), "1/5");
        assert_eq!(THEMES.len(), 5);
        assert_eq!(
            THEMES.iter().map(|t| t.name).collect::<Vec<_>>(),
            vec!["anime", "ariadne", "catppuccin", "everblush", "everforest"]
        );
        for t in THEMES.iter() {
            assert_eq!(t.dots.len(), 6);
        }
        // Anime palette order corrected (green before yellow).
        assert_eq!(THEMES[0].dots[1], (0x9E, 0xCE, 0x6A));
        assert_eq!(THEMES[0].dots[2], (0xE0, 0xAF, 0x68));
        // Thumbnail blob is the real 225x96 RGBA (86,400 B).
        assert_eq!(THUMB_THEMES.len(), (225 * 96 * 4) as usize);
        // iter128: every shipped row fits the panel whole (last row
        // 312..360 of 366, 6 px spare); a hypothetical 6th row
        // (360..408) must not fit.
        for i in 0..5 {
            assert!(row_fits(i), "row {i} does not fit the panel");
        }
        assert!(!row_fits(5), "row 5 (360..408) must not fit a 366 panel");
    }

    #[test]
    fn renders_real_content() {
        let (canvas, _font) = render_default();
        // Search line: magnifier + "Search themes..." present, no fill.
        let search = count_bright(&canvas, 15, 14, 200, 48, 150);
        println!("search-band bright px: {search}");
        assert!(search > 100, "search line missing: {search} bright px");
        // Counter "1/5" top-right.
        let counter = count_bright(&canvas, 540, 14, 590, 40, 150);
        println!("counter bright px: {counter}");
        assert!(counter > 10, "counter missing: {counter} bright px");
        // iter128: meta text is white now (was #9AA0A6, peak ~166 in
        // the buffer) — counter + placeholder must peak bright like
        // the row labels (255), not mid-grey.
        let counter_peak = peak_lum(&canvas, 540, 14, 590, 40);
        println!("counter peak lum: {counter_peak}");
        assert!(counter_peak > 200, "counter still dim: peak {counter_peak}");
        let search_peak = peak_lum(&canvas, 45, 14, 300, 48);
        println!("search placeholder peak lum: {search_peak}");
        assert!(search_peak > 200, "search placeholder still dim: peak {search_peak}");
        // Selected-row highlight: mean in the row (left of thumb) is
        // the NEUTRAL dark fill #191A1F (25,26,31) — r1d: the reference
        // selection is neutral, not the saturated navy #05163C this test
        // used to pin. Measured mean lands ~(29,29,34): the fill plus the
        // bullet and the selected name, so R and G track the fill and B
        // sits a few units higher from the AA text.
        let (mr, mg, mb) = mean_rgb(&canvas, 20, 70, 350, 158);
        println!("highlight mean RGB ({mr:.0},{mg:.0},{mb:.0})");
        assert!(mr > 20.0 && mr < 45.0, "highlight fill wrong: {mr:.0}");
        assert!(mg > 20.0 && mg < 45.0, "highlight fill wrong: {mg:.0}");
        assert!(mb > 24.0 && mb < 50.0, "highlight fill wrong: {mb:.0}");
        // Neutral: the channels sit within a few units of each other. A
        // navy fill would put B 30+ above R (the old (5,22,60) gave 55).
        assert!(mb - mr < 12.0, "highlight not neutral: {mr:.0},{mb:.0}");
        assert!(mb - mg < 12.0, "highlight not neutral: {mg:.0},{mb:.0}");
        // Bullet: bright pixels near (34,100).
        let bullet = count_bright(&canvas, 28, 94, 42, 107, 300);
        println!("bullet bright px: {bullet}");
        assert!(bullet > 10, "selected-row bullet missing: {bullet} px");
        // Selected name "anime": bright pixels in the name band.
        let name = count_bright(&canvas, 46, 94, 120, 110, 300);
        println!("anime name bright px: {name}");
        assert!(name > 30, "anime name missing: {name} bright px");
        // Selected dots: 6 saturated dots under the name.
        let mut dot_px = 0;
        for y in 126..139 {
            for x in 28..136 {
                let (r, g, b) = px(&canvas, x, y);
                let (mx, mn) = (
                    r.max(g).max(b) as i32,
                    r.min(g).min(b) as i32,
                );
                if mx - mn > 40 {
                    dot_px += 1;
                }
            }
        }
        println!("selected dots saturated px: {dot_px}");
        assert!(dot_px > 60, "selected-row dots missing: {dot_px} px");
        // Thumbnail: bright sky photo on the right of the selected row;
        // the fade makes its left edge dark and its right side bright.
        let (lr, lg, lb) = mean_rgb(&canvas, 360, 80, 420, 150);
        let (rr, rg, rb) = mean_rgb(&canvas, 500, 80, 575, 150);
        let lum_l = 0.299 * lr + 0.587 * lg + 0.114 * lb;
        let lum_r = 0.299 * rr + 0.587 * rg + 0.114 * rb;
        println!("thumb left mean lum {lum_l:.0}, right mean lum {lum_r:.0}");
        assert!(lum_r > 80.0, "thumbnail too dark on the right: {lum_r:.0}");
        assert!(lum_r > lum_l + 20.0, "thumbnail fade missing: {lum_l:.0} vs {lum_r:.0}");
        // Unselected rows: icon teal dots present (greenish pixels
        // near each row's icon centre) + names + right-edge dots.
        for (i, row_y) in [168.0f32, 216.0, 264.0, 312.0].into_iter().enumerate() {
            let cy = row_y + 24.0;
            let mut teal = 0;
            for y in (cy as i32 - 6)..(cy as i32 + 7) {
                for x in 30..43 {
                    let (r, g, b) = px(&canvas, x, y);
                    if g > 120 && g as i32 - r as i32 > 20 {
                        teal += 1;
                    }
                }
            }
            println!("row {} icon teal px: {teal}", i + 1);
            assert!(teal > 10, "row {} icon teal dot missing: {teal} px", i + 1);
            let nm = count_bright(&canvas, 63, row_y as i32 + 13, 160, row_y as i32 + 28, 300);
            println!("row {} name bright px: {nm}", i + 1);
            assert!(nm > 15, "row {} name missing: {nm} bright px", i + 1);
            let mut rdots = 0;
            for y in (row_y as i32 + 15)..(row_y as i32 + 28) {
                for x in 502..586 {
                    let (r, g, b) = px(&canvas, x, y);
                    let (mx, mn) = (r.max(g).max(b) as i32, r.min(g).min(b) as i32);
                    if mx - mn > 40 {
                        rdots += 1;
                    }
                }
            }
            println!("row {} right dots saturated px: {rdots}", i + 1);
            assert!(rdots > 40, "row {} right-edge dots missing: {rdots} px", i + 1);
        }
        // iter128: nothing is drawn in the content-free strip below
        // the last row (y 350..366) — the iter127 "smear" band must
        // not exist in the panel's own render (it was the frost fill,
        // not this module; this assert pins the panel side clean).
        let bottom = count_bright(&canvas, 0, 350, PANEL_W as i32, PANEL_H as i32, 60);
        println!("bottom-strip bright px (y 350..366): {bottom}");
        assert_eq!(bottom, 0, "bottom strip not clean: {bottom} bright px");
        // Optional render dump for visual inspection (builder only — set
        // DYNAMITE_DUMP_RENDER=/tmp/themes-panel-render.bgra when
        // running the test; the buffer is BGRA bytes, 600x366x4).
        if let Ok(path) = std::env::var("DYNAMITE_DUMP_RENDER") {
            std::fs::write(&path, &canvas).expect("dump render");
            println!("render dumped to {path}");
        }
    }

    // iter36 a6b: row -> library theme index mapping + live draw smoke.
    #[test]
    fn row_theme_index_wraps() {
        // Fallback count is 5 when the library is absent; with the
        // library deployed it is 24. Either way the mapping wraps.
        // theme_reset() first: the query is a shared static and a
        // sibling test that types into the search box can leave it
        // non-empty, which switched this to the FILTERED branch.
        theme_reset();
        let n = theme_total();
        assert_eq!(row_theme_index(0, 0), Some(0));
        assert_eq!(row_theme_index(1, 0), Some(1 % n.max(1)));
        assert_eq!(row_theme_index(4, n - 1), Some((n - 1 + 4) % n));
    }

    // iter128 B2: type-to-filter matching + render behaviour.
    #[test]
    fn search_filters_theme_rows_and_empties_honestly() {
        // One test fn (not three): the query state is a shared
        // static, so the whole sequence runs serially here.
        // Manifest-dependent counts are gated on the library being
        // deployed (it is, on the Surface scratch copy).
        theme_reset();
        assert_eq!(filtered_theme_indices().len(), theme_total());
        let Some(m) = crate::library::manifest() else {
            theme_reset();
            return;
        };
        assert_eq!(theme_total(), 24);
        // "anime" matches exactly one theme name.
        theme_push_text("anime");
        assert_eq!(filtered_theme_indices().len(), 1);
        let anime = m.index_of_theme("anime").unwrap();
        assert_eq!(theme_highlighted(0), Some(anime));
        assert_eq!(row_theme_index(0, 0), Some(anime));
        // Renders: query in the search band, the match as row 0.
        let font = load_test_font();
        let mut canvas = vec![0u8; (PANEL_W * PANEL_H * 4) as usize];
        for px in canvas.chunks_exact_mut(4) {
            px[3] = 255;
        }
        draw_themes_panel_live(&mut canvas, PANEL_W, PANEL_H, &font, 0);
        let search = count_bright(&canvas, 15, 14, 200, 48, 150);
        assert!(search > 40, "filtered query not drawn: {search} bright px");
        // Highlight moves clamp at the single match.
        theme_move_selection(1);
        assert_eq!(theme_highlighted(0), Some(anime));
        // Extend the query to a zero-match filter: honest empty
        // state, no rows apply.
        theme_push_text("zzz-no-such-theme");
        assert_eq!(filtered_theme_indices().len(), 0);
        assert_eq!(theme_highlighted(0), None);
        assert_eq!(row_theme_index(0, 0), None);
        let mut canvas2 = vec![0u8; (PANEL_W * PANEL_H * 4) as usize];
        for px in canvas2.chunks_exact_mut(4) {
            px[3] = 255;
        }
        draw_themes_panel_live(&mut canvas2, PANEL_W, PANEL_H, &font, 0);
        let msg = count_bright(&canvas2, 60, 190, 540, 220, 150);
        assert!(msg > 30, "empty-state message missing: {msg} bright px");
        // Reset restores the full list.
        theme_reset();
        assert_eq!(filtered_theme_indices().len(), 24);
        assert!(theme_query().is_empty());
        assert_eq!(theme_highlighted(0), Some(0));
    }

    #[test]
    fn live_draw_renders_without_panic() {
        // Works with or without the deployed library (fallback path
        // renders the compiled-in 5-theme panel when absent).
        let font = load_test_font();
        let mut canvas = vec![0u8; (PANEL_W * PANEL_H * 4) as usize];
        for px in canvas.chunks_exact_mut(4) {
            px[3] = 255;
        }
        draw_themes_panel_live(&mut canvas, PANEL_W, PANEL_H, &font, 0);
        assert!(count_bright(&canvas, 0, 0, PANEL_W as i32, PANEL_H as i32, 60) > 100);
    }

}
