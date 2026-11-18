//! lockscreen — giant-clock lockscreen draw for dynamite-rust.
//!
//! The lockscreen's giant clock + lock pill, drawn into a FULLSCREEN
//! canvas (one per output, logical px — Surface: 1504x1003), NOT into the
//! island pill. This module is draw-only: it renders one locked frame
//! (wallpaper blit + radial scrim + giant clock + lock pill) and the pure
//! morph parameters for the lock-in / unlock-out animations. Everything
//! compositor-level lives OUTSIDE this file — see "COMPOSITOR WORK" below.
//!
//! SOURCES (do not re-derive from memory):
//! - `analysis/lockscreen-scope.md` — the only authority for geometry and
//!   timings. All numbers below are quoted from it; where the doc says a
//!   value is unmeasured, this file says so too instead of inventing one.
//! - `analysis/frame-measurements.md` — island pill geometry (90x24, 15 px
//!   top gap) used as the morph endpoints.
//! - `src/power_panel.rs` / `src/controlcenter_panel.rs` — drawing
//!   conventions copied here: premultiplied Argb8888 canvas (BGRA byte
//!   order), fontdue rasterisation, per-module private helpers duplicated
//!   because main.rs's are private. No fake-bold: main.rs loads the static
//!   `Quicksand-Bold.ttf` as THE text font (v0.4+), so the giant clock
//!   renders in real Bold (iter13: the old double-blit is removed).
//!
//! ---------------------------------------------------------------------------
//! GEOMETRY — all values are @1080p reference unless marked; the layout
//! scales by s = surface_height / 1080 (Surface logical 1003 -> s = 0.928).
//! ---------------------------------------------------------------------------
//!   Giant clock block: 759 x 204 @1080 (39.5 % of width x 18.9 % of
//!     height), TRUE CENTRE on both axes (block centre = screen centre).
//!     Glyphs: Quicksand Bold, pixelSize = 0.27 x surface height
//!     (~292 @1080, ~270 on the Surface). Cap height 204 @1080.
//!     Advance: tabular — digit cells 0.60 em (~175 @1080), colon cell
//!     0.21 em (~60 @1080, measured narrow). 4 x 175 + 60 = 760 ~ 759.
//!   Cyan specular: REAL, glyph-clipped, #AAF9FE (170,249,254) at alpha
//!     0.9 at the block's left edge, fading to transparent by ~42 % of
//!     block width. Digits 3-5 stay pure white. Implemented as a second
//!     pass over the same glyph mask with a horizontal gradient alpha.
//!   Drop shadow: offset (0, +3 x s), blur radius 8 x s, black alpha 0.35
//!     (thumbnail value @720 ~+4 px -> +3 logical on the Surface).
//!   Scrim: the wallpaper itself, sharp, NO blur, darkened by a RADIAL
//!     scrim: black alpha 0.22 at centre -> 0.42 at the corners
//!     (iter13 re-measurement, patch-averaged 9x9 f0620 vs f0571:
//!     corners mean ratio 0.58, centre band 0.78).
//!   Lock pill (inside the lock surface, top-centre): black capsule,
//!     57 x 27 @1080, radius h/2, top margin 42 @1080 (~39 Surface) —
//!     NOTE: ~27 px LOWER than the island's 15 px gap; reproduce it.
//!     Idle: white Material Symbols `lock` glyph (~16 px @1080).
//!     Typing: dots replace the glyph; dot dia 8-10 @1080, pitch 12.0
//!     (iter13: dot centres read off the f0635 y=19 row, 4.0 px @640),
//!     pill width = 57 + (n-1) x 12.0 @1080, height unchanged.
//!     The pill width ramps smoothly (~120 ms per dot) while dots
//!     arrive ~1/1.2 frames, so per-frame widths LAG the dot count —
//!     never derive pitch from width/dots (that gave the old 13.5).
//!     Wrong password: NOT in the video — minimal 150 ms x-shake only,
//!     represented here as a fixed +4*s offset single frame (see below).
//!   DATE LINE: the video frames show NO date line under the giant clock
//!     — only "HH:MM". `LockscreenData::date_line` exists so the caller
//!     CAN pass one, but it defaults to None (not drawn) and its placement
//!     below (clock bottom + 24*s, 18*s px) is INVENTED, not measured.
//!     Do not claim video parity for it.
//!
//! TIMINGS (30 fps frame scan, lockscreen-scope.md):
//!   Lock-in:  scrim 0->1 in ~100 ms, THEN clock scale 0.92->1 + fade
//!     over ~150 ms while the pill morphs collapsed->lock form
//!     (shrink 90->57 wide + drop to the 42 px offset) — ~300 ms total.
//!   Locked hold: giant clock + tiny lock pill; pill widens per dot.
//!   Unlock: clock scales down + flies up into the pill over ~250 ms,
//!     pill morphs back (rise to the 15 px gap, re-widen to 90), scrim
//!     fades out over ~170 ms, THEN the lock is released.
//!   `LockAnim::lock_in()` / `unlock_out()` encode these as pure
//!   functions of elapsed ms so the caller can drive them from a frame
//!   timer. Two values in them are NOT measured and are marked as such:
//!   the unlock clock's vertical travel (120*s placeholder — the video
//!   only shows it "passing the top area", s22) and the exact easing
//!   curve (ease-out cubic assumed; the video is 30 fps, so the curve
//!   shape is under-determined).
//!
//! TEXT WEIGHT: main.rs loads the static `Quicksand-Bold.ttf` as THE text
//! font (v0.4+), so the giant clock renders in real Bold — no fake-bold
//! double-blit (iter13 removed it; the earlier note claiming no Bold file
//! existed on the Surface is stale).
//!
//! ---------------------------------------------------------------------------
//! INTEGRATION (main.rs — do NOT edit main.rs from here):
//! ---------------------------------------------------------------------------
//!   STATUS iter13: the PREVIEW is already wired in LOCAL main.rs (iter10
//!   builder; NOT yet on the Surface — ~/rust-island-build/src has NO
//!   lockscreen.rs and the live binary has no lockscreen strings):
//!   `mod lockscreen;`, `IslandState::Lockscreen` (fullscreen 1504x1002,
//!   radius 0, margin 0), draw() dispatches to `draw_lockscreen` with
//!   the LIVE clock, dots=0, wallpaper=None, anim=locked_hold; socket
//!   words `lockscreen` / `unlock` enter/exit instantly (no morph);
//!   minute timer redraws it. The full wiring patch (dots input,
//!   lock-in/unlock morph timer, wallpaper) is the DELTA in
//!   `iterations/builder-lockscreen-iter13.md` § "main.rs wiring patch".
//!   The preview is a VISUAL HARNESS ONLY — it does NOT lock the
//!   session. NEVER wire the power-menu Lock button, Super+L, or
//!   swayidle to the preview: the screen would LOOK locked while the
//!   session stays open (security hole). Real lock triggers wait for
//!   the ext-session-lock surface in COMPOSITOR WORK (a) below.
//!   The lock-surface owner (see COMPOSITOR WORK) calls, per frame:
//!        lockscreen::draw_lockscreen(
//!            canvas, w, h, font, icon_font.as_ref(),
//!            wallpaper_rgba, &lockscreen::LockscreenData::video_defaults());
//!      - `wallpaper_rgba`: RGBA8 bytes, exactly w*h*4, already
//!        aspect-cropped to the output by the caller (same file swaybg
//!        shows, `~/.config/mango/wallpaper/wallpaper.png`); None =>
//!        opaque-black fallback (lock still works, clock still shows).
//!      - Redraw on: lock request, each typed character (dots), each
//!        minute boundary (clock rollover), and every animation frame
//!        during lock-in / unlock-out (see the frame-timer note below).
//!      - `LockscreenData::video_defaults()` reproduces the held lock
//!        frame f0620 ("00:05", 0 dots) for screenshot diffing.
//!      - NOTE: `video_defaults()` renders the HELD frame (f0620), but
//!        the video's typing actually starts ~f0620 (2 dots visible in
//!        the pill-f620 crop; pill begins growing f0621), not f0625 as
//!        the scope doc's timeline says — see the iter13 builder note.
//!        The held frame is still the right screenshot-diff target
//!        (dots=0 is the pre-typing state at f0607-f0619).
//!
//! ---------------------------------------------------------------------------
//! COMPOSITOR WORK BEYOND THIS FILE — documented precisely, NOT faked:
//! ---------------------------------------------------------------------------
//!   (a) Session-lock surface: the lockscreen needs an
//!       `ext-session-lock-v1` surface (fullscreen, per output) in the
//!       SAME process as the island, so the giant clock, pill and both
//!       morphs are drawn by one renderer (lockscreen-scope.md Q1 proves
//!       the video's lock is drawn by the shell itself, not hyprlock).
//!       What is missing in this crate today: no ext-session-lock
//!       binding at all. Next step is to check whether
//!       smithay-client-toolkit 0.19 (already in Cargo.toml) exposes
//!       `ext-session-lock-v1`; if not, a raw wayland-client protocol
//!       binding + generated code from wayland-protocols is needed.
//!       The wl_shm fullscreen canvas this module draws into is the
//!       same buffer pattern main.rs already uses for the pill.
//!   (b) PAM auth: a PAM binding (e.g. the `pam` crate — NOT yet in
//!       Cargo.toml) with a dedicated `/etc/pam.d/dynamite` entry
//!       falling back to `login`; the password string must be cleared
//!       on unlock AND on failure; on PAM success run the unlock
//!       animation (~250 ms) FIRST, then release the lock. This module
//!       draws dots for a password length it is given — it never reads
//!       a password and never talks to PAM.
//!   (c) Lazy creation / RAM: the lock surface + its fullscreen buffer
//!       (~6.0 MB for 1504x1003 Argb8888) + the decoded wallpaper
//!       (~6 MB at logical size, ~13 MB at 2256x1504) must be created
//!       LAZILY on first lock and kept only while locked, so the idle
//!       RAM delta is zero and the while-locked delta stays well under
//!       the 100 MB budget. The island's own idle PSS (~4 MB) is
//!       unaffected. Do NOT pre-create the lock surface at startup.
//!   (d) Frame timer for the morphs: the island today draws only on
//!       state change (calloop, no frame timer). Lock-in (~300 ms) and
//!       unlock-out (~250 ms) need a ~30 fps calloop timer active ONLY
//!       during the animation, feeding elapsed ms into
//!       `LockAnim::lock_in` / `unlock_out` and redrawing each tick.
//!       Without it the morphs degrade to instant cuts — the same
//!       smoothness defect the island already has for state changes.
//!   (e) Triggers: island power-menu Lock button + a new IPC word
//!       (e.g. `lock` in `IslandState::from_word` / the socket dispatch)
//!       + a Mango keybind (e.g. Super+L) spawning that IPC call +
//!       swayidle (`timeout 300 ... before-sleep ...`, 300 s is a
//!       placeholder — Alexandre has not specified an idle timeout).
//!       Keep swaylock installed and bound to nothing as the fallback
//!       until the crash-while-locked path is tested once on the Surface.
//!   (f) Pre-build check (read-only, on the Surface):
//!       `wayland-info | grep -A2 ext_session_lock` — Mango is
//!       dwl-lineage wlroots, which implements ext-session-lock-v1, but
//!       verify it is actually advertised before writing (a).
//!
//! TYPECHECK NOTE (honest): this file was written in the Muse VM, which
//! has NO cargo/rustc and no vendored fontdue — it could NOT be
//! typechecked here and must NOT be assumed to compile. Every fontdue
//! call (`rasterize`, `horizontal_line_metrics`, `Metrics` fields) is
//! copied verbatim from power_panel.rs / controlcenter_panel.rs, which
//! DO compile on the Surface; the integrator must run
//! `cargo check --offline` on the Surface after adding `mod lockscreen;`
//! and fix any drift. Do NOT build this file on the Surface from this
//! task — wiring is a separate integrator step.

use std::sync::OnceLock;

use fontdue::{Font, FontSettings};

// ---------------------------------------------------------------------------
// Reference geometry — @1080p unless noted. s = h / 1080.
// ---------------------------------------------------------------------------
pub const REF_H: f32 = 1080.0;

/// Giant clock pixelSize as a fraction of surface height (0.27).
pub const CLOCK_PX_FRAC: f32 = 0.27;
/// Digit advance, in em (0.60 em ~ 175 px @1080).
pub const DIGIT_ADVANCE_EM: f32 = 0.60;
/// Colon advance, in em (0.21 em ~ 60 px @1080, measured narrow).
pub const COLON_ADVANCE_EM: f32 = 0.21;
/// Cap-height ratio of Quicksand Bold (~0.70) — used for the layout box.
pub const CAP_HEIGHT_RATIO: f32 = 0.70;

pub const C_WHITE: (u8, u8, u8) = (255, 255, 255);
pub const C_BLACK: (u8, u8, u8) = (0, 0, 0);
/// Cyan specular tint, video-sampled: #AAF9FE.
pub const C_CYAN: (u8, u8, u8) = (170, 249, 254);
pub const CYAN_ALPHA: f32 = 0.9; // at the block's left edge
pub const CYAN_END_FRAC: f32 = 0.42; // transparent by 42 % of block width
pub const SHADOW_ALPHA: f32 = 0.35;
pub const SHADOW_DY_FRAC: f32 = 3.0; // px @1080, times s
pub const SHADOW_BLUR_FRAC: f32 = 8.0; // px @1080, times s
pub const SCRIM_CENTRE: f32 = 0.22; // black alpha at screen centre
pub const SCRIM_EDGE: f32 = 0.42; // black alpha at the corners
// ^^ iter13 re-measurement (patch-averaged 9x9, f0620 vs f0571):
// corners mean ratio 0.58 (alpha 0.42; five points 0.39-0.49), centre
// band 0.78 (alpha 0.22). The scope doc's 0.36 edge was ~0.06 light.
// Top-centre (320,25) reads 0.32 but is pill-contaminated — excluded.
pub const PILL_TOP_1080: f32 = 42.0; // lock pill top margin @1080
pub const PILL_W_1080: f32 = 57.0; // idle lock pill width @1080
pub const PILL_H_1080: f32 = 27.0; // lock pill height @1080 (all states)
pub const DOT_PITCH_1080: f32 = 12.0; // password-dot pitch @1080
// ^^ iter13 re-measurement (was 13.5): dot CENTRES read directly off
// the f0635 y=19 row — bright peaks at x 304,308,312,316,320,324,328,
// 332,336 (@640) = 4.0 px pitch @640 = 12.0 @1080; f0639 confirms
// (301,305,309,313,317,322,326,330,334,338, pitch ~4.1 @640). The old
// 13.5 came from the pill-width arithmetic (145-57)/8, which mixed the
// mid-animation lagging width in as a settled width. The pill width
// itself ramps smoothly (~+1.8 px/frame @640, f0620-f0639) because the
// width animates ~120 ms per dot while dots arrive ~1/1.2 frames —
// per-frame widths are LAGGING indicators, never settled widths, so
// pitch must come from dot centres, not from width/dots. Dot diameter
// stays 9.0 @1080 (bright core ~2-3 px @640 on the y=19 row).
// Settled width(9) = 57 + 8*12 = 153 @1080; the f0635 mid-animation
// true width ~141 @1080 is ~1 dot behind that — consistent.
pub const DOT_D_1080: f32 = 9.0; // password-dot diameter @1080 (8-10 measured)
pub const LOCK_GLYPH_PX_1080: f32 = 16.0; // padlock glyph size @1080
pub const ISLAND_TOP_GAP: f32 = 15.0; // island pill top gap, Surface logical
pub const ISLAND_PILL_W: f32 = 90.0; // island collapsed pill width, logical
pub const DATE_LINE_PX_1080: f32 = 18.0; // date line size @1080 (UNMEASURED)
pub const DATE_LINE_GAP_1080: f32 = 24.0; // gap clock-bottom -> date line
pub const WRONG_SHAKE_DX_1080: f32 = 4.0; // wrong-password offset (NOT in video)

/// Material Symbols Rounded `lock` codepoint — VERIFIED on the Surface
/// (power_panel.rs: E88D; the font also maps E899 to "lock").
pub const ICON_LOCK: char = '\u{E88D}';

/// Scale factor for a surface of height `h` against the 1080p reference.
pub fn scale(h: u32) -> f32 {
    h as f32 / REF_H
}

/// Lock pill width @1080 for `dots` typed characters (idle: 57).
pub fn pill_width_1080(dots: u8) -> f32 {
    if dots == 0 {
        PILL_W_1080
    } else {
        PILL_W_1080 + (dots as f32 - 1.0) * DOT_PITCH_1080
    }
}

// ---------------------------------------------------------------------------
// Morph parameters — pure functions of elapsed ms (see header timings).
// ---------------------------------------------------------------------------
/// One frame of the lock/unlock animation. `pill_top` / `pill_w` are in
/// logical px of the lock surface; `clock_dy` is the clock's vertical
/// offset from screen centre (negative = up).
#[derive(Clone, Copy, Debug)]
pub struct LockAnim {
    pub clock_scale: f32,
    pub clock_opacity: f32,
    pub clock_dy: f32,
    pub scrim: f32, // 0..1 multiplier on the radial scrim alphas
    pub pill_top: f32,
    pub pill_w: f32,
}

fn ease_out_cubic(t: f32) -> f32 {
    // Reuse morph.rs — do not keep a second copy of this curve here.
    // (iter10 builder: the local duplicate was deleted; LockAnim's phase
    // sequencing stays lock-specific, only the easing is shared.)
    crate::morph::Easing::EaseOutCubic.apply(t)
}

impl LockAnim {
    /// The held lock frame (f0607-f0624): everything at rest.
    pub fn locked_hold(h: u32) -> Self {
        let s = scale(h);
        Self {
            clock_scale: 1.0,
            clock_opacity: 1.0,
            clock_dy: 0.0,
            scrim: 1.0,
            pill_top: PILL_TOP_1080 * s,
            pill_w: PILL_W_1080 * s,
        }
    }

    /// Lock-in morph: scrim 0->1 in 100 ms, then clock scale 0.92->1 +
    /// fade over 150 ms while the pill shrinks 90->57 and drops to the
    /// 42 px offset. Total ~300 ms; call once per animation frame with
    /// ms since the lock request.
    pub fn lock_in(elapsed_ms: u32, h: u32) -> Self {
        let s = scale(h);
        let scrim = (elapsed_ms as f32 / 100.0).clamp(0.0, 1.0);
        let e = ease_out_cubic((elapsed_ms as f32 - 100.0) / 150.0);
        Self {
            clock_scale: 0.92 + 0.08 * e,
            clock_opacity: e,
            clock_dy: 0.0,
            scrim,
            // v0.7: ISLAND_TOP_GAP / ISLAND_PILL_W are already
            // Surface-logical (15 / 90) — the morph must START exactly
            // at the island pill, so only the @1080 lock-pill endpoint
            // scales by s. (The old formula multiplied the island
            // endpoint by s too, starting the pill 6.5 px too narrow.)
            pill_top: ISLAND_TOP_GAP + (PILL_TOP_1080 * s - ISLAND_TOP_GAP) * e,
            pill_w: ISLAND_PILL_W + (PILL_W_1080 * s - ISLAND_PILL_W) * e,
        }
    }

    /// Unlock morph (after PAM success): clock scales down + flies up
    /// into the pill over ~250 ms, pill rises to the 15 px gap and
    /// re-widens to 90, scrim fades out over the last ~170 ms.
    /// `clock_dy` travel (120*s) is a PLACEHOLDER — the video only shows
    /// the clock "passing the top area" (s22); verify against s22 before
    /// claiming parity. Release the lock only after this finishes.
    pub fn unlock_out(elapsed_ms: u32, h: u32) -> Self {
        let s = scale(h);
        let e = ease_out_cubic(elapsed_ms as f32 / 250.0);
        let scrim_t = ((elapsed_ms as f32 - 80.0) / 170.0).clamp(0.0, 1.0);
        Self {
            clock_scale: 1.0 - 0.35 * e,
            clock_opacity: 1.0 - e,
            clock_dy: -120.0 * s * e,
            scrim: 1.0 - scrim_t,
            // v0.7: same fix as lock_in — the island endpoint is
            // already Surface-logical; only the @1080 lock endpoint
            // scales (see lock_in).
            pill_top: PILL_TOP_1080 * s + (ISLAND_TOP_GAP - PILL_TOP_1080 * s) * e,
            pill_w: PILL_W_1080 * s + (ISLAND_PILL_W - PILL_W_1080 * s) * e,
        }
    }
}

// ---------------------------------------------------------------------------
// Data
// ---------------------------------------------------------------------------
/// Everything `draw_lockscreen` needs for one frame. The wallpaper is a
/// separate parameter (it is a big buffer, not frame state).
#[derive(Clone, Debug)]
pub struct LockscreenData {
    /// "HH:MM" from the SAME clock source as the island pill.
    pub time_hhmm: String,
    /// Date line under the clock. NOT observed in the video — None means
    /// not drawn (the default). See the header before enabling it.
    pub date_line: Option<String>,
    /// Password characters typed so far: 0 = idle (padlock glyph),
    /// n >= 1 = n dots, pill widened per `pill_width_1080`.
    pub dots: u8,
    /// Wrong password: dots clear, pill shakes. NOT in the video — the
    /// draw is a single fixed-offset frame, not a real shake animation.
    pub wrong_password: bool,
    /// Morph state for this frame; `LockAnim::locked_hold(h)` at rest.
    pub anim: LockAnim,
}

impl LockscreenData {
    /// The held lock frame f0620, exactly: "00:05", 0 dots, at rest.
    /// `h` is the lock surface height (Surface logical: 1003).
    pub fn video_defaults(h: u32) -> Self {
        Self {
            time_hhmm: "00:05".to_string(),
            date_line: None,
            dots: 0,
            wrong_password: false,
            anim: LockAnim::locked_hold(h),
        }
    }
}

/// Simple axis-aligned rect, surface-relative logical px.
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

/// Every element rect for a lock frame, computed from the constants so
/// the spec is auditable in one place. The geometry measurer checks
/// THESE numbers against f0620 (scaled to the Surface).
/// NOTE (iter10): the fields are the auditable spec — they are read by
/// the critics from this file, not by draw_lockscreen (which recomputes
/// from the same constants inline). Hence allow(dead_code).
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct LockLayout {
    pub scale: f32,
    pub clock_px: f32,
    pub clock_block: Rect, // glyph bbox: 759 x 204 @1080, centred
    pub pill: Rect,        // lock pill at rest (idle width)
    pub date_line: Rect,   // text top row; only meaningful if drawn
}

pub fn lockscreen_layout(w: u32, h: u32, dots: u8) -> LockLayout {
    let s = scale(h);
    let px = CLOCK_PX_FRAC * h as f32;
    // 4 digit cells + 1 narrow colon cell.
    let block_w = (4.0 * DIGIT_ADVANCE_EM + COLON_ADVANCE_EM) * px;
    let block_h = CAP_HEIGHT_RATIO * px;
    let pill_w = pill_width_1080(dots) * s;
    LockLayout {
        scale: s,
        clock_px: px,
        clock_block: Rect::new(
            (w as f32 - block_w) / 2.0,
            (h as f32 - block_h) / 2.0,
            block_w,
            block_h,
        ),
        pill: Rect::new(
            (w as f32 - pill_w) / 2.0,
            PILL_TOP_1080 * s,
            pill_w,
            PILL_H_1080 * s,
        ),
        date_line: Rect::new(
            0.0,
            (h as f32 + block_h) / 2.0 + DATE_LINE_GAP_1080 * s,
            w as f32,
            DATE_LINE_PX_1080 * s,
        ),
    }
}

// ---------------------------------------------------------------------------
// Icon font: lazy load, once. None => geometric fallback padlock.
// ---------------------------------------------------------------------------
static ICON_FONT: OnceLock<Option<Font>> = OnceLock::new();

fn icon_font() -> Option<&'static Font> {
    ICON_FONT
        .get_or_init(|| {
            // iter116 (Lane A): embedded 15-glyph subset first (3,904 B,
            // ~64 kB parse); full disk font as fallback. See main.rs.
            if let Ok(f) = Font::from_bytes(
                include_bytes!("icons/MaterialSymbolsRounded-Island.ttf").to_vec(),
                FontSettings::default(),
            ) {
                return Some(f);
            }
            let home = std::env::var_os("HOME").map(std::path::PathBuf::from)?;
            let path = home.join(".local/share/fonts/MaterialSymbolsRounded.ttf");
            let bytes = std::fs::read(path).ok()?;
            Font::from_bytes(bytes, FontSettings::default()).ok()
        })
        .as_ref()
}

// ---------------------------------------------------------------------------
// Pixel helpers — same style as main.rs / power_panel.rs: premultiplied
// Argb8888 canvas, BGRA byte order, src-over onto an opaque destination.
// ---------------------------------------------------------------------------
fn blend_px(canvas: &mut [u8], w: u32, h: u32, x: i32, y: i32, rgb: (u8, u8, u8), cov: f32) {
    if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 || cov <= 0.0 {
        return;
    }
    let c = cov.clamp(0.0, 1.0);
    let i = ((y as u32 * w + x as u32) * 4) as usize;
    canvas[i] = (rgb.2 as f32 * c + canvas[i] as f32 * (1.0 - c)) as u8;
    canvas[i + 1] = (rgb.1 as f32 * c + canvas[i + 1] as f32 * (1.0 - c)) as u8;
    canvas[i + 2] = (rgb.0 as f32 * c + canvas[i + 2] as f32 * (1.0 - c)) as u8;
    canvas[i + 3] = 255;
}

/// SDF for a rounded rect: `x, y` are relative to the rect's top-left,
/// `w, h` its size, `r` its corner radius. Same formula as main.rs.
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

fn fill_rounded_rect(canvas: &mut [u8], w: u32, h: u32, rect: Rect, radius: f32, rgb: (u8, u8, u8)) {
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
            if a > 0.0 {
                blend_px(canvas, w, h, x, y, rgb, a);
            }
        }
    }
}

fn fill_circle(canvas: &mut [u8], w: u32, h: u32, cx: f32, cy: f32, r: f32, rgb: (u8, u8, u8)) {
    let x0 = (cx - r - 1.0).floor() as i32;
    let x1 = (cx + r + 1.0).ceil() as i32;
    let y0 = (cy - r - 1.0).floor() as i32;
    let y1 = (cy + r + 1.0).ceil() as i32;
    for y in y0..y1 {
        for x in x0..x1 {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let d = (dx * dx + dy * dy).sqrt() - r;
            blend_px(canvas, w, h, x, y, rgb, (0.5 - d).clamp(0.0, 1.0));
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
            if a > 0.0 {
                blend_px(canvas, w, h, x, y, rgb, a);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Text helpers (same pattern as power_panel.rs).
// ---------------------------------------------------------------------------
fn text_width(font: &Font, text: &str, px: f32) -> f32 {
    text.chars()
        .map(|ch| font.rasterize(ch, px).0.advance_width)
        .sum()
}

/// Draw `text` with its left edge at `x`, top of the line box at `y_top`.
/// `bold` double-blits with a 0.75 px x-offset (fake bold — only used
/// where the caller explicitly asks; the giant clock does NOT use it,
/// it renders in real Quicksand Bold).
/// `alpha` scales the coverage (1.0 = opaque).
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
    alpha: f32,
) {
    if text.is_empty() || alpha <= 0.0 {
        return;
    }
    let ascent = font
        .horizontal_line_metrics(px)
        .map(|lm| lm.ascent)
        .unwrap_or(px);
    let baseline = y_top + ascent;
    let passes = if bold { 2 } else { 1 };
    for pass in 0..passes {
        let mut pen = x + pass as f32 * 0.75;
        for ch in text.chars() {
            let (m, cov) = font.rasterize(ch, px);
            let gx0 = (pen + m.xmin as f32).round() as i32;
            let gy0 = (baseline - m.ymin as f32 - m.height as f32).round() as i32;
            for gy in 0..m.height as i32 {
                for gx in 0..m.width as i32 {
                    let c = cov[gy as usize * m.width + gx as usize] as f32 / 255.0 * alpha;
                    if c > 0.0 {
                        blend_px(canvas, w, h, gx0 + gx, gy0 + gy, rgb, c);
                    }
                }
            }
            pen += m.advance_width;
        }
    }
}

/// Draw `text` horizontally centred on `cx`, top of the line box at `y_top`.
fn draw_text_centred(
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
    alpha: f32,
) {
    let tw = text_width(font, text, px);
    draw_text(canvas, w, h, font, text, px, cx - tw / 2.0, y_top, rgb, bold, alpha);
}

// ---------------------------------------------------------------------------
// Clock mask: the giant "HH:MM" rasterised once into a float coverage
// mask, glyphs centred in fixed-width tabular cells (digit 0.60 em,
// colon 0.21 em) so the minute rollover never shifts the digits.
// ---------------------------------------------------------------------------
struct Mask {
    w: usize,
    h: usize,
    cov: Vec<f32>,
    /// Bounding box of pixels with coverage > 0.01: (x0, y0, x1, y1).
    bbox: (usize, usize, usize, usize),
}

fn mask_bbox(cov: &[f32], w: usize, h: usize) -> (usize, usize, usize, usize) {
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0usize, 0usize);
    for y in 0..h {
        for x in 0..w {
            if cov[y * w + x] > 0.01 {
                if x < x0 {
                    x0 = x;
                }
                if x > x1 {
                    x1 = x;
                }
                if y < y0 {
                    y0 = y;
                }
                if y > y1 {
                    y1 = y;
                }
            }
        }
    }
    if x1 < x0 {
        // Empty mask (should not happen for "HH:MM"): degenerate 1x1 box.
        return (0, 0, 0, 0);
    }
    (x0, y0, x1, y1)
}

fn rasterize_clock(font: &Font, text: &str, px: f32) -> Mask {
    let lm = font.horizontal_line_metrics(px).unwrap();
    let ascent = lm.ascent;
    let mh = (lm.ascent - lm.descent).ceil().max(1.0) as usize;
    // Total advance with fixed tabular cells.
    let mut mw = 0.0f32;
    for ch in text.chars() {
        mw += if ch == ':' {
            COLON_ADVANCE_EM * px
        } else {
            DIGIT_ADVANCE_EM * px
        };
    }
    let mw = mw.ceil().max(1.0) as usize;
    let mut cov = vec![0.0f32; mw * mh];
    let mut pen = 0.0f32;
    for ch in text.chars() {
        let cell_w = if ch == ':' {
            COLON_ADVANCE_EM * px
        } else {
            DIGIT_ADVANCE_EM * px
        };
        let (m, c) = font.rasterize(ch, px);
        // Centre the glyph's advance box inside its fixed cell.
        let gx = pen + (cell_w - m.advance_width) / 2.0 + m.xmin as f32;
        let gy = ascent - m.ymin as f32 - m.height as f32;
        let x0 = gx.round() as i32;
        let y0 = gy.round() as i32;
        for yy in 0..m.height as i32 {
            for xx in 0..m.width as i32 {
                let (dx, dy) = (x0 + xx, y0 + yy);
                if dx < 0 || dy < 0 || dx >= mw as i32 || dy >= mh as i32 {
                    continue;
                }
                let v = c[yy as usize * m.width + xx as usize] as f32 / 255.0;
                let i = dy as usize * mw + dx as usize;
                cov[i] = (cov[i] + v).min(1.0);
            }
        }
        pen += cell_w;
    }
    let bbox = mask_bbox(&cov, mw, mh);
    Mask { w: mw, h: mh, cov, bbox }
}

/// Separable box blur (edge-clamped sliding window), radius in mask px.
fn box_blur(src: &[f32], w: usize, h: usize, radius: usize) -> Vec<f32> {
    if radius == 0 || w == 0 || h == 0 {
        return src.to_vec();
    }
    let win = (2 * radius + 1) as f32;
    let mut tmp = vec![0.0f32; w * h];
    for y in 0..h {
        let mut acc = 0.0f32;
        for k in 0..=2 * radius {
            let x = (k as isize - radius as isize).clamp(0, w as isize - 1) as usize;
            acc += src[y * w + x];
        }
        tmp[y * w] = acc / win;
        for x in 1..w {
            let x_out = (x as isize - radius as isize - 1).clamp(0, w as isize - 1) as usize;
            let x_in = (x as isize + radius as isize).clamp(0, w as isize - 1) as usize;
            acc += src[y * w + x_in] - src[y * w + x_out];
            tmp[y * w + x] = acc / win;
        }
    }
    let mut out = vec![0.0f32; w * h];
    for x in 0..w {
        let mut acc = 0.0f32;
        for k in 0..=2 * radius {
            let y = (k as isize - radius as isize).clamp(0, h as isize - 1) as usize;
            acc += tmp[y * w + x];
        }
        out[x] = acc / win;
        for y in 1..h {
            let y_out = (y as isize - radius as isize - 1).clamp(0, h as isize - 1) as usize;
            let y_in = (y as isize + radius as isize).clamp(0, h as isize - 1) as usize;
            acc += tmp[y_in * w + x] - tmp[y_out * w + x];
            out[y * w + x] = acc / win;
        }
    }
    out
}

/// Blit a coverage mask onto the canvas, scaled about the mask bbox
/// centre onto screen centre (cx, cy), plus a vertical offset `dy`.
/// `grad` is Some((block_left, block_w)) for the cyan pass: coverage is
/// multiplied by the horizontal gradient (CYAN_ALPHA at the left edge,
/// 0 past CYAN_END_FRAC of the block width). Returns the screen-space
/// bbox of the drawn block (for the date-line anchor).
fn blit_mask(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    mask: &Mask,
    scale_f: f32,
    cx: f32,
    cy: f32,
    dy: f32,
    rgb: (u8, u8, u8),
    alpha: f32,
    grad: Option<(f32, f32)>,
) -> Rect {
    if alpha <= 0.0 {
        return Rect::new(cx, cy, 0.0, 0.0);
    }
    let (bx0, by0, bx1, by1) = mask.bbox;
    let bcx = (bx0 + bx1) as f32 / 2.0;
    let bcy = (by0 + by1) as f32 / 2.0;
    let bw = (bx1 - bx0 + 1) as f32 * scale_f;
    let bh = (by1 - by0 + 1) as f32 * scale_f;
    for my in by0..=by1 {
        for mx in bx0..=bx1 {
            let c = mask.cov[my * mask.w + mx];
            if c <= 0.003 {
                continue;
            }
            let dx = cx + (mx as f32 - bcx) * scale_f;
            let dy2 = cy + (my as f32 - bcy) * scale_f + dy;
            let ix = dx.round() as i32;
            let iy = dy2.round() as i32;
            let mut cov = c * alpha;
            if let Some((block_left, block_w)) = grad {
                let g = 1.0 - (ix as f32 - block_left) / (CYAN_END_FRAC * block_w);
                cov *= CYAN_ALPHA * g.clamp(0.0, 1.0);
            }
            if cov > 0.0 {
                blend_px(canvas, w, h, ix, iy, rgb, cov);
            }
        }
    }
    Rect::new(cx - bw / 2.0, cy - bh / 2.0 + dy, bw, bh)
}

// ---------------------------------------------------------------------------
// Lock pill (idle glyph / typing dots / wrong-password offset).
// ---------------------------------------------------------------------------
fn draw_lock_glyph(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    shared_icon_font: Option<&Font>,
    cx: f32,
    cy: f32,
    px: f32,
) {
    let icon: Option<&Font> = match shared_icon_font {
        Some(f) => Some(f),
        None => icon_font(),
    };
    match icon {
        Some(f) => {
            let (m, cov) = f.rasterize(ICON_LOCK, px);
            if m.width == 0 || m.height == 0 {
                return;
            }
            let gx0 = (cx - m.width as f32 / 2.0).round() as i32;
            let gy0 = (cy - m.height as f32 / 2.0).round() as i32;
            for gy in 0..m.height as i32 {
                for gx in 0..m.width as i32 {
                    let c = cov[gy as usize * m.width + gx as usize] as f32 / 255.0;
                    if c > 0.0 {
                        blend_px(canvas, w, h, gx0 + gx, gy0 + gy, C_WHITE, c);
                    }
                }
            }
        }
        // Geometric fallback padlock (only if the icon font is missing):
        // shackle arc + body + keyhole dot, same footprint as the glyph.
        None => {
            let r = px * 0.32;
            stroke_circle(canvas, w, h, cx, cy - r * 0.45, r, px * 0.14, C_WHITE);
            fill_rounded_rect(
                canvas,
                w,
                h,
                Rect::new(cx - px * 0.36, cy - px * 0.10, px * 0.72, px * 0.52),
                px * 0.08,
                C_WHITE,
            );
            fill_circle(canvas, w, h, cx, cy + px * 0.14, px * 0.06, C_BLACK);
        }
    }
}

fn draw_lock_pill(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    shared_icon_font: Option<&Font>,
    data: &LockscreenData,
    s: f32,
) {
    let anim = data.anim;
    // Typing widens the pill beyond the morph width; wrong-password
    // clears the dots (pill back to idle width, offset single frame).
    let dots = if data.wrong_password { 0 } else { data.dots };
    let pill_w = if dots > 0 {
        (pill_width_1080(dots) * s).max(anim.pill_w)
    } else {
        anim.pill_w
    };
    let pill_h = PILL_H_1080 * s;
    let shake_dx = if data.wrong_password {
        WRONG_SHAKE_DX_1080 * s
    } else {
        0.0
    };
    let pill = Rect::new(
        (w as f32 - pill_w) / 2.0 + shake_dx,
        anim.pill_top,
        pill_w,
        pill_h,
    );
    fill_rounded_rect(canvas, w, h, pill, pill_h / 2.0, C_BLACK);
    if dots == 0 {
        draw_lock_glyph(
            canvas,
            w,
            h,
            shared_icon_font,
            pill.cx(),
            pill.cy(),
            LOCK_GLYPH_PX_1080 * s,
        );
    } else {
        let d = DOT_D_1080 * s;
        let pitch = DOT_PITCH_1080 * s;
        let total = (dots as f32 - 1.0) * pitch;
        let mut x = pill.cx() - total / 2.0;
        for _ in 0..dots {
            fill_circle(canvas, w, h, x, pill.cy(), d / 2.0, C_WHITE);
            x += pitch;
        }
    }
}

// ---------------------------------------------------------------------------
// Public entry point.
// ---------------------------------------------------------------------------
/// Draw one lockscreen frame into `canvas` (fullscreen, w x h logical,
/// Argb8888 BGRA). Order: wallpaper (or black fallback) -> radial scrim
/// -> giant clock (shadow, white, cyan specular) -> lock pill -> date
/// line (only if `data.date_line` is Some — NOT in the video, off by
/// default).
///
/// `font` is Quicksand (same font main.rs loads); `shared_icon_font` is
/// the Material Symbols font main.rs already loaded (None => this module
/// loads it lazily from its own OnceLock — same file — or draws a
/// geometric padlock). `wallpaper` is RGBA8, exactly w*h*4, already
/// aspect-cropped to the output by the caller; pass None and the screen
/// is opaque black behind the scrim instead. A wrong-length slice is
/// ignored (black fallback) rather than panicking.
pub fn draw_lockscreen(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &Font,
    shared_icon_font: Option<&Font>,
    wallpaper: Option<&[u8]>,
    data: &LockscreenData,
) {
    let s = scale(h);
    let anim = data.anim;
    let l = lockscreen_layout(w, h, data.dots);

    // --- (1) Wallpaper, or opaque black if none / wrong size. ---
    match wallpaper {
        Some(wp) if wp.len() == (w * h * 4) as usize => {
            // RGBA -> BGRA blit, opaque.
            for i in 0..(w * h) as usize {
                canvas[i * 4] = wp[i * 4 + 2]; // B
                canvas[i * 4 + 1] = wp[i * 4 + 1]; // G
                canvas[i * 4 + 2] = wp[i * 4]; // R
                canvas[i * 4 + 3] = 255; // A
            }
        }
        _ => {
            for i in 0..(w * h) as usize {
                canvas[i * 4] = 0;
                canvas[i * 4 + 1] = 0;
                canvas[i * 4 + 2] = 0;
                canvas[i * 4 + 3] = 255;
            }
        }
    }

    // --- (2) Radial scrim: black alpha 0.22 centre -> 0.42 corners. ---
    // t = normalised elliptical radius squared, 0 at centre, 1 at corners.
    if anim.scrim > 0.0 {
        let rx = w as f32 / 2.0;
        let ry = h as f32 / 2.0;
        for y in 0..h {
            let dy = (y as f32 + 0.5 - ry) / ry;
            for x in 0..w {
                let dx = (x as f32 + 0.5 - rx) / rx;
                let t = (dx * dx + dy * dy).clamp(0.0, 1.0);
                let a = (SCRIM_CENTRE + (SCRIM_EDGE - SCRIM_CENTRE) * t) * anim.scrim;
                if a > 0.0 {
                    blend_px(canvas, w, h, x as i32, y as i32, C_BLACK, a);
                }
            }
        }
    }

    // --- (3) Giant clock: shadow, white base, cyan specular. ---
    // Real Bold (Quicksand-Bold.ttf is the loaded font) — single white
    // pass, no fake-bold double-blit (iter13).
    let px = CLOCK_PX_FRAC * h as f32;
    let mask = rasterize_clock(font, &data.time_hhmm, px);
    let cx = w as f32 / 2.0;
    let cy = h as f32 / 2.0;
    let (bx0, _, bx1, _) = mask.bbox;
    let block_w = (bx1 - bx0 + 1) as f32 * anim.clock_scale;
    let block_left = cx - block_w / 2.0;
    // Shadow: blurred mask, offset (0, +3*s), black 0.35.
    let blur_r = (SHADOW_BLUR_FRAC * s).round().max(1.0) as usize;
    let shadow_cov = box_blur(&mask.cov, mask.w, mask.h, blur_r);
    let shadow_mask = Mask {
        w: mask.w,
        h: mask.h,
        bbox: mask_bbox(&shadow_cov, mask.w, mask.h),
        cov: shadow_cov,
    };
    blit_mask(
        canvas, w, h, &shadow_mask, anim.clock_scale, cx, cy,
        anim.clock_dy + SHADOW_DY_FRAC * s, C_BLACK, SHADOW_ALPHA * anim.clock_opacity, None,
    );
    // White base (single pass — real Bold).
    blit_mask(
        canvas, w, h, &mask, anim.clock_scale, cx, cy,
        anim.clock_dy, C_WHITE, anim.clock_opacity, None,
    );
    // Cyan specular, glyph-clipped, gradient to transparent at 42 %.
    let clock_block = blit_mask(
        canvas, w, h, &mask, anim.clock_scale, cx, cy,
        anim.clock_dy, C_CYAN, anim.clock_opacity, Some((block_left, block_w)),
    );

    // --- (4) Lock pill (idle glyph / typing dots / wrong offset). ---
    draw_lock_pill(canvas, w, h, shared_icon_font, data, s);

    // --- (5) Date line — NOT in the video; drawn only if requested. ---
    if let Some(date) = &data.date_line {
        draw_text_centred(
            canvas, w, h, font, date, DATE_LINE_PX_1080 * s,
            cx, clock_block.y + clock_block.h + DATE_LINE_GAP_1080 * s,
            C_WHITE, false, 0.8,
        );
    }

    // Silence unused-layout warnings in builds that only call this fn:
    // `l` documents the auditable layout (see lockscreen_layout).
    let _ = l;
}

// ---------------------------------------------------------------------------
// Unit tests — LockAnim phase math only (no font, no canvas). Run on the
// Surface scratch copy: `cargo test --offline lockscreen`.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    const H: u32 = 1002; // Surface logical height (mmsg eDP-1, 12 Nov 2026)

    #[test]
    fn lock_in_endpoints() {
        // Tolerance 1e-4, not 1e-6: the pill formulas combine 42*s and
        // 27*s in f32, so the settled value differs from the direct
        // 15*s / 42*s by ~1e-6 of rounding at these magnitudes.
        // v0.7: at t=0 the pill is EXACTLY the island pill (90 wide,
        // top gap 15, both already Surface-logical — NOT scaled by s).
        let a0 = LockAnim::lock_in(0, H);
        assert_eq!(a0.scrim, 0.0);
        assert_eq!(a0.clock_opacity, 0.0);
        assert!((a0.clock_scale - 0.92).abs() < 1e-4);
        assert!((a0.pill_w - ISLAND_PILL_W).abs() < 1e-4);
        assert!((a0.pill_top - ISLAND_TOP_GAP).abs() < 1e-4);
        // Scrim is done at 100 ms; clock still mid-fade.
        let a100 = LockAnim::lock_in(100, H);
        assert_eq!(a100.scrim, 1.0);
        assert_eq!(a100.clock_opacity, 0.0);
        // Settled at 250 ms (100 scrim + 150 clock): the held frame.
        let a250 = LockAnim::lock_in(250, H);
        let hold = LockAnim::locked_hold(H);
        assert!((a250.clock_scale - hold.clock_scale).abs() < 1e-4);
        assert!((a250.clock_opacity - hold.clock_opacity).abs() < 1e-4);
        assert!((a250.pill_w - hold.pill_w).abs() < 1e-4);
        assert!((a250.pill_top - hold.pill_top).abs() < 1e-4);
        assert_eq!(a250.scrim, 1.0);
    }

    #[test]
    fn unlock_out_endpoints() {
        // At t=0 the unlock morph is exactly the held frame.
        let u0 = LockAnim::unlock_out(0, H);
        let hold = LockAnim::locked_hold(H);
        assert!((u0.clock_scale - hold.clock_scale).abs() < 1e-4);
        assert_eq!(u0.clock_opacity, 1.0);
        assert_eq!(u0.scrim, 1.0);
        assert!((u0.pill_w - hold.pill_w).abs() < 1e-4);
        // At 250 ms the clock is gone and the pill is back to island form.
        let u250 = LockAnim::unlock_out(250, H);
        assert_eq!(u250.clock_opacity, 0.0);
        assert!((u250.clock_scale - 0.65).abs() < 1e-4);
        // v0.7: the island endpoint is the literal island pill (90 /
        // 15), not the s-scaled value (83.5 / 13.9) the old formula
        // produced — the morph must land exactly on the pill it becomes.
        assert!((u250.pill_w - ISLAND_PILL_W).abs() < 1e-4);
        assert!((u250.pill_top - ISLAND_TOP_GAP).abs() < 1e-4);
        assert_eq!(u250.scrim, 0.0);
    }

    #[test]
    fn lock_morph_pill_is_monotonic() {
        // Lock-in: the pill shrinks 90 -> ~53 and drops 15 -> ~39,
        // monotonically (no overshoot past either endpoint).
        let mut prev_w = f32::MAX;
        let mut prev_top = f32::MIN;
        for ms in (0..=250).step_by(10) {
            let a = LockAnim::lock_in(ms, H);
            assert!(a.pill_w <= prev_w + 1e-4, "pill widened at {ms} ms");
            assert!(a.pill_top >= prev_top - 1e-4, "pill rose at {ms} ms");
            prev_w = a.pill_w;
            prev_top = a.pill_top;
        }
        // Unlock-out is the reverse: widening 53 -> 90, rising 39 -> 15.
        let mut prev_w = f32::MIN;
        let mut prev_top = f32::MAX;
        for ms in (0..=250).step_by(10) {
            let a = LockAnim::unlock_out(ms, H);
            assert!(a.pill_w >= prev_w - 1e-4, "pill narrowed at {ms} ms");
            assert!(a.pill_top <= prev_top + 1e-4, "pill dropped at {ms} ms");
            prev_w = a.pill_w;
            prev_top = a.pill_top;
        }
    }

    #[test]
    fn pill_width_grows_per_dot() {
        assert_eq!(pill_width_1080(0), 57.0);
        assert_eq!(pill_width_1080(1), 57.0);
        assert!((pill_width_1080(9) - 153.0).abs() < 1e-6); // 57 + 8*12.0
        // Video f0635: 9 dots, core 44 px @640 -> true ~47 @640 ->
        // ~141 @1080. That width is MID-ANIMATION (the pill ramps
        // ~120 ms per dot while dots arrive ~1/1.2 frames, so it lags
        // ~1 dot behind the count); the settled width is 153 @1080.
        // Dot pitch 12.0 comes from the dot CENTRES (f0635 y=19 row:
        // peaks at x 304,308,...,336 @640), not from this width.
        let l = lockscreen_layout(1504, H, 9);
        assert!(l.pill.w > 138.0 && l.pill.w < 146.0); // 153 * 1002/1080
    }

    #[test]
    fn clock_block_is_centred() {
        let l = lockscreen_layout(1504, H, 0);
        assert!((l.clock_block.cx() - 752.0).abs() < 1.0);
        assert!((l.clock_block.cy() - H as f32 / 2.0).abs() < 1.0);
        // 759 x 204 @1080 -> ~705 x 189 on the Surface.
        assert!(l.clock_block.w > 690.0 && l.clock_block.w < 720.0);
        assert!(l.clock_block.h > 180.0 && l.clock_block.h < 200.0);
    }
}
