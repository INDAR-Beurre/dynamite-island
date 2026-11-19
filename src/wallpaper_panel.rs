//! wallpaper_panel — Wallpaper picker state (1008x270) for dynamite-rust.
//!
//! Layout spec + `draw_wallpaper_panel()` in the same manual-SDF /
//! Argb8888 style as `main.rs` (no tiny-skia, no cosmic-text, no new
//! dependencies — only `fontdue`, already in Cargo.toml).
//!
//! SOURCES (do not re-derive from memory):
//! - `analysis/frame-measurements.md`: wallpaper picker 1008x270, top 15.
//! - `frames/per-second/s10.png` — video frame: flat "Search
//!   wallpapers..." line on black (magnifier + grey placeholder, NO
//!   visible field), an "anime" chip top-right, five thumbnails with
//!   the centre one enlarged ~1.46x and bright-outlined, filename
//!   bottom-left, counter bottom-right.
//! - `iterations/builder-pickers-iter13.md` — measured spec (PIL,
//!   640x360, ±3 logical px): thumb rects, search band, chip, bottom
//!   row. All layout constants below come from that spec.
//! - `iterations/critic-pickers-iter13.md` — harsh critic: geometry
//!   PASS for five iterations, content 0% (placeholder word only).
//!   This module is the first real content for the state.
//!
//! WIRING (integrator — `main.rs` is patched separately, this file
//! only defines the panel):
//! ```ignore
//! // main.rs, with the other state-content mods:
//! mod wallpaper_panel;
//! // main.rs, in draw(), before the `other =>` placeholder arm:
//! //   IslandState::Wallpaper => {
//! //       wallpaper_panel::draw_wallpaper_panel(canvas, w, h, font);
//! //   }
//! // No socket-word changes: `wallpaper` already dispatches via
//! // IslandState::from_word. No icon-font change: the panel needs no
//! // icon_font_for entry (procedural magnifier only) — do NOT add one
//! // (avoids the ~19 MB lazy load). The Themes state stays on the
//! // placeholder arm until its own builder lands it.
//! ```
//!
//! SEARCH — FLAT line, REAL type-to-filter (iter128 B2):
//! The video has NO search field: just a magnifier glyph and grey
//! "Search wallpapers..." sitting flat on the black panel. No
//! #1A1C1F fill is drawn. Typing while the view is open filters the
//! 56 library wallpapers live (filename / theme substring,
//! case-insensitive); the query renders as plain white text, with NO
//! caret and NO box — the same flat look as the launcher. The live
//! state is WALL_SEARCH below (`WallpaperData::search_text` is only
//! the compiled-in fallback panel's display field).
//!
//! THUMBNAILS — REAL FILES, pre-decoded RGBA blobs:
//! Each thumb is one of the REAL wallpaper files in
//! `~/.config/mango/wallpaper/`, downscaled on the Surface by
//! `icons-src/extract-wallpaper-thumbs.py` (PIL, cover-fit LANCZOS,
//! rounded-corner alpha baked in) and embedded here via
//! `include_bytes!`. No PNG decoder exists in this binary and none is
//! needed: the decode happened once, at extraction time, and the
//! island only blits bytes. The blobs are `&'static [u8]` in .rodata
//! (~360 KB total for all five): demand-paged, ~0 PSS until the
//! wallpaper state is first drawn, never growing.
//!
//! FILMSTRIP (display order, selected = index 2 = centre):
//! ```text
//!   i   blob (this file)          source file (Surface)          size
//!   0   thumb-day.rgba            wallpaper-day.jpg              154x96
//!   1   thumb-dusk.rgba           wallpaper-dusk.png             154x96
//!   2   thumb-wallpaper.rgba      wallpaper.png (LIVE)           225x138
//!   3   thumb-czd.rgba            czd.png                        154x96
//!   4   thumb-catppuccin.rgba     wallpaper.catppuccin.png       154x96
//! ```
//! TRUTHFULNESS (hard rules, do not relax):
//! - The bottom-left label shows the REAL live filename
//!   `wallpaper.png` — NEVER `wallhaven-4d9....png` (the exact
//!   Wallhaven file is proven deleted from Wallhaven; see
//!   analysis/wallpaper.md). The label is derived from
//!   FILMSTRIP_NAMES[selected], not hard-coded.
//! - The counter shows the REAL filmstrip position `3/5` — NOT the
//!   video's `14/16` (no 16-item collection exists).
//! - `wallpaper-twilight.png` is BYTE-IDENTICAL to `wallpaper-dusk.png`
//!   (md5 8ad8192945a0b9c457838e15814f0a02, both files — verified
//!   13 Nov 2026), so it is NOT in the filmstrip: two identical
//!   thumbs would be a visible defect. `czd.png` takes its slot.
//!   Excluded: mango.png (500x500 logo, not a wallpaper),
//!   wallpaper.prev.png (older-wallpaper backup),
//!   wallpaper.png.bak-iter14-20261113 (dark-Miku backup).
//! - The "anime" chip is the video's filter chip, display-only.
//!
//! APPROXIMATIONS (honest, flagged — not video-sampled):
//! - Selection-outline colour #7CA2F5 is the island's established
//!   accent (power panel); the s10 outline is NOT pixel-separable
//   from the selected thumb's own sky at 640x360 (iter13).
//! - Chip fill (59,59,56) is the single grey sample recorded in
//!   iter13; chip text is white (iter128, was #9AA0A6 — the panel's
//!   secondary grey, too dim at 11 px).
//! - Thumb corner radii (8 / 10) are baked into the blobs from the
//!   s10 look, not measured to the pixel.
//! - Unselected thumbs are NOT dimmed: no dimming factor is
//!   separable from content brightness at 640x360.
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
//! GEOMETRY — all panel-relative logical px, canvas 1008x270, radius 20.
//! (Panel top edge sits at screen y = 15 via the layer margin, as usual.)
//! ---------------------------------------------------------------------------
//!   Element                     x            y            w      h
//!   --------------------------  -----------  -----------  -----  ----
//!   Panel                       0            0            1008   270
//!   Search line (flat)          14           14..48       980    34
//!     magnifier (ring r6)       centre (23,31)             ~17    ~17
//!     "Search wallpapers..."    38           19..33       —      12px
//!     chip "anime"              944..1000     21..41       56     20 r10
//!   Thumb i (i = 0,1,3,4)       59/225/628/794 99..195     154    96
//!   Thumb 2 (selected)          391..616     78..216       225    138
//!     selection outline         389..618     76..218       229    142 r12
//!     (outline = thumb rect expanded 2 px, 2 px stroke)
//!   Filename "wallpaper.png"    21           241..255      —      12px (iter128, was 10px)
//!   Counter "3/5"               ends 984      241..255      —      12px (iter128, was 10px)
//!   Column arithmetic: thumbs x = 59, 225, 391, 628, 794; gaps 12;
//!   row span 59..948, centred (1008-889)/2 = 59.5. Selected thumb
//!   centre y = 147 = unselected centre y (99+48). Bottom row top
//!   241, panel bottom pad 270-252 = 18.
//! ---------------------------------------------------------------------------
//! COLOURS:
//!   PANEL           #000000  (0,0,0)      — filled by main.rs, not here
//!   SECONDARY       #FFFFFF  (255,255,255)— placeholder + labels + chip
//!                                            text (iter128: was #9AA0A6,
//!                                            measured peak lum ~143 on
//!                                            screen — too dim at 10 px;
//!                                            white peaks ~255 in-buffer.
//!                                            Hierarchy kept by SIZE:
//!                                            meta 12 px stays below
//!                                            the thumbs' visual weight.)
//!   CHIP_BG         (59,59,56)             — single-sample grey, iter13
//!   ACCENT          #7CA2F5  (124,162,245)— selection outline (island
//!                                            accent; see APPROXIMATIONS)
//! ---------------------------------------------------------------------------

use fontdue::Font;
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
pub const PANEL_W: u32 = 1008;
pub const PANEL_H: u32 = 270;
// (Panel corner radius 20 is owned by main.rs IslandState::radius;
// this module never fills the capsule, so it defines no radius const.)

/// Search bar: flat line, no field fill (video wins over QS).
const SEARCH_Y: f32 = 14.0;
const SEARCH_H: f32 = 34.0; // 14..48, centre 31
const MAGNIFIER_CX: f32 = 23.0;
const MAGNIFIER_CY: f32 = SEARCH_Y + SEARCH_H / 2.0; // 31
const SEARCH_TEXT_X: f32 = 38.0;
const SEARCH_TEXT_TOP: f32 = 19.0; // 12 px, glyph band ~21..33 (iter13)
const SEARCH_PX: f32 = 12.0;
const SEARCH_PLACEHOLDER: &str = "Search wallpapers...";

/// "anime" chip, top-right: pill 56x20, right margin 8, centred y 31.
const CHIP_X: f32 = 944.0;
const CHIP_Y: f32 = 21.0;
const CHIP_W: f32 = 56.0;
const CHIP_H: f32 = 20.0;
const CHIP_R: f32 = 10.0;
const CHIP_PX: f32 = 11.0;
const CHIP_LABEL: &str = "anime";

/// Thumbnail geometry (iter13 spec; see header table).
pub const THUMB_W: u32 = 154;
pub const THUMB_H: u32 = 96;
pub const SEL_W: u32 = 225;
pub const SEL_H: u32 = 138;
/// Thumb left edges, display order (gaps 12, row span 59..948).
pub const THUMB_XS: [f32; 5] = [59.0, 225.0, 391.0, 628.0, 794.0];
/// Unselected thumb top (centred on the selected span 78..216, cy 147).
pub const THUMB_Y: f32 = 99.0;
/// Selected thumb top (78..216).
pub const SEL_Y: f32 = 78.0;
/// Selection outline: thumb rect expanded 2 px, 2 px stroke, r 12.
const OUTLINE_PAD: f32 = 2.0;
const OUTLINE_T: f32 = 2.0;
const OUTLINE_R: f32 = 12.0;

/// Bottom row: 12 px secondary, top 241 (glyph band ~243..255).
/// iter128: was 10 px — filename + counter must read at least as
/// tall as the search line (12 px); panel bottom pad stays 15 px.
const BOTTOM_TEXT_TOP: f32 = 241.0;
const BOTTOM_PX: f32 = 12.0;
const FILENAME_X: f32 = 21.0;
const COUNTER_RIGHT: f32 = 984.0;

pub const C_SECONDARY: (u8, u8, u8) = (255, 255, 255); // #FFFFFF — iter128: meta text white, was #9AA0A6 (154,160,166), peak lum ~143 on screen, contrast ~6:1. White peaks ~255 in-buffer (~237 live after the 1.5x upscale). Hierarchy kept by size (12 px), not by dimming. (Only this panel uses this const — launcher/clipboard/CC have their own.)
pub const C_CHIP_BG: (u8, u8, u8) = (59, 59, 56); // iter13 sample
pub const C_ACCENT: (u8, u8, u8) = (124, 162, 245); // #7CA2F5, island accent
const C_QUERY: (u8, u8, u8) = (255, 255, 255);

// ---------------------------------------------------------------------------
// Thumbnails — REAL wallpaper files, pre-decoded RGBA8 blobs.
// Extracted on the Surface by icons-src/extract-wallpaper-thumbs.py
// (PIL, cover-fit LANCZOS, rounded-corner alpha baked in). The island
// blits these 1:1 — no scaling, no decoder, no file I/O at runtime.
// ---------------------------------------------------------------------------
/// wallpaper-day.jpg (2256x1504) — anime sky, daytime.
/// sha256 7708dadeacbb0e538d72e8e76bd3f048e535b1696030b8ef163491656668a9cb
pub const THUMB_DAY: &[u8; (THUMB_W * THUMB_H * 4) as usize] =
    include_bytes!("wallpaper-thumbs/thumb-day.rgba");
/// wallpaper-dusk.png (2256x1504) — same scene at dusk.
/// sha256 7d4e6b1889fde11537f9fb3ba1a015938425ea28f49a66a4b5423773dd0925d3
pub const THUMB_DUSK: &[u8; (THUMB_W * THUMB_H * 4) as usize] =
    include_bytes!("wallpaper-thumbs/thumb-dusk.rgba");
/// wallpaper.png (2256x1504) — the LIVE wallpaper (iter14 fallback sky).
/// sha256 a9a4080d84a323d96c7b6e6c8fc090492c6ca11038c3aaca363e8453a517a34b
pub const THUMB_WALLPAPER: &[u8; (SEL_W * SEL_H * 4) as usize] =
    include_bytes!("wallpaper-thumbs/thumb-wallpaper.rgba");
/// czd.png (1914x1029) — anime illustration.
/// sha256 f982af1885a9d8adaddbce7f90f06f56a17974bc12220d1e5cb10639e9f21d69
pub const THUMB_CZD: &[u8; (THUMB_W * THUMB_H * 4) as usize] =
    include_bytes!("wallpaper-thumbs/thumb-czd.rgba");
/// wallpaper.catppuccin.png (1920x1080) — dark catppuccin text art.
/// sha256 b337cc43dbf7efe0675648a2e47ec01c0943db3e1119883915da954b7a62929d
pub const THUMB_CATPPUCCIN: &[u8; (THUMB_W * THUMB_H * 4) as usize] =
    include_bytes!("wallpaper-thumbs/thumb-catppuccin.rgba");

/// Real filenames, display order. The bottom-left label shows
/// FILMSTRIP_NAMES[selected] — never a reconstructed Wallhaven ID.
pub const FILMSTRIP_NAMES: [&str; 5] = [
    "wallpaper-day.jpg",
    "wallpaper-dusk.png",
    "wallpaper.png",
    "czd.png",
    "wallpaper.catppuccin.png",
];

/// The blob for filmstrip index `i` (selected index 2 is the large one).
/// Returns (bytes, width, height).
pub fn thumb_bytes_for(i: usize) -> (&'static [u8], u32, u32) {
    match i {
        0 => (THUMB_DAY, THUMB_W, THUMB_H),
        1 => (THUMB_DUSK, THUMB_W, THUMB_H),
        2 => (THUMB_WALLPAPER, SEL_W, SEL_H),
        3 => (THUMB_CZD, THUMB_W, THUMB_H),
        4 => (THUMB_CATPPUCCIN, THUMB_W, THUMB_H),
        _ => (THUMB_WALLPAPER, SEL_W, SEL_H),
    }
}

/// Panel-relative rect of filmstrip thumb `i` (x, y, w, h).
pub fn thumb_rect(i: usize) -> (f32, f32, f32, f32) {
    let x = THUMB_XS[i.min(4)];
    if i == 2 {
        (x, SEL_Y, SEL_W as f32, SEL_H as f32)
    } else {
        (x, THUMB_Y, THUMB_W as f32, THUMB_H as f32)
    }
}

// ---------------------------------------------------------------------------
// Data
// ---------------------------------------------------------------------------
/// The picker's state. Defaults = the video's settled state: the live
/// wallpaper selected (centre, index 2), empty search (placeholder).
/// `search_text` is the compiled-in fallback panel's display field
/// only — the live library panel reads the real query from
/// WALL_SEARCH (iter128 B2).
pub struct WallpaperData {
    pub selected: usize,
    pub search_text: String,
}

impl Default for WallpaperData {
    fn default() -> Self {
        Self {
            selected: 2,
            search_text: String::new(),
        }
    }
}

impl WallpaperData {
    /// Truthful counter for the real 5-item filmstrip, e.g. "3/5".
    pub fn counter_text(&self) -> String {
        format!("{}/{}", self.selected.min(4) + 1, FILMSTRIP_NAMES.len())
    }
    /// The real filename of the selected wallpaper.
    pub fn selected_filename(&self) -> &'static str {
        FILMSTRIP_NAMES[self.selected.min(4)]
    }
}

// ---------------------------------------------------------------------------
// Drawing primitives — same SDF/Argb8888 style as main.rs and
// launcher_panel.rs. Canvas is BGRA bytes, little-endian
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

/// Stroke a rounded-rect outline: outer SDF minus inner SDF, `t` px thick.
fn stroke_rounded_rect(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    rect: Rect,
    radius: f32,
    t: f32,
    rgb: (u8, u8, u8),
) {
    let inner = Rect::new(rect.x + t, rect.y + t, rect.w - 2.0 * t, rect.h - 2.0 * t);
    let x0 = rect.x.floor().max(0.0) as i32;
    let y0 = rect.y.floor().max(0.0) as i32;
    let x1 = (rect.x + rect.w).ceil().min(w as f32) as i32;
    let y1 = (rect.y + rect.h).ceil().min(h as f32) as i32;
    for y in y0..y1 {
        for x in x0..x1 {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let ao = rounded_rect_alpha(px - rect.x, py - rect.y, rect.w, rect.h, radius);
            let ai = rounded_rect_alpha(
                px - inner.x,
                py - inner.y,
                inner.w,
                inner.h,
                (radius - t).max(0.0),
            );
            let a = (ao - ai).clamp(0.0, 1.0);
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

/// AA line segment, `t` px thick (same as launcher_panel.rs).
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

/// Blit one straight-alpha RGBA thumbnail onto the BGRA premultiplied
/// canvas, top-left at (`x0`, `y0`), 1:1 (no scaling — the blobs are
/// extracted at exactly their blit size). Fully transparent source
/// pixels (the baked rounded corners) are skipped; out-of-bounds
/// pixels are clipped, never wrapped. Same math as
/// launcher_icons::blit_icon, generalised to (tw, th).
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

// ---------------------------------------------------------------------------
// Text — fontdue, same pattern as main.rs / launcher_panel.rs
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

/// Draw `text` horizontally centred on `cx`, top of the line box at `y_top`.
fn draw_text_centered(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &Font,
    text: &str,
    px: f32,
    cx: f32,
    y_top: f32,
    rgb: (u8, u8, u8),
    bold: bool,
) {
    let tw = text_width(font, text, px);
    draw_text(canvas, w, h, font, text, px, cx - tw / 2.0, y_top, rgb, bold);
}

/// Search magnifier: ring r 6, 2 px stroke, handle to the lower right.
/// Procedural (same as launcher_panel.rs): the Material Symbols
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
/// Draw the full wallpaper picker into `canvas` (1008x270, Argb8888).
/// `font` is the UI font (Quicksand Bold, loaded by main.rs). The
/// caller (main.rs `draw()`) owns the black capsule fill; this
/// function draws content only. Defaults = the video's settled state
/// (live wallpaper selected, empty search).
pub fn draw_wallpaper_panel_with_data(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &Font,
    data: &WallpaperData,
) {
    let selected = data.selected.min(FILMSTRIP_NAMES.len() - 1);

    // Search line — flat, NO field fill (video wins over QS).
    draw_magnifier(canvas, w, h, MAGNIFIER_CX, MAGNIFIER_CY, C_SECONDARY);
    if data.search_text.is_empty() {
        draw_text(
            canvas, w, h, font, SEARCH_PLACEHOLDER, SEARCH_PX, SEARCH_TEXT_X, SEARCH_TEXT_TOP,
            C_SECONDARY, false,
        );
    } else {
        draw_text(
            canvas, w, h, font, &data.search_text, SEARCH_PX, SEARCH_TEXT_X, SEARCH_TEXT_TOP,
            C_QUERY, false,
        );
    }

    // "anime" chip, top-right.
    fill_rounded_rect(
        canvas,
        w,
        h,
        Rect::new(CHIP_X, CHIP_Y, CHIP_W, CHIP_H),
        CHIP_R,
        C_CHIP_BG,
    );
    draw_text_centered(
        canvas, w, h, font, CHIP_LABEL, CHIP_PX, CHIP_X + CHIP_W / 2.0,
        CHIP_Y + (CHIP_H - CHIP_PX) / 2.0 - 1.0, C_SECONDARY, false,
    );

    // Filmstrip: five REAL thumbnails, centre (selected) enlarged.
    // The selection outline is drawn UNDER the selected thumb's blit
    // only in the sense that the stroke sits 2 px outside the thumb
    // rect, so draw order does not matter — outline first, then blits.
    let (sx, sy, sw, sh) = thumb_rect(selected);
    stroke_rounded_rect(
        canvas,
        w,
        h,
        Rect::new(
            sx - OUTLINE_PAD,
            sy - OUTLINE_PAD,
            sw + 2.0 * OUTLINE_PAD,
            sh + 2.0 * OUTLINE_PAD,
        ),
        OUTLINE_R,
        OUTLINE_T,
        C_ACCENT,
    );
    for i in 0..FILMSTRIP_NAMES.len() {
        let (rgba, tw, th) = thumb_bytes_for(i);
        let (x, y, _, _) = thumb_rect(i);
        blit_rgba(
            canvas,
            w,
            h,
            x.round() as i32,
            y.round() as i32,
            tw,
            th,
            rgba,
        );
    }

    // iter128: filmstrip chevrons (nav-disc family, browse only —
    // see `draw_chevron` / `filmstrip_hit`). Same two calls in the
    // live path below; wherever the hit arm can return Prev/Next,
    // the discs must be drawn.
    draw_chevron(canvas, w, h, CHEV_LEFT_CX, CHEV_CY, -1.0);
    draw_chevron(canvas, w, h, CHEV_RIGHT_CX, CHEV_CY, 1.0);

    // Bottom row: REAL filename (left) + truthful counter (right).
    draw_text(
        canvas, w, h, font, data.selected_filename(), BOTTOM_PX, FILENAME_X, BOTTOM_TEXT_TOP,
        C_SECONDARY, false,
    );
    let counter = data.counter_text();
    let counter_w = text_width(font, &counter, BOTTOM_PX);
    draw_text(
        canvas, w, h, font, &counter, BOTTOM_PX, COUNTER_RIGHT - counter_w, BOTTOM_TEXT_TOP,
        C_SECONDARY, false,
    );
}

/// Defaults: live wallpaper selected (centre), empty search.
pub fn draw_wallpaper_panel(canvas: &mut [u8], w: u32, h: u32, font: &Font) {
    draw_wallpaper_panel_with_data(canvas, w, h, font, &WallpaperData::default());
}

// ---------------------------------------------------------------------------
// iter36 a6b: LIVE library filmstrip (wallpaper-library/themes.json,
// 24 themes / 56 wallpapers). The strip always shows five thumbs:
// [sel-2, sel-1, sel, sel+1, sel+2] (wrapping), the selected one
// enlarged at the centre. ‹ › chevrons browse the selection without
// applying; a thumb click applies that wallpaper + its theme (see
// main.rs WallApply). Falls back to the compiled-in 5-thumb panel
// when the library is not deployed (yet).
// ---------------------------------------------------------------------------

/// Filmstrip chevrons (panel-local): browse the selection without
/// applying. They sit in the gaps beside the first/last thumb
/// (filmstrip x 59..948, cy 147), clear of every thumb rect.
/// Drawn as nav-disc-family discs (Ø28 dark disc + near-white
/// chevron, see `draw_chevron`); hit is circular, disc radius + 2 px
/// slack, checked before the thumbs so the discs own their gaps.
pub const CHEV_LEFT_CX: f32 = 30.0;
pub const CHEV_RIGHT_CX: f32 = 978.0;
pub const CHEV_CY: f32 = 147.0;
pub const CHEV_HIT_R: f32 = 16.0;
/// Chevron disc radius (Ø28, same as the nav discs in main.rs
/// `draw_nav_disc` / NAV_DISC_R) and its material: the nav-disc dark
/// fill #1C1E22 with a near-white (232,234,237) chevron glyph.
pub const CHEV_DISC_R: f32 = 14.0;
const C_CHEV_DISC: (u8, u8, u8) = (28, 30, 34); // #1C1E22
const C_CHEV_GLYPH: (u8, u8, u8) = (232, 234, 237);

/// Hit result for a press in the filmstrip area (panel-local coords).
pub enum FilmHit {
    Thumb(usize),
    Prev,
    Next,
}

pub fn filmstrip_hit(px: f32, py: f32) -> Option<FilmHit> {
    // iter128 B2 (integrator hand-merge): zero matches => the strip
    // is empty (honest empty state) — no thumb exists to hit, so
    // nothing applies. Checked BEFORE the B1 chevron arm below: with
    // zero matches even a chevron click is a no-op (the strip is empty).
    if !wall_query().trim().is_empty() && filtered_wallpaper_indices().is_empty() {
        return None;
    }
    // iter128: the chevron hit arm is back (iter89 F4 removed it
    // with the draw; iter127 proved the picker unusable without it —
    // 30 futile clicks, counter never moved). Circular hit, disc
    // radius + slack, checked BEFORE the thumbs: the discs own the
    // gaps (x 14..46 / 962..994), clear of every thumb rect, so no
    // chevron click can ever land on a thumb and apply.
    let dl = ((px - CHEV_LEFT_CX).powi(2) + (py - CHEV_CY).powi(2)).sqrt();
    if dl <= CHEV_HIT_R {
        return Some(FilmHit::Prev);
    }
    let dr = ((px - CHEV_RIGHT_CX).powi(2) + (py - CHEV_CY).powi(2)).sqrt();
    if dr <= CHEV_HIT_R {
        return Some(FilmHit::Next);
    }
    for pos in 0..5 {
        let (x, y, w, h) = thumb_rect(pos);
        if px >= x && px < x + w && py >= y && py < y + h {
            return Some(FilmHit::Thumb(pos));
        }
    }
    None
}

/// Number of wallpapers the filmstrip browses: the library count when
/// deployed, else the 5 compiled-in thumbs.
pub fn wallpaper_total() -> usize {
    crate::library::manifest()
        .map(|m| m.wallpaper_count())
        .unwrap_or(FILMSTRIP_NAMES.len())
}

/// Global library index shown at filmstrip position `pos` (0..5) when
/// `selected` is the centre: neighbours wrap so the strip is always
/// full: [sel-2, sel-1, sel, sel+1, sel+2] mod n.
pub fn filmstrip_index_n(pos: usize, selected: usize, n: usize) -> usize {
    ((selected as i32 + pos as i32 - 2).rem_euclid(n as i32)) as usize
}

/// Global library index shown at filmstrip position `pos` (0..5),
/// filter-aware (iter128 B2): no query => neighbours of `selected`
/// in the full library (as before); with a query => neighbours of
/// the HIGHLIGHTED match inside the filtered list, wrapping — the
/// strip IS the filtered list. Zero matches => unfiltered neighbours
/// (the draw shows the empty state, so no thumb is visible to click,
/// and filmstrip_hit already returns None there).
pub fn filmstrip_index(pos: usize, selected: usize) -> usize {
    if wall_query().trim().is_empty() {
        return filmstrip_index_n(pos, selected, wallpaper_total());
    }
    let f = filtered_wallpaper_indices();
    if f.is_empty() {
        return filmstrip_index_n(pos, selected, wallpaper_total());
    }
    let centre = with_search(|s| s.selected).min(f.len() - 1);
    f[((centre as i32 + pos as i32 - 2).rem_euclid(f.len() as i32)) as usize]
}

// ---------------------------------------------------------------------------
// iter128 B2: REAL type-to-filter search. Same model as the Launcher
// (`launcher_panel::LIVE`) and Clipboard (`clipboard_panel::CACHE`):
// the panel owns its query + highlight in one Mutex; main.rs owns
// the key dispatch (on_wallpaper_key) and calls wall_* here.
// ---------------------------------------------------------------------------

/// Live search state: the typed query + the highlight position inside
/// the FILTERED list (0 = top result — the Launcher/Clipboard
/// `selected` semantics). Every caller is on the island's single
/// event-loop thread.
struct WallSearch {
    query: String,
    selected: usize,
}

static WALL_SEARCH: Mutex<WallSearch> = Mutex::new(WallSearch {
    query: String::new(),
    selected: 0,
});

fn with_search<R>(f: impl FnOnce(&mut WallSearch) -> R) -> R {
    let mut guard = WALL_SEARCH.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut guard)
}

/// Current query (untrimmed — trimming happens at match time).
pub fn wall_query() -> String {
    with_search(|s| s.query.clone())
}

/// Substring match for one library entry: the query matches when it
/// appears (case-insensitive) in the wallpaper filename or the
/// owning theme name. Empty query matches everything. Pure — the
/// unit tests below cover it without a manifest.
fn matches_query(file: &str, theme: &str, q: &str) -> bool {
    let q = q.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    file.to_lowercase().contains(&q) || theme.to_lowercase().contains(&q)
}

/// Global library indices matching the current query, in library
/// order. Empty query returns every index (0..n) — the unfiltered
/// strip. No manifest (compiled-in fallback panel) matches against
/// the five compiled-in filenames only.
pub fn filtered_wallpaper_indices() -> Vec<usize> {
    let q = wall_query();
    if let Some(m) = crate::library::manifest() {
        let n = m.wallpaper_count();
        (0..n)
            .filter(|&i| {
                m.wallpaper_at(i)
                    .map(|(theme, file)| matches_query(file, theme, &q))
                    .unwrap_or(false)
            })
            .collect()
    } else {
        (0..FILMSTRIP_NAMES.len())
            .filter(|&i| matches_query(FILMSTRIP_NAMES[i], "", &q))
            .collect()
    }
}

/// Global library index of the highlighted match, if any. Empty
/// query => the island's current selection (`selected`, the strip
/// centre); a query => filtered[selected], clamped to the last
/// match. None only when the filter matches nothing.
pub fn wall_highlighted(selected: usize) -> Option<usize> {
    if wall_query().trim().is_empty() {
        let n = wallpaper_total().max(1);
        return Some(selected.min(n - 1));
    }
    let f = filtered_wallpaper_indices();
    if f.is_empty() {
        return None;
    }
    let pos = with_search(|s| s.selected).min(f.len() - 1);
    Some(f[pos])
}

/// Fresh search: empty query, highlight back to the top result.
/// Called on Wallpaper entry and on Esc (main.rs); also after an
/// Enter-apply, so the strip recentres on the applied wallpaper.
pub fn wall_reset() {
    with_search(|s| {
        s.query.clear();
        s.selected = 0;
    });
}

/// Append typed text; highlight resets to the top result.
pub fn wall_push_text(text: &str) {
    with_search(|s| {
        s.query.push_str(text);
        s.selected = 0;
    });
}

/// Delete the last query character (char-safe); highlight resets.
pub fn wall_backspace() {
    with_search(|s| {
        s.query.pop();
        s.selected = 0;
    });
}

/// Move the highlight one match (dir -1 up / +1 down), clamped at
/// both ends (no wrap — same as Launcher/Clipboard). No-op when the
/// filter matches nothing.
pub fn wall_move_selection(dir: i32) {
    let n = filtered_wallpaper_indices().len();
    if n == 0 {
        with_search(|s| s.selected = 0);
        return;
    }
    with_search(|s| {
        let (sel, _) = crate::selection::move_by(s.selected, 0, dir, n, 5);
        s.selected = sel;
    });
}

/// Truncate `text` to `max_w` at `px` (char cut + "…" — a query has
/// no word boundaries worth keeping). The full text stays in state;
/// only the display truncates, so a long query never runs under the
/// theme chip.
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

/// Honest empty state for a filter with zero matches: the query
/// stays visible in the search line, the filmstrip area carries "No
/// wallpapers match ..." in secondary grey, and the counter reads
/// 0/<total>. No stale thumbs, no stale filename — a filter with
/// zero matches must never show the old list.
fn draw_wallpaper_empty(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &Font,
    query: &str,
    total: usize,
) {
    draw_magnifier(canvas, w, h, MAGNIFIER_CX, MAGNIFIER_CY, C_SECONDARY);
    let shown = truncate_to_width(font, query.trim(), SEARCH_PX, 900.0);
    draw_text(
        canvas, w, h, font, &shown, SEARCH_PX, SEARCH_TEXT_X, SEARCH_TEXT_TOP,
        C_QUERY, false,
    );
    let msg = format!(
        "No wallpapers match \"{}\"",
        truncate_to_width(font, query.trim(), SEARCH_PX, 560.0)
    );
    draw_text_centered(
        canvas, w, h, font, &msg, 12.0, PANEL_W as f32 / 2.0, 141.0, C_SECONDARY, false,
    );
    let counter = format!("0/{}", total);
    let counter_w = text_width(font, &counter, BOTTOM_PX);
    draw_text(
        canvas, w, h, font, &counter, BOTTOM_PX, COUNTER_RIGHT - counter_w, BOTTOM_TEXT_TOP,
        C_SECONDARY, false,
    );
}

/// One browse step (iter128): move `selected` by `dir` over `n`
/// items, wrapping. `dir` is ±1 ONLY — the chevron contract
/// (main.rs `WallWindow` is fed exclusively by `FilmHit::Prev` /
/// `Next`, i.e. −1 / +1). Every index 0..n is reachable from any
/// start, so the iter127 parity trap is impossible: a target at an
/// odd index is never unreachable from an even one (48 → 47 → 46 →
/// 45 → 44 reaches gwwwxd in 4 chevron clicks; the old edge-thumb
/// walk needed 13 +1 clicks to recover from its own overshoot).
/// This is the ONLY stepping in the picker: a thumb click is not a
/// step, it is a direct apply of the visible thumb (see
/// `filmstrip_index` — pos 0/4 apply sel∓2 / sel±2, pos 1/3 sel∓1 /
/// sel±1, pos 2 re-applies sel). That jump is visible and
/// predictable from the strip itself; what was broken was that no
/// ±1 navigation existed at all, and this function + the chevrons
/// are the fix for exactly that.
pub fn step_selection(selected: usize, dir: i32, n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    debug_assert!(
        dir == -1 || dir == 1,
        "wallpaper browse steps are ±1, got {dir}"
    );
    (selected as i32 + dir).rem_euclid(n as i32) as usize
}

fn draw_chevron(canvas: &mut [u8], w: u32, h: u32, cx: f32, cy: f32, dir: f32) {
    // iter128: nav-disc family, not a bare glyph. The iter89 bare
    // chevron (2.5 px secondary-grey strokes, no disc) was removed
    // with the hit arm; the iter127 screenshots measured the gap
    // regions at luminance 4.0/7.1 with zero px > 40 — nothing was
    // drawn at all. This version draws the same Ø28 disc as the
    // panel's nav discs (main.rs `draw_nav_disc`: fill #1C1E22,
    // near-white chevron, 2.4 px strokes, half-run 4 / half-height 6)
    // so the control reads over a bright thumb edge as well as over
    // the dark panel. Full coverage, not the nav discs' 0.92: over a
    // bright frost pixel the disc must stay dark, not let 8% of the
    // wallpaper through. No rim highlight (main.rs `disc_rim` is
    // private to main.rs) — the disc + glyph carry the read.
    fill_rounded_rect(
        canvas,
        w,
        h,
        Rect::new(
            cx - CHEV_DISC_R,
            cy - CHEV_DISC_R,
            2.0 * CHEV_DISC_R,
            2.0 * CHEV_DISC_R,
        ),
        CHEV_DISC_R,
        C_CHEV_DISC,
    );
    let tip = (cx + dir * 4.0, cy);
    let a = (cx - dir * 4.0, cy - 6.0);
    let b = (cx - dir * 4.0, cy + 6.0);
    stroke_segment(canvas, w, h, tip, a, 2.4, C_CHEV_GLYPH);
    stroke_segment(canvas, w, h, tip, b, 2.4, C_CHEV_GLYPH);
}

/// Live filmstrip for the selected library index `selected`./// Live filmstrip for the selected library index `selected`.
/// iter128 B2: with a query the strip IS the filtered list — centre
/// is the highlighted match, neighbours are the adjacent matches,
/// counter reads "<match>/<matches>"; zero matches draws the honest
/// empty state (no stale thumbs). No query => the old behaviour.
pub fn draw_wallpaper_panel_live(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &Font,
    selected: usize,
) {
    let Some(m) = crate::library::manifest() else {
        draw_wallpaper_panel(canvas, w, h, font);
        return;
    };
    let n = m.wallpaper_count();
    if n == 0 {
        draw_wallpaper_panel(canvas, w, h, font);
        return;
    }
    let query = wall_query();
    let filtering = !query.trim().is_empty();

    // Centre index + counter + the five strip indices. Filtered: the
    // highlighted match is the centre; unfiltered: `selected`.
    let (sel, counter, strip): (usize, String, Vec<usize>) = if filtering {
        let f = filtered_wallpaper_indices();
        if f.is_empty() {
            draw_wallpaper_empty(canvas, w, h, font, &query, n);
            return;
        }
        let centre = with_search(|s| s.selected).min(f.len() - 1);
        let strip: Vec<usize> = (0..5)
            .map(|pos| {
                f[((centre as i32 + pos as i32 - 2).rem_euclid(f.len() as i32)) as usize]
            })
            .collect();
        (f[centre], format!("{}/{}", centre + 1, f.len()), strip)
    } else {
        let sel = selected.min(n - 1);
        let strip: Vec<usize> = (0..5).map(|pos| filmstrip_index_n(pos, sel, n)).collect();
        (sel, format!("{}/{}", sel + 1, n), strip)
    };
    let (sel_theme, sel_file) = m.wallpaper_at(sel).unwrap();

    // Theme chip, top-right: the selected wallpaper's owning theme.
    // Width grows with the name (right edge fixed at 1000). Computed
    // before the search line so a live query can be truncated to it.
    let chip_w = (text_width(font, sel_theme, CHIP_PX) + 16.0).max(CHIP_W);
    let chip_x = 1000.0 - chip_w;

    // Search line — flat, NO field fill (video wins over QS).
    // Placeholder, or the live query in white (truncated to the chip).
    draw_magnifier(canvas, w, h, MAGNIFIER_CX, MAGNIFIER_CY, C_SECONDARY);
    if query.trim().is_empty() {
        draw_text(
            canvas, w, h, font, SEARCH_PLACEHOLDER, SEARCH_PX, SEARCH_TEXT_X, SEARCH_TEXT_TOP,
            C_SECONDARY, false,
        );
    } else {
        let shown = truncate_to_width(
            font,
            query.trim(),
            SEARCH_PX,
            chip_x - 8.0 - SEARCH_TEXT_X,
        );
        draw_text(
            canvas, w, h, font, &shown, SEARCH_PX, SEARCH_TEXT_X, SEARCH_TEXT_TOP,
            C_QUERY, false,
        );
    }

    // Theme chip, top-right.
    fill_rounded_rect(
        canvas,
        w,
        h,
        Rect::new(chip_x, CHIP_Y, chip_w, CHIP_H),
        CHIP_R,
        C_CHIP_BG,
    );
    draw_text_centered(
        canvas, w, h, font, sel_theme, CHIP_PX, chip_x + chip_w / 2.0,
        CHIP_Y + (CHIP_H - CHIP_PX) / 2.0 - 1.0, C_SECONDARY, false,
    );

    // Filmstrip: five REAL library thumbnails, centre (selected)
    // enlarged. A thumb whose file has not arrived yet (library
    // transfer in progress) leaves its slot empty — the click still
    // routes and reports the missing file instead of faking a thumb.
    let (sx, sy, sw, sh) = thumb_rect(2);
    let accent = crate::library::theme_accent();
    stroke_rounded_rect(
        canvas,
        w,
        h,
        Rect::new(
            sx - OUTLINE_PAD,
            sy - OUTLINE_PAD,
            sw + 2.0 * OUTLINE_PAD,
            sh + 2.0 * OUTLINE_PAD,
        ),
        OUTLINE_R,
        OUTLINE_T,
        accent,
    );
    for pos in 0..5 {
        let idx = strip[pos];
        let (_, file) = m.wallpaper_at(idx).unwrap();
        let stem = file.rsplit_once('.').map(|(s, _)| s).unwrap_or(file);
        let (tw, th) = if pos == 2 {
            (SEL_W, SEL_H)
        } else {
            (THUMB_W, THUMB_H)
        };
        if let Some(rgba) = crate::library::thumb_rgba(stem, tw) {
            let (x, y, _, _) = thumb_rect(pos);
            blit_rgba(canvas, w, h, x.round() as i32, y.round() as i32, tw, th, &rgba);
        }
    }

    // iter128 B1 (integrator hand-merge): filmstrip chevrons
    // (nav-disc family, browse only — see `draw_chevron` /
    // `filmstrip_hit`), drawn after the thumbs, in the gaps beside
    // the end thumbs — clear of every thumb rect. Click = browse ±1
    // only, never apply (filmstrip_hit → main.rs WallWindow).
    draw_chevron(canvas, w, h, CHEV_LEFT_CX, CHEV_CY, -1.0);
    draw_chevron(canvas, w, h, CHEV_RIGHT_CX, CHEV_CY, 1.0);

    // Bottom row: REAL filename (left) + truthful counter (right).
    draw_text(
        canvas, w, h, font, sel_file, BOTTOM_PX, FILENAME_X, BOTTOM_TEXT_TOP,
        C_SECONDARY, false,
    );
    let counter_w = text_width(font, &counter, BOTTOM_PX);
    draw_text(
        canvas, w, h, font, &counter, BOTTOM_PX, COUNTER_RIGHT - counter_w, BOTTOM_TEXT_TOP,
        C_SECONDARY, false,
    );
}

// ---------------------------------------------------------------------------
// Tests — render the real panel offscreen (no Wayland) and assert the
// content is there: search text, five distinct real thumbnails, the
// selection outline, the truthful filename + counter, the chip.
// Run on the Surface scratch copy: `cargo test --offline`.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    fn load_test_font() -> Font {
        let home = std::env::var("HOME").expect("HOME must be set for the panel test");
        for name in ["Quicksand-Bold.ttf", "Quicksand.ttf"] {
            let path = format!("{home}/.local/share/fonts/{name}");
            if let Ok(bytes) = std::fs::read(&path) {
                if let Ok(f) = Font::from_bytes(bytes, fontdue::FontSettings::default()) {
                    return f;
                }
            }
        }
        panic!("no Quicksand font found under ~/.local/share/fonts");
    }

    /// Render the default panel into a fresh buffer. main.rs fills the
    /// black capsule first, so the test pre-fills opaque black too.
    fn render_default() -> (Vec<u8>, Font) {
        let font = load_test_font();
        let mut canvas = vec![0u8; (PANEL_W * PANEL_H * 4) as usize];
        for px in canvas.chunks_exact_mut(4) {
            px[0] = 0;
            px[1] = 0;
            px[2] = 0;
            px[3] = 255;
        }
        draw_wallpaper_panel(&mut canvas, PANEL_W, PANEL_H, &font);
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
    fn layout_matches_iter13_spec() {
        // Thumb rects (iter13): T1 59..213, T2 225..379, T3 sel 391..616,
        // T4 628..782, T5 794..948; unselected y 99..195, selected 78..216.
        assert_eq!(thumb_rect(0), (59.0, 99.0, 154.0, 96.0));
        assert_eq!(thumb_rect(1), (225.0, 99.0, 154.0, 96.0));
        assert_eq!(thumb_rect(2), (391.0, 78.0, 225.0, 138.0));
        assert_eq!(thumb_rect(3), (628.0, 99.0, 154.0, 96.0));
        assert_eq!(thumb_rect(4), (794.0, 99.0, 154.0, 96.0));
        // Centres share y = 147.
        for i in 0..5 {
            let (_, y, _, h) = thumb_rect(i);
            assert!((y + h / 2.0 - 147.0).abs() < 0.01, "thumb {i} not centred on 147");
        }
        // Truthfulness: label + counter derive from the real filmstrip.
        let data = WallpaperData::default();
        assert_eq!(data.selected, 2);
        assert_eq!(data.selected_filename(), "wallpaper.png");
        assert_eq!(data.counter_text(), "3/5");
        assert_eq!(FILMSTRIP_NAMES.len(), 5);
        assert!(!FILMSTRIP_NAMES.contains(&"wallhaven-4d9....png"));
        // No duplicate source in the set (dusk != twilight by exclusion).
        assert_eq!(thumb_bytes_for(1).0.len(), (154 * 96 * 4) as usize);
        assert_eq!(thumb_bytes_for(2).0.len(), (225 * 138 * 4) as usize);
    }

    #[test]
    fn renders_real_content() {
        let (canvas, _font) = render_default();
        // Search line: magnifier + "Search wallpapers..." present.
        let search = count_bright(&canvas, 15, 14, 200, 48, 150);
        println!("search-band bright px: {search}");
        assert!(search > 150, "search line missing: {search} bright px");
        // Chip: grey pill present top-right (bg sum 174, text brighter).
        let chip = count_bright(&canvas, 944, 21, 1000, 41, 100);
        println!("chip bright px: {chip}");
        assert!(chip > 400, "anime chip missing: {chip} bright px");
        // Selected thumb: bright cloud image, mean luminance high.
        let (mr, mg, mb) = mean_rgb(&canvas, 400, 90, 607, 204);
        let lum = 0.299 * mr + 0.587 * mg + 0.114 * mb;
        println!("selected thumb mean RGB ({mr:.0},{mg:.0},{mb:.0}) lum {lum:.0}");
        assert!(lum > 80.0, "selected thumb too dark: lum {lum:.0}");
        // Selection outline: accent-blue pixels on the top edge band.
        let mut outline = 0;
        for y in 76..79 {
            for x in 410..600 {
                let (r, g, b) = px(&canvas, x, y);
                if b > 180 && b as i32 - r as i32 > 30 {
                    outline += 1;
                }
            }
        }
        println!("outline accent px (top edge): {outline}");
        assert!(outline > 100, "selection outline missing: {outline} px");
        // Bottom row: filename left, counter right.
        let label = count_bright(&canvas, 15, 240, 140, 258, 150);
        let counter = count_bright(&canvas, 950, 240, 990, 258, 150);
        println!("filename bright px: {label}, counter bright px: {counter}");
        assert!(label > 30, "filename label missing: {label} bright px");
        assert!(counter > 10, "counter missing: {counter} bright px");
        // iter128: meta text is white now (was #9AA0A6, peak ~166 in
        // the buffer) — filename + counter must peak bright like the
        // pill clock (255), not mid-grey.
        let label_peak = peak_lum(&canvas, 15, 240, 200, 258);
        println!("filename peak lum: {label_peak}");
        assert!(label_peak > 200, "filename still dim: peak {label_peak}");
        let counter_peak = peak_lum(&canvas, 940, 240, 990, 258);
        println!("counter peak lum: {counter_peak}");
        assert!(counter_peak > 200, "counter still dim: peak {counter_peak}");
        // Thumbs are distinct images, not one blob repeated five times.
        let m0 = mean_rgb(&canvas, 70, 110, 200, 184);
        let m3 = mean_rgb(&canvas, 640, 110, 770, 184);
        let m4 = mean_rgb(&canvas, 806, 110, 936, 184);
        println!("thumb means day={m0:?} czd={m3:?} cat={m4:?}");
        let diff = |a: (f32, f32, f32), b: (f32, f32, f32)| {
            (a.0 - b.0).abs().max((a.1 - b.1).abs()).max((a.2 - b.2).abs())
        };
        assert!(diff(m0, m3) > 15.0, "day and czd thumbs too similar");
        assert!(diff(m0, m4) > 15.0, "day and catppuccin thumbs too similar");
        assert!(diff(m3, m4) > 15.0, "czd and catppuccin thumbs too similar");
        // Optional render dump for visual inspection (builder only — set
        // DYNAMITE_DUMP_RENDER=/tmp/wallpaper-panel-render.bgra when
        // running the test; the buffer is BGRA bytes, 1008x270x4).
        if let Ok(path) = std::env::var("DYNAMITE_DUMP_RENDER") {
            std::fs::write(&path, &canvas).expect("dump render");
            println!("render dumped to {path}");
        }
    }

    // iter128 B2: type-to-filter matching + render behaviour.
    #[test]
    fn search_matches_filename_theme_or_nothing() {
        assert!(matches_query("wallhaven-k8jgl6.png", "anime", "k8jgl6"));
        assert!(matches_query("wallhaven-k8jgl6.png", "anime", "K8JGL6"));
        assert!(matches_query("wallhaven-k8jgl6.png", "anime", "wallhaven"));
        assert!(matches_query("wallhaven-k8jgl6.png", "anime", "anime"));
        assert!(matches_query("wallhaven-k8jgl6.png", "anime", "ANIME"));
        assert!(matches_query("wallhaven-gwwwxd.jpg", "snowshrine", "snow"));
        assert!(!matches_query("wallhaven-k8jgl6.png", "anime", "zzz"));
        assert!(!matches_query("wallhaven-k8jgl6.png", "anime", "snow"));
        assert!(matches_query("wallhaven-k8jgl6.png", "anime", ""));
        assert!(matches_query("wallhaven-k8jgl6.png", "anime", "   "));
    }

    #[test]
    fn search_filters_live_strip_and_empties_honestly() {
        // One test fn (not three): the query state is a shared
        // static, so the whole sequence runs serially here.
        // Manifest-dependent counts are gated on the library being
        // deployed (it is, on the Surface scratch copy).
        wall_reset();
        assert_eq!(filtered_wallpaper_indices().len(), wallpaper_total());
        let Some(_) = crate::library::manifest() else {
            wall_reset();
            return;
        };
        assert_eq!(wallpaper_total(), 56);
        // "k8jgl6" matches exactly one library file (global index 0).
        wall_push_text("k8jgl6");
        assert_eq!(filtered_wallpaper_indices().len(), 1);
        assert_eq!(wall_highlighted(44), Some(0));
        assert_eq!(filmstrip_index(2, 44), 0);
        // The filtered strip renders: query text in the search band,
        // the one match as the centre thumb.
        let font = load_test_font();
        let mut canvas = vec![0u8; (PANEL_W * PANEL_H * 4) as usize];
        for px in canvas.chunks_exact_mut(4) {
            px[3] = 255;
        }
        draw_wallpaper_panel_live(&mut canvas, PANEL_W, PANEL_H, &font, 44);
        let search = count_bright(&canvas, 15, 14, 200, 48, 150);
        assert!(search > 40, "filtered query not drawn: {search} bright px");
        let (mr, mg, mb) = mean_rgb(&canvas, 400, 90, 607, 204);
        let lum = 0.299 * mr + 0.587 * mg + 0.114 * mb;
        assert!(lum > 80.0, "filtered centre thumb too dark: lum {lum:.0}");
        // Highlight moves clamp at the single match.
        wall_move_selection(1);
        assert_eq!(wall_highlighted(44), Some(0));
        wall_move_selection(-1);
        assert_eq!(wall_highlighted(44), Some(0));
        // Extend the query to a zero-match filter: honest empty
        // state, no stale thumbs, no highlight, no hit.
        wall_push_text("zzz-no-such-wallpaper");
        assert_eq!(filtered_wallpaper_indices().len(), 0);
        assert_eq!(wall_highlighted(44), None);
        assert!(filmstrip_hit(500.0, 147.0).is_none());
        let mut canvas2 = vec![0u8; (PANEL_W * PANEL_H * 4) as usize];
        for px in canvas2.chunks_exact_mut(4) {
            px[3] = 255;
        }
        draw_wallpaper_panel_live(&mut canvas2, PANEL_W, PANEL_H, &font, 44);
        let msg = count_bright(&canvas2, 200, 130, 808, 165, 150);
        assert!(msg > 30, "empty-state message missing: {msg} bright px");
        let dark = mean_rgb(&canvas2, 400, 90, 607, 204);
        let lum2 = 0.299 * dark.0 + 0.587 * dark.1 + 0.114 * dark.2;
        assert!(lum2 < 20.0, "empty state shows a stale thumb: lum {lum2:.0}");
        // Reset restores the full list.
        wall_reset();
        assert_eq!(filtered_wallpaper_indices().len(), 56);
        assert!(wall_query().is_empty());
        assert_eq!(wall_highlighted(44), Some(44));
    }

    // iter36 a6b: live filmstrip hit-testing + index wrapping.
    #[test]
    fn filmstrip_hit_chevrons_and_thumbs() {
        assert!(matches!(filmstrip_hit(30.0, 147.0), Some(FilmHit::Prev)));
        assert!(matches!(filmstrip_hit(978.0, 147.0), Some(FilmHit::Next)));
        // The iter127 probe points (screen-logical (278,178) /
        // (1225,178) minus the (248,29) panel origin).
        assert!(matches!(filmstrip_hit(30.0, 149.0), Some(FilmHit::Prev)));
        assert!(matches!(filmstrip_hit(977.0, 149.0), Some(FilmHit::Next)));
        // Disc edge (r 16): inside at 15, outside at 17.
        assert!(matches!(filmstrip_hit(30.0 + 15.0, 147.0), Some(FilmHit::Prev)));
        assert!(filmstrip_hit(30.0 - 17.0, 147.0).is_none());
        assert!(matches!(filmstrip_hit(978.0 - 15.0, 147.0), Some(FilmHit::Next)));
        assert!(filmstrip_hit(978.0 + 17.0, 147.0).is_none());
        assert!(matches!(filmstrip_hit(391.0 + 10.0, 78.0 + 10.0), Some(FilmHit::Thumb(2))));
        assert!(matches!(filmstrip_hit(59.0 + 10.0, 99.0 + 10.0), Some(FilmHit::Thumb(0))));
        // Gap between thumbs / search band: no hit.
        assert!(filmstrip_hit(500.0, 31.0).is_none());
        assert!(filmstrip_hit(500.0, 250.0).is_none());
    }

    // iter128: chevron stepping is uniformly ±1 with wrap — the
    // iter127 parity trap (gwwwxd at 45 unreachable by −2 from 48)
    // is impossible by construction.
    #[test]
    fn step_selection_is_uniform_pm1() {
        // Single steps around the live position (44 = gwwwxd).
        assert_eq!(step_selection(44, -1, 56), 43);
        assert_eq!(step_selection(44, 1, 56), 45);
        // Wrap at both ends: 1/56 <- -1 -> 56/56, 56/56 -+1-> 1/56.
        assert_eq!(step_selection(0, -1, 56), 55);
        assert_eq!(step_selection(55, 1, 56), 0);
        // The iter127 recovery, now by chevron: 48 -> 44 in 4 left
        // steps (was 13 +1 thumb clicks after an 8-click overshoot).
        let mut sel = 48;
        for _ in 0..4 {
            sel = step_selection(sel, -1, 56);
        }
        assert_eq!(sel, 44);
        // Every index reachable: 56 left steps from any start visit
        // all 56 indices exactly once.
        let mut seen = [false; 56];
        let mut s = 48;
        for _ in 0..56 {
            assert!(!seen[s], "index {s} visited twice in one full turn");
            seen[s] = true;
            s = step_selection(s, -1, 56);
        }
        assert!(seen.iter().all(|v| *v));
        // Degenerate library: no panic, stays 0.
        assert_eq!(step_selection(0, 1, 0), 0);
    }

    // iter128: the chevron discs are actually drawn in the live
    // render — near-white glyph pixels AND the dark disc fill in
    // both gap regions (the iter127 regression was zero px > 40
    // there). Rendered at the live selection (44 = gwwwxd, counter
    // 45/56); the discs sit in the thumb-row gaps, clear of every
    // thumb, so the counts below can only come from the discs.
    #[test]
    fn live_chevrons_visible() {
        let font = load_test_font();
        let mut canvas = vec![0u8; (PANEL_W * PANEL_H * 4) as usize];
        for chunk in canvas.chunks_exact_mut(4) {
            chunk[3] = 255;
        }
        draw_wallpaper_panel_live(&mut canvas, PANEL_W, PANEL_H, &font, 44);
        for (label, cx) in [("left", CHEV_LEFT_CX), ("right", CHEV_RIGHT_CX)] {
            let (mut glyph, mut disc) = (0usize, 0usize);
            let x0 = (cx - CHEV_DISC_R) as i32;
            let y0 = (CHEV_CY - CHEV_DISC_R) as i32;
            for y in y0..(y0 + 28) {
                for x in x0..(x0 + 28) {
                    let (r, g, b) = px(&canvas, x, y);
                    if r > 200 && g > 200 && b > 200 {
                        glyph += 1;
                    }
                    let s = r as u32 + g as u32 + b as u32;
                    if (60..=150).contains(&s) {
                        disc += 1;
                    }
                }
            }
            println!("{label} chevron: near-white glyph px={glyph}, disc-fill px={disc}");
            assert!(glyph > 20, "{label} chevron glyph missing: {glyph} near-white px");
            assert!(disc > 300, "{label} chevron disc missing: {disc} disc-fill px");
        }
    }

    #[test]
    fn filmstrip_index_wraps() {
        assert_eq!(filmstrip_index_n(2, 5, 56), 5);
        assert_eq!(filmstrip_index_n(0, 0, 56), 54);
        assert_eq!(filmstrip_index_n(4, 55, 56), 1);
        assert_eq!(filmstrip_index_n(0, 2, 5), 0);
    }

    #[test]
    fn live_draw_renders_without_panic() {
        // Works with or without the deployed library (fallback path
        // renders the compiled-in strip when the manifest is absent).
        let font = load_test_font();
        let mut canvas = vec![0u8; (PANEL_W * PANEL_H * 4) as usize];
        for px in canvas.chunks_exact_mut(4) {
            px[3] = 255;
        }
        draw_wallpaper_panel_live(&mut canvas, PANEL_W, PANEL_H, &font, 0);
        assert!(count_bright(&canvas, 0, 0, PANEL_W as i32, PANEL_H as i32, 60) > 100);
    }

}
