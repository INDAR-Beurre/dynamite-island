//! Morph animation for dynamite-rust — Dynamic Island expand/collapse.
//!
//! Pure geometry + opacity math. **No dependencies** (no smithay, no
//! fontdue, no Wayland): this file compiles standalone with
//! `rustc --edition 2021 --crate-type lib morph.rs` and its unit tests run
//! with `rustc --edition 2021 --test morph.rs`.
//!
//! # Design (see `iterations/morph-design-iter9.md` for the full write-up)
//!
//! One morph = 233 ms interpolation between two geometries
//! `(width, height, corner_radius)` plus a staggered two-phase content
//! cross-fade. The reference has a ~150 ms, ~1% overshoot, but the
//! overshoot factors in this build are disabled, so the live morph ends
//! at the 233 ms growth endpoint without an empty settle tail.
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
//!   r1d §2: eased curve is EaseOutQuad; after the 233 ms growth the
//!   panel overshoots to `to * (1 + OVERSHOOT)` and decays back over
//!   SETTLE_MS on dimensions that GREW. Shrinking dimensions never
//!   exceed their endpoints (reference shows no undershoot), so the
//!   ease-out-back/spring paths stay unwired.
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

/// Default island morph duration: 233 ms growth, expand AND collapse, all
/// states (r1d §2: the reference's 14 measured poses span 233 ms of
/// growth at 60 fps; the live config's 160 ms produced only 2-3 poses —
/// the motion reads as a jump-cut, which the owner calls "laggy").
pub const MORPH_DURATION_MS: u64 = 233;

/// Settle tail after the growth phase, on dimensions that GREW. The live
/// overshoot factors below are zero, so geometry is already at its target
/// at the growth endpoint and no additional frames are needed. Keep a tail
/// only when measured overshoot is enabled.
pub const SETTLE_MS: u64 = 0;
/// Width overshoot as a fraction of the target width. Reference measured:
/// peak 825 device px -> rest 816 device px (825/816 - 1 = 0.01103).
/// 0.0 = disabled, see above.
pub const W_OVERSHOOT: f32 = 0.0;
/// Height overshoot as a fraction of the target height. Reference measured:
/// peak 188 -> rest 186 device px (188/186 - 1 = 0.01075).
/// 0.0 = disabled, see above.
pub const H_OVERSHOOT: f32 = 0.0;
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
    /// `1 - (1 - p)^2`. Fast start, gentle landing, no overshoot (the
    /// settle overshoot is a separate phase owned by `Morph`, not the
    /// easing, so this curve stays monotone). **Default** (r1d §2):
    /// best MSE fit to the 14 pose widths from the 60 fps reference
    /// capture — 0.00259 vs 0.01201 for the old ease-out cubic.
    EaseOutQuad,
    /// `1 - (1 - p)^3`. Fast start, gentle landing, no overshoot.
    /// Retired as default in r1d §2 (worse MSE fit than quad).
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
    /// the monotone curves, overshoots slightly past 1 for EaseOutBack
    /// and Spring. Callers must not assume `<= 1.0` except for the
    /// monotone curves (EaseOutQuad, EaseOutCubic, Linear).
    pub fn apply(self, p: f32) -> f32 {
        let p = p.clamp(0.0, 1.0);
        match self {
            Easing::Linear => p,
            Easing::EaseOutQuad => 1.0 - (1.0 - p).powi(2),
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
    /// General constructor. `duration` is normally 233 ms (the growth
    /// phase); pass anything else only for tests or for the lockscreen's
    /// own (separate, faster) timings — never pass for lock code
    /// (design doc §5).
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
    /// 233 ms, ease-out quad, then the settle tail on growing dims.
    /// `to` is the target state's `(size, radius)` from `IslandState`.
    pub fn island_expand(from: Geometry, to: Geometry) -> Self {
        Self::new(
            from,
            to,
            Duration::from_millis(MORPH_DURATION_MS),
            Easing::EaseOutQuad,
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

    /// True once growth has reached its endpoint and any measured settle
    /// tail is complete. With the current zero overshoot/tail, this is the
    /// same instant that growth reaches duration.
    pub fn is_done(&self, now: Instant) -> bool {
        self.raw_progress(now) >= 1.0 && self.settle_t(now) >= 1.0
    }

    /// Settle progress is 0 during growth; after growth it is 1 for the
    /// current zero-tail configuration, or progresses across SETTLE_MS if a
    /// measured overshoot is enabled again. Non-growing morphs always report 1.
    fn settle_t(&self, now: Instant) -> f32 {
        let elapsed = now.duration_since(self.start);
        if elapsed < self.duration {
            return 0.0;
        }
        let grows = self.to.width > self.from.width || self.to.height > self.from.height;
        if !grows {
            return 1.0;
        }
        let tail = elapsed - self.duration;
        if SETTLE_MS == 0 {
            return 1.0;
        }
        (tail.as_secs_f32() / (SETTLE_MS as f32 / 1000.0)).clamp(0.0, 1.0)
    }

    /// Overshoot envelope in [0, 1]: 0 during growth, 1 at the moment the
    /// growth tops out (raw progress 1.0 — the reference peak frame),
    /// decaying quadratically to 0 at the end of SETTLE_MS. (1 - s)^2:
    /// fast initial decay, gentle landing — one overshoot and a smooth
    /// touchdown, never a spring.
    fn overshoot_at(&self, s: f32) -> f32 {
        if self.to.width <= self.from.width && self.to.height <= self.from.height {
            return 0.0;
        }
        (1.0 - s.clamp(0.0, 1.0)) * (1.0 - s.clamp(0.0, 1.0))
    }

    /// The full frame at `now`: interpolated geometry + both content
    /// opacities/scales. Pure function of `now` — calling it twice with
    /// the same instant returns the same frame.
    pub fn frame_at(&self, now: Instant) -> MorphFrame {
        let raw = self.raw_progress(now);
        let settle_t = if raw >= 1.0 {
            self.settle_t(now)
        } else {
            0.0
        };
        let growing = self.to.width > self.from.width || self.to.height > self.from.height;
        let done = raw >= 1.0 && (!growing || settle_t >= 1.0);
        // Envelope only wakes AT the growth end (raw >= 1); during
        // growth the ease-out quad itself never exceeds the target.
        let over = if raw >= 1.0 {
            self.overshoot_at(settle_t)
        } else {
            0.0
        };
        self.frame_with(raw, over, done)
    }

    /// The full frame at raw time progress `raw` (0..1) with NO settle
    /// tail — the historic contract used by unit tests and by callers
    /// that drive the morph on raw progress. Growth geometry here is
    /// identical to `frame_at` below raw 1.0; at raw 1.0 (no settle in
    /// the raw domain) `done` is true when no dimension grows.
    pub fn frame_for_raw(&self, raw: f32) -> MorphFrame {
        let raw = raw.clamp(0.0, 1.0);
        let growing = self.to.width > self.from.width || self.to.height > self.from.height;
        // Raw domain has no settle: the envelope stays 0 even at raw
        // 1.0 (the growth-only endpoint), and done is only true for
        // morphs with no growing dimension.
        self.frame_with(raw, 0.0, raw >= 1.0 && !growing)
    }

    /// Shared frame computation: eased growth + settle overshoot + fade
    /// windows. `over` is the pre-computed overshoot envelope (0 during
    /// growth and for the whole raw-only domain; the settle decay for
    /// growing dims after raw 1.0, via `frame_at`). `done` is decided by
    /// the caller (settle-aware from `frame_at`, raw-only from
    /// `frame_for_raw`).
    fn frame_with(&self, raw: f32, over: f32, done: bool) -> MorphFrame {
        let raw = raw.clamp(0.0, 1.0);
        let p = self.easing.apply(raw);
        let mut geom = self.from.lerp(self.to, p);
        // r1d §2: after growth, growing dims swell past the target by the
        // measured overshoot factors and decay back over SETTLE_MS.
        if over > 0.0 {
            if self.to.width > self.from.width {
                geom.width = self.to.width * (1.0 + W_OVERSHOOT * over);
            }
            if self.to.height > self.from.height {
                geom.height = self.to.height * (1.0 + H_OVERSHOOT * over);
            }
        }
        // Outgoing: 1 -> 0 over RAW time 0..OUTGOING_FADE_END (unchanged
        // — the burst measurements pinned this wall-clock timing).
        let out_t = smoothstep(0.0, OUTGOING_FADE_END, raw);
        // Incoming: 0 -> 1 over EASED progress INCOMING_FADE_START..1.0
        // (iter18 S1 fix, iter33 start 0.5 -> 0.8): the fade starts
        // when the shell is 80% grown, so content is never sliced by
        // the capsule edge. Keying to eased progress is the point —
        // it IS the shell-width progress. On the quad curve the window
        // opens at raw ~0.55 (~129 ms of 233) — the reference content
        // itself only appears in the last ~30% of the growth poses.
        let in_t = smoothstep(INCOMING_FADE_START, 1.0, p);
        // During an optional measured settle tail progress is eased-pinned
        // at 1.0, so content stays fully opaque while geometry lands.
        MorphFrame {
            width: geom.width,
            height: geom.height,
            radius: geom.radius,
            progress: p,
            raw_progress: raw,
            outgoing_scale: 1.0 + (OUTGOING_SCALE_END - 1.0) * out_t,
            outgoing_opacity: 1.0 - out_t,
            incoming_opacity: in_t,
            incoming_scale: INCOMING_SCALE_START + (1.0 - INCOMING_SCALE_START) * in_t,
            done,
        }
    }

    /// Invert the easing: given eased progress `p`, return the raw time
    /// progress that produces it. Exact for Linear, EaseOutQuad and
    /// EaseOutCubic; for EaseOutBack / Spring (overshooting curves,
    /// never used for the island) returns None and the caller must fall
    /// back to wall-clock. Used by the integrator (S-1, v0.7) to draw the
    /// morph frame matching the PRESENTED geometry: the configure for
    /// a requested size can lag the timer by tens of ms, and
    /// frame_at(now) then draws a later frame's (already-faded)
    /// content into an earlier frame's geometry.
    pub fn raw_for_eased(&self, p: f32) -> Option<f32> {
        let p = p.clamp(0.0, 1.0);
        match self.easing {
            Easing::Linear => Some(p),
            // p = 1 - (1-raw)^2  =>  raw = 1 - sqrt(1 - p). Exact.
            Easing::EaseOutQuad => Some(1.0 - (1.0 - p).sqrt()),
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
            Easing::EaseOutQuad,
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
        // r1d §2 with r1f's overshoot disabled: the growth tops out AT the
        // target. The assertions stay in terms of the factors so restoring
        // the measured 1.1% needs no test edit — at 0.0 they pin the exact
        // target, which is what a bounce-free build must render.
        let f1 = m.frame_at(at(start, MORPH_DURATION_MS));
        let approx = |got: f32, want: f32| (got - want).abs() < 0.02;
        assert!(approx(f1.width, 342.0 * (1.0 + W_OVERSHOOT)), "w {}", f1.width);
        assert!(approx(f1.height, 300.0 * (1.0 + H_OVERSHOOT)), "h {}", f1.height);
        assert_eq!(f1.outgoing_opacity, 0.0);
        assert_eq!(f1.incoming_opacity, 1.0);
        assert_eq!(f1.incoming_scale, 1.0);
        assert!(f1.done, "zero overshoot has no settle tail");
        // Well past the end clamps to the settled state.
        let f3 = m.frame_at(at(start, 900));
        assert_eq!((f3.width, f3.height, f3.radius), (342.0, 300.0, 20.0));
        assert!(f3.done);
        let mut prev = f1.width;
        for ms in MORPH_DURATION_MS..=MORPH_DURATION_MS + 10 {
            let w = m.frame_at(at(start, ms)).width;
            assert!(w <= prev + f32::EPSILON, "settle not monotone at {ms} ms");
            prev = w;
        }
        assert!(prev >= 342.0, "settle must land at the target, not below");
    }

    #[test]
    fn nothing_pops_in_at_t0() {
        // Incoming content is exactly 0 at t=0 and stays 0 until the
        // shell is 80% grown (eased progress 0.8 — iter33; on the quad
        // curve that is raw 0.5528 = ~129 ms of the 233 ms growth).
        let (m, start) = morph_at(Geometry::collapsed(), Geometry::new(342.0, 300.0, 20.0));
        for ms in [0, 10, 25, 40, 50, 60, 66] {
            assert_eq!(
                m.frame_at(at(start, ms)).incoming_opacity,
                0.0,
                "incoming opacity must be 0 at {ms} ms"
            );
        }
        // r1d §2 recheck on the quad curve: incoming stays 0 well past
        // 66 ms now — eased 0.8 is reached at raw 1-sqrt(0.2)=0.5528
        // (~129 ms of 233). Content appears in the last ~44% of the
        // growth, matching the reference poses (content ink only shows
        // in the final ~30% of the growth frames).
        for ms in [80, 100, 120] {
            assert_eq!(
                m.frame_at(at(start, ms)).incoming_opacity,
                0.0,
                "incoming must be 0 at {ms} ms (fade starts ~129 ms)"
            );
        }
        let approx = |got: f32, want: f32| (got - want).abs() < 0.01;
        let expect_at = |ms: u64, want: f32| {
            let got = m.frame_at(at(start, ms)).incoming_opacity;
            assert!(approx(got, want), "{ms} ms: {got} != {want}");
        };
        // smoothstep(0.8, 1, p): quad p(140 ms) = 0.8409 -> 0.108;
        // p(160) = 0.9015 -> 0.512; p(200) = 0.9796 -> 0.972.
        expect_at(140, 0.108);
        expect_at(160, 0.512);
        expect_at(200, 0.972);
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
        // Outgoing fade window is RAW-based: 0..OUTGOING_FADE_END of
        // the 233 ms growth = fully out at ~81.5 ms.
        let mid = m.frame_at(at(start, 90));
        assert_eq!(mid.outgoing_opacity, 0.0);
        // Eased width at 90 ms (raw 0.386) = 90 + 252 * 0.6128 ~ 244.
        assert!(mid.width > 240.0 && mid.width < 250.0, "width {}", mid.width);
        // The incoming content has not started yet anywhere in the
        // outgoing window — content is never sliced by the edge.
        assert_eq!(mid.incoming_opacity, 0.0);
        // Past the outgoing window (90 ms) with the incoming still
        // closed (to ~129 ms): bare frost capsule, no content at all.
        let f110 = m.frame_at(at(start, 110));
        assert_eq!(f110.outgoing_opacity, 0.0);
        assert_eq!(f110.incoming_opacity, 0.0);
        // At 70 ms of 233 the outgoing clock is nearly gone (raw 0.30,
        // smoothstep to 0.945 -> opacity ~0.055) and the incoming
        // content is still 0 — nothing is ever drawn half-sliced.
        let f70 = m.frame_at(at(start, 70));
        assert!(f70.outgoing_opacity > 0.0 && f70.outgoing_opacity < 0.1);
        assert_eq!(f70.incoming_opacity, 0.0);
        // By the growth end the incoming content is fully in.
        let f1 = m.frame_at(at(start, MORPH_DURATION_MS));
        assert_eq!(f1.outgoing_opacity, 0.0);
        assert_eq!(f1.incoming_opacity, 1.0);
    }

    #[test]
    fn mid_morph_grown_and_content_visible() {
        // iter18 S1 fix, iter33 fade start 0.5 -> 0.8, r1d §2 retimed on
        // the quad curve: at 160 ms of the 233 ms growth the panel is
        // 90% of its width grown and the incoming content is at ~0.51 —
        // grown WITH content, inside the capsule, not near-empty and
        // never sliced. Fully opaque by the growth end.
        let (m, start) = morph_at(Geometry::collapsed(), Geometry::new(342.0, 300.0, 20.0));
        let mid = m.frame_at(at(start, 160));
        assert!(mid.width > 300.0, "width {} should be near 342", mid.width);
        assert!(mid.incoming_opacity > 0.3, "incoming {}", mid.incoming_opacity);
        assert_eq!(mid.outgoing_opacity, 0.0);
        let late = m.frame_at(at(start, 200));
        assert!(late.incoming_opacity > 0.8, "incoming {}", late.incoming_opacity);
    }

    #[test]
    fn incoming_fade_is_keyed_to_shell_width() {
        // The iter18 contract, iter33 start 0.5 -> 0.8, pinned on the
        // wallpaper geometry (90 -> 1008, the critic-pickers-iter16 S1
        // case): the fade starts at exactly 80% shell width and ramps
        // with it. r1d §2 retimed on the quad curve: eased 0.5 at
        // raw 0.2929 (~68 ms), eased 0.8 (fade opens) at raw 0.5528
        // (~129 ms).
        let (m, start) = morph_at(Geometry::collapsed(), Geometry::new(1008.0, 270.0, 20.0));
        let approx = |got: f32, want: f32| (got - want).abs() < 0.5;
        // Half-grown (~68 ms): width = 90 + 918 * 0.4985 ~ 547.7, no
        // content yet (eased below the 0.8 key).
        let f68 = m.frame_at(at(start, 68));
        assert!(approx(f68.width, 547.7), "width {} should be half-grown", f68.width);
        assert_eq!(f68.incoming_opacity, 0.0);
        // 129 ms: eased p = 1-(1-0.5536)^2 ~ 0.8008 — just at the key;
        // smoothstep gives ~5e-5 (f32 rounds to 0.0). The fade opens
        // EXACTLY when the shell passes 80% grown.
        let f129 = m.frame_at(at(start, 129));
        assert!(f129.incoming_opacity < 0.01, "fade opens at 80% of the growth");
        // 140 ms: eased 0.8407 -> smoothstep ~0.107 — content is
        // flowing in, width 862 > 80% of 1008.
        let f140 = m.frame_at(at(start, 140));
        assert!(f140.incoming_opacity > 0.0, "fade must be open past 80% grown");
        assert!(f140.width > 824.0, "width {}", f140.width);
        // 80 ms: eased 1-(1-0.343)^2 = 0.568 -> 90 + 918*0.568 ~ 612.
        let f80 = m.frame_at(at(start, 80));
        assert!(approx(f80.width, 611.8), "width {}", f80.width);
    }

    #[test]
    fn incoming_opacity_is_monotonic() {
        // The fade never goes backwards: no flicker mid-morph.
        let (m, start) = morph_at(Geometry::collapsed(), Geometry::new(342.0, 300.0, 20.0));
        let mut prev = 0.0f32;
        for ms in (0..MORPH_DURATION_MS).step_by(10).chain([MORPH_DURATION_MS]) {
            let v = m.frame_at(at(start, ms)).incoming_opacity;
            assert!(v >= prev - f32::EPSILON, "incoming dipped at {ms} ms");
            prev = v;
        }
        assert_eq!(prev, 1.0);
    }

    #[test]
    fn width_is_monotonic_on_default_easing() {
        // Growth phase only (the settle tail DECAYS back by design).
        let (m, start) = morph_at(Geometry::collapsed(), Geometry::new(342.0, 300.0, 20.0));
        let mut prev = 0.0f32;
        for ms in (0..=MORPH_DURATION_MS).step_by(10) {
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
        let mid = col.frame_at(at(start, 160));
        assert!(mid.width < 342.0 && mid.width > 90.0, "width {}", mid.width);
        assert_eq!(mid.outgoing_opacity, 0.0);
        // r1d §2, quad basis: the incoming clock fades in WITH the
        // shrinking shell — ~0.51 at 160 ms, fully in at the 233 ms end
        // (eased window opens at raw 0.5528 on this curve too). No
        // sliced content in either direction.
        let approx = |got: f32, want: f32| (got - want).abs() < 0.01;
        assert!(approx(mid.incoming_opacity, 0.512));
        // Shrinking dims never overshoot: done lands at the growth end.
        let f1 = col.frame_at(at(start, MORPH_DURATION_MS));
        assert_eq!((f1.width, f1.height, f1.radius), (90.0, 24.0, 12.0));
        assert_eq!(f1.outgoing_opacity, 0.0);
        assert_eq!(f1.incoming_opacity, 1.0);
        assert!(f1.done, "shrink morph must be done at growth end");
        let mut prev = f32::MAX;
        for ms in (0..=MORPH_DURATION_MS).step_by(10) {
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
        // Both dims shrink (w) / stay equal (h, r): no settle overshoot,
        // done exactly at the growth end.
        assert!(m.frame_at(at(start, MORPH_DURATION_MS)).done);
        assert_eq!(m.frame_at(at(start, MORPH_DURATION_MS)).width, 54.0);
    }

    #[test]
    fn expanded_to_expanded_interpolates_directly() {
        // Power 400x150 -> calendar 342x300: no collapse to the pill in
        // between; both dimensions move at once.
        let (m, start) = morph_at(
            Geometry::new(400.0, 150.0, 20.0),
            Geometry::new(342.0, 300.0, 20.0),
        );
        let mid = m.frame_at(at(start, 120));
        assert!(mid.width < 400.0 && mid.width > 342.0);
        assert!(mid.height > 150.0 && mid.height < 300.0);
        assert_eq!(mid.radius, 20.0);
        // r1d §2: width shrinks (no settle), height grows -> the settle
        // tail swells HEIGHT only; done waits for the tail. r1f: the swell
        // factor is 0, so the top-out height IS the target (was asserted
        // strictly greater than it, which can only hold with overshoot on).
        let top = m.frame_at(at(start, MORPH_DURATION_MS));
        assert!(top.height >= 300.0, "height {} must be at the top-out", top.height);
        let approx = |got: f32, want: f32| (got - want).abs() < 0.02;
        assert!(approx(top.height, 300.0 * (1.0 + H_OVERSHOOT)));
        assert_eq!(top.width, 342.0, "shrinking dim must NOT overshoot");
        assert!(top.done, "zero overshoot has no settle tail");
        let done = m.frame_at(at(start, MORPH_DURATION_MS));
        assert_eq!((done.width, done.height), (342.0, 300.0));
        assert!(done.done);
    }

    #[test]
    fn restart_from_current_never_jumps() {
        // A command at 80 ms restarts from the interpolated frame, not
        // from the stale nominal size of the old state.
        let (m, start) = morph_at(Geometry::collapsed(), Geometry::new(342.0, 300.0, 20.0));
        // 200 ms of 233 on the quad: eased 0.980 — a near-grown frame.
        let now = at(start, 200);
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
        // During growth the two entry points agree exactly: settle_t is
        // 0 on both paths, and done is false on both.
        for ms in [0, 20, 40, 80, 120, 200] {
            let a = m.frame_at(at(start, ms));
            let b = m.frame_for_raw(ms as f32 / MORPH_DURATION_MS as f32);
            assert_eq!(a, b, "mismatch at {ms} ms");
        }
        // frame_for_raw clamps at raw 1.0 and reports the GROWTH-only
        // endpoint (overshoot envelope only enters via frame_at's
        // settle clock): geometry = target, done reflects "would be
        // done if there were no settle" = false for a growing morph.
        let raw1 = m.frame_for_raw(1.0);
        assert_eq!((raw1.width, raw1.height, raw1.radius), (342.0, 300.0, 20.0));
        assert!(m.frame_at(at(start, 900)).done);
    }

    #[test]
    fn raw_for_eased_inverts_ease_out_quad() {
        // S-1 (v0.7): the drawn frame must match the PRESENTED width.
        // Inverting the eased progress recovered from a width must
        // return the raw progress the geometry was requested at.
        let (m, start) = morph_at(Geometry::collapsed(), Geometry::new(342.0, 300.0, 20.0));
        for ms in [0, 10, 20, 40, 56, 80, 100, 120, 140, 200] {
            let f = m.frame_at(at(start, ms));
            let raw = ms as f32 / MORPH_DURATION_MS as f32;
            let back = m.raw_for_eased(f.progress).expect("quad must invert");
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
        // Ease-out = fast start: at any instant the eased fraction is
        // AHEAD of the raw fraction. At 40 ms of 233 (raw 0.1717) the
        // quad curve has the panel 31-32% of the way grown.
        let (m, start) = morph_at(Geometry::collapsed(), Geometry::new(342.0, 300.0, 20.0));
        let f = m.frame_at(at(start, 40));
        let grown = (f.width - 90.0) / (342.0 - 90.0);
        let raw = 40.0 / MORPH_DURATION_MS as f32;
        assert!(
            grown > raw + 0.10,
            "ease-out must be well ahead of raw at 40 ms: grown {grown}, raw {raw}"
        );
        let approx = |got: f32, want: f32| (got - want).abs() < 0.01;
        assert!(approx(grown, 1.0 - (1.0 - raw) * (1.0 - raw)));
    }
}
