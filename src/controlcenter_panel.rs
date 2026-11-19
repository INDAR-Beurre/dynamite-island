//! controlcenter_panel — Control Centre state (438x447) for dynamite-rust.
//!
//! Layout spec + `draw_controlcenter_panel()` skeleton, in the same
//! manual-SDF / Argb8888 style as `main.rs` (no tiny-skia, no cosmic-text,
//! no new dependencies — only `fontdue`, already in Cargo.toml).
//!
//! SOURCES (do not re-derive from memory):
//! - `analysis/frame-measurements.md` CORRECTION: s07-s08 is the CONTROL
//!   CENTRE, 438x525 — top row time "00:05" + battery "81%", toggles Wi-Fi /
//!   Focus / Bluetooth / Night Light, Display + Sound sliders, notifications
//!   block "All caught up".
//! - `iterations/critic-cc.md` — harsh critic on the live Quickshell render
//!   (12 Nov ~21:21 CET, surface-cc-critic.png). All video-sampled colours
//!   and the slider/toggle redesign below come from that verdict; where the
//!   critic's numbers disagree with `configs/quickshell/dynamite/shell.qml`,
//!   the CRITIC (video) wins and the Quickshell value is noted as superseded.
//! - `configs/quickshell/dynamite/shell.qml` — structural skeleton only
//!   (column order, spacing 12, inner padding 14, toggle/slider defaults).
//!
//! WIRING (integrator — do NOT do it in this file, `main.rs` is untouched):
//! ```ignore
//! // main.rs, top of file:
//! mod controlcenter_panel;
//! // main.rs, in draw(): replace the ControlCenter placeholder arm with:
//! //   IslandState::ControlCenter => controlcenter_panel::draw_controlcenter_panel(
//! //       canvas, w, h, font, icon_font.as_ref(),
//! //       &controlcenter_panel::ControlCenterData::video_defaults()),
//! ```
//! The icon font is a second `fontdue::Font` loaded in `main.rs` from
//! `~/.local/share/fonts/MaterialSymbolsRounded.ttf` (verified present on
//! the Surface, 12 Nov 21:19). Pass `None` until it is loaded; icons then
//! render as a hollow-ring "missing icon" marker so a screenshot can tell
//! "icon not wired" apart from "icon drawn blank".
//!
//! ICONS — READ THIS BEFORE "FIXING" THEM:
//! `fontdue` rasterises single chars; it does NOT shape ligatures, so the
//! Material Symbols glyph NAME ("wifi") cannot be rendered — only its
//! Private Use Area codepoint can. `icon_char()` below holds the table,
//! VERIFIED 12 Nov 2026 on the Surface with fontTools against the installed
//! MaterialSymbolsRounded.ttf (see the fn comment for the sha256).
//! Two icons are drawn procedurally instead and need no font:
//! Focus = target ring + centre dot (critic defect 7: the video's Focus
//! glyph is a concentric/target glyph, NOT do-not-disturb), Battery = SDF
//! outline + proportional fill + nub.
//!
//! TEXT WEIGHT: iter10 — `Quicksand-Bold.ttf` (static, wght 700) IS now
//! installed on the Surface and main.rs loads it as THE text font, so
//! the `bold` flag's double-blit is OFF at every call site in this file
//! (the base font is already Bold; double-blitting it would over-thicken
//! the labels). The flag stays in the signature — pass `false`.
//!
//! ---------------------------------------------------------------------------
//! GEOMETRY — all panel-relative logical px, canvas 438x447, radius 20.
//! (Panel top edge sits at screen y = 15 via the layer margin, as usual.)
//! iter85: panel SHRUNK 525 -> 421 — the old 525 framed ~75 rows of void
//! between the Sound track and the notification card plus ~56 rows below
//! the card (iter37: ~26% dead space; iter84: CC 46% empty black). The
//! content is now one compact column with no gap > 14 px and a 14 px
//! bottom margin, so the panel is sized to its content. REQUIRES the
//! matching main.rs change (IslandState::size: ControlCenter (438, 409)
//! -> (438, 401) after iter91) — see the iter91 note on CC_H below;
//! this file alone cannot shrink the live panel.
//! iter89: 421 -> 409 (whole stack -12). iter91: 409 -> 401 (R3 card
//! 99 -> 91 tall, bell block -4, padding 12 -> 8; header divider at
//! y=30 added). Dead bands: NONE — verified live on the v0.33 shot
//! 14 Nov 2026 (card bottom 396 / panel 410 pre-iter91; gaps 12-14
//! throughout; the iter37 85+52-row bands are gone).
//! ---------------------------------------------------------------------------
//!   Element                     x            y            w      h
//!   --------------------------  -----------  -----------  -----  ----
//!   Panel                       0            0            438    401
//!   Header row                  14           2..24        410    22
//!     time (live HH:MM)         14           2..24        ~40    14px
//!       — was hardcoded "00:05" (s07) until iter9; `live()` now fills
//!       it from the system clock, same value as the collapsed pill.
//!     battery icon              ~384..399    ~7..18       ~15    15px
//!     battery (live %, was "81%" in s07) ~404..424 ~6..18  ~20    12px
//!     power-source text (iter90) right-anchored 8 px left of the
//!       battery icon, 12 px — "On battery" / "Plugged in · {W} W"
//!     header divider (iter91)   14..424      30..31       410    1
//!   Toggle grid                 14           36..148      410    112
//!     row 1                     14/223       36..88       201    52
//!     row 2                     14/223       96..148      201    52
//!     (col gap 8, row gap 8, pill radius 12)
//!     icon circle Ø48           pill_x+10    pill_y+2     48     48
//!     icon glyph 16px           centred in circle
//!     name 12px bold            pill_x+67    pill_y+15..27
//!     sub  10px #BEC4CA         pill_x+67    pill_y+27..37
//!   Display label "Display"     14           160..174      —      11px
//!   Display track               14..390      178..214     376    36
//!   Display fill                14..          178..214     376*lv 36
//!   Display thumb Ø28 white     fill end, full track span (iter107)
//!   Display fill-glyph 20px     22..42       centred y=196 (dark navy)
//!   Display end-button (hit)   396..424     178..214     28     36
//!     (bare dimmed chevron 14 px, iter91 — no pill, no disc; iter86:
//!     the outlined pill read as a nav-disc clone)
//!   Sound label "Sound"         14           228..242      —      11px
//!   Sound track                 14..390      246..282     376    36
//!   Sound fill                  14..          246..282     376*lv 36
//!   Sound thumb Ø28 white       fill end, full track span (iter107)
//!   Sound fill-glyph 20px       22..42       centred y=264 (dark navy)
//!   Sound end-button (hit)      396..424     246..282     28     36
//!   Notification card           14           296..387     410    91
//!     (8 px padding around bell + two text lines — sized to content;
//!     iter91: was 99 tall with 12 px padding, card (13,12,15))
//!   Bell circle Ø34             202..236     304..338     34     34
//!     centre (219, 321)
//!   Bell glyph 16px             centred in circle
//!   "All caught up" 13px bold   centred x=219  348..361
//!   "Nothing new since HH:MM"   centred x=219  366..379  10px #BEC4CA
//! ---------------------------------------------------------------------------
//! ANCHOR NOTES (why these numbers, and what is still open):
//! - Slider track rows 207..227 / 276..296 are VIDEO-ANCHORED: the critic
//!   measured the pale fill blobs at panel-relative rows 207-225 and
//!   276-297 (fill height 18-21 logical). The 20 px height and the 28 px
//!   gap between the toggle grid bottom (160) and the Display label (188)
//!   follow from those anchors; Quickshell used spacing 12 + 6 px tracks,
//!   which the critic FAILED (defect 1). The geometry measurer should
//!   re-confirm the two rows on s07.png before pixel-parity is claimed.
//! - Bell block (iter18 D-BELL-Y/D3a): disc centre 357 -> 400, title
//!   top 381 -> 430, sub top 404 -> 447. The critic measured the block
//!   ~43 px too high two ways (absolute panel-rel centres vs s07/s08
//!   x3.0, and the Sound-slider->bell gap 45 vs the video's ~90). The
//!   shift is deliberately NOT uniform: title +49 vs disc/sub +43,
//!   because the replica's internal gaps were also off (disc->title
//!   32.3 vs video ~39; title->sub 24.7 vs video ~18). Predicted
//!   post-fix centres: disc ~400, title ~438, sub ~456; gap ~87;
//!   void below the sub-line ~69 (was ~112). Re-measure on the next
//!   settled CC shot; the critic's pass band is a 40-46 px shift.
//! - iter85 RE-ANCHOR (dead-space kill): the bell block moves UP with the
//!   compact layout — disc centre 400 -> 337, title 430 -> 364, sub
//!   447 -> 382, card 371..469 -> 308..407 (99 tall, 12 px padding),
//!   panel 525 -> 421. The iter18 video-relative offsets above are
//!   history now: the video's own 525-tall frame carries the same void
//!   the critics kept failing, and Alexandre's iter85 instruction is
//!   to size the panel to its content instead of matching the video's
//!   empty bands. Slider rows are likewise no longer video-anchored:
//!   Display 190..226 / Sound 258..294 at 36 px height (was 207..227 /
//!   276..296 at 20 px).
//! - iter89: whole stack -12 — bell centre 337 -> 325, title 364 ->
//!   352, sub 382 -> 370, card 308..407 -> 296..395, panel 421 -> 409,
//!   Display track 190..226 -> 178..214, Sound 258..294 -> 246..282.
//!   The iter27 no-op point (panel-local (219, 303)) is now INSIDE the
//!   card (top 296) — still NoOp, because the card has no click action;
//!   the Sound track's hit slack ends 286, so 303 stays clear of it.
//! - iter91 (R3): bell block -4 more — centre 325 -> 321, title 352 ->
//!   348, sub 370 -> 366, card 296..395 -> 296..387 (91 tall, 8 px
//!   padding), panel 409 -> 401. Card colour (32,34,38) -> (13,12,15),
//!   video-sampled from s07 (the old step above the panel is what made
//!   the card read as a grey slab).
//! - Fill fractions (iter10 fixer, re-measured 13 Nov 00:40 CET from
//!   frames/per-second/s07.png + s08.png, panel x 247..392 in the 640-px
//!   frame, scale 1/3): Display fill ends frame x 323, Sound ends 363;
//!   track starts frame x 251.7 (panel 247 + 14/3), width 125.3 frame px
//!   => Display (323-251.7)/125.3 = 0.57, Sound (363-251.7)/125.3 = 0.89
//!   (±0.01; +1 px AA-cap correction lands on 0.58 / 0.90). The iter8
//!   critic measured the same values independently (0.58 ± 0.02 /
//!   0.90 ± 0.02). The old 0.63 / 0.78 (Quickshell defaults, copied into
//!   video_defaults/live) were WRONG — the earlier comment claiming they
//!   "match the video within measurement error" is retracted. These are
//!   the VIDEO values, the pixel-close target; `video_defaults()` keeps
//!   them for diffing against s07/s08. The LIVE levels live in `live()`
//!   only — display from `brightnessctl g`/`m` (iter94; falls back to
//!   0.58 only if both readers fail, see LIVE DATA), sound from
//!   wpctl/pactl (0.40 at wiring time, NOT 0.90) — and must be
//!   screenshot-verified against the LIVE system values at deploy,
//!   not against s07/s08.
//! - The video's track width reads ~324 px in the critic's row profile vs
//!   376 px here (Quickshell geometry, track x 14..390 + 28 px end button).
//!   Open question for the measurer; the fill FRACTION is what matters.
//! ---------------------------------------------------------------------------
//! COLOURS — video-sampled (critic-cc.md) unless marked QS:
//!   PANEL           #000000  (0,0,0)      — video core (1,0,3), QS same
//!   PILL_BG         #0C0B0E  (12,11,14)   — video; QS #1A1C1F SUPERSEDED
//!   TOGGLE_ON       #93A4F5  (147,164,245)— video pale periwinkle;
//!                                            QS accent #3B82F6 SUPERSEDED
//!   TOGGLE_ON_GLYPH #0B1B3A  dark navy    — video; QS white SUPERSEDED
//!   TOGGLE_OFF      #34373C                — QS (video: dark, unverified)
//!   TOGGLE_OFF_GLYPH #FFFFFF               — QS (video: light glyphs)
//!   SLIDER_FILL     #74A3F6  (116,163,246)— video; QS #A8C7FA SUPERSEDED
//!   SLIDER_TRACK    #33363B                — QS (video: dark, unverified)
//!   END_BTN_CHEVRON #FFFFFF               — bare row-action chevron (iter86;
//!                                            pill bg + outline deleted)
//!   PRIMARY         #FFFFFF   SECONDARY #BEC4CA  (190,196,202 — iter85:
//!     the old #9AA0A6 sub-labels failed legibility; the brighter value
//!     keeps ~7:1 on the card and ~10:1 on the pill bg)
//!   BELL_BG         #1A1C1F                — QS fieldBg (bell circle only;
//!                                            NOT the toggle pill bg)
//! ---------------------------------------------------------------------------
//! LIVE DATA — exact sources (time + battery WIRED in iter9, wifi/SSID
//! WIRED in iter10, display/volume/bluetooth/focus/notifications WIRED
//! in iter13, see `ControlCenterData::live()`; night light still TODO):
//!   time / "since"   LIVE via `live_local_hhmm()` — libc localtime_r, the
//!                    same call as main.rs `hhmm()`, so the CC header can
//!                    never disagree with the collapsed pill again. The
//!                    "Nothing new since HH:MM" line uses the SAME live
//!                    value (empty-state semantics: nothing has arrived
//!                    since the panel was drawn — the video's s07 shows
//!                    both readings equal, 00:05, for the same reason).
//!                    Verified need: iter8 CC header read "00:05" while
//!                    the pill read "22:46" (critic-rust-realism-iter8.md
//!                    Defect 2). `video_defaults()` keeps the frozen
//!                    00:05 ONLY for pixel-diffing against s07.png —
//!                    never for the live island.
//!   battery %        LIVE via `live_battery_pct()` —
//!                    /sys/class/power_supply/BAT*/capacity, first `BAT*`
//!                    match, re-read at construction. Verified on the
//!                    Surface 12 Nov 23:18 CET: `BAT1`, capacity **100**,
//!                    status "Not charging" (the video's 81% was stale
//!                    too). `ADP1` is the AC adapter, not a battery, and
//!                    is ignored by the `BAT` prefix filter. None/absent
//!                    => hide the whole battery cluster (QS behaviour).
//!   wifi on + SSID   WIRED in iter10 (`live_wifi()` below): `nmcli -t -f
//!                    ACTIVE,SSID dev wifi` — the line starting `yes:`
//!                    gives the connected SSID; `iw dev` fallback parses
//!                    the `ssid` line of the managed interface. Verified
//!                    on the Surface 13 Nov 00:39 CET: `yes:SNORKY2`,
//!                    `iw dev` wlan0 `ssid SNORKY2`. No `yes:` line (nmcli
//!                    ran, nothing connected) => wifi off, subtext "Off".
//!                    Both commands failing => keeps the old fallback
//!                    (wifi on, SSID unknown, subtext "Connected") —
//!                    documented, not silent. SSID truncated to 20 chars
//!                    so it cannot overflow the 201 px pill.
//!                    (critic defect 8: subtext must be the real SSID.)
//!   bluetooth on     WIRED in iter13 (`live_bluetooth_on()`):
//!                    `bluetoothctl show` => "Powered: yes"; fallback
//!                    `rfkill list bluetooth` (soft/hard blocked => off).
//!                    Verified 13 Nov 01:50 CET: `Powered: yes`,
//!                    rfkill "Soft blocked: no". Both failing => video
//!                    default ON.
//!   focus on         WIRED in iter13 (`live_focus_on()`): mirrors swaync
//!                    Do-Not-Disturb via `swaync-client -D` ("true"/"false").
//!                    JUDGEMENT CALL (flagged for the critic): MangoWM
//!                    exposes no DND protocol, so the island's Focus
//!                    toggle has no native live source; swaync DND is the
//!                    machine's actual do-not-disturb state and the
//!                    closest real equivalent. Verified 13 Nov 01:50 CET:
//!                    `swaync-client -D` => `false` (Focus OFF, matches
//!                    the video). swaync not running / command failing =>
//!                    video default OFF.
//!   night light on   WIRED in iter34 a6a: `wlsunset` EXISTS on this
//!                    Surface (/usr/bin/wlsunset, verified 13 Nov 18:30
//!                    CET) but was NOT running — the video's lit ON was
//!                    therefore a frozen lie. State is now
//!                    `pgrep -x wlsunset`; toggling starts it
//!                    (`wlsunset -l 50.1 -L 8.7 -t 3500 -T 6500`, Frankfurt
//!                    coords, so at night it is warm immediately) or kills
//!                    it. `gammastep` / `hyprshade` are still absent.
//!   display level    LIVE since iter94 (un-froze the iter85 0.58 — the
//!                    iter94 task explicitly ordered live tracking, and
//!                    the reader is now proven: `brightnessctl g`/`m`
//!                    16832/96000 = 17.5% on a lit panel, and iter34
//!                    verified set+readback tracking 17.5% -> 27% ->
//!                    80%). History: iter13 read sysfs backlight
//!                    (returned 0 / 96000 on a lit panel 13 Nov 01:50
//!                    CET); iter34 made `brightnessctl g`/`m` primary;
//!                    iter35 trusted a 0 as a real 0. The refresher
//!                    polls `live_display_level()` every 5 s (fallback
//!                    0.58 only if BOTH readers fail), `cc_set_display()`
//!                    writes the dragged level to the cache at once
//!                    (like Sound), and the 1 s fingerprint timer
//!                    repaints on change — so the thumb/fill now follow
//!                    the real brightness, and the panel seeds from the
//!                    live value on open (cache at most 5 s old).
//!   sound level      WIRED in iter13 (`live_sound_level()`):
//!                    `wpctl get-volume @DEFAULT_AUDIO_SINK@` — parse
//!                    "Volume: 0.40" (a trailing "[MUTED]" is ignored;
//!                    the level is still the level); fallback
//!                    `pactl get-sink-volume @DEFAULT_SINK@` — parse the
//!                    "40%" token. Verified 13 Nov 01:50 CET: wpctl
//!                    "Volume: 0.40 [MUTED]", pactl "40%" — both agree.
//!                    Both failing => video default 0.90.
//!                    CONSEQUENCE FOR THE CRITIC: after this deploys, the
//!                    Sound slider shows the LIVE 0.40 (fill end ≈164
//!                    logical), NOT the video's 0.90 — pixel-diffing the
//!                    sliders against s07.png will now FAIL by design.
//!                    The new check is "slider == live system value".
//!   notifications    WIRED in iter13 (`live_notification_count()`):
//!                    `swaync-client -c` prints the pending count.
//!                    Verified 13 Nov 01:50 CET: prints `4`, stable
//!                    across two runs; swaync pid 13167 running.
//!                    The bell block renders "N notifications" /
//!                    "N unread since HH:MM" when the count is > 0 and
//!                    keeps the video's empty state ("All caught up" /
//!                    "Nothing new since HH:MM") when it is 0 or unknown.
//!                    The per-notification LIST rows have no video spec
//!                    yet — that design is still open, do not invent it.
//!                    Do NOT invent notifications; a failed read keeps
//!                    the empty state (count `None`).
//! ---------------------------------------------------------------------------

use fontdue::Font;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
pub const CC_W: u32 = 438;
/// iter85: 525 -> 421 — panel sized to its content (see header geometry).
/// iter89: 421 -> 409 (whole stack -12 with GRID_Y).
/// iter91: 409 -> 401 (R3 card tightening: card 99 -> 91 tall, bell
/// block -4, padding 12 -> 8; panel bottom margin stays 14).
/// REQUIRED companion changes OUTSIDE this file (do not edit from here,
/// main.rs belongs to another builder — integrator, apply in the SAME
/// build or a debug build PANICS in `draw_controlcenter_panel`'s
/// `debug_assert_eq!((w, h), (CC_W, CC_H))` on CC open):
/// - main.rs:191 `IslandState::size()`: `Self::ControlCenter =>
///   (438, 409)` must become `(438, 401)`.
/// - main.rs:407 comment "tallest is ControlCenter 409" -> 401.
/// - main.rs:445 comment "shadow bottom is 29 + 409 + 15 = 453" ->
///   29 + 401 + 15 = 445.
/// - main.rs:5485 nav-disc mid-height `(IslandState::ControlCenter,
///   233.5), // 29 + 409/2` -> `(IslandState::ControlCenter, 229.5),
///   // 29 + 401/2` — the a3b nav discs centre on the panel's own
///   height, so this literal MUST move with CC_H.
/// - main.rs:5480 comment "(... CC_H 421 -> 409)" -> 401 (history).
/// The stage-fit test at main.rs:4340-4356 uses the literal 525 and
/// still passes with 401 (smaller fits).
/// iter109: 401 -> 447 — three power-level buttons below the card
/// (row 401..433, 14 px above and below it).
pub const CC_H: u32 = 447;
pub const CC_RADIUS: f32 = 20.0;
pub const CC_PAD: f32 = 14.0; // inner padding (settings readout: 14 px)
pub const CC_CONTENT_W: f32 = CC_W as f32 - 2.0 * CC_PAD; // 410

pub const C_PANEL: (u8, u8, u8) = (0, 0, 0);
pub const C_PILL_BG: (u8, u8, u8) = (12, 11, 14); // #0C0B0E, video-sampled
/// iter36 a6b: the live theme accent (library palette slot 3)
/// drives the toggle-on circles + slider fills. C_TOGGLE_ON /
/// C_SLIDER_FILL keep the video-matched values as the fallback when
/// no theme is set (library::theme_accent falls back to #7C96D5).
/// The on-glyph flips to white on a dark/saturated accent (same
/// luminance rule as power_panel).
pub fn accent() -> (u8, u8, u8) {
    if crate::library::manifest().is_some() {
        crate::library::theme_accent()
    } else {
        C_TOGGLE_ON
    }
}

/// Slider-fill accent: the theme accent when the library is present,
/// the video-matched #74A3F6 otherwise.
fn slider_accent() -> (u8, u8, u8) {
    if crate::library::manifest().is_some() {
        crate::library::theme_accent()
    } else {
        C_SLIDER_FILL
    }
}

fn accent_glyph() -> (u8, u8, u8) {
    let a = accent();
    let lum = (0.299 * a.0 as f32 + 0.587 * a.1 as f32 + 0.114 * a.2 as f32) / 255.0;
    if lum < 0.5 {
        (255, 255, 255)
    } else {
        C_TOGGLE_ON_GLYPH
    }
}

pub const C_TOGGLE_ON: (u8, u8, u8) = (147, 164, 245); // #93A4F5, video
pub const C_TOGGLE_ON_GLYPH: (u8, u8, u8) = (11, 27, 58); // #0B1B3A, dark navy
pub const C_TOGGLE_OFF: (u8, u8, u8) = (52, 55, 60); // #34373C (QS)
pub const C_TOGGLE_OFF_GLYPH: (u8, u8, u8) = (255, 255, 255);
pub const C_SLIDER_FILL: (u8, u8, u8) = (116, 163, 246); // #74A3F6, video
pub const C_SLIDER_TRACK: (u8, u8, u8) = (51, 54, 59); // #33363B (QS)
pub const C_PRIMARY: (u8, u8, u8) = (255, 255, 255);
/// iter85: #9AA0A6 (154,160,166) -> #BEC4CA (190,196,202). The old
/// sub-label grey failed legibility ("dim grey on dark grey"); the
/// brighter value reads ~10:1 on the pill bg and ~7:1 on the notif
/// card. Used for sub-labels, slider labels, bell glyph, end-button
/// glyph fallback — everything secondary.
pub const C_SECONDARY: (u8, u8, u8) = (190, 196, 202); // #BEC4CA
/// iter85: slider thumb — solid white disc with a thin dark ring so it
/// reads on both the periwinkle fill and the dark track, at ANY level
/// including 0% (the old knob-less bar rendered 0% as a dead bar).
pub const C_THUMB: (u8, u8, u8) = (255, 255, 255);
pub const C_THUMB_RING: (u8, u8, u8) = (40, 42, 48);
/// iter86: end-button row-action treatment — a BARE white chevron
/// with no pill, disc or outline (the iter85 outlined 28x36 pill
/// read as a nav-disc clone: same 28 width class as the Ø28 nav
/// discs in main.rs). The 28x36 hit rect is unchanged.
/// iter91: chevron DIMMED to (140,146,152) and shrunk 18 -> 14 px in
/// draw_slider: the bare WHITE 18 px chevron still read as a stray
/// nav arrow (same white, same stroke language as the nav-disc
/// chevrons, floating on black at the row end). Dimmer + smaller it
/// reads as a quiet row affordance; the ">" itself is the video's.
pub const C_END_BTN_CHEVRON: (u8, u8, u8) = (140, 146, 152);
pub const C_BELL_BG: (u8, u8, u8) = (26, 28, 31); // #1A1C1F (QS fieldBg)
/// iter91: (32,34,38) -> (13,12,15) — VIDEO-sampled from s07.png
/// centre column (y 112..176 frame px: (13,12,15) steady). The old
/// (32,34,38) put a ~28-level step above the (5,5,5) panel while the
/// video's card is only ~8 levels lighter — that step is what made
/// the replica's card read as a grey slab (~93% empty by pixel area
/// on the v0.33 shot). At (13,12,15) the card is the video's subtle
/// inset again; the bell disc (26,28,31) still reads against it.
pub const C_NOTIF_CARD: (u8, u8, u8) = (13, 12, 15); // iter34 a6a: the
// notifications CARD behind the bell block (video frame: a distinct inset
// card, slightly lighter than the panel). iter36 a6b (R3): sized to its
// content (bell + two text lines, 12 px padding) instead of filling the
// panel bottom — the old 175 px tall card framed ~100 px of void.
// iter91 (R3, tightened again): padding 12 -> 8, bell block -4,
// card 99 -> 91 tall (296..387), panel 409 -> 401.
pub const SLIDER_GLYPH_PX: f32 = 20.0; // iter34 a6a: fill glyph em size —
/// the old 16 px fill glyph and 14 px end-button glyph read tiny and
/// mismatched (PIL: fill ink ~22 logical wide but end-button ink only
/// ~11). Fill glyph is now 20 px; the end button is a chevron (video).
pub const NOTIF_CARD_R: f32 = 16.0;

const HEADER_Y: f32 = 2.0; // iter89: 14.0 -> 2.0 (first ink ~5 px below panel top)
const HEADER_H: f32 = 22.0;
/// iter91: hairline divider under the header row (iter90 beauty
/// critic: "No divider, no grouping, nothing ties it to the clock on
/// the other side" — the ~250 px void between the clock and the
/// battery cluster made the header read as a status-bar fragment
/// pasted into a black card). 1 px, full content width, dark grey —
/// a header rule, not a nav element: it groups clock + power/battery
/// into one header band. The power text stays right-anchored next to
/// the battery (iter90 pixel-PASS, 8.7 px text->icon gap); the
/// divider, not a move, is the anchoring fix.
const HEADER_DIVIDER_Y: f32 = 30.0;
const C_HEADER_DIVIDER: (u8, u8, u8) = (36, 38, 42);
const GRID_Y: f32 = 36.0; // iter89: 48 -> 36 (keep the 12 px header->grid gap)
const PILL_W: f32 = 201.0;
const PILL_H: f32 = 52.0;
const PILL_R: f32 = 12.0;
const GRID_GAP: f32 = 8.0;
const CIRCLE_D: f32 = 48.0; // video-measured icon circle (critic defect 11)
const CIRCLE_MARGIN: f32 = 10.0;
const TEXT_DX: f32 = CIRCLE_MARGIN + CIRCLE_D + 9.0; // 67 — name/sub x offset
const TRACK_X: f32 = 14.0;
const TRACK_W: f32 = 376.0; // ends 390; 28 px end button at 396..424
/// iter85: track height 20 -> 36 (~double the old 20 px bar, per the
/// a6a brief). 36 — not a full 40 — so the Sound track still clears
/// the iter27 no-op point (panel-local (219, 303)): track ends 294,
/// hit slack (track + 4) ends 298, 303 stays NoOp. Radius is half
/// the height: the track is a full pill.
const TRACK_H: f32 = 36.0;
const TRACK_R: f32 = 18.0;
/// iter85: thumb Ø30 — chunkier than the old (nonexistent) knob, but
/// smaller than the 36 px track, so it never protrudes above/below
/// the bar; clamped inside the track by `thumb_cx()`.
/// iter107: Ø30 -> Ø28, and the clamp is gone (`thumb_cx()` spans
/// the full track, so the rendered centre matches the live value at
/// 0% and 100% too). Ø28 is the largest thumb that still never leaves
/// the PANEL: at 0% it spans [track.x - 14, track.x + 14] = [0, 28]
/// (flush with the panel edge), at 100% [376, 404] — clear of the
/// end-button chevron, whose ink starts ~405.1. Ø30 would poke 1 px
/// past the panel edge at 0% (a white sliver on the wallpaper).
const THUMB_D: f32 = 28.0;
const END_BTN: f32 = 28.0; // iter86: hit-rect width only — no pill is drawn anymore (bare chevron).
const BELL_D: f32 = 34.0; // video-scaled (critic defect 10; QS used 44)
/// iter85: bell block re-anchored to the compact layout (was 400.0 —
/// see header anchor notes). Card 308..407: bell 320..354.
/// iter91 (R3): bell block -4 with the card tightening — bell
/// 304..338, card 296..387 (91 tall, 8 px padding), panel 409 -> 401.
const BELL_CY: f32 = 321.0; // iter91: 325 -> 321 (R3 card tightening, -4)
/// iter85: was 430.0. Title top = bell bottom (354) + 10 gap.
/// iter91: 352 -> 348 (bell block -4).
const NOTIF_TITLE_Y: f32 = 348.0; // iter91: 352 -> 348 (R3, -4)
/// iter85: was 447.0. Sub top = title top + 16 (13 px line) + 2 gap;
/// sub line ends ~395, card bottom 407 = 395 + 12 padding.
/// iter91: 370 -> 366 (bell block -4); sub line ends ~379, card
/// bottom 387 = 379 + 8 padding.
const NOTIF_SUB_Y: f32 = 366.0; // iter91: 370 -> 366 (R3, -4)

/// Power-level row below the notification card: three equal pills.
const POWER_Y: f32 = 401.0;
const POWER_H: f32 = 32.0;
const POWER_GAP: f32 = 8.0;
const POWER_R: f32 = 10.0;

// Submenus (Bluetooth + Sound): same 14 px side padding, no new
// panel height. Both share the row geometry — they are the same
// compact list with a different header.
const BT_ROWS: usize = 5;
const BT_ROW_H: f32 = 48.0;
const BT_ROW_GAP: f32 = 6.0;

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
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// Every element rect, computed from the constants above so the spec is
/// auditable in one place. The geometry measurer checks THESE numbers
/// against s07.png.
#[derive(Clone, Copy, Debug)]
pub struct CcLayout {
    pub panel: Rect,
    pub header: Rect,
    pub header_divider: Rect, // 1 px hairline under the header row (iter91)
    pub toggle_pills: [Rect; 4], // row-major: Wi-Fi, Focus, Bluetooth, Night Light
    pub toggle_circles: [Rect; 4],
    pub display_label: Rect, // text top row (11 px); y only, w unused
    pub display_track: Rect,
    pub display_end_btn: Rect,
    pub sound_label: Rect,
    pub sound_track: Rect,
    pub sound_end_btn: Rect,
    pub bell_circle: Rect,
    pub notif_title: Rect, // text top row (13 px); y only, w unused
    pub notif_sub: Rect,   // text top row (10 px); y only, w unused
    pub notif_card: Rect,  // iter34 a6a: inset card behind the bell block
    pub power_buttons: [Rect; 3], // Saver / Balanced / Performance
    // Bluetooth submenu (shown in place of the normal body).
    pub bt_back: Rect,
    pub bt_power_pill: Rect,
    pub bt_power_circle: Rect,
    pub bt_switch: Rect,
    pub bt_devices_card: Rect,
    pub bt_device_rows: [Rect; BT_ROWS],
    // Sound submenu (audio output routes).
    pub sound_card: Rect,
    pub sound_rows: [Rect; BT_ROWS],
}

pub fn cc_layout() -> CcLayout {
    let pill = |col: usize, row: usize| {
        Rect::new(
            CC_PAD + col as f32 * (PILL_W + GRID_GAP),
            GRID_Y + row as f32 * (PILL_H + GRID_GAP),
            PILL_W,
            PILL_H,
        )
    };
    let pills = [pill(0, 0), pill(1, 0), pill(0, 1), pill(1, 1)];
    let circles = pills.map(|p| {
        Rect::new(
            p.x + CIRCLE_MARGIN,
            p.y + (PILL_H - CIRCLE_D) / 2.0,
            CIRCLE_D,
            CIRCLE_D,
        )
    });
    // iter85: slider rows are NO LONGER video-anchored (the video's
    // 207..227 / 276..296 rows belong to its 20 px tracks and its empty
    // bands). Compact column, no gap > 14: grid ends 148, Display
    // label 160, track 178..214 (36 px), Sound label 228, track
    // 246..282, card 296..395, panel bottom 409. Labels sit 4 px above
    // their track (11 px text in a 14 px line box).
    // iter89: whole stack -12 with GRID_Y 48 -> 36.
    let display_track = Rect::new(TRACK_X, 178.0, TRACK_W, TRACK_H);
    let sound_track = Rect::new(TRACK_X, 246.0, TRACK_W, TRACK_H);
    // iter86: the end-button HIT rect still spans the full track
    // height (28x36) — only its drawing changed (bare chevron now).
    let end_btn = |track: Rect| {
        Rect::new(
            CC_W as f32 - CC_PAD - END_BTN,
            track.y,
            END_BTN,
            TRACK_H,
        )
    };
    let power_w = (CC_CONTENT_W - 2.0 * POWER_GAP) / 3.0;
    let power_buttons = [
        Rect::new(CC_PAD, POWER_Y, power_w, POWER_H),
        Rect::new(CC_PAD + power_w + POWER_GAP, POWER_Y, power_w, POWER_H),
        Rect::new(CC_PAD + 2.0 * (power_w + POWER_GAP), POWER_Y, power_w, POWER_H),
    ];
    let bt_back = Rect::new(CC_PAD, 38.0, 32.0, 32.0);
    let bt_power_pill = Rect::new(CC_PAD, 76.0, CC_CONTENT_W, 56.0);
    let bt_power_circle = Rect::new(bt_power_pill.x + 10.0, bt_power_pill.y + 4.0, 48.0, 48.0);
    let bt_switch = Rect::new(
        bt_power_pill.x + bt_power_pill.w - 50.0, bt_power_pill.cy() - 11.0, 40.0, 22.0,
    );
    let bt_devices_card = Rect::new(CC_PAD, 160.0, CC_CONTENT_W, 273.0);
    let bt_device_rows = std::array::from_fn(|i| {
        Rect::new(
            bt_devices_card.x + 4.0,
            bt_devices_card.y + 4.5 + i as f32 * (BT_ROW_H + BT_ROW_GAP),
            bt_devices_card.w - 8.0,
            BT_ROW_H,
        )
    });
    let sound_card = Rect::new(CC_PAD, 76.0, CC_CONTENT_W, 357.0);
    let sound_rows = std::array::from_fn(|i| {
        Rect::new(
            sound_card.x + 4.0,
            sound_card.y + 4.5 + i as f32 * (BT_ROW_H + BT_ROW_GAP),
            sound_card.w - 8.0,
            BT_ROW_H,
        )
    });
    CcLayout {
        panel: Rect::new(0.0, 0.0, CC_W as f32, CC_H as f32),
        header: Rect::new(CC_PAD, HEADER_Y, CC_CONTENT_W, HEADER_H),
        header_divider: Rect::new(CC_PAD, HEADER_DIVIDER_Y, CC_CONTENT_W, 1.0),
        toggle_pills: pills,
        toggle_circles: circles,
        display_label: Rect::new(CC_PAD, 160.0, CC_CONTENT_W, 14.0),
        display_track,
        display_end_btn: end_btn(display_track),
        sound_label: Rect::new(CC_PAD, 228.0, CC_CONTENT_W, 14.0),
        sound_track,
        sound_end_btn: end_btn(sound_track),
        bell_circle: Rect::new(
            CC_W as f32 / 2.0 - BELL_D / 2.0,
            BELL_CY - BELL_D / 2.0,
            BELL_D,
            BELL_D,
        ),
        // iter85 (R3, tightened again): the card is sized to its
        // content — bell 320..354, title top 364, sub top 382, content
        // ends ~395 — with 12 px padding (308..407, 99 tall). The
        // iter36 card (371..469) already cut the old 330..505 void;
        // this one rides 63 px higher with the compact column, and the
        // panel bottom margin below it is exactly 14 px (407 -> 421),
        // so there is no dead band anywhere in the panel. The iter27
        // no-op point (panel-local (219, 303)) sits in the 10 px gap
        // between the Sound hit-slack end (298) and the card top (308).
        // iter89: stack -12 — card 296..395, bell 308..342, title 352,
        // sub 370, panel 409. NOTE the iter27 no-op point (219, 303)
        // is now INSIDE the card (top 296), not in a gap: still NoOp,
        // because the card has no click action (only the Sound track's
        // hit slack, ending 286, must clear it — see the test).
        // iter91 (R3): padding 12 -> 8, bell block -4 — card 296..387
        // (91 tall), bell 304..338, title 348, sub 366, panel 401.
        notif_card: Rect::new(CC_PAD, 296.0, CC_CONTENT_W, 91.0),
        notif_title: Rect::new(CC_PAD, NOTIF_TITLE_Y, CC_CONTENT_W, 16.0),
        notif_sub: Rect::new(CC_PAD, NOTIF_SUB_Y, CC_CONTENT_W, 14.0),
        power_buttons,
        bt_back,
        bt_power_pill,
        bt_power_circle,
        bt_switch,
        bt_devices_card,
        bt_device_rows,
        sound_card,
        sound_rows,
    }
}

// ---------------------------------------------------------------------------
// Data — defaults reproduce the VIDEO frame (s07), not the live machine,
// so a screenshot can be diffed against s07.png pixel-for-pixel.
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToggleKey {
    Wifi,
    Focus,
    Bluetooth,
    NightLight,
}

pub struct ToggleDef {
    pub key: ToggleKey,
    pub icon: &'static str, // Material Symbols glyph name (see icon_char)
    pub name: &'static str,
    /// Icon em size in px. Ink-calibrated (iter34 a6a, PIL on the a3d
    /// shot): at the old uniform 16 px the ink was 14.0 / 10.7 / 12.0
    /// logical for wifi / bluetooth / nightlight in a 48 px disc
    /// (0.29 / 0.22 / 0.25 of the disc — "absolute garbage", wildly
    /// inconsistent). iter35 pixel critic (strictly-inside-disc):
    /// wifi 0.433 / bluetooth 0.573 / nightlight 0.545 / focus 0.545
    /// in a ~47 px disc. iter36 trims to the 0.50-0.55 band by linear
    /// ink scaling: wifi 29 -> 35 (0.433 x 35/29 = 0.52), bluetooth
    /// 34 -> 32 (0.573 x 32/34 = 0.54). iter37 measured wifi 0.493
    /// at 35.0 (SDF ink is not linear) - iter39 nudges wifi 35 -> 37
    /// (predicts ~0.52). Night Light 31 and Focus
    /// (procedural) already pass — untouched.
    /// iter84 VERIFIED on the live shot: Wi-Fi 0.535 strict (0.521
    /// strict / 0.534 AA-incl. on re-measure) — KEEP 37.0, do not
    /// shrink it. Bluetooth 0.507 strict at 32.0 (iter37) — inside
    /// the 0.50-0.55 band, KEEP 32.0. Focus 0.534 / Night Light 0.507
    /// (iter37 strict) — inside the band, untouched. iter85 therefore
    /// changes NO glyph size: all four discs are the uniform Ø48
    /// (CIRCLE_D) drawn as true circles (radius CIRCLE_D / 2) with
    /// optically centred glyphs at 0.50-0.55 of the disc. The iter84
    /// typography note ("2 of 4 CC glyphs undersized") names no glyph
    /// and contradicts the strict measurements above; if a later
    /// critic re-measures a glyph below 0.50, nudge THAT glyph's
    /// `glyph_px` by +1-2 px — never the disc.
    /// iter91 (R2, AGREED METHOD — pinned down because the band was
    /// measured three different ways across iters): the 0.50-0.55
    /// band applies to the glyph's VERTICAL ink extent (ink bbox
    /// height / disc diameter), strict inside-disc basis, measured
    /// on the live v0.33 shot 14 Nov 2026: Wi-Fi 0.549, Bluetooth
    /// 0.521, Focus 0.528, Night Light 0.528 — ALL FOUR in band, so
    /// NO glyph_px changes in iter91 (Wi-Fi 37.0 / BT 32.0 / NL 31.0
    /// / Focus ring 11.5 all confirmed). The WIDTH is explicitly NOT
    /// the metric: the glyphs have irreconcilable aspect ratios —
    /// Wi-Fi is a wide fan (ink 52x39 phys, w 0.732), Bluetooth a
    /// tall narrow mark (23x37, w 0.324), Night Light a crescent
    /// (27x38, w 0.375). No single size puts both widths in band:
    /// shrinking Wi-Fi until its width hits 0.55 would drop its
    /// height to ~0.41 and make it visibly smaller than the other
    /// three. A verifier MUST use the height ratio, or Wi-Fi will
    /// "fail" on a property no size can fix.
    /// iter95 (R2 glyph-band fix, agreed method): Wi-Fi 37.0 -> 35.5,
    /// Night Light 31.0 -> 29.0. iter94 critic: Wi-Fi 40 phys (0.556) /
    /// NL 41@>45, 40@>80 FAIL; BT + Focus 39 (0.542) PASS, untouched.
    /// Linear ink prediction: Wi-Fi ~38.4 phys (0.533), NL ~37.9 (0.526)
    /// — mid-band 0.52-0.53. Disc (48) and band (0.50-0.55) unchanged.
    /// iter98 (RESTORE, 14 Nov 2026): iter95's fix was LOST — it lived
    /// only in the iter95 scratch tree, the live tree was never synced,
    /// so iter96 (built in the live tree) + iter97 (cp -a of it) shipped
    /// the pre-fix 37.0/31.0 again; v0.38/v0.39 measure byte-identical
    /// to v0.36. NOT the accent: NL's OFF fill is the same grey in both
    /// versions, and Focus/BT kept height 39 across mint->periwinkle.
    /// Values restored to the iter95-proven 35.5/29.0, no further nudge.
    /// iter86: Focus ring r 12.0 -> 11.5 in draw_focus_target: iter85
    /// measured 0.571, above the band; predicted ~0.53-0.55 now.
    /// Focus is procedural (see draw_focus_target) and ignores this.
    pub glyph_px: f32,
}

/// Row-major grid order, matching shell.qml and the video.
pub const TOGGLES: [ToggleDef; 4] = [
    ToggleDef { key: ToggleKey::Wifi, icon: "wifi", name: "Wi-Fi", glyph_px: 35.5 },
    ToggleDef { key: ToggleKey::Focus, icon: "center_focus", name: "Focus", glyph_px: 0.0 },
    ToggleDef { key: ToggleKey::Bluetooth, icon: "bluetooth", name: "Bluetooth", glyph_px: 32.0 },
    ToggleDef { key: ToggleKey::NightLight, icon: "nightlight", name: "Night Light", glyph_px: 29.0 },
];

/// One known Bluetooth device (paired/bonded or recently discovered).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BtDevice {
    pub addr: String,
    pub name: String,
    pub paired: bool,
    pub connected: bool,
}

/// One PipeWire/PulseAudio output sink a user can route sound to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioRoute {
    pub name: String, // pactl sink name — the id used by set-default-sink
    pub desc: String, // human label ("Built-in Audio Analog Stereo")
}

#[derive(Clone, Debug)]
pub struct ControlCenterData {
    pub time_hhmm: String,
    pub battery_pct: Option<u8>, // None => hide the battery cluster
    /// iter90: header power-source text — None => hide (unknown);
    /// "On battery" / "Plugged in" / "Plugged in · 45 W".
    pub power_source_text: Option<String>,
    pub wifi_on: bool,
    pub wifi_ssid: Option<String>, // None => fall back to "Connected"/"Off"
    /// iter85: the Wi-Fi RADIO state (`nmcli radio`), cached by the
    /// refresher alongside the connected state. The toggle decision
    /// uses the radio: on-but-disconnected must toggle OFF, not ON
    /// (old code read this synchronously in `cc_toggle` — a blocking
    /// nmcli on the input path — now it reads the cache instead).
    pub wifi_radio_on: bool,
    pub focus_on: bool,
    pub bluetooth_on: bool,
    /// r1c finding 2: false when NO bluetooth adapter state could be
    /// read at all (`live_bluetooth_on() == None`). The panel used to
    /// default to `true`, so an unreadable/no adapter rendered
    /// accent-blue "On" — a lie. `bluetooth_on` alone is false in both
    /// the "off" and "no adapter" cases, so the two are distinguished
    /// here and the subtext says "No adapter".
    pub bluetooth_available: bool,
    pub bt_devices: Vec<BtDevice>,
    /// Audio routes are only enumerated while the Sound submenu is open.
    pub audio_routes: Vec<AudioRoute>,
    pub audio_default: String,
    pub night_light_on: bool,
    pub display_level: f32, // 0.0..=1.0
    pub sound_level: f32,   // 0.0..=1.0
    pub notifications_since_hhmm: String,
    pub notification_count: Option<u32>, // None => unknown (swaync read failed); Some(0) => empty state
}

impl ControlCenterData {
    /// The video frame s07, exactly: "00:05", battery 81%, Wi-Fi on,
    /// Focus off, Bluetooth on, Night Light ON (lit in the video — critic
    /// defect 3; QS hard-codes it off), Display 0.58, Sound 0.90
    /// (video-measured, iter10 fixer — the old 0.63 / 0.78 were the
    /// Quickshell defaults, not the video).
    ///
    /// REFERENCE ONLY — for pixel-diffing a render against s07.png.
    /// Do NOT pass this to the live island: its time and battery are
    /// frozen at the video's values (iter8 Defect 2). Use `live()`.
    pub fn video_defaults() -> Self {
        Self {
            time_hhmm: "00:05".to_string(),
            battery_pct: Some(81),
            power_source_text: None,
            wifi_on: true,
            wifi_ssid: None, // video shows an SSID-like string; unreadable
            wifi_radio_on: true, // video shows connected => radio on
            focus_on: false,
            bluetooth_on: true,
            bluetooth_available: true,
            bt_devices: Vec::new(),
            audio_routes: Vec::new(),
            audio_default: String::new(),
            night_light_on: true,
            display_level: 0.58,
            sound_level: 0.90,
            notifications_since_hhmm: "00:05".to_string(),
            notification_count: None, // video shows the empty state
        }
    }

    /// Live data for the real island — CACHED, never blocking (iter85
    /// NO-FREEZE; same pattern as `music_panel::live_cached()`).
    ///
    /// `live()` only clones the cache and ensures the background
    /// refresher thread is running; it runs NO external command on the
    /// calling thread. The refresher polls the `live_*()` readers every
    /// 5 s on its own thread (each command hard-timeout-guarded by
    /// `run_cmd_timeout()`), so a hung nmcli / bluetoothctl / wpctl can
    /// never stall a frame — main.rs calls `live()` on EVERY draw,
    /// including every morph frame, and the old synchronous version
    /// spawned ~10 processes per frame. First call after daemon start
    /// returns the video-defaults seed with the LIVE clock (cheap libc
    /// call) until the refresher's first poll lands (~100-300 ms later;
    /// morph frames re-read `live()`, so the settled frame is fresh).
    /// After that the cache is at most 5 s old. A volume change made
    /// while the panel is open shows on the next redraw within ~5 s.
    ///
    /// Sources (iter9/10/13, unchanged): time libc, battery sysfs,
    /// wifi/SSID nmcli (+ iw fallback), bluetooth `bluetoothctl show`
    /// (+ rfkill fallback), Focus swaync DND, Night Light pgrep
    /// wlsunset (iter34 a6a), Sound wpctl (+ pactl fallback, video
    /// 0.90 fallback). Display is LIVE since iter94
    /// (`live_display_level()`: `brightnessctl g`/`m` primary, sysfs
    /// fallback, 0.58 only if both readers fail — see the header LIVE
    /// DATA note). No new crates: `std::sync` / `std::process` only.
    ///
    /// INTEGRATOR (done in v0.4 — verified 13 Nov 00:40 CET, do not redo):
    /// main.rs already calls `ControlCenterData::live()` at the
    /// `IslandState::ControlCenter` draw arm, and the minute-boundary
    /// timer callback already includes `|| state.state ==
    /// IslandState::ControlCenter`, so a CC panel left open across a
    /// minute boundary redraws. NOTE for the main.rs owner (iter85):
    /// the clipboard's `apply_state_instant` fires a one-shot 250 ms
    /// redraw after entry so its threaded refresh lands on screen;
    /// CC had no equivalent until iter89 — the first CC open after daemon start
    /// shows the video-defaults seed until the next draw trigger
    /// (morph frames usually cover it). If a stale first frame is
    /// ever photographed, the same 250 ms one-shot redraw for `IslandState::ControlCenter`
    /// now lives in main.rs (`arm_cc_entry_timer`, iter89) — no change
    /// needed in this file for that.
    pub fn live() -> Self {
        ensure_cc_refresher();
        let cache = CC_CACHE.get_or_init(|| {
            std::sync::RwLock::new(cc_seed())
        });
        cache.read().unwrap().clone()
    }

    pub fn toggle(&self, key: ToggleKey) -> bool {
        match key {
            ToggleKey::Wifi => self.wifi_on,
            ToggleKey::Focus => self.focus_on,
            ToggleKey::Bluetooth => self.bluetooth_on,
            ToggleKey::NightLight => self.night_light_on,
        }
    }

    /// Subtext under a toggle name. Wi-Fi shows the real SSID when known
    /// (critic defect 8); everything else shows its state word.
    pub fn subtext(&self, def: &ToggleDef) -> String {
        let on = self.toggle(def.key);
        match def.key {
            ToggleKey::Wifi => {
                if on {
                    self.wifi_ssid.clone().unwrap_or_else(|| "Connected".to_string())
                } else {
                    "Off".to_string()
                }
            }
            ToggleKey::NightLight => {
                // TODO: when wired to gammastep/hyprsunset, show its schedule
                // (e.g. "Until 07:00") instead of the bare state word.
                if on { "On".to_string() } else { "Off".to_string() }
            }
            ToggleKey::Bluetooth if !self.bluetooth_available => "No adapter".to_string(),
            _ => {
                if on { "On".to_string() } else { "Off".to_string() }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// NO-FREEZE cache (iter85) — mirrors music_panel::live_cached().
// ---------------------------------------------------------------------------
/// Hard timeout for any single system command on the refresher thread.
/// Local daemon calls (nmcli / wpctl / swaync-client) answer in
/// 5-50 ms; 1500 ms is generous and still bounds a hung daemon to a
/// stall the user never sees (it runs off the hot path entirely).
const CMD_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(1500);
/// Refresher poll interval — the cache is at most this old.
const CC_REFRESH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

static CC_CACHE: std::sync::OnceLock<std::sync::RwLock<ControlCenterData>> =
    std::sync::OnceLock::new();

/// Seed for the cache before the first poll lands: the video defaults
/// with the LIVE clock, so the header time is never the frozen "00:05"
/// even on the very first frame.
fn cc_seed() -> ControlCenterData {
    let mut d = ControlCenterData::video_defaults();
    let now = live_local_hhmm();
    d.time_hhmm = now.clone();
    d.notifications_since_hhmm = now;
    d
}

/// One full refresh from the real system (runs ONLY on the refresher
/// thread — never on the render or input path). Display is LIVE since
/// iter94 (`live_display_level()`, 0.58 fallback only if both readers
/// fail); everything else is live as before.
fn read_live_snapshot() -> ControlCenterData {
    let now = live_local_hhmm();
    let (wifi_on, wifi_ssid) = live_wifi();
    let bt = live_bluetooth_on();
    ControlCenterData {
        time_hhmm: now.clone(),
        battery_pct: live_battery_pct(),
        power_source_text: live_power_source_text(),
        wifi_on,
        wifi_ssid,
        wifi_radio_on: live_radio_wifi_on().unwrap_or(wifi_on),
        focus_on: live_focus_on().unwrap_or(false),
        bluetooth_on: bt.unwrap_or(false),
        bluetooth_available: bt.is_some(),
        // Device enumeration is 3 bluetoothctl calls: only pay for it
        // while the submenu is actually visible.
        bt_devices: if cc_bt_submenu_open() {
            live_bluetooth_devices()
        } else {
            Vec::new()
        },
        // Two pactl calls per poll, only while the submenu is visible.
        audio_routes: if cc_sound_submenu_open() {
            live_audio_routes()
        } else {
            Vec::new()
        },
        audio_default: if cc_sound_submenu_open() {
            live_audio_default().unwrap_or_default()
        } else {
            String::new()
        },
        night_light_on: live_night_light_on().unwrap_or(false),
        display_level: live_display_level().unwrap_or(0.58), // iter94: live; 0.58 only if both readers fail
        sound_level: live_sound_level().unwrap_or(0.90),
        notifications_since_hhmm: now,
        notification_count: live_notification_count(),
    }
}

/// Spawn the refresher thread once (first poll immediately, then every
/// CC_REFRESH_INTERVAL). Idempotent; safe to call from `live()`.
fn ensure_cc_refresher() {
    static STARTED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    STARTED.get_or_init(|| {
        // NOTE: the trailing `;` matters — without it the closure
        // returns the JoinHandle and `OnceLock<()>` fails to compile.
        std::thread::spawn(|| loop {
            refresh_cc_cache();
            std::thread::sleep(CC_REFRESH_INTERVAL);
        });
    });
}

/// One live snapshot into the cache. Pending toggles keep their
/// optimistic bit (iter86) instead of being un-flipped by a stale poll.
fn refresh_cc_cache() {
    let data = read_live_snapshot();
    if let Some(cache) = CC_CACHE.get() {
        let mut cur = cache.write().unwrap();
        *cur = merge_snapshot_preserving_pending(data, &cur, toggle_pending_counts());
    }
}

// Bluetooth submenu: a view flag inside the control center, not a new
// IslandState — the panel keeps its 438x447 geometry and its morph/input
// paths. main.rs resets it whenever the panel is entered or left.
static BT_SUBMENU_OPEN: AtomicBool = AtomicBool::new(false);
static BT_SCAN_RUNNING: AtomicBool = AtomicBool::new(false);

pub fn cc_bt_submenu_open() -> bool {
    BT_SUBMENU_OPEN.load(Ordering::Relaxed)
}

pub fn cc_bt_scanning() -> bool {
    BT_SCAN_RUNNING.load(Ordering::Relaxed)
}

/// Opening the submenu runs ONE bounded discovery pass off-thread so
/// nearby devices appear in the list without a 5 s cache wait.
pub fn cc_bt_submenu_set(open: bool) {
    BT_SUBMENU_OPEN.store(open, Ordering::Relaxed);
    if open {
        SOUND_SUBMENU_OPEN.store(false, Ordering::Relaxed);
    }
    if open && !BT_SCAN_RUNNING.swap(true, Ordering::Relaxed) {
        std::thread::spawn(|| {
            // Cached devices first: the submenu is useful instantly.
            refresh_cc_cache();
            if live_bluetooth_on().unwrap_or(false) {
                let _ = run_cmd_timeout(
                    "bluetoothctl",
                    &["--timeout", "5", "scan", "on"],
                    std::time::Duration::from_secs(8),
                );
            }
            // Second read picks up anything the scan just discovered.
            refresh_cc_cache();
            BT_SCAN_RUNNING.store(false, Ordering::Relaxed);
        });
    }
}

/// Pair if needed, then connect — off the input path, with hard
/// timeouts so a PIN prompt can never freeze the island.
pub fn cc_bt_connect(index: usize) {
    let dev = CC_CACHE.get().and_then(|c| {
        c.read()
            .ok()
            .and_then(|d| d.bt_devices.get(index).cloned())
    });
    if let Some(dev) = dev {
        std::thread::spawn(move || {
            if !dev.paired {
                let _ = run_cmd_timeout(
                    "bluetoothctl",
                    &["pair", &dev.addr],
                    std::time::Duration::from_secs(12),
                );
            }
            let _ = run_cmd_timeout(
                "bluetoothctl",
                &["connect", &dev.addr],
                std::time::Duration::from_secs(12),
            );
            refresh_cc_cache();
        });
    }
}

// Sound submenu: same pattern as Bluetooth — a view flag, not a new
// IslandState, so the panel geometry and morph paths stay untouched.
static SOUND_SUBMENU_OPEN: AtomicBool = AtomicBool::new(false);

pub fn cc_sound_submenu_open() -> bool {
    SOUND_SUBMENU_OPEN.load(Ordering::Relaxed)
}

pub fn cc_sound_submenu_set(open: bool) {
    SOUND_SUBMENU_OPEN.store(open, Ordering::Relaxed);
    if open {
        BT_SUBMENU_OPEN.store(false, Ordering::Relaxed);
    }
    if open {
        // Off the input path: pactl can take ~1.5 s per call.
        std::thread::spawn(refresh_cc_cache);
    }
}

/// Set the default sink and move existing streams to it off the input
/// path. `set-default-sink` alone leaves already-playing audio on the
/// old device, so the sink-inputs move is what actually reroutes sound.
pub fn cc_audio_select(index: usize) {
    let route = CC_CACHE.get().and_then(|c| {
        c.read()
            .ok()
            .and_then(|d| d.audio_routes.get(index).cloned())
    });
    if let Some(route) = route {
        if let Some(cache) = CC_CACHE.get() {
            cache.write().unwrap().audio_default = route.name.clone();
        }
        std::thread::spawn(move || {
            let name = route.name.as_str();
            let _ = run_cmd_timeout(
                "pactl",
                &["set-default-sink", name],
                std::time::Duration::from_secs(5),
            );
            if let Some(out) = run_cmd_timeout(
                "pactl",
                &["list", "short", "sink-inputs"],
                std::time::Duration::from_secs(5),
            ) {
                for line in String::from_utf8_lossy(&out.stdout).lines() {
                    if let Some(id) = line.split_whitespace().next() {
                        if id.parse::<u32>().is_ok() {
                            let _ = run_cmd_timeout(
                                "pactl",
                                &["move-sink-input", id, name],
                                std::time::Duration::from_secs(5),
                            );
                        }
                    }
                }
            }
            refresh_cc_cache();
        });
    }
}

/// Optimistic cache write after a user action (iter85): the click has
/// already dispatched its command on a detached thread, so flip the
/// cached bit now and the immediate `draw_if_ready()` in main.rs shows
/// the new state at once; the refresher corrects it from the real
/// system within ~5 s if the command failed. (Display has its own
/// optimistic write, `cache_set_display`, since iter94 — a drag moves
/// the shown level at once, like Sound.)
fn cache_set_toggle(key: ToggleKey, on: bool) {
    if let Some(cache) = CC_CACHE.get() {
        let mut d = cache.write().unwrap();
        match key {
            ToggleKey::Wifi => {
                d.wifi_on = on;
                d.wifi_radio_on = on;
            }
            ToggleKey::Focus => d.focus_on = on,
            ToggleKey::Bluetooth => d.bluetooth_on = on,
            ToggleKey::NightLight => d.night_light_on = on,
        }
    }
}

fn cache_set_sound(level: f32) {
    if let Some(cache) = CC_CACHE.get() {
        cache.write().unwrap().sound_level = level.clamp(0.0, 1.0);
    }
}

/// Optimistic cache write for a Display drag (iter94 — mirrors
/// `cache_set_sound`): the `brightnessctl set` is dispatched detached,
/// so write the dragged level now and the immediate `draw_if_ready()`
/// in main.rs moves the thumb/fill at once; the refresher confirms the
/// live value within ~5 s. (One race, same as Sound: a poll landing
/// before the detached set executes can briefly show the old level;
/// the next poll corrects it.)
fn cache_set_display(level: f32) {
    if let Some(cache) = CC_CACHE.get() {
        cache.write().unwrap().display_level = level.clamp(0.0, 1.0);
    }
}

/// Run `cmd args` with a HARD timeout, capturing stdout. `None` on
/// spawn failure, non-zero exit, or timeout (the child is killed and
/// reaped on timeout, so a hung daemon cannot pile up processes).
/// Every `live_*()` reader below goes through this — no bare
/// `.output()` remains on any CC path. Stdout is drained on a short
/// reader thread so a chatty command cannot deadlock the wait.
/// The `_raw` sibling returns the output regardless of exit status;
/// it exists for `pgrep`, where non-zero IS the answer ("not running").
fn run_cmd_timeout(
    cmd: &str,
    args: &[&str],
    timeout: std::time::Duration,
) -> Option<std::process::Output> {
    let out = run_cmd_timeout_raw(cmd, args, timeout)?;
    if out.status.success() {
        Some(out)
    } else {
        None
    }
}

fn run_cmd_timeout_raw(
    cmd: &str,
    args: &[&str],
    timeout: std::time::Duration,
) -> Option<std::process::Output> {
    let mut child = std::process::Command::new(cmd)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        use std::io::Read;
        let _ = stdout.read_to_end(&mut buf);
        let _ = tx.send(buf);
    });
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let buf = rx
                    .recv_timeout(std::time::Duration::from_millis(200))
                    .unwrap_or_default();
                // No success check here — the caller (`run_cmd_timeout`
                // wrapper) applies it; `_raw` callers need the status
                // itself (pgrep).
                return Some(std::process::Output {
                    status,
                    stdout: buf,
                    stderr: Vec::new(),
                });
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err(_) => return None,
        }
    }
}

// ---------------------------------------------------------------------------
// Live readers (iter9) — no new crates, no main.rs changes.
// ---------------------------------------------------------------------------
/// Live local time as "HH:MM", byte-for-byte the same call main.rs
/// `hhmm()` makes (libc `time` + `localtime_r`), so the CC header and
/// the collapsed pill can never disagree again.
pub fn live_local_hhmm() -> String {
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        format!("{:02}:{:02}", tm.tm_hour, tm.tm_min)
    }
}

/// Live battery level (0..=100) from sysfs, or `None` when the machine
/// has no battery / the value is unreadable — in which case the draw
/// code hides the whole battery cluster instead of showing a stale
/// number. Reads the first `BAT*` entry under
/// `/sys/class/power_supply/` (sorted), i.e. `BAT1/capacity` on the
/// Surface (verified 12 Nov 23:18 CET: 100, "Not charging"). The `ADP1`
/// adapter entry is not a battery and is skipped by the `BAT` filter.
/// Never returns a hardcoded fallback percentage.
pub fn live_battery_pct() -> Option<u8> {
    let dir = std::fs::read_dir("/sys/class/power_supply").ok()?;
    let mut bats: Vec<std::path::PathBuf> = dir
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().starts_with("BAT"))
                .unwrap_or(false)
        })
        .collect();
    bats.sort();
    let capacity = std::fs::read_to_string(bats.first()?.join("capacity")).ok()?;
    let pct: u8 = capacity.trim().parse().ok()?;
    Some(pct.min(100))
}

/// Live power-source text for the CC header (iter90, watts restored
/// iter94): "On battery" when the AC adapter is offline, "Plugged in"
/// on AC, plus the battery power flow in whole watts whenever it is
/// known ("Plugged in · 45 W" — including 0 W when the battery is
/// Full and `power_now` reads 0; the iter90 formatter dropped the
/// suffix unless status was "Charging" with w > 0, so a full battery
/// silently lost the readout). Reads /sys/class/power_supply only (no
/// process spawn): the AC entry is the one whose `type` is "Mains"
/// (ADP1 on the Surface — its NAME does not start with "AC"), the
/// battery is the first BAT* entry. Watts from `power_now` (µW) or,
/// when absent, `current_now` × `voltage_now` (µA × µV = 1e-12 W).
/// None when nothing is readable — the draw code then hides the text
/// instead of showing a stale value. Runs ONLY on the refresher
/// thread (read_live_snapshot); `live()` stays cache-only.
pub fn live_power_source_text() -> Option<String> {
    let dir = std::fs::read_dir("/sys/class/power_supply").ok()?;
    let entries: Vec<std::path::PathBuf> = dir
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    let mut ac_online: Option<bool> = None;
    let mut bat_status: Option<String> = None;
    let mut bat_watts: Option<u32> = None;
    for p in &entries {
        let is_mains = std::fs::read_to_string(p.join("type"))
            .map(|t| t.trim().to_string())
            .ok()
            .as_deref()
            == Some("Mains");
        if is_mains {
            ac_online = std::fs::read_to_string(p.join("online"))
                .ok()
                .and_then(|s| s.trim().parse::<u8>().ok())
                .map(|v| v != 0);
        }
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if name.starts_with("BAT") {
            if bat_status.is_none() {
                bat_status =
                    std::fs::read_to_string(p.join("status")).ok().map(|s| s.trim().to_string());
            }
            if bat_watts.is_none() {
                bat_watts = read_battery_watts(p);
            }
        }
    }
    format_power_source(ac_online, bat_status.as_deref().unwrap_or(""), bat_watts)
}

/// Battery charge/discharge power in whole watts, rounded
/// (44,892,000 µW -> 45 W). `power_now` first; falls back to
/// `current_now` × `voltage_now` when `power_now` is absent.
fn read_battery_watts(bat: &std::path::Path) -> Option<u32> {
    if let Ok(s) = std::fs::read_to_string(bat.join("power_now")) {
        if let Ok(uw) = s.trim().parse::<i64>() {
            if uw >= 0 {
                return Some(microwatts_to_watts(uw));
            }
        }
    }
    let cur: i64 = std::fs::read_to_string(bat.join("current_now"))
        .ok()?
        .trim()
        .parse()
        .ok()?;
    let vol: i64 = std::fs::read_to_string(bat.join("voltage_now"))
        .ok()?
        .trim()
        .parse()
        .ok()?;
    if cur < 0 || vol < 0 {
        return None;
    }
    Some(((cur as f64 * vol as f64) / 1_000_000_000_000.0).round() as u32)
}

/// µW -> whole watts, rounded (not truncated).
fn microwatts_to_watts(uw: i64) -> u32 {
    (uw as f64 / 1_000_000.0).round() as u32
}

/// Pure formatter for the header power text (unit-tested below).
/// `ac_online`: Some(true) AC plugged, Some(false) on battery,
/// None unknown. `status`: BAT `status` ("Charging", "Discharging",
/// "Full", "Not charging", ...). `watts`: battery power flow when
/// known. iter94: on AC the watts suffix shows whenever watts is
/// known — including 0 W (Full battery, `power_now` 0) and any
/// status — because the value IS the reading; hiding it at 0 made
/// the header lose the readout exactly when the battery filled up.
fn format_power_source(
    ac_online: Option<bool>,
    status: &str,
    watts: Option<u32>,
) -> Option<String> {
    match ac_online {
        // iter94: watts suffix whenever known on AC (incl. 0 W / Full).
        Some(true) => match watts {
            Some(w) => Some(format!("Plugged in · {w} W")),
            None => Some("Plugged in".to_string()),
        },
        Some(false) => Some("On battery".to_string()),
        None => {
            if status == "Discharging" {
                Some("On battery".to_string())
            } else {
                None
            }
        }
    }
}

/// Live Wi-Fi state + SSID, or the honest fallback (iter10).
///
/// Primary source: `nmcli -t -f ACTIVE,SSID dev wifi`. A line starting
/// with `yes:` names the connected network — everything after the first
/// `:` is the SSID (SSIDs may themselves contain `:`). If nmcli runs
/// but no `yes:` line exists, the radio is off / nothing is connected
/// and this returns `(false, None)` — the pill then shows "Off".
///
/// Fallback: `iw dev`, first `ssid` line of the interface whose block
/// says `type managed` (the p2p-dev interface has no ssid line).
///
/// If BOTH commands fail (no NetworkManager, no iw), this returns
/// `(true, None)`: the previous behaviour is preserved and the pill
/// shows the documented placeholder "Connected" instead of wrongly
/// claiming the radio is off because of a tooling failure. That
/// fallback is honest only as a placeholder — it must never be
/// presented as a measured SSID.
///
/// Verified on the Surface 13 Nov 00:39 CET: nmcli prints
/// `yes:SNORKY2` (wlan0, connected) and `iw dev` shows
/// `ssid SNORKY2`. SSIDs are truncated to 20 chars so the 9 px
/// subtext cannot overflow the 201 px pill (text starts at pill_x+67).
///
/// Cost: two short-lived process spawns per CC draw. Draws happen on
/// state entry and at minute boundaries only, so this is negligible;
/// do NOT call it per frame (there is no per-frame draw in this daemon).
pub fn live_wifi() -> (bool, Option<String>) {
    // iter85: timeout-guarded (was a bare blocking `.output()`).
    if let Some(out) = run_cmd_timeout(
        "nmcli",
        &["-t", "-f", "ACTIVE,SSID", "dev", "wifi"],
        CMD_TIMEOUT,
    ) {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if let Some(ssid) = line.strip_prefix("yes:") {
                let ssid = ssid.trim();
                if ssid.is_empty() {
                    return (true, None);
                }
                return (true, Some(truncate_ssid(ssid)));
            }
        }
        // nmcli ran, nothing connected.
        return (false, None);
    }
    // nmcli missing/failed — try iw before giving up.
    if let Some(ssid) = live_wifi_iw_fallback() {
        return (true, Some(truncate_ssid(&ssid)));
    }
    (true, None)
}

/// Truncate an SSID to 20 chars (char-boundary safe) for the pill subtext.
fn truncate_ssid(ssid: &str) -> String {
    ssid.chars().take(20).collect()
}

/// `iw dev` fallback for `live_wifi()`: returns the SSID of the first
/// managed interface block, or `None`. Parses lines of the form
/// `Interface wlan0` (new block), `ssid <name>` (pending for the block),
/// `type managed` (confirms the pending ssid belongs to a managed iface).
fn live_wifi_iw_fallback() -> Option<String> {
    // iter85: timeout-guarded (was a bare blocking `.output()`).
    let out = run_cmd_timeout("iw", &["dev"], CMD_TIMEOUT)?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut pending: Option<String> = None;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("Interface ") {
            pending = None;
        } else if let Some(ssid) = t.strip_prefix("ssid ") {
            pending = Some(ssid.trim().to_string());
        } else if t == "type managed" {
            if let Some(ssid) = pending.take() {
                if !ssid.is_empty() {
                    return Some(ssid);
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Live readers (iter13) — display / sound / bluetooth / focus /
// notifications. Same rules as the iter9/iter10 readers: no new crates,
// `std::fs` / `std::process` only, each returns `None` on any failure so
// `live()` can fall back to the documented video value instead of
// showing a wrong number. All verified against the Surface 13 Nov
// 01:50 CET — see the per-function notes.
// ---------------------------------------------------------------------------
/// Live display (backlight) level 0.0..=1.0, or `None`.
///
/// Source: `/sys/class/backlight/*/brightness` /
/// `/sys/class/backlight/*/max_brightness` (first sorted match — the
/// Surface has exactly one backlight-class device, `intel_backlight`).
///
/// A brightness of **0 is the honest live value** (iter35 R1): the
/// old code treated 0 as a failed read and `live()` fell back to the
/// video 0.58 — the iter34 verifier caught the slider showing 0.58
/// while the system read 0. `brightnessctl` was verified in iter34
/// tracking real changes (17.5% -> 27% -> 80% with exact readbacks),
/// so a 0 from it is trusted as a real 0, and `None` is reserved for
/// both readers actually failing. Caveat kept on record: on 13 Nov
/// 01:50 sysfs AND brightnessctl both read 0 on a demonstrably lit
/// panel (a possible intel_backlight quirk or idle-blank) — if the
/// panel is ever visibly lit while this returns 0.0, that quirk, not
/// the fallback, is the thing to chase.
pub fn live_display_level() -> Option<f32> {
    // iter34 a6a: brightnessctl FIRST — `brightnessctl g` / `m`
    // (verified 13 Nov 18:30 CET: 16832/96000 = 17.5% on a lit panel,
    // where the old sysfs-only read could return 0 and the slider then
    // showed the 1.00 fallback... in fact the a3d shot showed a FULL
    // Display bar while the real level was 17.5%).
    if let (Some(cur), Some(max)) = (brightnessctl_value("g"), brightnessctl_value("m")) {
        if max > 0.0 && cur >= 0.0 && cur <= max {
            return Some((cur / max).clamp(0.0, 1.0));
        }
    }
    let dir = std::fs::read_dir("/sys/class/backlight").ok()?;
    let mut devs: Vec<std::path::PathBuf> =
        dir.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    devs.sort();
    let dev = devs.first()?;
    let cur: f32 = std::fs::read_to_string(dev.join("brightness"))
        .ok()?
        .trim()
        .parse()
        .ok()?;
    let max: f32 = std::fs::read_to_string(dev.join("max_brightness"))
        .ok()?
        .trim()
        .parse()
        .ok()?;
    if max <= 0.0 || cur < 0.0 || cur > max {
        return None;
    }
    Some((cur / max).clamp(0.0, 1.0))
}

/// Live sound (sink volume) level 0.0..=1.0, or `None`.
///
/// Primary: `wpctl get-volume @DEFAULT_AUDIO_SINK@` — parse the first
/// number in "Volume: 0.40" / "Volume: 0.40 [MUTED]". A trailing
/// `[MUTED]` is ignored: the slider shows the LEVEL, and there is no
/// mute glyph spec for this state, so mute is not rendered (the level
/// is still the level).
///
/// Fallback: `pactl get-sink-volume @DEFAULT_SINK@` — parse the "40%"
/// token in "Volume: front-left: 26216 /  40% / -23.87 dB, ...".
///
/// Verified 13 Nov 01:50 CET: wpctl "Volume: 0.40 [MUTED]" and pactl
/// "40%" agree. Both failing => `None` (live() falls back to 0.90).
pub fn live_sound_level() -> Option<f32> {
    // iter85: timeout-guarded (was bare blocking `.output()` calls).
    if let Some(out) = run_cmd_timeout(
        "wpctl",
        &["get-volume", "@DEFAULT_AUDIO_SINK@"],
        CMD_TIMEOUT,
    ) {
        let text = String::from_utf8_lossy(&out.stdout);
        for tok in text.split_whitespace() {
            if let Ok(v) = tok.parse::<f32>() {
                if (0.0..=1.5).contains(&v) {
                    return Some(v.clamp(0.0, 1.0));
                }
            }
        }
    }
    if let Some(out) = run_cmd_timeout(
        "pactl",
        &["get-sink-volume", "@DEFAULT_SINK@"],
        CMD_TIMEOUT,
    ) {
        let text = String::from_utf8_lossy(&out.stdout);
        for tok in text.split_whitespace() {
            if let Some(pct) = tok.strip_suffix('%') {
                if let Ok(v) = pct.parse::<f32>() {
                    return Some((v / 100.0).clamp(0.0, 1.0));
                }
            }
        }
    }
    None
}

/// Parse `pactl list sinks` blocks into output routes. Pure so the
/// shape can be unit-tested without a running sound server.
fn parse_pactl_sinks(text: &str) -> Vec<AudioRoute> {
    let mut out: Vec<AudioRoute> = Vec::new();
    for block in text.split("\n\n") {
        let mut name: Option<String> = None;
        let mut desc: Option<String> = None;
        for line in block.lines() {
            let line = line.trim();
            if let Some(v) = line.strip_prefix("Name:") {
                name = Some(v.trim().to_string());
            } else if let Some(v) = line.strip_prefix("Description:") {
                desc = Some(v.trim().to_string());
            }
        }
        if let Some(name) = name {
            if !name.is_empty() && !out.iter().any(|r| r.name == name) {
                let desc = desc.filter(|d| !d.is_empty()).unwrap_or_else(|| name.clone());
                out.push(AudioRoute { name, desc });
            }
        }
    }
    out
}

/// Available audio outputs, or an empty list if pactl is unavailable.
fn live_audio_routes() -> Vec<AudioRoute> {
    run_cmd_timeout("pactl", &["list", "sinks"], CMD_TIMEOUT)
        .map(|o| parse_pactl_sinks(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

/// Name of the current default sink.
fn live_audio_default() -> Option<String> {
    let out = run_cmd_timeout("pactl", &["get-default-sink"], CMD_TIMEOUT)?;
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// Live Bluetooth power state, or `None`.
///
/// Primary: `bluetoothctl show` — the "Powered: yes/no" line.
/// Fallback: `rfkill list bluetooth` — any "blocked: yes" => off,
/// "Soft blocked: no" => on.
///
/// Verified 13 Nov 01:50 CET: `bluetoothctl show` => "Powered: yes",
/// rfkill "Soft blocked: no" / "Hard blocked: no". Both failing =>
/// `None` (live() falls back to the video ON).
pub fn live_bluetooth_on() -> Option<bool> {
    // iter85: timeout-guarded (was bare blocking `.output()` calls;
    // bluetoothctl is the slowest reader when the daemon is busy).
    if let Some(out) = run_cmd_timeout("bluetoothctl", &["show"], CMD_TIMEOUT) {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if let Some(v) = line.trim().strip_prefix("Powered:") {
                return Some(v.trim() == "yes");
            }
        }
    }
    if let Some(out) = run_cmd_timeout("rfkill", &["list", "bluetooth"], CMD_TIMEOUT) {
        let text = String::from_utf8_lossy(&out.stdout);
        if text.contains("Soft blocked: yes") || text.contains("Hard blocked: yes") {
            return Some(false);
        }
        if text.contains("Soft blocked: no") {
            return Some(true);
        }
    }
    None
}

fn bt_line(line: &str) -> Option<(String, String)> {
    let mut parts = line.split_whitespace();
    if parts.next()? != "Device" {
        return None;
    }
    let addr = parts.next()?.to_string();
    let name = parts.collect::<Vec<_>>().join(" ");
    Some((
        addr.clone(),
        if name.is_empty() { addr } else { name },
    ))
}

/// Pure helper: merge the three `bluetoothctl devices` views into rows.
fn parse_bt_devices(all: &str, paired: &str, connected: &str) -> Vec<BtDevice> {
    let paired_set: std::collections::HashSet<String> =
        paired.lines().filter_map(bt_line).map(|(a, _)| a).collect();
    let connected_set: std::collections::HashSet<String> =
        connected.lines().filter_map(bt_line).map(|(a, _)| a).collect();
    let mut out: Vec<BtDevice> = Vec::new();
    for (addr, name) in all.lines().filter_map(bt_line) {
        if out.iter().any(|d| d.addr == addr) {
            continue;
        }
        out.push(BtDevice {
            paired: paired_set.contains(&addr),
            connected: connected_set.contains(&addr),
            addr,
            name,
        });
    }
    out
}

/// Nearby / bonded devices from BlueZ's cache. Every call is
/// timeout-guarded — a slow daemon must never stall the island draw.
pub fn live_bluetooth_devices() -> Vec<BtDevice> {
    let read = |args: &[&str]| {
        run_cmd_timeout("bluetoothctl", args, CMD_TIMEOUT)
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default()
    };
    let all = read(&["devices"]);
    let paired = read(&["devices", "Paired"]);
    let connected = read(&["devices", "Connected"]);
    parse_bt_devices(&all, &paired, &connected)
}

/// Live Focus state, or `None`.
///
/// Source: swaync Do-Not-Disturb via `swaync-client -D` ("true"/"false").
/// JUDGEMENT CALL — see the module header: MangoWM exposes no DND
/// protocol, so the island's Focus toggle has no native live source;
/// swaync DND is the machine's actual do-not-disturb state and the
/// closest real equivalent. Verified 13 Nov 01:50 CET: `swaync-client
/// -D` => `false` (Focus OFF, matches the video). swaync not running
/// or the command failing => `None` (live() falls back to OFF).
pub fn live_focus_on() -> Option<bool> {
    // iter85: timeout-guarded (was a bare blocking `.output()`).
    let out = run_cmd_timeout("swaync-client", &["-D"], CMD_TIMEOUT)?;
    match String::from_utf8_lossy(&out.stdout).trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// Live pending-notification count from swaync, or `None`.
///
/// Source: `swaync-client -c` prints the current count as a bare
/// integer. Verified 13 Nov 01:50 CET: prints `4`, stable across two
/// runs (swaync pid 13167, config `~/.config/mango/swaync/`).
/// `None` (swaync not running / command failing) means "unknown" — the
/// bell block then keeps the empty state rather than inventing a count.
/// There is NO D-Bus fallback wired: `swaync-client -c` is the
/// documented count interface and it works.
pub fn live_notification_count() -> Option<u32> {
    // iter85: timeout-guarded (was a bare blocking `.output()`).
    let out = run_cmd_timeout("swaync-client", &["-c"], CMD_TIMEOUT)?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

/// One `brightnessctl` number (`g` = current, `m` = max), or `None`.
/// iter85: timeout-guarded (was a bare blocking `.output()`). iter94:
/// primary source for the LIVE Display slider via
/// `live_display_level()` (see the header LIVE DATA note).
fn brightnessctl_value(arg: &str) -> Option<f32> {
    let out = run_cmd_timeout("brightnessctl", &[arg], CMD_TIMEOUT)?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

/// Live Night Light state: is the wlsunset daemon running? `None` only
/// if the `pgrep` call itself fails (wlsunset binary missing etc.).
/// iter85: timeout-guarded via `run_cmd_timeout_raw` — `pgrep` exits
/// non-zero when nothing matches, so the exit status IS the answer
/// here (not a failure).
pub fn live_night_light_on() -> Option<bool> {
    let out = run_cmd_timeout_raw("pgrep", &["-x", "wlsunset"], CMD_TIMEOUT)?;
    Some(out.status.success())
}

/// Live Wi-Fi RADIO state (`nmcli radio`), distinct from `live_wifi()`
/// (which reports whether a network is connected). The toggle decision
/// uses the radio: on-but-disconnected must toggle OFF, not ON.
/// iter85: timeout-guarded (was a bare blocking `.output()`).
pub fn live_radio_wifi_on() -> Option<bool> {
    let out = run_cmd_timeout(
        "nmcli",
        &["-t", "-f", "WIFI", "radio"],
        CMD_TIMEOUT,
    )?;
    match String::from_utf8_lossy(&out.stdout).trim() {
        "enabled" => Some(true),
        "disabled" => Some(false),
        _ => None,
    }
}

/// Dispatch a command WITHOUT waiting for it (iter85 NO-FREEZE): the
/// caller (main.rs press arm) returns to the next frame immediately.
/// The command runs on a detached thread with a hard timeout; its exit
/// status is irrelevant to the UI — the refresher re-reads the real
/// state within ~5 s, and the optimistic cache flip (see
/// `cache_set_toggle` / `cache_set_sound`) already showed the action.
/// This REPLACES the old blocking `run_ok()`: `nmcli radio wifi off`
/// and `bluetoothctl power off` can each block a second or more, and
/// the old code ran them synchronously on the input path.
fn run_detached(cmd: &str, args: &[&str]) {
    let cmd = cmd.to_string();
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    std::thread::spawn(move || {
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let _ = run_cmd_timeout(&cmd, &refs, CMD_TIMEOUT);
    });
}

// ---------------------------------------------------------------------------
// iter86: per-toggle SERIAL command queue — double-click race fix.
// ---------------------------------------------------------------------------
/// ROOT CAUSE (iter85 stress; reproduced iter86 on v0.28: 7 rapid
/// Bluetooth flips from OFF ended OFF instead of ON): `cc_toggle`
/// dispatched every flip on its OWN detached thread, so two
/// `bluetoothctl power off/on` commands ran CONCURRENTLY and could
/// complete out of order — the second flip's command landing before the
/// first's, swallowing a flip. A second, rarer hazard: the 5 s refresher
/// overwrote the whole cache with a snapshot read BEFORE a pending
/// command landed, so a click arriving in that window re-read the stale
/// pre-click state and dispatched the SAME direction twice.
///
/// FIX, two parts, both in this file (dispatch/cache logic only — no
/// drawing/layout change):
///  1. Commands for one toggle execute IN ORDER on that toggle's single
///     worker thread (FIFO queue). Clicks still flip the cache
///     optimistically at once, so the UI stays instant; the worker drains
///     the queue one command at a time and re-reads the REAL state into
///     the cache after each command — but only when the queue is empty,
///     so a queued next flip's optimistic value is never clobbered.
///  2. While a toggle has jobs pending, the refresher's snapshot write
///     keeps the CACHED bits for that toggle (see
///     `merge_snapshot_preserving_pending`) instead of overwriting them
///     with a possibly-stale read.
/// Together: N rapid clicks enqueue N alternating commands, executed in
/// order — N flips, always, for every toggle (Wi-Fi included, same path;
/// Wi-Fi is never test-toggled, but its clicks go through this queue).
/// Sliders are untouched: `cc_set_display` / `cc_set_sound` still use
/// `run_detached` — absolute set-commands are idempotent, so their
/// ordering cannot swallow a change.
///
/// LOCK ORDER (no deadlock): no code ever holds the queue lock while
/// taking the cache lock. `cc_toggle` takes the cache lock, drops it,
/// then the queue lock. The worker takes the queue lock, drops it, runs
/// the command, then takes the queue lock (pending count), drops it,
/// then the cache lock (refresh). The refresher holds the cache lock
/// while taking the queue lock only for a `pending` snapshot read —
/// a brief, non-blocking take it releases before writing.
///
/// One system command per toggle job. A job's command has a hard
/// 1500 ms timeout (`CMD_TIMEOUT`), so a burst of 7 clicks drains in
/// ~2 s worst case, typically < 1 s; the UI never waits for it.
#[derive(Clone, Debug)]
struct ToggleJob {
    key: ToggleKey,
    /// Command + args, e.g. ("bluetoothctl", ["power", "off"]).
    /// Night Light ON is ("sh", ["-c", "setsid -f wlsunset ..."]).
    cmd: &'static str,
    args: Vec<String>,
}

fn toggle_key_index(key: ToggleKey) -> usize {
    match key {
        ToggleKey::Wifi => 0,
        ToggleKey::Focus => 1,
        ToggleKey::Bluetooth => 2,
        ToggleKey::NightLight => 3,
    }
}

/// One FIFO per toggle + a pending count per toggle, under one lock.
/// `pending[i]` counts jobs enqueued but not yet finished for toggle i
/// (incremented on push, decremented after the job's command + settle).
struct ToggleQueueState {
    queues: [std::collections::VecDeque<ToggleJob>; 4],
    pending: [usize; 4],
}

impl ToggleQueueState {
    fn new() -> Self {
        Self {
            queues: [
                std::collections::VecDeque::new(),
                std::collections::VecDeque::new(),
                std::collections::VecDeque::new(),
                std::collections::VecDeque::new(),
            ],
            pending: [0; 4],
        }
    }

    fn push(&mut self, job: ToggleJob) {
        let i = toggle_key_index(job.key);
        self.pending[i] += 1;
        self.queues[i].push_back(job);
    }
}

static TOGGLE_QUEUES: std::sync::OnceLock<std::sync::Mutex<ToggleQueueState>> =
    std::sync::OnceLock::new();
static TOGGLE_QUEUE_CV: std::sync::Condvar = std::sync::Condvar::new();

fn toggle_queues() -> &'static std::sync::Mutex<ToggleQueueState> {
    TOGGLE_QUEUES.get_or_init(|| std::sync::Mutex::new(ToggleQueueState::new()))
}

/// Pending-job counts per toggle (index by `toggle_key_index`).
/// No queue yet (no toggle ever clicked) means all counts are zero.
fn toggle_pending_counts() -> [usize; 4] {
    match TOGGLE_QUEUES.get() {
        Some(q) => q.lock().unwrap().pending,
        None => [0; 4],
    }
}

/// Spawn the four worker threads once (first click). Idempotent.
fn ensure_toggle_workers() {
    static STARTED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    STARTED.get_or_init(|| {
        for idx in 0..4 {
            std::thread::spawn(move || toggle_worker(idx));
        }
        // NOTE: the trailing `;` matters — see `ensure_cc_refresher`.
    });
}

/// One worker per toggle: pops jobs FIFO and runs them strictly one at
/// a time, so rapid clicks can never reorder or overlap a toggle's
/// commands. Runs forever; the queue lock is held only to pop.
fn toggle_worker(idx: usize) {
    loop {
        let job = {
            let q = toggle_queues();
            let mut g = q.lock().unwrap();
            loop {
                if let Some(job) = g.queues[idx].pop_front() {
                    break job;
                }
                g = TOGGLE_QUEUE_CV.wait(g).unwrap();
            }
        };
        run_toggle_job(&job);
        // Settle: pkill needs a moment before pgrep sees the death, and
        // bluetoothctl's D-Bus state can lag its exit by a few ms.
        // Off the hot path (worker thread), so a 200 ms sleep is free.
        std::thread::sleep(std::time::Duration::from_millis(200));
        // Job done: drop the pending count, then — ONLY if nothing else
        // is queued for this toggle — correct the cache from the REAL
        // system state, so a failed command self-heals within ~200 ms.
        let still_pending = {
            let q = toggle_queues();
            let mut g = q.lock().unwrap();
            g.pending[idx] = g.pending[idx].saturating_sub(1);
            g.pending[idx]
        };
        if still_pending == 0 {
            refresh_toggle_from_live(job.key);
        }
    }
}

/// Run one queued job: the exact command `cc_toggle` computed at click
/// time. Exit status is irrelevant — the worker re-reads the real state
/// afterwards, and the refresher corrects the cache within ~5 s anyway.
fn run_toggle_job(job: &ToggleJob) {
    let refs: Vec<&str> = job.args.iter().map(|s| s.as_str()).collect();
    let _ = run_cmd_timeout(job.cmd, &refs, CMD_TIMEOUT);
}

/// Re-read ONE toggle's real state and write it into the cache.
/// Called by the worker after a job completes with an empty queue.
/// A failed read (`None`) leaves the optimistic value in place until
/// the refresher's next poll corrects it.
fn refresh_toggle_from_live(key: ToggleKey) {
    let on = match key {
        ToggleKey::Wifi => live_radio_wifi_on(),
        ToggleKey::Focus => live_focus_on(),
        ToggleKey::Bluetooth => live_bluetooth_on(),
        ToggleKey::NightLight => live_night_light_on(),
    };
    if let Some(on) = on {
        cache_set_toggle(key, on);
    }
}

/// Refresher write path: for any toggle with jobs pending, keep the
/// CACHED bits instead of the snapshot's — the snapshot may predate a
/// pending command and would otherwise un-flip the optimistic bit.
/// Pure (no locks, no I/O) so it is unit-testable; see the tests.
fn merge_snapshot_preserving_pending(
    mut fresh: ControlCenterData,
    cached: &ControlCenterData,
    pending: [usize; 4],
) -> ControlCenterData {
    if pending[toggle_key_index(ToggleKey::Wifi)] > 0 {
        fresh.wifi_on = cached.wifi_on;
        fresh.wifi_radio_on = cached.wifi_radio_on;
    }
    if pending[toggle_key_index(ToggleKey::Focus)] > 0 {
        fresh.focus_on = cached.focus_on;
    }
    if pending[toggle_key_index(ToggleKey::Bluetooth)] > 0 {
        fresh.bluetooth_on = cached.bluetooth_on;
    }
    if pending[toggle_key_index(ToggleKey::NightLight)] > 0 {
        fresh.night_light_on = cached.night_light_on;
    }
    fresh
}

/// Pure intent for one click: the exact system command that flips
/// toggle `key` from its current (cached) state `on` to `!on`.
/// Directional for Wi-Fi / Bluetooth / Night Light (`off` iff currently
/// on); Focus is directionless (`swaync-client -d` is a blind
/// server-side toggle — one invocation per click is still exactly one
/// flip). The command is computed from the state at CLICK time and
/// queued, never re-derived later, so a stale re-read can never turn
/// two clicks into the same command twice.
fn toggle_job_for(key: ToggleKey, on: bool) -> ToggleJob {
    match key {
        ToggleKey::Wifi => ToggleJob {
            key,
            cmd: "nmcli",
            args: vec![
                "radio".to_string(),
                "wifi".to_string(),
                if on { "off".to_string() } else { "on".to_string() },
            ],
        },
        ToggleKey::Focus => ToggleJob {
            key,
            // swaync-client -d toggles DND and prints the new state.
            cmd: "swaync-client",
            args: vec!["-d".to_string()],
        },
        ToggleKey::Bluetooth => ToggleJob {
            key,
            cmd: "bluetoothctl",
            args: vec![
                "power".to_string(),
                if on { "off".to_string() } else { "on".to_string() },
            ],
        },
        ToggleKey::NightLight => ToggleJob {
            key,
            cmd: if on { "pkill" } else { "sh" },
            args: if on {
                vec!["-x".to_string(), "wlsunset".to_string()]
            } else {
                // Frankfurt coords; at night this is warm immediately.
                // Detached like main.rs spawn_exec (setsid + nulled stdio).
                vec![
                    "-c".to_string(),
                    "setsid -f wlsunset -l 50.1 -L 8.7 -t 3500 -T 6500 >/dev/null 2>&1"
                        .to_string(),
                ]
            },
        },
    }
}

/// Perform the real system action for a toggle (iter34 a6a — the wiring
/// Alexandre asked for: "When I'm clicking on Wi-Fi, nothing happens").
/// iter85 NO-FREEZE: this runs on the INPUT path, so it must return
/// immediately. The toggle direction is read from the CACHE (never a
/// live command — the old code ran `nmcli radio` / `bluetoothctl show`
/// / `pgrep` synchronously here, each able to block a second or more),
/// the cached bit is flipped optimistically so the immediate redraw
/// shows the action, and the real command is ENQUEUED on the toggle's
/// serial worker queue (`toggle_queues`).
/// iter86 RACE FIX: the old code dispatched each flip on its own
/// detached thread, so rapid clicks ran their commands concurrently
/// and out of order — one flip swallowed (reproduced iter86: 7 rapid
/// Bluetooth flips from OFF ended OFF instead of ON). Now every click
/// computes its command from the cache at click time and queues it;
/// the worker executes the queue strictly FIFO, and the refresher
/// never overwrites a pending toggle's cached bit with a stale read.
/// The refresher re-reads the real state within ~5 s and the worker
/// re-reads it after each drained job, so the UI can show a local-only
/// flip for at most one refresh interval, and never freezes. Wi-Fi
/// stays WIRED for real via nmcli (radio state from the real system,
/// cached); it is NOT tested here, only kept.
pub fn cc_toggle(key: ToggleKey) {
    // Cached current state (cache always exists after the first draw;
    // fall back to the video defaults if a click somehow precedes it).
    let cached = CC_CACHE
        .get()
        .map(|c| c.read().unwrap().clone())
        .unwrap_or_else(ControlCenterData::video_defaults);
    let on = match key {
        // Radio state decides Wi-Fi (see the `wifi_radio_on` field):
        // on-but-disconnected toggles OFF, not ON.
        ToggleKey::Wifi => cached.wifi_radio_on,
        ToggleKey::Focus => cached.focus_on,
        ToggleKey::Bluetooth => cached.bluetooth_on,
        ToggleKey::NightLight => cached.night_light_on,
    };
    // Optimistic flip FIRST (instant redraw), then enqueue the real
    // command — the worker runs it in click order (iter86 race fix).
    // NOTE: this read-flip-enqueue sequence runs on the single input
    // thread only (main.rs press arm), so two clicks can never
    // interleave here; the races this fix closes are the concurrent
    // detached commands and the refresher's stale overwrite.
    cache_set_toggle(key, !on);
    let job = toggle_job_for(key, on);
    ensure_toggle_workers();
    {
        let q = toggle_queues();
        q.lock().unwrap().push(job);
    }
    TOGGLE_QUEUE_CV.notify_all();
}

/// Toggle by grid index (0 Wi-Fi, 1 Focus, 2 Bluetooth, 3 Night Light),
/// matching `TOGGLES` row-major order. Out-of-range indices are ignored.
pub fn cc_toggle_index(i: usize) {
    if let Some(def) = TOGGLES.get(i) {
        cc_toggle(def.key);
    }
}

/// Set the display level 0.0..=1.0 via brightnessctl (percent, rounded).
/// Clamped to 1..=100%: 0% would black the panel on this driver quirk.
/// iter85: dispatched detached (was blocking `run_ok` on the drag
/// path — every drag step waited on brightnessctl). iter94: optimistic
/// cache write, like Sound — a drag moves the shown thumb/fill at
/// once; the refresher confirms the live value within ~5 s. (The
/// 1% clamp means a drag to the far left shows ~0.01, not 0.00; a
/// true 0.00 only arrives from an external `brightnessctl set 0%`.)
pub fn cc_set_display(level: f32) {
    let pct = (level.clamp(0.0, 1.0) * 100.0).round() as u32;
    let pct = pct.clamp(1, 100);
    let pct_arg = format!("{pct}%");
    cache_set_display(level);
    run_detached("brightnessctl", &["set", &pct_arg]);
}

/// Set the sink volume 0.0..=1.0 via wpctl.
/// iter85: dispatched detached (was blocking `run_ok` on the drag
/// path) + optimistic cache write, so a drag moves the shown fill at
/// once; the refresher confirms the live wpctl value within ~5 s.
pub fn cc_set_sound(level: f32) {
    let v = level.clamp(0.0, 1.0);
    cache_set_sound(v);
    let v_arg = format!("{v:.2}");
    run_detached("wpctl", &["set-volume", "@DEFAULT_AUDIO_SINK@", &v_arg]);
}

// ---------------------------------------------------------------------------
// Power levels: three buttons at the bottom of the CC panel.
// profile + turbo + EPP must be set together: writing platform_profile
// alone leaves energy_performance_preference at "power", so the CPU
// stays clocked down even after the profile says "balanced".
// ---------------------------------------------------------------------------
const POWER_LEVELS: [(&str, &str, &str, &str); 3] = [
    ("Saver",       "low-power",   "1", "power"),
    ("Balanced",    "balanced",    "0", "balance_performance"),
    ("Performance", "performance", "0", "performance"),
];

/// Non-255 while a level change is in flight: the UI shows the click
/// immediately, then falls back to the real sysfs value.
static POWER_PENDING: AtomicU8 = AtomicU8::new(u8::MAX);

fn power_command(i: usize) -> String {
    let (_, profile, turbo, epp) = POWER_LEVELS[i];
    format!(
        "printf {profile} > /sys/firmware/acpi/platform_profile; printf {turbo} > /sys/devices/system/cpu/intel_pstate/no_turbo; for f in /sys/devices/system/cpu/cpu*/cpufreq/energy_performance_preference; do printf {epp} > \"$f\"; done"
    )
}

/// Current level: 0 Saver, 1 Balanced, 2 Performance. Reading one small
/// sysfs file per frame is fine; no external command on the draw path.
pub fn live_power_level() -> u8 {
    let pending = POWER_PENDING.load(Ordering::Relaxed);
    if pending < POWER_LEVELS.len() as u8 {
        return pending;
    }
    match std::fs::read_to_string("/sys/firmware/acpi/platform_profile") {
        Ok(s) => match s.trim() {
            "low-power" => 0,
            "performance" => 2,
            _ => 1,
        },
        Err(_) => 1,
    }
}

/// Apply a level as root. NOPASSWD sudo is configured on this machine;
/// the command is built only from the constant table above (no user input).
pub fn cc_set_power_level(i: usize) {
    if i >= POWER_LEVELS.len() {
        return;
    }
    let cmd = power_command(i);
    POWER_PENDING.store(i as u8, Ordering::Relaxed);
    std::thread::spawn(move || {
        let _ = run_cmd_timeout("sudo", &["-n", "sh", "-c", &cmd], CMD_TIMEOUT);
        POWER_PENDING.store(u8::MAX, Ordering::Relaxed);
    });
}

// ---------------------------------------------------------------------------
// Icons — PUA codepoints to be filled in (see header). Returning None draws
// a hollow-ring placeholder so unwired icons are VISIBLE in screenshots.
// ---------------------------------------------------------------------------
fn icon_char(name: &str) -> Option<char> {
    // PUA codepoints VERIFIED 12 Nov 2026 on the Surface with fontTools
    // against ~/.local/share/fonts/MaterialSymbolsRounded.ttf
    // (sha256 116b9607cde6d2046ac4d2832956fcec451ef02919f6cf32f93f44088e1bfaca).
    // "notifications_none" is ABSENT from this font build, so the bell uses
    // the plain "notifications" glyph (0xE7F5) instead — same bell outline.
    Some(match name {
        "wifi" => '\u{E63E}',
        "bluetooth" => '\u{E1A7}',
        "nightlight" => '\u{F03D}',
        "brightness_6" => '\u{E3AB}',
        "volume_up" => '\u{E050}',
        "headphones" => '\u{E310}',
        "notifications_none" => '\u{E7F5}',
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Drawing primitives — same SDF/Argb8888 style as main.rs.
// Canvas is BGRA bytes, little-endian (Format::Argb8888), premultiplied.
// ---------------------------------------------------------------------------

/// Flat frosted fill (iter31, a3 REVISED) — same treatment as main.rs
/// (frost_fill_px_blur): blurred wallpaper under a plain, neutral
/// opaque. NO glass effects anywhere in this file: no specular, rim
/// light, inner glow, milky stripe or tint tricks — Alexandre cancelled
/// glass on 13 Nov. Blur-only frost under a plain neutral dark fill.
/// Neutral dark floor mixed under the blurred wallpaper (iter29,
/// see main.rs). iter100: FROST_ALPHA_MENU 0.94 -> 0.80 and the floor
/// back to scalar 5.0 — finishing the iter89 unification that iter91
/// reverted for the CC alone. iter91 re-darkened on a (30,28,25)
/// "warm grey" bright-leg reading that conflated the R<>B channel
/// swap fixed in the same iteration; with channels correct, the day
/// file's own sample (89,106,117) is blue-leaning and fills at
/// (21,25,27), B-R +6. iter128 (B4, R5 re-grade): 0.80/5.0 -> 0.92/2.5
/// — the bright leg still filled at mean ~11.3 (a grey slab; the
/// reference panels read ~2-3). At 0.92/2.5 the CC matches main.rs and
/// launcher_panel.rs exactly on every sample (bright sky
/// (170,180,200) -> (15,16,18); white -> (22,22,22)). Honest limit
/// (frost-invest-iter100): over a dark window the backdrop factor is
/// 0 and every view still reads (2,2,2) — this change lifts the
/// bright leg and kills the per-view divergence, it does not make
/// frost visible over a dark desktop. That needs live-backdrop
/// sampling (Option 2, NOT attempted in iter100 — see
/// iterations/frost-build-iter100.md). The no-wallpaper path now
/// returns the same opaque floor [2,2,2,255] as main.rs (iter128;
/// was [4,4,4,255] at 5.0*0.80).
/// CHANNEL ORDER (iter91, read before touching the fill loop):
/// frost_fill_px_blur returns RGB (p[0]=R, from the RGB wallpaper
/// buffer), but the canvas is BGRA — the fill loop MUST write
/// p[2]->B, p[1]->G, p[0]->R. Writing p straight across flips R/B
/// see the iter91 builder report for the one-line fix.
/// iter103: the live path now neutralises at capture (shared
/// strengthened neutralise_sample — the shipped strength left a
/// bright sky at sample B-R ~+29) and adds the high-pass term;
/// see main.rs frost_fill_px_blur for the formula.
/// iter103 (Builder B) — cards/band frost constants. SAME treatment as
/// the panel fill (prep-iter103.md): neutralised live sample +
/// high-pass deviation, own darker floor. The panel fill itself is
/// Builder A's (main.rs + the CC panel loop); these names are the
/// cards-side mirror so the deployer can unify them 1:1:
///   FROST_CARD_FA    <-> A's fa           (prep: ~0.80; iter128: 0.92)
///   FROST_CARD_K     <-> A's k            (prep: 0.6-0.8; 0.70 = centre)
///   FROST_CARD_FLOOR <-> A's panel floor  (2.5 since iter128); cards
///                        use 3.0, i.e. the card mean now tracks the
///                        panel within ~0.5 levels at fa 0.92
///                        ((2.5-3)*0.92 = -0.46) — the floors nearly
///                        coincide, so the card/panel step is ~0
///                        instead of the old 1.6 levels at 0.80/5.0.
/// MERGE (integrator): B's FROST_LOCAL_R (25.0 grid radius) is RETIRED
/// — the local mean now comes from A's precomputed BoxBlur(25) frame
/// (wallpaper_blur::FROST_HP_RADIUS = 25), the exact quantity B's 5x5
/// grid approximated. A strengthened neutralise_sample IN PLACE (same
/// name, exposure 0.30, min strength 0.88) and neutralises the live frame
/// at capture, so the card path below reads lb.sample()/lb.local_mean()
/// directly and must NOT call neutralise_sample a second time (it
/// would apply the exposure twice and the cards would read too dark).
const FROST_CARD_FA: f32 = 0.92;
const FROST_CARD_K: f32 = 0.70;
const FROST_CARD_FLOOR: f32 = 3.0;

const FROST_ALPHA_MENU: f32 = 0.92;
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
/// no local mean pass `lm = bg` (deviation 0). iter128: the deviation
/// is capped at FROST_HP_MAX_DEV (12) — sharp backdrop edges must
/// not print through as bright specks.
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

/// Fill a rounded rect with a solid colour (opaque where the SDF says so).
/// `rect` is in canvas coordinates; the SDF is evaluated at pixel centres.
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

/// Source-over blend of one opaque-colour pixel at coverage `cov` (0..1).
/// Destination is opaque inside the panel, so alpha is forced to 255 —
/// same shortcut as main.rs (glyphs never leave the capsule).
/// iter103 (Builder B, MERGED by the integrator) — local mean of the
/// NEUTRALISED live sample around screen-logical (sx, sy). B's version
/// took a 5x5 offset grid of neutralise_sample(lb.sample(...)) — an
/// approximation of the prep's BoxBlur(25) local mean. In the MERGED
/// tree the live frame is ALREADY neutralised at capture (A's
/// finish_live_frame), so re-neutralising per grid sample would apply
/// the exposure a second time and the cards would read too dark.
/// The merge therefore delegates to the frame's precomputed BoxBlur(25)
/// local mean — the exact quantity B's grid approximated, same formula
/// and same constants, and the same frame the panel fill uses. Kept as
/// a named function so the callers and tests below are untouched.
fn frost_card_local_mean(
    lb: &crate::wallpaper_blur::LiveBackdrop,
    sx: f32,
    sy: f32,
) -> [f32; 3] {
    let m = lb.local_mean(sx, sy);
    [m[0] as f32, m[1] as f32, m[2] as f32]
}

/// iter103 (Builder B) — one cards/band frost pixel (RGB, [0] = R):
/// out = n*(1-fa) + floor*fa + k*clamp(n - local_mean, +/-HP_MAX_DEV),
/// f32-truncated like frost_fill_px_blur (same convention, so the
/// card/panel step is exactly (panel_floor - card_floor)*fa = -0.46
/// levels at zero deviation, iter128 — the floors nearly coincide).
/// `neutral` is the neutralised live sample at the pixel,
/// `local_mean` its frost_card_local_mean. Clamped to [0, 255].
/// iter128: the deviation is capped at FROST_HP_MAX_DEV (12) — the
/// uncapped card printed a white sample over a black mean as 231.
fn frost_card_px(neutral: [u8; 3], local_mean: [f32; 3]) -> [u8; 3] {
    let mut out = [0u8; 3];
    for c in 0..3 {
        let n = neutral[c] as f32;
        let dev = (n - local_mean[c]).clamp(
            -crate::wallpaper_blur::FROST_HP_MAX_DEV,
            crate::wallpaper_blur::FROST_HP_MAX_DEV,
        );
        let v = n * (1.0 - FROST_CARD_FA) + FROST_CARD_FLOOR * FROST_CARD_FA
            + FROST_CARD_K * dev;
        // iter113: clamp at the card floor (3.0*0.92 = 2.76, iter128;
        // was 3.0*0.80 = 2.4), not 0 — same fix as the panel fill:
        // no card pixel reads below its dark-wallpaper value.
        out[c] = v.clamp(FROST_CARD_FLOOR * FROST_CARD_FA, 255.0) as u8;
    }
    out
}

/// iter103 (Builder B, MERGED) — fill a rounded rect with the cards/band
/// frost: per-pixel neutralised live sample + high-pass deviation, own
/// darker floor. Same SDF/coverage loop as fill_rounded_rect (same
/// geometry, same edge AA, same source-over blend); only the per-pixel
/// colour differs. `bg_origin` maps canvas coords to screen-logical
/// coords. No live frame => the caller keeps the flat colour.
/// MERGE: `lb.sample()` is already the NEUTRALISED sample (A's
/// finish_live_frame) — it is used directly, NOT neutralised again.
fn fill_rounded_rect_frost(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    rect: Rect,
    radius: f32,
    lb: &crate::wallpaper_blur::LiveBackdrop,
    bg_origin: (f32, f32),
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
            let sx = bg_origin.0 + x as f32 + 0.5;
            let sy = bg_origin.1 + y as f32 + 0.5;
            let neutral = lb.sample(sx, sy);
            let mean = frost_card_local_mean(lb, sx, sy);
            let rgb = frost_card_px(neutral, mean);
            blend_pixel(canvas, w, x, y, (rgb[0], rgb[1], rgb[2]), a);
        }
    }
}

thread_local! {
    /// r1f (2x): logical -> device scale for the CURRENT draw, set by the
    /// public entry point. Thread-local because the tests draw these
    /// primitives at arbitrary canvas sizes in parallel: it stays 1.0
    /// outside a draw, so w == CC_W is byte-identical to the 1x path.
    static SCALE: std::cell::Cell<f32> = const { std::cell::Cell::new(1.0) };
}
fn scale_of(_w: u32) -> f32 {
    SCALE.with(|s| s.get())
}
/// Begin a draw: set the logical -> device scale (see `SCALE`).
fn set_scale(w: u32) {
    SCALE.with(|c| c.set(w as f32 / CC_W as f32));
}

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

// ---------------------------------------------------------------------------
// Text — fontdue, same pattern as main.rs (private helpers duplicated here
// because main.rs is not edited to share them).
// ---------------------------------------------------------------------------
struct TextRow {
    glyphs: Vec<(fontdue::Metrics, std::sync::Arc<[u8]>)>,
    width: f32,
    ascent: f32,
    height: f32,
}

fn layout_text(font: &Font, text: &str, px: f32) -> TextRow {
    let mut glyphs = Vec::new();
    let mut width = 0.0f32;
    for ch in text.chars() {
        let (m, cov) = crate::lock_timer::rasterize_cached(font, ch, px);
        width += m.advance_width;
        glyphs.push((m, cov));
    }
    // Degrade to the em box instead of panicking (r1c finding 9).
    let (ascent, height) = font
        .horizontal_line_metrics(px)
        .map(|lm| (lm.ascent, lm.ascent - lm.descent))
        .unwrap_or((px, px));
    TextRow { glyphs, width, ascent, height }
}

fn text_width(font: &Font, text: &str, px: f32) -> f32 {
    text.chars().map(|ch| crate::lock_timer::rasterize_cached(font, ch, px).0.advance_width).sum()
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

/// Draw one Material Symbols glyph centred on (cx, cy) at `px` size.
/// Falls back to a hollow ring of the same size while the PUA codepoint
/// table in `icon_char()` is still empty (see header).
fn draw_icon(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    icon_font: Option<&Font>,
    name: &str,
    px: f32,
    cx: f32,
    cy: f32,
    rgb: (u8, u8, u8),
) {
    // Scale size AND position, exactly as draw_text does. Without this the
    // glyph was rasterised at logical `px` and placed at logical (cx, cy) on a
    // device-sized canvas: at 2x it landed at a quarter of the disc's position
    // and half its size, so Wi-Fi/Bluetooth discs rendered as flat colour with
    // no icon at all (r2 accessibility critic measured 0 glyph pixels).
    let s = scale_of(w);
    let (cx, cy) = (cx * s, cy * s);
    if let (Some(font), Some(ch)) = (icon_font, icon_char(name)) {
        let (m, cov) = crate::lock_timer::rasterize_cached(font, ch, px * s);
        // Glyph top row, same formula as main.rs:
        //   top = baseline - ymin - height, centre = baseline - ymin - height/2
        // so baseline = cy + ymin + height/2, and top simplifies to cy - height/2.
        let gx0 = (cx - m.advance_width / 2.0 + m.xmin as f32).round() as i32;
        let gy0 = (cy - m.height as f32 / 2.0).round() as i32;
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
    } else {
        // Placeholder: hollow ring, same footprint as the glyph would have.
        stroke_circle(canvas, w, h, cx, cy, (px / 2.0 - 1.0) * s, 1.5 * s, rgb);
    }
}

// ---------------------------------------------------------------------------
// Procedural icons (no font needed)
// ---------------------------------------------------------------------------
/// Focus target glyph (critic defect 7): outer ring + centre dot.
fn draw_focus_target(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    cx: f32,
    cy: f32,
    rgb: (u8, u8, u8),
) {
    // iter34 a6a: ring r 9 -> 12, stroke 2 -> 2.5, dot 5 -> 7: the old
    // target read 20 logical (0.42 of the disc) while bluetooth read
    // 10.7 (0.22); all four glyphs now land at ~25 logical (0.52).
    // iter86: ring r 12.0 -> 11.5 — iter85 measured the Focus glyph
    // at 0.571 of the disc, a hair above the 0.50-0.55 band; outer
    // diameter is now 25.5/48 = 0.531 (measured ~0.547 with AA).
    stroke_circle(canvas, w, h, cx, cy, 11.5, 2.5, rgb);
    fill_rounded_rect(
        canvas,
        w,
        h,
        Rect::new(cx - 3.5, cy - 3.5, 7.0, 7.0),
        3.5,
        rgb,
    );
}

/// Mirror of `draw_chevron_right` — vertex on the LEFT, for "back".
fn draw_chevron_left(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    cx: f32,
    cy: f32,
    size: f32,
    rgb: (u8, u8, u8),
) {
    let s = scale_of(w);
    let (cx, cy, size) = (cx * s, cy * s, size * s);
    let half = size / 2.0;
    let left = cx - half * 0.7;
    let right = cx + half * 0.7;
    let x0 = (left - 3.0).floor().max(0.0) as i32;
    let y0 = (cy - half - 3.0).floor().max(0.0) as i32;
    let x1 = (right + 3.0).ceil().min(w as f32) as i32;
    let y1 = (cy + half + 3.0).ceil().min(h as f32) as i32;
    for y in y0..y1 {
        for x in x0..x1 {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let d = seg_dist(px, py, left, cy, right, cy - half)
                .min(seg_dist(px, py, left, cy, right, cy + half));
            let a = (1.6 - d).clamp(0.0, 1.0);
            if a > 0.0 {
                blend_pixel(canvas, w, x, y, rgb, a);
            }
        }
    }
}

/// Trim to `max_w` logical px with an ellipsis, so a long device name
/// can never bleed into the row's chevron.
fn fit_text(font: &Font, text: &str, px: f32, max_w: f32) -> String {
    if text_width(font, text, px) <= max_w {
        return text.to_string();
    }
    let mut s = text.to_string();
    while !s.is_empty() && text_width(font, &s, px) > max_w {
        s.pop();
    }
    s.push('…');
    s
}

/// Chevron-right for the slider end buttons (video frame: the end button
/// holds a ">" chevron, NOT a tiny duplicate of the fill icon; iter86:
/// drawn BARE — no pill/disc behind it). Two
/// 2.2 px strokes meeting at the right, `size` tall, centred on (cx, cy).
/// Same stroke language as the nav-disc chevrons in main.rs.
fn draw_chevron_right(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    cx: f32,
    cy: f32,
    size: f32,
    rgb: (u8, u8, u8),
) {
    let s = scale_of(w);
    let (cx, cy, size) = (cx * s, cy * s, size * s);
    let half = size / 2.0;
    let ax = cx - half * 0.7;
    let bx = cx + half * 0.7;
    let x0 = (ax - 3.0).floor().max(0.0) as i32;
    let y0 = (cy - half - 3.0).floor().max(0.0) as i32;
    let x1 = (bx + 3.0).ceil().min(w as f32) as i32;
    let y1 = (cy + half + 3.0).ceil().min(h as f32) as i32;
    for y in y0..y1 {
        for x in x0..x1 {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let d = seg_dist(px, py, ax, cy - half, bx, cy)
                .min(seg_dist(px, py, bx, cy, ax, cy + half));
            let a = (1.6 - d).clamp(0.0, 1.0);
            if a <= 0.0 {
                continue;
            }
            blend_pixel(canvas, w, x, y, rgb, a);
        }
    }
}

/// Distance from (px, py) to the segment (x1,y1)-(x2,y2).
fn seg_dist(px: f32, py: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len2 = dx * dx + dy * dy;
    let t = if len2 <= 0.0 {
        0.0
    } else {
        ((px - x1) * dx + (py - y1) * dy) / len2
    }
    .clamp(0.0, 1.0);
    let qx = x1 + t * dx - px;
    let qy = y1 + t * dy - py;
    (qx * qx + qy * qy).sqrt()
}

/// Battery icon: rounded outline + proportional fill + nub on the right.
/// `pct` 0..=100. Drawn in `rgb` (white in the header row).
fn draw_battery(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    x: f32,
    y: f32,
    pct: u8,
    rgb: (u8, u8, u8),
) {
    // Body 20x11, radius 2.5, 1.5 px stroke; nub 2x5 centred on the right.
    let body = Rect::new(x, y, 20.0, 11.0);
    stroke_rounded_rect(canvas, w, h, body, 2.5, 1.5, rgb);
    fill_rounded_rect(
        canvas,
        w,
        h,
        Rect::new(x + 22.0, y + 3.0, 2.0, 5.0),
        1.0,
        rgb,
    );
    let frac = (pct as f32 / 100.0).clamp(0.0, 1.0);
    if frac > 0.0 {
        let inner_w = (20.0 - 2.0 * 2.5) * frac; // inside the stroke
        fill_rounded_rect(
            canvas,
            w,
            h,
            Rect::new(x + 2.5, y + 2.5, inner_w.max(2.0), 11.0 - 2.0 * 2.5),
            1.5,
            rgb,
        );
    }
}

// ---------------------------------------------------------------------------
// Panel
// ---------------------------------------------------------------------------
/// Draw the full control-centre panel into `canvas` (438x447, Argb8888).
/// `ui_font` is Quicksand; `icon_font` is Material Symbols Rounded
/// (None => icon placeholders, see `draw_icon`).
/// The caller owns the panel background: this function fills the whole
/// canvas with the black capsule first, exactly like `draw()` in main.rs.
/// Bluetooth submenu: top power toggle, then a compact device list.
/// Drawn into the same panel canvas; the header above is unchanged.
fn draw_bt_submenu(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    ui_font: &Font,
    icon_font: Option<&Font>,
    data: &ControlCenterData,
    l: &CcLayout,
) {
    draw_chevron_left(
        canvas, w, h, l.bt_back.cx(), l.bt_back.cy(), 12.0, C_PRIMARY,
    );
    draw_text_centered(
        canvas, w, h, ui_font, "Bluetooth", 14.0, CC_W as f32 / 2.0, 41.0, C_PRIMARY, false,
    );

    let on = data.bluetooth_on;
    // No readable adapter: the accent fill would claim "On". Dim the
    // pill and say so instead. shortcut: the switch stays clickable
    // (a click sends `bluetoothctl power on`, which simply fails when
    // there is no adapter); disabling hit-testing would need the input
    // path to know this flag too.
    let available = data.bluetooth_available;
    fill_rounded_rect(canvas, w, h, l.bt_power_pill, PILL_R, C_PILL_BG);
    let circle = l.bt_power_circle;
    fill_rounded_rect(
        canvas, w, h, circle, circle.w / 2.0,
        if on && available { accent() } else { C_TOGGLE_OFF },
    );
    draw_icon(
        canvas, w, h, icon_font, "bluetooth", 26.0, circle.cx(), circle.cy(),
        if on && available { accent_glyph() } else { C_TOGGLE_OFF_GLYPH },
    );
    let text_x = circle.x + circle.w + 12.0;
    draw_text(
        canvas, w, h, ui_font, "Bluetooth", 13.0, text_x, l.bt_power_pill.y + 10.0,
        C_PRIMARY, false,
    );
    draw_text(
        canvas, w, h, ui_font,
        if !available { "No adapter" } else if on { "On" } else { "Off" },
        10.0, text_x, l.bt_power_pill.y + 29.0, C_SECONDARY, false,
    );
    let sw = l.bt_switch;
    fill_rounded_rect(
        canvas, w, h, sw, 11.0, if on && available { accent() } else { C_SLIDER_TRACK },
    );
    let knob_x = if on && available { sw.x + sw.w - 19.0 } else { sw.x + 3.0 };
    fill_rounded_rect(
        canvas, w, h, Rect::new(knob_x, sw.y + 3.0, 16.0, 16.0), 8.0, C_PRIMARY,
    );

    let count = data.bt_devices.len();
    let label = if count > BT_ROWS {
        format!("Devices · +{} more", count - BT_ROWS)
    } else if count > 0 {
        format!("Devices · {count} nearby")
    } else {
        "Devices".to_string()
    };
    draw_text(canvas, w, h, ui_font, &label, 11.0, CC_PAD, 143.0, C_SECONDARY, false);
    // Size the card to the rows actually present — no empty slab
    // under a short device list.
    let shown = count.min(BT_ROWS);
    let card_h = if shown == 0 {
        120.0
    } else {
        (9.0 + shown as f32 * BT_ROW_H + shown.saturating_sub(1) as f32 * BT_ROW_GAP + 4.0)
            .min(l.bt_devices_card.h)
    };
    let card = Rect::new(
        l.bt_devices_card.x,
        l.bt_devices_card.y,
        l.bt_devices_card.w,
        card_h,
    );
    fill_rounded_rect(canvas, w, h, card, 14.0, C_NOTIF_CARD);

    for (i, dev) in data.bt_devices.iter().take(BT_ROWS).enumerate() {
        let row = l.bt_device_rows[i];
        fill_rounded_rect(canvas, w, h, row, 10.0, C_PILL_BG);
        let known = dev.connected || dev.paired;
        let icon_bg = if known { accent() } else { C_TOGGLE_OFF };
        let icon_rect = Rect::new(row.x + 7.0, row.y + 10.0, 28.0, 28.0);
        fill_rounded_rect(canvas, w, h, icon_rect, 14.0, icon_bg);
        draw_icon(
            canvas, w, h, icon_font, "bluetooth", 16.0, icon_rect.cx(), icon_rect.cy(),
            if known { accent_glyph() } else { C_TOGGLE_OFF_GLYPH },
        );
        let name_x = icon_rect.x + icon_rect.w + 9.0;
        let name = fit_text(ui_font, &dev.name, 12.0, row.w - 112.0);
        draw_text(canvas, w, h, ui_font, &name, 12.0, name_x, row.y + 6.0, C_PRIMARY, false);
        let status = if dev.connected {
            "Connected"
        } else if dev.paired {
            "Paired · tap to connect"
        } else {
            "Tap to pair"
        };
        draw_text(
            canvas, w, h, ui_font, status, 10.0, name_x, row.y + 26.0, C_SECONDARY, false,
        );
        draw_chevron_right(
            canvas, w, h, row.x + row.w - 17.0, row.cy(), 12.0, C_END_BTN_CHEVRON,
        );
    }

    if count == 0 {
        let title = if cc_bt_scanning() {
            "Searching nearby…"
        } else {
            "No devices found"
        };
        draw_text_centered(canvas, w, h, ui_font, title, 12.0, CC_W as f32 / 2.0, 195.0, C_PRIMARY, false);
        draw_text_centered(
            canvas, w, h, ui_font, "Make your device discoverable", 10.0,
            CC_W as f32 / 2.0, 216.0, C_SECONDARY, false,
        );
    }
}

/// Pick a recognisable glyph for an output without poking the sound
/// server: Bluetooth/headphone sinks get headphones, everything else
/// gets a speaker.
fn audio_route_icon(route: &AudioRoute) -> &'static str {
    let s = format!("{} {}", route.name, route.desc).to_ascii_lowercase();
    if s.contains("bluez") || s.contains("headphone") || s.contains("wf-") {
        "headphones"
    } else {
        // E32D ("speaker") renders as a box in this font build;
        // the verified volume_up glyph is the legible speaker.
        "volume_up"
    }
}

/// Sound submenu: a compact list of outputs. Selecting a row reroutes
/// all existing streams, not just the default for new ones.
fn draw_sound_submenu(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    ui_font: &Font,
    icon_font: Option<&Font>,
    data: &ControlCenterData,
    l: &CcLayout,
) {
    draw_chevron_left(
        canvas, w, h, l.bt_back.cx(), l.bt_back.cy(), 12.0, C_PRIMARY,
    );
    draw_text_centered(
        canvas, w, h, ui_font, "Sound", 14.0, CC_W as f32 / 2.0, 41.0, C_PRIMARY, false,
    );

    let count = data.audio_routes.len();
    let label = if count > BT_ROWS {
        format!("Outputs · showing {BT_ROWS} of {count}")
    } else {
        "Outputs".to_string()
    };
    draw_text(canvas, w, h, ui_font, &label, 11.0, CC_PAD, 62.0, C_SECONDARY, false);

    let shown = count.min(BT_ROWS);
    let card_h = if shown == 0 {
        96.0
    } else {
        (9.0 + shown as f32 * BT_ROW_H + shown.saturating_sub(1) as f32 * BT_ROW_GAP + 4.0)
            .min(l.sound_card.h)
    };
    let card = Rect::new(
        l.sound_card.x, l.sound_card.y, l.sound_card.w, card_h,
    );
    fill_rounded_rect(canvas, w, h, card, 14.0, C_NOTIF_CARD);

    for (i, route) in data.audio_routes.iter().take(BT_ROWS).enumerate() {
        let row = l.sound_rows[i];
        let active = route.name == data.audio_default;
        fill_rounded_rect(canvas, w, h, row, 10.0, C_PILL_BG);
        let icon_rect = Rect::new(row.x + 7.0, row.y + 10.0, 28.0, 28.0);
        fill_rounded_rect(
            canvas, w, h, icon_rect, 14.0,
            if active { accent() } else { C_TOGGLE_OFF },
        );
        draw_icon(
            canvas, w, h, icon_font, audio_route_icon(route), 16.0,
            icon_rect.cx(), icon_rect.cy(),
            if active { accent_glyph() } else { C_TOGGLE_OFF_GLYPH },
        );
        let name_x = icon_rect.x + icon_rect.w + 9.0;
        let name = fit_text(ui_font, &route.desc, 12.0, row.w - 112.0);
        draw_text(canvas, w, h, ui_font, &name, 12.0, name_x, row.y + 6.0, C_PRIMARY, false);
        draw_text(
            canvas, w, h, ui_font,
            if active { "Current output" } else { "Tap to switch" },
            10.0, name_x, row.y + 26.0, C_SECONDARY, false,
        );
        if active {
            fill_rounded_rect(
                canvas, w, h,
                Rect::new(row.x + row.w - 25.0, row.cy() - 5.0, 10.0, 10.0),
                5.0, accent(),
            );
        } else {
            draw_chevron_right(
                canvas, w, h, row.x + row.w - 17.0, row.cy(), 12.0, C_END_BTN_CHEVRON,
            );
        }
    }

    if count == 0 {
        draw_text_centered(
            canvas, w, h, ui_font, "No audio outputs found", 12.0,
            CC_W as f32 / 2.0, 112.0, C_PRIMARY, false,
        );
    }
}

pub fn draw_controlcenter_panel(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    ui_font: &Font,
    icon_font: Option<&Font>,
    data: &ControlCenterData,
    _bg_origin: (f32, f32),
) {
    set_scale(w);
    debug_assert!(
        (h as f32 - scale_of(w) * CC_H as f32).abs() < 1.5,
        "controlcenter canvas {w}x{h} is not a scaled {CC_W}x{CC_H}"
    );
    let l = cc_layout();

    // Opaque capsule. The old live-backdrop frost sampled a blurred
    // frame for every one of the 438x447 pixels on every draw and let
    // window text bleed through the menu. Flat black is both cleaner
    // and a straight fill — no backdrop load, no per-pixel blur lookups.
    for y in 0..h {
        for x in 0..w {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let d = rounded_rect_sdf(px, py, w as f32, h as f32, CC_RADIUS * scale_of(w));
            let a = (0.5 - d).clamp(0.0, 1.0);
            let i = ((y * w + x) * 4) as usize;
            canvas[i] = 0;
            canvas[i + 1] = 0;
            canvas[i + 2] = 0;
            canvas[i + 3] = (a * 255.0).round() as u8;
        }
    }

    // --- Header: live time left (14 px, regular — video is small/light,
    // critic defect 5: QS rendered ~20 px bold), live battery right.
    // `data.time_hhmm` / `data.battery_pct` are live when the caller
    // builds the data with `ControlCenterData::live()` (iter9); the
    // frozen s07 values live only in `video_defaults()`. ---
    draw_text(
        canvas, w, h, ui_font, &data.time_hhmm, 14.0, l.header.x, l.header.y,
        C_PRIMARY, false,
    );
    if let Some(pct) = data.battery_pct {
        // Battery cluster, right-aligned to content edge (x = 424):
        // icon 20x11 + 5 gap + "81%" 12 px. Total width ~20+5+~22 = ~47.
        let pct_text = format!("{pct}%");
        let pct_w = text_width(ui_font, &pct_text, 12.0);
        let pct_x = l.header.x + l.header.w - pct_w;
        let icon_x = pct_x - 5.0 - 24.0; // 20 body + 2 nub + 2 right pad
        draw_battery(canvas, w, h, icon_x, l.header.y + 5.5, pct, C_PRIMARY);
        draw_text(
            canvas, w, h, ui_font, &pct_text, 12.0, pct_x, l.header.y + 4.0,
            C_PRIMARY, false,
        );
        // iter90: power-source text, right-aligned to end 8 px left
        // of the battery icon (same 12 px as the % text). Room check:
        // the time ends ~x50 and the icon starts ~x374, so even the
        // longest text ("Plugged in · 999 W", ~115 px) clears both.
        if let Some(src) = &data.power_source_text {
            let src_w = text_width(ui_font, src, 12.0);
            draw_text(
                canvas, w, h, ui_font, src, 12.0, icon_x - 8.0 - src_w,
                l.header.y + 4.0, C_PRIMARY, false,
            );
        }
    }

    // --- Header divider (iter91): 1 px hairline under the header row,
    // full content width — groups clock + power/battery into one
    // header band (iter90 beauty: "No divider, no grouping"). ---
    fill_rounded_rect(canvas, w, h, l.header_divider, 0.5, C_HEADER_DIVIDER);

    // --- Bluetooth submenu replaces the whole body but keeps the
    // --- same header (and the same 438x447 panel geometry). ---
    if cc_bt_submenu_open() {
        draw_bt_submenu(canvas, w, h, ui_font, icon_font, data, &l);
        return;
    }
    if cc_sound_submenu_open() {
        draw_sound_submenu(canvas, w, h, ui_font, icon_font, data, &l);
        return;
    }

    // --- Toggle grid: 2x2 pills. iter103 (Builder B): the pill fill
    // --- is the same frost as the panel (neutralised live sample +
    // --- high-pass deviation, own darker floor) whenever a live
    // --- frame exists; the flat C_PILL_BG survives only as the
    // --- no-live-frame fallback. Circles, glyphs, labels unchanged.
    // --- MERGE: the live frame is already neutralised at capture
    // --- (Builder A), so the frost fn reads lb.sample() directly.
    for (i, def) in TOGGLES.iter().enumerate() {
        let pill = l.toggle_pills[i];
        let circle = l.toggle_circles[i];
        let on = data.toggle(def.key);
        fill_rounded_rect(canvas, w, h, pill, PILL_R, C_PILL_BG);
        let (circle_bg, glyph_rgb) = if on {
            (accent(), accent_glyph()) // live theme accent + adaptive glyph
        } else {
            (C_TOGGLE_OFF, C_TOGGLE_OFF_GLYPH)
        };
        fill_rounded_rect(canvas, w, h, circle, CIRCLE_D / 2.0, circle_bg);
        if def.key == ToggleKey::Focus {
            // Procedural target glyph — no font codepoint needed.
            draw_focus_target(canvas, w, h, circle.cx(), circle.cy(), glyph_rgb);
        } else {
            draw_icon(
                canvas, w, h, icon_font, def.icon, def.glyph_px, circle.cx(), circle.cy(),
                glyph_rgb,
            );
        }
        // Name (12 px bold) + subtext (10 px, brighter secondary —
        // iter85: the old 9 px #9AA0A6 failed legibility), left-aligned
        // at pill_x + 67, vertically centred as a two-line block.
        let tx = pill.x + TEXT_DX;
        draw_text(canvas, w, h, ui_font, def.name, 12.0, tx, pill.y + 15.0, C_PRIMARY, false);
        let sub = data.subtext(def);
        draw_text(canvas, w, h, ui_font, &sub, 10.0, tx, pill.y + 27.0, C_SECONDARY, false);
    }

    // --- Sliders: thick pill bars (h 36, iter85 — was 20, no knob),
    // --- each with a white Ø28 thumb that stays visible at 0%. ---
    draw_slider(
        canvas, w, h, ui_font, icon_font, "Display", "brightness_6",
        l.display_label, l.display_track, l.display_end_btn, data.display_level,
    );
    draw_slider(
        canvas, w, h, ui_font, icon_font, "Sound", "volume_up",
        l.sound_label, l.sound_track, l.sound_end_btn, data.sound_level,
    );

    // --- Notifications: empty state (bell + "All caught up" +
    // "Nothing new since HH:MM") when the swaync count is 0 or unknown
    // (iter13: `live_notification_count()`); when notifications are
    // pending, say so truthfully instead of claiming "All caught up".
    // The per-notification LIST rows have no video spec yet — that
    // design is still open, do not invent it here.
    // iter85 (R3): the inset CARD, sized to its content with 12 px
    // padding (308..407) and a 14 px panel bottom margin below it —
    // no dead band anywhere in the panel.
    // iter91 (R3): 8 px padding (296..387, 91 tall), card colour
    // (13,12,15) — the video's subtle inset, not a grey slab. ---
    // iter103 (Builder B): same frost as the toggle pills (see the
    // grid comment above); flat C_NOTIF_CARD is the
    // no-live-frame fallback. Bell circle + texts unchanged.
    fill_rounded_rect(canvas, w, h, l.notif_card, NOTIF_CARD_R, C_NOTIF_CARD);
    fill_rounded_rect(canvas, w, h, l.bell_circle, BELL_D / 2.0, C_BELL_BG);
    draw_icon(
        canvas, w, h, icon_font, "notifications_none", 16.0,
        l.bell_circle.cx(), l.bell_circle.cy(), C_SECONDARY,
    );
    match data.notification_count {
        Some(n) if n > 0 => {
            let title = if n == 1 {
                "1 notification".to_string()
            } else {
                format!("{n} notifications")
            };
            draw_text_centered(
                canvas, w, h, ui_font, &title, 13.0, CC_W as f32 / 2.0,
                l.notif_title.y, C_PRIMARY, true,
            );
            let since = format!("{n} unread since {}", data.notifications_since_hhmm);
            draw_text_centered(
                canvas, w, h, ui_font, &since, 10.0, CC_W as f32 / 2.0,
                l.notif_sub.y, C_SECONDARY, false,
            );
        }
        _ => {
            draw_text_centered(
                canvas, w, h, ui_font, "All caught up", 13.0, CC_W as f32 / 2.0,
                l.notif_title.y, C_PRIMARY, true,
            );
            let since = format!("Nothing new since {}", data.notifications_since_hhmm);
            draw_text_centered(
                canvas, w, h, ui_font, &since, 10.0, CC_W as f32 / 2.0,
                l.notif_sub.y, C_SECONDARY, false,
            );
        }
    }

    // --- Power levels: three pills, the active one in the theme accent. ---
    let level = live_power_level() as usize;
    for (i, btn) in l.power_buttons.iter().enumerate() {
        let (bg, fg) = if i == level {
            (accent(), C_TOGGLE_ON_GLYPH)
        } else {
            (C_PILL_BG, C_SECONDARY)
        };
        fill_rounded_rect(canvas, w, h, *btn, POWER_R, bg);
        draw_text_centered(
            canvas, w, h, ui_font, POWER_LEVELS[i].0, 11.0,
            btn.cx(), btn.y + 10.0, fg, false,
        );
    }
}

/// Thumb centre x for a slider level: the fill end, spanning the FULL
/// track (iter107 — the rendered value IS the live value, at every
/// level including the extremes).
/// The old form clamped the centre to
/// [track.x + THUMB_D/2, track.x + track.w - THUMB_D/2], so at 0% the
/// centre read 4.0% of the track and at 100% 96.0% — the rendered
/// value could never match the live wpctl value at the extremes, and
/// the thumb sat visibly in from the track ends. The clamp is gone:
/// centre = track.x + track.w * level, exactly the fill end, exactly
/// where a press/drag at that x sets the level (main.rs maps press x
/// with the same formula, so input and output are exact inverses).
/// The thumb still never leaves the PANEL — THUMB_D is 28 (see its
/// comment): [0, 28] at 0%, [376, 404] at 100%. Pure geometry,
/// unit-tested below.
fn thumb_cx(track: Rect, level: f32) -> f32 {
    let level = level.clamp(0.0, 1.0);
    track.x + track.w * level
}

/// One slider row: 11 px secondary label, 36 px pill track, periwinkle
/// fill, WHITE Ø28 thumb with a thin dark ring (iter85 — the old
/// knob-less 20 px bar rendered 0% as a dead grey bar; a 0% slider now
/// still shows its track and its thumb clearly), dark-navy glyph at
/// the left INSIDE the fill when the fill is wide enough, otherwise
/// dimmed on the track just right of the thumb (never under it), and a
/// BARE white chevron at the row end — a row action, clearly distinct
/// from the filled nav discs (critic defect 9 / iter85 item 5; iter86:
/// the outlined pill was dropped because it cloned the nav discs).
/// `level` is clamped to 0.0..=1.0.
fn draw_slider(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    ui_font: &Font,
    icon_font: Option<&Font>,
    label: &str,
    icon: &str,
    label_rect: Rect,
    track: Rect,
    end_btn: Rect,
    level: f32,
) {
    let level = level.clamp(0.0, 1.0);
    draw_text(
        canvas, w, h, ui_font, label, 11.0, label_rect.x, label_rect.y,
        C_SECONDARY, false,
    );
    fill_rounded_rect(canvas, w, h, track, TRACK_R, C_SLIDER_TRACK);
    let fill_w = track.w * level;
    if fill_w > 0.0 {
        // iter107: the fill's leading edge IS the value edge, so it is
        // SQUARE (rounded left end only). The old fully-rounded fill
        // hid its true end twice over: at the centre row the solid
        // thumb covers the fill's last THUMB_D/2 px, and at every
        // off-centre row the r=18 corner pulled the blue edge left by
        // up to 18 px — on the v0.41 shot the visible blue never got
        // past ~46% while wpctl read 49% (45.7% at the handle centre
        // against the critic's span, 44.9% blue at the centre row
        // against the true track). With a square end the fill reaches
        // exactly track.x + fill_w on the rows above/below the thumb
        // (|dy| in (THUMB_D/2, TRACK_H/2] — 4 logical px top and
        // bottom), so a pixel measurement there reads the live value.
        // Two parts: the rounded left cap (the track's left end is a
        // capsule — a square fill would poke out of it at the corners)
        // plus a sharp rect (radius 0) from the cap's centre to the
        // fill end. fill_w <= 2*TRACK_R (level < ~9.6%) is a single
        // small pill — no checklist value lives there, and the cap
        // alone is exact at the centreline.
        if fill_w <= 2.0 * TRACK_R {
            fill_rounded_rect(
                canvas, w, h,
                Rect::new(track.x, track.y, fill_w, track.h),
                (fill_w / 2.0).min(TRACK_R), slider_accent(),
            );
        } else {
            fill_rounded_rect(
                canvas, w, h,
                Rect::new(track.x, track.y, 2.0 * TRACK_R, track.h),
                TRACK_R, slider_accent(),
            );
            fill_rounded_rect(
                canvas, w, h,
                Rect::new(track.x + TRACK_R, track.y, fill_w - TRACK_R, track.h),
                0.0, slider_accent(),
            );
        }
    }
    // Thumb: white disc + thin dark ring, on top of fill and track.
    // Drawn even at 0% (fill_w == 0) — that is the whole point: the
    // 0% state keeps a visible handle instead of a dead bar.
    let tcx = thumb_cx(track, level);
    fill_rounded_rect(
        canvas, w, h,
        Rect::new(tcx - THUMB_D / 2.0, track.cy() - THUMB_D / 2.0, THUMB_D, THUMB_D),
        THUMB_D / 2.0,
        C_THUMB,
    );
    stroke_circle(canvas, w, h, tcx, track.cy(), THUMB_D / 2.0 - 0.75, 1.5, C_THUMB_RING);
    // Glyph: inside the fill (dark navy) when the fill is wide enough
    // to hold it clear of the thumb (fill_w >= 48: glyph ink ends
    // ~track.x+26, thumb starts at fill end - 14 >= track.x+34);
    // otherwise dimmed on the track just right of the thumb, so a
    // near-zero level never collides with it or overflows the fill.
    // iter34 a6a: 20 px em (was 16) so the fill glyph matches the
    // toggle glyphs' visual weight instead of reading tiny.
    if fill_w >= 48.0 {
        draw_icon(
            canvas, w, h, icon_font, icon, SLIDER_GLYPH_PX,
            track.x + 16.0, track.cy(), accent_glyph(),
        );
    } else {
        draw_icon(
            canvas, w, h, icon_font, icon, SLIDER_GLYPH_PX,
            tcx + THUMB_D / 2.0 + 10.0, track.cy(), C_SECONDARY,
        );
    }
    // End button: BARE chevron, no pill, no disc, no outline
    // (iter86: the 28x36 outlined pill read as a nav-disc clone —
    // same 28 width class as the Ø28 nav discs, same ">" — so the
    // row action is now just the glyph itself, floating on the panel).
    // iter91: DIMMED (140,146,152) + shrunk 18 -> 14 px — the bare
    // WHITE 18 px chevron still read as a stray nav arrow (same
    // white, same stroke language as the nav-disc chevrons). Dimmer
    // and smaller it reads as a quiet row affordance, not navigation.
    // The hit rect (end_btn, 28x36) is unchanged — same click target.
    draw_chevron_right(canvas, w, h, end_btn.cx(), end_btn.cy(), 14.0, C_END_BTN_CHEVRON);
}

// ---------------------------------------------------------------------------
// Tests (iter85) — pure geometry + cache contract. No font, no Surface,
// no process spawns: `cc_layout()` and `thumb_cx()` are pure fns.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod iter85_tests {
    use super::*;

    #[test]
    fn panel_sized_to_content_no_dead_band() {
        let l = cc_layout();
        // Panel bottom margin below the power row is exactly 14.
        assert!((l.panel.h - (l.power_buttons[0].y + l.power_buttons[0].h) - 14.0).abs() < 1e-6);
        // 14 px from the card to the power row — no dead band at the tail.
        assert!((l.power_buttons[0].y - (l.notif_card.y + l.notif_card.h) - 14.0).abs() < 1e-6);
        // No inter-section gap exceeds 14 (old layout: 75 above the
        // card, 56 below it).
        let grid_bottom = l.toggle_pills[2].y + l.toggle_pills[2].h;
        assert!((l.display_label.y - grid_bottom - 12.0).abs() < 1e-6);
        assert!((l.display_track.y - (l.display_label.y + 14.0) - 4.0).abs() < 1e-6);
        assert!((l.sound_label.y - (l.display_track.y + l.display_track.h) - 14.0).abs() < 1e-6);
        assert!((l.sound_track.y - (l.sound_label.y + 14.0) - 4.0).abs() < 1e-6);
        assert!((l.notif_card.y - (l.sound_track.y + l.sound_track.h) - 14.0).abs() < 1e-6);
    }

    #[test]
    fn bt_submenu_layout_fits_the_panel() {
        let l = cc_layout();
        let inside = |r: Rect| {
            r.x >= CC_PAD
                && r.y >= 32.0
                && r.x + r.w <= CC_W as f32 - CC_PAD
                && r.y + r.h <= CC_H as f32
        };
        assert!(inside(l.bt_back));
        assert!(inside(l.bt_power_pill));
        assert!(inside(l.bt_devices_card));
        for row in l.bt_device_rows {
            assert!(inside(row));
            assert!(row.y >= l.bt_devices_card.y);
            assert!(row.y + row.h <= l.bt_devices_card.y + l.bt_devices_card.h);
        }
    }

    #[test]
    fn bt_device_rows_parse_paired_and_connected() {
        let all = "Device AA:BB Headphones\nDevice CC:DD Speaker\n";
        let paired = "Device AA:BB Headphones\n";
        let connected = "Device AA:BB Headphones\n";
        let rows = parse_bt_devices(all, paired, connected);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], BtDevice {
            addr: "AA:BB".into(),
            name: "Headphones".into(),
            paired: true,
            connected: true,
        });
        assert_eq!(rows[1].name, "Speaker");
        assert!(!rows[1].paired);
        assert!(!rows[1].connected);
        // Nameless devices still get a usable label.
        let rows = parse_bt_devices("Device EE:FF\n", "", "");
        assert_eq!(rows[0].name, "EE:FF");
    }

    #[test]
    fn sound_submenu_layout_fits_the_panel() {
        let l = cc_layout();
        assert!(l.sound_card.y >= l.header_divider.y + 10.0);
        assert!(l.sound_card.y + l.sound_card.h <= CC_H as f32);
        for row in l.sound_rows {
            assert!(row.y >= l.sound_card.y);
            assert!(row.y + row.h <= l.sound_card.y + l.sound_card.h);
        }
    }

    #[test]
    fn pactl_sinks_parse_names_and_descriptions() {
        let text = "Sink #61:\n\tState: RUNNING\n\tName: alsa_output.pci\n\
                    \tDescription: Built-in Audio Analog Stereo\n\n\
                    Sink #88:\n\tState: SUSPENDED\n\tName: bluez_output.AA\n\
                    \tDescription: WF-1000XM5\n\n";
        let routes = parse_pactl_sinks(text);
        assert_eq!(routes.len(), 2);
        assert_eq!(routes[0].name, "alsa_output.pci");
        assert_eq!(routes[0].desc, "Built-in Audio Analog Stereo");
        assert_eq!(routes[1].desc, "WF-1000XM5");
        assert_eq!(audio_route_icon(&routes[0]), "volume_up");
        assert_eq!(audio_route_icon(&routes[1]), "headphones");
    }

    #[test]
    fn notification_card_sized_to_content() {
        let l = cc_layout();
        // iter91: 8 px padding (was 12): bell top = card top + 8,
        // sub line ends ~379, card bottom = 379 + 8.
        assert!((l.bell_circle.y - l.notif_card.y - 8.0).abs() < 1e-6);
        assert!((l.notif_card.y + l.notif_card.h - (l.notif_sub.y + 13.0) - 8.0).abs() < 1e-6);
        assert!((l.notif_card.h - 91.0).abs() < 1e-6);
        // Panel bottom margin below the power row is still exactly 14.
        assert!((l.panel.h - (l.power_buttons[0].y + l.power_buttons[0].h) - 14.0).abs() < 1e-6);
    }

    // iter91: R3 + frost + header divider + chevron restyle.
    #[test]
    fn panel_height_447_no_dead_band() {
        let l = cc_layout();
        assert!((l.panel.h - 447.0).abs() < 1e-6);
        assert_eq!(CC_H, 447);
        // No inter-section gap exceeds 14 (old layout: 75 above the
        // card, 56 below it); card top still 296, so the Sound->card
        // gap is still exactly 14.
        assert!((l.notif_card.y - (l.sound_track.y + l.sound_track.h) - 14.0).abs() < 1e-6);
    }

    #[test]
    fn power_row_has_three_equal_buttons() {
        let l = cc_layout();
        assert_eq!(l.power_buttons.len(), 3);
        for (i, b) in l.power_buttons.iter().enumerate() {
            assert!((b.w - (CC_CONTENT_W - 2.0 * POWER_GAP) / 3.0).abs() < 1e-6);
            assert!((b.h - POWER_H).abs() < 1e-6);
            if i > 0 {
                assert!((b.x - (l.power_buttons[i - 1].x + l.power_buttons[i - 1].w) - POWER_GAP).abs() < 1e-6);
            }
        }
        assert!((l.power_buttons[0].x - CC_PAD).abs() < 1e-6);
        assert!((l.power_buttons[2].x + l.power_buttons[2].w - (CC_W as f32 - CC_PAD)).abs() < 1e-6);
    }

    #[test]
    fn header_divider_groups_header() {
        let l = cc_layout();
        // 1 px hairline, full content width, between the header row
        // bottom (HEADER_Y + HEADER_H = 24) and the grid top (36).
        assert!((l.header_divider.y - 30.0).abs() < 1e-6);
        assert!((l.header_divider.h - 1.0).abs() < 1e-6);
        assert!((l.header_divider.x - CC_PAD).abs() < 1e-6);
        assert!((l.header_divider.w - CC_CONTENT_W).abs() < 1e-6);
        assert!(l.header_divider.y > l.header.y + l.header.h);
        assert!(l.header_divider.y + l.header_divider.h < GRID_Y);
    }

    #[test]
    fn chevron_is_dimmed_row_affordance() {
        // iter91: the row chevron is dimmed + small (14 px) so it no
        // longer clones the white nav-disc chevrons; the hit rect is
        // unchanged (28x36, full track height).
        assert_eq!(C_END_BTN_CHEVRON, (140, 146, 152));
        let l = cc_layout();
        for btn in [l.display_end_btn, l.sound_end_btn] {
            assert!((btn.w - 28.0).abs() < 1e-6);
            assert!((btn.h - 36.0).abs() < 1e-6);
        }
    }

    #[test]
    fn frost_menu_092_unified_with_main() {
        // iter128: FROST_ALPHA_MENU 0.92 + scalar floor 2.5 (was
        // 0.80/5.0) — the bright leg still filled at mean ~11.3
        // (a grey slab); the reference panels read ~2-3. fill =
        // bg*(1-fa) + floor*fa, SDF alpha 1.0 inside. Values are
        // exact f32-truncation results, cross-checked against
        // main.rs iter89/iter128 tests.
        // (a) iter89 bright leg, day.jpg sample ~(130,120,105):
        // (12,11,10) — same formula as main/launcher now.
        let bright = frost_fill_px_blur(1.0, -1.0, 0.0, 0.0, 1.0, 1.0, FROST_ALPHA_MENU, [130, 120, 105], [130, 120, 105]);
        assert_eq!(bright, [12, 11, 10, 255]);
        // (b) warm bright sample (200,170,120): (18,15,11). The
        // fill itself does NOT neutralise — it preserves the
        // sample's tilt (scalar floor adds no cast). Cast control
        // lives upstream in neutralise_sample (wallpaper_blur.rs),
        // which (iter103, strength 0.88 + exposure 0.30) pulls this
        // sample to (53,52,50) -> (6,6,6) at fa 0.92.
        // Do NOT re-assert B>R here: a raw warm sample is warm.
        let warm = frost_fill_px_blur(1.0, -1.0, 0.0, 0.0, 1.0, 1.0, FROST_ALPHA_MENU, [200, 170, 120], [200, 170, 120]);
        assert_eq!(warm, [18, 15, 11, 255]);
        // (c) dark backdrop (5,5,5): (2,2,2) — was (5,5,5) at
        // 0.80/5.0, (2,3,9) at 0.94/(2,3,10).
        let dark = frost_fill_px_blur(1.0, -1.0, 0.0, 0.0, 1.0, 1.0, FROST_ALPHA_MENU, [5, 5, 5], [5, 5, 5]);
        assert_eq!(dark, [2, 2, 2, 255]);
        // (d) pure-white sample: (22,22,22) — still dark (lum < 25).
        let white = frost_fill_px_blur(1.0, -1.0, 0.0, 0.0, 1.0, 1.0, FROST_ALPHA_MENU, [255, 255, 255], [255, 255, 255]);
        assert_eq!(white, [22, 22, 22, 255]);
        // (e) bright sky (170,180,200): (15,16,18), B-R +3 —
        // matches main.rs/launcher exactly (unification check).
        let sky = frost_fill_px_blur(1.0, -1.0, 0.0, 0.0, 1.0, 1.0, FROST_ALPHA_MENU, [170, 180, 200], [170, 180, 200]);
        assert_eq!(sky, [15, 16, 18, 255]);
        assert!(sky[2] as i32 - sky[0] as i32 >= 2);
        // (f) no-wallpaper path is the same opaque floor as main.rs:
        // 2.5*0.92 = 2.3 -> [2,2,2,255] (was [4,4,4,255] at 5.0*0.80).
        let none = frost_fill_px(1.0, -1.0, 0.0, 0.0, 1.0, 1.0, FROST_ALPHA_MENU);
        assert_eq!(none, [2, 2, 2, 255]);
    }

    #[test]
    fn frost_live_neutral_sky_iter103() {
        // Same shared fill as main.rs: the gap1 implied live sky
        // sample, through the shared strengthened neutralise_sample
        // at capture, fills with B-R in +0..+5 at fa 0.92 and a
        // mean in the reference's (4,5,5)-class band +/-6
        // (iter128: was (10,11,11) at fa 0.80).
        let n = crate::wallpaper_blur::neutralise_sample([56, 133, 187]);
        let p = frost_fill_px_blur(1.0, -1.0, 0.0, 0.0, 1.0, 1.0, crate::wallpaper_blur::FROST_ALPHA, n, n);
        let br = p[2] as i32 - p[0] as i32;
        assert!((0..=5).contains(&br), "CC B-R {} in {:?}", br, p);
        assert!(p[0] >= 4 && p[0] <= 16, "CC R {} in {:?}", p[0], p);
    }

    #[test]
    fn frost_card_sky_neutral_is_pinned() {
        // iter103 (Builder B, MERGE) — TRIPWIRE: the bright-sky neutral
        // the card expectations below are computed from. MERGE: A
        // strengthened neutralise_sample in place, so the pinned value
        // moves from B's [122,139,151] to A's value, hand-computed
        // from the strengthened fn ([80,151,200]: lum 135.36, chroma
        // 0.471, s pinned to the 0.88 floor, k 0.12, exposure 0.30 ->
        // [39,41,43]). iter113: the offset is gone; the pinned value
        // moves from [29,37,43] to [39,41,43]. If neutralisation
        // changes again, this test
        // fails on purpose: update the pinned value AND the card
        // expectations together, so cards keep tracking the panel.
        assert_eq!(
            crate::wallpaper_blur::neutralise_sample([80, 151, 200]),
            [39, 41, 43]
        );
    }

    #[test]
    fn frost_card_px_zero_dev_matches_panel_minus_floor_step() {
        // iter103 (Builder B): zero deviation, bright-sky neutral
        // [122,139,151]. Card = n*0.08 + 3*0.92 = [12,13,14]; panel =
        // n*0.08 + 2.5*0.92 = [12,13,14] — the card now tracks the
        // panel within ~0.5 levels (iter128: the floors nearly
        // coincide, (2.5-3)*0.92 = -0.46; was 1.6 levels under at
        // 0.80/5.0), well inside the ~5-level acceptance. B-R
        // tracks the panel.
        // MERGE: frost_fill_px_blur gained the `lm` arg (A) — the
        // zero-deviation case passes lm = bg.
        let card = frost_card_px([122, 139, 151], [122.0, 139.0, 151.0]);
        assert_eq!(card, [12, 13, 14]);
        let panel = frost_fill_px_blur(
            1.0, -1.0, 0.0, 0.0, 1.0, 1.0, FROST_ALPHA_MENU, [122, 139, 151],
            [122, 139, 151],
        );
        assert_eq!(panel, [12, 13, 14, 255]);
        for c in 0..3 {
            assert!((panel[c] as i32 - card[c] as i32).abs() <= 2);
        }
        assert_eq!(card[2] as i32 - card[0] as i32, 2);
    }

    #[test]
    fn frost_card_px_deviation_moves_output_by_k() {
        // iter103 (Builder B): a +10 deviation on R moves R by
        // k*10 = 7 (12.5 -> 19.5 -> 19); -10 moves it to 5.5 -> 5.
        // G/B untouched (zero deviation there).
        // iter128: base card R is 12.5 at fa 0.92 (was 26.8 at 0.80).
        assert_eq!(
            frost_card_px([122, 139, 151], [112.0, 139.0, 151.0]),
            [19, 13, 14]
        );
        assert_eq!(
            frost_card_px([122, 139, 151], [132.0, 139.0, 151.0]),
            [5, 13, 14]
        );
    }

    #[test]
    fn frost_card_px_dark_leg_stays_black() {
        // iter103 (Builder B): dark desktop, zero deviation — the
        // card lands at the floor (3.16 -> 3, iter128; was 3.4 -> 3),
        // inside the <= (12,12,16) dark-leg class.
        assert_eq!(frost_card_px([5, 5, 5], [5.0, 5.0, 5.0]), [3, 3, 3]);
        assert_eq!(
            frost_card_px([10, 10, 14], [10.0, 10.0, 14.0]),
            [3, 3, 3]
        );
    }

    #[test]
    fn frost_card_px_clamps_both_ways() {
        // iter103 (Builder B; iter113: the low clamp is the card
        // floor 2.76 -> 2, not 0, at fa 0.92): extreme deviations
        // clamp, never wrap. iter128: the high clamp caps the
        // deviation at +12, so a white sample over a black mean
        // lands at 31 (20.4+2.76+8.4) — not 231 as before, when the
        // uncapped +255 deviation printed straight through.
        assert_eq!(
            frost_card_px([255, 255, 255], [0.0, 0.0, 0.0]),
            [31, 31, 31]
        );
        assert_eq!(
            frost_card_px([0, 0, 0], [255.0, 255.0, 255.0]),
            [2, 2, 2]
        );
    }

    #[test]
    fn frost_card_local_mean_uniform_is_identity() {
        // iter103 (Builder B, MERGE): on a uniform backdrop the local
        // mean is the sample itself, so the deviation is exactly zero
        // and the card is flat — as it must be when there is nothing
        // to mottle with. MERGE: the local mean now delegates to the
        // frame's precomputed BoxBlur mean, so this pins that a
        // uniform frame's mean equals its sample; the mean buffer is
        // built alongside data, as finish_live_frame does.
        let data = vec![100u8; crate::wallpaper_blur::LIVE_W
            * crate::wallpaper_blur::LIVE_H
            * 3];
        let lb = crate::wallpaper_blur::LiveBackdrop {
            mean: data.clone(),
            data,
        };
        assert_eq!(
            frost_card_local_mean(&lb, 100.0, 100.0),
            [100.0, 100.0, 100.0]
        );
    }

    #[test]
    fn frost_card_fill_mottles_on_sine_backdrop() {
        // iter103 (Builder B, MERGE): render-level proof — a 201x52 pill
        // filled from a grey sine backdrop (128 +/- 8, 25-buffer-px
        // period) must come out mottled (std >= 3, the acceptance
        // bar) at the panel-step mean (~13), not flat like the old
        // (12,11,14). Predicted mean 13, std ~4.4 (out = 13 +
        // 0.70*8*sin + 0.08*8*sin; the sine deviation of +/-8 is
        // inside the iter128 cap of 12, so it passes untouched).
        // iter128: the mean band was 24..=32 at fa 0.80 (128*0.20 +
        // 2.4 = 28); at fa 0.92 it is 128*0.08 + 2.76 = 13.0
        // (measured 12.48 over the partial-period window — the
        // window covers 6.48 sine periods and the per-pixel f32
        // truncation costs ~0.5). The assertions are the acceptance
        // bounds, not the prediction, so a local-mean change still
        // has to pass.
        // Measured window (x 20..181, y 20..32) is fully inside the
        // pill (PILL_R 12) at coverage 1.0. Canvas is BGRA.
        // MERGE: the frame carries its precomputed BoxBlur(25) local
        // mean alongside data; a sine of period 25 averages to its
        // mid value 128 under that blur, so the mean buffer is flat
        // 128 here. The fill reads lb.sample() directly (already
        // neutral in production; this synthetic frame is grey, which
        // neutralises to itself, so the raw sine is the right
        // stand-in). With the merged k = 0.70 the std is ~5.1, still
        // inside the 3..=10 bound.
        let (lw, lh) = (
            crate::wallpaper_blur::LIVE_W,
            crate::wallpaper_blur::LIVE_H,
        );
        let mut data = vec![0u8; lw * lh * 3];
        for by in 0..lh {
            for bx in 0..lw {
                let v = (128.0
                    + 8.0 * ((2.0 * std::f32::consts::PI * bx as f32) / 25.0).sin())
                    .round() as u8;
                let i = (by * lw + bx) * 3;
                data[i] = v;
                data[i + 1] = v;
                data[i + 2] = v;
            }
        }
        let lb = crate::wallpaper_blur::LiveBackdrop {
            data,
            mean: vec![128u8; lw * lh * 3],
        };
        let (w, h) = (201u32, 52u32);
        let mut canvas = vec![0u8; (w * h * 4) as usize];
        fill_rounded_rect_frost(
            &mut canvas,
            w,
            h,
            Rect::new(0.0, 0.0, 201.0, 52.0),
            PILL_R,
            &lb,
            (0.0, 0.0),
        );
        for ch in [0usize, 1, 2] {
            // BGRA: R is byte 2, G byte 1, B byte 0.
            let byte = 2 - ch;
            let mut vals: Vec<f32> = Vec::new();
            for y in 20..33 {
                for x in 20..182 {
                    let i = ((y * w + x) * 4) as usize + byte;
                    vals.push(canvas[i] as f32);
                }
            }
            let n = vals.len() as f32;
            let mean = vals.iter().sum::<f32>() / n;
            let var = vals.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / n;
            let std = var.sqrt();
            assert!(
                (10.0..=16.0).contains(&mean),
                "ch {ch}: mean {mean:.2} outside 10..=16"
            );
            assert!(
                (3.0..=10.0).contains(&std),
                "ch {ch}: std {std:.2} outside 3..=10 (mottle bar: >= 3)"
            );
        }
    }

    #[test]
    fn notif_card_is_video_dark() {
        // iter91: (32,34,38) -> (13,12,15), video-sampled from s07.
        assert_eq!(C_NOTIF_CARD, (13, 12, 15));
    }

    #[test]
    fn sliders_thick_end_button_full_height() {
        let l = cc_layout();
        assert!((l.display_track.h - 36.0).abs() < 1e-6);
        assert!((l.sound_track.h - 36.0).abs() < 1e-6);
        // End-button HIT rects span the full track height (row action,
        // not a 28x28 square) and sit at the same y as their track.
        // iter86: only the drawing changed (bare chevron) — the rects
        // and this test are unchanged.
        for (track, btn) in [
            (l.display_track, l.display_end_btn),
            (l.sound_track, l.sound_end_btn),
        ] {
            assert!((btn.h - track.h).abs() < 1e-6);
            assert!((btn.y - track.y).abs() < 1e-6);
        }
        // The iter27 no-op point (panel-local (219, 303)) still clears
        // the Sound track hit area (track + 4 px slack ends at 298).
        assert!(l.sound_track.y + l.sound_track.h + 4.0 < 303.0);
    }

    #[test]
    fn toggle_discs_uniform_circles() {
        let l = cc_layout();
        for c in l.toggle_circles {
            assert!((c.w - 48.0).abs() < 1e-6);
            assert!((c.h - 48.0).abs() < 1e-6);
            assert!((c.w - c.h).abs() < 1e-6);
        }
    }

    #[test]
    fn thumb_spans_full_track_at_0_and_100_iter107() {
        // iter107: the thumb centre spans the FULL track — the old
        // clamp pinned it to [track.x + 15, track.x + track.w - 15],
        // so the rendered centre read 4.0% at 0% and 96.0% at 100%
        // against the live wpctl value. The thumb still never leaves
        // the PANEL: THUMB_D is 28, so at 0% it spans [0, 28] (flush
        // with the panel edge) and at 100% [376, 404] (clear of the
        // end-button chevron, whose ink starts ~405.1).
        let l = cc_layout();
        for track in [l.display_track, l.sound_track] {
            assert!((thumb_cx(track, 0.0) - track.x).abs() < 1e-6);
            assert!((thumb_cx(track, 1.0) - (track.x + track.w)).abs() < 1e-6);
            assert!((thumb_cx(track, 0.5) - track.cx()).abs() < 1e-6);
            // Panel containment at both extremes (thumb fully on-panel).
            assert!((thumb_cx(track, 0.0) - THUMB_D / 2.0).abs() < 1e-6);
            assert!(thumb_cx(track, 1.0) + THUMB_D / 2.0 <= CC_W as f32);
            // Chevron clearance at 100%: thumb right edge 404, the
            // chevron's left tip is at end_btn.cx() - 7*0.7 = 405.1.
            assert!(thumb_cx(track, 1.0) + THUMB_D / 2.0 < 405.1);
        }
        assert!((THUMB_D - 28.0).abs() < 1e-6);
    }

    #[test]
    fn slider_fill_end_is_square_and_visible_iter107() {
        // iter107: the fill's leading edge is the value edge. The
        // construction in draw_slider is: rounded left cap
        // [track.x, track.x + 2*TRACK_R] + sharp rect
        // [track.x + TRACK_R, track.x + fill_w]. This test pins the
        // geometry that makes the fill MEASURABLE: (a) the sharp rect
        // ends exactly at track.x + fill_w for every checklist level;
        // (b) the thumb (Ø28) leaves 4-logical-px slivers top/bottom
        // of the 36-tall track where the fill is not occluded, so the
        // square end is visible there; (c) the thumb centre equals the
        // fill end (same expression), so handle-centre and fill-end
        // readings agree by construction.
        let l = cc_layout();
        for track in [l.display_track, l.sound_track] {
            for level in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
                let fill_w = track.w * level;
                let fill_end = track.x + fill_w;
                // (c) centre == fill end.
                assert!((thumb_cx(track, level) - fill_end).abs() < 1e-6);
                // (a) sharp-rect end, for levels where it exists.
                if fill_w > 2.0 * TRACK_R {
                    let rect_end = track.x + TRACK_R + (fill_w - TRACK_R);
                    assert!((rect_end - fill_end).abs() < 1e-6);
                }
                // (b) sliver height per side: track half-height minus
                // thumb radius.
                assert!((TRACK_H / 2.0 - THUMB_D / 2.0 - 4.0).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn live_display_tracks_system() {
        // iter94: the frozen-0.58 contract is gone — `live()` returns
        // the refresher's cached LIVE brightness (seed 0.58 until the
        // first poll lands ~100-300 ms after daemon start). `live()`
        // itself stays cache-only (no spawn on the calling thread —
        // structural, see `live()` docs); this test asserts the cached
        // value tracks the system reader once the first poll has run.
        // (The first poll is immediate on refresher spawn; allow up to
        // ~6 s for the thread + two `brightnessctl` spawns.)
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(6);
        let mut d = ControlCenterData::live();
        while std::time::Instant::now() < deadline {
            if let Some(lv) = live_display_level() {
                if (d.display_level - lv).abs() < 0.02 {
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
            d = ControlCenterData::live();
        }
        assert!((0.0..=1.0).contains(&d.display_level));
        if let Some(lv) = live_display_level() {
            assert!(
                (d.display_level - lv).abs() < 0.02,
                "cached display {} vs live reader {}",
                d.display_level,
                lv
            );
        }
    }

    // iter86: double-click race — flip-parity proofs. Pure logic (no
    // I/O, no system commands): they test the intent / queue / merge
    // pieces the fix rests on. Live parity on the real system is
    // re-verified by the verifier after deploy (iter86 re-test script).

    fn all_toggle_keys() -> [ToggleKey; 4] {
        [
            ToggleKey::Wifi,
            ToggleKey::Focus,
            ToggleKey::Bluetooth,
            ToggleKey::NightLight,
        ]
    }

    /// The command a job carries must actually flip the state it was
    /// computed from: for directional toggles the last arg is "off" iff
    /// the toggle was on; Focus is directionless ("-d" always) and flips
    /// server-side once per invocation.
    fn job_flips_from(job: &ToggleJob, on: bool) {
        match job.key {
            ToggleKey::Focus => {
                assert_eq!(job.cmd, "swaync-client");
                assert_eq!(job.args, vec!["-d".to_string()]);
            }
            ToggleKey::Wifi | ToggleKey::Bluetooth => {
                let dir = job.args.last().unwrap();
                assert_eq!(dir, if on { "off" } else { "on" });
            }
            ToggleKey::NightLight => {
                if on {
                    assert_eq!(job.cmd, "pkill");
                    assert_eq!(job.args, vec!["-x".to_string(), "wlsunset".to_string()]);
                } else {
                    assert_eq!(job.cmd, "sh");
                    assert!(job.args[1].contains("wlsunset"));
                }
            }
        }
    }

    #[test]
    fn toggle_job_parity_n_flips() {
        // N rapid clicks from a known state must produce N alternating
        // commands whose net effect is exactly N flips (even = back to
        // start, odd = flipped) — for every toggle, 1..=8 clicks.
        for key in all_toggle_keys() {
            for start in [false, true] {
                for n in 1..=8usize {
                    let mut on = start;
                    for _ in 0..n {
                        let job = toggle_job_for(key, on);
                        job_flips_from(&job, on);
                        on = !on;
                    }
                    assert_eq!(
                        on,
                        start ^ (n % 2 == 1),
                        "key={key:?} start={start} n={n}"
                    );
                }
            }
        }
    }

    // iter90: power-source text — pure format + µW -> W rounding.
    #[test]
    fn power_source_text_format() {
        assert_eq!(
            format_power_source(Some(true), "Charging", Some(45)),
            Some("Plugged in · 45 W".to_string())
        );
        assert_eq!(
            format_power_source(Some(true), "Charging", Some(0)),
            Some("Plugged in · 0 W".to_string())
        );
        assert_eq!(
            format_power_source(Some(true), "Charging", None),
            Some("Plugged in".to_string())
        );
        assert_eq!(
            format_power_source(Some(true), "Full", Some(0)),
            Some("Plugged in · 0 W".to_string())
        );
        assert_eq!(
            format_power_source(Some(true), "Not charging", None),
            Some("Plugged in".to_string())
        );
        assert_eq!(
            format_power_source(Some(true), "Full", Some(11)),
            Some("Plugged in · 11 W".to_string())
        );
        assert_eq!(
            format_power_source(Some(true), "Discharging", Some(12)),
            Some("Plugged in · 12 W".to_string())
        );
        assert_eq!(
            format_power_source(Some(false), "Discharging", Some(12)),
            Some("On battery".to_string())
        );
        assert_eq!(
            format_power_source(None, "Discharging", None),
            Some("On battery".to_string())
        );
        assert_eq!(format_power_source(None, "", None), None);
        assert_eq!(format_power_source(None, "Unknown", None), None);
    }

    #[test]
    fn microwatts_to_watts_rounds() {
        // Same-minute Surface reads: 44,892,000 µW -> 45 W,
        // 35,954,000 µW -> 36 W (rounds, does not truncate).
        assert_eq!(microwatts_to_watts(44_892_000), 45);
        assert_eq!(microwatts_to_watts(35_954_000), 36);
        assert_eq!(microwatts_to_watts(0), 0);
        assert_eq!(microwatts_to_watts(500_000), 1);
        assert_eq!(microwatts_to_watts(499_999), 0);
    }

    #[test]
    fn toggle_queue_fifo_order() {
        // The worker pops from the front, so jobs execute in click
        // order: 7 clicks from OFF must dequeue as
        // on,off,on,off,on,off,on — never two "off" in a row.
        let mut state = ToggleQueueState::new();
        let mut on = false;
        for _ in 0..7 {
            state.push(toggle_job_for(ToggleKey::Bluetooth, on));
            on = !on;
        }
        assert_eq!(state.pending[toggle_key_index(ToggleKey::Bluetooth)], 7);
        let mut on = false;
        while let Some(job) = state.queues[toggle_key_index(ToggleKey::Bluetooth)].pop_front()
        {
            job_flips_from(&job, on);
            on = !on;
        }
        assert!(on, "7 flips from OFF must end ON");
        // A second toggle's queue is independent (no cross-talk).
        assert_eq!(state.pending[toggle_key_index(ToggleKey::Focus)], 0);
    }

    #[test]
    fn merge_snapshot_preserves_pending_toggles() {
        // Fresh snapshot says everything is OFF; the cache says BT and
        // Focus are ON (optimistic flips, commands still queued). With
        // BT/Focus pending, the merged snapshot must keep the cached ON;
        // with nothing pending, it must take the snapshot's OFF.
        let mut fresh = ControlCenterData::video_defaults();
        fresh.bluetooth_on = false;
        fresh.focus_on = false;
        fresh.wifi_on = false;
        fresh.wifi_radio_on = false;
        fresh.night_light_on = false;
        let mut cached = ControlCenterData::video_defaults();
        cached.bluetooth_on = true;
        cached.focus_on = true;
        let mut pending = [0usize; 4];
        pending[toggle_key_index(ToggleKey::Bluetooth)] = 1;
        pending[toggle_key_index(ToggleKey::Focus)] = 2;
        let merged = merge_snapshot_preserving_pending(fresh.clone(), &cached, pending);
        assert!(merged.bluetooth_on, "pending BT keeps cached ON");
        assert!(merged.focus_on, "pending Focus keeps cached ON");
        assert!(!merged.night_light_on, "idle NL takes snapshot OFF");
        assert!(!merged.wifi_on, "idle Wi-Fi takes snapshot OFF");
        let merged_idle = merge_snapshot_preserving_pending(fresh, &cached, [0; 4]);
        assert!(!merged_idle.bluetooth_on, "no pending: snapshot wins");
        assert!(!merged_idle.focus_on, "no pending: snapshot wins");
    }

    // iter91: offline render proof — renders the panel with the real
    // fonts and writes raw BGRA (canvas order) to /tmp so PIL can
    // verify discs (w==h), glyph height ratios, the 0% slider (thumb
    // + track visible), the card colour and the divider WITHOUT a live
    // deploy. Skips silently when the fonts are absent (e.g. a machine
    // without the Surface fonts); never fails for a missing font.
    // Renders: video defaults (0.58/0.90), both sliders 0%, both 100%.
    #[test]
    fn render_proof_iter91() {
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
        let Some(home) = home else { return };
        let ui_font = crate::ui_font();
        let icon_bytes = match std::fs::read(home.join(".local/share/fonts/MaterialSymbolsRounded.ttf")) {
            Ok(b) => b,
            Err(_) => return,
        };
        let icon_font = match Font::from_bytes(icon_bytes, fontdue::FontSettings::default()) {
            Ok(f) => f,
            Err(_) => return,
        };
        let mut render = |name: &str, data: &ControlCenterData| {
            let mut canvas = vec![0u8; (CC_W * CC_H * 4) as usize];
            draw_controlcenter_panel(
                &mut canvas,
                CC_W,
                CC_H,
                &ui_font,
                Some(&icon_font),
                data,
                (0.0, 0.0),
            );
            std::fs::write(format!("/tmp/cc91-render-{name}.bgra"), &canvas).unwrap();
        };
        render("video", &ControlCenterData::video_defaults());
        let mut zero = ControlCenterData::video_defaults();
        zero.display_level = 0.0;
        zero.sound_level = 0.0;
        render("zero", &zero);
        let mut full = ControlCenterData::video_defaults();
        full.display_level = 1.0;
        full.sound_level = 1.0;
        render("full", &full);
    }

    /// r1c finding 2: an unreadable adapter must NOT render as "On".
    /// The old snapshot default was `unwrap_or(true)`, which painted the
    /// accent-blue "On" state for a machine with no bluetooth at all.
    #[test]
    fn bluetooth_unknown_is_not_on() {
        let mut d = ControlCenterData::video_defaults();
        d.bluetooth_available = false;
        d.bluetooth_on = false;
        let def = ToggleDef {
            key: ToggleKey::Bluetooth,
            name: "Bluetooth",
            icon: "bluetooth",
            glyph_px: 32.0,
        };
        assert_eq!(d.subtext(&def), "No adapter");
        assert!(!d.toggle(ToggleKey::Bluetooth), "unknown must not read On");
        // A readable, powered-off adapter still says Off.
        d.bluetooth_available = true;
        assert_eq!(d.subtext(&def), "Off");
        d.bluetooth_on = true;
        assert_eq!(d.subtext(&def), "On");
    }

    /// Off-screen render of the Bluetooth submenu with real fonts.
    /// Calls the submenu painter directly so the global view flag is
    /// untouched and parallel tests stay deterministic.
    #[test]
    fn render_proof_bt_submenu() {
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
        let Some(home) = home else { return };
        let ui_font = crate::ui_font();
        let icon_bytes =
            match std::fs::read(home.join(".local/share/fonts/MaterialSymbolsRounded.ttf")) {
                Ok(b) => b,
                Err(_) => return,
            };
        let icon_font = match Font::from_bytes(icon_bytes, fontdue::FontSettings::default()) {
            Ok(f) => f,
            Err(_) => return,
        };
        let mut data = ControlCenterData::video_defaults();
        data.bluetooth_on = true;
        data.bt_devices = vec![
            BtDevice { addr: "AA:BB".into(), name: "Headphones".into(), paired: true, connected: true },
            BtDevice { addr: "CC:DD".into(), name: "Desk Speaker".into(), paired: true, connected: false },
            BtDevice { addr: "EE:FF".into(), name: "Keyboard".into(), paired: false, connected: false },
        ];
        let mut canvas = vec![0u8; (CC_W * CC_H * 4) as usize];
        draw_controlcenter_panel(
            &mut canvas, CC_W, CC_H, &ui_font, Some(&icon_font), &data, (0.0, 0.0),
        );
        // Mimic the real early-return: wipe the normal body, keep the
        // header, then paint only the submenu.
        for i in ((32 * CC_W as usize * 4)..canvas.len()).step_by(4) {
            canvas[i] = 0;
            canvas[i + 1] = 0;
            canvas[i + 2] = 0;
            canvas[i + 3] = 255;
        }
        let l = cc_layout();
        draw_bt_submenu(&mut canvas, CC_W, CC_H, &ui_font, Some(&icon_font), &data, &l);
        std::fs::write("/tmp/cc-bt-submenu.bgra", &canvas).unwrap();
    }

    /// Off-screen render of the Sound submenu with real fonts.
    /// Direct painter call, so no global view flag is touched.
    #[test]
    fn render_proof_sound_submenu() {
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
        let Some(home) = home else { return };
        let ui_font = crate::ui_font();
        let icon_bytes =
            match std::fs::read(home.join(".local/share/fonts/MaterialSymbolsRounded.ttf")) {
                Ok(b) => b,
                Err(_) => return,
            };
        let icon_font = match Font::from_bytes(icon_bytes, fontdue::FontSettings::default()) {
            Ok(f) => f,
            Err(_) => return,
        };
        let mut data = ControlCenterData::video_defaults();
        data.audio_default = "alsa_output.pci".into();
        data.audio_routes = vec![
            AudioRoute { name: "alsa_output.pci".into(), desc: "Built-in Audio Analog Stereo".into() },
            AudioRoute { name: "bluez_output.AA".into(), desc: "WF-1000XM5".into() },
        ];
        let mut canvas = vec![0u8; (CC_W * CC_H * 4) as usize];
        draw_controlcenter_panel(
            &mut canvas, CC_W, CC_H, &ui_font, Some(&icon_font), &data, (0.0, 0.0),
        );
        for i in ((32 * CC_W as usize * 4)..canvas.len()).step_by(4) {
            canvas[i] = 0;
            canvas[i + 1] = 0;
            canvas[i + 2] = 0;
            canvas[i + 3] = 255;
        }
        let l = cc_layout();
        draw_sound_submenu(&mut canvas, CC_W, CC_H, &ui_font, Some(&icon_font), &data, &l);
        std::fs::write("/tmp/cc-sound-submenu.bgra", &canvas).unwrap();
    }

    // iter107: PIXEL proof that the rendered slider value matches the
    // level — the acceptance the v0.41 critic could not verify from
    // pixels (visible blue stopped at ~46% for a 49% level; handle
    // centre read 45.7% against the critic's span). Renders the whole
    // panel off-screen with the real fonts (same pattern as
    // render_proof_iter91) at sound_level 0/25/50/75/100% and measures
    // BOTH readings from the BGRA canvas:
    //   (a) handle centre = centre of the white-thumb bbox on the
    //       Sound row (white = all channels > 235), windowed to
    //       expected centre ± 20 so the fill glyph / chevron cannot
    //       pollute it, as a fraction of the TRUE track span
    //       [track.x, track.x + track.w];
    //   (b) fill end = rightmost accent pixel on the top sliver rows
    //       (track.y+1..track.y+3 — above the thumb, which covers
    //       |dy| <= THUMB_D/2, and clear of the fill's top-edge AA),
    //       same fraction. The accent is read from slider_accent()
    //       (theme accent on the Surface, #74A3F6 fallback), matched
    //       within ±15 per channel.
    // Both must read the level within 1% (centre ±1.0 logical px,
    // fill end +0.5 px pixel-centre convention). Skips silently when
    // the fonts are absent, like render_proof_iter91.
    #[test]
    fn slider_rendered_value_matches_level_iter107() {
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
        let Some(home) = home else { return };
        let ui_font = crate::ui_font();
        let icon_bytes = match std::fs::read(home.join(".local/share/fonts/MaterialSymbolsRounded.ttf")) {
            Ok(b) => b,
            Err(_) => return,
        };
        let icon_font = match Font::from_bytes(icon_bytes, fontdue::FontSettings::default()) {
            Ok(f) => f,
            Err(_) => return,
        };
        let l = cc_layout();
        let track = l.sound_track;
        let accent = slider_accent();
        // Canvas is BGRA (see blend_pixel): [B, G, R, A].
        let px = |canvas: &[u8], x: i32, y: i32| -> (i32, i32, i32) {
            let i = ((y as usize) * (CC_W as usize) + (x as usize)) * 4;
            (canvas[i + 2] as i32, canvas[i + 1] as i32, canvas[i] as i32)
        };
        let is_white = |c: (i32, i32, i32)| c.0 > 235 && c.1 > 235 && c.2 > 235;
        let is_accent = |c: (i32, i32, i32)| {
            (c.0 - accent.0 as i32).abs() <= 15
                && (c.1 - accent.1 as i32).abs() <= 15
                && (c.2 - accent.2 as i32).abs() <= 15
        };
        for level in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
            let mut data = ControlCenterData::video_defaults();
            data.display_level = 0.5;
            data.sound_level = level;
            let mut canvas = vec![0u8; (CC_W * CC_H * 4) as usize];
            draw_controlcenter_panel(
                &mut canvas,
                CC_W,
                CC_H,
                &ui_font,
                Some(&icon_font),
                &data,
                (0.0, 0.0),
            );
            let expect = track.x + track.w * level;
            // (a) handle centre from the white bbox on the Sound row.
            let y0 = track.y as i32;
            let y1 = (track.y + track.h) as i32;
            let x_lo = (expect as i32 - 20).max(0);
            let x_hi = (expect as i32 + 20).min(CC_W as i32 - 1);
            let mut wx0 = i32::MAX;
            let mut wx1 = i32::MIN;
            for y in y0..y1 {
                for x in x_lo..=x_hi {
                    if is_white(px(&canvas, x, y)) {
                        wx0 = wx0.min(x);
                        wx1 = wx1.max(x);
                    }
                }
            }
            assert!(wx0 <= wx1, "no white thumb pixels at level {level}");
            // Pixel-centre convention: pixel i covers [i, i+1).
            let centre = (wx0 + wx1 + 1) as f32 / 2.0;
            assert!(
                (centre - expect).abs() <= 1.0,
                "handle centre {centre} != {expect} at level {level} (white x {wx0}..{wx1})"
            );
            let frac = (centre - track.x) / track.w;
            assert!((frac - level).abs() <= 0.01, "handle fraction {frac} != {level}");
            // (b) fill end from the top sliver rows (above the thumb).
            let mut bx1 = i32::MIN;
            for y in (track.y as i32 + 1)..=(track.y as i32 + 3) {
                for x in (track.x as i32)..((track.x + track.w) as i32) {
                    if is_accent(px(&canvas, x, y)) {
                        bx1 = bx1.max(x);
                    }
                }
            }
            if level == 0.0 {
                assert_eq!(bx1, i32::MIN, "blue fill present at 0%");
            } else {
                // Rightmost accent pixel centre is fill_end - 0.5.
                let fill_end = bx1 as f32 + 0.5;
                assert!(
                    (fill_end - expect).abs() <= 1.0,
                    "fill end {fill_end} != {expect} at level {level} (rightmost accent x {bx1})"
                );
                let ffrac = (fill_end - track.x) / track.w;
                assert!((ffrac - level).abs() <= 0.01, "fill fraction {ffrac} != {level}");
            }
        }
    }
}


/// iter98: offline glyph-band measurement — renders one toggle disc exactly
/// as the panel does (fill_rounded_rect disc + draw_icon / draw_focus_target
/// into a logical-size BGRA canvas, same helpers, same icon font) and
/// measures ink height with the AGREED method (iter94 critic2 E2 / iter95):
/// fill sampled 20 logical above the centre on the centreline; ink =
/// colour-distance from fill >45/>80 within r<34 phys of centre; band
/// 36.0-39.6 phys. The live buffer is logical 1504x1002 and the compositor
/// upscales x1.5, so the PRIMARY assertion is the LOGICAL ink height
/// (deterministic, compositor-independent): phys = logical x 1.5, verified
/// against every known live pair — logical 26 -> phys 39 (Wi-Fi 35.5/mint,
/// Bluetooth, Focus, all live 39), logical 27 -> phys 40.5 -> live 40/41
/// (Wi-Fi 37.0, NL 31.0), logical 25 -> phys 37.5 -> live 38 (NL 29.0).
/// A bilinear x1.5 upscale is also measured and printed, but only asserted
/// loosely (+/-1 row): the compositor's exact filter/phase is unknown.
/// NEAREST upscaling is deliberately NOT used: the disc-only control proves
/// it invents stray rows ([109, 118] at thr>45) from the disc-edge AA that
/// the live compositor never shows (disc-only logical is [0, 0] and
/// disc-only bilinear is empty). Guards the iter95 fix (Wi-Fi 35.5 /
/// NL 29.0) against a repeat of the iter96/97 loss, when the live tree
/// shipped the pre-fix 37.0/31.0 and v0.38/v0.39 measured byte-identical
/// to v0.36. The disc fills read EXACTLY as coded on live grims (no frost
/// over the discs), so the solid-fill render replicates live pixels.
#[cfg(test)]
mod iter98_tests {
    use super::*;

    const PERIWINKLE: (u8, u8, u8) = (124, 162, 245); // #7CA2F5, iter96 accent
    const MINT: (u8, u8, u8) = (140, 207, 126); // #8CCF7E, pre-iter96 accent
    const NAVY: (u8, u8, u8) = (11, 27, 58); // C_TOGGLE_ON_GLYPH
    const OFF_GREY: (u8, u8, u8) = (52, 55, 60); // C_TOGGLE_OFF
    const WHITE: (u8, u8, u8) = (255, 255, 255); // C_TOGGLE_OFF_GLYPH

    fn load_icon_font() -> Font {
        let home = std::env::var("HOME").expect("HOME is set on the Surface");
        let path = format!("{home}/.local/share/fonts/MaterialSymbolsRounded.ttf");
        let bytes =
            std::fs::read(&path).unwrap_or_else(|e| panic!("icon font missing: {path}: {e}"));
        Font::from_bytes(bytes, fontdue::FontSettings::default()).expect("icon font parses")
    }

    /// One toggle disc (48 logical, centre 60,60) in a 120x120 BGRA canvas.
    fn render_disc(
        font: &Font,
        name: &str,
        glyph_px: f32,
        fill: (u8, u8, u8),
        glyph_rgb: (u8, u8, u8),
    ) -> Vec<u8> {
        let (w, h) = (120u32, 120u32);
        let mut canvas = vec![0u8; (w * h * 4) as usize];
        fill_rounded_rect(
            &mut canvas,
            w,
            h,
            Rect::new(36.0, 36.0, CIRCLE_D, CIRCLE_D),
            CIRCLE_D / 2.0,
            fill,
        );
        if name == "focus" {
            draw_focus_target(&mut canvas, w, h, 60.0, 60.0, glyph_rgb);
        } else {
            draw_icon(&mut canvas, w, h, Some(font), name, glyph_px, 60.0, 60.0, glyph_rgb);
        }
        canvas
    }

    /// Canvas pixel as (R, G, B) floats; canvas is BGRA bytes (see blend_pixel).
    fn px_rgb(canvas: &[u8], w: u32, x: i32, y: i32) -> (f32, f32, f32) {
        let i = ((y as u32) * w + x as u32) as usize * 4;
        (canvas[i + 2] as f32, canvas[i + 1] as f32, canvas[i] as f32)
    }

    fn col_dist(a: (f32, f32, f32), b: (f32, f32, f32)) -> f32 {
        ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2) + (a.2 - b.2).powi(2)).sqrt()
    }

    /// Ink heights [thr>45, thr>80] on the LOGICAL canvas (centre 60,60,
    /// fill sampled 20 logical above centre, r < 34/1.5 logical).
    /// Band 36..=39 phys => logical 24..=26 (x1.5).
    fn ink_h_logical(canvas: &[u8]) -> [u32; 2] {
        let fill = px_rgb(canvas, 120, 60, 40);
        let mut out = [0u32; 2];
        for (k, thr) in [45.0f32, 80.0].iter().enumerate() {
            let mut rows: Vec<i32> = Vec::new();
            for y in 0..120i32 {
                let mut hit = false;
                for x in 0..120i32 {
                    let d = (((x - 60) as f32).powi(2) + ((y - 60) as f32).powi(2)).sqrt();
                    if d < 34.0 / 1.5 && col_dist(px_rgb(canvas, 120, x, y), fill) > *thr {
                        hit = true;
                        break;
                    }
                }
                if hit {
                    rows.push(y);
                }
            }
            out[k] = if rows.is_empty() {
                0
            } else {
                (rows[rows.len() - 1] - rows[0] + 1) as u32
            };
        }
        out
    }

    /// Bilinear x1.5 upscale of the 120x120 logical render to 180x180 phys,
    /// measured with the agreed method (centre 90,90, fill at (90,60),
    /// r<34 phys). Diagnostic / loose check only: the compositor's exact
    /// filter differs by up to ~1 row (observed: logical 26 -> bilin 40/38
    /// vs live 39/39).
    fn ink_h_phys_bilin(canvas: &[u8]) -> [u32; 2] {
        let mut log: Vec<(f32, f32, f32)> = Vec::with_capacity(120 * 120);
        for y in 0..120i32 {
            for x in 0..120i32 {
                log.push(px_rgb(canvas, 120, x, y));
            }
        }
        let sample = |sx: f32, sy: f32| -> (f32, f32, f32) {
            let x0 = sx.floor() as i32;
            let y0 = sy.floor() as i32;
            let fx = sx - x0 as f32;
            let fy = sy - y0 as f32;
            let mut acc = (0.0f32, 0.0f32, 0.0f32);
            for (dx, dy, w) in [
                (0, 0, (1.0 - fx) * (1.0 - fy)),
                (1, 0, fx * (1.0 - fy)),
                (0, 1, (1.0 - fx) * fy),
                (1, 1, fx * fy),
            ] {
                let xi = (x0 + dx).clamp(0, 119) as usize;
                let yi = (y0 + dy).clamp(0, 119) as usize;
                let p = log[yi * 120 + xi];
                acc.0 += p.0 * w;
                acc.1 += p.1 * w;
                acc.2 += p.2 * w;
            }
            acc
        };
        // Phys pixel (x, y) samples logical ((x+0.5)/1.5 - 0.5, (y+0.5)/1.5 - 0.5).
        let phys = |x: i32, y: i32| -> (f32, f32, f32) {
            sample((x as f32 + 0.5) / 1.5 - 0.5, (y as f32 + 0.5) / 1.5 - 0.5)
        };
        let fill = phys(90, 60);
        let mut out = [0u32; 2];
        for (k, thr) in [45.0f32, 80.0].iter().enumerate() {
            let mut rows: Vec<i32> = Vec::new();
            for y in 0..180i32 {
                let mut hit = false;
                for x in 0..180i32 {
                    let d = (((x - 90) as f32).powi(2) + ((y - 90) as f32).powi(2)).sqrt();
                    if d < 34.0 && col_dist(phys(x, y), fill) > *thr {
                        hit = true;
                        break;
                    }
                }
                if hit {
                    rows.push(y);
                }
            }
            out[k] = if rows.is_empty() {
                0
            } else {
                (rows[rows.len() - 1] - rows[0] + 1) as u32
            };
        }
        out
    }

    /// Logical band: 36..=39 phys at x1.5 => 24..=26 logical.
    fn in_band_logical(h: u32) -> bool {
        (24..=26).contains(&h)
    }

    #[test]
    fn toggle_defs_carry_iter95_fix() {
        // Source-level guard: the values that were lost in iter96/97.
        let wifi = TOGGLES.iter().find(|d| d.name == "Wi-Fi").expect("Wi-Fi toggle");
        let nl = TOGGLES
            .iter()
            .find(|d| d.name == "Night Light")
            .expect("Night Light toggle");
        assert_eq!(wifi.glyph_px, 35.5, "Wi-Fi glyph_px (iter95/iter98)");
        assert_eq!(nl.glyph_px, 29.0, "Night Light glyph_px (iter95/iter98)");
        let bt = TOGGLES
            .iter()
            .find(|d| d.name == "Bluetooth")
            .expect("Bluetooth toggle");
        assert_eq!(bt.glyph_px, 32.0, "Bluetooth untouched");
    }

    #[test]
    fn glyph_band_offline_matches_live() {
        let font = load_icon_font();
        // (label, icon, glyph_px, fill, glyph colour, expected logical [>45, >80])
        let cases: [(&str, &str, f32, (u8, u8, u8), (u8, u8, u8), [u32; 2]); 7] = [
            ("wifi 35.5 peri (FIX)", "wifi", 35.5, PERIWINKLE, NAVY, [26, 26]),
            ("wifi 37.0 peri (v0.39)", "wifi", 37.0, PERIWINKLE, NAVY, [27, 27]),
            ("wifi 35.5 mint (v0.37)", "wifi", 35.5, MINT, NAVY, [26, 26]),
            ("nightlight 29.0 grey (FIX)", "nightlight", 29.0, OFF_GREY, WHITE, [25, 25]),
            ("nightlight 31.0 grey (v0.39)", "nightlight", 31.0, OFF_GREY, WHITE, [27, 27]),
            ("bluetooth 32.0 peri", "bluetooth", 32.0, PERIWINKLE, NAVY, [26, 26]),
            ("focus procedural peri", "focus", 0.0, PERIWINKLE, NAVY, [26, 26]),
        ];
        println!("iter98 offline glyph band (logical band 24..=26, phys band 36..=39):");
        println!("{:28} {:>10} {:>12}", "case", "log>45/>80", "physBilin>45/>80");
        for (label, name, px, fill, glyph, expect_log) in cases {
            let canvas = render_disc(&font, name, px, fill, glyph);
            let lh = ink_h_logical(&canvas);
            let pb = ink_h_phys_bilin(&canvas);
            println!("{:28} {:>4}/{:<5} {:>4}/{:<5}", label, lh[0], lh[1], pb[0], pb[1]);
            assert_eq!(lh, expect_log, "{label}: logical ink {lh:?}, expected {expect_log:?}");
            if label.contains("(FIX)") || label.contains("bluetooth") || label.contains("focus") || label.contains("(v0.37)") {
                // Fixed / untouched cases: logical in band, and the bilinear
                // phys estimate within +/-1 row of the logical prediction.
                for (k, thr) in [" >45", " >80"].iter().enumerate() {
                    assert!(in_band_logical(lh[k]), "{label}{thr}: logical {lh:?} outside band 24..=26");
                    let pred = (lh[k] as f32 * 1.5).round() as i32;
                    assert!(
                        (pb[k] as i32 - pred).abs() <= 1,
                        "{label}{thr}: bilin phys {} vs logical prediction {pred} (off by >1)",
                        pb[k]
                    );
                }
            } else {
                // Pre-fix cases: logical taller than the fix (27 vs 26/25),
                // predicting phys 40.5 — over the band ceiling, as live.
                assert!(lh[0] >= 27 && lh[1] >= 27, "{label}: expected logical 27/27, got {lh:?}");
            }
        }
        // Disc-only control: no glyph => no ink at either threshold, logical
        // or bilinear (guards against disc-edge contamination of the metric).
        let (w, h) = (120u32, 120u32);
        let mut canvas = vec![0u8; (w * h * 4) as usize];
        fill_rounded_rect(
            &mut canvas,
            w,
            h,
            Rect::new(36.0, 36.0, CIRCLE_D, CIRCLE_D),
            CIRCLE_D / 2.0,
            PERIWINKLE,
        );
        assert_eq!(ink_h_logical(&canvas), [0, 0], "disc-only logical ink");
        assert_eq!(ink_h_phys_bilin(&canvas), [0, 0], "disc-only bilin ink");
    }
    /// r2 accessibility: a toggle disc must carry a glyph at EVERY draw scale.
    ///
    /// `draw_icon` used to rasterise at logical `px` and place at logical
    /// (cx, cy) on a device-sized canvas, so at 2x the icon landed at a quarter
    /// of the disc's position and half its size: the live Wi-Fi and Bluetooth
    /// discs were flat periwinkle with ZERO glyph pixels. The existing glyph
    /// tests all draw at scale 1.0, which is exactly why they never caught it.
    #[test]
    fn toggle_disc_glyph_is_present_and_centred_at_every_scale() {
        let font = load_icon_font();
        for scale in [1.0f32, 1.5, 2.0] {
            let w = (CC_W as f32 * scale).round() as u32;
            let h = (160.0 * scale).round() as u32;
            let mut canvas = vec![0u8; (w * h * 4) as usize];
            set_scale(w);
            // disc centre in LOGICAL units, as every caller passes it
            let (cx, cy) = (80.0f32, 80.0f32);
            let r = CIRCLE_D / 2.0;
            fill_rounded_rect(
                &mut canvas, w, h,
                Rect::new(cx - r, cy - r, CIRCLE_D, CIRCLE_D), r, PERIWINKLE,
            );
            draw_icon(&mut canvas, w, h, Some(&font), "bluetooth", 26.0, cx, cy, NAVY);

            // count glyph pixels inside the disc, in DEVICE units
            let (dcx, dcy) = ((cx * scale) as i32, (cy * scale) as i32);
            let rad = (r * scale) as i32;
            let mut glyph = 0usize;
            let mut sx = 0i64;
            let mut sy = 0i64;
            for y in (dcy - rad)..(dcy + rad) {
                for x in (dcx - rad)..(dcx + rad) {
                    if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
                        continue;
                    }
                    let (pr, pg, pb) = px_rgb(&canvas, w, x, y);
                    // navy glyph on periwinkle: far from the fill colour
                    if col_dist((pr, pg, pb), (124.0, 162.0, 245.0)) > 90.0
                        && pr + pg + pb > 30.0
                    {
                        glyph += 1;
                        sx += x as i64;
                        sy += y as i64;
                    }
                }
            }
            let disc_px = (std::f32::consts::PI * (rad * rad) as f32) as usize;
            assert!(
                glyph * 100 > disc_px * 8,
                "scale {scale}: only {glyph} glyph px in a {disc_px} px disc \
                 (icon missing or mis-scaled)"
            );
            // and it must sit ON the disc, not a quarter of the way there
            let (mx, my) = (sx / glyph as i64, sy / glyph as i64);
            assert!(
                (mx - dcx as i64).abs() <= 3 && (my - dcy as i64).abs() <= 3,
                "scale {scale}: glyph centroid ({mx},{my}) vs disc centre \
                 ({dcx},{dcy})"
            );
            SCALE.with(|c| c.set(1.0));
        }
    }

}

#[cfg(test)]
mod iter108_tests {
    use super::*;

    const TOGGLE_FILL: (u8, u8, u8) = (124, 162, 245); // #7CA2F5, iter96 accent

    fn max_run(profile: &[u32]) -> u32 {
        let mut run = 1u32;
        let mut worst = 1u32;
        for i in 1..profile.len() {
            if profile[i] == profile[i - 1] {
                run += 1;
                worst = worst.max(run);
            } else {
                run = 1;
            }
        }
        worst
    }

    /// Shared profile verdict (iter108 roundness acceptance):
    /// every row where the ideal is at least 30% of max within tol 3
    /// of the native-2x ideal; extent (first/last nonzero rows within
    /// 1 row of the ideal's, at most 2 fewer nonzero rows); shoulder
    /// jump (rows with ideal in [45%, 85%] of max) at most
    /// radius_buf/8, min 4 — the tip rows are excluded, a true circle
    /// legitimately jumps ~10 px between its first two rows; no flat
    /// run longer than the ideal's run + 4 (same quantisation note as main.rs: measured excesses WS +2, nav +4, toggle +2, bell 0). (Live v0.46 workspace
    /// circles, compositor-upscaled 1x: shoulder jumps 5-6, tip row
    /// 3 px.)
    fn assert_profile_close(name: &str, measured: &[u32], ideal: &[u32], radius_buf: f32) {
        assert_eq!(measured.len(), ideal.len(), "{name}: profile length");
        let max_ideal = *ideal.iter().max().unwrap_or(&0);
        let shoulder_bound = ((radius_buf / 8.0) as u32).max(4);
        let mut shoulder_jump = 0u32;
        for (i, (m, e)) in measured.iter().zip(ideal.iter()).enumerate() {
            if *e * 10 >= max_ideal * 3 {
                let diff = m.abs_diff(*e);
                assert!(
                    diff <= 3,
                    "{name} row {i}: measured {m} vs ideal {e} (tol 3); profile {measured:?}"
                );
            }
            if i > 0 && *e * 20 >= max_ideal * 9 && *e * 20 <= max_ideal * 17 {
                shoulder_jump = shoulder_jump.max(measured[i].abs_diff(measured[i - 1]));
            }
        }
        let run = max_run(measured);
        let ideal_run = max_run(ideal);
        let first = |p: &[u32]| p.iter().position(|w| *w > 0);
        let last = |p: &[u32]| p.iter().rposition(|w| *w > 0);
        let count = |p: &[u32]| p.iter().filter(|w| **w > 0).count();
        eprintln!("{name} profile: {measured:?} (shoulder_jump={shoulder_jump} max_run={run} ideal_run={ideal_run} span={:?}..{:?} ideal_span={:?}..{:?})", first(measured), last(measured), first(ideal), last(ideal));
        match (first(measured), first(ideal)) {
            (Some(a), Some(b)) => assert!(
                a.abs_diff(b) <= 1,
                "{name}: first nonzero row {a} vs ideal {b}"
            ),
            _ => panic!("{name}: empty profile"),
        }
        match (last(measured), last(ideal)) {
            (Some(a), Some(b)) => assert!(
                a.abs_diff(b) <= 1,
                "{name}: last nonzero row {a} vs ideal {b}"
            ),
            _ => panic!("{name}: empty profile"),
        }
        assert!(
            count(measured) + 2 >= count(ideal),
            "{name}: nonzero rows {} vs ideal {}",
            count(measured),
            count(ideal)
        );
        assert!(
            shoulder_jump <= shoulder_bound,
            "{name}: shoulder_jump {shoulder_jump} > {shoulder_bound}"
        );
        assert!(
            run <= ideal_run + 4,
            "{name}: max_run {run} > ideal {ideal_run} + 4"
        );
    }

    /// One toggle disc exactly as the panel draws it (fill_rounded_rect
    /// disc, same helper, no icon — the glyph sits inside the disc and
    /// cannot move the span), in a 120x120 BGRA canvas, centre (60,60).
    fn render_toggle_1x(fill: (u8, u8, u8)) -> Vec<u8> {
        let (w, h) = (120u32, 120u32);
        let mut canvas = vec![0u8; (w * h * 4) as usize];
        fill_rounded_rect(&mut canvas, w, h, Rect::new(36.0, 36.0, CIRCLE_D, CIRCLE_D), CIRCLE_D / 2.0, fill);
        canvas
    }

    /// The same disc rendered NATIVELY at 2x: the true-circle reference.
    fn render_toggle_2x(fill: (u8, u8, u8)) -> Vec<u8> {
        let (w, h) = (240u32, 240u32);
        let mut canvas = vec![0u8; (w * h * 4) as usize];
        fill_rounded_rect(
            &mut canvas,
            w,
            h,
            Rect::new(72.0, 72.0, 2.0 * CIRCLE_D, 2.0 * CIRCLE_D),
            CIRCLE_D,
            fill,
        );
        canvas
    }

    fn render_bell_1x() -> Vec<u8> {
        let (w, h) = (120u32, 120u32);
        let mut canvas = vec![0u8; (w * h * 4) as usize];
        fill_rounded_rect(&mut canvas, w, h, Rect::new(43.0, 43.0, BELL_D, BELL_D), BELL_D / 2.0, C_BELL_BG);
        canvas
    }

    fn render_bell_2x() -> Vec<u8> {
        let (w, h) = (240u32, 240u32);
        let mut canvas = vec![0u8; (w * h * 4) as usize];
        fill_rounded_rect(
            &mut canvas,
            w,
            h,
            Rect::new(86.0, 86.0, 2.0 * BELL_D, 2.0 * BELL_D),
            BELL_D,
            C_BELL_BG,
        );
        canvas
    }

    #[test]
    fn cc_toggle_disc_round_at_2x() {
        let up = crate::upscale_canvas_2x(&render_toggle_1x(TOGGLE_FILL), 120, 120);
        let ideal_canvas = render_toggle_2x(TOGGLE_FILL);
        // Disc is 96 px across at 2x, centred (120,120); 112x112 tile.
        let fill = (TOGGLE_FILL.0 as f32, TOGGLE_FILL.1 as f32, TOGGLE_FILL.2 as f32);
        let prof = crate::disc_row_widths(&up, 240, 64, 64, 112, fill, 0.5);
        let ideal = crate::disc_row_widths(&ideal_canvas, 240, 64, 64, 112, fill, 0.5);
        assert_profile_close("cc-toggle-O48", &prof, &ideal, CIRCLE_D);
    }

    #[test]
    fn cc_bell_disc_round_at_2x() {
        let up = crate::upscale_canvas_2x(&render_bell_1x(), 120, 120);
        let ideal_canvas = render_bell_2x();
        // Disc is 68 px across at 2x, centred (120,120); 84x84 tile.
        let fill = (C_BELL_BG.0 as f32, C_BELL_BG.1 as f32, C_BELL_BG.2 as f32);
        let prof = crate::disc_row_widths(&up, 240, 78, 78, 84, fill, 0.5);
        let ideal = crate::disc_row_widths(&ideal_canvas, 240, 78, 78, 84, fill, 0.5);
        assert_profile_close("cc-bell-O34", &prof, &ideal, BELL_D);
    }

}
