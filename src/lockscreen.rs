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
//!   Giant clock block: 868 x 279 ink at 2560x1440 (ink/W .33906,
//!     cap/H .19375), TRUE CENTRE on both axes. Glyphs: Quicksand Bold at
//!     pixelSize = 0.27 x surface height: cap/H matches the reference
//!     within 0.6 %, so CLOCK_PX_FRAC stays. Advance: the font's own
//!     PROPORTIONAL advances ('1' .389 em, '2' .569, ':' .273), NOT tabular
//!     cells — the reference's digit pitches are 160 px for 1->1 and 207 px
//!     for 2->8, which a tabular layout cannot produce.
//!   Clock material (no white base, no cyan specular: both were the OLD
//!     design and are deleted): the glyphs are wallpaper-sampled frosting.
//!     The registered (1.030x) wallpaper is blurred at radius
//!     0.016667 x H and pushed through a per-channel affine fit, then
//!     composited INSIDE the glyph coverage over the veiled backdrop.
//!     The ink therefore carries the wallpaper's own hue and varies with
//!     it (reference ink median [87,235,206], 92.8 % teal-ish).
//!   Drop shadow: offset (0, +3 x s), blur radius 8 x s, black alpha 0.35
//!     (thumbnail value @720 ~+4 px -> +3 logical on the Surface).
//!   Veil: the wallpaper itself (zoomed 1.030x), SHARP (best-fit global
//!     blur is radius 0.6 device px, i.e. compression, not UI), darkened
//!     by a VERTICAL black veil: alpha .430 at the top edge, .224 flat
//!     over the middle, .507 at the bottom edge. See `veil_alpha`.
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
/// Cap-height ratio of Quicksand Bold (~0.70) — used for the layout box.
pub const CAP_HEIGHT_RATIO: f32 = 0.70;

pub const C_WHITE: (u8, u8, u8) = (255, 255, 255);
pub const C_BLACK: (u8, u8, u8) = (0, 0, 0);
pub const SHADOW_ALPHA: f32 = 0.35;
pub const SHADOW_DY_FRAC: f32 = 3.0; // px @1080, times s
pub const SHADOW_BLUR_FRAC: f32 = 8.0; // px @1080, times s
// ---------------------------------------------------------------------------
// Locked-state materials: docs/lockscreen-spec.md §3 (measured from the
// reference frames, re-checked by the parent; do NOT re-derive by eye).
// ---------------------------------------------------------------------------
/// The locked wallpaper is the desktop wallpaper zoomed 1.030x about
/// screen centre (verified against pole/support edges over a 1.000-1.070
/// scan). Applied to BOTH the veiled backdrop and the clock material.
pub const LOCK_ZOOM: f32 = 1.030;

/// Veil: black, **vertical only**. A radial vignette cannot reproduce the
/// reference, which transmits the same amount at the left and right edges
/// of any given row. Fitted against nine measured patch alphas (mean abs
/// error 0.0037, max 0.0105): plateau alpha .224 over y .278-.750 H, .430
/// at the top edge, .507 at the bottom edge. Normalised to screen height,
/// so the profile is resolution-independent.
pub const VEIL_TOP_STOP: f32 = 0.27778; // top ramp ends (400/1440 H)
pub const VEIL_BOT_SPLIT: f32 = 0.75; // bottom ramp starts
pub const VEIL_TRANSMIT_PLATEAU: f32 = 0.77635;
pub const VEIL_TOP_DROP: f32 = 0.20660;
pub const VEIL_BOT_DROP: f32 = 0.28306;

/// Clock material: the wallpaper, blurred, seen through the glyphs. The
/// blur is substantial (radius 0.016667 H = 24 px at 1440; fitted gradient
/// residual -.11 %, RGB RMSE 2.39/255) but confined to the glyph interiors
/// only: the backdrop itself stays sharp.
pub const CLOCK_MAT_BLUR_FRAC: f32 = 0.016667;
/// Per-channel affine fitted against 114,924 glyph-interior pixels:
/// (gain, offset) on the blurred wallpaper. This keeps the wallpaper's own
/// hue; it is NOT a palette entry (the teal #57EBCE is that wallpaper's
/// colour, not a brand token).
pub const CLOCK_MAT_R: (f32, f32) = (1.19396, 83.78911);
pub const CLOCK_MAT_G: (f32, f32) = (1.25239, 83.09509);
pub const CLOCK_MAT_B: (f32, f32) = (1.25567, 76.69171);

/// Black alpha of the veil at row `y` of a screen `h` px tall.
pub fn veil_alpha(y: f32, h: f32) -> f32 {
    1.0 - veil_transmission(y, h)
}

pub fn veil_transmission(y: f32, h: f32) -> f32 {
    let top = ((VEIL_TOP_STOP * h - y) / (VEIL_TOP_STOP * h)).clamp(0.0, 1.0);
    let bot = ((y - VEIL_BOT_SPLIT * h) / ((1.0 - VEIL_BOT_SPLIT) * h)).clamp(0.0, 1.0);
    VEIL_TRANSMIT_PLATEAU - VEIL_TOP_DROP * top - VEIL_BOT_DROP * bot
}

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

pub fn lockscreen_layout(w: u32, h: u32, dots: u8, font: &Font, time: &str) -> LockLayout {
    let s = scale(h);
    let px = CLOCK_PX_FRAC * h as f32;
    // Same proportional advances the rasteriser uses, so the audited block
    // and the drawn block agree. Depends on the time string: "11:11" is
    // narrower than "00:08", exactly as in the reference.
    let block_w: f32 = time.chars().map(|c| font.rasterize(c, px).0.advance_width).sum();
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
    // Degrade to the em box instead of panicking (r1c finding 9).
    let (ascent, mh) = match font.horizontal_line_metrics(px) {
        Some(lm) => (lm.ascent, (lm.ascent - lm.descent).ceil().max(1.0) as usize),
        None => (px, px.ceil().max(1.0) as usize),
    };
    // PROPORTIONAL advances, not tabular cells. Measured on the reference
    // '11:28' frame: glyph starts 844/1004/1173/1277/1484 -> pitches
    // 160/169/104/207 px. A tabular layout would make every digit->digit
    // pitch equal; 1->1 is 160 while 2->8 is 207, so the reference uses the
    // font's own advances. Quicksand-Bold hmtx: '1' .389 em, ':' .273,
    // '2' .569 — matching the measured .4014/.2609/.5194 within run-edge
    // error. Natural total for '11:28' = 2.1680 em vs the reference ink
    // span 2.1853 em (ratio 1.008). Forcing 0.60 em per digit gave 2.6100 em
    // = 1.204x too wide (live ink 1073 px vs the reference's 871 at the same
    // cap height). The small pill clock stays tabular on purpose: it is
    // verified at w/cap 3.600 and must not shift as digits change.
    let mut mw = 0.0f32;
    for ch in text.chars() {
        mw += font.rasterize(ch, px).0.advance_width;
    }
    let mw = mw.ceil().max(1.0) as usize;
    let mut cov = vec![0.0f32; mw * mh];
    let mut pen = 0.0f32;
    for ch in text.chars() {
        let (m, c) = font.rasterize(ch, px);
        let cell_w = m.advance_width;
        let gx = pen + m.xmin as f32;
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
/// Returns the screen-space bbox of the drawn block (for the date-line
/// anchor).
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
            let cov = c * alpha;
            if cov > 0.0 {
                blend_px(canvas, w, h, ix, iy, rgb, cov);
            }
        }
    }
    Rect::new(cx - bw / 2.0, cy - bh / 2.0 + dy, bw, bh)
}

/// The fitted per-channel affine, applied to one blurred wallpaper pixel.
///
/// The coefficients were fitted where the reference clock sits over a DARK
/// region (blurred ~(3,121,103) -> ink (87,235,206)). Applied to a bright
/// wallpaper they overflow all three channels and the ink turns pure white
/// (measured 89.7% of the owner's sky-backdropped clock), which loses the
/// hue the material exists for. So an overflow is scaled DOWN uniformly
/// instead of being clipped per channel: identical output everywhere the
/// fit was made (nothing there reaches 255), hue preserved where it would
/// have blown out.
fn clock_material_px(b: [u8; 3]) -> [f32; 3] {
    let mut m = [
        CLOCK_MAT_R.0 * b[0] as f32 + CLOCK_MAT_R.1,
        CLOCK_MAT_G.0 * b[1] as f32 + CLOCK_MAT_G.1,
        CLOCK_MAT_B.0 * b[2] as f32 + CLOCK_MAT_B.1,
    ];
    let hi = m[0].max(m[1]).max(m[2]);
    if hi > 255.0 {
        let k = 255.0 / hi;
        for v in m.iter_mut() {
            *v *= k;
        }
    }
    for v in m.iter_mut() {
        *v = v.clamp(0.0, 255.0);
    }
    m
}

/// Composite the glyph material (wallpaper-sampled frosting) through the
/// clock coverage, over the already-veiled backdrop: every covered pixel
/// interpolates `backdrop -> material` by the glyph coverage. The material
/// is sampled at the DESTINATION pixel, so a morph frame slides the
/// frosting with the glyphs rather than stretching it.
/// `mat` is RGB u8, w*h*3, registered to canvas pixels; None (no usable
/// wallpaper) falls back to opaque white ink on the black backdrop.
fn blit_glyph_material(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    mask: &Mask,
    scale_f: f32,
    cx: f32,
    cy: f32,
    dy: f32,
    mat: Option<&[u8]>,
    alpha: f32,
) -> Rect {
    let (bx0, by0, bx1, by1) = mask.bbox;
    let bcx = (bx0 + bx1) as f32 / 2.0;
    let bcy = (by0 + by1) as f32 / 2.0;
    let bw = (bx1 - bx0 + 1) as f32 * scale_f;
    let bh = (by1 - by0 + 1) as f32 * scale_f;
    if alpha > 0.0 {
        for my in by0..=by1 {
            for mx in bx0..=bx1 {
                let c = mask.cov[my * mask.w + mx] * alpha;
                if c <= 0.003 {
                    continue;
                }
                let ix = (cx + (mx as f32 - bcx) * scale_f).round() as i32;
                let iy = (cy + (my as f32 - bcy) * scale_f + dy).round() as i32;
                if ix < 0 || iy < 0 || ix >= w as i32 || iy >= h as i32 {
                    continue;
                }
                let i = ((iy as u32 * w + ix as u32) * 4) as usize;
                let cc = c.min(1.0);
                let keep = 1.0 - cc;
                // `mat` and clock_material_px are RGB; the canvas is BGRA.
                let m = match mat {
                    Some(m) => {
                        let bi = ((iy as u32 * w + ix as u32) * 3) as usize;
                        clock_material_px([m[bi], m[bi + 1], m[bi + 2]])
                    }
                    None => [C_WHITE.0 as f32, C_WHITE.1 as f32, C_WHITE.2 as f32],
                };
                canvas[i] = (m[2] * cc + canvas[i] as f32 * keep) as u8;
                canvas[i + 1] = (m[1] * cc + canvas[i + 1] as f32 * keep) as u8;
                canvas[i + 2] = (m[0] * cc + canvas[i + 2] as f32 * keep) as u8;
                canvas[i + 3] = 255;
            }
        }
    }
    Rect::new(cx - bw / 2.0, cy - bh / 2.0 + dy, bw, bh)
}

// ---------------------------------------------------------------------------
// Registered wallpaper + clock material.
// ---------------------------------------------------------------------------
/// Bilinear sample of `src` (RGBA8, sw x sh) at fractional source coords.
/// Edges clamp. Returns [r, g, b].
fn sample_rgba(src: &[u8], sw: u32, sh: u32, fx: f32, fy: f32) -> [f32; 3] {
    let x = fx.clamp(0.0, sw as f32 - 1.0);
    let y = fy.clamp(0.0, sh as f32 - 1.0);
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(sw - 1);
    let y1 = (y0 + 1).min(sh - 1);
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;
    let at = |px: u32, py: u32| -> [f32; 3] {
        let i = ((py * sw + px) * 4) as usize;
        [src[i] as f32, src[i + 1] as f32, src[i + 2] as f32]
    };
    let a = at(x0, y0);
    let b = at(x1, y0);
    let c = at(x0, y1);
    let d = at(x1, y1);
    let mut out = [0.0f32; 3];
    for k in 0..3 {
        let top = a[k] + (b[k] - a[k]) * tx;
        let bot = c[k] + (d[k] - c[k]) * tx;
        out[k] = top + (bot - top) * ty;
    }
    out
}

/// Three box passes ~ one Gaussian of `sigma` (central limit).
/// Pass radius r has variance ((2r+1)^2 - 1)/12, so n passes of radius r
/// give sigma = sqrt(n*((2r+1)^2 - 1)/12).
fn box_radius_for_sigma(sigma: f32) -> usize {
    const PASSES: f32 = 3.0;
    let v = sigma * sigma * 12.0 / PASSES + 1.0;
    let r = (v.sqrt() - 1.0) / 2.0;
    r.round().max(1.0) as usize
}

/// Cached lock materials: the REGISTERED wallpaper (1.030x about centre)
/// and a blurred copy of it. Both are fixed for as long as the lock screen
/// is up, so they are built on the first frame and reused: measured 252 ms
/// cold and 99 ms warm before this cache, of which registration was 79 ms
/// EVERY frame. Keyed on size, blur radius and a cheap wallpaper signature,
/// so a different wallpaper still rebuilds. Cleared on the first frame with
/// no wallpaper, which is how the ~9 MB is released on unlock.
struct MatCache {
    w: u32,
    h: u32,
    radius: usize,
    sig: u64,
    reg: Vec<u8>,  // w*h*3, registered wallpaper, pre-veil
    blur: Vec<u8>, // w*h*3, blurred registered wallpaper, pre-affine
}

static MAT_CACHE: std::sync::Mutex<Option<MatCache>> = std::sync::Mutex::new(None);

/// 64-sample FNV over the wallpaper: enough to notice a different image
/// without hashing 6 MB. The caller re-reads the file on every lock entry,
/// so the slice pointer alone would not be a reliable key.
fn wallpaper_sig(wp: &[u8]) -> u64 {
    let n = wp.len() / 4;
    if n == 0 {
        return 0;
    }
    let step = (n / 64).max(1);
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut i = 0;
    while i < n {
        for k in 0..3 {
            h = (h ^ wp[i * 4 + k] as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
        i += step;
    }
    h
}

fn build_registered(w: u32, h: u32, wp: &[u8]) -> Vec<u8> {
    let cx = (w as f32 - 1.0) / 2.0;
    let cy = (h as f32 - 1.0) / 2.0;
    let mut reg = vec![0u8; (w * h * 3) as usize];
    for y in 0..h {
        let sy = cy + (y as f32 - cy) / LOCK_ZOOM;
        for x in 0..w {
            let sx = cx + (x as f32 - cx) / LOCK_ZOOM;
            let p = sample_rgba(wp, w, h, sx, sy);
            let i = ((y * w + x) * 3) as usize;
            reg[i] = p[0] as u8;
            reg[i + 1] = p[1] as u8;
            reg[i + 2] = p[2] as u8;
        }
    }
    reg
}

fn blur_registered(reg: &[u8], w: u32, h: u32, radius: usize) -> Vec<u8> {
    let n = (w * h) as usize;
    let mut out = vec![0u8; n * 3];
    let mut buf = vec![0.0f32; n];
    for k in 0..3 {
        for i in 0..n {
            buf[i] = reg[i * 3 + k] as f32;
        }
        for _ in 0..3 {
            buf = box_blur(&buf, w as usize, h as usize, radius);
        }
        for i in 0..n {
            out[i * 3 + k] = buf[i].clamp(0.0, 255.0) as u8;
        }
    }
    out
}

/// Run `f` with the registered backdrop and the blurred material for this
/// size, building them only when size, radius or wallpaper changed. The
/// mutex guard is held across `f`, so the slices cannot be freed by a
/// concurrent rebuild while a frame reads them.
fn with_lock_materials<R>(
    w: u32,
    h: u32,
    wallpaper: &[u8],
    f: impl FnOnce(&[u8], &[u8]) -> R,
) -> Option<R> {
    let radius = box_radius_for_sigma(CLOCK_MAT_BLUR_FRAC * h as f32);
    let sig = wallpaper_sig(wallpaper);
    let mut guard = MAT_CACHE.lock().ok()?;
    let stale = match guard.as_ref() {
        Some(c) => c.w != w || c.h != h || c.radius != radius || c.sig != sig,
        None => true,
    };
    if stale {
        let reg = build_registered(w, h, wallpaper);
        let blur = blur_registered(&reg, w, h, radius);
        *guard = Some(MatCache { w, h, radius, sig, reg, blur });
    }
    guard.as_ref().map(|c| f(&c.reg, &c.blur))
}

/// Free the cached registered wallpaper and its blur (~9 MB at 1504x1003).
/// Called automatically on the first lock frame drawn without a wallpaper;
/// `main.rs` can also call it when the state leaves Lockscreen, which is the
/// only path that releases the memory before the next lock.
pub fn drop_lock_materials() {
    if let Ok(mut g) = MAT_CACHE.lock() {
        if g.is_some() {
            *g = None;
        }
    }
}

// ---------------------------------------------------------------------------
// Lock pill (idle glyph / typing dots / wrong-password offset).
// ---------------------------------------------------------------------------
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
/// Copy the cached registered wallpaper into `canvas` and lay the vertical
/// veil over it. One alpha per row (the reference transmits equally at the
/// left and right edge of a row, which a radial vignette cannot do), so this
/// is also cheaper than the radial form it replaced: no per-pixel divide.
fn paint_backdrop(canvas: &mut [u8], w: u32, h: u32, reg: &[u8], anim: &LockAnim) {
    // 8.8 fixed point per row: one integer multiply-shift per channel
    // instead of a float multiply (measured 19 -> 6 ms per frame at
    // 1504x1003). The +128 rounds to nearest: truncating instead left the
    // whole backdrop ~0.006 alpha darker than the recipe.
    let row = (w * 4) as usize;
    let row3 = (w * 3) as usize;
    for y in 0..h {
        let keep = ((1.0 - veil_alpha(y as f32 + 0.5, h as f32) * anim.scrim)
            * 256.0)
            .round() as u32;
        let dst = &mut canvas[y as usize * row..(y as usize + 1) * row];
        let src = &reg[y as usize * row3..(y as usize + 1) * row3];
        for x in 0..w as usize {
            let si = x * 3;
            let i = x * 4;
            dst[i] = ((src[si + 2] as u32 * keep + 128) >> 8) as u8; // B
            dst[i + 1] = ((src[si + 1] as u32 * keep + 128) >> 8) as u8; // G
            dst[i + 2] = ((src[si] as u32 * keep + 128) >> 8) as u8; // R
            dst[i + 3] = 255;
        }
    }
}

/// Opaque black backdrop with the same veil (no wallpaper, or a poisoned
/// material cache).
fn paint_backdrop_black(canvas: &mut [u8], w: u32, h: u32) {
    // The veil multiplies toward black, and the base is black already, so
    // every pixel is opaque black whatever the scrim alpha.
    for i in 0..(w * h) as usize {
        canvas[i * 4] = 0;
        canvas[i * 4 + 1] = 0;
        canvas[i * 4 + 2] = 0;
        canvas[i * 4 + 3] = 255;
    }
}

/// Shadow, then the ink: the wallpaper seen THROUGH the glyphs, blurred and
/// pushed through the fitted per-channel affine. No white base and no cyan
/// specular: both were the old design. `mat` is the cached blurred
/// registered wallpaper (w*h*3); None falls back to white (no wallpaper).
fn clock_ink(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    mask: &Mask,
    anim: &LockAnim,
    s: f32,
    mat: Option<&[u8]>,
) -> Rect {
    let cx = w as f32 / 2.0;
    let cy = h as f32 / 2.0;
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
        anim.clock_dy + SHADOW_DY_FRAC * s, C_BLACK, SHADOW_ALPHA * anim.clock_opacity,
    );
    blit_glyph_material(
        canvas, w, h, mask, anim.clock_scale, cx, cy, anim.clock_dy,
        mat, anim.clock_opacity,
    )
}

/// Draw one lockscreen frame into `canvas` (fullscreen, w x h logical,
/// Argb8888 BGRA). Order: registered wallpaper (1.030x, or black fallback)
/// -> vertical veil -> giant clock (shadow, wallpaper-sampled frosting
/// inside the glyphs) -> lock pill -> date line (only if
/// `data.date_line` is Some, NOT in the video, off by default).
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
    let l = lockscreen_layout(w, h, data.dots, font, &data.time_hhmm);

    // --- (1) Registered wallpaper + (2) vertical veil + (3) clock. ---
    // Registration (1.030x about centre) and the material blur are the same
    // for every frame of a lock, so both are cached (see with_lock_materials):
    // measured 252 ms cold / 99 ms warm before the cache, 79 ms of which was
    // registration on EVERY frame.
    let src: Option<&[u8]> = match wallpaper {
        Some(wp) if wp.len() == (w * h * 4) as usize => Some(wp),
        _ => None,
    };
    let cx = w as f32 / 2.0;
    let px = CLOCK_PX_FRAC * h as f32;
    let mask = rasterize_clock(font, &data.time_hhmm, px);

    let clock_block = match src {
        Some(wp) => with_lock_materials(w, h, wp, |reg, blurred| {
            paint_backdrop(canvas, w, h, reg, &anim);
            clock_ink(canvas, w, h, &mask, &anim, s, Some(blurred))
        })
        .unwrap_or_else(|| {
            // Mutex poisoned: still draw something rather than a blank
            // screen: the veil over black plus a white clock.
            paint_backdrop_black(canvas, w, h);
            clock_ink(canvas, w, h, &mask, &anim, s, None)
        }),
        None => {
            drop_lock_materials();
            paint_backdrop_black(canvas, w, h);
            clock_ink(canvas, w, h, &mask, &anim, s, None)
        }
    };

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
        let l = lockscreen_layout(1504, H, 9, &clock_test_font(), "11:28");
        assert!(l.pill.w > 138.0 && l.pill.w < 146.0); // 153 * 1002/1080
    }

    fn clock_test_font() -> Font {
        Font::from_bytes(
            include_bytes!("../assets/fonts/Quicksand-Bold.ttf").to_vec(),
            FontSettings::default(),
        )
        .expect("vendored Quicksand-Bold parses")
    }

    #[test]
    fn clock_block_is_centred() {
        let f = clock_test_font();
        let l = lockscreen_layout(1504, H, 0, &f, "11:28");
        assert!((l.clock_block.cx() - 752.0).abs() < 1.0);
        assert!((l.clock_block.cy() - H as f32 / 2.0).abs() < 1.0);
        assert!(l.clock_block.h > 180.0 && l.clock_block.h < 200.0);
    }

    /// The reference clock uses the font's PROPORTIONAL advances, so the
    /// block width tracks the time string. Reference '11:28' ink span is
    /// 871 px at cap 279 on 2560x1440 => width/cap 3.122. Tabular cells
    /// (the old 0.60 em per digit) gave 1.204x that. Pin the ratio, and pin
    /// that a narrow-digit time really is narrower — a tabular regression
    /// makes both assertions fail.
    #[test]
    fn clock_block_uses_proportional_advances() {
        let f = clock_test_font();
        let px = CLOCK_PX_FRAC * H as f32;
        let cap = CAP_HEIGHT_RATIO * px;
        let wide = lockscreen_layout(1504, H, 0, &f, "11:28").clock_block.w;
        let ratio = wide / cap;
        assert!(
            (ratio - 3.122).abs() < 0.10,
            "'11:28' width/cap {ratio:.4}, reference 3.122"
        );
        let narrow = lockscreen_layout(1504, H, 0, &f, "11:11").clock_block.w;
        assert!(
            narrow < wide * 0.92,
            "'11:11' ({narrow:.1}) should be clearly narrower than '11:28' ({wide:.1})"
        );
    }

    /// The discriminating property of a VERTICAL veil: at any given row the
    /// alpha is the same at the left edge, the centre and the right edge.
    /// A radial vignette fails exactly this (its row alpha is .34 at both
    /// edges and .22 in the middle), and the old code was radial.
    #[test]
    fn veil_is_vertical_not_radial() {
        for &(w, h) in &[(1504u32, 1003u32), (1280, 720), (2256, 1504)] {
            // Flat mid-grey source, so the capture ratio IS the veil alpha.
            let reg = vec![200u8; (w * h * 3) as usize];
            let mut canvas = vec![0u8; (w * h * 4) as usize];
            let anim = LockAnim::locked_hold(h);
            paint_backdrop(&mut canvas, w, h, &reg, &anim);
            for (name, y) in [("top", 0u32), ("plateau", h / 2), ("bottom", h - 1)] {
                let row = &canvas[(y * w * 4) as usize..((y + 1) * w * 4) as usize];
                let left = row[2] as f32 / 200.0;
                let mid = row[((w / 2) * 4 + 2) as usize] as f32 / 200.0;
                let right = row[((w - 1) * 4 + 2) as usize] as f32 / 200.0;
                let want = 1.0 - veil_alpha(y as f32 + 0.5, h as f32);
                assert!(
                    (left - mid).abs() < 0.01 && (mid - right).abs() < 0.01,
                    "{name} row of {w}x{h} is not flat: {left} {mid} {right}"
                );
                assert!((mid - want).abs() < 0.01, "{name} row alpha {mid} != {want}");
            }
        }
    }

    /// The veil runs darker at both edges than in the middle, and its fitted
    /// anchors are the brief's: .430 top, .224 plateau, .507 bottom.
    #[test]
    fn veil_edges_match_the_fitted_recipe() {
        let h = 1440.0f32;
        assert!((veil_alpha(0.0, h) - 0.430).abs() < 0.005);
        assert!((veil_alpha(h, h) - 0.507).abs() < 0.005);
        assert!((veil_alpha(0.5 * h, h) - 0.224).abs() < 0.005);
        // The plateau covers y .278-.750 H: flat within 1e-3.
        let p0 = veil_alpha(0.278 * h, h);
        for k in 1..=10 {
            let y = (0.278 + 0.0472 * k as f32) * h;
            assert!((veil_alpha(y, h) - p0).abs() < 1e-3);
        }
        // Darker at both ends than in the middle.
        assert!(veil_alpha(0.02 * h, h) > veil_alpha(0.2 * h, h));
        assert!(veil_alpha(0.2 * h, h) > veil_alpha(0.5 * h, h));
        assert!(veil_alpha(0.9 * h, h) > veil_alpha(0.6 * h, h));
    }

    #[test]
    fn clock_material_keeps_the_wallpaper_hue() {
        // The affine, not a palette entry: a teal background must come out
        // teal, a warm one warm, and black still lifted.
        let f = clock_material_px;
        // Reference: blurred wallpaper under the first `1` is teal
        // ~(3,121,103); the observed ink there is ~(87,235,206).
        let ink = f([3, 121, 103]);
        assert!((ink[0] - 87.0).abs() < 6.0, "R {}", ink[0]);
        assert!((ink[1] - 235.0).abs() < 6.0, "G {}", ink[1]);
        assert!((ink[2] - 206.0).abs() < 6.0, "B {}", ink[2]);
        let warm = f([150, 40, 20]);
        assert!(warm[0] > ink[0], "a warm background must give warm ink");
        assert!(warm[1] < ink[1]);
        assert!(f([0, 0, 0])[1] > 40.0, "a black background is still lifted");
        // A bright wallpaper overflows all three channels; the hue is kept
        // by scaling down instead of clipping to white. The owner's sky
        // under the clock is exactly this case.
        let sky = f([194, 215, 250]);
        assert!(sky[2] > sky[1] && sky[1] > sky[0], "sky ink lost its hue: {sky:?}");
        assert!(sky[2] <= 255.0 && sky.iter().all(|v| *v >= 0.0));
        // Wherever the fit was made, nothing saturates, so the scale-down
        // is a no-op there: the fitted domain is unchanged.
        // The domain the fit was made on (the reference's blurred wallpaper
        // under the clock) must come out exactly as the plain per-channel
        // affine: the scale-down only ever fires outside it.
        let plain = |b: [u8; 3]| {
            [
                CLOCK_MAT_R.0 * b[0] as f32 + CLOCK_MAT_R.1,
                CLOCK_MAT_G.0 * b[1] as f32 + CLOCK_MAT_G.1,
                CLOCK_MAT_B.0 * b[2] as f32 + CLOCK_MAT_B.1,
            ]
        };
        let mut worst = 0.0f32;
        for r in (0..=60u8).step_by(6) {
            for g in (85..=130u8).step_by(9) {
                for b in (55..=130u8).step_by(9) {
                    let got = f([r, g, b]);
                    let want = plain([r, g, b]);
                    for k in 0..3 {
                        worst = worst.max((got[k] - want[k]).abs());
                    }
                }
            }
        }
        assert!(worst < 1e-4, "fitting domain drifted by {worst}");
    }

    #[test]
    fn box_radius_matches_the_fitted_gaussian() {
        // sigma 24 px at 1440 (CLOCK_MAT_BLUR_FRAC), 3 box passes.
        let sigma = CLOCK_MAT_BLUR_FRAC * 1440.0;
        assert!((sigma - 24.0).abs() < 0.5);
        let r = box_radius_for_sigma(sigma);
        let d = (2 * r + 1) as f32;
        let eff = (3.0 * (d * d - 1.0) / 12.0).sqrt();
        assert!((eff - sigma).abs() < 1.5, "effective sigma {eff} for r {r}");
    }

    /// Real-pixel check of the composite: over a mid-grey backdrop, a
    /// half-covered glyph pixel must land halfway between backdrop and the
    /// affine-mapped material, in BGRA byte order, opaque.
    #[test]
    fn glyph_material_composites_over_the_backdrop() {
        let (w, h) = (4u32, 4u32);
        let mut canvas = vec![0u8; (w * h * 4) as usize];
        for i in 0..(w * h) as usize {
            canvas[i * 4] = 100; // B
            canvas[i * 4 + 1] = 100; // G
            canvas[i * 4 + 2] = 100; // R
            canvas[i * 4 + 3] = 255;
        }
        // 2x2 mask, coverage .5 on one pixel, 1.0 on another.
        let mut cov = vec![0.0f32; 4];
        cov[0] = 0.5;
        cov[1] = 1.0;
        let mask = Mask { w: 2, h: 2, cov, bbox: (0, 0, 1, 1) };
        // Blurred wallpaper: teal (3,121,103) -> ink ~(87,235,206).
        let mat = [3u8, 121, 103].repeat((w * h) as usize);
        blit_glyph_material(&mut canvas, w, h, &mask, 1.0, 2.0, 2.0, 0.0, Some(&mat), 1.0);
        // bbox centre (0.5,0.5) maps to (2.0,2.0). Mask px (1,0) has
        // coverage 1.0 -> (2.5,2.0).round() = (3,2); mask px (0,0) has 0.5
        // -> (1.5,2.0).round() = (2,2).
        let at = |x: u32, y: u32| {
            let i = ((y * w + x) * 4) as usize;
            (canvas[i + 2], canvas[i + 1], canvas[i])
        };
        let full = at(3, 2);
        assert!((full.0 as i32 - 87).abs() < 6, "R {}", full.0);
        assert!((full.1 as i32 - 235).abs() < 6, "G {}", full.1);
        assert!((full.2 as i32 - 206).abs() < 6, "B {}", full.2);
        let half = at(2, 2);
        assert!((half.0 as i32 - (100 + 87) / 2).abs() < 4, "half R {}", half.0);
        // Untouched pixel keeps the backdrop exactly.
        assert_eq!(at(0, 3), (100, 100, 100));
    }

    /// The veil is applied as one alpha per row: two pixels in the same row
    /// must darken identically whatever their x, and the row alpha must be
    /// the fitted `veil_alpha`.
    #[test]
    fn veil_darkens_a_row_uniformly() {
        let (w, h) = (16u32, 8u32);
        let mut canvas = vec![200u8; (w * h * 4) as usize];
        for y in 0..h {
            let keep = 1.0 - veil_alpha(y as f32 + 0.5, h as f32);
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                canvas[i] = (canvas[i] as f32 * keep) as u8;
                canvas[i + 1] = (canvas[i + 1] as f32 * keep) as u8;
                canvas[i + 2] = (canvas[i + 2] as f32 * keep) as u8;
            }
        }
        for y in 0..h {
            let i0 = ((y * w) * 4) as usize;
            let i1 = ((y * w + w - 1) * 4) as usize;
            assert_eq!(canvas[i0], canvas[i1], "row {y} is not flat");
            let expect = (200.0 * (1.0 - veil_alpha(y as f32 + 0.5, h as f32))) as u8;
            assert_eq!(canvas[i0 + 2], expect, "row {y} alpha");
        }
    }

    #[test]
    fn registration_magnifies_about_the_centre() {
        let w = 2560.0f32;
        let cx = (w - 1.0) / 2.0;
        let dst = 0.25 * w;
        let src = cx + (dst - cx) / LOCK_ZOOM;
        // 1.030x means a source pixel 620 px off centre lands 640 px off
        // centre: the sampled offset is SMALLER than the destination one.
        assert!(src > dst, "left of centre must sample right of it at 1.03x");
        assert!((src - cx).abs() < (dst - cx).abs());
        let ratio = (dst - cx).abs() / (src - cx).abs();
        assert!((ratio - LOCK_ZOOM).abs() < 1e-5, "ratio {ratio}");
        // Centre is a fixed point.
        assert!((cx + (cx - cx) / LOCK_ZOOM - cx).abs() < 1e-6);
    }
}



