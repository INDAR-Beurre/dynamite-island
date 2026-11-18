//! Lock icon + collapsed-pill clock for dynamite-rust.
//!
//! Drop-in module: add `mod lock_timer;` to `main.rs` — no new crates,
//! no new state-machine variant, no extra allocation at runtime.
//! Everything here renders into the same wl_shm Argb8888 canvas `main.rs`
//! already owns (BGRA bytes, premultiplied), using the same AA convention
//! (`alpha = (0.5 - sdf).clamp(0, 1)`, white glyphs composited with `max()`).
//!
//! # (a) Lock icon — `draw_lock_icon()`
//!
//! Video ground truth (0:00, `analysis/frames-0-30s.md`): "tiny black pill
//! ~40px wide (in 1350px player) w/ white lock icon" → ~57 px @1080p, i.e.
//! the 54x24 `IslandState::Lock` pill in `main.rs`. The video crop
//! (`frames/crops/lockpill-video.png`) is too upscaled/noisy to resolve the
//! outline — only a small centred bright core survives thresholding — so the
//! glyph follows the Material Symbols Rounded "lock" proportions (the same
//! glyph family the Quickshell reference renders via its icon font).
//! All coordinates below are design-space px in a 54x24 canvas, centre
//! (27, 12). The icon is drawn from two SDF shapes + one SDF cutout:
//!
//! | part     | shape (SDF)                                  | spans (x, y)        |
//! |----------|------------------------------------------------|-----------------------|
//! | shackle  | ring = outer round-box MINUS inner round-box | x 20..34, y 4..14    |
//! |          |   outer: centre (27, 9), half (7, 5), r 4    | stroke 2.0 all round  |
//! |          |   inner: centre (27, 10.5), half (5, 4.5), r 3|                     |
//! | body     | round-box centre (27, 15.5), half (7.25, 4.5), r 2.5 | x 19.75..34.25, y 11..20 |
//! | keyhole  | CUTOUT: circle centre (27, 14.3) r 1.5,     | dot y 12.8..15.8    |
//! |          |   + stem round-box centre (27, 16.3), half (0.8, 1.5), r 0.8 | stem y 14.8..17.8 |
//!
//! Resulting icon bbox: **x 19.75..34.25 (w 14.5), y 4..20 (h 16)**,
//! centre (27, 12) = pill centre, height ratio 16/24 = 0.667. The body
//! (x 19.75..34.25) is wider than the ring (x 20..34), so the ring's bottom
//! edge (y 14) is fully hidden inside the body — no stray nubs. The keyhole
//! is cut back to the black capsule (coverage multiplied by `1 - key`).
//! If the pill size ever changes, the icon scales with
//! `s = min(w / 54, h / 24)` about the canvas centre.
//!
//! # (b) There is NO timer — the video's "00:05" is the wall clock
//!
//! DECIDED iter15 (`iterations/decision-timer-semantics-iter15.md`):
//! the collapsed pill reads "00:05" in all 16 collapsed frames over
//! 30 s, and the control-centre header shows the same "00:05" next to
//! "81%" battery — both are the live wall clock `HH:MM`, not a
//! stopwatch. The old count-up stopwatch mode (MM:SS from daemon
//! start) was a misreading and was DELETED in iter16, with its
//! formatters, size alias, the "timer" socket word and the 1 s tick
//! in main.rs. The collapsed pill shows the live clock only. Do not
//! re-add a stopwatch without a decision doc.
//!
//! # CLOCK_PX verification — 13.0 FAILS, 17.0 is the value
//!
//! Font: Quicksand variable TTF, sha256 39c9b64223561f56a… (same file as on
//! the Surface). unitsPerEm = 1000, OS/2 sCapHeight = 700. Digit outline
//! bbox heights (font units, default instance wght 300 — the instance
//! fontdue loads): '0' 720, '1' 701, '2' 710, '3' 705, '4' 700, '5' 693,
//! '6' 737 (tallest), '7' 697, '8' 710, '9' 722; mean ≈ 711, i.e. a digit
//! is ≈ 0.71 em tall (cap height 0.70 + overshoot).
//!
//! fontdue `rasterize(ch, px)` treats `px` as the em size in buffer px and
//! the glyph bitmap height is ≈ ceil(bbox_units / 1000 × px) (±1 row for
//! AA alignment). Buffer px == logical px here (buffer_scale 1, mango
//! upscales ×1.5), so the critic's logical-pixel ratio applies directly.
//!
//! - At CLOCK_PX = 13.0: '0' → 720 × 13 / 1000 = 9.36 → 10 bitmap rows;
//!   measured (white-threshold, AA edge rows drop out) ≈ 8.6–9.4 rows,
//!   ratio ≈ 0.36–0.39 of the 24 px pill. Critic A measured 0.36.
//!   **FAILS the 0.50 target (12 logical px).**
//! - Solve for 12.0 px: px = 12 / 0.720 = 16.67 ('0'); 12 / 0.700 = 17.14
//!   (cap height); 12 / 0.737 = 16.28 (tallest digit '6') → **px ≈ 17.0**.
//! - At 17.0: '0' → 12.24 → 12–13 bitmap rows, measured 12.0–12.7,
//!   ratio 0.50–0.53. Independent confirmation: the Quickshell track
//!   (iter 5) needed pixelSize 17 for digit 12.0, ratio 0.500.
//! - Fit check at 17.0: line height = (1000 + 250) / 1000 × 17 = 21.25 px
//!   < 24 px pill ✓. Baseline = (24 − 21.25) / 2 + 17 = 18.375; '0' spans
//!   y ≈ 6.3..18.5, centre ≈ 12.4 — 0.4 below pill centre, inside the
//!   critic's ±0.5 tolerance (sub-logical nudges quantise to physical rows
//!   at scale 1.5, so do not chase it).
//!
//! => `CLOCK_PX` / `TEXT_PX` = **13.4** with Inter (iter126): Inter's
//! cap height is 1490/2048 em = 0.7275, so 13.4 px gives 9.75 px of digit
//! ink in the 28 px pill = ratio 0.348, inside the reference's 0.333 ± 0.03.
//! The old 17.0 was derived for Quicksand's 0.70 cap in a 24 px pill
//! (ratio 0.50 — 46% oversized against the reference).
//!
//! # (c) Wiring into main.rs
//!
//! ```ignore
//! mod lock_timer; // top of main.rs, next to the other `mod`s
//!
//! // draw(): the Collapsed foreground is just the live clock:
//! IslandState::Collapsed => lock_timer::draw_collapsed(
//!     canvas, w, h, font, &hhmm()),
//! // (The capsule loop stays first; the old `if text.is_empty() { return }`
//! // early-return must NOT skip the Lock branch — draw the icon after it.)
//!
//! // Control socket: "clock" is an accepted word (no-op alias for
//! // "collapsed" — the pill always shows the live clock); "timer" is
//! // rejected as an unknown word. The minute-boundary timer in main.rs
//! // redraws the collapsed pill when the minute rolls over; there is
//! // no 1 s tick — the clock needs no per-second redraw.
//! ```
//!
//! CLOCK_PX: 13.0 -> lock_timer::TEXT_PX (13.4).

// ---------------------------------------------------------------------------
// Sizes
// ---------------------------------------------------------------------------

/// Design canvas of the lock pill (`IslandState::Lock` in main.rs).
pub const LOCK_W: u32 = 54;
pub const LOCK_H: u32 = 24;

/// Clock size for the collapsed pill (iter126): Inter cap height
/// 1490/2048 em × 13.4 px = 9.75 px ink in a 28 px pill (ratio 0.348,
/// reference 0.333 ± 0.03). Was 17.0 for Quicksand's 0.70 cap in 24 px.
pub const TEXT_PX: f32 = 13.4;
/// Negative tracking between glyphs, logical px (reference is tight;
/// the old advance-width layout read loose at display sizes).
pub const TRACKING: f32 = -0.4;
/// Alias kept so call sites can name the intent.
pub const CLOCK_PX: f32 = TEXT_PX;

/// Expected lock-icon bbox in design-space px (54x24 canvas), for the
/// geometry measurer / harsh critics: (x0, y0, x1, y1).
/// Icon: x 19.75..34.25 (w 14.5), y 4..20 (h 16), centre (27, 12).
pub const ICON_BBOX: (f32, f32, f32, f32) = (19.75, 4.0, 34.25, 20.0);

// ---------------------------------------------------------------------------
// SDF helpers (same AA convention as main.rs: alpha = (0.5 - d).clamp(0, 1))
// ---------------------------------------------------------------------------

/// Signed distance to an axis-aligned rounded box.
/// `(cx, cy)` centre, `(hx, hy)` half-sizes, `r` corner radius.
fn sd_round_box(px: f32, py: f32, cx: f32, cy: f32, hx: f32, hy: f32, r: f32) -> f32 {
    let qx = (px - cx).abs() - (hx - r);
    let qy = (py - cy).abs() - (hy - r);
    let ax = qx.max(0.0);
    let ay = qy.max(0.0);
    (ax * ax + ay * ay).sqrt() + qx.max(qy).min(0.0) - r
}

fn sd_circle(px: f32, py: f32, cx: f32, cy: f32, r: f32) -> f32 {
    ((px - cx) * (px - cx) + (py - cy) * (py - cy)).sqrt() - r
}

/// 1 px anti-aliased edge, same as main.rs `rounded_rect_alpha`.
fn aa(d: f32) -> f32 {
    (0.5 - d).clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// (a) Lock icon
// ---------------------------------------------------------------------------

/// White coverage (0..1) of the padlock glyph at design-space point (x, y)
/// in a 54x24 canvas. Union of shackle-ring + body, minus the keyhole.
/// Pure function of the coordinates — unit-testable without Wayland.
pub fn lock_icon_alpha(x: f32, y: f32) -> f32 {
    // Shackle: ring = outer round-box minus inner round-box (2.0 stroke).
    let d_outer = sd_round_box(x, y, 27.0, 9.0, 7.0, 5.0, 4.0);
    let d_inner = sd_round_box(x, y, 27.0, 10.5, 5.0, 4.5, 3.0);
    let d_ring = d_outer.max(-d_inner);
    // Body: x 19.75..34.25, y 11..20, r 2.5. Wider than the ring, so the
    // ring's bottom edge (y 14) hides inside the body.
    let d_body = sd_round_box(x, y, 27.0, 15.5, 7.25, 4.5, 2.5);
    let d_lock = d_ring.min(d_body);
    // Keyhole cutout: dot + stem, punched back to the black capsule.
    let d_dot = sd_circle(x, y, 27.0, 14.3, 1.5);
    let d_stem = sd_round_box(x, y, 27.0, 16.3, 0.8, 1.5, 0.8);
    let d_key = d_dot.min(d_stem);
    aa(d_lock) * (1.0 - aa(d_key))
}

/// Draw the padlock into `canvas` (Argb8888, BGRA bytes, same convention as
/// main.rs `draw()`). The black capsule must already be drawn; this only
/// adds white icon pixels (keyhole pixels are left as the black capsule).
/// `w`/`h` are the canvas size — normally 54x24; the icon scales about the
/// canvas centre if the pill size ever changes.
pub fn draw_lock_icon(canvas: &mut [u8], w: u32, h: u32) {
    if w == 0 || h == 0 {
        return;
    }
    // Design-space (54x24) -> canvas-space transform.
    let s = (w as f32 / LOCK_W as f32).min(h as f32 / LOCK_H as f32);
    let ox = w as f32 / 2.0 - 27.0 * s;
    let oy = h as f32 / 2.0 - 12.0 * s;
    // Icon bbox in canvas px, with a 2 px AA margin.
    let x0 = ((ox + ICON_BBOX.0 * s - 2.0).floor() as i32).max(0);
    let x1 = ((ox + ICON_BBOX.2 * s + 2.0).ceil() as i32).min(w as i32 - 1);
    let y0 = ((oy + ICON_BBOX.1 * s - 2.0).floor() as i32).max(0);
    let y1 = ((oy + ICON_BBOX.3 * s + 2.0).ceil() as i32).min(h as i32 - 1);
    for py in y0..=y1 {
        for px in x0..=x1 {
            // Sample at the pixel centre, in design space.
            let lx = (px as f32 + 0.5 - ox) / s;
            let ly = (py as f32 + 0.5 - oy) / s;
            let cov = lock_icon_alpha(lx, ly);
            if cov <= 0.0 {
                continue;
            }
            let i = ((py as u32 * w + px as u32) * 4) as usize;
            // White over black, same max() convention as main.rs glyph blit.
            let v = (cov * 255.0) as u8;
            canvas[i] = canvas[i].max(v);
            canvas[i + 1] = canvas[i + 1].max(v);
            canvas[i + 2] = canvas[i + 2].max(v);
            canvas[i + 3] = 255;
        }
    }
}

/// Centre `text` in the canvas at `px` size, Quicksand via fontdue.
/// Same layout/blit algorithm as main.rs `draw()` (baseline centred on
/// the line box, glyphs composited white with max()). The black capsule
/// must already be drawn.
pub fn draw_text_centred(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &fontdue::Font,
    text: &str,
    px: f32,
) {
    if text.is_empty() || w == 0 || h == 0 {
        return;
    }
    let (wf, hf) = (w as f32, h as f32);
    // Layout: total advance width + line metrics (duplicated from main.rs
    // on purpose — this module must stay drop-in with no cross-refs).
    let mut glyphs: Vec<(fontdue::Metrics, Vec<u8>)> = Vec::new();
    let mut width = 0.0f32;
    for ch in text.chars() {
        let (m, cov) = font.rasterize(ch, px);
        width += m.advance_width;
        glyphs.push((m, cov));
    }
    // Tighten tracking between glyphs (not after the last one).
    let tracking = TRACKING * (px / TEXT_PX);
    if glyphs.len() > 1 {
        width += tracking * (glyphs.len() - 1) as f32;
    }
    let lm = match font.horizontal_line_metrics(px) {
        Some(lm) => lm,
        None => return,
    };
    let height = lm.ascent - lm.descent;
    let baseline = (hf - height) / 2.0 + lm.ascent;
    let mut pen = (wf - width) / 2.0;
    for (gi, (m, cov)) in glyphs.iter().enumerate() {
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
                let i = (((px_y as u32) * w + px_x as u32) * 4) as usize;
                let v = (c * 255.0) as u8;
                canvas[i] = canvas[i].max(v);
                canvas[i + 1] = canvas[i + 1].max(v);
                canvas[i + 2] = canvas[i + 2].max(v);
                canvas[i + 3] = 255;
            }
        }
        pen += m.advance_width;
        if gi + 1 < glyphs.len() {
            pen += tracking;
        }
    }
}

/// Collapsed-pill foreground: the live wall clock "hh:mm" only.
/// `clock_text` is the caller-supplied wall-clock string (main.rs `hhmm()`).
/// Draw AFTER the capsule loop in main.rs `draw()`. There is no timer
/// mode (iter15 decision, iter16 deletion): the video's "00:05" is 00:05 AM.
pub fn draw_collapsed(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &fontdue::Font,
    clock_text: &str,
) {
    draw_text_centred(canvas, w, h, font, clock_text, TEXT_PX)
}
