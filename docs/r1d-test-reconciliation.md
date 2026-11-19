# r1d test reconciliation — 19 failures, ONE root cause

`cargo build --release --offline` is clean (0 errors). `cargo test --release
--offline` = **297 passed / 19 failed**. Every failure traces to **r1d §0**,
which is CORRECT and must not be reverted.

## The change that is right

`panel_origin()` (`src/main.rs:930-944`) now returns `STAGE_PAD_TOP` (3.0) for
the cycled menus; only `TrayMenu` keeps `panel_top_menu()` (30.0). Reference
60 fps frames pin the panel top at y=19–20 while height grows 42→188, i.e. the
pill **grows into** the panel. Verified on the live desktop: panel top y=20,
reference y=19.

Consequence: in a cycled menu the panel now **covers the pill row**, so the
workspace discs and tray icons are neither drawn nor clickable there.
`classify_press` already encodes this correctly (`src/main.rs:1901-1910`
restricts cluster hits to Collapsed/Dictation/Lock/TrayMenu).

## Failure groups

### A. Tests asserting the OLD hang-below geometry (7) — update expectations
| test | line | asserts | now |
|---|---|---|---|
| `iter30_tests::panel_below_pill_geometry` | 8065 | `moy == panel_top_menu()` for menus | 3.0 |
| `iter30_tests::morph_capsule_oy_interpolates_pill_to_panel` | 8511 | endpoint 33.0 | 3.0 |
| `iter30_tests::discs_at_panel_midheight` | 8499 | Music ly 108 | 78 |
| `iter30_tests::pill_and_bridge_in_menu_region` | 8531 | `!point_in_state_region((400,28), Music)` | inside now |
| `iter26_tests::iter27b_pill_rect_collapses_from_every_menu` | 7586 | `!point_in_panel_stage((588,16), state)` | inside now |
| `iter26_tests::iter27b_just_outside_pill_rect_stays_noop` | 7619 | NoOp just outside pill | inside panel |
| `iter26_tests::iter27_empty_panel_press_is_noop_in_all_menus` | 7511 | Power point (411,144) in panel | panel moved/resized |

Rename `panel_below_pill_geometry` — the name now states the opposite of the
design. Assert `oy == STAGE_PAD_TOP` for MENU_CYCLE and `== panel_top_menu()`
for TrayMenu **only**. Keep the TrayMenu case: it is the one state that still
hangs below, and it is what keeps this test meaningful.

For the hit tests, **derive the probe point from `panel_rect_stage(state)`**
(e.g. centre, or centre + a small offset) instead of hardcoding coordinates.
A hardcoded point that happens to fall inside a wrong rect is worse than a
failing test. Where the test's purpose is "just outside", compute the edge
from the rect and step 1 px past it.

### B. Cluster/tray hits in menu states (5) — the behaviour changed on purpose
- `iter30_tests::classify_press_workspace_tag` (8283) — second assert uses Music
- `iter30_tests::classify_press_tray_icon` (8417)
- `iter108_tray_tests::classify_press_tray_click_activates` (9644) — comment
  says "icons ride above every panel"; no longer true
- `iter36_tests::{wallpaper_chevron_click_browses, themes_row_click_applies_that_theme, power_disc_click_is_lock, calendar_chevron_click_shifts_month}`
  (8593/8611/8626/8644) — nav-disc and row probes moved with the panel

For the cluster/tray ones: keep the Collapsed/Dictation/Lock/TrayMenu asserts,
and **flip the menu-state assert to `NoOp`** with a comment saying the panel
swallows the row since §0. Do not delete the menu case — it is now the
regression guard for exactly the bug the owner reported.

### C. Poisoned mutex (3) — collateral, not independent bugs
`iter30_tests::tray_count_store_roundtrip` (8465),
`iter108_tray_tests::tray_region_follows_snapshot` (9656),
`iter108_tray_tests::tray_menu_rows_hit_and_size` (9670)
all fail with `PoisonError` on `TRAY_TEST_LOCK.lock().unwrap()`.

**Proof**: `cargo test --release --offline tray_count_store_roundtrip` alone
→ **1 passed**. They only fail because
`classify_press_tray_click_activates` panics *while holding the lock*.
Fix group B and these three go green with no edit. Do not "fix" them by
switching to `.lock().unwrap_or_else(|e| e.into_inner())` — that would hide
the real failure.

### D. Genuine value changes (2)
- `library::tests::accent_is_fixed_periwinkle_for_any_theme` (`src/library.rs:649`)
  left (121,161,245) right (124,162,245). Reference measures **(121,161,245)**;
  the new value is correct — update the expectation.
- `themes_panel::tests::renders_real_content` (`src/themes_panel.rs:1399`)
  "search-band bright px: 518, counter bright px: 76". Investigate: this may be
  a real rendering regression from the `C_HIGHLIGHT` change, not a stale
  expectation. Check the rendered band before editing the threshold.

## Acceptance
- `cargo test --release --offline` → **0 failed**
- `python3 scripts/halo_check.py` → `holes: 0`, `RESULT PASS` (currently PASS)
- `python3 scripts/halo_check.py collapsed` → PASS
- `python3 scripts/fontcheck.py assets/fonts/Inter-Clock.ttf 12.105 0.0275` → PASS

## Do not touch
Frame-callback back-pressure; the four socket literals; the exit-75
single-instance guard; `collapsed_width=67 bar_height=25 gap_top=13`;
`assets/fonts/Inter-Clock.ttf`; `scripts/`; no PAM / session-lock.

## Still open after the tests (critic #24, score 4)
1. **`src/main.rs:5830-5834`** — both arms of `if frame_running` call
   `upscale_canvas_2x_region`. Expanded panels are 1×-rasterised then
   bilinearly doubled: Power "Lock" lapvar **1609 vs reference 10222 = 6.4×
   blurrier**. Biggest remaining visual defect.
2. `C_HIGHLIGHT = (5,22,60)` at `src/themes_panel.rs:262`,
   `src/launcher_panel.rs:178`, `src/clipboard_panel.rs:107` — reference
   selection is neutral **(25,26,31)**.
3. Power 400 logical = 26.6 % of screen width vs reference **21.0 %**;
   label peak 251 vs **222**.
4. Missing 1 px search divider + caret (themes, launcher); themes uses
   circular chips where the reference has rounded-square ~35×29 r≈6.
