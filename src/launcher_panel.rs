//! launcher_panel — Launcher state (504x366) for dynamite-rust.
//!
//! Layout spec + `draw_launcher_panel()` in the same manual-SDF /
//! Argb8888 style as `main.rs` (no tiny-skia, no cosmic-text, no new
//! dependencies — only `fontdue`, already in Cargo.toml).
//!
//! SOURCES (do not re-derive from memory):
//! - `analysis/frame-measurements.md`: launcher 504x366, top gap 15.
//! - `frames/per-second/s06.png` — video frame: flat "Search..." line on
//!   black (magnifier + grey placeholder, NO visible field), then 6 rows,
//!   alphabetical, each row = real app icon + bold name + grey
//!   description, first row highlighted.
//! - `iterations/critic-launcher.md` — harsh critic on the live Quickshell
//!   render (12 Nov ~21:22 CET). Its verdicts drive this module:
//!   defect 1 (every icon the same `apps` dot-grid placeholder),
//!   defect 2 (big grey `#1A1C1F` search box the video does not have),
//!   defect 5 (rows in raw enumeration order, not sorted),
//!   defect 6 (caret striking through the placeholder).
//! - `iterations/critic-rust-launcher-iter6.md` — harsh critic on Rust v0.1
//!   (panel geometry PASS, content 0%: no search bar, no rows, no icons,
//!   no highlight, only a centred grey "launcher" stub). Its iter7
//!   checklist items 2-5 are implemented here; item 1 (grey search bar
//!   "matching QS") is DELIBERATELY NOT followed — see SEARCH below.
//! - `configs/quickshell/dynamite/shell.qml` — structural skeleton only
//!   (inner padding 14, search 36 px, rows 42 px, pitch 47.5, radius 9,
//!   name 13 px bold, comment 10 px, highlight `#05163C` (iter96 tint; QS was `#1F2124`), field
//!   `#1A1C1F`). Where QS disagrees with the VIDEO, the video wins and
//!   the QS value is noted as superseded.
//!
//! WIRING (integrator — do NOT do it in this file, `main.rs` is untouched):
//! ```ignore
//! // main.rs, top of file:
//! mod launcher_panel;
//! // main.rs, in draw(): replace the Launcher placeholder arm with:
//! //   IslandState::Launcher => {
//! //       launcher_panel::draw_launcher_panel(
//! //           canvas, w, h, font, icon_font.as_ref());
//! //       return;
//! //   }
//! // To show a live query / a different app set, use
//! // draw_launcher_panel_with_data(..., &LauncherData::...) instead.
//! ```
//!
//! SEARCH — FLAT, no fake input (critic-launcher defect 2 + 6):
//! The video has NO search field: just a magnifier glyph and grey
//! "Search..." sitting flat on the black panel. The Quickshell grey box
//! (`fieldBg #1A1C1F`, 36 px, radius 10) is therefore NOT drawn here —
//! `C_FIELD_BG` is kept only as a documented-superseded constant. The
//! search line is display-only: a non-empty `query` in `LauncherData`
//! renders as plain white text, with NO caret and NO box, because this
//! module owns no keyboard input (the island has no pointer/keyboard
//! input yet). Filtering itself is pure: `LauncherData::filtered()`
//! matches the query case-insensitively against app names and draws at
//! most 6 rows; the full filtered list is reachable by Up/Down or
//! wheel scrolling (iter86), and when fewer than 6 rows match the
//! drawn rows stretch to fill the row block (see
//! `launcher_layout_for_count`). The iter6 checklist item 1 ("rounded grey bar ... matching
//! QS") conflicts with the video and with critic-launcher defect 2;
//! the video wins — if a future critic re-measures s06 and finds a
//! field after all, reintroduce the box then, with the measurement.
//!
//! ICONS — REAL ICON FILES, drawn glyphs only as fallback:
//! Each row's icon is blitted from `launcher_icons.rs` (24x24 RGBA8
//! blobs of the apps' actual icon files — see that file for sources,
//! digests and saturation stats). The blit is centred on the 20 px
//! slot (24 px icon, 2 px overshoot each side, inside the 42 px row).
//! The procedural glyphs below (gauge, Bluetooth rune, "B", archive,
//! folder, brush) are FALLBACK ONLY, used when `icon_bytes_for()`
//! returns None for a row's name — i.e. only for app rows that have
//! no extracted real icon. None of the fallback glyphs is the app's
//! real icon file. Do NOT describe a screenshot of this panel as
//! showing "real icons" unless the row's icon came from
//! `launcher_icons.rs` — check the per-slot saturation test
//! (critic-rust-launcher-iter10.md D1) instead of assuming.
//! Two upstream icons differ in hue from the video's Papirus-theme
//! renders (File Roller tan vs video green; Files blue vs video pale
//! grey) — documented in launcher_icons.rs HUE NOTE; they are still
//! the apps' real icons, and swapping in Papirus SVGs is a later
//! follow-up, not a reason to draw glyphs.
//!
//! APP SET: `LauncherData::video_defaults()` is the six apps from s06 /
//! the QS static list, already alphabetical (case-insensitive sort
//! keeps this order). The Surface's live QS set (Foot, VSCodium, Files,
//! magpie, Display Scaler, NectarLink — critic-launcher.md) is NOT
//! used: its comments/exec lines were never verified against the
//! Surface's .desktop files, and the iter6 checklist explicitly allows
//! either set ("decide and document"). The integrator may swap in a
//! live set via `LauncherData { apps, .. }` once it has read the real
//! .desktop entries — that is the honest route to the Surface's apps.
//!
//! TEXT WEIGHT: iter10 — `Quicksand-Bold.ttf` (static, wght 700) IS now
//! installed on the Surface and main.rs loads it as THE text font, so
//! the `bold` flag's double-blit is OFF at every call site in this file
//! (the base font is already Bold; double-blitting it would over-thicken
//! the names). The flag stays in the signature — pass `false`.
//!
//! TYPECHECK HONESTY: this file was written 12 Nov 2026 in a VM with NO
//! Rust toolchain installed (`which cargo rustc` → nothing), and the
//! task forbids building on the Surface. It has NOT been typechecked.
//! The integrator MUST run `cargo check --offline` on the Surface
//! before wiring it in; the API surface is deliberately minimal
//! (`draw_launcher_panel` + `draw_launcher_panel_with_data` +
//! `LauncherData`) so a check failure is local to this file.
//!
//! ---------------------------------------------------------------------------
//! GEOMETRY — all panel-relative logical px, canvas 504x366, radius 20.
//! (Panel top edge sits at screen y = 15 via the layer margin, as usual.
//!  QS row rects measured by the iter6 critic: selected row 58.0-99.3,
//!  then 116.0-142.0, 166.0-192.0, 218.0-242.7, 266.0-292.7,
//!  316.0-342.7 — consistent with the QS arithmetic below: rows at
//!  y = 58 + i*50, h = 42, last row bottom 350, bottom pad 16.)
//! ---------------------------------------------------------------------------
//!   Element                     x            y            w      h
//!   --------------------------  -----------  -----------  -----  ----
//!   Panel                       0            0            504    366
//!   Search line (flat)          14           14..50       476    36
//!     magnifier (ring r6.5)     centre (32,32)             ~17    ~17
//!     "Search..." 13px          49           24..40       —      13px
//!   Row i (i = 0..5)            14           74+i*47.5    476    42
//!     highlight (row 0)         same rect                  476    42 r9
//!     icon slot                 24..44       row_y+11..31 20     20
//!     name 13px bold            55           row_y+7      ≤435   13px
//!     comment 10px #9AA0A6      55           row_y+23     ≤435   10px
//!   Column arithmetic (iter16 re-anchor, critic iter15 D2): 14 pad +
//!   36 search + 24 gap + 6*42 rows + 5*5.5 gaps = 353.5, row-rect
//!   bottom pad 12.5 → 366; last comment bottom 344.5, bottom pad 21.5
//!   (video ~18-30). Row-1 name top 74+7 = 81, pitch 47.5 (video
//!   name tops 81/129/177/225/270/318 — replica 81/128.5/176/223.5/
//!   271/318.5, within s06 blur ±5). Highlight = row rect, so it
//!   reads ~75.4-114.7 (AA) vs video 78-114 — top 2.6 px inside blur.
//!   (Was: rows at 58+i*50, last row bottom 350, bottom pad 16,
//!   row-1 name top 65-68 — 6-13 px too high, bottom pad 8.7.)
//!   Iter86 (item a5): the row BLOCK (74..353.5) is invariant — with
//!   1..5 visible rows the rows stretch to tile it (row height =
//!   279.5/n, pitch = row height, text/icon vertically centred), so
//!   the last row's bottom edge always lands at 353.5 and no dead
//!   void sits at the panel bottom. With 6+ matches the geometry
//!   above is byte-identical to pre-iter86.
//! ---------------------------------------------------------------------------
//! COLOURS — video / QS (shell.qml), critic-launcher.md:
//!   PANEL           #000000  (0,0,0)      — video core, QS same
//!   PRIMARY         #FFFFFF                — row names, icons, query
//!   SECONDARY       #9AA0A6  (154,160,166)— placeholder + comments (QS)
//!   HIGHLIGHT       #05163C  (5,22,60)   — selected row (iter96 tint; was QS rowHighlight #1F2124,
//!                                            critic-launcher: PASS)
//!   FIELD_BG        #1A1C1F  (26,28,31)   — QS search-box bg, SUPERSEDED:
//!                                            NOT drawn (video has no box)
//! ---------------------------------------------------------------------------
//! LIVE DATA — exact sources (all TODO until wired; see `LauncherData`):
//!   apps            /usr/share/applications/*.desktop (+ ~/.local/share/
//!                   applications) — name, comment/genericName, Icon,
//!                   NoDisplay filtering; sort case-insensitive by
//!                   name; drawn window capped at 6 AFTER filtering
//!                   by the query — the full filtered list scrolls
//!                   (Up/Down + wheel, iter86).
//!   query           keyboard input once the island has an input path;
//!                   until then "" (placeholder) or a test string via
//!                   draw_launcher_panel_with_data.
//!   selected        keyboard navigation index, default 0 (first row).
//!   launch          `entry.execute()` / detached exec of the .desktop
//!                   Exec line — integrator's job in main.rs, NOT here.
//! ---------------------------------------------------------------------------

use fontdue::Font;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
pub const PANEL_W: u32 = 504;
pub const PANEL_H: u32 = 366;
pub const PANEL_RADIUS: f32 = 20.0;

thread_local! {
    /// r1f (2x): logical -> device scale for the CURRENT draw, set by the
    /// public entry point. Thread-local because the tests draw these
    /// primitives at arbitrary canvas sizes in parallel: it stays 1.0
    /// outside a draw, so w == PANEL_W is byte-identical to the 1x path.
    static SCALE: std::cell::Cell<f32> = const { std::cell::Cell::new(1.0) };
}
fn scale_of(_w: u32) -> f32 {
    SCALE.with(|s| s.get())
}
/// Begin a draw: set the logical -> device scale (see `SCALE`).
fn set_scale(w: u32) {
    SCALE.with(|c| c.set(w as f32 / PANEL_W as f32));
}
pub const PAD: f32 = 14.0; // inner padding (shell.qml innerPadding)
pub const ROW_COUNT: usize = 6;

pub const C_PANEL: (u8, u8, u8) = (0, 0, 0);
pub const C_PRIMARY: (u8, u8, u8) = (255, 255, 255);
pub const C_SECONDARY: (u8, u8, u8) = (154, 160, 166); // #9AA0A6
pub const C_HIGHLIGHT: (u8, u8, u8) = (25, 26, 31); // #191A1F, selected row - r1d: reference selection is neutral dark, not navy
pub const C_SCROLLBAR: (u8, u8, u8) = (64, 66, 70); // #404246, scroll thumb (iter39 a10)
/// r1d: measured reference accent (121,161,245) — the 3 px bar on the
/// selected row's left inner edge.
pub const C_ACCENT: (u8, u8, u8) = (121, 161, 245);
/// r1d: 1 px search-row divider #1F2127 and the 2 px caret before an
/// empty placeholder.
pub const C_DIVIDER: (u8, u8, u8) = (31, 33, 39);
const CARET_W: f32 = 2.0;
const CARET_H: f32 = 14.0;
const SEL_BAR_W: f32 = 3.0;
const SEL_BAR_H: f32 = 24.0;
/// QS search-box background — SUPERSEDED, kept for the record only.
/// The video's search line is flat on black; this colour is never drawn.
#[allow(dead_code)]
pub const C_FIELD_BG: (u8, u8, u8) = (26, 28, 31); // #1A1C1F

const SEARCH_Y: f32 = 14.0;
const SEARCH_H: f32 = 36.0;
const SEARCH_TEXT_X: f32 = 49.0; // magnifier right edge + 8 (QS geometry)
const SEARCH_TEXT_TOP: f32 = 24.0; // 13 px text vertically centred in 36
const MAGNIFIER_CX: f32 = 32.0; // panel x 14 + leftMargin 10 + ~8.5
const MAGNIFIER_CY: f32 = SEARCH_Y + SEARCH_H / 2.0; // 32
const ROW_X: f32 = PAD; // 14
const ROW_W: f32 = PANEL_W as f32 - 2.0 * PAD; // 476
const ROW_Y0: f32 = 74.0; // iter16 re-anchor (critic iter15 D2): search ends 50,
// video row-1 name top is panel-rel ~81, so row-1 top 74 + name off 7
const ROW_H: f32 = 42.0;
const ROW_PITCH: f32 = 47.5; // iter16: 42 + 5.5 gap — video pitch ~47.5-48
// (was 50.0; block sat 6-13 px too high, bottom pad only 8.7)
const ROW_R: f32 = 9.0;
const ICON_X: f32 = 24.0; // row x 14 + leftMargin 10
const ICON_SIZE: f32 = 20.0;
const TEXT_X: f32 = 55.0; // icon right (44) + 11 (QS leftMargin)
const TEXT_MAX_W: f32 = PANEL_W as f32 - PAD - TEXT_X; // 435
const NAME_TOP_OFF: f32 = 7.0; // two-line block (13 + 1 + 10) centred in 42
const COMMENT_TOP_OFF: f32 = 23.0;
const EMPTY_MSG_TOP: f32 = 180.0;
/// Bottom edge of the 6-row block (iter16 geometry): last row top
/// 74 + 5*47.5 + 42 = 353.5, bottom pad 12.5 to the 366 panel edge.
/// Iter86: the row BLOCK always spans ROW_Y0..ROW_BLOCK_BOTTOM —
/// when fewer than ROW_COUNT rows match, the visible rows stretch
/// (taller rows, same block) so no dead void sits at the panel bottom.
const ROW_BLOCK_BOTTOM: f32 = ROW_Y0 + (ROW_COUNT as f32 - 1.0) * ROW_PITCH + ROW_H; // 353.5
const ROW_BLOCK_H: f32 = ROW_BLOCK_BOTTOM - ROW_Y0; // 279.5
/// Height of the two-line name+comment text block: name top 7,
/// comment top 23, comment 10 px -> 23 + 10 - 7 = 26.
const TEXT_BLOCK_H: f32 = COMMENT_TOP_OFF + 10.0 - NAME_TOP_OFF; // 26.0

/// Simple axis-aligned rect, panel-relative logical px.
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
    pub fn cx(&self) -> f32 {
        self.x + self.w / 2.0
    }
    pub fn cy(&self) -> f32 {
        self.y + self.h / 2.0
    }
}

/// Every element rect, computed from the constants above so the spec is
/// auditable in one place. The geometry measurer checks THESE numbers
/// against s06.png and the QS reference screenshot.
#[derive(Clone, Copy, Debug)]
pub struct LauncherLayout {
    pub panel: Rect,
    pub search_line: Rect,
    pub magnifier_centre: (f32, f32),
    pub rows: [Rect; ROW_COUNT],
    pub icon_slots: [Rect; ROW_COUNT], // 20x20, left of each row
    /// Name/comment top offsets within a row: 7/23 at the legacy
    /// 42 px row height; vertically centred when rows stretch
    /// (iter86 — see `launcher_layout_for_count`).
    pub name_top_off: f32,
    pub comment_top_off: f32,
}

/// Row layout for `visible` drawn rows (iter86 stretch rule, item a5).
///
/// * `visible >= ROW_COUNT` — the legacy video-measured geometry,
///   byte-identical to pre-iter86: rows at 74 + i*47.5, 42 px tall,
///   name/comment offsets 7/23, last row bottom 353.5.
/// * `1 <= visible < ROW_COUNT` — the rows TILE the whole row block:
///   each row grows to `ROW_BLOCK_H / visible` tall (pitch = row
///   height, no gaps), so the first row still starts at 74 and the
///   last row's bottom edge still lands at ROW_BLOCK_BOTTOM (353.5)
///   — no dead void at the panel bottom when a query leaves few
///   matches. The icon slot and the 26 px name+comment block stay
///   vertically centred in each (taller) row.
/// * `visible == 0` — no matches: all rects zero, so click
///   hit-testing can never match a row.
/// The icon slot stays 20x20 and the 24x24 real-icon blit stays
/// centred on it (see `draw_app_icon`) — the icon builder's geometry
/// is untouched; only the slot's y follows its row.
pub fn launcher_layout_for_count(visible: usize) -> LauncherLayout {
    let mut rows = [Rect::new(0.0, 0.0, 0.0, 0.0); ROW_COUNT];
    let mut slots = [Rect::new(0.0, 0.0, 0.0, 0.0); ROW_COUNT];
    let (mut name_off, mut comment_off) = (NAME_TOP_OFF, COMMENT_TOP_OFF);
    if visible == 0 {
        // No matches: zero rects (see doc above).
    } else if visible >= ROW_COUNT {
        // Legacy video-measured geometry.
        for (i, (r, s)) in rows.iter_mut().zip(slots.iter_mut()).enumerate() {
            let y = ROW_Y0 + i as f32 * ROW_PITCH;
            *r = Rect::new(ROW_X, y, ROW_W, ROW_H);
            *s = Rect::new(ICON_X, y + (ROW_H - ICON_SIZE) / 2.0, ICON_SIZE, ICON_SIZE);
        }
    } else {
        // 1..5 visible rows: stretch to fill the row block.
        let row_h = ROW_BLOCK_H / visible as f32;
        name_off = (row_h - TEXT_BLOCK_H) / 2.0;
        comment_off = name_off + (COMMENT_TOP_OFF - NAME_TOP_OFF);
        for i in 0..visible {
            let y = ROW_Y0 + i as f32 * row_h;
            rows[i] = Rect::new(ROW_X, y, ROW_W, row_h);
            slots[i] = Rect::new(ICON_X, y + (row_h - ICON_SIZE) / 2.0, ICON_SIZE, ICON_SIZE);
        }
    }
    LauncherLayout {
        panel: Rect::new(0.0, 0.0, PANEL_W as f32, PANEL_H as f32),
        search_line: Rect::new(PAD, SEARCH_Y, ROW_W, SEARCH_H),
        magnifier_centre: (MAGNIFIER_CX, MAGNIFIER_CY),
        rows,
        icon_slots: slots,
        name_top_off: name_off,
        comment_top_off: comment_off,
    }
}

/// Highlight rect for a selected row (iter115, Lane G — the
/// filtered-launcher wash, recap iter114 failure (8)). With 6 rows
/// shown the row rect IS 42 px tall, so the band is the row rect
/// itself (byte-identical behaviour). With a filter leaving 1–5
/// matches the rows stretch to tile the whole block (up to 279.5
/// px for one match) and the old fill washed the entire viewport
/// navy — while the icon slot and the 26 px name+comment block
/// stay vertically CENTRED in the stretched row. The band is
/// therefore the 42 px row height, vertically centred in the
/// stretched rect: it hugs the icon + text and leaves the rest of
/// the stretched row as plain panel background.
pub fn selection_highlight_rect(rect: Rect) -> Rect {
    if rect.h <= ROW_H {
        return rect;
    }
    Rect::new(rect.x, rect.y + (rect.h - ROW_H) / 2.0, rect.w, ROW_H)
}

// ---------------------------------------------------------------------------
// Data — defaults reproduce the VIDEO frame (s06), not the live machine,
// so a screenshot can be diffed against s06.png.
// ---------------------------------------------------------------------------
/// Which drawn glyph a row gets. All are procedural SDF drawings in the
/// 20 px icon slot — see the module header: none of these is the app's
/// real icon file.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AppIcon {
    Gauge,     // auto-cpufreq: speedometer arc + needle
    Bluetooth, // Bluetooth Manager: Material glyph if font given, else rune
    InitialB,  // Brave: "B" in Quicksand bold
    Archive,   // File Roller: box + zipper + pull tab
    Folder,    // Files: tab + rounded body outline
    Brush,     // GIMP: handle + ferrule + bristle
}

#[derive(Clone, Copy, Debug)]
pub struct AppRow {
    pub name: &'static str,
    pub comment: &'static str,
    pub icon: AppIcon,
}

#[derive(Clone, Debug)]
pub struct LauncherData {
    /// Rows in display order (caller sorts; `video_defaults` is sorted).
    pub apps: Vec<AppRow>,
    /// Current search text; "" shows the grey "Search..." placeholder.
    /// Rendered as plain text — never with a caret or a box.
    pub query: String,
    /// Selected row index (keyboard navigation); 0 = first row.
    pub selected: usize,
    /// First visible row: the drawn window is `offset .. offset +
    /// ROW_COUNT` of the filtered list. Follows the selection (see
    /// selection.rs); 0 until the selection scrolls past row 5.
    /// Iter39 a10 (iter38 design step 1).
    pub offset: usize,
}

impl LauncherData {
    /// The video frame s06, exactly: the six apps from the QS static
    /// list (which is the video's list), alphabetical case-insensitive,
    /// no query, first row selected. Names/comments are the .desktop
    /// values the video shows — notably the last row is "GNU Image
    /// Manipulation Program" (its .desktop Name), not "GIMP"; the
    /// icon lookup accepts both spellings (launcher_icons.rs).
    pub fn video_defaults() -> Self {
        Self {
            apps: vec![
                AppRow {
                    name: "auto-cpufreq",
                    comment: "Automatic CPU speed & power optimizer",
                    icon: AppIcon::Gauge,
                },
                AppRow {
                    name: "Bluetooth Manager",
                    comment: "Blueman — manage Bluetooth devices",
                    icon: AppIcon::Bluetooth,
                },
                AppRow {
                    name: "Brave",
                    comment: "Web Browser",
                    icon: AppIcon::InitialB,
                },
                AppRow {
                    name: "File Roller",
                    comment: "Archive Manager",
                    icon: AppIcon::Archive,
                },
                AppRow {
                    name: "Files",
                    comment: "Access and organize files",
                    icon: AppIcon::Folder,
                },
                AppRow {
                    name: "GNU Image Manipulation Program",
                    comment: "Image Editor",
                    icon: AppIcon::Brush,
                },
            ],
            query: String::new(),
            selected: 0,
            offset: 0,
        }
    }

    /// Rows matching the current query (case-insensitive substring on
    /// the name), sorted case-insensitively by name. ALL matches, no
    /// cap: `selected` indexes into this list, and `filtered()` shows
    /// the `offset` window of it. Empty query returns all apps, sorted.
    pub fn filtered_all(&self) -> Vec<&AppRow> {
        let q = self.query.trim().to_lowercase();
        let mut rows: Vec<&AppRow> = self
            .apps
            .iter()
            .filter(|a| q.is_empty() || a.name.to_lowercase().contains(&q))
            .collect();
        rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        rows
    }

    /// Number of matches for the current query.
    pub fn match_count(&self) -> usize {
        self.filtered_all().len()
    }

    /// Largest legal window offset for the current matches.
    pub fn max_offset(&self) -> usize {
        crate::selection::max_offset(self.match_count(), ROW_COUNT)
    }

    /// The visible window: at most ROW_COUNT matches starting at
    /// `offset` (iter38 design step 2 — `skip(offset).take(6)`).
    /// Draw iterates this; `selected` indexes `filtered_all()`.
    pub fn filtered(&self) -> Vec<&AppRow> {
        self.filtered_all()
            .into_iter()
            .skip(self.offset)
            .take(ROW_COUNT)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Drawing primitives — same SDF/Argb8888 style as main.rs and
// controlcenter_panel.rs. Canvas is BGRA bytes, little-endian
// (Format::Argb8888), premultiplied.
// ---------------------------------------------------------------------------

/// Flat frosted fill (iter31, a3 REVISED) — same treatment as main.rs
/// (frost_fill_px_blur): blurred wallpaper under a plain, neutral
/// dark fill, uniform from edge to edge.
/// Fill alpha 0.92 (menu, iter128 — was 0.80, floor was 5.0: the
/// pre-a6a copy the iter86 unification missed). Content drawn later
/// stays opaque.
/// iter103: live path neutralises at capture (shared
/// neutralise_sample) + high-pass term — see main.rs
/// frost_fill_px_blur.
const FROST_ALPHA_MENU: f32 = 0.92;
/// Neutral dark floor mixed under the blurred wallpaper (iter29,
/// see main.rs): FLOOR*fa = 2.3 on a pure-black backdrop (iter128;
/// was 4.0 at 5.0*0.80).
const FROST_FLOOR: f32 = 2.5;

/// Signed distance to a rounded rect (negative inside) — same math as
/// rounded_rect_alpha, which returns (0.5 - d) clamped to [0, 1].
fn rounded_rect_sdf(x: f32, y: f32, w: f32, h: f32, r: f32) -> f32 {
    let cx = x - w / 2.0;
    let cy = y - h / 2.0;
    let qx = cx.abs() - (w / 2.0 - r);
    let qy = cy.abs() - (h / 2.0 - r);
    let ax = qx.max(0.0);
    let ay = qy.max(0.0);
    (ax * ax + ay * ay).sqrt() + qx.max(qy).min(0.0) - r
}


/// Premultiplied BGRA bytes for one flat-fill pixel (see main.rs).
fn frost_fill_px(a: f32, _d: f32, _x: f32, _y: f32, _w: f32, _h: f32, fill_alpha: f32) -> [u8; 4] {
    if a <= 0.0 {
        return [0, 0, 0, 0];
    }
    // iter100: same opaque floor as main.rs frost_fill_px —
    // [2,2,2,255] at full coverage (2.5*0.92, iter128; was
    // [4,4,4,255] at 5.0*0.80), never translucent
    // black (iter85: a translucent fallback lets the backdrop leak
    // through and fake a panel edge).
    let v = (FROST_FLOOR * fill_alpha * a) as u8;
    [v, v, v, (a * 255.0) as u8]
}

/// Flat-frost fill (iter31; iter103: mean and variation decoupled)
/// — blurred wallpaper under the neutral dark floor, same math as
/// main.rs frost_fill_px_blur (duplicated per-module, as the other
/// fill helpers are in this codebase): `bg` is the NEUTRALISED
/// sample, `lm` its BoxBlur(25) local mean, fill is
/// `bg*(1-fa) + FROST_FLOOR*fa + FROST_HP_K*clamp(bg-lm, +/-HP_MAX_DEV)`.
/// Callers with
/// no local mean pass `lm = bg` (deviation 0). iter113: the base
/// clamps at the floor (FROST_FLOOR*fa), not 0. iter128: the
/// deviation is capped at FROST_HP_MAX_DEV (12) — sharp backdrop
/// edges must not print through as bright specks.
fn frost_fill_px_blur(a: f32, _d: f32, _x: f32, _y: f32, _w: f32, _h: f32, fill_alpha: f32, bg: [u8; 3], lm: [u8; 3]) -> [u8; 4] {
    if a <= 0.0 {
        return [0, 0, 0, 0];
    }
    let mut out = [0u8; 4];
    for c in 0..3 {
        let n = bg[c] as f32;
        let dev = (n - lm[c] as f32).clamp(
            -crate::wallpaper_blur::FROST_HP_MAX_DEV,
            crate::wallpaper_blur::FROST_HP_MAX_DEV,
        );
        let base = n * (1.0 - fill_alpha) + FROST_FLOOR * fill_alpha
            + crate::wallpaper_blur::FROST_HP_K * dev;
        // iter113: clamp at the floor, not 0 — no pixel ever reads
        // below the dark-wallpaper value (FROST_FLOOR*fa).
        out[c] = (base.clamp(FROST_FLOOR * fill_alpha, 255.0) * a) as u8;
    }
    out[3] = (a * 255.0) as u8;
    out
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
    let s = scale_of(w);
    let rect = Rect::new(rect.x * s, rect.y * s, rect.w * s, rect.h * s);
    let radius = radius * s;
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
    let s = scale_of(w);
    let rect = Rect::new(rect.x * s, rect.y * s, rect.w * s, rect.h * s);
    let (radius, t) = (radius * s, t * s);
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

fn fill_rect(canvas: &mut [u8], w: u32, _h: u32, x0: f32, y0: f32, x1: f32, y1: f32, rgb: (u8, u8, u8)) {
    let s = scale_of(w);
    let (x0, y0, x1, y1) = (x0 * s, y0 * s, x1 * s, y1 * s);
    for y in (y0.floor() as i32)..(y1.ceil() as i32) {
        for x in (x0.floor() as i32)..(x1.ceil() as i32) {
            blend_pixel(canvas, w, x, y, rgb, 1.0);
        }
    }
}

fn fill_circle(canvas: &mut [u8], w: u32, _h: u32, cx: f32, cy: f32, r: f32, rgb: (u8, u8, u8)) {
    let s = scale_of(w);
    let (cx, cy, r) = (cx * s, cy * s, r * s);
    let x0 = (cx - r - 1.0).floor() as i32;
    let x1 = (cx + r + 1.0).ceil() as i32;
    let y0 = (cy - r - 1.0).floor() as i32;
    let y1 = (cy + r + 1.0).ceil() as i32;
    for y in y0..y1 {
        for x in x0..x1 {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let d = (dx * dx + dy * dy).sqrt() - r;
            blend_pixel(canvas, w, x, y, rgb, (0.5 - d).clamp(0.0, 1.0));
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
    let s = scale_of(w);
    let (cx, cy, r, t) = (cx * s, cy * s, r * s, t * s);
    let x0 = (cx - r - t).floor().max(0.0) as i32;
    let y0 = (cy - r - t).floor().max(0.0) as i32;
    let x1 = (cx + r + t).ceil().min(w as f32) as i32;
    let y1 = (cy + r + t).ceil().min(h as f32) as i32;
    for y in y0..y1 {
        for x in x0..x1 {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let d = (dx * dx + dy * dy).sqrt();
            let a = (t / 2.0 + 0.5 - (d - r).abs()).clamp(0.0, 1.0);
            if a <= 0.0 {
                continue;
            }
            blend_pixel(canvas, w, x, y, rgb, a);
        }
    }
}

/// Ring segment: circle outline of radius `r`, stroke `thick`, with a wedge
/// of half-angle `gap_half` around `gap_angle` (radians, atan2 convention,
/// y down) left open. `gap_half` = PI draws nothing; 0.0 draws a full ring.
/// (Same helper as power_panel.rs.)
fn ring_segment(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    cx: f32,
    cy: f32,
    r: f32,
    thick: f32,
    gap_angle: f32,
    gap_half: f32,
    rgb: (u8, u8, u8),
) {
    let s = scale_of(w);
    let (cx, cy, r, thick) = (cx * s, cy * s, r * s, thick * s);
    let x0 = (cx - r - thick).floor() as i32;
    let x1 = (cx + r + thick).ceil() as i32;
    let y0 = (cy - r - thick).floor() as i32;
    let y1 = (cy + r + thick).ceil() as i32;
    for y in y0..y1 {
        for x in x0..x1 {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            let mut da = (dy.atan2(dx) - gap_angle).abs();
            if da > std::f32::consts::PI {
                da = 2.0 * std::f32::consts::PI - da;
            }
            if da < gap_half {
                continue;
            }
            let sdf = (dist - r).abs() - thick / 2.0;
            blend_pixel(canvas, w, x, y, rgb, (0.5 - sdf).clamp(0.0, 1.0));
        }
    }
}

/// AA line segment from (x1, y1) to (x2, y2), `t` px thick.
fn stroke_segment(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    (x1, y1): (f32, f32),
    (x2, y2): (f32, f32),
    t: f32,
    rgb: (u8, u8, u8),
) {
    let s = scale_of(w);
    let ((x1, y1), (x2, y2), t) = ((x1 * s, y1 * s), (x2 * s, y2 * s), t * s);
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len2 = dx * dx + dy * dy;
    let pad = t + 1.0;
    let x0 = (x1.min(x2) - pad).floor() as i32;
    let x1i = (x1.max(x2) + pad).ceil() as i32;
    let y0 = (y1.min(y2) - pad).floor() as i32;
    let y1i = (y1.max(y2) + pad).ceil() as i32;
    for y in y0..y1i {
        for x in x0..x1i {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let s = if len2 == 0.0 {
                0.0
            } else {
                ((px - x1) * dx + (py - y1) * dy) / len2
            }
            .clamp(0.0, 1.0);
            let qx = x1 + dx * s;
            let qy = y1 + dy * s;
            let d = ((px - qx) * (px - qx) + (py - qy) * (py - qy)).sqrt();
            let a = (t / 2.0 + 0.5 - d).clamp(0.0, 1.0);
            if a <= 0.0 {
                continue;
            }
            blend_pixel(canvas, w, x, y, rgb, a);
        }
    }
}

fn fill_triangle(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    (ax, ay): (f32, f32),
    (bx, by): (f32, f32),
    (cx, cy): (f32, f32),
    rgb: (u8, u8, u8),
) {
    // Edge-function fill, 1px AA on the boundary (same as power_panel.rs).
    let s = scale_of(w);
    let ((ax, ay), (bx, by), (cx, cy)) =
        ((ax * s, ay * s), (bx * s, by * s), (cx * s, cy * s));
    let x0 = ax.min(bx).min(cx).floor() as i32 - 1;
    let x1 = ax.max(bx).max(cx).ceil() as i32 + 1;
    let y0 = ay.min(by).min(cy).floor() as i32 - 1;
    let y1 = ay.max(by).max(cy).ceil() as i32 + 1;
    let edge = |px: f32, py: f32, x1: f32, y1: f32, x2: f32, y2: f32| -> f32 {
        (px - x1) * (y2 - y1) - (py - y1) * (x2 - x1)
    };
    let area = edge(ax, ay, bx, by, cx, cy);
    if area == 0.0 {
        return;
    }
    for y in y0..y1 {
        for x in x0..x1 {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let w0 = edge(px, py, bx, by, cx, cy) / area;
            let w1 = edge(px, py, cx, cy, ax, ay) / area;
            let w2 = edge(px, py, ax, ay, bx, by) / area;
            let inside = w0.min(w1).min(w2);
            if inside >= 0.0 {
                blend_pixel(canvas, w, x, y, rgb, 1.0);
            } else if inside > -0.06 {
                blend_pixel(canvas, w, x, y, rgb, (1.0 + inside / 0.06).clamp(0.0, 1.0));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Text — fontdue, same pattern as main.rs / controlcenter_panel.rs
// (private helpers duplicated here because main.rs is not edited).
// ---------------------------------------------------------------------------
struct TextRow {
    glyphs: Vec<(fontdue::Metrics, std::sync::Arc<[u8]>)>,
    width: f32,
    ascent: f32,
}

fn layout_text(font: &Font, text: &str, px: f32) -> TextRow {
    let mut glyphs = Vec::new();
    let mut width = 0.0f32;
    for ch in text.chars() {
        let (m, cov) = crate::lock_timer::rasterize_cached(font, ch, px);
        width += m.advance_width;
        glyphs.push((m, cov));
    }
    let ascent = font
        .horizontal_line_metrics(px)
        .map(|lm| lm.ascent)
        .unwrap_or(px);
    TextRow {
        glyphs,
        width,
        ascent,
    }
}

fn text_width(font: &Font, text: &str, px: f32) -> f32 {
    text.chars()
        .map(|ch| crate::lock_timer::rasterize_cached(font, ch, px).0.advance_width)
        .sum()
}

/// Truncate `text` to `max_w` px at size `px`, appending "…" when cut.
/// A name/comment that would run past the panel edge is cut, never
/// clipped mid-glyph by the canvas bounds.
fn truncate_to_width(font: &Font, text: &str, px: f32, max_w: f32) -> String {
    if text_width(font, text, px) <= max_w {
        return text.to_string();
    }
    let ellipsis_w = text_width(font, "…", px);
    let mut out = String::new();
    let mut w = 0.0f32;
    let mut cut = false;
    for ch in text.chars() {
        let cw = crate::lock_timer::rasterize_cached(font, ch, px).0.advance_width;
        if w + cw + ellipsis_w > max_w {
            cut = true;
            break;
        }
        out.push(ch);
        w += cw;
    }
    // iter33: cut at a word boundary when one exists ("network an…"
    // read as a broken truncation, iter32 beauty critic §2) — drop
    // the partial last word instead of slicing it mid-word. A
    // single long word with no space keeps the char cut.
    if cut {
        if let Some(sp) = out.rfind(' ') {
            let trimmed = out[..sp].trim_end();
            if !trimmed.is_empty() {
                out = trimmed.to_string();
            }
        }
    }
    out.push('…');
    out
}

/// Draw `text` with its left edge at `x`, top of the line box at `y_top`.
/// `bold` double-blits with a 0.75 px x-offset (fake bold — see header).
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
    let s = scale_of(w);
    let row = layout_text(font, text, px * s);
    let baseline = y_top * s + row.ascent;
    let passes = if bold { 2 } else { 1 };
    for pass in 0..passes {
        let mut pen = x * s + pass as f32 * s * 0.75;
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
    let s = scale_of(w);
    let tw = text_width(font, text, px * s);
    draw_text(canvas, w, h, font, text, px, cx - tw / 2.0 / s, y_top, rgb, bold);
}

/// Draw one Material Symbols glyph so its visual centre lands on (cx, cy).
/// Used ONLY for codepoints already verified on the Surface (see header):
/// `bluetooth` U+E1A7. Any other name falls back to the procedural glyph
/// in `draw_app_icon` — never guess a codepoint.
fn draw_font_glyph(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    icon: &Font,
    ch: char,
    px: f32,
    cx: f32,
    cy: f32,
    rgb: (u8, u8, u8),
) {
    let s = scale_of(w);
    let (m, cov) = crate::lock_timer::rasterize_cached(icon, ch, px * s);
    if m.width == 0 || m.height == 0 {
        return;
    }
    let gx0 = (cx * s - m.advance_width / 2.0 + m.xmin as f32).round() as i32;
    let gy0 = (cy * s - m.height as f32 / 2.0).round() as i32;
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
}

// ---------------------------------------------------------------------------
// Icons — procedural drawn glyphs in the 20 px slot (see header).
// All coordinates are relative to the slot centre (cx, cy); the slot is
// 20x20, so glyphs stay within roughly +/-9 px of the centre.
// ---------------------------------------------------------------------------
/// Search magnifier: ring r 6.5, 2 px stroke, handle to the lower right.
/// NOTE: the Material Symbols `search` codepoint was NOT verified on the
/// Surface, so the magnifier is drawn procedurally even when an icon
/// font is available — do not "fix" this by guessing a codepoint.
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

/// Gauge (auto-cpufreq): 270° arc with the gap at the bottom, needle
/// pointing up-right, centre dot.
fn draw_gauge(canvas: &mut [u8], w: u32, h: u32, cx: f32, cy: f32, rgb: (u8, u8, u8)) {
    // Gap centred on straight down (atan2 +PI/2, y down), half-angle 0.65.
    ring_segment(
        canvas, w, h, cx, cy, 8.5, 2.0,
        std::f32::consts::FRAC_PI_2, 0.65,
        rgb,
    );
    // Needle: centre -> up-right at -0.9 rad, length 6.5.
    let a = -0.9f32;
    stroke_segment(
        canvas, w, h,
        (cx, cy),
        (cx + 6.5 * a.cos(), cy + 6.5 * a.sin()),
        2.0,
        rgb,
    );
    fill_circle(canvas, w, h, cx, cy, 1.8, rgb);
}

/// Bluetooth rune fallback (only when no icon font is passed — the font
/// glyph U+E1A7 is used otherwise). Approximate bind-rune: vertical
/// spine on the left, two chevrons meeting at the centre. It reads as
/// the Bluetooth mark at 20 px; it is NOT the exact trademark shape.
fn draw_bluetooth_rune(canvas: &mut [u8], w: u32, h: u32, cx: f32, cy: f32, rgb: (u8, u8, u8)) {
    let t = 2.0;
    stroke_segment(canvas, w, h, (cx - 6.0, cy - 8.0), (cx - 6.0, cy + 8.0), t, rgb);
    stroke_segment(canvas, w, h, (cx - 6.0, cy - 8.0), (cx + 6.0, cy - 1.0), t, rgb);
    stroke_segment(canvas, w, h, (cx + 6.0, cy - 1.0), (cx - 6.0, cy + 2.0), t, rgb);
    stroke_segment(canvas, w, h, (cx - 6.0, cy + 2.0), (cx + 6.0, cy + 8.0), t, rgb);
    stroke_segment(canvas, w, h, (cx + 6.0, cy + 8.0), (cx - 6.0, cy + 8.0), t, rgb);
}

/// Archive box (File Roller): 16x14 outline, horizontal zipper line,
/// pull tab below the line at centre-right.
fn draw_archive(canvas: &mut [u8], w: u32, h: u32, cx: f32, cy: f32, rgb: (u8, u8, u8)) {
    stroke_rounded_rect(
        canvas, w, h,
        Rect::new(cx - 8.0, cy - 7.0, 16.0, 14.0),
        2.0, 1.75, rgb,
    );
    fill_rect(canvas, w, h, cx - 8.0, cy - 1.0, cx + 8.0, cy + 1.0, rgb);
    fill_rect(canvas, w, h, cx + 1.0, cy + 1.0, cx + 4.0, cy + 5.0, rgb);
}

/// Folder (Files): tab on top-left + rounded body outline.
fn draw_folder(canvas: &mut [u8], w: u32, h: u32, cx: f32, cy: f32, rgb: (u8, u8, u8)) {
    fill_rect(canvas, w, h, cx - 8.0, cy - 8.0, cx - 1.0, cy - 4.0, rgb);
    stroke_rounded_rect(
        canvas, w, h,
        Rect::new(cx - 8.0, cy - 5.0, 16.0, 12.0),
        2.0, 1.75, rgb,
    );
}

/// Paintbrush (GIMP): handle rising to the lower left, ferrule, bristle
/// triangle pointing up-right.
fn draw_brush(canvas: &mut [u8], w: u32, h: u32, cx: f32, cy: f32, rgb: (u8, u8, u8)) {
    stroke_segment(canvas, w, h, (cx - 7.0, cy + 8.0), (cx, cy + 1.0), 2.5, rgb);
    stroke_segment(canvas, w, h, (cx, cy + 1.0), (cx + 3.5, cy - 2.5), 4.0, rgb);
    fill_triangle(
        canvas, w, h,
        (cx + 3.0, cy - 4.5),
        (cx + 9.0, cy - 9.0),
        (cx + 6.5, cy - 1.5),
        rgb,
    );
}

/// Dispatch one app icon into its 20 px slot, centred on (cx, cy).
/// The row's REAL icon from `launcher_icons.rs` is blitted first
/// (24x24, centred); the drawn glyphs below are fallback ONLY for
/// rows with no extracted icon. `ui_font` renders the fallback Brave
/// "B" initial; `icon_font` supplies the verified Bluetooth glyph
/// when present.
fn draw_app_icon(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    ui_font: &Font,
    icon_font: Option<&Font>,
    app: &AppRow,
    cx: f32,
    cy: f32,
) {
    // Real icon first — see launcher_icons.rs for sources/digests.
    if let Some(rgba) = crate::launcher_icons::icon_bytes_for(app.name) {
        let scale = scale_of(w);
        crate::launcher_icons::blit_icon(
            canvas,
            w,
            h,
            ((cx - crate::launcher_icons::ICON_W as f32 / 2.0) * scale).round() as i32,
            ((cy - crate::launcher_icons::ICON_H as f32 / 2.0) * scale).round() as i32,
            scale,
            rgba,
        );
        return;
    }
    match app.icon {
        AppIcon::Gauge => draw_gauge(canvas, w, h, cx, cy, C_PRIMARY),
        AppIcon::Bluetooth => match icon_font {
            // U+E1A7 verified on the Surface (controlcenter_panel.rs).
            Some(f) => draw_font_glyph(canvas, w, h, f, '\u{E1A7}', 20.0, cx, cy, C_PRIMARY),
            None => draw_bluetooth_rune(canvas, w, h, cx, cy, C_PRIMARY),
        },
        AppIcon::InitialB => {
            // 16 px bold initial, centred in the slot.
            draw_text_centered(canvas, w, h, ui_font, "B", 16.0, cx, cy - 8.0, C_PRIMARY, false);
        }
        AppIcon::Archive => draw_archive(canvas, w, h, cx, cy, C_PRIMARY),
        AppIcon::Folder => draw_folder(canvas, w, h, cx, cy, C_PRIMARY),
        AppIcon::Brush => draw_brush(canvas, w, h, cx, cy, C_PRIMARY),
    }
}

// ---------------------------------------------------------------------------
// Panel
// ---------------------------------------------------------------------------
/// Draw the full launcher panel into `canvas` (504x366, Argb8888).
/// `font` is the UI font (Quicksand); `icon_font` is Material Symbols
/// Rounded (None => the Bluetooth row falls back to a procedural rune;
/// every other icon is procedural either way — see header).
/// The caller owns the panel background in the power module's convention,
/// but this function fills the black capsule itself (like
/// controlcenter_panel.rs) so the module is self-contained; a second
/// identical black fill by the caller would be invisible.
pub fn draw_launcher_panel_with_data(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &Font,
    icon_font: Option<&Font>,
    data: &LauncherData,
    // Screen-logical coords of this panel canvas's top-left (fake frost).
    bg_origin: (f32, f32),
) {
    set_scale(w);
    debug_assert!(
        (h as f32 - scale_of(w) * PANEL_H as f32).abs() < 1.5,
        "launcher canvas {w}x{h} is not a scaled {PANEL_W}x{PANEL_H}"
    );
    // Iter86: the row layout follows the DRAWN row count, so a
    // filtered list stretches to fill the row block (see
    // `launcher_layout_for_count`). Computed up front so the empty
    // check below can reuse it.
    let rows = data.filtered();
    let l = launcher_layout_for_count(rows.len());

    // --- Panel background: flat-frost capsule (iter31, a3 REVISED;
    // --- fake frost: blurred wallpaper under the fill). ---
    let bg = crate::wallpaper_blur::ensure_loaded();
    // iter101 (Option 2): live-backdrop frost, frozen at open (see
    // wallpaper_blur.rs). Per-pixel below; the launcher is always a
    // menu, so the live frame applies whenever one exists.
    let live = crate::wallpaper_blur::live_backdrop();
    // iter33: ONE frost sample at the panel centre (see main.rs
    // draw_panel_into) — the panel reads flat, no stain blobs.
    let sc = scale_of(w);
    let uniform_bg: [u8; 3] = match &bg {
        Some(bw) => {
            bw.sample_frost_uniform(
                bg_origin.0 + w as f32 / 2.0 / sc,
                bg_origin.1 + h as f32 / 2.0 / sc,
            )
        }
        None => [0, 0, 0],
    };
    for y in 0..h {
        for x in 0..w {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let d = rounded_rect_sdf(px, py, w as f32, h as f32, PANEL_RADIUS * sc);
            let a = (0.5 - d).clamp(0.0, 1.0);
            let i = ((y * w + x) * 4) as usize;
            let p = match (&live, &bg) {
                (Some(lb), _) => frost_fill_px_blur(
                    a,
                    d,
                    px,
                    py,
                    w as f32,
                    h as f32,
                    // iter103: the live path uses the shared
                    // FROST_ALPHA const (0.80, same value).
                    crate::wallpaper_blur::FROST_ALPHA,
                    // Live sample: NEUTRALISED at capture (iter103
                    // — the frame stored in LiveBackdrop is
                    // neutralise_sample + its BoxBlur(25) local
                    // mean, so B-R lands approx +3, not +27).
                    lb.sample(bg_origin.0 + px / sc, bg_origin.1 + py / sc),
                    lb.local_mean(bg_origin.0 + px / sc, bg_origin.1 + py / sc),
                ),
                (None, Some(_)) => frost_fill_px_blur(
                    a,
                    d,
                    px,
                    py,
                    w as f32,
                    h as f32,
                    FROST_ALPHA_MENU,
                    uniform_bg,
                    uniform_bg,
                ),
                (None, None) => frost_fill_px(a, d, px, py, w as f32, h as f32, FROST_ALPHA_MENU),
            };
            // iter91: p is RGB, canvas is BGRA - swap channels on
            // write (p[2]->B, p[1]->G, p[0]->R). Same R<>B fix as
            // main.rs / controlcenter_panel.rs iter91.
            canvas[i] = p[2]; // B
            canvas[i + 1] = p[1]; // G
            canvas[i + 2] = p[0]; // R
            canvas[i + 3] = p[3]; // A
        }
    }

    // --- Search line: FLAT on black (critic-launcher defect 2). No grey
    // box, no caret (defect 6). Magnifier + placeholder, or the query
    // as plain white text when one is set. ---
    draw_magnifier(canvas, w, h, l.magnifier_centre.0, l.magnifier_centre.1, C_SECONDARY);
    if data.query.trim().is_empty() {
        // r1d: 2 px caret before the empty placeholder (the iter-launcher
        // "no caret" note referred to the grey field box, which stays gone).
        fill_rounded_rect(
            canvas,
            w,
            h,
            Rect::new(SEARCH_TEXT_X - CARET_W - 4.0, SEARCH_TEXT_TOP, CARET_W, CARET_H),
            0.5,
            C_PRIMARY,
        );
        draw_text(
            canvas, w, h, font, "Search...", 13.0,
            SEARCH_TEXT_X, SEARCH_TEXT_TOP, C_SECONDARY, false,
        );
    } else {
        let q = truncate_to_width(font, data.query.trim(), 13.0, TEXT_MAX_W);
        draw_text(
            canvas, w, h, font, &q, 13.0,
            SEARCH_TEXT_X, SEARCH_TEXT_TOP, C_PRIMARY, false,
        );
    }
    // r1d: 1 px divider on the search row's bottom edge.
    fill_rounded_rect(
        canvas,
        w,
        h,
        Rect::new(ROW_X, SEARCH_Y + SEARCH_H, ROW_W, 1.0),
        0.0,
        C_DIVIDER,
    );

    // --- App rows: sorted, drawn window of at most 6 (the full
    // filtered list scrolls via live_move_selection / wheel — iter86);
    // first/selected row highlighted. When fewer than 6 match, the
    // rows are stretched (layout above), so no void sits at the bottom.
    if rows.is_empty() {
        draw_text_centered(
            canvas, w, h, font, "No matching apps", 12.0,
            PANEL_W as f32 / 2.0, EMPTY_MSG_TOP, C_SECONDARY, false,
        );
        return;
    }
    for (i, app) in rows.iter().enumerate() {
        let rect = l.rows[i];
        let slot = l.icon_slots[i];
        // Selected row: `selected` indexes the FULL filtered list;
        // the drawn window starts at `offset`, so the visible slot
        // of the selected row is `selected - offset`.
        if i + data.offset == data.selected {
            let clip = selection_highlight_rect(rect);
            fill_rounded_rect(canvas, w, h, clip, ROW_R, C_HIGHLIGHT);
            // r1d: 3 px accent bar on the highlight's left inner edge.
            fill_rounded_rect(
                canvas,
                w,
                h,
                Rect::new(
                    clip.x + 2.0,
                    clip.y + (clip.h - SEL_BAR_H) / 2.0,
                    SEL_BAR_W,
                    SEL_BAR_H,
                ),
                1.5,
                C_ACCENT,
            );
        }
        draw_app_icon(canvas, w, h, font, icon_font, app, slot.cx(), slot.cy());
        // Name (13 px bold) + comment (10 px secondary), truncated so
        // neither ever runs past the panel's right padding.
        let name = truncate_to_width(font, app.name, 13.0, TEXT_MAX_W);
        draw_text(
            canvas, w, h, font, &name, 13.0,
            TEXT_X, rect.y + l.name_top_off, C_PRIMARY, false,
        );
        if !app.comment.is_empty() {
            let comment = truncate_to_width(font, app.comment, 10.0, TEXT_MAX_W);
            draw_text(
                canvas, w, h, font, &comment, 10.0,
                TEXT_X, rect.y + l.comment_top_off, C_SECONDARY, false,
            );
        }
    }
    // Scrollbar (iter38 design step 5): a thin 4 px thumb in the
    // right padding gutter (x 496..500), only when scrolling is
    // possible. It sits clear of the rows (which end at x 490) so the
    // video-measured row width is untouched; the nav disc crosses it
    // at mid-height and is drawn over it (main.rs draws discs last).
    // `max_offset() > 0` here, so no division by zero.
    if data.match_count() > ROW_COUNT {
        let track_y0 = ROW_Y0;
        let track_y1 = ROW_Y0 + ROW_COUNT as f32 * ROW_PITCH - (ROW_PITCH - ROW_H);
        let track_h = track_y1 - track_y0;
        let max_off = data.max_offset() as f32;
        let thumb_h = (track_h * ROW_COUNT as f32 / data.match_count() as f32).max(20.0);
        let thumb_y = track_y0 + (track_h - thumb_h) * (data.offset as f32 / max_off);
        fill_rounded_rect(
            canvas,
            w,
            h,
            Rect::new(PANEL_W as f32 - 8.0, thumb_y, 4.0, thumb_h),
            2.0,
            C_SCROLLBAR,
        );
    }
}

/// Draw the launcher with the video defaults: the six s06 apps, sorted,
/// no query, first row selected. This is the entry point the integrator
/// wires into `draw()`; use `draw_launcher_panel_with_data` for a live
/// query or a different app set.
pub fn draw_launcher_panel(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &fontdue::Font,
    icon_font: Option<&fontdue::Font>,
    bg_origin: (f32, f32),
) {
    set_scale(w);
    draw_launcher_panel_with_data(
        canvas,
        w,
        h,
        font,
        icon_font,
        &LauncherData::video_defaults(),
        bg_origin,
    );
}

// ---------------------------------------------------------------------------
// Live .desktop apps + interactive launcher state (iter20, pointer /
// keyboard spec). Everything above this section is unchanged: the
// video-default list stays for screenshot diffs. The FUNCTIONAL
// launcher draws `LIVE` instead — real apps scanned once from the
// XDG application directories (so a newly installed app, e.g. the
// Dynamite Settings app with its own .desktop file, appears with no
// extra wiring), filtered live by the typed query.
//
// main.rs drives LIVE from its keyboard/pointer handlers
// (live_push_text / live_backspace / live_reset / live_selected_exec
// / live_row_exec); draw() reads it via live_draw_data. All state is
// behind one Mutex; every caller is on the island's single event-loop
// thread, so the lock is never contended.
// ---------------------------------------------------------------------------

use std::path::PathBuf;

/// One real application from a .desktop file. `name` / `comment` are
/// leaked to 'static at scan time (a few KB, once) so they can fill
/// AppRow's &'static fields without changing the panel's data types;
/// `exec` stays owned and is looked up by name at launch time
/// (exec_for_name) — names are deduplicated case-insensitively at
/// scan time, so the lookup is unambiguous.
pub struct DesktopApp {
    pub name: &'static str,
    pub comment: &'static str,
    pub exec: String,
}

fn leak_str(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// Scan the XDG application directories (user dir first — its
/// entries win name collisions, per the desktop-entry spec). Parses
/// only the [Desktop Entry] group: Name / Comment / Exec, skipping
/// NoDisplay / Hidden entries and non-Application types. Localized
/// Name[xx] lines are ignored; the plain Name= is the display name.
fn scan_desktop_apps() -> Vec<DesktopApp> {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let mut dirs = vec![home.join(".local/share/applications")];
    match std::env::var_os("XDG_DATA_DIRS") {
        Some(x) => {
            for d in std::env::split_paths(&x) {
                dirs.push(d.join("applications"));
            }
        }
        None => {
            dirs.push(PathBuf::from("/usr/local/share/applications"));
            dirs.push(PathBuf::from("/usr/share/applications"));
        }
    }
    let mut out: Vec<DesktopApp> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        let mut paths: Vec<PathBuf> = rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().map(|x| x == "desktop").unwrap_or(false))
            .collect();
        paths.sort();
        for path in paths {
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let mut in_entry = false;
            let mut name: Option<String> = None;
            let mut comment: Option<String> = None;
            let mut exec: Option<String> = None;
            let mut nodisplay = false;
            let mut hidden = false;
            let mut type_ok = true;
            for raw in text.lines() {
                let line = raw.trim();
                if line.starts_with('[') {
                    in_entry = line == "[Desktop Entry]";
                    continue;
                }
                if !in_entry {
                    continue;
                }
                let Some((k, v)) = line.split_once('=') else { continue };
                let (k, v) = (k.trim(), v.trim());
                match k {
                    "Name" => {
                        if name.is_none() {
                            name = Some(v.to_string());
                        }
                    }
                    "Comment" => {
                        if comment.is_none() {
                            comment = Some(v.to_string());
                        }
                    }
                    "Exec" => {
                        if exec.is_none() {
                            exec = Some(v.to_string());
                        }
                    }
                    "NoDisplay" => nodisplay = v.eq_ignore_ascii_case("true"),
                    "Hidden" => hidden = v.eq_ignore_ascii_case("true"),
                    "Type" => type_ok = v == "Application",
                    _ => {}
                }
            }
            if nodisplay || hidden || !type_ok {
                continue;
            }
            let (Some(name), Some(exec)) = (name, exec) else { continue };
            let key = name.to_lowercase();
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            out.push(DesktopApp {
                name: leak_str(name),
                comment: leak_str(comment.unwrap_or_default()),
                exec,
            });
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// The scanned app list, built on first use.
pub fn desktop_apps() -> &'static [DesktopApp] {
    static APPS: std::sync::OnceLock<Vec<DesktopApp>> = std::sync::OnceLock::new();
    APPS.get_or_init(scan_desktop_apps)
}

/// Exec line for an app name (scan-order first match; names are
/// deduplicated, so this is the app's Exec).
pub fn exec_for_name(name: &str) -> Option<&'static str> {
    desktop_apps()
        .iter()
        .find(|a| a.name == name)
        .map(|a| a.exec.as_str())
}

/// Procedural-icon fallback for scanned apps (draw_app_icon tries
/// the real icon by name first; this only covers names with no
/// extracted icon in launcher_icons.rs).
fn app_icon_for(name: &str) -> AppIcon {
    let n = name.to_lowercase();
    if n.contains("bluetooth") {
        AppIcon::Bluetooth
    } else if n.contains("roller") || n.contains("archive") {
        AppIcon::Archive
    } else if n.contains("gimp") || n.contains("image") || n.contains("photo") {
        AppIcon::Brush
    } else if n.contains("cpufreq") || n.contains("cpu") {
        AppIcon::Gauge
    } else if n.contains("brave")
        || n.contains("browser")
        || n.contains("chrom")
        || n.contains("firefox")
    {
        AppIcon::InitialB
    } else {
        AppIcon::Folder
    }
}

/// The interactive launcher data: scanned apps + the live query.
/// Built on first access (that is also when the one-time directory
/// scan happens — a few ms, off the startup path).
static LIVE: std::sync::Mutex<Option<LauncherData>> = std::sync::Mutex::new(None);

fn with_live<R>(f: impl FnOnce(&mut LauncherData) -> R) -> R {
    let mut guard = LIVE.lock().unwrap();
    let data = guard.get_or_insert_with(|| LauncherData {
        apps: desktop_apps()
            .iter()
            .map(|a| AppRow {
                name: a.name,
                comment: a.comment,
                icon: app_icon_for(a.name),
            })
            .collect(),
        query: String::new(),
        selected: 0,
        offset: 0,
    });
    f(data)
}

/// Read the LIVE data (draw path — main.rs's draw() calls
/// draw_launcher_panel_with_data through this).
pub fn live_draw_data<R>(f: impl FnOnce(&LauncherData) -> R) -> R {
    with_live(|d| f(d))
}

/// Number of rows actually drawn right now (visible window size —
/// min(match_count, ROW_COUNT)). main.rs's click hit-test uses this
/// so clicks land on the STRETCHED rows iter86 draws (see
/// `launcher_layout_for_count`); 0 when nothing matches.
pub fn live_visible_count() -> usize {
    with_live(|d| d.match_count().min(ROW_COUNT))
}

/// Fresh search: empty query, selection back to the top result.
/// Called on launcher entry, after a launch, and on Esc.
pub fn live_reset() {
    with_live(|d| {
        d.query.clear();
        d.selected = 0;
        d.offset = 0;
    });
}

/// Append typed text to the query; selection resets to the top.
pub fn live_push_text(s: &str) {
    with_live(|d| {
        d.query.push_str(s);
        d.selected = 0;
        d.offset = 0;
    });
}

/// Delete the last query character (char-safe); selection resets.
pub fn live_backspace() {
    with_live(|d| {
        d.query.pop();
        d.selected = 0;
        d.offset = 0;
    });
}

/// Move the keyboard selection one row (dir -1 up / +1 down),
/// clamped at both ends (no wrap); the window follows so the
/// selected row stays visible. Iter39 a10; shared math in
/// `crate::selection` (same semantics as the Clipboard's
/// `clip_move_selection`). Returns true iff the selection (or its
/// window) actually moved — false at the clamped edge. Iter112:
/// on_wheel uses this to tell a real list scroll from a swallowed
/// notch at the edge, so the wheel can leave the Launcher by
/// cycling menus instead of no-opping (iter110 up2).
pub fn live_move_selection(dir: i32) -> bool {
    with_live(|d| {
        let count = d.match_count();
        let (sel, off) =
            crate::selection::move_by(d.selected, d.offset, dir, count, ROW_COUNT);
        let moved = (sel, off) != (d.selected, d.offset);
        d.selected = sel;
        d.offset = off;
        moved
    })
}

/// Exec line of the currently selected (top) filtered result.
pub fn live_selected_exec() -> Option<String> {
    // Defensive clamp: `selected` is maintained by move/reset, but a
    // list that shrank under it must not make Enter a silent no-op
    // for a valid top row — clamp to the last match instead.
    let name = with_live(|d| {
        let rows = d.filtered_all();
        let idx = d.selected.min(rows.len().saturating_sub(1));
        rows.get(idx).map(|r| r.name)
    })?;
    exec_for_name(name).map(|e| e.to_string())
}

/// Exec line of filtered row `idx` (index into the FULL filtered
/// list — NOT a visible slot; see `live_visible_row_exec`).
pub fn live_row_exec(idx: usize) -> Option<String> {
    let name = with_live(|d| d.filtered_all().get(idx).map(|r| r.name))?;
    exec_for_name(name).map(|e| e.to_string())
}

/// Exec line of VISIBLE row `i` (mouse click on a drawn row):
/// the drawn window starts at `offset`, so visible i is filtered
/// index `offset + i`. The click site in main.rs must call this,
/// not `live_row_exec`, once scrolling exists (see §6.4 K9).
pub fn live_visible_row_exec(visible: usize) -> Option<String> {
    let idx = with_live(|d| d.offset + visible);
    live_row_exec(idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_with_names(names: &[&'static str]) -> LauncherData {
        LauncherData {
            apps: names
                .iter()
                .map(|n| AppRow {
                    name: n,
                    comment: "",
                    icon: AppIcon::Folder,
                })
                .collect(),
            query: String::new(),
            selected: 0,
            offset: 0,
        }
    }

    #[test]
    fn embedded_icon_stays_in_its_row_at_device_scale() {
        let font = Font::from_bytes(
            include_bytes!("../assets/fonts/Quicksand-Bold.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .unwrap();
        let app = data_with_names(&["Keyguard"]).apps[0];
        let slot = launcher_layout_for_count(ROW_COUNT).icon_slots[3];
        for scale in [1, 2] {
            let (w, h) = (PANEL_W * scale, PANEL_H * scale);
            set_scale(w);
            let mut canvas = vec![0; (w * h * 4) as usize];
            draw_app_icon(&mut canvas, w, h, &font, None, &app, slot.cx(), slot.cy());
            let cx = slot.cx() * scale as f32;
            let cy = slot.cy() * scale as f32;
            let mut count = 0;
            for (i, p) in canvas.chunks_exact(4).enumerate().filter(|(_, p)| p[3] != 0) {
                let (x, y) = ((i % w as usize) as f32, (i / w as usize) as f32);
                assert!((x - cx).abs() <= 12.0 * scale as f32 + 0.5, "scale {scale}: icon x={x}");
                assert!((y - cy).abs() <= 12.0 * scale as f32 + 0.5, "scale {scale}: icon y={y}");
                assert_eq!(p[3], 255);
                count += 1;
            }
            let source_pixels = crate::launcher_icons::icon_bytes_for(app.name)
                .unwrap()
                .chunks_exact(4)
                .filter(|p| p[3] != 0)
                .count();
            assert!(source_pixels > 0);
            assert_eq!(count, source_pixels * (scale * scale) as usize);
        }
    }

    #[test]
    fn filtered_windows_by_offset() {
        let mut d = data_with_names(&["a", "b", "c", "d", "e", "f", "g", "h"]);
        assert_eq!(d.match_count(), 8);
        assert_eq!(d.max_offset(), 2);
        assert_eq!(d.filtered().len(), 6);
        assert_eq!(d.filtered()[0].name, "a");
        d.offset = 2;
        assert_eq!(d.filtered().len(), 6);
        assert_eq!(d.filtered()[0].name, "c");
        assert_eq!(d.filtered()[5].name, "h");
    }

    #[test]
    fn filtered_all_is_unwindowed_and_sorted() {
        let d = data_with_names(&["Banana", "apple", "Cherry"]);
        let all = d.filtered_all();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].name, "apple");
        assert_eq!(all[1].name, "Banana");
        assert_eq!(all[2].name, "Cherry");
    }

    #[test]
    fn layout_for_count_six_is_legacy() {
        // 6+ visible rows: byte-identical to the pre-iter86 layout.
        let legacy = launcher_layout_for_count(ROW_COUNT);
        for n in [6, 7, 37] {
            let l = launcher_layout_for_count(n);
            for i in 0..ROW_COUNT {
                assert_eq!(l.rows[i].y, legacy.rows[i].y, "n={n} row {i} y");
                assert_eq!(l.rows[i].h, legacy.rows[i].h, "n={n} row {i} h");
                assert_eq!(l.icon_slots[i].y, legacy.icon_slots[i].y);
            }
            assert_eq!(l.name_top_off, NAME_TOP_OFF);
            assert_eq!(l.comment_top_off, COMMENT_TOP_OFF);
        }
    }

    #[test]
    fn layout_for_count_stretches_to_block_bottom() {
        // 1..5 rows tile the whole row block: first row at 74, last
        // row bottom at 353.5, no gaps, text/icon centred per row.
        for n in 1..ROW_COUNT {
            let l = launcher_layout_for_count(n);
            assert!((l.rows[0].y - ROW_Y0).abs() < 0.001, "n={n}");
            assert!(
                (l.rows[n - 1].y + l.rows[n - 1].h - ROW_BLOCK_BOTTOM).abs() < 0.01,
                "n={n}: last row bottom {} != {ROW_BLOCK_BOTTOM}",
                l.rows[n - 1].y + l.rows[n - 1].h
            );
            for i in 1..n {
                assert!(
                    (l.rows[i].y - (l.rows[i - 1].y + l.rows[i - 1].h)).abs() < 0.01,
                    "n={n}: gap before row {i}"
                );
                assert_eq!(l.rows[i].h, l.rows[0].h, "n={n}: uneven rows");
            }
            // Name+comment block (26 px) vertically centred in the row.
            assert!(
                (l.name_top_off + TEXT_BLOCK_H / 2.0 - l.rows[0].h / 2.0).abs() < 0.01,
                "n={n}: text not centred"
            );
            assert_eq!(l.comment_top_off - l.name_top_off, COMMENT_TOP_OFF - NAME_TOP_OFF);
            // Icon slot centred in its row.
            assert!((l.icon_slots[0].cy() - (l.rows[0].y + l.rows[0].h / 2.0)).abs() < 0.01);
            // Unused slots are zero rects (hit-test never matches).
            for i in n..ROW_COUNT {
                assert_eq!(l.rows[i].h, 0.0, "n={n}: slot {i} not zeroed");
            }
        }
    }

    #[test]
    fn layout_for_count_zero_is_empty() {
        let l = launcher_layout_for_count(0);
        assert!(l.rows.iter().all(|r| r.h == 0.0));
        assert!(l.icon_slots.iter().all(|s| s.h == 0.0));
    }

    #[test]
    fn highlight_band_hugs_content_when_rows_stretch() {
        // iter115 (8): the band is the 42 px row height, centred
        // in the stretched row — never the whole viewport.
        for n in 1..ROW_COUNT {
            let l = launcher_layout_for_count(n);
            let band = selection_highlight_rect(l.rows[0]);
            assert_eq!(band.h, ROW_H, "n={n}: band height");
            assert_eq!(band.x, l.rows[0].x, "n={n}: band x");
            assert_eq!(band.w, l.rows[0].w, "n={n}: band w");
            assert!(
                (band.cy() - (l.rows[0].y + l.rows[0].h / 2.0)).abs() < 0.01,
                "n={n}: band not centred in stretched row"
            );
            // Icon slot and name+comment block sit inside the band.
            assert!(l.icon_slots[0].y >= band.y - 0.01);
            assert!(l.icon_slots[0].y + l.icon_slots[0].h <= band.y + band.h + 0.01);
            assert!(l.rows[0].y + l.name_top_off >= band.y - 0.01);
            assert!(
                l.rows[0].y + l.name_top_off + TEXT_BLOCK_H <= band.y + band.h + 0.01,
                "n={n}: text block escapes the band"
            );
        }
        // 6 rows: band IS the row rect (no behaviour change).
        let l = launcher_layout_for_count(ROW_COUNT);
        for i in 0..ROW_COUNT {
            let band = selection_highlight_rect(l.rows[i]);
            assert_eq!(band.x, l.rows[i].x);
            assert_eq!(band.y, l.rows[i].y);
            assert_eq!(band.w, l.rows[i].w);
            assert_eq!(band.h, l.rows[i].h);
        }
    }

    #[test]
    fn live_list_filters_by_name_substring() {
        // No LIVE mutation: a LOCAL LauncherData built from the real
        // scanned list, so this never races the LIVE-touching test.
        // Surface-only assertions (gated on the 37-app list).
        let apps: Vec<AppRow> = desktop_apps()
            .iter()
            .map(|a| AppRow {
                name: a.name,
                comment: a.comment,
                icon: app_icon_for(a.name),
            })
            .collect();
        if apps.len() != 37 {
            return; // not the Surface list; nothing to assert here
        }
        let mut d = LauncherData {
            apps,
            query: String::new(),
            selected: 0,
            offset: 0,
        };
        assert_eq!(d.filtered_all()[6].name, "Dynamite Settings");
        assert_eq!(d.filtered_all()[36].name, "Zeron");
        d.query = "dynamite".to_string();
        assert_eq!(d.match_count(), 1);
        assert_eq!(d.filtered()[0].name, "Dynamite Settings");
        // One match -> one stretched row filling the whole block.
        let l = launcher_layout_for_count(d.filtered().len());
        assert!((l.rows[0].y + l.rows[0].h - ROW_BLOCK_BOTTOM).abs() < 0.01);
        d.query = "FOOT".to_string(); // case-insensitive
        assert_eq!(d.match_count(), 3); // Foot, Foot Client, Foot Server
        d.query = "xgps".to_string();
        assert_eq!(d.match_count(), 2);
        d.query = "zzz-no-such-app".to_string();
        assert_eq!(d.match_count(), 0);
        assert!(d.filtered().is_empty());
        d.query = String::new();
        assert_eq!(d.match_count(), 37);
    }

    #[test]
    fn query_filters_and_window_resets() {
        let mut d = data_with_names(&["Firefox", "Files", "Foot"]);
        d.query = "f".to_string();
        assert_eq!(d.match_count(), 3);
        d.query = "foot".to_string();
        assert_eq!(d.match_count(), 1);
        assert_eq!(d.filtered()[0].name, "Foot");
    }

    #[test]
    fn live_move_selection_scrolls_to_settings() {
        // Environment-proof: only asserts when the machine actually
        // has more than one app (the Surface has 37; a bare VM may
        // have none). live_reset first — the LIVE static is shared,
        // and this is the ONLY test in this module that touches it
        // (the other tests build LOCAL LauncherData instead).
        live_reset();
        let count = with_live(|d| d.match_count());
        if count < 7 {
            return; // nothing to scroll on this machine; not a failure
        }
        for _ in 0..6 {
            live_move_selection(1);
        }
        let (sel, off) = with_live(|d| (d.selected, d.offset));
        assert_eq!((sel, off), (6, 1));
        // Iter86 (Surface 37-app list only): the LAST app is
        // reachable — 30 more downs land on index 36 with the
        // window at max offset 31, highlight still visible.
        if count == 37 {
            for _ in 0..30 {
                live_move_selection(1);
            }
            let (sel, off) = with_live(|d| (d.selected, d.offset));
            assert_eq!((sel, off), (36, 31));
            // Iter112: bottom edge — down moves nothing there, up moves.
            assert!(!live_move_selection(1));
            assert!(live_move_selection(-1));
            assert_eq!(with_live(|d| (d.selected, d.offset)), (35, 31));
            assert_eq!(with_live(|d| d.filtered_all()[36].name), "Zeron");
            assert_eq!(
                with_live(|d| d.filtered_all()[6].name),
                "Dynamite Settings"
            );
            // Search path: typing filters the full list and resets
            // the selection to the top result.
            live_push_text("dynamite");
            assert_eq!(with_live(|d| d.match_count()), 1);
            assert_eq!(
                with_live(|d| d.filtered()[0].name),
                "Dynamite Settings"
            );
            assert_eq!(live_visible_count(), 1);
            for _ in 0..8 {
                // "dynamite" is 8 chars: clear the query entirely.
                live_backspace();
            }
            assert_eq!(with_live(|d| d.match_count()), 37);
            // Iter112: top edge — after clearing the query the full
            // 37-app list is back with the selection on the top row.
            assert!(!live_move_selection(-1)); // top edge: up moves nothing
            assert!(live_move_selection(1)); // one row down moves
            assert!(live_move_selection(-1)); // ...and back up moves too
            assert!(!live_move_selection(-1)); // top again: up is an edge
        }
        live_reset();
    }
}
