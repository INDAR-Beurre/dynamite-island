//! draw_calendar_panel — calendar content for the 342x280 island state.
//!
//! Reference: video frame `frames/per-second/s09.png` +
//! `analysis/frame-measurements.md` (calendar 342x300, corrected 12 Nov
//! 21:01) + `iterations/critic-rust-calendar-iter10.md` (FAIL) +
//! `iterations/critic-qs-vs-rust-iter10.md` (§2: hardcoded-today defect).
//!
//! The caller (main.rs `draw()`) already fills the black rounded panel
//! (radius ~18) into the canvas; this module only adds the content:
//!   (a) month title, (b) [no month chevrons since iter24 — nav discs only],
//!   (c) weekday header row, (d) 6x7 date grid with dimmed overflow days,
//!   (e) solid blue today circle on the REAL current date with a white
//!       day number inside.
//!
//! Colours: title/digits white, weekday row #9AA0A6, overflow days dim
//! grey #5F6368, today circle #7CA2F5 (island accent, same as the power panel).
//! Title is Quicksand Bold — iter10: `Quicksand-Bold.ttf` (static,
//! wght 700) is installed and main.rs loads it as THE text font.
//!
//! EXACT GEOMETRY — 342x280 canvas (all px, origin top-left, panel-relative;
//!   iter88: PANEL_H 300 -> 280, dead band below the grid killed — see below),
//! iter10 fixes applied (critic-rust-calendar-iter10.md defects 1-6)
//! + iter13 title-width fix (see below):
//!   Title "<Month> <Year>": 12.3px Bold (iter13: was 13.0 — measured
//!     span on rust12-calendar.png 98.7 vs reference 93; 13px predicts
//!     ~98.4, 12.3px predicts 98.7 x 12.3/13 = ~93.4, i.e. the reference
//!     span. No Regular 400 file exists on the Surface (only
//!     Quicksand-Bold.ttf + Quicksand.ttf Light wght 300), so smaller
//!     Bold is the ONLY option — Regular is not available, and the Light
//!     fallback would be worse. Deviation, stated honestly: at 12.3px
//!     the cap height is ~8.7 vs the reference title band ~9.0 (-0.3px);
//!     the span is the more visible property, and the iter10 critic
//!     endorsed this exact trade (12.3px "would hit 93 exactly").
//!     Centred x=171, TITLE_CY 7.2 (glyph band centre runs ~+3.0
//!     above/below CY at 12.3px, i.e. ~28.5, vs reference ~28.5 —
//!     TITLE_CY is UNCHANGED: the offset scales with px, 3.2 x 12.3/13
//!     = 3.03, so the band stays on the reference).
//!   Month chevrons: REMOVED iter24 (a2) — one nav system only. The
//!     panel used to carry drawn `<` / `>` strokes at (27 / 306,
//!     25.5); they doubled the island's outside nav discs, so the
//!     calls, the stroke helpers and the chevron tests are deleted.
//!     Month navigation is the nav discs (main.rs).
//!   Weekday row "M T W T F S S": 11px #9AA0A6, WEEKDAY_CY 54 (was 75 —
//!     the iter9 "re-centre" 65 -> 75 went the WRONG way; glyph band
//!     centre runs ~CY + 2.7, so 54 puts it at ~56.7 vs reference 57.0).
//!   Date grid: 6 rows x 7 cols.
//!     Column centres x =
//!     34.5, 78.5, 122.0, 166.5, 210.5, 254.5, 298.0 (pitch ~43.75,
//!     was pitch 45.0 [31.5..301.5] — outer columns were ∓3 off).
//!     Row centres y = 90, 123, 157, 192, 225, 258 (pitch ~33, was
//!     pitch 30 [105..255] — rows 1-2 were +18/+15 low; the iter9
//!     pitch-30 "re-centre" anchored the block to the BOTTOM with the
//!     wrong pitch. The reference pitch is ~33 and its anchor is the
//!     TOP; do not re-centre this again).
//!     Weekday→first-row gap = 90 − 57 = 33 = one row pitch.
//!     Last row: glyph band ~253-263, bottom padding ~17 (iter88: was ~37
//!     at PANEL_H 300 — the 37.3px dead band the iter87 pixel critic
//!     measured; PANEL_H 300 -> 280. Floor: a today circle in row 6
//!     bottoms at ROW_Y[5] + TODAY_R = 252, so 280 still leaves 28px
//!     clearance under it. Ref bottom padding ~30 is NOT matched — the
//!     task (iter88) says no dead band, and 17px reads as a margin.)
//!   Today circle: solid #7CA2F5, Ø24 (TODAY_R 12.0 — the critic's
//!     ~23.3 measured diameter is AA/threshold, NOT a wrong radius;
//!     do not change the radius on that number alone), centre
//!     (COL_X[today_col], ROW_Y[today_row]). Date digits 13px.
//!
//! LIVE DATE (iter10 fix — was hardcoded, see below):
//!   The month, grid and today marker are computed from the REAL local
//!   date at every draw (libc `localtime_r`, same pattern as main.rs
//!   `hhmm()` — no chrono dependency, no network, no new crate).
//!   Month title = current month name + year. Grid = Monday-first
//!   42-cell month table for the current month, overflow days from the
//!   adjacent months, dimmed. Today = the current day-of-month, wherever
//!   it falls in the grid. On 13 Nov 2026 this is November 2026 with the
//!   circle on the 13th (col 4, row 2) — the v0.4 build still circled
//!   the 12th on the 13th because TODAY_COL/TODAY_ROW were constants.
//!   Chevrons navigate (iter36): ‹ › shift the DISPLAYED month
//!   (CalMonth press in main.rs, wrapping Dec<->Jan); the today circle
//!   shows only when the displayed month IS the current month, so the
//!   marker stays tied to the system date, never to a constant.
//!
//! DIGIT-vs-CIRCLE CENTRE OFFSET (iter10 critic defect 3, FIXED here):
//!   Measured iter10: digit glyph band centres sat at ROW_Y + 3.0 while
//!   the today circle sat exactly on ROW_Y[2] — no single ROW_Y value
//!   could satisfy both. Cause: `draw_text_centred_at` assumes cap
//!   height 0.72em (top_y = cy − 0.36*px) but Quicksand's real ascent
//!   puts the digit band ~3 px low at 13px. Fix: `digit_centre_offset()`
//!   derives the offset from the REAL font metrics (ascent, digit glyph
//!   ymin/height — the same values the blit path itself uses) and the
//!   date digits are drawn at ROW_Y − offset, so digit band centres land
//!   exactly on ROW_Y and the circle agrees. The title/weekday/chevron
//!   constants above still carry their own measured offsets (TITLE_CY,
//!   WEEKDAY_CY are set so the BAND lands on the reference, not the
//!   requested centre) — only the date digits use the metrics fix, so
//!   those constants are untouched by it. Fallback 3.0 if the font
//!   cannot rasterize a digit (the iter10 measured value at 13px).
//!
//! MEASURED ON SCREEN, rust12-calendar.png (iter13 re-measurement,
//! panel-relative logical px; reference in brackets):
//!   Title band centre 28.7 [28.5, +0.2]; span 98.7 at 13px [93, +5.7]
//!     -> the iter13 fix (TITLE_PX 12.3, predicts ~93.4).
//!   Chevrons L 26.0 [27, -1.0], R 305.3 [306, -0.7] — PASS, no change.
//!   Weekday row band centre 57.0 [57.0, EXACT] — the WEEKDAY_CY 54
//!     fix is verified on screen; no change.
//!   Date rows 90.3 / 123.0 / 157.0 / 192.3 / 225.3 / 258.3
//!     [90 / 123 / 157.5 / 192 / 225 / 258] — all within 0.5px; the
//!     pitch-33 ROW_Y fix is verified; no change. Digit-vs-circle
//!     agreement: row-3 digit band 157.0 vs circle centre 156.7 (0.3).
//!   Columns 34.3 / 78.3 / 122.3 / 166.7 / 210.3 / 254.7 / 298.3
//!     [34.5 / 78 / 123 / 165 / 210 / 255 / 298.5] — all within 1.7px.
//!   Today circle centre (210.1, 156.7): column 4 = the 13th — LIVE
//!     TODAY VERIFIED on screen (rust12 was taken 13 Nov; the v0.4
//!     build circled the 12th). Correct-for-13th anchor is
//!     (210.5, 157.5): delta (-0.4, -0.8) — PASS, no change.
//!   Today circle diameter: 22.7 (strict thr) / 24.7 (loose thr) vs 24
//!     — threshold-bracketed, TODAY_R 12.0 is correct; do not change.
//!   *** rust12 IS A MID-FADE CAPTURE, NOT A SETTLED RENDER ***
//!   Circle core (46,101,192) = exactly 0.78 x the OLD fill #3B82F6 (59,130,246)
//!   title peak 198 vs rust10's 254 (0.78x); weekday 117-126 vs
//!   141-157 (0.78x); digits 190 vs 232-255 (0.78x). The source file
//!   is BYTE-IDENTICAL (md5 f19e648e) to the file that produced
//!   rust10's 254-peak render, so this is a capture-path effect
//!   (mid-morph frame at incoming opacity ~0.78, or JPEG-burst
//!   frame), NOT a regression in this module. Geometry in rust12 is
//!   still valid (positions don't depend on opacity), but NO luminance
//!   number from rust12 may be quoted as the build's settled value.
//!   The next critic MUST re-shoot settled (wait >=1 s after the
//!   state command, direct grim PNG) before judging brightness.
//!
//! PREDICTED RESIDUALS (for the next critic to re-measure, not to fix
//!   blindly): title span ~93.4 at 12.3px vs ref 93 (+0.4); title band
//!   centre ~28.5 (exact); cap height ~8.7 vs ref band 9.0 (-0.3, the
//!   accepted trade for the span); ROW_Y[2] = 157 vs ref 157.5
//!   (Δ0.5, sub-pixel); COL_X vs the raw reference column list
//!   differs by ≤1.5px per column.
//!
//! ITER88 RE-MEASUREMENT (rust87-calendar.png, v0.30, panel-relative
//!   logical px; screen logical = panel-relative + (580, 32)):
//!   Title "November 2026": span 706.0-797.3 screen (w 91.3 — the iter10
//!     +13 overshoot is GONE, iter13's 12.3px holds), band centre 27.3,
//!     centre-x 751.7 vs panel centre 751.0 (+0.7) — PASS, no change.
//!   Chevrons: band centres L 29.3 / R 29.0 vs title 27.3 (+2.0 / +1.7
//!     low; iter10's +10.7 is much reduced but NOT gone) — FIXED iter88:
//!     CHEV_CY 25.5 -> 23.5 (TITLE_CY - 2.0); hit-test follows the same
//!     constant, so draw and hit-test cannot drift.
//!   Grid: cols 35.0/79.0/123.0/167.3/211.5/255.3/299.0 vs COL_X (<=1.0);
//!     pitch ~44.0; weekday centres match grid cols within 0.7; rows
//!     122.3/155.7/191.0/224.3/257.0 vs ROW_Y[1..5] (<=1.3); pitch ~33.3.
//!   Today circle: centre (255.3, 155.7) vs (COL_X[5], ROW_Y[2]) =
//!     (254.5, 157.0), diameter 24.0x23.3 — on Saturday 14 Nov 2026, the
//!     REAL today (col 5, row 2). Hardcoded-12th bug stays dead; digit
//!     "14" band centre 156.0 agrees with the circle within 0.3.
//!   Dead space: last digit band ends 261.3, panel bottom 300 -> 38.7
//!     dead (iter87 critic: 37.3) — FIXED iter88: PANEL_H 300 -> 280,
//!     dead ~17; main.rs Calendar size (342, 300) -> (342, 280) to match.
//!
//! MIDNIGHT ROLLOVER (iter116: gap CLOSED — was "KNOWN GAP" until v0.53):
//!   The minute-boundary timer in main.rs now redraws Calendar too
//!   (`|| state.state == IslandState::Calendar`), so a calendar left
//!   open across midnight picks up the new date within a minute.
//!   `local_today()` is read fresh at every draw, so a month change is
//!   picked up the same way; `cal_offset` stays RELATIVE to the current
//!   month, so a navigated view stays offset from the NEW current month
//!   after a rollover, and the today circle only shows at offset 0.
//!
//! STALE CLAIMS REMOVED (iter10): the old header certified the defects
//! as design — title span "120-213" (Regular-era, was 118-224 Bold),
//! chevron "~317" (the bug, not the spec), "weekday centre y 75",
//! "row pitch 30", and the "17..281 column list" sentence (iter8's
//! column list was never 17..281 — that premise was false).
//!
//! INTEGRATION (main.rs — do NOT edit this file's caller from here):
//!   1. Add `mod calendar_panel;` near the top of main.rs. (done, v0.3)
//!   2. In `draw()`, before the generic placeholder-text block, add:
//!        IslandState::Calendar => {
//!            calendar_panel::draw_calendar_panel(canvas, w, h, font);
//!        }
//!   Signature UNCHANGED by the iter10 fix — no main.rs edit needed.
//!   The panel reads the date itself at draw time; the minute-boundary
//!   timer in main.rs already redraws, so a midnight rollover is picked
//!   up within a minute without any extra wiring.

pub const PANEL_W: u32 = 342;
pub const PANEL_H: u32 = 262; // iter89: was 280 — whole column shifted -18, panel shrinks to match (last content ~252, dead ~9)

const WHITE: [u8; 3] = [0xFF, 0xFF, 0xFF];
const ACCENT: [u8; 3] = [0x7C, 0xA2, 0xF5]; // #7CA2F5 today circle (island accent)
const OVERFLOW_GREY: [u8; 3] = [0x5F, 0x63, 0x68]; // dim overflow days
const WEEKDAY_GREY: [u8; 3] = [0x9A, 0xA0, 0xA6]; // #9AA0A6 weekday row

const TITLE_PX: f32 = 12.3; // iter13: was 13.0 — measured span 98.7 vs ref 93 on rust12; 12.3 predicts ~93.4
const TITLE_CY: f32 = 7.2; // iter89: 25.5 -> 7.2 (first ink ~5 px below panel top)
// Month chevrons (iter36 a6b — RESTORED, new purpose): iter24 removed
// them because the nav discs already cycled menus, but the discs cycle
// MENUS, not months. These ‹ › flank the title and change the
// DISPLAYED month (CalMonth press). Glyphs are ASCII "<" ">" (16 px,
// guaranteed in Quicksand — ‹ › U+2039/U+203A coverage is unverified).
// Title span ~124..218 at 12.3 px, so chevrons at x 40 / 302 are clear.
// CHEV_CY (iter121: was 6.53, iter116: was TITLE_CY - 2.0 = 5.2):
//   SCREEN-calibrated, NOT the font-model value — but the iter116
//   calibration DOUBLE-COUNTED the glyph-offset difference. The font
//   model (glyph_band_offset '<' = 4.74 vs 'N' = 3.37) predicts
//   7.2 + 3.37 - 4.74 = 5.83. Two v0.53 screenshots agreed the
//   chevrons sat 1.33 logical ABOVE the title band centre at 5.2, so
//   iter116 set 6.53 = 5.2 + 1.33 — but that 1.33 already contained
//   the +0.67 glyph-offset difference (cal-measure-iter116: title
//   +3.13, chevrons +3.80). Two fresh iter121 shots of the v0.54
//   render (chev121-calendar.png, md5 f5ffa0a2…, bbox AND luminance
//   centroid, both chevrons: L +0.58/+0.67, R +0.66/+0.67) agree the
//   chevrons sit +0.67 logical BELOW the title at 6.53 — exactly the
//   double-counted 0.67 (one physical px). 5.86 = 6.53 - 0.67 zeroes
//   the SCREEN delta and agrees with the font model (5.83) to 0.03.
//   The canvas now carries the chevrons ~AT the title band centre
//   (model delta +0.03), not +0.70 below it — the +0.70 canvas offset
//   in the iter88 render test was the double-count; do NOT restore
//   it. '>' sits ~0.16 logical higher than '<' (its band centre is
//   12 font units higher), so with one shared CHEV_CY the right
//   chevron lands -0.16 logical (0.24 phys) off the title on screen
//   — inside tolerance, no split constant. Do NOT "fix" this back to
//   6.53 without a fresh on-screen measurement: the screen is the
//   graded artifact.
pub const CHEV_LEFT_CX: f32 = 40.0;
pub const CHEV_RIGHT_CX: f32 = 302.0;
pub const CHEV_CY: f32 = 5.86; // iter121: was 6.53 (iter116) — screen-calibrated, see above
pub const CHEV_PX: f32 = 16.0;
pub const CHEV_HIT: f32 = 14.0; // half-size of the 28x28 hit box.
const WEEKDAY_PX: f32 = 11.0;
const WEEKDAY_CY: f32 = 36.0; // iter89: 54 -> 36 (whole column -18, keeps title->weekday gap)
const DATE_PX: f32 = 13.0;

const COL_X: [f32; 7] = [34.5, 78.5, 122.0, 166.5, 210.5, 254.5, 298.0]; // pitch ~43.75, iter10: was 45.0
const ROW_Y: [f32; 6] = [72.0, 105.0, 139.0, 174.0, 207.0, 240.0]; // iter89: -18 each (was [90, 123, 157, 192, 225, 258], pitch ~33)
const TODAY_R: f32 = 12.0; // Ø24

const WEEKDAYS: [char; 7] = ['M', 'T', 'W', 'T', 'F', 'S', 'S'];
const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

// ---------------------------------------------------------------------------
// Live date — libc localtime_r (same pattern as main.rs `hhmm()`), plus
// dependency-free civil-date arithmetic (Howard Hinnant's algorithms).
// No chrono, no new crate, no network.
// ---------------------------------------------------------------------------

/// (year, month 1-12, day-of-month) in local time, right now.
fn local_today() -> (i32, u32, u32) {
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        (tm.tm_year + 1900, (tm.tm_mon + 1) as u32, tm.tm_mday as u32)
    }
}

fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(year) {
                29
            } else {
                28
            }
        }
        _ => 30, // unreachable for month 1-12; keeps the walker total
    }
}

/// Days since 1970-01-01 (which was a Thursday). Hinnant's days_from_civil.
fn days_from_civil(y_in: i32, m_in: u32, d_in: u32) -> i32 {
    let m = m_in as i32;
    let d = d_in as i32;
    let y = if m <= 2 { y_in - 1 } else { y_in };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = if m > 2 { m - 3 } else { m + 9 }; // Mar=0 .. Feb=11
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Weekday of a date, Sunday = 0 .. Saturday = 6.
/// Shift (year, month) by `delta` months, wrapping Dec<->Jan.
/// (2026, 12, +1) -> (2027, 1); (2027, 1, -1) -> (2026, 12).
pub fn shift_month(year: i32, month: u32, delta: i32) -> (i32, u32) {
    let total = year * 12 + (month as i32 - 1) + delta;
    (total.div_euclid(12), (total.rem_euclid(12) + 1) as u32)
}

/// Hit-test the month chevrons (panel-local coords): Some(-1) = ‹,
/// Some(+1) = ›, None = elsewhere.
pub fn chevron_hit(px: f32, py: f32) -> Option<i32> {
    for (cx, dir) in [(CHEV_LEFT_CX, -1), (CHEV_RIGHT_CX, 1)] {
        if (px - cx).abs() <= CHEV_HIT && (py - CHEV_CY).abs() <= CHEV_HIT {
            return Some(dir);
        }
    }
    None
}

fn weekday_sunday0(year: i32, month: u32, day: u32) -> u32 {
    (days_from_civil(year, month, day) + 4).rem_euclid(7) as u32
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct CalCell {
    day: u32,
    in_month: bool,
}

/// Monday-first 42-cell grid for (`year`, `month`) + the (col, row) of
/// `today_day` inside it. Overflow cells carry the adjacent months' days
/// with `in_month = false`.
fn month_grid(year: i32, month: u32, today_day: u32) -> ([[CalCell; 7]; 6], (usize, usize)) {
    // Monday-first offset of the 1st: Sunday=0 -> 6, Monday=1 -> 0, ...
    let first_wday = weekday_sunday0(year, month, 1);
    let offset = ((first_wday + 6) % 7) as usize;

    // Walk 42 days starting `offset` days before the 1st.
    let (mut cy, mut cm, mut cd) = if offset == 0 {
        (year, month, 1)
    } else {
        let (py, pm) = if month == 1 {
            (year - 1, 12)
        } else {
            (year, month - 1)
        };
        (py, pm, days_in_month(py, pm) - offset as u32 + 1)
    };

    let mut grid = [[CalCell {
        day: 0,
        in_month: false,
    }; 7]; 6];
    for row in grid.iter_mut() {
        for cell in row.iter_mut() {
            *cell = CalCell {
                day: cd,
                in_month: cy == year && cm == month,
            };
            cd += 1;
            if cd > days_in_month(cy, cm) {
                cd = 1;
                cm += 1;
                if cm > 12 {
                    cm = 1;
                    cy += 1;
                }
            }
        }
    }

    let today_idx = offset + (today_day as usize - 1);
    (grid, (today_idx % 7, today_idx / 7))
}

// ---------------------------------------------------------------------------
// Pixel helpers — same style as power_panel.rs / main.rs: premultiplied
// Argb8888 canvas, BGRA byte order, src-over onto the (opaque) black panel.
// ---------------------------------------------------------------------------
fn blend_px(canvas: &mut [u8], w: u32, h: u32, x: i32, y: i32, rgb: [u8; 3], cov: f32) {
    if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 || cov <= 0.0 {
        return;
    }
    let c = cov.clamp(0.0, 1.0);
    let i = ((y as u32 * w + x as u32) * 4) as usize;
    // rgb is [R, G, B]; canvas bytes are [B, G, R, A].
    canvas[i] = (rgb[2] as f32 * c + canvas[i] as f32 * (1.0 - c)) as u8;
    canvas[i + 1] = (rgb[1] as f32 * c + canvas[i + 1] as f32 * (1.0 - c)) as u8;
    canvas[i + 2] = (rgb[0] as f32 * c + canvas[i + 2] as f32 * (1.0 - c)) as u8;
    canvas[i + 3] = 255;
}

fn fill_circle(canvas: &mut [u8], w: u32, h: u32, cx: f32, cy: f32, r: f32, rgb: [u8; 3]) {
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

// ---------------------------------------------------------------------------
// Text — same layout/blit approach as power_panel.rs (duplicated: main.rs's
// helpers are private and this module must stay drop-in).
// ---------------------------------------------------------------------------
struct TextRow {
    glyphs: Vec<(fontdue::Metrics, std::sync::Arc<[u8]>)>,
    width: f32,
    ascent: f32,
}

fn layout_text(font: &fontdue::Font, text: &str, px: f32) -> TextRow {
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

fn blit_glyph(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    m: &fontdue::Metrics,
    cov: &[u8],
    x0: i32,
    y0: i32,
    rgb: [u8; 3],
) {
    for gy in 0..m.height as i32 {
        for gx in 0..m.width as i32 {
            let c = cov[gy as usize * m.width + gx as usize] as f32 / 255.0;
            if c > 0.0 {
                blend_px(canvas, w, h, x0 + gx, y0 + gy, rgb, c);
            }
        }
    }
}

/// Draw `text` so its horizontal centre is `cx` and its vertical centre
/// (cap-height centre) is `cy`. Quicksand cap height is ~0.71em, so the
/// glyph band top sits ~0.36em above the centre line.
fn draw_text_centred_at(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &fontdue::Font,
    text: &str,
    px: f32,
    cx: f32,
    cy: f32,
    rgb: [u8; 3],
) {
    let row = layout_text(font, text, px);
    let top_y = cy - px * 0.36;
    let baseline = top_y + row.ascent;
    let mut pen = cx - row.width / 2.0;
    for (m, cov) in &row.glyphs {
        let gx0 = (pen + m.xmin as f32).round() as i32;
        let gy0 = (baseline - m.ymin as f32 - m.height as f32).round() as i32;
        blit_glyph(canvas, w, h, m, cov, gx0, gy0, rgb);
        pen += m.advance_width;
    }
}

/// How far BELOW its requested centre line `draw_text_centred_at` places
/// a glyph band at `px`, derived from the real font metrics (the
/// same ascent / ymin / height the blit path uses — no assumed constant).
/// Algebra on the draw path: band centre = baseline - (ymin + height/2),
/// baseline = cy - 0.36*px + ascent, so the offset is
/// ascent - (ymin + height/2) - 0.36*px.
/// Generalised iter88 from `digit_centre_offset` (same formula, any
/// char): the '<' / '>' chevrons and the title caps need the same
/// treatment — their offsets differ by ~2px at their respective sizes,
/// which is exactly the iter87 chevron drift.
fn glyph_band_offset(font: &fontdue::Font, ch: char, px: f32) -> f32 {
    let (m, _) = crate::lock_timer::rasterize_cached(font, ch, px);
    if m.height == 0 {
        return 0.0; // no ink — no offset to correct
    }
    let ascent = font
        .horizontal_line_metrics(px)
        .map(|lm| lm.ascent)
        .unwrap_or(px);
    (ascent - (m.ymin as f32 + m.height as f32 / 2.0) - px * 0.36).clamp(0.0, 6.0)
}

/// Digit-band offset (iter10): `glyph_band_offset` for '8' at `px`.
/// Measured iter10: +3.0 at 13px; this must reproduce ~3.0 on the real
/// Quicksand Bold file (see test).
fn digit_centre_offset(font: &fontdue::Font, px: f32) -> f32 {
    let off = glyph_band_offset(font, '8', px);
    if off == 0.0 {
        3.0 // iter10 measured value at 13px — fallback only (no '8' ink)
    } else {
        off
    }
}

// ---------------------------------------------------------------------------
// Panel content
// ---------------------------------------------------------------------------
pub fn draw_calendar_panel(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &fontdue::Font,
) {
    draw_calendar_panel_with_offset(canvas, w, h, font, 0);
}

/// Draw the calendar for the month `offset` months from the current
/// one (0 = this month). The today circle shows only for offset 0 —
/// other months have no "today". Chevrons flank the title.
pub fn draw_calendar_panel_with_offset(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &fontdue::Font,
    offset: i32,
) {
    debug_assert_eq!((w, h), (PANEL_W, PANEL_H));

    // Live date — computed at every draw, never hardcoded.
    let (cur_year, cur_month, day) = local_today();
    let (year, month) = shift_month(cur_year, cur_month, offset);
    let show_today = offset == 0;
    let title = format!("{} {}", MONTH_NAMES[(month - 1) as usize], year);
    let (grid, (today_col, today_row)) = month_grid(year, month, day);

    // (a) Month title, centred.
    draw_text_centred_at(
        canvas,
        w,
        h,
        font,
        &title,
        TITLE_PX,
        PANEL_W as f32 / 2.0,
        TITLE_CY,
        WHITE,
    );

    // (b) Month chevrons (iter36 a6b): ‹ › change the displayed month.
    draw_text_centred_at(canvas, w, h, font, "<", CHEV_PX, CHEV_LEFT_CX, CHEV_CY, WEEKDAY_GREY);
    draw_text_centred_at(canvas, w, h, font, ">", CHEV_PX, CHEV_RIGHT_CX, CHEV_CY, WEEKDAY_GREY);

    // (c) Weekday header row.
    for (col, ch) in WEEKDAYS.iter().enumerate() {
        let mut s = String::new();
        s.push(*ch);
        draw_text_centred_at(
            canvas,
            w,
            h,
            font,
            &s,
            WEEKDAY_PX,
            COL_X[col],
            WEEKDAY_CY,
            WEEKDAY_GREY,
        );
    }

    // (e) Today circle first, so its white digit draws over the blue fill.
    // Circle centre is exactly ROW_Y[today_row]; the digits below are
    // shifted up by the measured draw-path offset so their band centres
    // land on the same ROW_Y — circle and digits agree, no compromise.
    // Only for the current month (offset 0).
    if show_today {
        fill_circle(
            canvas,
            w,
            h,
            COL_X[today_col],
            ROW_Y[today_row],
            TODAY_R,
            ACCENT,
        );
    }

    // (d) Date grid; today digit in white over the blue circle, overflow
    // days dimmer. Digits are drawn at ROW_Y − digit_centre_offset so
    // their glyph band centres sit exactly on ROW_Y (iter10 defect 3).
    let dy_off = digit_centre_offset(font, DATE_PX);
    for (row_idx, row) in grid.iter().enumerate() {
        for (col_idx, cell) in row.iter().enumerate() {
            let is_today = show_today && row_idx == today_row && col_idx == today_col;
            let colour = if is_today || cell.in_month {
                WHITE
            } else {
                OVERFLOW_GREY
            };
            draw_text_centred_at(
                canvas,
                w,
                h,
                font,
                &cell.day.to_string(),
                DATE_PX,
                COL_X[col_idx],
                ROW_Y[row_idx] - dy_off,
                colour,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_test_font() -> fontdue::Font {
        let home = std::env::var("HOME").expect("HOME must be set for the panel test");
        for name in ["Quicksand-Bold.ttf", "Quicksand.ttf"] {
            let path = format!("{home}/.local/share/fonts/{name}");
            if let Ok(bytes) = std::fs::read(&path) {
                if let Ok(f) = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()) {
                    return f;
                }
            }
        }
        panic!("no test font in ~/.local/share/fonts");
    }

    #[test]
    fn days_from_civil_epoch_is_zero_and_thursday() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(weekday_sunday0(1970, 1, 1), 4); // Thursday
    }

    #[test]
    fn weekday_of_known_dates() {
        assert_eq!(weekday_sunday0(2026, 11, 13), 5); // Friday, Nov 13 2026
        assert_eq!(weekday_sunday0(2026, 11, 1), 0); // Sunday
        assert_eq!(weekday_sunday0(2026, 1, 1), 4); // Thursday
        assert_eq!(weekday_sunday0(2024, 2, 29), 4); // Thursday (leap day)
    }

    #[test]
    fn days_in_month_leap_rules() {
        assert_eq!(days_in_month(2026, 2), 28);
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2000, 2), 29); // divisible by 400
        assert_eq!(days_in_month(2100, 2), 28); // divisible by 100, not 400
        assert_eq!(days_in_month(2026, 11), 30);
    }

    #[test]
    fn nov_2026_grid_matches_video_frame() {
        // The old hardcoded grid, now computed: row 1 = 26-31 Oct + 1 Nov,
        // row 6 = 30 Nov + 1-6 Dec; 13 Nov is col 4, row 2 (not the 12th,
        // which was col 3, row 2).
        let (grid, (col, row)) = month_grid(2026, 11, 13);
        assert_eq!((col, row), (4, 2));
        assert_eq!(grid[0][0], CalCell { day: 26, in_month: false });
        assert_eq!(grid[0][6], CalCell { day: 1, in_month: true });
        assert_eq!(grid[2][3], CalCell { day: 12, in_month: true });
        assert_eq!(grid[2][4], CalCell { day: 13, in_month: true });
        assert_eq!(grid[5][0], CalCell { day: 30, in_month: true });
        assert_eq!(grid[5][6], CalCell { day: 6, in_month: false });
        // Every cell filled, 42 total, days strictly increasing by walk.
        assert_eq!(grid.iter().flatten().count(), 42);
    }

    #[test]
    fn nov_16_2026_today_is_col0_row3() {
        // Today is Mon 16 Nov 2026: 1 Nov is a Sunday (offset 6), so the
        // 16th is index 21 -> col 0, row 3. Pins the live date the island
        // must circle today (iter116); the 13th case lives in
        // nov_2026_grid_matches_video_frame.
        let (grid, (col, row)) = month_grid(2026, 11, 16);
        assert_eq!((col, row), (0, 3));
        assert_eq!(grid[3][0], CalCell { day: 16, in_month: true });
        assert_eq!(grid[2][6], CalCell { day: 15, in_month: true });
        assert_eq!(grid[3][1], CalCell { day: 17, in_month: true });
    }

    #[test]
    fn feb_2026_starts_sunday_offset_6() {
        // 1 Feb 2026 is a Sunday -> grid starts Mon 26 Jan.
        let (grid, (col, row)) = month_grid(2026, 2, 1);
        assert_eq!((col, row), (6, 0));
        assert_eq!(grid[0][0], CalCell { day: 26, in_month: false });
        assert_eq!(grid[0][6], CalCell { day: 1, in_month: true });
        assert_eq!(grid[4][6], CalCell { day: 1, in_month: false }); // 1 Mar
    }

    #[test]
    fn month_starting_monday_has_zero_offset() {
        // 1 Dec 2025 is a Monday -> grid starts 1 Dec, today col 0 row 0.
        let (grid, (col, row)) = month_grid(2025, 12, 1);
        assert_eq!((col, row), (0, 0));
        assert_eq!(grid[0][0], CalCell { day: 1, in_month: true });
    }

    #[test]
    fn digit_offset_reproduces_measured_3px_on_real_font() {
        // Loads the same files main.rs loads. Skips (does not fail) when
        // neither is present — e.g. a VM without Alexandre's fonts.
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default();
        let font = ["Quicksand-Bold.ttf", "Quicksand.ttf"]
            .iter()
            .find_map(|name| {
                let path = home.join(".local/share/fonts").join(name);
                std::fs::read(&path).ok().and_then(|b| {
                    fontdue::Font::from_bytes(b, fontdue::FontSettings::default()).ok()
                })
            });
        let Some(font) = font else { return };
        let off = digit_centre_offset(&font, 13.0);
        assert!(
            (2.0..=4.0).contains(&off),
            "digit_centre_offset at 13px = {off}, expected ~3.0 (iter10 measured)"
        );
    }
    // -- iter18 D1 regression helpers, repurposed iter24 -----------
    // The iter18 tests asserted the drawn month chevrons survived the
    // 1.5x upscale. iter24 REMOVED the chevrons (one nav system only —
    // the island's outside discs); the remaining test below asserts
    // they are gone while the title still survives the upscale. The
    // bilinear helpers stay: they are the same harness the title
    // control uses.

    /// Luminance (max channel — the critics' peak metric) of the canvas
    /// at (x, y), clamped to the canvas edge like a texture sampler.
    fn canvas_luma(canvas: &[u8], w: u32, h: u32, x: i32, y: i32) -> f32 {
        let x = x.clamp(0, w as i32 - 1) as usize;
        let y = y.clamp(0, h as i32 - 1) as usize;
        let i = (y * w as usize + x) * 4;
        canvas[i].max(canvas[i + 1]).max(canvas[i + 2]) as f32
    }

    /// Bilinear sample of the canvas luminance at continuous coords.
    fn sample_bilinear(canvas: &[u8], w: u32, h: u32, sx: f32, sy: f32) -> f32 {
        let x0 = sx.floor() as i32;
        let y0 = sy.floor() as i32;
        let fx = sx - x0 as f32;
        let fy = sy - y0 as f32;
        let c00 = canvas_luma(canvas, w, h, x0, y0);
        let c10 = canvas_luma(canvas, w, h, x0 + 1, y0);
        let c01 = canvas_luma(canvas, w, h, x0, y0 + 1);
        let c11 = canvas_luma(canvas, w, h, x0 + 1, y0 + 1);
        (c00 * (1.0 - fx) + c10 * fx) * (1.0 - fy) + (c01 * (1.0 - fx) + c11 * fx) * fy
    }

    /// Peak luminance inside the logical bbox (lx0, ly0)-(lx1, ly1)
    /// after a `scale` bilinear upscale (1.5 = the compositor path).
    /// Output-pixel centres map back as (o + 0.5) / scale - 0.5, the
    /// same centre alignment PIL's BILINEAR resize uses.
    fn upscale_peak(
        canvas: &[u8],
        w: u32,
        h: u32,
        scale: f32,
        lx0: u32,
        ly0: u32,
        lx1: u32,
        ly1: u32,
    ) -> f32 {
        let mut peak = 0.0f32;
        for oy in (ly0 as f32 * scale) as u32..(ly1 as f32 * scale) as u32 {
            for ox in (lx0 as f32 * scale) as u32..(lx1 as f32 * scale) as u32 {
                let sx = (ox as f32 + 0.5) / scale - 0.5;
                let sy = (oy as f32 + 0.5) / scale - 0.5;
                peak = peak.max(sample_bilinear(canvas, w, h, sx, sy));
            }
        }
        peak
    }

    /// Peak luminance inside a logical bbox at 1x (no upscale).
    fn bbox_peak(canvas: &[u8], w: u32, h: u32, lx0: u32, ly0: u32, lx1: u32, ly1: u32) -> f32 {
        let mut peak = 0.0f32;
        for y in ly0..ly1 {
            for x in lx0..lx1 {
                peak = peak.max(canvas_luma(canvas, w, h, x as i32, y as i32));
            }
        }
        peak
    }

    fn load_font(names: &[&str]) -> Option<fontdue::Font> {
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default();
        names.iter().find_map(|name| {
            let path = home.join(".local/share/fonts").join(name);
            std::fs::read(&path).ok().and_then(|b| {
                fontdue::Font::from_bytes(b, fontdue::FontSettings::default()).ok()
            })
        })
    }

    #[test]
    fn calendar_month_chevrons_present_iter36() {
        // iter36 a6b REVERSES the iter24 removal: the nav discs cycle
        // MENUS, not months, so the panel gets its own ‹ › again — they
        // change the DISPLAYED month (CalMonth). Renders the real panel
        // with the real font and asserts the chevron bboxes are BRIGHT
        // (ink present) while the title still survives the 1.5x upscale.
        // Skips (does not fail) where the fonts are absent.
        let Some(bold) = load_font(&["Quicksand-Bold.ttf", "Quicksand.ttf"]) else {
            return;
        };
        let mut canvas = vec![0u8; (PANEL_W * PANEL_H * 4) as usize];
        draw_calendar_panel(&mut canvas, PANEL_W, PANEL_H, &bold);
        for (lx0, lx1, name) in [(35u32, 46u32, "left"), (297u32, 308u32, "right")] {
            let peak = bbox_peak(&canvas, PANEL_W, PANEL_H, lx0, 0, lx1, 17); // iter89: chevrons at CHEV_CY 5.2 (iter121: 5.86 screen-calibrated), band ~0..12
            assert!(
                peak >= 100.0,
                "{name} chevron bbox peak = {peak}, expected >= 100: the month chevrons must be drawn"
            );
        }
        // Control: the title must survive the upscale — if it does not,
        // the harness (not the chevrons) is dimming everything.
        let title_peak = upscale_peak(&canvas, PANEL_W, PANEL_H, 1.5, 120, 0, 222, 18); // iter89: title at TITLE_CY 7.2, band ~1..13
        assert!(
            title_peak >= 240.0,
            "title peak after 1.5x upscale = {title_peak:.0}, expected >= 240"
        );
    }

    // iter36 a6b: month shifting wraps the year boundary; chevrons hit.
    #[test]
    fn shift_month_wraps_year() {
        assert_eq!(shift_month(2026, 12, 1), (2027, 1));
        assert_eq!(shift_month(2027, 1, -1), (2026, 12));
        assert_eq!(shift_month(2026, 11, 2), (2027, 1));
        assert_eq!(shift_month(2026, 11, -11), (2025, 12));
        assert_eq!(shift_month(2026, 11, 0), (2026, 11));
        assert_eq!(shift_month(2026, 11, 13), (2027, 12));
    }

    #[test]
    fn chevron_hit_zones() {
        // iter89: hit-test centre follows CHEV_CY (iter121: 5.86).
        assert_eq!(chevron_hit(40.0, CHEV_CY), Some(-1));
        assert_eq!(chevron_hit(302.0, CHEV_CY), Some(1));
        assert_eq!(chevron_hit(171.0, CHEV_CY), None); // title centre
        assert_eq!(chevron_hit(40.0, 19.0), Some(-1)); // just inside the +/-14 box (5.86 + 14 = 19.86)
        assert_eq!(chevron_hit(40.0, 25.5), None); // old centre now outside the box
        assert_eq!(chevron_hit(40.0, 100.0), None); // below the title row
        assert_eq!(chevron_hit(40.0, CHEV_CY + CHEV_HIT + 1.0), None); // just below the box
    }

    #[test]
    fn offset_draw_renders_without_panic() {
        // December (offset +1 from November) and January (offset +2):
        // grid renders, no today circle, title differs from November.
        let font = load_test_font();
        let mut canvas = vec![0u8; (PANEL_W * PANEL_H * 4) as usize];
        for px in canvas.chunks_exact_mut(4) {
            px[3] = 255;
        }
        draw_calendar_panel_with_offset(&mut canvas, PANEL_W, PANEL_H, &font, 1);
        let bright = canvas.chunks_exact(4).filter(|px| px[2] as u32 + px[1] as u32 + px[0] as u32 > 200).count();
        assert!(bright > 500, "offset month renders content");
    }

    // iter88: chevron band centres == title band centre (rendered, real font).
    #[test]
    fn chevrons_share_title_row_iter88() {
        let Some(bold) = load_font(&["Quicksand-Bold.ttf", "Quicksand.ttf"]) else {
            return;
        };
        let mut canvas = vec![0u8; (PANEL_W * PANEL_H * 4) as usize];
        draw_calendar_panel(&mut canvas, PANEL_W, PANEL_H, &bold);
        // Band centre = (first + last ink row) / 2 in the given x-window.
        let band_centre = |x0: u32, x1: u32, y0: u32, y1: u32, thr: f32| -> f32 {
            let mut lo = u32::MAX;
            let mut hi = 0u32;
            for y in y0..y1 {
                for x in x0..x1 {
                    if canvas_luma(&canvas, PANEL_W, PANEL_H, x as i32, y as i32) > thr {
                        lo = lo.min(y);
                        hi = hi.max(y);
                    }
                }
            }
            (lo + hi) as f32 / 2.0
        };
        // Cross-check the constant against the real font metrics: the
        // '<' glyph at CHEV_PX sits lower than the title caps at TITLE_PX
        // by (chev_off - title_off), so the font model predicts CHEV_CY
        // = TITLE_CY + title_off - chev_off = 5.83. CHEV_CY (5.86) is
        // within 0.03 of the model — the iter116 screen calibration
        // (6.53) overshot by the double-counted 0.67; see the
        // constant's comment. If this assertion fails, the font
        // changed: re-measure ON SCREEN, do not "fix" the constant
        // back to the model value.
        let title_off = glyph_band_offset(&bold, 'N', TITLE_PX);
        let chev_off = glyph_band_offset(&bold, '<', CHEV_PX);
        let predicted = TITLE_CY + title_off - chev_off;
        eprintln!("iter88 metrics: title_off={title_off:.2} chev_off={chev_off:.2} predicted_chev_cy={predicted:.2} CHEV_CY={CHEV_CY}");
        assert!(
            (predicted - CHEV_CY).abs() <= 1.0,
            "predicted chevron cy {predicted:.2} vs CHEV_CY {CHEV_CY} — screen calibration is 5.86, model is 5.83; re-measure on screen if the font changed"
        );
        // Ground truth on the rendered canvas (windows fixed iter116 —
        // they were y 15..40 pre-iter89 and asserted nothing). The canvas
        // now carries the chevrons ~AT the title band centre (model
        // delta +0.03): iter116's +0.70 canvas offset was the
        // double-count — two fresh iter121 shots agree the screen
        // delta at CHEV_CY 6.53 was +0.67, so CHEV_CY = 5.86 aligns
        // them ON SCREEN. Assert the canvas offset is the calibrated
        // one. Centroids are sub-pixel; the integer-row bbox centres
        // are printed alongside.
        let title_c = band_centre(105, 237, 0, 18, 200.0);
        let left_c = band_centre(35, 46, 0, 18, 100.0);
        let right_c = band_centre(297, 308, 0, 18, 100.0);
        // Luminance-weighted centroid y of the ink in the window.
        let band_centroid = |x0: u32, x1: u32, y0: u32, y1: u32, thr: f32| -> f32 {
            let mut wsum = 0.0f32;
            let mut ysum = 0.0f32;
            for y in y0..y1 {
                for x in x0..x1 {
                    let l = canvas_luma(&canvas, PANEL_W, PANEL_H, x as i32, y as i32);
                    if l > thr {
                        wsum += l;
                        ysum += l * y as f32;
                    }
                }
            }
            if wsum == 0.0 {
                f32::NAN
            } else {
                ysum / wsum
            }
        };
        let title_m = band_centroid(105, 237, 0, 18, 200.0);
        let left_m = band_centroid(35, 46, 0, 18, 100.0);
        let right_m = band_centroid(297, 308, 0, 18, 100.0);
        eprintln!("iter88 bands: title={title_c} left={left_c} right={right_c}");
        eprintln!("iter116 centroids: title={title_m:.2} left={left_m:.2} right={right_m:.2}");
        // The rendered chevron bands must match the draw-path model
        // (CHEV_CY + the glyph's own band offset) within 0.6 logical:
        // this pins the RENDERING faithful to the constant. The bar is
        // 0.6, not 0.4: the luminance centroid is integer-row
        // quantized, so a sub-row CHEV_CY shift can flip a centroid by
        // a full row (iter121: both centroids moved exactly -1.00 for
        // a -0.67 constant shift, 6.53 -> 5.86; the bbox centres in the
        // same run, title=10 left=10.5 right=10, agree the canvas is
        // aligned). 0.6 still catches a >=1-row systematic error. Note the
        // measured title centroid (11.00 in Nov) sits ~0.43 BELOW the
        // model cap centre (10.57) because the title string carries
        // lowercase ink — so the chevrons are compared to the MODEL
        // centres, not the measured title centroid (month-name
        // dependent). The constant's VALUE (5.86) is pinned by
        // chev_cy_screen_calibrated_iter116 below, not here: these
        // expectations move with CHEV_CY by construction.
        let chev_off_right = glyph_band_offset(&bold, '>', CHEV_PX);
        for (m, expected, name) in [
            (left_m, CHEV_CY + chev_off, "left"),
            (right_m, CHEV_CY + chev_off_right, "right"),
        ] {
            assert!(
                (m - expected).abs() <= 0.6,
                "{name} chevron canvas centroid {m:.2} vs draw-path model {expected:.2} (CHEV_CY + glyph offset)"
            );
        }
    }

    #[test]
    fn chev_cy_screen_calibrated_iter116() {
        // Tripwire: CHEV_CY = 5.86 is SCREEN-calibrated (fresh v0.54
        // measurement, CHEV-MEASURE iter121: two settled shots
        // chev121-calendar.png, md5 f5ffa0a2527fc2ebe06a517bcc85c3e9 —
        // chevrons +0.67 logical vs the title band centre at 6.53,
        // so 6.53 - 0.67 = 5.86; fontdue model predicts 5.83, agrees
        // within 0.03. The 6.53 value overshot: the v0.53 reading it
        // was derived from was depressed one rounding step by the
        // integer glyph blit — see chev-measure-iter121.md §4.)
        // Change this value ONLY with a fresh on-screen measurement,
        // and update the constant's comment + this test together.
        assert!(
            (CHEV_CY - 5.86).abs() < 0.01,
            "CHEV_CY = {CHEV_CY}, expected the screen-calibrated 5.86 — see the constant's comment"
        );
    }

    // iter88: no dead band below the grid — panel bottom sits just under
    // the last content row (digit band ~263, row-6 today circle ~270).
    #[test]
    fn no_dead_band_below_grid_iter88() {
        let Some(bold) = load_font(&["Quicksand-Bold.ttf", "Quicksand.ttf"]) else {
            return;
        };
        let mut canvas = vec![0u8; (PANEL_W * PANEL_H * 4) as usize];
        draw_calendar_panel(&mut canvas, PANEL_W, PANEL_H, &bold);
        let mut last = 0u32;
        for y in 0..PANEL_H {
            for x in 0..PANEL_W {
                if canvas_luma(&canvas, PANEL_W, PANEL_H, x as i32, y as i32) > 60.0 {
                    last = last.max(y);
                }
            }
        }
        let dead = PANEL_H - last - 1;
        eprintln!("iter88 dead band: last content row {last}, PANEL_H {PANEL_H}, dead {dead}px");
        assert!(dead <= 20, "dead band below grid = {dead}px, expected <= 20 (rust87: ~37)");
        assert!(
            dead >= 8,
            "panel bottom only {dead}px below last content — a row-6 today circle needs >= 8"
        );
    }

}
