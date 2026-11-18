//! Morph animation for dynamite-rust — Dynamic Island expand/collapse.
//!
//! Pure geometry + opacity math. **No dependencies** (no smithay, no
//! fontdue, no Wayland): this file compiles standalone with
//! `rustc --edition 2021 --crate-type lib morph.rs` and its unit tests run
//! with `rustc --edition 2021 --test morph.rs`.
//!
//! # Design (see `iterations/morph-design-iter9.md` for the full write-up)
//!
//! One morph = 160 ms interpolation between two geometries
//! `(width, height, corner_radius)` plus a staggered two-phase content
//! cross-fade. The same machine drives expand and collapse in every state
//! (`island_expand` with `from`/`to` swapped — collapse uses the same
//! ease-out cubic, NOT a time-reversed easing).
//! CORRECTION (iter10 integrator): an earlier draft of this header
//! claimed collapse is the EXACT TIME-REVERSE of expand (geometry and
//! both opacities). That identity cannot hold with role-based fade
//! windows: the windows are anchored to the start of each morph
//! (outgoing raw [0, 0.35], incoming eased [0.5, 1]), so no smoothstep
//! pair satisfies outgoing_col(t) == incoming_exp(160-t) for all t (it
//! would need a + b == 1 AND a(1-b) == 0). The role-based timing is also
//! the right UX: a collapse must fade the LEAVING content out early and
//! the ARRIVING clock in late — the literal time-reverse would fade the
//! clock in early. So collapse is the same 160 ms machine with roles
//! swapped, and the unit test now asserts the true properties
//! (endpoints, monotonic shrink, no dead band, incoming fully in at
//! 160 ms).
//!
//! RETUNE (iter112): 400 -> 160 ms, EaseOutCubic kept. The live path
//!   (main.rs begin_morph) never used this constant — it uses config
//!   anim_ms (400 in the live config); both are retuned together so
//!   the module contract and the live behaviour agree again. Measured
//!   v0.50 settle (grim burst, pixel-identical criterion): expand
//!   350-454 ms, collapse 505-616 ms; the collapse tail is a faint
//!   post-morph pill refresh, not morph geometry (see
//!   iterations/rust112/morph-fix-iter112.md). 160 ms nominal puts
//!   the measured settle at ~200 ms, inside the 130-230 ms reference
//!   band, with headroom for the 16 ms timer chain + draw/commit.
//! - Size: 90x24 -> target, continuously, on an eased curve (NOT linear).
//! - Corner radius: 12 -> 20, same eased progress.
//! - Top edge pinned at the 15 px gap; centre x stays 752 (the layer is
//!   TOP-anchored and horizontally unanchored, so the compositor re-centres
//!   on every `set_size` — this module only interpolates the size).
//! - Outgoing content scales 1.0 -> 0.80 and fades 1 -> 0 over raw time
//!   progress 0 -> 0.35 (first ~56 ms). Incoming content fades 0 -> 1
//!   over EASED progress 0.5 -> 1.0 — i.e. it starts when the shell is
//!   half-grown (~66 ms wall clock on the ease-out cubic) and reaches
//!   ~0.32 by 80 ms, ~0.83 by 100 ms, ~0.97 by ~120 ms (iter33:
     //!   the fade starts only once the shell is 80% grown, so content is
     //!   never sliced by the capsule edge — the 120 ms frame used to show
     //!   only "55" / "vember 13"). The content grows WITH the shell:
//!   there is NO dead band (iter18 S1 fix — critic-pickers-iter16 S1,
//!   critic-animation-iter16 §2.1: the old raw window [0.625, 1] held
//!   content until the shell was ~99% grown, so burst frame 2 was a
//!   full-width empty black slab). The outgoing window stays on RAW time
//!   progress: its wall-clock timing ("first ~56 ms") is what the burst
//!   measurements pinned (S-1 tests below), and eased progress would end
//!   it at ~21 ms raw. The two windows overlap in raw ~0.21..0.35 — a
//!   true cross-fade, both contents faint at once; the compositor takes
//!   the per-channel max, so nothing muddies.
//! - The capsule itself is always fully opaque; only *contents* fade.
//! - Progress is wall-clock based (`Instant`), never frame-count based:
//!   a late frame skips ahead, it never drifts the animation long.
//!
//! # Non-goals (explicit)
//!
//! - **Lockscreen morphs are NOT this.** The lock sequence (scrim 100 ms in,
//!   giant clock ~130 ms in / ~230 ms out, scrim 170 ms out, scrim-first
//!   ordering) has different timing and sequencing — see
//!   `analysis/lockscreen-scope.md`. Do not call `Morph::island_expand()` /
//!   `island_collapse()` for the lockscreen, and do not reuse the 160 ms
//!   constants there. `Morph` is generic over duration/easing, so lock code
//!   may reuse the *type* with its own durations, but only as a dumb
//!   interpolator with its own phase sequencing.
//! - This module draws nothing and knows nothing about Wayland, layer
//!   surfaces, `IslandState`, or fonts. The integrator (see §6 of the design
//!   doc) owns: the calloop 16 ms timer, `set_size` + commit per frame, the
//!   morph-aware `draw()` variant, and threading an `alpha` through the
//!   panel functions. Endpoints come from `IslandState::size()`/`radius()`
//!   at morph start — they are NOT duplicated here.

use std::time::{Duration, Instant};

/// Default island morph duration: 160 ms, expand AND collapse, all states.
pub const MORPH_DURATION_MS: u64 = 160;
/// Raw time progress at which the outgoing content has fully faded out
/// (first ~56 ms of the 160 ms morph). Raw basis: the burst
/// measurements pinned this wall-clock timing (see S-1 tests below).
pub const OUTGOING_FADE_END: f32 = 0.35;
/// Eased (shell-width) progress at which the incoming content starts
/// fading in: the shell is 80% grown (~66 ms wall clock on the
/// ease-out cubic). (iter33: was 0.5, which started the fade while
/// the capsule was only ~66% grown at 120 ms — the settled-size
/// content was sliced by the capsule edge ("55" / "vember 13" in the
/// 120 ms frame, iter32 beauty critic §3). At 0.8 the content is
/// ~fully inside the capsule from its first visible frame. The price
/// is a ~10 ms window (outgoing gone at 56 ms, incoming starts at
/// ~66 ms) showing the bare capsule — but the capsule is frost now,
/// not black, so the window reads as the island growing, not an empty
/// slab. Easing and duration are UNCHANGED: 160 ms ease-out cubic.)
pub const INCOMING_FADE_START: f32 = 0.8;
/// Outgoing content scale at the end of its fade window. Judgement call
/// (see design doc §8 Q1): the video's mid-morph frame s05 shows the clock
/// "small" inside the grown panel, but its digit height was never measured
/// against the collapsed 12 px. Correct from the burst battery if wrong.
pub const OUTGOING_SCALE_END: f32 = 0.80;
/// Incoming content scale at the start of its fade window (v1: the
/// integrator may ignore `incoming_scale` and use opacity only — see
/// design doc §4).
pub const INCOMING_SCALE_START: f32 = 0.80;

// ---------------------------------------------------------------------------
// Easing
// ---------------------------------------------------------------------------

/// Easing curve applied to the raw 0..1 time progress.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Easing {
    /// `1 - (1 - p)^3`. Fast start, gentle landing, no overshoot.
    /// **Default.** Chosen because no video frame shows the panel ever
    /// exceeding its target size, and a monotonic curve keeps the
    /// layer-surface `set_size`/`configure` handshake simple.
    EaseOutCubic,
    /// Cubic-bezier with control overshoot (c1 = 1.70158): settles ~10%
    /// past the target, then back. The "springy iOS Dynamic Island" feel.
    /// One-line switch if critics judge the default too flat — only on
    /// burst-frame evidence, never on a hunch.
    EaseOutBack,
    /// True underdamped harmonic oscillator, normalised to settle at 1:
    /// `1 - e^(-zwt) * (cos(wd*t) + z/sqrt(1-z^2) * sin(wd*t))`.
    /// Provided for experiments / iter10. NOT wired: it overshoots and
    /// oscillates, so it needs the non-monotonic-size handling discussed
    /// in design doc §2.1(2) before anyone turns it on.
    Spring { zeta: f32, omega: f32 },
    /// For unit tests only — asserts the window math without the easing
    /// curve in the way. **Never use in production.**
    Linear,
}

impl Easing {
    /// Apply the curve to raw progress `p` (0..1). Output is ~0..1 for
    /// EaseOutCubic/Linear, overshoots slightly past 1 for EaseOutBack and
    /// Spring. Callers must not assume `<= 1.0` except for EaseOutCubic.
    pub fn apply(self, p: f32) -> f32 {
        let p = p.clamp(0.0, 1.0);
        match self {
            Easing::Linear => p,
            Easing::EaseOutCubic => 1.0 - (1.0 - p).powi(3),
            Easing::EaseOutBack => {
                // c1 = 1.70158, c3 = c1 + 1.
                let c1 = 1.70158f32;
                let c3 = c1 + 1.0;
                1.0 + c3 * (p - 1.0).powi(3) + c1 * (p - 1.0).powi(2)
            }
            Easing::Spring { zeta, omega } => {
                // Normalised underdamped step response, evaluated at time
                // t = p * MORPH_DURATION. zeta < 1 required; clamp the
                // caller out of the overdamped regime.
                let zeta = zeta.clamp(0.05, 0.95);
                let t = p * MORPH_DURATION_MS as f32 / 1000.0;
                let wd = omega * (1.0 - zeta * zeta).sqrt();
                let exp = (-zeta * omega * t).exp();
                let phase = wd * t;
                1.0 - exp
                    * (phase.cos() + (zeta / (1.0 - zeta * zeta).sqrt()) * phase.sin())
            }
        }
    }
}

/// Smoothstep: 0 below `edge0`, 1 above `edge1`, zero slope at both edges.
/// Used for the opacity windows so no content appears/disappears with a
/// visible kink.
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    debug_assert!(edge0 < edge1);
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ---------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------

/// One endpoint of a morph: logical width, height, corner radius (px).
/// Endpoints come from `IslandState::size()` / `radius()` in main.rs at
/// morph start — they are deliberately NOT duplicated here.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Geometry {
    pub width: f32,
    pub height: f32,
    pub radius: f32,
}

impl Geometry {
    pub const fn new(width: f32, height: f32, radius: f32) -> Self {
        Self {
            width,
            height,
            radius,
        }
    }

    /// Collapsed pill: 90x24, radius 12 (video spec, top gap 15 px).
    pub const fn collapsed() -> Self {
        Self::new(90.0, 24.0, 12.0)
    }

    fn lerp(self, to: Self, t: f32) -> Self {
        Self {
            width: self.width + (to.width - self.width) * t,
            height: self.height + (to.height - self.height) * t,
            radius: self.radius + (to.radius - self.radius) * t,
        }
    }
}

// ---------------------------------------------------------------------------
// Morph frame (one instant of the animation)
// ---------------------------------------------------------------------------

/// Everything the renderer needs for one morph frame. Width/height/radius
/// are interpolated in logical px (round to u32 only at the `set_size` call
/// site — never inside the interpolation). Opacities are 0..1; scales are
/// multipliers on the content's settled size.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MorphFrame {
    pub width: f32,
    pub height: f32,
    pub radius: f32,
    /// Eased progress, 0..1 (1.0 exactly when done, for the default easing).
    pub progress: f32,
    /// Raw time progress, 0..1 (for logging / tests).
    pub raw_progress: f32,
    /// Outgoing content opacity: 1 -> 0 over raw time 0..OUTGOING_FADE_END.
    pub outgoing_opacity: f32,
    /// Outgoing content scale: 1.0 -> OUTGOING_SCALE_END, same window.
    pub outgoing_scale: f32,
    /// Incoming content opacity: 0 -> 1 over EASED progress
    /// INCOMING_FADE_START..1 (shell 80%-grown at fade start, ~66 ms;
    /// ~0.32 at 80 ms, ~0.83 at 100 ms). Keyed to shell width, not
    /// wall clock (iter18; start moved 0.5 -> 0.8 in iter33 so content
    /// is never sliced by the capsule edge).
    pub incoming_opacity: f32,
    /// Incoming content scale: INCOMING_SCALE_START -> 1.0, same window.
    /// (v1: the integrator may ignore this and use opacity only.)
    pub incoming_scale: f32,
    /// True once raw progress reaches 1.0 — the morph is over and the
    /// island must be drawn settled at the exact target geometry.
    pub done: bool,
}

// ---------------------------------------------------------------------------
// Morph state machine
// ---------------------------------------------------------------------------

/// A single 160 ms island morph. Create with `island_expand` /
/// `island_collapse` (or `new` for a custom duration/easing), poll with
/// `frame_at(Instant::now())` from the 16 ms calloop timer, and drop it
/// when `frame.done` is true and the settled draw has landed.
///
/// The machine is direction-agnostic: it interpolates `from` -> `to` and
/// applies the fade windows to the *roles* outgoing/incoming. Expand and
/// collapse are the same code with `from`/`to` swapped — but that does NOT
/// make collapse the exact time-reverse of expand (see the header
/// CORRECTION above: role-based fade windows anchored to each morph's
/// start break the identity; collapse is the same 160 ms machine with
/// roles swapped, which is close to the time-reverse but differs in the
/// window edges).
#[derive(Clone, Debug)]
pub struct Morph {
    from: Geometry,
    to: Geometry,
    start: Instant,
    duration: Duration,
    easing: Easing,
}

impl Morph {
    /// General constructor. `duration` is normally 160 ms; pass anything
    /// else only for tests or for the lockscreen's own (separate, faster)
    /// timings — never pass 160 ms for lock code (design doc §5).
    pub fn new(from: Geometry, to: Geometry, duration: Duration, easing: Easing) -> Self {
        Self {
            from,
            to,
            start: Instant::now(),
            duration,
            easing,
        }
    }

    /// Island expand: collapsed (or any state) -> target state geometry.
    /// 160 ms, ease-out cubic. `to` is the target state's
    /// `(size, radius)` from `IslandState`.
    pub fn island_expand(from: Geometry, to: Geometry) -> Self {
        Self::new(
            from,
            to,
            Duration::from_millis(MORPH_DURATION_MS),
            Easing::EaseOutCubic,
        )
    }

    /// Island collapse: any state -> collapsed geometry. Identical
    /// construction to `island_expand` with `from`/`to` swapped. (The
    /// swap makes collapse the same 160 ms machine with roles swapped —
    /// NOT the exact time-reverse of expand; see the header CORRECTION.
    /// The old "exact reverse guarantee" wording was wrong — iter13 D8.)
    pub fn island_collapse(from: Geometry) -> Self {
        Self::island_expand(from, Geometry::collapsed())
    }

    /// Test / restart helper: build a morph whose clock starts at `start`
    /// instead of now. Lets unit tests and the integrator's burst harness
    /// evaluate exact instants without sleeping.
    pub fn new_at(
        from: Geometry,
        to: Geometry,
        duration: Duration,
        easing: Easing,
        start: Instant,
    ) -> Self {
        Self {
            from,
            to,
            start,
            duration,
            easing,
        }
    }

    /// Start a replacement morph from the CURRENT interpolated geometry
    /// (for a state command arriving mid-morph). The island never jumps
    /// back to a stale nominal size. `to` is the new target geometry.
    pub fn restart_from_current(&self, now: Instant, to: Geometry) -> Self {
        let frame = self.frame_at(now);
        // The replacement morph's clock starts at `now` (not at the
        // wall-clock instant of this call): frame_at(now) must return
        // exactly the current interpolated geometry, with no jump.
        // (iter10 fix: `Self::new` here stamped Instant::now(), so the
        // restarted morph had already run ~200 ms by the time the caller
        // evaluated frame_at(now) — the unit test caught it.)
        Self::new_at(
            Geometry::new(frame.width, frame.height, frame.radius),
            to,
            self.duration,
            self.easing,
            now,
        )
    }

    /// Raw time progress 0..1 at `now` (clamped; wall-clock based).
    pub fn raw_progress(&self, now: Instant) -> f32 {
        let elapsed = now.duration_since(self.start);
        if self.duration.is_zero() {
            return 1.0;
        }
        (elapsed.as_secs_f32() / self.duration.as_secs_f32()).clamp(0.0, 1.0)
    }

    /// True once the morph's time is up.
    pub fn is_done(&self, now: Instant) -> bool {
        self.raw_progress(now) >= 1.0
    }

    /// The full frame at `now`: interpolated geometry + both content
    /// opacities/scales. Pure function of `now` — calling it twice with
    /// the same instant returns the same frame.
    pub fn frame_at(&self, now: Instant) -> MorphFrame {
        self.frame_for_raw(self.raw_progress(now))
    }

    /// The full frame at raw time progress `raw` (0..1), without
    /// needing an Instant. `frame_at(now)` is
    /// `frame_for_raw(self.raw_progress(now))`.
    pub fn frame_for_raw(&self, raw: f32) -> MorphFrame {
        let raw = raw.clamp(0.0, 1.0);
        let p = self.easing.apply(raw);
        let geom = self.from.lerp(self.to, p);
        // Outgoing: 1 -> 0 over RAW time 0..OUTGOING_FADE_END (unchanged
        // — the burst measurements pinned this wall-clock timing).
        let out_t = smoothstep(0.0, OUTGOING_FADE_END, raw);
        // Incoming: 0 -> 1 over EASED progress INCOMING_FADE_START..1.0
        // (iter18 S1 fix, iter33 start 0.5 -> 0.8): the fade starts
        // when the shell is 80% grown (~66 ms), so content is never
        // sliced by the capsule edge. Keying to eased progress is the
        // point — it IS the shell-width progress. The old raw [0.625, 1]
        // window held content until the shell was ~99% grown (frame 2
        // empty slab); the 0.5 window sliced it at 120 ms.
        let in_t = smoothstep(INCOMING_FADE_START, 1.0, p);
        MorphFrame {
            width: geom.width,
            height: geom.height,
            radius: geom.radius,
            progress: p,
            raw_progress: raw,
            outgoing_opacity: 1.0 - out_t,
            outgoing_scale: 1.0 + (OUTGOING_SCALE_END - 1.0) * out_t,
            incoming_opacity: in_t,
            incoming_scale: INCOMING_SCALE_START + (1.0 - INCOMING_SCALE_START) * in_t,
            done: raw >= 1.0,
        }
    }

    /// Invert the easing: given eased progress `p`, return the raw time
    /// progress that produces it. Exact for Linear and EaseOutCubic
    /// (p = 1 - (1 - raw)^3  =>  raw = 1 - (1 - p)^(1/3)); for
    /// EaseOutBack / Spring (overshooting curves, never used for the
    /// island) returns None and the caller must fall back to
    /// wall-clock. Used by the integrator (S-1, v0.7) to draw the
    /// morph frame matching the PRESENTED geometry: the configure for
    /// a requested size can lag the timer by tens of ms, and
    /// frame_at(now) then draws a later frame's (already-faded)
    /// content into an earlier frame's geometry.
    pub fn raw_for_eased(&self, p: f32) -> Option<f32> {
        let p = p.clamp(0.0, 1.0);
        match self.easing {
            Easing::Linear => Some(p),
            Easing::EaseOutCubic => Some(1.0 - (1.0 - p).cbrt()),
            _ => None,
        }
    }

    /// Convenience: the settled target geometry (what the final draw uses).
    pub fn target(&self) -> Geometry {
        self.to
    }

    /// Convenience: the morph duration (for the integrator's watchdog:
    /// force the settled draw if no configure arrives within ~600 ms
    /// after `done`).
    pub fn duration(&self) -> Duration {
        self.duration
    }
}

// ---------------------------------------------------------------------------
// Unit tests — run with: rustc --edition 2021 --test morph.rs && ./morph
// (or `cargo test` from a scratch crate; this file has no dependencies.)
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    fn morph_at(from: Geometry, to: Geometry) -> (Morph, Instant) {
        let start = Instant::now();
        let m = Morph::new_at(
            from,
            to,
            Duration::from_millis(MORPH_DURATION_MS),
            Easing::EaseOutCubic,
            start,
        );
        (m, start)
    }

    fn at(start: Instant, ms: u64) -> Instant {
        start + Duration::from_millis(ms)
    }

    #[test]
    fn endpoints_are_exact() {
        let (m, start) = morph_at(Geometry::collapsed(), Geometry::new(342.0, 300.0, 20.0));
        let f0 = m.frame_at(start);
        assert_eq!((f0.width, f0.height, f0.radius), (90.0, 24.0, 12.0));
        assert_eq!(f0.outgoing_opacity, 1.0);
        assert_eq!(f0.incoming_opacity, 0.0);
        assert!(!f0.done);
        let f1 = m.frame_at(at(start, 160));
        assert_eq!((f1.width, f1.height, f1.radius), (342.0, 300.0, 20.0));
        assert_eq!(f1.outgoing_opacity, 0.0);
        assert_eq!(f1.incoming_opacity, 1.0);
        assert_eq!(f1.incoming_scale, 1.0);
        assert!(f1.done);
        // Past the end clamps to the settled state, never beyond it.
        let f2 = m.frame_at(at(start, 900));
        assert_eq!((f2.width, f2.height, f2.radius), (342.0, 300.0, 20.0));
        assert!(f2.done);
    }

    #[test]
    fn nothing_pops_in_at_t0() {
        // Incoming content is exactly 0 at t=0 and stays 0 until the
        // shell is 80% grown (eased progress 0.8, ~66 ms — iter33;
        // was 0.5/~82 ms in iter18, which sliced the content at
        // 120 ms). It is faintly visible by ~80 ms and past halfway
        // by ~100 ms: the content grows WITH the shell, inside it.
        let (m, start) = morph_at(Geometry::collapsed(), Geometry::new(342.0, 300.0, 20.0));
        for ms in [0, 10, 25, 40, 50, 60, 66] {
            assert_eq!(
                m.frame_at(at(start, ms)).incoming_opacity,
                0.0,
                "incoming opacity must be 0 at {ms} ms"
            );
        }
        // ~80 ms: ~0.32; ~100 ms: ~0.83 — clearly ramping.
        let approx = |got: f32, want: f32| (got - want).abs() < 0.01;
        assert!(approx(m.frame_at(at(start, 80)).incoming_opacity, 0.316));
        assert!(approx(m.frame_at(at(start, 100)).incoming_opacity, 0.828));
    }

    #[test]
    fn no_dead_band_content_grows_with_shell() {
        // iter18 S1 fix, iter33 start moved 0.5 -> 0.8. At 80 ms the
        // panel is ~87% grown (ease-out cubic at raw 0.5) and the
        // incoming content is at ~0.32 — inside the capsule, never
        // sliced by its edge. The outgoing content is gone by 56 ms
        // and the incoming starts at ~66 ms, so there IS a ~10 ms
        // window showing the bare capsule — but the capsule is frost
        // now, not black, so it reads as the island growing, not an
        // empty slab (and the old 0.5 start sliced "55"/"vember 13").
        let (m, start) = morph_at(Geometry::collapsed(), Geometry::new(342.0, 300.0, 20.0));
        let mid = m.frame_at(at(start, 80));
        assert_eq!(mid.outgoing_opacity, 0.0);
        let approx = |got: f32, want: f32| (got - want).abs() < 0.01;
        assert!(approx(mid.incoming_opacity, 0.316));
        assert!(mid.width > 300.0 && mid.width < 320.0, "width {}", mid.width);
        // At 56 ms the outgoing fade has just ended and the incoming
        // has not started yet (bare frost capsule, ~10 ms).
        let f56 = m.frame_at(at(start, 56));
        assert_eq!(f56.outgoing_opacity, 0.0);
        assert_eq!(f56.incoming_opacity, 0.0);
        // At 48 ms the outgoing clock is faint; the incoming content
        // is still 0 — nothing is ever drawn half-sliced.
        let f48 = m.frame_at(at(start, 48));
        assert!(f48.outgoing_opacity > 0.0 && f48.outgoing_opacity < 0.1);
        assert_eq!(f48.incoming_opacity, 0.0);
        // By 100 ms the incoming content is past 0.8.
        let f100 = m.frame_at(at(start, 100));
        assert!(approx(f100.incoming_opacity, 0.828));
    }

    #[test]
    fn mid_morph_grown_and_content_visible() {
        // iter18 S1 fix, iter33 fade start 0.5 -> 0.8 — REPLACES
        // mid_morph_is_grown_but_near_empty. At ~80 ms the panel is
        // ~87% grown and the incoming content is at ~0.32 opacity:
        // grown WITH content, inside the capsule, not near-empty and
        // never sliced. Past 0.8 by ~100 ms.
        let (m, start) = morph_at(Geometry::collapsed(), Geometry::new(342.0, 300.0, 20.0));
        let mid = m.frame_at(at(start, 80));
        assert!(mid.width > 300.0, "width {} should be near 342", mid.width);
        assert!(mid.incoming_opacity > 0.3, "incoming {}", mid.incoming_opacity);
        assert_eq!(mid.outgoing_opacity, 0.0);
        let late = m.frame_at(at(start, 100));
        assert!(late.incoming_opacity > 0.8, "incoming {}", late.incoming_opacity);
    }

    #[test]
    fn incoming_fade_is_keyed_to_shell_width() {
        // The iter18 contract, iter33 start 0.5 -> 0.8, pinned on the
        // wallpaper geometry (90 -> 1008, the critic-pickers-iter16 S1
        // case): the fade starts at exactly 80% shell width and ramps
        // with it. 80% width (~824) is reached at ~66 ms; frame 2
        // (~80 ms, width ~893-907) shows content at ~0.32 — inside
        // the capsule, never sliced by its edge.
        let (m, start) = morph_at(Geometry::collapsed(), Geometry::new(1008.0, 270.0, 20.0));
        let approx = |got: f32, want: f32| (got - want).abs() < 0.01;
        let f32 = m.frame_at(at(start, 32));
        assert!(f32.width < 549.0, "width {} not yet half-grown", f32.width);
        assert_eq!(f32.incoming_opacity, 0.0);
        let f40 = m.frame_at(at(start, 40));
        assert!(f40.width > 549.0, "width {} past half-grown", f40.width);
        assert_eq!(f40.incoming_opacity, 0.0);
        let f80 = m.frame_at(at(start, 80));
        assert!(approx(f80.incoming_opacity, 0.316));
        assert!(f80.width > 890.0, "width {}", f80.width);
    }

    #[test]
    fn incoming_opacity_is_monotonic() {
        // The fade never goes backwards: no flicker mid-morph.
        let (m, start) = morph_at(Geometry::collapsed(), Geometry::new(342.0, 300.0, 20.0));
        let mut prev = 0.0f32;
        for ms in (0..=160).step_by(10) {
            let v = m.frame_at(at(start, ms)).incoming_opacity;
            assert!(v >= prev - f32::EPSILON, "incoming dipped at {ms} ms");
            prev = v;
        }
        assert_eq!(prev, 1.0);
    }

    #[test]
    fn width_is_monotonic_on_default_easing() {
        let (m, start) = morph_at(Geometry::collapsed(), Geometry::new(342.0, 300.0, 20.0));
        let mut prev = 0.0f32;
        for ms in (0..=160).step_by(10) {
            let w = m.frame_at(at(start, ms)).width;
            assert!(w >= prev - f32::EPSILON, "width went backwards at {ms} ms");
            prev = w;
        }
    }

    #[test]
    fn collapse_uses_same_machine_roles_swapped() {
        // Replaces the old collapse_is_time_reverse_of_expand, which
        // asserted collapse(t) == expand(160-t) in geometry AND both
        // opacities. That identity cannot hold with role-based fade
        // windows (see module header correction, iter10): the windows
        // are anchored to each morph's start, so no smoothstep pair
        // satisfies it for all t. What IS true and asserted here:
        // collapse is the same 160 ms ease-out machine with from/to
        // swapped — endpoints exact, width monotonic decreasing, no
        // sliced content (incoming clock ~0.32 at 80 ms, iter18 +
        // iter33), the incoming clock fully in at 160 ms.
        let (col, start) = morph_at(Geometry::new(342.0, 300.0, 20.0), Geometry::collapsed());
        let f0 = col.frame_at(start);
        assert_eq!((f0.width, f0.height, f0.radius), (342.0, 300.0, 20.0));
        assert_eq!(f0.outgoing_opacity, 1.0);
        assert_eq!(f0.incoming_opacity, 0.0);
        assert!(!f0.done);
        let mid = col.frame_at(at(start, 80));
        assert!(mid.width < 342.0 && mid.width > 90.0, "width {}", mid.width);
        assert_eq!(mid.outgoing_opacity, 0.0);
        // iter18, iter33 fade start 0.5 -> 0.8: the incoming clock
        // fades in WITH the shrinking shell (same eased-basis window,
        // roles swapped) — ~0.32 at 80 ms, ~0.83 at 100 ms, ~1.0 by
        // 160 ms. No sliced content in either direction.
        let approx = |got: f32, want: f32| (got - want).abs() < 0.01;
        assert!(approx(mid.incoming_opacity, 0.316));
        let f1 = col.frame_at(at(start, 160));
        assert_eq!((f1.width, f1.height, f1.radius), (90.0, 24.0, 12.0));
        assert_eq!(f1.outgoing_opacity, 0.0);
        assert_eq!(f1.incoming_opacity, 1.0);
        assert!(f1.done);
        let mut prev = f32::MAX;
        for ms in (0..=160).step_by(10) {
            let w = col.frame_at(at(start, ms)).width;
            assert!(w <= prev + f32::EPSILON, "width grew at {ms} ms");
            prev = w;
        }
    }

    #[test]
    fn lock_shrink_is_direction_agnostic() {
        // Collapsed 90 -> Lock 54: width shrinks, height/radius constant.
        let (m, start) = morph_at(Geometry::collapsed(), Geometry::new(54.0, 24.0, 12.0));
        let mid = m.frame_at(at(start, 80));
        assert!(mid.width < 90.0 && mid.width > 54.0);
        assert_eq!(mid.height, 24.0);
        assert_eq!(mid.radius, 12.0);
    }

    #[test]
    fn expanded_to_expanded_interpolates_directly() {
        // Power 400x150 -> calendar 342x300: no collapse to the pill in
        // between; both dimensions move at once.
        let (m, start) = morph_at(
            Geometry::new(400.0, 150.0, 20.0),
            Geometry::new(342.0, 300.0, 20.0),
        );
        let mid = m.frame_at(at(start, 80));
        assert!(mid.width < 400.0 && mid.width > 342.0);
        assert!(mid.height > 150.0 && mid.height < 300.0);
        assert_eq!(mid.radius, 20.0);
    }

    #[test]
    fn restart_from_current_never_jumps() {
        // A command at 80 ms restarts from the interpolated frame, not
        // from the stale nominal size of the old state.
        let (m, start) = morph_at(Geometry::collapsed(), Geometry::new(342.0, 300.0, 20.0));
        let now = at(start, 80);
        let restarted = m.restart_from_current(now, Geometry::new(400.0, 150.0, 20.0));
        let f0 = restarted.frame_at(now);
        let orig = m.frame_at(now);
        assert_eq!((f0.width, f0.height), (orig.width, orig.height));
        assert!(f0.width > 300.0, "restart must start from ~grown size");
    }

    #[test]
    fn frame_for_raw_matches_frame_at() {
        // frame_at(now) must be exactly frame_for_raw(raw_progress(now)).
        let (m, start) = morph_at(Geometry::collapsed(), Geometry::new(342.0, 300.0, 20.0));
        for ms in [0, 20, 40, 80, 120, 160, 900] {
            let a = m.frame_at(at(start, ms));
            let b = m.frame_for_raw(ms as f32 / 160.0);
            assert_eq!(a, b, "mismatch at {ms} ms");
        }
    }

    #[test]
    fn raw_for_eased_inverts_ease_out_cubic() {
        // S-1 (v0.7): the drawn frame must match the PRESENTED width.
        // Inverting the eased progress recovered from a width must
        // return the raw progress the geometry was requested at.
        let (m, start) = morph_at(Geometry::collapsed(), Geometry::new(342.0, 300.0, 20.0));
        for ms in [0, 10, 20, 40, 56, 80, 100, 120, 140, 160] {
            let f = m.frame_at(at(start, ms));
            let raw = ms as f32 / 160.0;
            let back = m.raw_for_eased(f.progress).expect("cubic must invert");
            assert!(
                (back - raw).abs() < 1e-4,
                "raw {raw} -> progress {} -> back {back}",
                f.progress
            );
        }
        // Linear inverts trivially; overshooting curves decline.
        let lin = Morph::new_at(
            Geometry::collapsed(),
            Geometry::new(342.0, 300.0, 20.0),
            Duration::from_millis(MORPH_DURATION_MS),
            Easing::Linear,
            start,
        );
        assert_eq!(lin.raw_for_eased(0.37), Some(0.37));
        let back = Morph::new_at(
            Geometry::collapsed(),
            Geometry::new(342.0, 300.0, 20.0),
            Duration::from_millis(MORPH_DURATION_MS),
            Easing::EaseOutBack,
            start,
        );
        assert_eq!(back.raw_for_eased(0.5), None);
    }

    #[test]
    fn easing_is_not_linear() {
        // At p=0.25 the panel must already be >25% grown (fast start).
        let (m, start) = morph_at(Geometry::collapsed(), Geometry::new(342.0, 300.0, 20.0));
        let f = m.frame_at(at(start, 40));
        let grown = (f.width - 90.0) / (342.0 - 90.0);
        assert!(
            grown > 0.5,
            "ease-out must be well past halfway by 40 ms, got {grown}"
        );
    }
}
