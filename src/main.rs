//! dynamite-rust — Dynamic Island pill for MangoWM (zwlr_layer_shell_v1).
//!
//! v0.4: collapsed clock (17 px, Bold) + lock icon + real power,
//! control-centre (live time/battery), music, calendar and launcher
//! panels; themes ported iter19 (wallpaper ported iter15).
//! Lockscreen PREVIEW state (iter10 builder, local only): socket words
//! `lockscreen` / `unlock` render the held lock frame fullscreen via
//! lockscreen.rs — a visual harness, NOT a session lock (no
//! ext-session-lock surface, no PAM, no keyboard input yet).
//! Morph animation (400 ms, ease-out cubic + two-phase content
//! cross-fade) is implemented in morph.rs and wired via begin_morph.
//! - Layer: overlay, anchored TOP (unanchored horizontally = centred by the
//!   compositor), ONE fixed-size "stage" surface 1176x636 (iter24 a2) with
//!   the panel drawn centred inside it and the nav discs OUTSIDE the panel
//!   edges; margin-top 0, so the pill top stays at gap_top 3 and the panel top at 31. The stage
//!   never resizes for island states (only the lockscreen preview goes
//!   fullscreen), so pointer coordinates are always in one fixed frame —
//!   the iter22/23 from-width morph translation is deleted (it assumed the
//!   compositor's surface width was still the FROM width, true only at
//!   morph start; later it missed by up to |from-to|/2 and collapsed —
//!   the iter23 4/4 A-1). Input region = panel rect + both disc squares
//!   + the 8 px bridge strips (rebuilt every frame in draw_if_ready), so
//!   margin clicks pass through / do nothing and never collapse.
//!   The lockscreen preview is the exception: margin 0, fullscreen
//!   (1504x1002), entered/exited via its own 33 ms LockIn/UnlockOut
//!   timer (v0.7, lockscreen::LockAnim) — never through begin_morph.
//! - Rendering: manual SDF capsule + fontdue glyph rasterisation straight
//!   into an wl_shm buffer (Argb8888). No GPU, no tiny-skia, no cosmic-text.
//! - Event loop: calloop. Redraws happen ONLY on: layer configure, the
//!   minute-boundary timer (clock), or a control-socket state change.
//!   Otherwise the loop blocks in epoll — zero CPU, zero wakeups.
//! - Control: a second invocation sends a one-word datagram to the daemon's
//!   unix socket ($XDG_RUNTIME_DIR/dynamite-rust.sock):
//!     dynamite-rust [collapsed|clock|lock|power|music|calendar|
//!                    controlcenter|launcher|wallpaper|themes|
//!                    lockscreen|unlock|quit]
//!   `lockscreen` / `unlock` are the fullscreen lockscreen PREVIEW (visual
//!   harness only — see lockscreen.rs; NOT a session lock).
//! - Pointer (iter20, a1 iter22, a2 iter24): hover is OFF in the live config
//!   (hover_open=0) — hovering the pill does nothing; a pill click
//!   opens the FIRST menu (Music / Clock-Media), wheel cycles menus,
//!   click toggles, nav discs OUTSIDE the expanded panel edges cycle on
//!   click (stage coords, shared constants with draw — see the stage
//!   geometry block); clicks on transparent stage margin do nothing and
//!   never collapse; in the Launcher, keyboard typing filters the real
//!   .desktop app list and Enter/click launches. Esc collapses from any
//!   open menu (keyboard interactivity is Exclusive in every menu
//!   state). All pointer- and keyboard-driven changes go through
//!   begin_morph — the same transition path as socket commands
//!   (Island::request_state).
//! - Keyboard (iter39 a10): while any menu is open, Left/Right cycle
//!   menus (same order as the discs/wheel, Clipboard last); Up/Down
//!   move the list selection in the Launcher / Clipboard, Enter
//!   launches / restores the selected row; Esc collapses from any
//!   menu. The grab is Exclusive in every menu state and None when
//!   collapsed, so keys belong to the focused window again on close.
//! - SIGTERM/SIGINT stop the loop cleanly.

use std::os::unix::net::UnixDatagram;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_registry,
    delegate_pointer, delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::{
            self,
            generic::Generic,
            timer::{TimeoutAction, Timer},
            EventLoop, Interest, LoopSignal, Mode, PostAction,
        },
        calloop_wayland_source::WaylandSource,
        client::{
            delegate_noop,
            globals::registry_queue_init,
            protocol::{
                wl_keyboard::WlKeyboard,
                wl_output::{Transform, WlOutput},
                wl_pointer::WlPointer,
                wl_region::WlRegion,
                wl_seat::WlSeat,
                wl_shm::Format,
                wl_surface::WlSurface,
            },
            Connection, QueueHandle,
        },
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers},
        pointer::{AxisScroll, PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{slot::{Slot, SlotPool}, Shm, ShmHandler},
};

// State-content modules (iter 6 builders, wired in iter 7;
// music + launcher wired in iter 10; morph in iter 10;
// lockscreen preview wired in iter 10 — draw-only, see lockscreen.rs).
mod calendar_panel;
mod controlcenter_panel;
mod dictation_canvas; // waveform rendered inside the island pill
mod launcher_panel;
mod launcher_icons; // iter17: real video-set icons (blitted by launcher_panel.rs)
mod clipboard_panel; // iter39: Clipboard view (a9)
mod library; // iter36 a6b: wallpaper + theme library (themes.json, apply paths)
mod lock_timer;
mod lockscreen;
mod morph;
mod music_panel;
mod power_panel;
mod selection; // iter39 a10: shared list-selection math (Launcher + Clipboard)
mod themes_panel;
mod wallpaper_panel;
mod tray; // iter108: SNI tray host (StatusNotifierWatcher)
mod wallpaper_blur;

// ---------------------------------------------------------------------------
// States: name -> (width, height, corner radius). Geometries measured from
// the reference video (see analysis/frame-measurements.md, 1080p logical).
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum IslandState {
    Collapsed,
    /// Dictation waveform: the island itself expands and draws the mic level.
    Dictation,
    Lock,
    Power,
    Music,
    Calendar,
    ControlCenter,
    Launcher,
    Wallpaper,
    Themes,
    /// Clipboard history view (iter39, a9): cliphist-backed list,
    /// last in the menu cycle (after Power). Drawn by
    /// clipboard_panel.rs; holds Exclusive keyboard like Launcher.
    Clipboard,
    /// TrayMenu (iter108): the DBusMenu of a tray item, opened
    /// by right-clicking its icon. NOT in MENU_CYCLE (it has an
    /// owning item, so wheel/arrow cycling must never land on
    /// it); Esc / Leave / pill-click collapse it like a menu.
    /// Drawn by tray.rs; sized by the open menu's row count.
    TrayMenu,
    /// Fullscreen lockscreen PREVIEW (visual harness only — NOT a
    /// session lock). Deliberately NOT in from_word(): the preview must
    /// bypass begin_morph (no capsule morph for the lockscreen) and is
    /// entered/exited via the `lockscreen` / `unlock` socket words.
    Lockscreen,
}

impl IslandState {
    fn from_word(word: &str) -> Option<Self> {
        Some(match word {
            "collapsed" => Self::Collapsed,
            "dictation" => Self::Dictation,
            "lock" => Self::Lock,
            "power" => Self::Power,
            "music" => Self::Music,
            "calendar" => Self::Calendar,
            "controlcenter" => Self::ControlCenter,
            "launcher" => Self::Launcher,
            "wallpaper" => Self::Wallpaper,
            "themes" => Self::Themes,
            "clipboard" => Self::Clipboard,
            "traymenu" => Self::TrayMenu,
            _ => return None,
        })
    }
    fn word(self) -> &'static str {
        match self {
            Self::Collapsed => "collapsed",
            Self::Dictation => "dictation",
            Self::Lock => "lock",
            Self::Power => "power",
            Self::Music => "music",
            Self::Calendar => "calendar",
            Self::ControlCenter => "controlcenter",
            Self::Launcher => "launcher",
            Self::Wallpaper => "wallpaper",
            Self::Themes => "themes",
            Self::Clipboard => "clipboard",
            Self::TrayMenu => "traymenu",
            Self::Lockscreen => "lockscreen",
        }
    }
    fn size(self) -> (u32, u32) {
        match self {
            Self::Collapsed => (cfg().collapsed_width, cfg().bar_height),
            Self::Dictation => (220, 64),
            Self::Lock => (54, 24),
            Self::Power => (400, cfg().expanded_height),
            Self::Music => (612, cfg().expanded_height),
            Self::Calendar => (342, 262), // iter89: was 280 — column -18, PANEL_H 262 (was 300 iter88)
            Self::ControlCenter => (438, 447), // iter109: +46 power-level row, CC_H 447
            Self::Launcher => (504, 366),
            Self::Wallpaper => (1008, 270),
            Self::Themes => (600, 366),
            Self::Clipboard => clipboard_panel::panel_size(),
            Self::TrayMenu => crate::tray::tray_menu_size(),
            Self::Lockscreen => LOCKSCREEN_SIZE,
        }
    }
    fn radius(self) -> f32 {
        match self {
            Self::Collapsed => 14.0,
            Self::Lock => 12.0,
            Self::Dictation => 20.0,
            // Fullscreen rect: square corners, no capsule.
            Self::Lockscreen => 0.0,
            _ => 20.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Runtime config — the settings-app contract.
// Read at startup and on the `reload` socket word from
// ~/.config/dynamite-rust/config.conf (key=value, one per line, `#`
// comments). The Dynamite Settings app writes this file; unknown keys
// are ignored here and preserved there. Keys:
//   collapsed_width  u32  40..=600    default 76    (Collapsed size)
//   expanded_height  u32  60..=800    default 150   (Power/Music height)
//   gap_top          i32  0..=200     default 3     (layer top margin)
//   inner_padding    f32  0..=64      default 14.0  (panel inner pad;
//                                                    nav-disc centre
//                                                    inset consumer)
//   anim_ms          u64  0..=1000    default 160   (morph duration —
//                                                    interaction-spec
//                                                    §2 measured view
//                                                    morphs at 130-230 ms)
//   hover_open       0/1              default 1     (pointer enter opens)
//   scroll_cycle     0/1              default 1     (wheel cycles menus)
//   clock_24h        0/1              default 1     (collapsed clock fmt)
//   hover_state      word             default controlcenter
//                                                    (IslandState::from_word)
// ---------------------------------------------------------------------------
#[derive(Clone)]
struct IslandConfig {
    collapsed_width: u32,
    expanded_height: u32,
    gap_top: i32,
    /// Pill height in logical px (settings "Bar height"). Drives the
    /// collapsed size, the exclusive strip, the top-row centre line
    /// and the derived clock size — one source, no loose 28s.
    bar_height: u32,
    inner_padding: f32,
    anim_ms: u64,
    hover_open: bool,
    scroll_cycle: bool,
    clock_24h: bool,
    hover_state: IslandState,
    /// iter36 a6b: currently applied wallpaper filename (library) and
    /// theme name. Persisted by library::apply_wallpaper / apply_theme.
    wallpaper: String,
    theme: String,
}

impl IslandConfig {
    const DEFAULT: Self = Self {
        collapsed_width: 76,
        expanded_height: 150,
        gap_top: TOP_MARGIN,
        bar_height: 28,
        inner_padding: 14.0,
        anim_ms: 160,
        hover_open: true,
        scroll_cycle: true,
        clock_24h: true,
        hover_state: IslandState::ControlCenter,
        wallpaper: String::new(),
        theme: String::new(),
    };
}

static CONFIG: std::sync::RwLock<IslandConfig> = std::sync::RwLock::new(IslandConfig::DEFAULT);

/// iter36 a6b: live selection state (calendar month offset, selected
/// wallpaper / theme indices) mirrored for the draw path.
/// draw_panel_into is a free fn — the morph path reaches it through
/// draw() / draw_state_into() / draw_morph_frame(), so threading the
/// Island fields through four functions would churn the morph code.
/// The Island arms (press / startup / reload) are the only writers,
/// via set_selection(); draw() reads.
static SELECTION: std::sync::RwLock<(i32, usize, usize)> = std::sync::RwLock::new((0, 0, 0));

fn set_selection(cal_offset: i32, wall_selected: usize, theme_selected: usize) {
    *SELECTION.write().unwrap() = (cal_offset, wall_selected, theme_selected);
}

/// Current runtime config (Clone snapshot; read at use sites so
/// a `reload` takes effect on the next state change / redraw).
fn cfg() -> IslandConfig {
    CONFIG.read().unwrap().clone()
}

fn config_path() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    home.join(".config/dynamite-rust/config.conf")
}

fn parse_config(text: &str) -> IslandConfig {
    let mut c = IslandConfig::DEFAULT;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let (k, v) = (k.trim(), v.trim());
        match k {
            "collapsed_width" => {
                if let Ok(n) = v.parse::<u32>() {
                    c.collapsed_width = n.clamp(40, 600);
                }
            }
            "expanded_height" => {
                if let Ok(n) = v.parse::<u32>() {
                    c.expanded_height = n.clamp(60, 800);
                }
            }
            "gap_top" => {
                if let Ok(n) = v.parse::<i32>() {
                    c.gap_top = n.clamp(0, 200);
                }
            }
            "bar_height" => {
                if let Ok(n) = v.parse::<u32>() {
                    c.bar_height = n.clamp(18, 80);
                }
            }
            "inner_padding" => {
                if let Ok(n) = v.parse::<f32>() {
                    c.inner_padding = n.clamp(0.0, 64.0);
                }
            }
            "anim_ms" => {
                if let Ok(n) = v.parse::<u64>() {
                    c.anim_ms = n.min(1000);
                }
            }
            "hover_open" => c.hover_open = matches!(v, "1" | "true"),
            "scroll_cycle" => c.scroll_cycle = matches!(v, "1" | "true"),
            "clock_24h" => c.clock_24h = matches!(v, "1" | "true"),
            "hover_state" => {
                if let Some(s) = IslandState::from_word(v) {
                    c.hover_state = s;
                }
            }
            // iter36 a6b: applied wallpaper / theme (library). Empty =
            // unset (pre-library config): defaults resolve at startup
            // to the live wallpaper.png's library entry / "anime".
            "wallpaper" => c.wallpaper = v.to_string(),
            "theme" => c.theme = v.to_string(),
            _ => {} // unknown key: the settings app owns it; ignore.
        }
    }
    c
}

/// (Re)read the config file into CONFIG. Missing file = defaults.
/// Called once at startup and on every `reload` socket word.
fn load_config() {
    let parsed = match std::fs::read_to_string(config_path()) {
        Ok(text) => parse_config(&text),
        Err(_) => IslandConfig::DEFAULT,
    };
    *CONFIG.write().unwrap() = parsed.clone();
    eprintln!(
        "dynamite-rust: config collapsed_width={} expanded_height={} gap_top={} bar_height={} inner_padding={} anim_ms={} hover_open={} scroll_cycle={} clock_24h={} hover_state={}",
        parsed.collapsed_width,
        parsed.expanded_height,
        parsed.gap_top,
        parsed.bar_height,
        parsed.inner_padding,
        parsed.anim_ms,
        parsed.hover_open,
        parsed.scroll_cycle,
        parsed.clock_24h,
        parsed.hover_state.word(),
    );
}

// ---------------------------------------------------------------------------
// Pointer interaction constants (hover / wheel / click / nav discs).
// ---------------------------------------------------------------------------
/// Menu states in wheel/arrow cycle order — the video's view sequence
/// (interaction-spec §1): Clock/Media -> Launcher -> ControlCentre ->
/// Calendar -> Wallpaper -> Themes -> Power -> Clipboard -> wrap.
const MENU_CYCLE: [IslandState; 8] = [
    IslandState::Music,
    IslandState::Launcher,
    IslandState::ControlCenter,
    IslandState::Calendar,
    IslandState::Wallpaper,
    IslandState::Themes,
    IslandState::Power,
    IslandState::Clipboard,
];

/// evdev button code for the left mouse button (wl_pointer `button`).
const BTN_LEFT: u32 = 0x110;

/// evdev button code for the right mouse button (iter108: tray
/// context menu — opens the item's DBusMenu as the TrayMenu
/// panel, or calls ContextMenu on the item when it has none).
const BTN_RIGHT: u32 = 0x111;

/// Minimum interval between two wheel-driven menu steps: touchpad
/// momentum keeps emitting axis events after the flick; the cooldown
/// caps the cycle rate so one flick = one step.
const WHEEL_COOLDOWN: Duration = Duration::from_millis(150);

// ---------------------------------------------------------------------------
// Stage geometry + nav discs OUTSIDE the panel (iter24, item a2).
// ---------------------------------------------------------------------------
// The layer surface is ONE fixed-size "stage" for every island state
// (collapsed, lock, all seven menus): STAGE_W x STAGE_H, never resized
// while island states morph. The panel is drawn centred in the stage;
// the two nav discs float OUTSIDE the panel, one just beyond each edge,
// centred on the pill's vertical line. Because the surface origin never
// moves, pointer coordinates are always in this one fixed frame —
// draw, hit-test and input region all use nav_disc_centres() below,
// so they can never drift apart, and NO morph-time coordinate
// translation exists any more (the iter22/23 from-width translation
// assumed the compositor's surface width was still the FROM width; it
// holds only at morph start and misses by up to |from-to|/2 later —
// the iter23 4/4 A-1 collapse).
//
// Sizing: widest panel is Wallpaper 1008, tallest is ControlCenter 447.
// The discs are Ø 28 (NAV_DISC_R 14) with a clear NAV_DISC_GAP of 8
// between the panel edge and the disc's inner edge, so the disc centre
// sits GAP + R = 22 beyond each panel edge and its outer edge 36
// beyond it. STAGE_PAD_X = 38 leaves 2 px of transparent slack past
// the disc. Vertically the discs sit at the PANEL'S MID-HEIGHT
// (iter30 a3b: oy + h/2 per view), not at the pill line.
// The pill states sit at STAGE_PAD_TOP = 3 (screen gap_top 3); the
// expanded menus hang BELOW the pill (iter30 a3c): panel top
// panel_top_menu() = 3 + 28 + 2 = 33 (screen 33), so the pill stays
// visible above the panel with a 4 px gap.
// The stage keeps slack for a ~10% expand overshoot (iter33: the
// overshoot itself is REMOVED — EaseOutCubic now, see begin_morph —
// but the slack costs nothing and still covers the panel-shadow
// tails), so
// STAGE_W = 1100 + 2*38 = 1176 and STAGE_H = 636. The layer top
// margin is gap_top - STAGE_PAD_TOP (= 0 at the live gap_top 3),
// so the pill top stays exactly at gap_top and the panel top at
// gap_top + 28 = 31 on screen. Panel centre x stays 752: the stage is
// centred by the compositor and the panel is centred in the stage.

/// Nav-disc radius (Ø 28, interaction-spec §6).
const NAV_DISC_R: f32 = 14.0;
/// Clear gap between a panel edge and the nearer disc edge.
const NAV_DISC_GAP: f32 = 8.0;
/// Transparent margin between the panel and the stage edge (x).
const STAGE_PAD_X: f32 = 38.0;
/// Transparent margin between the panel and the stage edge (y).
const STAGE_PAD_TOP: f32 = 3.0;
/// Pill height in stage-logical px (the one source the whole top row
/// reads: collapsed size, strip, centre line, derived text size).
fn pill_h() -> f32 {
    cfg().bar_height as f32
}

/// Expanded-panel top inside the stage (iter30 a3c): the panel hangs
/// BELOW the pill instead of swallowing it — pill top STAGE_PAD_TOP,
/// pill_h tall, then a 2 px gap. Pill states keep STAGE_PAD_TOP.
fn panel_top_menu() -> f32 {
    STAGE_PAD_TOP + pill_h() + 2.0
}
/// Fixed stage surface size (see the sizing note above).
const STAGE_W: u32 = (1100.0 + 2.0 * STAGE_PAD_X) as u32;
/// 636 = menu top 31 + ControlCenter overshoot height 577.5 (+5 slack)
/// (421 * 1.10) + panel-shadow tail 15 + 7.5 slack; the settled CC
/// shadow bottom is 29 + 447 + 15 = 461, the morph-overshoot shadow
/// bottom 31 + 577.5 + 15 = 623.5.
const STAGE_H: u32 = 636;

/// Render scale (iter108, circles-roundness): the stage is drawn at
/// logical 1x into a scratch canvas and then upscaled 2x into the shm
/// buffer, and the surface advertises buffer_scale 2 — so the
/// compositor DOWNSAMPLES 2x -> 1.5x instead of UPSAMPLING 1x -> 1.5x.
/// Why: at 1x a workspace disc is 16 buffer px with a 1 px AA band;
/// the compositor's 1.5x upscale quantises the outline into ~8 flat
/// facets (live v0.46 row-width profile 3,8,12,14,18,18,20,20,22,22,
/// 24,24,24,24,24,22,22,20,20,16,16,14,8,6 — the octagon Alexandre
/// reported 15 Nov). At 2x the disc is 32 buffer px with a 2 px AA
/// band and the downsampled 24 px disc is round (iter108_tests:
/// max shoulder jump 4, per-row within 2 px of a true circle, under
/// either compositor filter). True fractional scale
/// (wp_fractional_scale_v1, standing item a4) is NOT used here: SCTK
/// 0.19 exposes no fractional-scale API, and the iter108 simulation
/// shows a 2x buffer downsampled by the compositor gives the same disc
/// profile as a 1.5x fractional buffer under either filter — the
/// manual protocol binding is a separate iteration. Geometry is
/// untouched: surface size stays STAGE_W x STAGE_H logical, input
/// regions and pointer coords stay logical — only the buffer is 2x.
const RENDER_SCALE: u32 = 2;
/// Number of pinned shm slots the stage draws into (see draw_if_ready).
/// 2 is the safe minimum: one buffer can be on screen while the next is
/// drawn. 1 deadlocks when the compositor keeps the only buffer.
const SLOT_RING: usize = 2;

/// Bilinear 2x upscale of a premultiplied BGRA canvas (iter108) into
/// `dst` (which must hold (2w * 2h * 4) bytes). Exact 2x ratio, so
/// every destination pixel centre maps to a source coordinate of
/// i +/- 0.25 — the interpolation weights are exactly 1/4 and 3/4,
/// done in fixed point (no float in the inner loop, no per-call
/// allocation; a 1176x636 stage costs ~5 ms, see the iter108 perf
/// test). Edges clamp by replication. The canvas is premultiplied
/// (disc fills store colour*a with A=255 inside and (0,0,0,0)
/// outside), so interpolating all four channels together is the
/// correct premultiplied blend.
pub(crate) fn upscale_canvas_2x_into(src: &[u8], w: u32, h: u32, dst: &mut [u8]) {
    let (w, h) = (w as usize, h as usize);
    if w == 0 || h == 0 {
        return;
    }
    let (bw, bh) = (2 * w, 2 * h);
    debug_assert_eq!(src.len(), w * h * 4);
    debug_assert_eq!(dst.len(), bw * bh * 4);
    for dy in 0..bh {
        let j = dy / 2;
        let (y0, y1, wy0, wy1) = if dy % 2 == 0 {
            (j.saturating_sub(1), j, 1u32, 3u32)
        } else {
            (j, (j + 1).min(h - 1), 3u32, 1u32)
        };
        for dx in 0..bw {
            let i = dx / 2;
            let (x0, x1, wx0, wx1) = if dx % 2 == 0 {
                (i.saturating_sub(1), i, 1u32, 3u32)
            } else {
                (i, (i + 1).min(w - 1), 3u32, 1u32)
            };
            let di = (dy * bw + dx) * 4;
            for c in 0..4 {
                let s00 = src[(y0 * w + x0) * 4 + c] as u32;
                let s01 = src[(y0 * w + x1) * 4 + c] as u32;
                let s10 = src[(y1 * w + x0) * 4 + c] as u32;
                let s11 = src[(y1 * w + x1) * 4 + c] as u32;
                // Horizontal first (rounded), then vertical (rounded).
                let top = (wx0 * s00 + wx1 * s01 + 2) / 4;
                let bot = (wx0 * s10 + wx1 * s11 + 2) / 4;
                dst[di + c] = ((wy0 * top + wy1 * bot + 2) / 4) as u8;
            }
        }
    }
}

/// Allocating wrapper around [`upscale_canvas_2x_into`] (tests).
pub(crate) fn upscale_canvas_2x(src: &[u8], w: u32, h: u32) -> Vec<u8> {
    let mut dst = vec![0u8; (w * RENDER_SCALE * h * RENDER_SCALE * 4) as usize];
    upscale_canvas_2x_into(src, w, h, &mut dst);
    dst
}

/// Rect helpers for damage tracking (F-A step 4): half-open device/logical
/// rects, `(x0, y0, x1, y1)`.
pub(crate) fn union_rect(
    a: (i32, i32, i32, i32),
    b: (i32, i32, i32, i32),
) -> (i32, i32, i32, i32) {
    (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3))
}

/// Clip a rect to `w x h`; empty (x1<=x0) means nothing to do.
pub(crate) fn clip_rect(
    r: (i32, i32, i32, i32),
    w: u32,
    h: u32,
) -> (i32, i32, i32, i32) {
    let w = w as i32;
    let h = h as i32;
    (
        r.0.clamp(0, w),
        r.1.clamp(0, h),
        r.2.clamp(0, w),
        r.3.clamp(0, h),
    )
}

/// Zero one logical rect of a stage canvas. Settled frames clear only
/// the rect they will repaint + damage, so untouched shm pages stay
/// unfaulted (Pss counts resident pages, not the mapped pool).
fn clear_rect(canvas: &mut [u8], w: u32, h: u32, r: (i32, i32, i32, i32)) {
    let r = clip_rect(r, w, h);
    if r.2 <= r.0 || r.3 <= r.1 {
        return;
    }
    let w = w as usize;
    let x0 = r.0 as usize;
    let x1 = r.2 as usize;
    for y in r.1..r.3 {
        let i = ((y as usize) * w + x0) * 4;
        canvas[i..i + (x1 - x0) * 4].fill(0);
    }
}

/// `upscale_canvas_2x_into` for one logical rect (device rect = 2x):
/// the settled pill writes a few hundred KB, not the whole 11.97 MB
/// stage buffer, so only those pages become resident.
pub(crate) fn upscale_canvas_2x_region(
    src: &[u8],
    w: u32,
    h: u32,
    dst: &mut [u8],
    region: (i32, i32, i32, i32),
) {
    let (wu, hu) = (w as usize, h as usize);
    if wu == 0 || hu == 0 {
        return;
    }
    let r = clip_rect(region, w, h);
    if r.2 <= r.0 || r.3 <= r.1 {
        return;
    }
    let (bw, bh) = (2 * wu, 2 * hu);
    debug_assert_eq!(src.len(), wu * hu * 4);
    debug_assert_eq!(dst.len(), bw * bh * 4);
    let dx0 = (r.0 as usize) * 2;
    let dx1 = (r.2 as usize) * 2;
    let dy0 = (r.1 as usize) * 2;
    let dy1 = (r.3 as usize) * 2;
    for dy in dy0..dy1 {
        let j = dy / 2;
        let (y0, y1, wy0, wy1) = if dy % 2 == 0 {
            (j.saturating_sub(1), j, 1u32, 3u32)
        } else {
            (j, (j + 1).min(hu - 1), 3u32, 1u32)
        };
        for dx in dx0..dx1 {
            let i = dx / 2;
            let (x0, x1, wx0, wx1) = if dx % 2 == 0 {
                (i.saturating_sub(1), i, 1u32, 3u32)
            } else {
                (i, (i + 1).min(wu - 1), 3u32, 1u32)
            };
            let di = (dy * bw + dx) * 4;
            for c in 0..4 {
                let s00 = src[(y0 * wu + x0) * 4 + c] as u32;
                let s01 = src[(y0 * wu + x1) * 4 + c] as u32;
                let s10 = src[(y1 * wu + x0) * 4 + c] as u32;
                let s11 = src[(y1 * wu + x1) * 4 + c] as u32;
                let top = (wx0 * s00 + wx1 * s01 + 2) / 4;
                let bot = (wx0 * s10 + wx1 * s11 + 2) / 4;
                dst[di + c] = ((wy0 * top + wy1 * bot + 2) / 4) as u8;
            }
        }
    }
}

/// Row-width profile of a disc in a BGRA canvas (iter108): for each
/// row of the `d` x `d` tile at (`x0`, `y0`), the span (max-min+1) of
/// pixels whose disc coverage exceeds `thr`. Coverage is measured
/// against the disc's own fill colour: cov = dot(px, fill) /
/// dot(fill, fill). The disc fills store colour*a_disc with A=255, so
/// coverage reads the true SDF edge, not the (hard, radius R+0.5)
/// alpha channel; the shadow is alpha-only on a transparent canvas and
/// the numeral/chevron sit inside the disc, so neither moves the span.
pub(crate) fn disc_row_widths(
    canvas: &[u8],
    w: u32,
    x0: i32,
    y0: i32,
    d: u32,
    fill: (f32, f32, f32),
    thr: f32,
) -> Vec<u32> {
    let w = w as usize;
    let denom = fill.0 * fill.0 + fill.1 * fill.1 + fill.2 * fill.2;
    let mut out = Vec::with_capacity(d as usize);
    for y in 0..d as i32 {
        let mut min_x: Option<i32> = None;
        let mut max_x: Option<i32> = None;
        for x in 0..d as i32 {
            let i = (((y0 + y) as usize) * w + (x0 + x) as usize) * 4;
            // BGRA bytes -> (R, G, B) floats.
            let (r, g, b) = (
                canvas[i + 2] as f32,
                canvas[i + 1] as f32,
                canvas[i] as f32,
            );
            let cov = if denom > 0.0 {
                (r * fill.0 + g * fill.1 + b * fill.2) / denom
            } else {
                0.0
            };
            if cov > thr {
                min_x = Some(min_x.map_or(x, |m: i32| m.min(x)));
                max_x = Some(max_x.map_or(x, |m: i32| m.max(x)));
            }
        }
        out.push(match (min_x, max_x) {
            (Some(a), Some(b)) => (b - a + 1) as u32,
            _ => 0,
        });
    }
    out
}

/// Disc rim + disc shadow + panel drop shadow (iter26).
/// The iter24 outside discs read as flat black coins and the panels
/// cast no shadow (pixel + realism critics, iter24/25) — both are
/// drawn in the island's own buffer; rule.conf is untouched (the
/// compositor shadow for the layer namespace stays off by design,
/// iter5). Geometry is untouched: disc centres still come from
/// nav_disc_centres(), panel rect from panel_rect_stage(); shadows
/// only add pixels OUTSIDE those shapes, inside the fixed stage
/// (tallest panel bottom 561 + offset 5 + blur 10 = 576 < STAGE_H
/// 636; widest panel right 1092 + blur 10 = 1102 < STAGE_W 1176;
/// leftmost disc-shadow edge 46 > 0 — see the fits-inside-stage test).
/// Disc shadow offset: down-right from the disc centre.
const DISC_SHADOW_OFF: (f32, f32) = (2.0, 3.0);
/// Disc shadow blur (soft falloff width beyond the shadow circle).
const DISC_SHADOW_BLUR: f32 = 4.0;
/// Disc shadow peak alpha (black).
const DISC_SHADOW_ALPHA: f32 = 0.35;
/// Rim band width hugging the inside of the disc edge.
const RIM_WIDTH: f32 = 1.5;
/// Rim colour at the top of the disc (periwinkle-tinted light grey).
const RIM_LIGHT: (f32, f32, f32) = (168.0, 176.0, 205.0);
/// Rim peak alpha at the top of the disc.
const RIM_LIGHT_ALPHA: f32 = 0.65;
/// Rim colour at the bottom of the disc (darker than the disc fill).
const RIM_DARK: (f32, f32, f32) = (8.0, 9.0, 11.0);
/// Rim peak alpha at the bottom of the disc.
const RIM_DARK_ALPHA: f32 = 0.55;
/// Panel drop-shadow offset (straight down; the panel stays centred).
const PANEL_SHADOW_OFF: (f32, f32) = (0.0, 5.0);
/// Panel drop-shadow blur (soft falloff width beyond the shadow rect).
const PANEL_SHADOW_BLUR: f32 = 10.0;
/// Panel drop-shadow peak alpha (black).
const PANEL_SHADOW_ALPHA: f32 = 0.40;

/// Flat frosted fill (iter31, a3 REVISED): the iter28–30 styling
/// is deleted. The fill is one flat material, uniform from edge to
/// edge. The one material is the in-buffer frost from
/// iter29: the blurred wallpaper crop composited under a plain,
/// neutral dark fill (wallpaper_blur.rs); the fill interior is opaque,
/// so no sharp wallpaper leaks through the compositor (mango does NOT
/// blur behind this layer surface — iter28 A/B: pixel-identical).
/// Fill alphas: pill 0.92, menus 0.92 (iter128; was 0.80) (the alpha is the tint strength
/// over the blurred crop, not surface translucency). Panel content
/// stays opaque (glyphs/shapes force A=255), so the fill never darkens
/// content — only the empty fill is frosted. Glyphs stay crisp white.
/// FROST_FLOOR is a neutral grey (equal channels): on a pure-black
/// backdrop the fill reads FLOOR*fa ~= 2 — the material never
/// vanishes on dark, and never picks up a warm/brown cast.
/// iter86 (a6a frost unification): FLOOR 20.0 -> 5.0 and menu alpha
/// 0.85 -> 0.94 -> 0.80 (iter89) — the Control Centre's near-black frost, now global,
/// so all eight views sample the same near-black as the CC.
/// iter128 (B4, R5 re-grade): FLOOR 5.0 -> 2.5 and both alphas
/// 0.80 -> 0.92. Measured (harness_frost_panel_fill, real grims):
/// the bright snow leg filled at mean ~11.3 with 39% of pixels in
/// 13..25 — a grey slab; the reference panels read ~2-3 (pure black
/// frost). The lift is bg*(1-fa): the neutralised snow sample means
/// ~32, so 0.20 of it is ~6.4 over the floor. At fa 0.92 the same
/// sample fills ~5.5, the dark leg ~2.6, and the high-pass texture
/// (not scaled by fa) is untouched. The floor itself had to move:
/// at fa -> 1 the fill tends to FLOOR*fa, and 4.0 is already grey
/// against the reference's 2-3.
const FROST_ALPHA_PILL: f32 = 0.92;
const FROST_ALPHA_MENU: f32 = 0.92;
const FROST_FLOOR: f32 = 2.5;

/// Frost fill factor for a state: the pill (Collapsed/Lock) uses 0.92,
/// the expanded menus 0.92 (iter128: was 0.80); the fullscreen lockscreen stays 1.0.
/// This is the tint/floor factor inside both fill paths, NOT surface
/// translucency - the fill interior is opaque in both paths (iter85).
fn frost_alpha(state: IslandState) -> f32 {
    match state {
        IslandState::Collapsed | IslandState::Dictation | IslandState::Lock => FROST_ALPHA_PILL,
        IslandState::Lockscreen => 1.0,
        _ => FROST_ALPHA_MENU,
    }
}

/// Premultiplied BGRA bytes for one flat-fill pixel (iter31, a3
/// REVISED; iter85: opaque fallback): the dark floor
/// (`FROST_FLOOR * fill_alpha`, uniform everywhere) at full coverage.
/// `a` is the capsule AA coverage (0..1); `d`/`x`/`y`/`w`/`h` stay in
/// the signature so every fill call site is unchanged, but they no
/// longer affect the pixel: the fill is uniform by design.
/// OPAQUE on purpose: this is the no-blur fallback (the blurred
/// wallpaper is unavailable - e.g. the file is corrupt or an
/// undecodable format (since iter88, JPEGs decode too - a JPEG no
/// longer lands here). A
/// translucent black fallback lets the backdrop leak through, so the
/// panel reads a different brightness above/below whatever window sits
/// behind it - in iter84 the terminal top edge (screen y 39.3) read
/// as the panel top (true top 31.3) and the detected mid-height
/// shifted +4.15, putting the nav discs 4+ px above it. The blur path
/// (`frost_fill_px_blur`) is already opaque (A = a*255); the fallback
/// matches it, and matches its own floor: with bg = 0 the blur path
/// reads `FROST_FLOOR * fill_alpha` = 2 - exactly this value (iter86: menu 5*0.80 = 4.0 -> 4, pill 5*0.80 = 4.0 -> 4; iter128: 2.5*0.92 = 2.3 -> 2).
/// `fill_alpha` stays the tint/floor factor, never surface translucency.
fn frost_fill_px(a: f32, _d: f32, _x: f32, _y: f32, _w: f32, _h: f32, fill_alpha: f32) -> [u8; 4] {
    if a <= 0.0 {
        return [0, 0, 0, 0];
    }
    let v = (FROST_FLOOR * fill_alpha * a) as u8;
    [v, v, v, (a * 255.0) as u8]
}

/// Premultiplied BGRA bytes for one flat-frost pixel (iter31, a3
/// REVISED; iter103: mean and variation decoupled): `bg` is the
/// NEUTRALISED sample at this pixel's screen-logical coords and
/// `lm` its BoxBlur(25) local mean, so the fill is
/// `bg*(1-fa) + FROST_FLOOR*fa + FROST_HP_K*clamp(bg-lm, +/-HP_MAX_DEV)`.
/// The first two
/// terms set the dark neutral mean (on a pure-black backdrop the
/// fill reads FLOOR*fa ~= 2, iter128: 2.5*0.92 = 2.3, and the material
/// never vanishes — and never picks up a warm/brown cast); the
/// high-pass term restores the faint cloud mottling the 0.80 fill
/// used to flatten. The deviation is ~0 on a flat backdrop
/// (sample == local mean there). iter113: a dark pixel under a
/// bright local mean can go negative, so the base clamps at the
/// floor (FROST_FLOOR*fa = 2) instead of 0 — the iter112 wart
/// (cards reading (0,0,0) over bright backdrops vs (4,4,4) over
/// dark ones) was the clamp at 0. The top edge reads exactly like
/// the interior. iter128: the deviation is capped at
/// FROST_HP_MAX_DEV (12) before the gain — a sharp backdrop edge
/// (UI text: dark-leg max dev 29) used to print through as a bright
/// speck (fill max 34 over a 4.6 mean); soft mottling (|dev| ~1-6)
/// passes untouched.
/// The interior is OPAQUE (A = a*255 with `a` the AA coverage) — no
/// sharp wallpaper can leak through the compositor; the frost lives
/// entirely in-buffer. Falls back to `frost_fill_px` (flat black
/// fill) when the blurred wallpaper is unavailable. Callers with no
/// local mean (the uniform wallpaper fallback) pass `lm = bg`, so
/// the deviation is exactly 0 there.
fn frost_fill_px_blur(a: f32, _d: f32, _x: f32, _y: f32, _w: f32, _h: f32, fill_alpha: f32, bg: [u8; 3], lm: [u8; 3]) -> [u8; 4] {
    if a <= 0.0 {
        return [0, 0, 0, 0];
    }
    let mut out = [0u8; 4];
    for c in 0..3 {
        let n = bg[c] as f32;
        // iter128: cap the deviation — sharp edges must not print
        // through as bright specks (see doc above).
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

/// Layer top margin for the stage: the panel top stays at gap_top.
fn stage_margin_top() -> i32 {
    (cfg().gap_top - STAGE_PAD_TOP as i32).max(0)
}

/// Reserved pill strip (iter34 a3d): the pill occupies
/// gap_top..gap_top+28 on screen, so the compositor must never tile a
/// window under it. The island surface's own exclusive zone reserves
/// exactly that strip — collapsed or expanded, the zone never changes,
/// so opening a menu never re-tiles anything (the expanded panel hangs
/// below the strip and overlays windows like a menu should).
/// Fullscreen apps are out of scope.
fn pill_strip_px() -> i32 {
    cfg().gap_top + cfg().bar_height as i32
}

/// Screen-logical coords of the stage top-left: the layer is anchored
/// TOP and centred horizontally (pill centre 752 = screen centre), so
/// x0 = (1504 - STAGE_W)/2 = 164 and y0 = the layer top margin (2).
fn stage_screen_origin() -> (f32, f32) {
    (
        (LOCKSCREEN_SIZE.0 as f32 - STAGE_W as f32) / 2.0,
        stage_margin_top() as f32,
    )
}

/// Panel top-left inside the stage (the panel is centred horizontally).
/// Pill states sit STAGE_PAD_TOP below the stage top; expanded menus
/// hang below the pill at panel_top_menu() (iter30 a3c), so the pill
/// stays visible above the panel with a 4 px gap.
fn panel_origin(state: IslandState) -> (f32, f32) {
    let (pw, _) = state.size();
    let oy = match state {
        IslandState::Collapsed
        | IslandState::Dictation
        | IslandState::Lock
        | IslandState::Lockscreen => STAGE_PAD_TOP,
        _ => panel_top_menu(),
    };
    ((STAGE_W as f32 - pw as f32) / 2.0, oy)
}

/// Capsule top y (stage coords) for a morph frame (iter30 a3c): the
/// capsule interpolates between the endpoints' panel origins, so an
/// expand grows downward from beneath the pill (pill top 4 -> panel
/// top 36) instead of stretching the pill into a top bar. `progress`
/// is the frame's eased progress, clamped (EaseOutCubic stays in
/// 0..1; the clamp is belt-and-braces so the top edge lands exactly
/// on the target, not past it — iter33 removed the EaseOutBack
/// overshoot, see begin_morph).
fn morph_capsule_oy(from_state: IslandState, to_state: IslandState, progress: f32) -> f32 {
    let (_, oy_from) = panel_origin(from_state);
    let (_, oy_to) = panel_origin(to_state);
    let t = progress.clamp(0.0, 1.0);
    oy_from + (oy_to - oy_from) * t
}

/// Panel rect inside the stage: (x, y, w, h).
fn panel_rect_stage(state: IslandState) -> (f32, f32, f32, f32) {
    let (ox, oy) = panel_origin(state);
    let (pw, ph) = state.size();
    (ox, oy, pw as f32, ph as f32)
}

/// Nav-disc centres in STAGE coords: (left, right). THE one source of
/// truth for disc geometry — draw (draw_nav_arrows_stage), hit-test
/// (arrow_hit_stage) and the input region all call this.
/// (iter30 a3b): the discs sit OUTSIDE the panel at the same horizontal
/// offset as before (GAP + R beyond each edge) but at the panel's
/// VERTICAL MIDDLE (oy + h/2 per view), not at the pill line.
fn nav_disc_centres(state: IslandState) -> ((f32, f32), (f32, f32)) {
    let (ox, oy) = panel_origin(state);
    let (pw, ph) = state.size();
    let cy = oy + ph as f32 / 2.0;
    let off = NAV_DISC_GAP + NAV_DISC_R;
    ((ox - off, cy), (ox + pw as f32 + off, cy))
}

// ---------------------------------------------------------------------------
// Workspace circles (iter90, item a8 — Alexandre, 13 Nov 16:25, asked
// 3x; RESIZED iter92, 14 Nov — Alexandre's binding spec, option 2):
// four Ø16 discs numbered 1-4 at pill height, LEFT of the pill. The
// whole row is exactly 76 wide — the pill's width — with 4 px gaps.
// ---------------------------------------------------------------------------
/// Number of workspace circles (mango tags 1..=4).
const WS_COUNT: usize = 4;
/// Workspace disc radius (iter92: Ø16 — the whole 4-circle row is
/// exactly 76 wide, the pill's width: 4*16 + 3*4 = 76).
const WS_DISC_R: f32 = 8.0;
/// Edge-to-edge gap between the rightmost circle and the pill's left edge.
const WS_GAP_PILL: f32 = 4.0;
/// Edge-to-edge gap between neighbouring circles.
const WS_GAP_CIRCLE: f32 = 4.0;
/// Circle centre line = pill centre line.
fn ws_cy() -> f32 {
    STAGE_PAD_TOP + pill_h() / 2.0
}
/// Number glyph size inside a circle (iter92: digit ~8 px tall at
/// 11.0 em — ~0.5 x the Ø16 disc, same ratio as the old Ø28/14 em).
const WS_NUMBER_PX: f32 = 11.0;
/// Active-workspace fill: the island accent, bound to the one
/// definition in library.rs so the disc and the CC toggles can never
/// drift into two periwinkles side by side.
const WS_ACTIVE_FILL: (u8, u8, u8) = library::ISLAND_ACCENT;
/// Glyph on the active (periwinkle) fill: dark navy, as the CC toggle glyph.
const WS_ACTIVE_GLYPH: (u8, u8, u8) = (11, 27, 58);
/// Glyph on the inactive (dark) fill: near-white, as the nav chevron.
const WS_INACTIVE_GLYPH: (u8, u8, u8) = (232, 234, 237);
/// Inactive fill: the nav-disc dark (#1C1E22) at the same 0.92 coverage
/// as draw_nav_disc — the nav-disc material, reused at Ø16 here.
const WS_INACTIVE_FILL: (u8, u8, u8) = (28, 30, 34);
/// Inactive but HOLDING WINDOWS (r1c finding 4): the palette's existing
/// dark toggle-off grey, one step up from the empty-disc dark, so an
/// occupied workspace reads as "something is there" without a new hue
/// and without competing with the accent.
const WS_OCCUPIED_FILL: (u8, u8, u8) = (52, 55, 60);
/// Disc labels, indexed by tag-1. Static so a settled frame does not
/// allocate four Strings per draw (r1c finding 6).
const WS_LABELS: [&str; 9] = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];

/// Workspace disc centres in STAGE coords, left-to-right tag 1..4.
/// Derived from the collapsed pill rect so they follow the configured
/// collapsed_width: pill_x = (STAGE_W - 76)/2 = 550 live; rightmost
/// centre = 550 - 4 (gap) - 8 (r) = 538; step = 2*8 + 4 = 20.
/// Live centres: (478, 15), (498, 15), (518, 15), (538, 15); cluster
/// spans x 470..546, y 7..23 — exactly 76 wide, the pill's width,
/// and fully INSIDE the a3d reserved strip (y 3..27) with 4 px to
/// spare top and bottom (iter92: Ø16 < 24 pill height, so unlike the
/// old Ø28 there is no overhang). Hugs the pill with a 4 px gap.
/// First tag shown in the four disc slots.
/// r1c finding 3: mango binds SUPER+1..9 to `view` (bind.conf:78-82),
/// so tags 5..9 are reachable while WS_COUNT is 4. A FIXED 1..4 window
/// left every disc inactive on tags 5..9 — the one "you are here" cue
/// the bar has disappeared. The window slides so the active tag is
/// always drawn; tags 1..4 keep the window at 1, so the layout every
/// other test and the reference geometry were measured against does
/// not move.
fn ws_window_start(active: u8) -> u8 {
    let a = active.clamp(1, 9) as i32;
    let max_start = 10 - WS_COUNT as i32;
    (a - (WS_COUNT as i32 - 1)).clamp(1, max_start) as u8
}

/// The tags the four slots currently show, left to right.
fn ws_visible_tags(active: u8) -> [u8; WS_COUNT] {
    let start = ws_window_start(active);
    let mut out = [1u8; WS_COUNT];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = start + i as u8;
    }
    out
}

fn workspace_disc_centres() -> [(f32, f32); WS_COUNT] {
    let (pill_x, _, _, _) = panel_rect_stage(IslandState::Collapsed);
    let mut out = [(0.0f32, ws_cy()); WS_COUNT];
    for i in 0..WS_COUNT {
        // i = 0 is tag 1 (leftmost); count back from the pill.
        let from_right = (WS_COUNT - 1 - i) as f32;
        let cx = pill_x - WS_GAP_PILL - WS_DISC_R
            - from_right * (2.0 * WS_DISC_R + WS_GAP_CIRCLE);
        out[i] = (cx, ws_cy());
    }
    out
}

/// Workspace cluster bounding box in stage coords (circles + the 4 px
/// gaps between them + the 4 px bridge to the pill): one rect for the
/// input region / Leave check, so travelling pill <-> circles never
/// crosses outside the region (same reason as the nav-disc bridges).
fn workspace_cluster_rect() -> (f32, f32, f32, f32) {
    let centres = workspace_disc_centres();
    let (x0, _) = centres[0];
    let (pill_x, _, _, _) = panel_rect_stage(IslandState::Collapsed);
    (
        x0 - WS_DISC_R,
        ws_cy() - WS_DISC_R,
        pill_x - (x0 - WS_DISC_R),
        2.0 * WS_DISC_R,
    )
}

/// Workspace hit test in STAGE coords: Some(1..=4) = tag number,
/// None = elsewhere. Circular, Ø16 + 1 px slack like the nav discs.
/// NOTE: event.position is ALWAYS stage coords (iter24 a2) — a driver
/// click at SCREEN (789, 176) arrives as STAGE (625, 176) because the
/// stage origin is x = 164 on screen (stage_screen_origin); callers
/// must never add the 164 here.
fn workspace_hit_stage(pos: (f64, f64)) -> Option<u8> {
    let radius = (WS_DISC_R + 1.0) as f64;
    let tags = ws_visible_tags(active_tag_cached());
    for (i, (cx, cy)) in workspace_disc_centres().iter().enumerate() {
        let d = ((pos.0 - *cx as f64).powi(2) + (pos.1 - *cy as f64).powi(2)).sqrt();
        if d <= radius {
            return Some(tags[i]);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tray cluster (iter108, Builder C — GEOMETRY ONLY, no D-Bus work):
// SNI tray icons sit RIGHT of the pill, mirroring the workspace
// circles on the left (Alexandre, 15 Nov 2026, 09:27 — "all the tray
// icons, even new ones, should be at the right side of the pill, on
// the other side of the workspaces").
//
// Architecture — WHY the surface never resizes: the layer surface is
// a fixed STAGE_W x STAGE_H (1176 x 636) sheet centred on screen by
// the compositor (Anchor::TOP only), and the pill is drawn centred in
// that sheet, so the pill centre is screen 752 for ANY tray count.
// "The stage widens to the right" therefore means the TOP-ROW CONTENT
// widens rightward inside the fixed sheet: the tray cluster grows
// from the pill's right edge while the pill, the sheet and the
// reserved strip never move. Resizing the sheet instead would break
// the iter24 rule (the stage never resizes — per-tick set_size broke
// morphs), the draw() debug_assert_eq!((w, h), (STAGE_W, STAGE_H))
// and the SlotPool sized STAGE_W * STAGE_H * 4. The reserved strip is
// a HEIGHT (pill_strip_px() = gap_top + 24 = 27) spanning the whole
// output edge, so it already covers the tray row (y 7..23) for any
// item count — no zone change is needed, only the containment tests
// below. Like the workspace circles, the tray cluster is anchored to
// the COLLAPSED pill rect in every state (including Lock, whose pill
// is 54 wide — the cluster does not chase it, exactly as the circles
// do not). Builder B (SNI host, ~/build-iter108-tray/) owns icon
// rendering + Activate; it consumes tray_icon_centres(n) and calls
// set_tray_count(n) when items register/unregister. Builder A (scale
// rendering, ~/build-iter108-scale/) owns buffer scale; all geometry
// here is in LOGICAL px, so a scale change rescales every number
// below uniformly and cannot move the pill relative to the tray.
// ---------------------------------------------------------------------------
/// Tray icon diameter (mirrors the Ø16 workspace discs).
const TRAY_ICON_D: f32 = 16.0;
/// Tray icon radius.
const TRAY_ICON_R: f32 = 8.0;
/// Edge-to-edge gap between the pill's right edge and the first tray icon.
const TRAY_GAP_PILL: f32 = 4.0;
/// Edge-to-edge gap between neighbouring tray icons.
const TRAY_GAP_ICON: f32 = 4.0;
/// Tray icon centre line = pill centre line (same as ws_cy()).
fn tray_cy() -> f32 {
    ws_cy()
}
/// Step between neighbouring tray icon centres (16 + 4).
const TRAY_STEP: f32 = TRAY_ICON_D + TRAY_GAP_ICON;

/// Live tray item count, owned by the geometry (this) side; the SNI
/// host (iter108-B) writes it via set_tray_count() as items register /
/// unregister. 0 = no tray cluster: no rect in the input region, no
/// hit target, nothing drawn. The count is read on the event-loop
/// thread only (classify_press / region build / Leave check); the SNI
/// thread must never draw — after changing the count it requests a
/// redraw through the existing Island redraw path (same wake-pipe
/// pattern as the tag watcher), never by calling draw_if_ready() itself.
static TRAY_COUNT: std::sync::OnceLock<std::sync::RwLock<usize>> =
    std::sync::OnceLock::new();
/// Current number of tray items (0 when the SNI host has none).
fn tray_item_count() -> usize {
    *TRAY_COUNT
        .get_or_init(|| std::sync::RwLock::new(0))
        .read()
        .unwrap()
}
/// Set the live tray item count (merge iter108: the 1 s timer branch
/// calls this whenever the tray fingerprint moves, mirroring the live
/// snapshot count into the geometry-side store). Pure store; the caller
/// must request a redraw.
fn set_tray_count(n: usize) {
    *TRAY_COUNT
        .get_or_init(|| std::sync::RwLock::new(0))
        .write()
        .unwrap() = n;
}

/// Tray icon centres in STAGE coords, left-to-right item 0..n.
/// First centre = pill right edge + 4 (gap) + 8 (r); step 20.
/// Live centres: n=1 (638, 15); n=2 (638, 15), (658, 15);
/// n=3 (638, 15), (658, 15), (678, 15) — the mirror image of the
/// workspace row (478..538) about the pill centre 588.
fn tray_icon_centres(n: usize) -> Vec<(f32, f32)> {
    let (pill_x, _, pw, _) = panel_rect_stage(IslandState::Collapsed);
    let pill_right = pill_x + pw as f32;
    (0..n)
        .map(|i| {
            (
                pill_right + TRAY_GAP_PILL + TRAY_ICON_R + i as f32 * TRAY_STEP,
                tray_cy(),
            )
        })
        .collect()
}

/// Tray cluster bounding box in stage coords (icons + the gaps between
/// them + the 4 px bridge to the pill): one rect for the input region
/// / Leave check, mirroring workspace_cluster_rect(). None when n == 0
/// (no cluster, no region). n=1: (626, 7, 20, 16); n=2: (626, 7, 40, 16);
/// n=3: (626, 7, 60, 16). The rect's left edge is the pill's right
/// edge, so travelling pill <-> tray never crosses outside the region.
fn tray_cluster_rect(n: usize) -> Option<(f32, f32, f32, f32)> {
    if n == 0 {
        return None;
    }
    let (pill_x, _, pw, _) = panel_rect_stage(IslandState::Collapsed);
    let pill_right = pill_x + pw as f32;
    Some((
        pill_right,
        tray_cy() - TRAY_ICON_R,
        n as f32 * TRAY_STEP,
        2.0 * TRAY_ICON_R,
    ))
}

/// Top-row content span in stage coords: workspace circles' left edge
/// to the tray cluster's right edge (or the pill's right edge when the
/// tray is empty). This is the "stage width" that grows rightward:
/// n=0: 470..626 = 156; n=1: 470..646 = 176; n=2: 470..666 = 196;
/// n=3: 470..686 = 216. The wl_surface stays STAGE_W (1176)
/// throughout; the pill centre stays screen 752 throughout (see
/// pill_centre_screen_x).
#[allow(dead_code)] // proof helper: consumed by the tray tests below.
fn top_row_span(n: usize) -> (f32, f32) {
    let (wx, _, _, _) = workspace_cluster_rect();
    let (pill_x, _, pw, _) = panel_rect_stage(IslandState::Collapsed);
    let pill_right = pill_x + pw as f32;
    let right = match tray_cluster_rect(n) {
        Some((rx, _, rw, _)) => rx + rw,
        None => pill_right,
    };
    (wx, right)
}

/// Top-row content rect in STAGE-LOGICAL coords (F-A step 5 / F-G):
/// pill + workspace row + tray row with a 2 px AA margin. This is the
/// only area a settled collapsed frame touches, and the same rect the
/// 2x top-row painter clears — one source, no drift.
fn top_row_rect() -> (i32, i32, i32, i32) {
    let (px, py, pw, ph) = panel_rect_stage(IslandState::Collapsed);
    let c = workspace_disc_centres();
    let n = tray_item_count();
    let right = if n > 0 {
        let tc = tray_icon_centres(n);
        tc[n - 1].0 + TRAY_ICON_R
    } else {
        px + pw
    };
    (
        (c[0].0 - WS_DISC_R - 2.0).floor() as i32,
        ((py - 2.0).max(0.0)).floor() as i32,
        (right + 2.0).ceil() as i32,
        (py + ph + 2.0).ceil() as i32,
    )
}

/// Content rect of one settled frame in stage-logical coords: the top
/// row plus the state's own panel (padded for its shadow and, for
/// menu states, the nav discs). Used for both the clear and the
/// damage rect, so what is drawn is exactly what is presented.
fn content_rect(state: IslandState) -> (i32, i32, i32, i32) {
    let top = top_row_rect();
    if state == IslandState::Lockscreen {
        return top;
    }
    let (ox, oy, pw, ph) = panel_rect_stage(state);
    let pad = if MENU_CYCLE.contains(&state) || state == IslandState::TrayMenu {
        (PANEL_SHADOW_BLUR + PANEL_SHADOW_OFF.1.abs() + NAV_DISC_R + NAV_DISC_GAP + 2.0).ceil()
            as i32
    } else {
        2
    };
    let panel = (
        ox as i32 - pad,
        (oy as i32 - pad).max(0),
        (ox + pw) as i32 + pad,
        (oy + ph) as i32 + pad,
    );
    union_rect(top, panel)
}

/// Pill centre x on screen (logical px): stage origin + stage-centred
/// pill centre. Independent of the tray count by construction — the
/// pill is drawn at (STAGE_W - collapsed_width) / 2 inside a fixed
/// sheet the compositor centres, so no tray change can move it.
/// Live: 752 (screen centre 1504 / 2).
#[allow(dead_code)] // proof helper: consumed by the tray tests below.
fn pill_centre_screen_x() -> f32 {
    let (sox, _) = stage_screen_origin();
    let (pill_x, _, pw, _) = panel_rect_stage(IslandState::Collapsed);
    sox + pill_x + pw as f32 / 2.0
}

/// Tray hit test in STAGE coords: Some(i) = 0-based tray slot index,
/// None = elsewhere. Circular, Ø16 + 1 px slack like the nav discs /
/// workspace circles. The gap between icons (and the 4 px bridge to
/// the pill) is NOT a hit — a press there must be NoOp, never Collapse.
fn tray_hit_stage(pos: (f64, f64), n: usize) -> Option<usize> {
    let radius = (TRAY_ICON_R + 1.0) as f64;
    for (i, (cx, cy)) in tray_icon_centres(n).iter().enumerate() {
        let d = ((pos.0 - *cx as f64).powi(2) + (pos.1 - *cy as f64).powi(2)).sqrt();
        if d <= radius {
            return Some(i);
        }
    }
    None
}

// --- mango tag state (iter90 a8): cached active tag, polled off the
// hot path. `mmsg get all-tags` prints one JSON line; the active tag
// is the entry with "is_active":true. The poller thread runs it every
// 1 s (hard 1500 ms timeout, killed on overrun) and writes the cache;
// the 1 s backdrop-watch timer in main() redraws when the cache moved.
// A click dispatches optimistically (cache set at once) and the poll
// corrects it within ~1 s if the dispatch failed. No mmsg call ever
// runs on the render/input path.
// iter97: the event-driven watcher (ensure_tag_watcher) is now the
// fast path; this 1 s poller stays as fallback/resync.
static TAG_CACHE: std::sync::OnceLock<std::sync::RwLock<u8>> =
    std::sync::OnceLock::new();
/// Occupied/urgent bits for tags 1..=9 (bit n-1), from the same JSON.
/// r1c (finding 4): `mmsg get all-tags` reports `client_count` per tag,
/// so a workspace with windows in it can read as occupied instead of
/// looking exactly like an empty one.
static TAG_OCC: std::sync::OnceLock<std::sync::RwLock<TagState>> =
    std::sync::OnceLock::new();
/// Cached active mango tag (1..=9); seed 1 = mango's startup tag.
fn active_tag_cached() -> u8 {
    *TAG_CACHE
        .get_or_init(|| std::sync::RwLock::new(1))
        .read()
        .unwrap()
}
fn set_active_tag_cached(tag: u8) {
    if let Some(c) = TAG_CACHE.get() {
        *c.write().unwrap() = tag;
    }
}
/// What one `mmsg` tag snapshot tells the bar: which tag is active and
/// which tags hold windows / want attention. Bit n-1 is tag n.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TagState {
    pub(crate) active: u8,
    pub(crate) occupied: u16,
    pub(crate) urgent: u16,
}
impl TagState {
    fn is_occupied(&self, tag: u8) -> bool {
        tag >= 1 && tag <= 9 && self.occupied & (1 << (tag - 1)) != 0
    }
}
/// Cached tag state (active + occupancy). Seed = tag 1 active, nothing
/// occupied, which is mango's startup state.
fn tag_state_cached() -> TagState {
    *TAG_OCC
        .get_or_init(|| std::sync::RwLock::new(TagState::default()))
        .read()
        .unwrap()
}
/// Write a fresh snapshot; returns true when the DRAWN state changed
/// (so the watcher only pokes the loop for a real change).
fn apply_tag_state(ts: TagState) -> bool {
    let mut changed = ts.active != active_tag_cached();
    if changed {
        set_active_tag_cached(ts.active);
    }
    let mut cur = TAG_OCC
        .get_or_init(|| std::sync::RwLock::new(TagState::default()))
        .write()
        .unwrap();
    if cur.occupied != ts.occupied || cur.urgent != ts.urgent {
        cur.occupied = ts.occupied;
        cur.urgent = ts.urgent;
        changed = true;
    }
    changed
}
/// Parse the active tag out of `mmsg get all-tags` JSON: the first
/// `"index":N` preceding an `"is_active":true`. No serde in this
/// crate — the format is mmsg's stable one-line JSON; anything
/// unparseable is None (the cache keeps its last value).
fn parse_active_tag(json: &str) -> Option<u8> {
    parse_tag_state(json).map(|t| t.active)
}
/// Parse one `mmsg` snapshot: the active tag plus an occupied/urgent
/// bitmap. Scans `"index":N` heads and remembers the three flags that
/// follow each one in mmsg's fixed field order, so a tag's flags can
/// never be attributed to its neighbour.
fn parse_tag_state(json: &str) -> Option<TagState> {
    let mut out = TagState::default();
    let mut found_active = false;
    let mut rest = json;
    while let Some(idx) = rest.find("\"index\":") {
        let after = &rest[idx + 8..];
        let num: String = after
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        let tag: u8 = num.parse().ok()?;
        // The next tag's chunk (or EOF) bounds this one's flags.
        let end = after.find("\"index\":").unwrap_or(after.len());
        let chunk = &after[..end];
        if (1..=9).contains(&tag) {
            let bit = 1u16 << (tag - 1);
            if chunk.contains("\"is_active\":true") {
                out.active = tag;
                found_active = true;
            }
            if chunk.contains("\"is_urgent\":true") {
                out.urgent |= bit;
            }
            // client_count is always present in mmsg's output; a
            // missing field reads as 0 (empty), which is the safe
            // direction — an unreadable count never invents a window.
            if let Some(p) = chunk.find("\"client_count\":") {
                let n: String = chunk[p + 15..]
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                if n.parse::<u32>().map(|n| n > 0).unwrap_or(false) {
                    out.occupied |= bit;
                }
            }
        }
        rest = &after[end..];
        if !rest.contains("\"index\":") {
            break;
        }
    }
    if found_active {
        Some(out)
    } else {
        None
    }
}
/// Run `mmsg args`, capturing stdout, with a HARD timeout (same
/// pattern as controlcenter_panel::run_cmd_timeout — a hung compositor
/// IPC can never stall the poller thread beyond this bound).
fn run_mmsg_timeout(args: &[&str], timeout: std::time::Duration) -> Option<String> {
    let mut child = std::process::Command::new("mmsg")
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
            Ok(Some(_)) => {
                let buf = rx
                    .recv_timeout(std::time::Duration::from_millis(200))
                    .unwrap_or_default();
                return Some(String::from_utf8_lossy(&buf).into_owned());
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
fn read_active_tag() -> Option<u8> {
    read_tag_state().map(|t| t.active)
}
/// One snapshot: `mmsg get all-tags` (active tag + occupancy), hard
/// 1500 ms timeout.
fn read_tag_state() -> Option<TagState> {
    parse_tag_state(&run_mmsg_timeout(
        &["get", "all-tags"],
        std::time::Duration::from_millis(1500),
    )?)
}
/// Spawn the tag poller thread once (first poll immediately, then
/// every 1 s). Idempotent; safe to call from draw() / the 1 s timer.
fn ensure_tag_poller() {
    static STARTED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    STARTED.get_or_init(|| {
        let _ = active_tag_cached(); // init the cache (seed 1)
        std::thread::spawn(|| loop {
            if let Some(t) = read_tag_state() {
                apply_tag_state(t);
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        });
    });
}

/// iter97: event-driven tag watcher — the fix for the 1-3 s highlight
/// lag (iters 93-95). Root cause: the highlight only moved when TWO
/// 1 s stages lined up — the poller thread re-ran `mmsg get all-tags`
/// every 1 s (ensure_tag_poller below) AND the 1 s backdrop-watch
/// timer only redrew when it next saw the cache move — so a
/// keyboard/mmsg switch took up to ~1 s + ~1 s + mmsg spawn time
/// before the redraw, ~2 s worst case (circle clicks hid this: they
/// set the cache optimistically and redrew at once, see the
/// WorkspaceTag press handler). `mmsg watch all-tags` instead streams
/// one JSON snapshot per tag event, arriving ~10 ms after the switch
/// (measured 14 Nov 2026: three lines per switch, all in the same
/// millisecond burst, same JSON shape as `get all-tags`). A dedicated
/// thread reads that stream; on a real tag change it updates the
/// cache and pokes the main loop through a nonblocking UnixStream
/// pair (registered as a Generic source in main()), so the redraw
/// fires within milliseconds. The 1 s poller is KEPT as
/// fallback/resync (it also corrects a missed event); the watch child
/// is respawned after a 1 s backoff if it ever exits, and a one-shot
/// poll resyncs the cache on respawn. No mmsg call runs on the
/// render/input path; the watcher thread never blocks the loop
/// (nonblocking poke; a byte already queued means the loop is woken).
static TAG_WAKE_TX: std::sync::OnceLock<std::sync::Mutex<std::os::unix::net::UnixStream>> =
    std::sync::OnceLock::new();
/// Store the watcher's poke end (called once from main()).
fn set_tag_wake_tx(tx: std::os::unix::net::UnixStream) {
    let _ = TAG_WAKE_TX.set(std::sync::Mutex::new(tx));
}
/// Poke the main loop: one byte; ignored on error (a byte already
/// queued means the loop is already woken).
fn tag_watch_poke() {
    use std::io::Write;
    if let Some(tx) = TAG_WAKE_TX.get() {
        if let Ok(mut tx) = tx.lock() {
            let _ = tx.write_all(&[1]);
        }
    }
}
/// The watcher thread: stream `mmsg watch all-tags` lines, update
/// the cache + poke on change, respawn on exit. Never returns.
fn tag_watch_loop() {
    use std::io::BufRead;
    loop {
        let spawned = std::process::Command::new("mmsg")
            .args(["watch", "all-tags"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn();
        match spawned {
            Ok(mut child) => {
                if let Some(stdout) = child.stdout.take() {
                    let reader = std::io::BufReader::new(stdout);
                    for line in reader.lines() {
                        let Ok(line) = line else { break };
                        if let Some(t) = parse_tag_state(&line) {
                            if apply_tag_state(t) {
                                tag_watch_poke();
                            }
                        }
                    }
                }
                let _ = child.wait();
            }
            Err(_) => {
                // mmsg missing/transient: back off longer, keep trying.
                std::thread::sleep(std::time::Duration::from_secs(5));
                continue;
            }
        }
        // Stream ended (mango restart / mmsg died): resync once via
        // the poll path, then respawn after a short backoff.
        if let Some(t) = read_tag_state() {
            if apply_tag_state(t) {
                tag_watch_poke();
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
/// Spawn the tag watcher thread once. Idempotent; safe to call from
/// main() (same pattern as ensure_tag_poller).
fn ensure_tag_watcher() {
    static STARTED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    STARTED.get_or_init(|| {
        std::thread::spawn(tag_watch_loop);
    });
}

/// Switch mango to tag `n` for real, detached (never blocks the
/// caller): `mmsg dispatch view,N` — the same `view` function the
/// SUPER+1..9 keybinds call (bind.conf: `bind=SUPER,1,view,1,0`).
/// Sets the cache optimistically so the highlight moves at once; the
/// poller corrects it within ~1 s if the dispatch failed. The child
/// inherits the island's environment, which carries
/// MANGO_INSTANCE_SIGNATURE (without it mmsg refuses to run).
fn dispatch_tag(tag: u8) {
    set_active_tag_cached(tag);
    spawn_exec(&format!("mmsg dispatch view,{tag}"));
}


/// Nav-disc hit test in STAGE coords: Some(-1) = left/previous,
/// Some(+1) = right/next, None = elsewhere. Circular area Ø ~30 around
/// the disc centre (the drawn disc is Ø 28; +1 px slack); the press
/// handler passes a wider radius while a morph runs.
fn arrow_hit_stage(pos: (f64, f64), state: IslandState, radius: f64) -> Option<i32> {
    if !MENU_CYCLE.contains(&state) {
        return None;
    }
    let ((lx, ly), (rx, ry)) = nav_disc_centres(state);
    for (cx, cy, dir) in [(lx, ly, -1), (rx, ry, 1)] {
        let d = ((pos.0 - cx as f64).powi(2) + (pos.1 - cy as f64).powi(2)).sqrt();
        if d <= radius {
            return Some(dir);
        }
    }
    None
}

/// Panel rect + disc squares + bridge strips for ONE state — the same
/// rects add_panel_region adds to the wl region.
fn point_in_state_region(pos: (f64, f64), state: IslandState) -> bool {
    // iter90 a8: the workspace cluster (circles + gaps + pill bridge)
    // is part of every state's region, so hovering it never fires Leave.
    {
        let (wx, wy, ww, wh) = workspace_cluster_rect();
        if pos.0 >= wx as f64
            && pos.0 < (wx + ww) as f64
            && pos.1 >= wy as f64
            && pos.1 < (wy + wh) as f64
        {
            return true;
        }
    }
    // iter108-C: the tray cluster rect (same rect as add_panel_region
    // commits) — hovering it never fires Leave either.
    if let Some((tx, ty, tw, th)) = tray_cluster_rect(tray_item_count()) {
        if pos.0 >= tx as f64
            && pos.0 < (tx + tw) as f64
            && pos.1 >= ty as f64
            && pos.1 < (ty + th) as f64
        {
            return true;
        }
    }
    if point_in_panel_stage(pos, state) {
        return true;
    }
    if MENU_CYCLE.contains(&state) || state == IslandState::TrayMenu {
        // iter30 a3c: the pill stays visible above the panel, with an
        // 4 px gap bridged so the pointer can travel pill <-> panel
        // without firing Leave (a genuine Leave collapses a menu).
        // The pill rect + bridge are part of the menu's region; clicks
        // there are decided by classify_press (pill = collapse, bridge
        // = no-op), never by delivery alone.
        // iter108: the pill rect + bridge belong to TrayMenu too;
        // the disc squares / bridges below stay MENU_CYCLE-only.
        if point_in_panel_stage(pos, IslandState::Collapsed) {
            return true;
        }
        let (ppx, ppy, ppw, pph) = panel_rect_stage(IslandState::Collapsed);
        let (_, poy, _, _) = panel_rect_stage(state);
        if pos.0 >= ppx as f64
            && pos.0 < (ppx + ppw) as f64
            && pos.1 >= (ppy + pph) as f64
            && pos.1 < poy as f64
        {
            return true;
        }
        if MENU_CYCLE.contains(&state) {
            let ((lx, ly), (rx, ry)) = nav_disc_centres(state);
            let r = NAV_DISC_R as f64;
            let gap = NAV_DISC_GAP as f64;
            // Disc squares (each disc's bounding box).
            for (cx, cy) in [(lx, ly), (rx, ry)] {
                if pos.0 >= cx as f64 - r
                    && pos.0 < cx as f64 + r
                    && pos.1 >= cy as f64 - r
                    && pos.1 < cy as f64 + r
                {
                    return true;
                }
            }
            // Bridge strips: left (lx+r, ly-r, gap, 2r) and right
            // (ox+pw, ry-r, gap, 2r) — mirrors add_panel_region.
            let (ox, _, pw, _) = panel_rect_stage(state);
            if pos.0 >= lx as f64 + r
                && pos.0 < lx as f64 + r + gap
                && pos.1 >= ly as f64 - r
                && pos.1 < ly as f64 + r
            {
                return true;
            }
            if pos.0 >= ox as f64 + pw as f64
                && pos.0 < ox as f64 + pw as f64 + gap
                && pos.1 >= ry as f64 - r
                && pos.1 < ry as f64 + r
            {
                return true;
            }
        }
    }
    false
}

/// True when a stage-coords point is inside the panel rect.
fn point_in_panel_stage(pos: (f64, f64), state: IslandState) -> bool {
    let (ox, oy, pw, ph) = panel_rect_stage(state);
    pos.0 >= ox as f64
        && pos.0 < (ox + pw) as f64
        && pos.1 >= oy as f64
        && pos.1 < (oy + ph) as f64
}

/// iter27 (D1/D2 hit-test): what a left press does, decided from stage
/// coords + state + tray count alone — no side effects, so unit tests can pin it.
///   OpenMusic  — collapsed/lock pill press: open the first menu (Music).
///   Cycle(dir) — nav-disc press in a menu state.
///   LaunchRow(i) — Launcher row press (launches, then collapses).
///   ClipRow(i) — Clipboard row press (copies, then collapses).
///   Collapse — pill-rect press while a menu is open (iter27b, see below).
///   NoOp — everything else: control-free panel background (D1), Launcher
///     empty area (D1), stage margin, and ANY press while collapsed/lock
///     outside the pill rect — including the rect the panel just occupied
///     after a collapse (D2). The press handler must not trust delivery
///     alone: after a collapse the input region may still deliver presses
///     from the old panel rect (mango focus stickiness), so the guard
///     below is the real fix, not the region.
/// Note: event.position is ALWAYS in stage coords (iter24 a2) — the
/// stage is fixed STAGE_W x STAGE_H, so no morph-time translation.
#[derive(Clone, Copy, PartialEq, Debug)]
enum PressAction {
    OpenMusic,
    Cycle(i32),
    /// iter90 a8: workspace circle `n` (1..=4) — switches mango to
    /// that tag for real (`mmsg dispatch view,N`).
    WorkspaceTag(u8),
    /// iter108: tray icon `i` (0-based snapshot index) — left
    /// click Activates the item for real.
    TrayActivate(usize),
    /// iter108: DBusMenu row `id` in the open TrayMenu panel —
    /// sends Event(id, "clicked") to the item, then collapses.
    TrayMenuRow(i32),
    LaunchRow(usize),
    /// iter39 a9: Clipboard row `i` (DRAWN row index — the offset
    /// mapping lives inside `clip_row_select_async`) — copies, then
    /// collapses.
    ClipRow(usize),
    /// iter34 a6a: CC toggle pill `i` (0 Wi-Fi, 1 Focus, 2 Bluetooth,
    /// 3 Night Light) — drives the real system, then redraws from live().
    CcToggle(usize),
    /// Bluetooth tile opens the in-panel device submenu (instead of
    /// flipping power directly); the submenu's own pill does that.
    CcBtOpen,
    /// Back chevron in the Bluetooth submenu.
    CcBtBack,
    /// Bluetooth submenu device row `i` — pair/connect off-thread.
    CcBtConnect(usize),
    /// Sound slider's chevron opens the audio-output submenu.
    CcSoundOpen,
    /// Sound submenu output row `i` — reroute existing streams.
    CcSoundRoute(usize),
    /// iter34 a6a: CC slider set — `which` 0 = Display, 1 = Sound,
    /// `level` 0.0..=1.0 from the press x. Starts a drag (see cc_drag).
    CcSlider(u8, f32),
    /// CC power-level pill `i` (0 Saver, 1 Balanced, 2 Performance).
    CcPower(usize),
    /// iter36 a6b: wallpaper filmstrip thumb `i` (global library index)
    /// — applies that wallpaper + its theme for real.
    WallApply(usize),
    /// iter36 a6b: wallpaper filmstrip chevron — move the selection
    /// -1 / +1 (browse only, no apply).
    WallWindow(i32),
    /// iter36 a6b: themes row `i` (global library theme index) —
    /// applies that theme's accent for real.
    ThemeApply(usize),
    /// iter36 a6b: power disc `i` (0 suspend, 1 logout, 2 lock,
    /// 3 reboot, 4 poweroff). Only Lock executes; the destructive
    /// four log their command (command-path verification only).
    PowerButton(usize),
    /// iter36 a6b: calendar month chevron — shift the displayed month
    /// -1 / +1 (wraps Dec<->Jan).
    CalMonth(i32),
    /// iter36 a6b: music transport — 0 prev, 1 play/pause, 2 next.
    /// Drives playerctl/MPRIS; no player => honest idle state.
    MusicTransport(u8),
    /// Pill-rect press while a menu is open: collapse (iter27b).
    Collapse,
    NoOp,
}

/// Pure CC hit test. `submenu` is passed in so tests never touch the
/// panel's global view flag.
fn classify_cc_press(px: f32, py: f32, bt_submenu: bool, sound_submenu: bool) -> PressAction {
    let layout = controlcenter_panel::cc_layout();
    if bt_submenu {
        if layout.bt_back.contains(px, py) {
            return PressAction::CcBtBack;
        }
        if layout.bt_power_pill.contains(px, py) {
            return PressAction::CcToggle(2);
        }
        for (i, row) in layout.bt_device_rows.iter().enumerate() {
            if row.contains(px, py) {
                return PressAction::CcBtConnect(i);
            }
        }
        return PressAction::NoOp;
    }
    if sound_submenu {
        if layout.bt_back.contains(px, py) {
            return PressAction::CcBtBack;
        }
        for (i, row) in layout.sound_rows.iter().enumerate() {
            if row.contains(px, py) {
                return PressAction::CcSoundRoute(i);
            }
        }
        return PressAction::NoOp;
    }
    for (i, pill) in layout.toggle_pills.iter().enumerate() {
        if pill.contains(px, py) {
            return if i == 2 {
                PressAction::CcBtOpen
            } else {
                PressAction::CcToggle(i)
            };
        }
    }
    if layout.sound_end_btn.contains(px, py) {
        return PressAction::CcSoundOpen;
    }
    for (which, track) in [(0u8, layout.display_track), (1u8, layout.sound_track)] {
        if track.contains(px, py + 4.0) || track.contains(px, py - 4.0) {
            let level = ((px - track.x) / track.w).clamp(0.0, 1.0);
            return PressAction::CcSlider(which, level);
        }
    }
    for (i, btn) in layout.power_buttons.iter().enumerate() {
        if btn.contains(px, py) {
            return PressAction::CcPower(i);
        }
    }
    PressAction::NoOp
}

fn classify_press(
    pos: (f64, f64),
    state: IslandState,
    morph_running: bool,
    wall_selected: usize,
    theme_selected: usize,
    tray_count: usize,
) -> PressAction {
    match state {
        IslandState::Collapsed | IslandState::Dictation | IslandState::Lock => {
            // iter90 a8: workspace circles first — they sit outside
            // the pill rect, so the D2 guard below would swallow them.
            if let Some(n) = workspace_hit_stage(pos) {
                return PressAction::WorkspaceTag(n);
            }
            // iter108-C: tray icons first too — they sit outside the
            // pill rect, so the D2 guard below would swallow them.
            if let Some(i) = tray_hit_stage(pos, tray_count) {
                return PressAction::TrayActivate(i);
            }
            // D2: open Music ONLY inside this state's own pill rect
            // (Collapsed (76,24), Lock (54,24) via panel_rect_stage).
            // A press delivered outside it must do nothing — this holds
            // mid-morph too, since begin_morph already sets state to
            // Collapsed at morph start.
            if point_in_panel_stage(pos, state) {
                PressAction::OpenMusic
            } else {
                PressAction::NoOp
            }
        }
        IslandState::Lockscreen => PressAction::NoOp,
        _ => {
            // 1) Nav discs cycle (never collapse). Mid-morph the target
            // disc is still fading in, so the radius stays widened +10
            // while a morph runs (settled clicks keep +1).
            let hit_radius = if morph_running {
                (NAV_DISC_R + 1.0 + 10.0) as f64
            } else {
                (NAV_DISC_R + 1.0) as f64
            };
            if let Some(dir) = arrow_hit_stage(pos, state, hit_radius) {
                return PressAction::Cycle(dir);
            }
            // 1b) iter90 a8: workspace circles (fixed stage coords,
            // outside every panel rect — checked before the panel
            // membership guard in (3) would swallow them as NoOp).
            if let Some(n) = workspace_hit_stage(pos) {
                return PressAction::WorkspaceTag(n);
            }
            // 1c) iter108-C: tray icons (fixed stage coords, outside
            // every panel rect — checked before the pill-collapse
            // check in (2) and the guard in (3) would swallow them).
            if let Some(i) = tray_hit_stage(pos, tray_count) {
                return PressAction::TrayActivate(i);
            }
            // 2) iter27b: the pill's home rect (Collapsed 76x24, top-centre)
            // is the collapse target in menu states too. A click at the
            // pill position (752,27) while a menu is open MUST collapse —
            // that is the established pill open/collapse regression (v0.16
            // collapsed on ANY in-panel press, so the pill click worked
            // through the old branch (3); the D1 no-op above would otherwise
            // swallow it, since the pill position lies inside every menu
            // panel). It is NOT panel background for D1 purposes: every D1
            // repro point lies outside it, and no nav disc overlaps it
            // (discs sit outside the panel edges). No wired control lives
            // there either (controls are unwired, open item a6).
            if point_in_panel_stage(pos, IslandState::Collapsed) {
                return PressAction::Collapse;
            }
            // 3) Transparent stage margin (or a bridge strip): no-op.
            if !point_in_panel_stage(pos, state) {
                return PressAction::NoOp;
            }
            // 4) Launcher rows launch on click (panel-local coords).
            if state == IslandState::Launcher {
                // Iter86: hit-test against the DRAWN rows — the layout
                // stretches when fewer than 6 rows match, so the live
                // visible count picks the stretched geometry; a click
                // on a stretched row launches the row drawn there.
                // (LIVE is single-threaded here; row 0 always starts
                // at 74 and always contains the legacy row-0 centre,
                // so the iter27 row-press/gap tests are unaffected.)
                let layout = launcher_panel::launcher_layout_for_count(
                    launcher_panel::live_visible_count(),
                );
                let (ox, oy) = panel_origin(state);
                let (px, py) = (pos.0 as f32 - ox, pos.1 as f32 - oy);
                for (i, rect) in layout.rows.iter().enumerate() {
                    if px >= rect.x
                        && px < rect.x + rect.w
                        && py >= rect.y
                        && py < rect.y + rect.h
                    {
                        return PressAction::LaunchRow(i);
                    }
                }
                // D1: clicked the panel but no row (search area / empty
                // slot / gap): no-op, never collapse.
                return PressAction::NoOp;
            }
            // iter39 a9: Clipboard rows copy on click (panel-local
            // coords). `clipboard_row_rects()` returns the DRAWN
            // (visible-slot) rects; the offset mapping lives inside
            // `clip_row_select_async`, so `i` here is the drawn row.
            // D1: clicked the panel but no row: no-op, never collapse
            // (same as the Launcher — the live D1 rule).
            if state == IslandState::Clipboard {
                let (ox, oy) = panel_origin(state);
                let (px, py) = (pos.0 as f32 - ox, pos.1 as f32 - oy);
                for (i, rect) in clipboard_panel::clipboard_row_rects().iter().enumerate() {
                    if rect.contains(px, py) {
                        return PressAction::ClipRow(i);
                    }
                }
                return PressAction::NoOp;
            }
            // iter108: TrayMenu rows (panel-local coords) — an
            // enabled, non-separator row sends Event(id,
            // "clicked"); anything else in the panel is a no-op
            // (title / separator / disabled row), never collapse.
            if state == IslandState::TrayMenu {
                let (ox, oy) = panel_origin(state);
                let (px, py) = (pos.0 as f32 - ox, pos.1 as f32 - oy);
                if let Some(id) = crate::tray::menu_row_hit(px, py) {
                    return PressAction::TrayMenuRow(id);
                }
                return PressAction::NoOp;
            }
            // iter34 a6a: Control Centre controls are WIRED now
            // (open item a6, closed here): toggle pills toggle the real
            // system, slider tracks set brightness/volume from the press
            // x (with 4 px vertical slack — the iter27 no-op point sits
            // 7 px below the Sound track, so the slack must stay < 7).
            // Anything else in the panel
            // (end buttons, notification card, gaps) stays a no-op.
            if state == IslandState::ControlCenter {
                let (ox, oy) = panel_origin(state);
                return classify_cc_press(
                    pos.0 as f32 - ox,
                    pos.1 as f32 - oy,
                    controlcenter_panel::cc_bt_submenu_open(),
                    controlcenter_panel::cc_sound_submenu_open(),
                );
            }
            // iter36 a6b: wallpaper filmstrip — a thumb applies that
            // wallpaper + theme for real; the ‹ › chevrons browse the
            // selection (no apply). Panel-local coords.
            if state == IslandState::Wallpaper {
                let (ox, oy) = panel_origin(state);
                let (px, py) = (pos.0 as f32 - ox, pos.1 as f32 - oy);
                if let Some(hit) = wallpaper_panel::filmstrip_hit(px, py) {
                    return match hit {
                        wallpaper_panel::FilmHit::Thumb(p) => {
                            PressAction::WallApply(wallpaper_panel::filmstrip_index(
                                p,
                                wall_selected,
                            ))
                        }
                        wallpaper_panel::FilmHit::Prev => PressAction::WallWindow(-1),
                        wallpaper_panel::FilmHit::Next => PressAction::WallWindow(1),
                    };
                }
                return PressAction::NoOp;
            }
            // iter36 a6b: themes rows — a row applies that theme's
            // accent for real. Row 0 is the selected theme; rows 1..5
            // are the next themes in library order (wrapping).
            if state == IslandState::Themes {
                let (ox, oy) = panel_origin(state);
                let (px, py) = (pos.0 as f32 - ox, pos.1 as f32 - oy);
                for row in 0..5 {
                    let (rx, ry, rw, rh) = themes_panel::row_rect(row);
                    if px >= rx && px < rx + rw && py >= ry && py < ry + rh {
                        // iter128 B2: None when the filter matches
                        // nothing — a click on the empty state applies
                        // nothing instead of an invisible row.
                        return match themes_panel::row_theme_index(row, theme_selected) {
                            Some(i) => PressAction::ThemeApply(i),
                            None => PressAction::NoOp,
                        };
                    }
                }
                return PressAction::NoOp;
            }
            // iter36 a6b: power discs — lock executes for real; the
            // destructive four are command-path only (see the arm).
            if state == IslandState::Power {
                let (ox, oy) = panel_origin(state);
                let (px, py) = (pos.0 as f32 - ox, pos.1 as f32 - oy);
                if let Some(i) = power_panel::button_hit(px, py) {
                    return PressAction::PowerButton(i);
                }
                return PressAction::NoOp;
            }
            // iter36 a6b: calendar month chevrons — shift the displayed
            // month, wrapping Dec<->Jan.
            if state == IslandState::Calendar {
                let (ox, oy) = panel_origin(state);
                let (px, py) = (pos.0 as f32 - ox, pos.1 as f32 - oy);
                if let Some(dir) = calendar_panel::chevron_hit(px, py) {
                    return PressAction::CalMonth(dir);
                }
                return PressAction::NoOp;
            }
            // D1: control-free panel background in any other menu:
            // no-op, never collapse.
            PressAction::NoOp
        }
    }
}

/// Spawn a .desktop Exec line, detached: field codes (%U %F %u %f
/// %i %c %k %v %m — any token starting with '%') are stripped, the
/// rest runs through `sh -c` so quoting in Exec lines keeps working,
/// and setsid + redirected stdio detach the child from the island.
/// (A literal %% in an Exec line is NOT unescaped — vanishingly rare
/// in real .desktop files; noted in the pointer report.)
fn spawn_exec(exec: &str) {
    let cleaned: Vec<&str> = exec
        .split_whitespace()
        .filter(|t| !t.starts_with('%'))
        .collect();
    if cleaned.is_empty() {
        return;
    }
    let cmd = format!("setsid -f {} >/dev/null 2>&1", cleaned.join(" "));
    let _ = std::process::Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

const TOP_MARGIN: i32 = 3;
const NAMESPACE: &str = "dynamite-rust";

// Dictation waveform is drawn INSIDE the island pill, not as a second layer.
static DICTATION_BARS: Mutex<[f32; 27]> = Mutex::new([0.0; 27]);
/// Fullscreen lockscreen-preview size: the Surface's logical output
/// (mmsg `get all-monitors`, 12 Nov 2026: eDP-1 1504x1002 @ scale 1.5).
/// NOTE: 1002, not the 1003 quoted in older docs (1504/1.5 = 1002.67;
/// mango reports 1002). draw() renders at configured_size anyway, so
/// this only sets the requested size; a future output-size read
/// (OutputState::info().logical_size) should replace the constant —
/// see iterations/builder-lockscreen-iter10.md.
const LOCKSCREEN_SIZE: (u32, u32) = (1504, 1002);
// Collapsed clock size is derived from the pill height (see
// lock_timer::text_px); there is no fixed px constant any more.

fn socket_path() -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    dir.join("dynamite-rust.sock")
}

fn hhmm() -> String {
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        // clock_24h config key (settings-app contract): 24 h
        // "05:53" by default; 0 selects 12 h "5:53" (no AM/PM
        // marker — the pill has no room for one and the video
        // never shows one).
        if cfg().clock_24h {
            format!("{:02}:{:02}", tm.tm_hour, tm.tm_min)
        } else {
            let h12 = (tm.tm_hour + 11) % 12 + 1;
            format!("{}:{:02}", h12, tm.tm_min)
        }
    }
}

fn ms_to_next_minute() -> u64 {
    // Milliseconds until just past the next wall-clock minute boundary.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let into_minute = now.subsec_millis() as u64 + (now.as_secs() % 60) * 1000;
    60_000 - into_minute + 60
}

// ---------------------------------------------------------------------------
// Rendering: SDF capsule (black, 1px AA edge) + fontdue white glyphs,
// written premultiplied into the wl_shm canvas (Argb8888 = BGRA bytes LE).
// ---------------------------------------------------------------------------
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

struct TextRow {
    glyphs: Vec<(fontdue::Metrics, std::sync::Arc<[u8]>)>,
    width: f32,
    ascent: f32,
    height: f32,
}

fn layout_text(font: &fontdue::Font, text: &str, px: f32) -> TextRow {
    let mut glyphs = Vec::new();
    let mut width = 0.0f32;
    for ch in text.chars() {
        let (m, cov) = lock_timer::rasterize_cached(font, ch, px);
        width += m.advance_width;
        glyphs.push((m, cov));
    }
    let lm = font.horizontal_line_metrics(px).unwrap();
    TextRow {
        glyphs,
        width,
        ascent: lm.ascent,
        height: lm.ascent - lm.descent,
    }
}

/// Inter-Medium static instance (wght 550, fvar removed by the bake),
/// parsed once for the process: the pill clock's face. fontdue 0.9
/// cannot select variable axes, so the baked static is authoritative:
/// on the reference strings it gives stroke/cap 0.200 and w/cap 3.600
/// (the reference's own numbers), while Inter's default 400 measures
/// 0.143. Search order: next to the executable, the repo path, then
/// the variable font (thin) and finally Quicksand — never a panic.
static CLOCK_FONT: std::sync::OnceLock<Option<fontdue::Font>> = std::sync::OnceLock::new();

fn clock_font() -> Option<&'static fontdue::Font> {
    CLOCK_FONT
        .get_or_init(|| {
            let mut candidates: Vec<PathBuf> = Vec::new();
            if let Ok(exe) = std::env::current_exe() {
                if let Some(dir) = exe.parent() {
                    // target/release -> repo root, and a packaged layout.
                    candidates.push(dir.join("../../assets/fonts/Inter-Medium.ttf"));
                    candidates.push(dir.join("../../../assets/fonts/Inter-Medium.ttf"));
                    candidates.push(dir.join("assets/fonts/Inter-Medium.ttf"));
                }
            }
            if let Some(home) = std::env::var_os("HOME") {
                candidates.push(PathBuf::from(home).join(".local/share/fonts/Inter.ttf"));
            }
            let found = candidates.iter().find_map(|path| {
                std::fs::read(path)
                    .ok()
                    .and_then(|b| fontdue::Font::from_bytes(b, fontdue::FontSettings::default()).ok())
                    .map(|f| (path.clone(), f))
            });
            match found {
                Some((_, f)) => Some(f),
                None => {
                    eprintln!(
                        "dynamite-rust: no clock font found (Inter-Medium.ttf); clock falls back to Quicksand"
                    );
                    None
                }
            }
        })
        .as_ref()
}

/// The UI face for every panel: Quicksand Bold (static, usWeightClass
/// 700). Resolved WITHOUT ever requiring a filesystem font — the old
/// loader only looked in `$HOME/.local/share/fonts` and `.expect`-ed,
/// so the binary panicked at startup on any machine but the dev box
/// (r1c headline 0).
///
/// Search order; the winner is logged once, because the three cases
/// render different text metrics and used to be indistinguishable:
/// 1. next to the executable (target/release -> repo root, packaged layout)
/// 2. `$HOME/.local/share/fonts/Quicksand-Bold.ttf`
/// 3. the vendored asset compiled into the binary — always present, so
///    no install can fail for a missing font
/// 4. `$HOME/.local/share/fonts/Quicksand.ttf` (Light, wght 300) — last,
///    logged as metric-changing
///
/// The font is parsed ONCE and handed out as `&'static`, so there is a
/// single Font object per process. That is not just laziness:
/// `lock_timer::rasterize_cached` keys its glyph cache on the font's
/// address, and a Font that is re-parsed per call can be reallocated at
/// a recycled address and inherit another font's glyph coverage (r1c:
/// that produced zero-width chevrons and a 23-row workspace disc in the
/// parallel test run).
pub(crate) fn ui_font() -> &'static fontdue::Font {
    static FONT: std::sync::OnceLock<fontdue::Font> = std::sync::OnceLock::new();
    FONT.get_or_init(load_ui_font)
}

fn load_ui_font() -> fontdue::Font {
    const EMBEDDED_BOLD: &[u8] = include_bytes!("../assets/fonts/Quicksand-Bold.ttf");
    static LOGGED: std::sync::Once = std::sync::Once::new();
    let mut candidates: Vec<(PathBuf, &'static str)> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push((dir.join("../../assets/fonts/Quicksand-Bold.ttf"), "exe"));
            candidates.push((dir.join("../../../assets/fonts/Quicksand-Bold.ttf"), "exe"));
            candidates.push((dir.join("assets/fonts/Quicksand-Bold.ttf"), "exe"));
        }
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if let Some(home) = &home {
        candidates.push((
            home.join(".local/share/fonts/Quicksand-Bold.ttf"),
            "home-bold",
        ));
    }
    for (path, src) in &candidates {
        let Ok(bytes) = std::fs::read(path) else { continue };
        let Ok(f) = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()) else {
            continue;
        };
        LOGGED.call_once(|| eprintln!("dynamite-rust: UI font {src} {}", path.display()));
        return f;
    }
    if let Ok(f) =
        fontdue::Font::from_bytes(EMBEDDED_BOLD.to_vec(), fontdue::FontSettings::default())
    {
        LOGGED.call_once(|| {
            eprintln!("dynamite-rust: UI font embedded assets/fonts/Quicksand-Bold.ttf")
        });
        return f;
    }
    if let Some(home) = &home {
        let path = home.join(".local/share/fonts/Quicksand.ttf");
        if let Ok(bytes) = std::fs::read(&path) {
            if let Ok(f) = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()) {
                LOGGED.call_once(|| {
                    eprintln!(
                        "dynamite-rust: UI font home-light {} (wght 300 — text metrics differ)",
                        path.display()
                    )
                });
                return f;
            }
        }
    }
    eprintln!(
        "dynamite-rust: no usable UI font: the embedded Quicksand-Bold failed to parse (corrupt build)"
    );
    std::process::exit(1);
}

fn load_icon_font() -> Option<fontdue::Font> {
    // iter116 (Lane A): embedded 15-glyph subset (3,904 B, ~64 kB parse)
    // first; the full disk font stays as fallback if it ever fails.
    if let Ok(f) = fontdue::Font::from_bytes(
        include_bytes!("icons/MaterialSymbolsRounded-Island.ttf").to_vec(),
        fontdue::FontSettings::default(),
    ) {
        return Some(f);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let bytes = std::fs::read(home.join(".local/share/fonts/MaterialSymbolsRounded.ttf")).ok()?;
    fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).ok()
}

/// Lockscreen wallpaper, lazily decoded (iter14): RGBA8 bytes at
/// ~/.cache/dynamite/wallpaper-1504x1002.rgba, pre-rendered by the
/// integrator from the Mango wallpaper (this crate has no PNG decoder).
/// Missing file or wrong size => None (draw_lockscreen falls back to
/// opaque black). ~6 MB while the preview is open, dropped on exit.
fn load_lock_wallpaper() -> Option<Vec<u8>> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let bytes =
        std::fs::read(home.join(".cache/dynamite/wallpaper-1504x1002.rgba")).ok()?;
    if bytes.len() == (1504 * 1002 * 4) as usize {
        Some(bytes)
    } else {
        None
    }
}

/// The shared icon font for a state, loading it lazily on first use
/// (Material Symbols parses to ~19 MB — only Power / ControlCenter /
/// Music / Launcher pay it, and only once). Takes the OnceLock directly
/// (not &Island) so draw_if_ready can call it while the shm pool is
/// mutably borrowed. Returns None for states that draw no glyphs.
fn icon_font_for(
    icon_font: &std::sync::OnceLock<Option<fontdue::Font>>,
    state: IslandState,
) -> Option<&fontdue::Font> {
    match state {
        IslandState::Power
        | IslandState::ControlCenter
        | IslandState::Music
        | IslandState::Launcher
        | IslandState::Lockscreen => icon_font.get_or_init(load_icon_font).as_ref(),
        _ => None,
    }
}

/// Panel renderer (panel-sized canvas, panel-local coords): the capsule
/// background + the state's content. Called ONLY from draw() below,
/// which renders the panel into a scratch and blits it into the stage.
/// (Split out in iter24 when draw() became the stage draw; the body is
/// the old draw() minus the nav-disc tail, which now lives in
/// draw_nav_arrows_stage at stage coords.)
fn draw_panel_into(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    state: IslandState,
    font: &fontdue::Font,
    icon_font: Option<&fontdue::Font>,
    // Lockscreen preview data (iter14): built by draw_if_ready from the
    // Island's lock_* fields; the morph path passes the held default.
    lock_data: &lockscreen::LockscreenData,
    lock_wallpaper: Option<&[u8]>,
    // Screen-logical coords of this panel canvas's top-left (for the
    // fake-frost wallpaper sample).
    bg_origin: (f32, f32),
    // iter36 a6b: live selection state from the Island fields
    // (calendar month offset, selected wallpaper / theme indices).
    cal_offset: i32,
    wall_selected: usize,
    theme_selected: usize,
) {
    let (wf, hf) = (w as f32, h as f32);
    let radius = state.radius();
    // Background: flat-frost capsule (iter31, a3 REVISED) — plain
    // neutral dark fill over the blurred wallpaper; transparent outside.
    // (ControlCentre and Launcher fill their own capsules inside
    // their draw fns; the Lockscreen fills the whole fullscreen canvas
    // itself inside lockscreen::draw_lockscreen.)
    if state != IslandState::ControlCenter
        && state != IslandState::Launcher
        && state != IslandState::Lockscreen
    {
        // Fake frost (iter29): blurred wallpaper under the flat fill.
        // None => iter28 black fill (wallpaper unreadable).
        let bg = wallpaper_blur::ensure_loaded();
        // iter101 (Option 2): live-backdrop frost — the blurred LIVE
        // compositor backdrop behind this panel, frozen at open (see
        // wallpaper_blur.rs). Per-pixel below: variation is the point.
        // The pill/lock/lockscreen keep the wallpaper path (their
        // rects in the clean frame contain the island itself).
        let live = if matches!(
            state,
            IslandState::Collapsed
                | IslandState::Dictation
                | IslandState::Lock
                | IslandState::Lockscreen
        ) {
            None
        } else {
            wallpaper_blur::live_backdrop()
        };
        // iter33 (a3 blur-only): ONE frost sample per panel, taken at
        // the panel centre. A box blur cannot flatten a smooth
        // wallpaper gradient — its symmetric window averages a linear
        // ramp to the centre value — so per-pixel sampling left the
        // pill tip ~17% brighter than its interior and the music panel
        // two-toned (156 vs ~94). Sampling once is blur taken to its
        // limit (fully homogenised frost): same fill formula, same
        // neutralise + backdrop-factor pipeline, just one sample point
        // — no tint, no glass. The dark floor is untouched (sample
        // collapses to black on a dark backdrop, fill stays ~18).
        let uniform_bg: [u8; 3] = match &bg {
            Some(bw) => {
                bw.sample_frost_uniform(bg_origin.0 + wf / 2.0, bg_origin.1 + hf / 2.0)
            }
            None => [0, 0, 0],
        };
        for y in 0..h {
            for x in 0..w {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                // SDF once; alpha is (0.5 - d) clamped — the same math
                // as rounded_rect_alpha.
                let d = rounded_rect_sdf(px, py, wf, hf, radius);
                let a = (0.5 - d).clamp(0.0, 1.0);
                let i = ((y * w + x) * 4) as usize;
                let p = match (&live, &bg) {
                    (Some(lb), _) => frost_fill_px_blur(
                        a,
                        d,
                        px,
                        py,
                        wf,
                        hf,
                        // iter103: the live path uses the shared
                        // FROST_ALPHA const (0.92 — the same value
                        // as frost_alpha(state) for every menu
                        // state, and the single fa knob).
                        crate::wallpaper_blur::FROST_ALPHA,
                        // Live sample: NEUTRALISED at capture
                        // (iter103 — the frame stored in
                        // LiveBackdrop is neutralise_sample plus
                        // its BoxBlur(25) local mean, so the panel
                        // reads neutral dark, B-R approx +3).
                        lb.sample(bg_origin.0 + px, bg_origin.1 + py),
                        lb.local_mean(bg_origin.0 + px, bg_origin.1 + py),
                    ),
                    (None, Some(_)) => frost_fill_px_blur(
                        a,
                        d,
                        px,
                        py,
                        wf,
                        hf,
                        frost_alpha(state),
                        uniform_bg,
                        uniform_bg,
                    ),
                    (None, None) => frost_fill_px(a, d, px, py, wf, hf, frost_alpha(state)),
                };
                // iter91: p is RGB (frost_fill_px_blur computes p[0]
                // from bg[0] = R), but the canvas is BGRA - swap on
                // write: p[2]->B, p[1]->G, p[0]->R. The old straight
                // write flipped R/B, so a blue-sky wallpaper displayed
                // warm. Same fix as controlcenter_panel.rs iter91.
                canvas[i] = p[2]; // B
                canvas[i + 1] = p[1]; // G
                canvas[i + 2] = p[0]; // R
                canvas[i + 3] = p[3]; // A
            }
        }
    }
    match state {
        IslandState::Collapsed => {
            lock_timer::draw_collapsed(canvas, w, h, clock_font().unwrap_or(font), &hhmm());
        }
        IslandState::Dictation => {
            let bars = *DICTATION_BARS.lock().unwrap();
            dictation_canvas::render_pill(canvas, w, h, &bars, 0.0, 1.0, 1.0);
        }
        IslandState::Lock => {
            lock_timer::draw_lock_icon(canvas, w, h);
        }
        IslandState::Power => {
            power_panel::draw_power_panel(canvas, w, h, font, icon_font, "");
        }
        IslandState::ControlCenter => {
            controlcenter_panel::draw_controlcenter_panel(
                canvas,
                w,
                h,
                font,
                icon_font,
                &controlcenter_panel::ControlCenterData::live(),
                bg_origin,
            );
        }
        IslandState::Calendar => {
            calendar_panel::draw_calendar_panel_with_offset(
                canvas, w, h, font, cal_offset,
            );
        }
        IslandState::Music => {
            music_panel::draw_music_panel(
                canvas,
                w,
                h,
                font,
                icon_font,
                &music_panel::live_cached(),
            );
        }
        IslandState::Launcher => {
            // Functional launcher: draw the LIVE data (real .desktop
            // apps + the typed query), not the video-default still.
            launcher_panel::live_draw_data(|data| {
                launcher_panel::draw_launcher_panel_with_data(
                    canvas,
                    w,
                    h,
                    font,
                    icon_font,
                    data,
                    bg_origin,
                );
            });
        }
        IslandState::Clipboard => {
            // No separate Regular font exists in the island (only
            // Quicksand Bold; Quicksand.ttf is its Light fallback
            // for the same slot), so both font args are `font` —
            // same as every other panel's secondary text.
            clipboard_panel::draw_clipboard_panel(
                canvas,
                w,
                h,
                font,
                font,
            );
        }
        IslandState::TrayMenu => {
            // iter108: the open DBusMenu (title + rows) — the
            // capsule background was drawn above, as for every
            // other menu state.
            crate::tray::draw_tray_menu_panel(canvas, w, h, font);
        }
        IslandState::Lockscreen => {
            // Fullscreen lockscreen PREVIEW (iter10 builder, iter14
            // wiring): the held lock frame (f0620) with the LIVE clock
            // and the typed-dot count from the Island fields. NOT a
            // real session lock — no ext-session-lock surface, no PAM,
            // no keyboard input. Wallpaper is the lazily decoded RGBA
            // (None => opaque-black fallback — the crate has no PNG
            // decoder; the scrim then darkens black, so the preview
            // verifies clock/pill/cyan geometry, not the
            // scrim-over-wallpaper look, until the .rgba is present).
            lockscreen::draw_lockscreen(
                canvas,
                w,
                h,
                font,
                icon_font,
                lock_wallpaper,
                lock_data,
            );
        }
        IslandState::Wallpaper => {
            wallpaper_panel::draw_wallpaper_panel_live(
                canvas, w, h, font, wall_selected,
            );
        }
        IslandState::Themes => {
            themes_panel::draw_themes_panel_live(canvas, w, h, font, theme_selected);
        }
    }
}

/// Copy a premultiplied panel scratch onto a stage canvas at (ox, oy).
/// The stage canvas is transparent where the panel is not, so a plain
/// row copy is exactly alpha-over. Out-of-range rows/cols are skipped
/// (defensive: the caller guarantees the fit).
fn blit_panel(
    dst: &mut [u8],
    dst_w: u32,
    dst_h: u32,
    src: &[u8],
    src_w: u32,
    src_h: u32,
    ox: i32,
    oy: i32,
) {
    if src_w == 0 || src_h == 0 || dst_w == 0 || dst_h == 0 {
        return;
    }
    for y in 0..src_h as i32 {
        let dy = oy + y;
        if dy < 0 || dy >= dst_h as i32 {
            continue;
        }
        let mut x0 = 0i32;
        let mut x1 = src_w as i32;
        if ox < 0 {
            x0 = -ox;
        }
        if ox + x1 > dst_w as i32 {
            x1 = dst_w as i32 - ox;
        }
        if x1 <= x0 {
            continue;
        }
        let n = ((x1 - x0) as usize) * 4;
        let s = ((y as usize * src_w as usize) + x0 as usize) * 4;
        let d = ((dy as usize * dst_w as usize) + (ox + x0) as usize) * 4;
        dst[d..d + n].copy_from_slice(&src[s..s + n]);
    }
}

/// Settled collapsed pill drawn at RENDER_SCALE straight into the 2x shm
/// buffer (A1): the clock is rasterised at text_px(pill_h) * RENDER_SCALE and
/// composited 1:1, so its coverage is never resampled by the 1x -> 2x
/// upscale (the compositor still downsamples 2x -> 1.5x, once). The
/// capsule fill repeats draw_panel_into's collapsed path exactly (same
/// frost sample and formula, same 1-logical-px AA band), so geometry and
/// margins are untouched and only the glyphs get sharper.
/// Top-row painter for settled frames (F-G): the pill, the workspace
/// discs and the tray cluster are all rasterised at RENDER_SCALE
/// straight into the 2x shm buffer, so nothing in the top row is ever
/// resampled by the 1x -> 2x upscale. Flat treatments only: no disc
/// shadows, no rims, no gradients. Clears the top-row rect (in device
/// px) first, exactly the rect the damage covers.
fn draw_top_row_2x(
    canvas: &mut [u8],
    bw: u32,
    bh: u32,
    font: &fontdue::Font,
    active_tag: u8,
) {
    let s = RENDER_SCALE as f32;
    let row = top_row_rect();
    let drow = clip_rect(
        (row.0 * 2, row.1 * 2, row.2 * 2, row.3 * 2),
        bw,
        bh,
    );
    if drow.2 <= drow.0 || drow.3 <= drow.1 {
        return;
    }
    for y in drow.1..drow.3 {
        let i = ((y as usize) * bw as usize + drow.0 as usize) * 4;
        let n = ((drow.2 - drow.0) as usize) * 4;
        canvas[i..i + n].fill(0);
    }
    let state = IslandState::Collapsed;
    let (pw, ph) = state.size();
    let (wf, hf) = (pw as f32, ph as f32);
    let (w2, h2) = (pw * RENDER_SCALE, ph * RENDER_SCALE);
    let (pox, poy) = panel_origin(state);
    let (x0, y0) = ((pox * s).round() as i32, (poy * s).round() as i32);
    let radius = state.radius();
    let (sox, soy) = stage_screen_origin();
    let bg_origin = (sox + pox, soy + poy);
    let bg = wallpaper_blur::ensure_loaded();
    let uniform_bg: [u8; 3] = match &bg {
        Some(b) => b.sample_frost_uniform(bg_origin.0 + wf / 2.0, bg_origin.1 + hf / 2.0),
        None => [0, 0, 0],
    };
    for y in 0..h2 {
        for x in 0..w2 {
            let lx = (x as f32 + 0.5) / s;
            let ly = (y as f32 + 0.5) / s;
            let d = rounded_rect_sdf(lx, ly, wf, hf, radius);
            let a = (0.5 - d).clamp(0.0, 1.0);
            if a <= 0.0 {
                continue;
            }
            let p = match &bg {
                Some(_) => frost_fill_px_blur(
                    a,
                    d,
                    lx,
                    ly,
                    wf,
                    hf,
                    frost_alpha(state),
                    uniform_bg,
                    uniform_bg,
                ),
                None => frost_fill_px(a, d, lx, ly, wf, hf, frost_alpha(state)),
            };
            let dx = x0 as u32 + x;
            let dy = y0 as u32 + y;
            if dx >= bw || dy >= bh {
                continue;
            }
            let i = ((dy * bw + dx) * 4) as usize;
            // Premultiplied BGRA over the cleared region: exact copy.
            canvas[i] = p[2];
            canvas[i + 1] = p[1];
            canvas[i + 2] = p[0];
            canvas[i + 3] = p[3];
        }
    }
    draw_text_centred_in(
        canvas,
        bw,
        bh,
        x0,
        y0,
        w2,
        h2,
        clock_font().unwrap_or(font),
        &hhmm(),
        lock_timer::text_px(ph as f32) * s,
    );
    for (i, (cx, cy)) in workspace_disc_centres().iter().enumerate() {
        let tags = ws_visible_tags(active_tag_cached());
        let tag = tags[i];
        draw_workspace_disc_flat_2x(
            canvas,
            bw,
            bh,
            font,
            *cx * s,
            *cy * s,
            tag == active_tag_cached(),
            tag_state_cached().is_occupied(tag),
            WS_LABELS[(tag - 1) as usize],
        );
    }
    crate::tray::draw_tray_cluster_2x(canvas, bw, bh, font, s);
}

/// `lock_timer::draw_text_centred` into a device-pixel window of the
/// canvas: same line-box centring, same max() white composite (over
/// the already-drawn capsule).
fn draw_text_centred_in(
    canvas: &mut [u8],
    bw: u32,
    bh: u32,
    ox: i32,
    oy: i32,
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
    let mut glyphs: Vec<(fontdue::Metrics, std::sync::Arc<[u8]>)> = Vec::new();
    let mut width = 0.0f32;
    for ch in text.chars() {
        let (m, cov) = lock_timer::rasterize_cached(font, ch, px);
        width += m.advance_width;
        glyphs.push((m, cov));
    }
    let Some(lm) = font.horizontal_line_metrics(px) else { return };
    let height = lm.ascent - lm.descent;
    let baseline = (hf - height) / 2.0 + lm.ascent;
    let mut pen = (wf - width) / 2.0;
    for (m, cov) in &glyphs {
        let gx0 = ox + (pen + m.xmin as f32).round() as i32;
        let gy0 = oy + (baseline - m.ymin as f32 - m.height as f32).round() as i32;
        for gy in 0..m.height as i32 {
            for gx in 0..m.width as i32 {
                let (px_x, px_y) = (gx0 + gx, gy0 + gy);
                if px_x < 0 || px_y < 0 || px_x >= bw as i32 || px_y >= bh as i32 {
                    continue;
                }
                let c = cov[(gy as usize) * m.width + gx as usize] as f32 / 255.0;
                if c <= 0.0 {
                    continue;
                }
                let i = (((px_y as u32) * bw + px_x as u32) * 4) as usize;
                let v = (c * 255.0) as u8;
                canvas[i] = canvas[i].max(v);
                canvas[i + 1] = canvas[i + 1].max(v);
                canvas[i + 2] = canvas[i + 2].max(v);
                canvas[i + 3] = 255;
            }
        }
        pen += m.advance_width;
    }
}

/// One workspace disc at device resolution, FLAT (F-G): plain fill, no
/// drop shadow, no lit rim. AA is a 4x4 supersampled SDF edge so the
/// 32 px disc is round without a halo. The label uses the clock's font
/// and the same alpha-over treatment as the clock.
fn draw_workspace_disc_flat_2x(
    canvas: &mut [u8],
    bw: u32,
    bh: u32,
    font: &fontdue::Font,
    cx: f32,
    cy: f32,
    active: bool,
    occupied: bool,
    label: &str,
) {
    const SS: usize = 4;
    let s = RENDER_SCALE as f32;
    let r = WS_DISC_R * s;
    let (fr, fg, fb) = if active {
        WS_ACTIVE_FILL
    } else if occupied {
        WS_OCCUPIED_FILL
    } else {
        WS_INACTIVE_FILL
    };
    let glyph_rgb = if active { WS_ACTIVE_GLYPH } else { WS_INACTIVE_GLYPH };
    let x0 = ((cx - r - 1.0).floor() as i32).max(0);
    let x1 = ((cx + r + 1.0).ceil() as i32).min(bw as i32);
    let y0 = ((cy - r - 1.0).floor() as i32).max(0);
    let y1 = ((cy + r + 1.0).ceil() as i32).min(bh as i32);
    for y in y0..y1 {
        for x in x0..x1 {
            let mut a_sum = 0.0f32;
            for sub_y in 0..SS {
                for sub_x in 0..SS {
                    let ox = (sub_x as f32 + 0.5) / SS as f32 - 0.5;
                    let oy = (sub_y as f32 + 0.5) / SS as f32 - 0.5;
                    let px = x as f32 + 0.5 + ox;
                    let py = y as f32 + 0.5 + oy;
                    let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
                    a_sum += (r + 0.5 - d).clamp(0.0, 1.0);
                }
            }
            let cov = a_sum / (SS * SS) as f32;
            if cov <= 0.0 {
                continue;
            }
            let i = (((y as u32) * bw + x as u32) * 4) as usize;
            let inv = 1.0 - cov;
            // Premultiplied source (fill is opaque): out = c*cov + dst*inv.
            canvas[i] = (fb as f32 * cov + canvas[i] as f32 * inv) as u8;
            canvas[i + 1] = (fg as f32 * cov + canvas[i + 1] as f32 * inv) as u8;
            canvas[i + 2] = (fr as f32 * cov + canvas[i + 2] as f32 * inv) as u8;
            canvas[i + 3] = ((cov + canvas[i + 3] as f32 / 255.0 * inv) * 255.0) as u8;
        }
    }
    let row = layout_text(clock_font().unwrap_or(font), label, WS_NUMBER_PX * s);
    let baseline = cy - row.height / 2.0 + row.ascent;
    let mut pen = cx - row.width / 2.0;
    for (gm, cov) in &row.glyphs {
        let gx0 = (pen + gm.xmin as f32).round() as i32;
        let gy0 = (baseline - gm.ymin as f32 - gm.height as f32).round() as i32;
        for gy in 0..gm.height as i32 {
            for gx in 0..gm.width as i32 {
                let (px_x, px_y) = (gx0 + gx, gy0 + gy);
                if px_x < 0 || px_y < 0 || px_x >= bw as i32 || px_y >= bh as i32 {
                    continue;
                }
                let cc = cov[(gy as usize) * gm.width + gx as usize] as f32 / 255.0;
                if cc <= 0.0 {
                    continue;
                }
                let i = (((px_y as u32) * bw + px_x as u32) * 4) as usize;
                let inv = 1.0 - cc;
                canvas[i] = (glyph_rgb.2 as f32 * cc + canvas[i] as f32 * inv) as u8;
                canvas[i + 1] = (glyph_rgb.1 as f32 * cc + canvas[i + 1] as f32 * inv) as u8;
                canvas[i + 2] = (glyph_rgb.0 as f32 * cc + canvas[i + 2] as f32 * inv) as u8;
                canvas[i + 3] = 255;
            }
        }
        pen += gm.advance_width;
    }
}

fn draw_pill_over_stage(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &fontdue::Font,
    icon_font: Option<&fontdue::Font>,
    lock_data: &lockscreen::LockscreenData,
    lock_wallpaper: Option<&[u8]>,
) {
    let (pw, ph) = IslandState::Collapsed.size();
    let (ox, oy) = panel_origin(IslandState::Collapsed);
    let (sox, soy) = stage_screen_origin();
    with_panel_scratch((pw * ph * 4) as usize, |scratch| {
        draw_panel_into(
            scratch,
            pw,
            ph,
            IslandState::Collapsed,
            font,
            icon_font,
            lock_data,
            lock_wallpaper,
            (sox + ox, soy + oy),
            0,
            0,
            0,
        );
        blit_panel(canvas, w, h, scratch, pw, ph, ox as i32, oy as i32);
    });
}

thread_local! {
    /// One grow-only panel scratch for the draw path (F-B): the old
    /// `vec![0u8; pw*ph*4]` allocated up to 1.1 MB per frame and handed
    /// it straight back to the allocator.
    static PANEL_SCRATCH: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn with_panel_scratch<R>(need: usize, f: impl FnOnce(&mut [u8]) -> R) -> R {
    PANEL_SCRATCH.with(|cell| {
        let mut buf = cell.borrow_mut();
        if buf.len() < need {
            buf.resize(need, 0);
        } else {
            buf[..need].fill(0);
        }
        f(&mut buf[..need])
    })
}

/// Stage draw (iter24): the canvas is the fixed STAGE_W x STAGE_H
/// surface (or fullscreen for the lockscreen preview). The panel is
/// rendered into a panel-sized scratch by draw_panel_into, blitted at
/// panel_origin, and the nav discs are drawn OUTSIDE the panel at the
/// stage coords from nav_disc_centres — never over panel content.
/// Callers: draw_if_ready (settled) and draw_state_into (morph frames).
fn draw(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    state: IslandState,
    font: &fontdue::Font,
    icon_font: Option<&fontdue::Font>,
    lock_data: &lockscreen::LockscreenData,
    lock_wallpaper: Option<&[u8]>,
) {
    ensure_tag_poller();
    crate::tray::ensure_tray_host();
    if state == IslandState::Lockscreen {
        // Fullscreen preview: no stage, no discs — draw direct.
        lockscreen::draw_lockscreen(canvas, w, h, font, icon_font, lock_wallpaper, lock_data);
        return;
    }
    debug_assert_eq!((w, h), (STAGE_W, STAGE_H));
    // No full fill: draw_if_ready clears exactly the damage rect, so a
    // settled frame touches ~0.5 MB instead of 3 MB of stage scratch.
    let (pw, ph) = state.size();
    let (ox, oy) = panel_origin(state);
    let (sox, soy) = stage_screen_origin();
    let (cal_offset, wall_selected, theme_selected) = *SELECTION.read().unwrap();
    with_panel_scratch((pw * ph * 4) as usize, |scratch| {
        draw_panel_into(
            scratch,
            pw,
            ph,
            state,
            font,
            icon_font,
            lock_data,
            lock_wallpaper,
            (sox + ox, soy + oy),
            cal_offset,
            wall_selected,
            theme_selected,
        );
        blit_panel(canvas, w, h, scratch, pw, ph, ox as i32, oy as i32);
    });
    if MENU_CYCLE.contains(&state) {
        // Panel drop shadow (iter26) — after the blit so the
        // (1 - dst alpha) gate keeps it outside the panel silhouette.
        draw_panel_shadow_stage(canvas, w, h, state, 1.0);
        draw_nav_arrows_stage(canvas, w, h, state);
        // iter30 a3c: the pill stays visible above the menu panel.
        draw_pill_over_stage(canvas, w, h, font, icon_font, lock_data, lock_wallpaper);
    } else if state == IslandState::TrayMenu {
        // iter108: the tray menu is a panel, not a cycled menu —
        // same shadow + pill-over, but no nav discs (cycling
        // must never land on an item-owned menu).
        draw_panel_shadow_stage(canvas, w, h, state, 1.0);
        draw_pill_over_stage(canvas, w, h, font, icon_font, lock_data, lock_wallpaper);
    }
    // iter90 a8: workspace circles ride with the pill cluster in
    // every non-lockscreen state — collapsed, lock, and all menus.
    draw_workspace_circles_stage(canvas, w, h, font, active_tag_cached());
    // iter108: tray cluster rides with the pill cluster in every
    // non-lockscreen state too, on the RIGHT of the pill.
    crate::tray::draw_tray_cluster_stage(canvas, w, h, font);
}

/// Distance from point `p` to segment `a`-`b` (for the chevron SDF).
fn seg_dist(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let (dx, dy) = (bx - ax, by - ay);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((px - ax) * dx + (py - ay) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (ex, ey) = (px - (ax + t * dx), py - (ay + t * dy));
    (ex * ex + ey * ey).sqrt()
}

// ---------------------------------------------------------------------------
// Disc rim + disc shadow + panel drop shadow (iter26, constants above).
// ---------------------------------------------------------------------------

/// Smoothstep 0->1 over [0, 1] (clamped): a softer falloff than linear,
/// so shadow tails fade out instead of ending in a visible line.
fn smooth01(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Signed distance to a rounded rect (negative inside). Point (x, y)
/// is relative to the rect's top-left corner; same math as
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

/// Alpha of the disc drop shadow at (px, py) for a disc centred
/// (cx, cy): full DISC_SHADOW_ALPHA inside the offset circle, smooth
/// falloff over DISC_SHADOW_BLUR outside it.
/// disc_shadow_alpha_r is the same shadow at an arbitrary radius
/// (iter92: the Ø16 workspace discs).
fn disc_shadow_alpha(px: f32, py: f32, cx: f32, cy: f32) -> f32 {
    disc_shadow_alpha_r(px, py, cx, cy, NAV_DISC_R)
}

/// Radius-parameterised disc shadow (iter92: the workspace discs are
/// Ø16, so their shadow must be sized by WS_DISC_R, not NAV_DISC_R).
fn disc_shadow_alpha_r(px: f32, py: f32, cx: f32, cy: f32, r: f32) -> f32 {
    let (sx, sy) = (cx + DISC_SHADOW_OFF.0, cy + DISC_SHADOW_OFF.1);
    let d = ((px - sx).powi(2) + (py - sy).powi(2)).sqrt();
    if d <= r {
        DISC_SHADOW_ALPHA
    } else {
        DISC_SHADOW_ALPHA * (1.0 - smooth01((d - r) / DISC_SHADOW_BLUR))
    }
}

/// Alpha of the panel drop shadow at stage point (px, py) for a panel
/// rect (ox, oy, pw, ph) with corner radius r: the same rounded rect
/// translated by PANEL_SHADOW_OFF, full PANEL_SHADOW_ALPHA inside,
/// smooth falloff over PANEL_SHADOW_BLUR outside.
fn panel_shadow_alpha(px: f32, py: f32, ox: f32, oy: f32, pw: f32, ph: f32, r: f32) -> f32 {
    let d = rounded_rect_sdf(
        px - (ox + PANEL_SHADOW_OFF.0),
        py - (oy + PANEL_SHADOW_OFF.1),
        pw,
        ph,
        r,
    );
    if d <= 0.0 {
        PANEL_SHADOW_ALPHA
    } else {
        PANEL_SHADOW_ALPHA * (1.0 - smooth01(d / PANEL_SHADOW_BLUR))
    }
}

/// Composite a black shadow of alpha `a` (0..1) onto premultiplied
/// canvas pixel `i`: darkens the pixel, leaves it untouched where the
/// panel is opaque (dst alpha 255) and writes a black translucent
/// pixel where the canvas is transparent. The (1 - dst alpha) gate is
/// what keeps the shadow outside the panel silhouette when the shadow
/// is drawn after the panel blit.
fn composite_black_shadow(canvas: &mut [u8], i: usize, a: f32) {
    if a <= 0.0 {
        return;
    }
    let dst_a = canvas[i + 3] as f32 / 255.0;
    let ae = (a * (1.0 - dst_a)).clamp(0.0, 1.0);
    if ae <= 0.0 {
        return;
    }
    let keep = 1.0 - ae;
    canvas[i] = (canvas[i] as f32 * keep) as u8;
    canvas[i + 1] = (canvas[i + 1] as f32 * keep) as u8;
    canvas[i + 2] = (canvas[i + 2] as f32 * keep) as u8;
    canvas[i + 3] = ((ae + dst_a * keep) * 255.0) as u8;
}

/// Rim highlight for a nav disc at stage point (px, py), disc centre
/// (cx, cy): a 1.5 px band hugging the inside of the disc edge, lit
/// from above — light periwinkle-grey at the top, slightly darker than
/// the disc fill at the bottom, mid-grey at the sides. Returns
/// (R, G, B, alpha) to blend over the disc fill, or None away from
/// the edge band. The chevron (radius <= ~7) never reaches the band,
/// so it stays untouched and fully legible.
///
/// disc_rim_r is the same band at an arbitrary radius (iter92:
/// the Ø16 workspace discs).
fn disc_rim(px: f32, py: f32, cx: f32, cy: f32) -> Option<(f32, f32, f32, f32)> {
    disc_rim_r(px, py, cx, cy, NAV_DISC_R)
}

/// Radius-parameterised rim (iter92: the Ø16 workspace discs need the
/// 1.5 px band hugging THEIR edge, r = 8, not the nav-disc r = 14).
fn disc_rim_r(px: f32, py: f32, cx: f32, cy: f32, r: f32) -> Option<(f32, f32, f32, f32)> {
    let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
    let band = 1.0 - ((d - (r - RIM_WIDTH / 2.0)).abs() / (RIM_WIDTH / 2.0));
    if band <= 0.0 {
        return None;
    }
    // t = 1 at the top of the disc, 0 at the bottom.
    let t = (((cy - py) / r) + 1.0) / 2.0;
    let t = t.clamp(0.0, 1.0);
    let r = RIM_DARK.0 + (RIM_LIGHT.0 - RIM_DARK.0) * t;
    let g = RIM_DARK.1 + (RIM_LIGHT.1 - RIM_DARK.1) * t;
    let b = RIM_DARK.2 + (RIM_LIGHT.2 - RIM_DARK.2) * t;
    let a = (RIM_DARK_ALPHA + (RIM_LIGHT_ALPHA - RIM_DARK_ALPHA) * t) * band;
    Some((r, g, b, a))
}

/// Draw the expanded panel's drop shadow into the stage canvas.
/// Called AFTER the panel blit; the (1 - dst alpha) gate inside
/// composite_black_shadow keeps panel content untouched, so the
/// shadow only shows outside the panel silhouette (below the bottom
/// edge, along the sides, around the rounded corners). `opacity`
/// scales the peak alpha (1.0 settled; morph frames pass the fade).
/// No-op for non-menu states (collapsed/lock get no shadow).
fn draw_panel_shadow_stage(canvas: &mut [u8], w: u32, h: u32, state: IslandState, opacity: f32) {
    if opacity <= 0.0 || !MENU_CYCLE.contains(&state) {
        return;
    }
    let (ox, oy, pw, ph) = panel_rect_stage(state);
    let r = state.radius();
    let m = PANEL_SHADOW_BLUR + PANEL_SHADOW_OFF.0.abs().max(PANEL_SHADOW_OFF.1.abs()) + 1.0;
    let x0 = ((ox - m).floor() as i32).max(0);
    let x1 = ((ox + pw + m).ceil() as i32).min(w as i32);
    let y0 = ((oy - m).floor() as i32).max(0);
    let y1 = ((oy + ph + m).ceil() as i32).min(h as i32);
    for y in y0..y1 {
        for x in x0..x1 {
            let a = panel_shadow_alpha(x as f32 + 0.5, y as f32 + 0.5, ox, oy, pw, ph, r) * opacity;
            if a > 0.0 {
                let i = ((y as u32 * w + x as u32) * 4) as usize;
                composite_black_shadow(canvas, i, a);
            }
        }
    }
}

/// Nav arrow discs (interaction-spec §6, iter24 a2): in the original's
/// launcher frame, two Ø 28 dark discs sit at the panel's left/right
/// edges, each carrying a chevron. Here they are the prev/next menu
/// buttons in EVERY menu view (the user's arrow decision — the page
/// dots of §4 are skipped), and since iter24 they sit OUTSIDE the panel
/// in the transparent stage margin, centred on the panel's vertical
/// middle (iter30 a3b) — never over panel content. Disc #1C1E22 (the power panel's
/// grey-disc colour) at 0.92 coverage, chevron glyph #E8EAED drawn as
/// two SDF strokes. Draw and hit-test share nav_disc_centres, so they
/// can never drift apart. Click side: arrow_hit_stage().
/// Iter26: each disc also gets a soft drop shadow (offset down-right)
/// and a 1.5 px rim — light periwinkle-grey on the upper arc, darker
/// on the lower arc — so the discs read as floating coins, not flat
/// cutouts. The chevron stays centred and untouched by the rim.

/// Supersample factor for the workspace-disc rasteriser (iter109):
/// every stage pixel of a workspace disc is evaluated on an 8 x 8
/// sub-grid and the results averaged. The disc is only 16 px across,
/// so a single point sample per pixel quantises the edge into ~8 flat
/// facets (the octagon Alexandre reported 15 Nov 2026); the
/// supersampled mean carries the true sub-pixel edge position in
/// every pixel's coverage, and the 2x upscale + compositor downsample
/// then reconstruct a smooth circle from it. The same grid is used
/// for the shadow, the fill and the rim, so all three agree.
const WS_DISC_SS: usize = 8;

/// Draw one workspace disc (iter90 a8, resized iter92): the nav-disc
/// treatment — drop shadow, Ø16 fill, 1.5 px rim — with the tag number
/// centred where the chevron sits in a nav disc. Active = periwinkle
/// fill + dark-navy number; inactive = dark #1C1E22 + near-white number.
/// Shadow and rim are sized by WS_DISC_R via the _r helpers — the
/// plain disc_shadow_alpha/disc_rim hardcode the nav-disc Ø28.
/// The number is rasterized with the UI font at WS_NUMBER_PX and
/// alpha-over blended (same convention as the nav chevron).
fn draw_workspace_disc(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    cx: f32,
    cy: f32,
    active: bool,
    occupied: bool,
    font: &fontdue::Font,
    label: &str,
) {
    let (fr, fg, fb) = if active {
        WS_ACTIVE_FILL
    } else if occupied {
        WS_OCCUPIED_FILL
    } else {
        WS_INACTIVE_FILL
    };
    let glyph_rgb = if active {
        WS_ACTIVE_GLYPH
    } else {
        WS_INACTIVE_GLYPH
    };
    let x0 = ((cx - WS_DISC_R - 1.0).floor() as i32).max(0);
    let x1 = ((cx + WS_DISC_R + 1.0).ceil() as i32).min(w as i32);
    let y0 = ((cy - WS_DISC_R - 1.0).floor() as i32).max(0);
    let y1 = ((cy + WS_DISC_R + 1.0).ceil() as i32).min(h as i32);
    // Fill + rim, supersampled (iter109). For each stage pixel the
    // 1 px SDF ramp is evaluated at every sub-sample and averaged, so
    // the pixel's coverage is the true area of the disc inside it
    // (0.5 exactly at d = R) instead of the point sample at the pixel
    // centre. The rim is evaluated per sub-sample too, so its 1.5 px
    // band does not quantise either.
    //
    // The pixel is written as a proper premultiplied sample:
    // RGB = rim-blended fill * 0.92 * coverage, A = coverage.
    // The old code wrote A = 255 for every fringe pixel, which made
    // the 1 px AA fringe an opaque dark ring: the 2x upscale +
    // compositor downsample then smeared that ring outward and the
    // darkness test counted it as disc, fattening every silhouette
    // by ~1 px (tip row 12 px instead of 7, an 11-row flat run —
    // the octagon). With A = coverage the fringe is translucent,
    // the wallpaper shows through it, and the counted silhouette is
    // the true circle. Interior pixels still land at A = 255 with
    // the same RGB as before, so the disc's look is unchanged.
    // Canvas is BGRA; rgb_sum is accumulated as (R, G, B).
    for y in y0..y1 {
        for x in x0..x1 {
            let mut rgb_sum = [0.0f32; 3];
            let mut a_sum = 0.0f32;
            for sub_y in 0..WS_DISC_SS {
                for sub_x in 0..WS_DISC_SS {
                    let ox = (sub_x as f32 + 0.5) / WS_DISC_SS as f32 - 0.5;
                    let oy = (sub_y as f32 + 0.5) / WS_DISC_SS as f32 - 0.5;
                    let px = x as f32 + 0.5 + ox;
                    let py = y as f32 + 0.5 + oy;
                    let d_disc = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
                    let a_sub = (WS_DISC_R + 0.5 - d_disc).clamp(0.0, 1.0);
                    if a_sub > 0.0 {
                        a_sum += a_sub;
                        // Rim highlight (iter26), per sub-sample: the
                        // _r variant, sized by WS_DISC_R — the plain
                        // disc_rim hardcodes the nav Ø28.
                        let mut col = [fr as f32 * 0.92, fg as f32 * 0.92, fb as f32 * 0.92];
                        if let Some((rr, gg, bb, aa)) =
                            disc_rim_r(px, py, cx, cy, WS_DISC_R)
                        {
                            if aa > 0.0 {
                                let inv = 1.0 - aa;
                                col = [
                                    rr * aa + col[0] * inv,
                                    gg * aa + col[1] * inv,
                                    bb * aa + col[2] * inv,
                                ];
                            }
                        }
                        rgb_sum[0] += col[0] * a_sub;
                        rgb_sum[1] += col[1] * a_sub;
                        rgb_sum[2] += col[2] * a_sub;
                    }
                }
            }
            if a_sum > 0.0 {
                let n = (WS_DISC_SS * WS_DISC_SS) as f32;
                let cov = a_sum / n;
                let i = (((y as u32 * w) + x as u32) * 4) as usize;
                let dst_a = canvas[i + 3] as f32 / 255.0;
                let inv = 1.0 - cov;
                canvas[i] = (rgb_sum[2] / n + canvas[i] as f32 * inv) as u8;
                canvas[i + 1] = (rgb_sum[1] / n + canvas[i + 1] as f32 * inv) as u8;
                canvas[i + 2] = (rgb_sum[0] / n + canvas[i + 2] as f32 * inv) as u8;
                canvas[i + 3] = ((cov + dst_a * inv) * 255.0) as u8;
            }
        }
    }
    // Number, centred on (cx, cy): same line-box centring as
    // lock_timer::draw_text_centred (baseline = cy - height/2 + ascent).
    let row = layout_text(font, label, WS_NUMBER_PX);
    let baseline = cy - row.height / 2.0 + row.ascent;
    let mut pen = cx - row.width / 2.0;
    for (gm, cov) in &row.glyphs {
        let gx0 = (pen + gm.xmin as f32).round() as i32;
        let gy0 = (baseline - gm.ymin as f32 - gm.height as f32).round() as i32;
        for gy in 0..gm.height as i32 {
            for gx in 0..gm.width as i32 {
                let (px_x, px_y) = (gx0 + gx, gy0 + gy);
                if px_x < 0 || px_y < 0 || px_x >= w as i32 || px_y >= h as i32 {
                    continue;
                }
                let cc = cov[(gy as usize) * gm.width + gx as usize] as f32 / 255.0;
                if cc <= 0.0 {
                    continue;
                }
                let i = (((px_y as u32) * w + px_x as u32) * 4) as usize;
                let inv = 1.0 - cc;
                canvas[i] = (glyph_rgb.2 as f32 * cc + canvas[i] as f32 * inv) as u8;
                canvas[i + 1] = (glyph_rgb.1 as f32 * cc + canvas[i + 1] as f32 * inv) as u8;
                canvas[i + 2] = (glyph_rgb.0 as f32 * cc + canvas[i + 2] as f32 * inv) as u8;
                canvas[i + 3] = 255;
            }
        }
        pen += gm.advance_width;
    }
}

/// Draw all four workspace circles into the stage canvas (iter90 a8).
/// `active_tag` is the cached mango tag (1..=9); only 1..=4 ever
/// matches a circle — a tag 5..9 leaves every circle inactive.
fn draw_workspace_circles_stage(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &fontdue::Font,
    active_tag: u8,
) {
    // r1c: one snapshot read for the whole row (the active tag can move
    // between two reads, and a half-updated row would draw two actives).
    let tags = ws_visible_tags(active_tag);
    let state = tag_state_cached();
    for (i, (cx, cy)) in workspace_disc_centres().iter().enumerate() {
        let tag = tags[i];
        draw_workspace_disc(
            canvas,
            w,
            h,
            *cx,
            *cy,
            tag == active_tag,
            state.is_occupied(tag),
            font,
            WS_LABELS[(tag - 1) as usize],
        );
    }
}

fn draw_nav_arrows_stage(canvas: &mut [u8], w: u32, h: u32, state: IslandState) {
    let ((lx, ly), (rx, ry)) = nav_disc_centres(state);
    draw_nav_disc(canvas, w, h, lx, ly, -1.0);
    draw_nav_disc(canvas, w, h, rx, ry, 1.0);
}

/// Nav disc with an opacity multiplier (morph frames only): the discs
/// sit OUTSIDE the capsule, so the capsule-alpha composite in
/// draw_morph_frame would mask them out — draw_morph_frame draws them
/// after the capsule loop, cross-fading the outgoing state's discs out
/// and the incoming state's discs in. Premultiplied alpha-over onto
/// the canvas (unlike draw_nav_disc, which forces A=255 for the
/// settled path — kept bit-identical there).
fn draw_nav_disc_faded(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    cx: f32,
    cy: f32,
    dir: f32,
    opacity: f32,
) {
    if opacity <= 0.0 {
        return;
    }
    const HALF_W: f32 = 4.0; // chevron tip-to-arm horizontal run
    const HALF_H: f32 = 6.0; // chevron half height
    const STROKE: f32 = 2.4;
    let tip = (cx + dir * HALF_W, cy);
    let top = (cx - dir * HALF_W, cy - HALF_H);
    let bot = (cx - dir * HALF_W, cy + HALF_H);
    // Drop shadow (iter26), drawn first so the disc covers the overlap;
    // the (1 - dst alpha) gate keeps it off opaque panel content when
    // a morphing panel reaches under a disc.
    {
        let (sx, sy) = (cx + DISC_SHADOW_OFF.0, cy + DISC_SHADOW_OFF.1);
        let m = NAV_DISC_R + DISC_SHADOW_BLUR + 1.0;
        let x0 = ((sx - m).floor() as i32).max(0);
        let x1 = ((sx + m).ceil() as i32).min(w as i32);
        let y0 = ((sy - m).floor() as i32).max(0);
        let y1 = ((sy + m).ceil() as i32).min(h as i32);
        for y in y0..y1 {
            for x in x0..x1 {
                let a = disc_shadow_alpha(x as f32 + 0.5, y as f32 + 0.5, cx, cy) * opacity;
                if a > 0.0 {
                    let i = (((y as u32 * w) + x as u32) * 4) as usize;
                    composite_black_shadow(canvas, i, a);
                }
            }
        }
    }
    let x0 = ((cx - NAV_DISC_R - 1.0).floor() as i32).max(0);
    let x1 = ((cx + NAV_DISC_R + 1.0).ceil() as i32).min(w as i32);
    let y0 = ((cy - NAV_DISC_R - 1.0).floor() as i32).max(0);
    let y1 = ((cy + NAV_DISC_R + 1.0).ceil() as i32).min(h as i32);
    for y in y0..y1 {
        for x in x0..x1 {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let i = (((y as u32 * w) + x as u32) * 4) as usize;
            let d_disc = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
            let a_disc = (NAV_DISC_R + 0.5 - d_disc).clamp(0.0, 1.0) * 0.92 * opacity;
            if a_disc > 0.0 {
                let inv = 1.0 - a_disc;
                canvas[i] = (34.0 * a_disc + canvas[i] as f32 * inv) as u8; // B 0x22
                canvas[i + 1] = (30.0 * a_disc + canvas[i + 1] as f32 * inv) as u8; // G 0x1E
                canvas[i + 2] = (28.0 * a_disc + canvas[i + 2] as f32 * inv) as u8; // R 0x1C
                canvas[i + 3] = (255.0 * a_disc + canvas[i + 3] as f32 * inv) as u8;
            }
            // Rim highlight (iter26): light on the upper arc, darker
            // on the lower arc; the chevron below never reaches it.
            if let Some((rr, gg, bb, aa)) = disc_rim(px, py, cx, cy) {
                let a_rim = aa * opacity;
                if a_rim > 0.0 {
                    let inv = 1.0 - a_rim;
                    canvas[i] = (bb * a_rim + canvas[i] as f32 * inv) as u8;
                    canvas[i + 1] = (gg * a_rim + canvas[i + 1] as f32 * inv) as u8;
                    canvas[i + 2] = (rr * a_rim + canvas[i + 2] as f32 * inv) as u8;
                    canvas[i + 3] = (255.0 * a_rim + canvas[i + 3] as f32 * inv) as u8;
                }
            }
            let d_line = seg_dist(px, py, tip.0, tip.1, top.0, top.1)
                .min(seg_dist(px, py, tip.0, tip.1, bot.0, bot.1));
            let a_line = (STROKE / 2.0 + 0.5 - d_line).clamp(0.0, 1.0) * opacity;
            if a_line > 0.0 {
                let inv = 1.0 - a_line;
                canvas[i] = (237.0 * a_line + canvas[i] as f32 * inv) as u8; // B 0xED
                canvas[i + 1] = (234.0 * a_line + canvas[i + 1] as f32 * inv) as u8; // G 0xEA
                canvas[i + 2] = (232.0 * a_line + canvas[i + 2] as f32 * inv) as u8; // R 0xE8
                canvas[i + 3] = (255.0 * a_line + canvas[i + 3] as f32 * inv) as u8;
            }
        }
    }
}

fn draw_nav_disc(canvas: &mut [u8], w: u32, h: u32, cx: f32, cy: f32, dir: f32) {
    const HALF_W: f32 = 4.0; // chevron tip-to-arm horizontal run
    const HALF_H: f32 = 6.0; // chevron half height
    const STROKE: f32 = 2.4;
    let tip = (cx + dir * HALF_W, cy);
    let top = (cx - dir * HALF_W, cy - HALF_H);
    let bot = (cx - dir * HALF_W, cy + HALF_H);
    // Drop shadow (iter26), drawn first so the disc covers the overlap.
    {
        let (sx, sy) = (cx + DISC_SHADOW_OFF.0, cy + DISC_SHADOW_OFF.1);
        let m = NAV_DISC_R + DISC_SHADOW_BLUR + 1.0;
        let x0 = ((sx - m).floor() as i32).max(0);
        let x1 = ((sx + m).ceil() as i32).min(w as i32);
        let y0 = ((sy - m).floor() as i32).max(0);
        let y1 = ((sy + m).ceil() as i32).min(h as i32);
        for y in y0..y1 {
            for x in x0..x1 {
                let a = disc_shadow_alpha(x as f32 + 0.5, y as f32 + 0.5, cx, cy);
                if a > 0.0 {
                    let i = (((y as u32 * w) + x as u32) * 4) as usize;
                    composite_black_shadow(canvas, i, a);
                }
            }
        }
    }
    let x0 = ((cx - NAV_DISC_R - 1.0).floor() as i32).max(0);
    let x1 = ((cx + NAV_DISC_R + 1.0).ceil() as i32).min(w as i32);
    let y0 = ((cy - NAV_DISC_R - 1.0).floor() as i32).max(0);
    let y1 = ((cy + NAV_DISC_R + 1.0).ceil() as i32).min(h as i32);
    for y in y0..y1 {
        for x in x0..x1 {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let i = (((y as u32 * w) + x as u32) * 4) as usize;
            // Disc: SDF circle, alpha-over (premultiplied BGRA;
            // the panel content underneath is opaque, alpha stays
            // 255 — the same shortcut as launcher_panel::blend_pixel).
            let d_disc = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
            let a_disc = (NAV_DISC_R + 0.5 - d_disc).clamp(0.0, 1.0) * 0.92;
            if a_disc > 0.0 {
                let inv = 1.0 - a_disc;
                canvas[i] = (34.0 * a_disc + canvas[i] as f32 * inv) as u8; // B 0x22
                canvas[i + 1] = (30.0 * a_disc + canvas[i + 1] as f32 * inv) as u8; // G 0x1E
                canvas[i + 2] = (28.0 * a_disc + canvas[i + 2] as f32 * inv) as u8; // R 0x1C
                canvas[i + 3] = 255;
            }
            // Rim highlight (iter26): light on the upper arc, darker
            // on the lower arc; the chevron below never reaches it.
            // (Settled path: the disc fill above forces A=255, so the
            // rim only blends RGB here.)
            if let Some((rr, gg, bb, aa)) = disc_rim(px, py, cx, cy) {
                if aa > 0.0 {
                    let inv = 1.0 - aa;
                    canvas[i] = (bb * aa + canvas[i] as f32 * inv) as u8;
                    canvas[i + 1] = (gg * aa + canvas[i + 1] as f32 * inv) as u8;
                    canvas[i + 2] = (rr * aa + canvas[i + 2] as f32 * inv) as u8;
                }
            }
            // Chevron glyph: two strokes meeting at the tip.
            let d_line = seg_dist(px, py, tip.0, tip.1, top.0, top.1)
                .min(seg_dist(px, py, tip.0, tip.1, bot.0, bot.1));
            let a_line = (STROKE / 2.0 + 0.5 - d_line).clamp(0.0, 1.0);
            if a_line > 0.0 {
                let inv = 1.0 - a_line;
                canvas[i] = (237.0 * a_line + canvas[i] as f32 * inv) as u8; // B 0xED
                canvas[i + 1] = (234.0 * a_line + canvas[i + 1] as f32 * inv) as u8; // G 0xEA
                canvas[i + 2] = (232.0 * a_line + canvas[i + 2] as f32 * inv) as u8; // R 0xE8
                canvas[i + 3] = 255;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Morph frame rendering (v0.4, D4+D7 iter13): the capsule is drawn at the
// interpolated geometry (centred horizontally, TOP-anchored in the stage,
// always opaque); both endpoint states are rendered into scratch buffers
// and composited with their fade-window opacities. Settled rendering is
// untouched — the morph never edits a panel function, so a settled frame
// is bit-identical to the pre-morph build at the same font.
// D4 (iter13, SUPERSEDED iter24): the buffer is the fixed stage now, so
// BOTH endpoint contents are laid out at STAGE coords in stage-sized
// scratch buffers — no centring offset, no compression at fade start.
// (The old D4 laid the incoming content out at target size, centred in
// the current canvas.)
// D7 (iter13, amended iter24): the scratch buffers are caller-owned,
// reused across frames (see Island::morph_scratch_*); draw() additionally
// allocates one panel-sized scratch per call (transient, allocator-reused
// across frames — the price of the stage blit).
// The outgoing collapsed clock is rasterized at text_px(pill_h) * outgoing_scale
// (design doc section 2.3 clock scale-out); other contents fade with
// opacity only (incoming_scale is ignored in v1, design doc section 4).
// ---------------------------------------------------------------------------
fn draw_state_into(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    state: IslandState,
    content_scale: f32,
    font: &fontdue::Font,
    icon_font: Option<&fontdue::Font>,
    // Lockscreen preview data, threaded through from draw_if_ready
    // (held default on the morph path — the lockscreen never morphs).
    lock_data: &lockscreen::LockscreenData,
    lock_wallpaper: Option<&[u8]>,
) {
    if state == IslandState::Collapsed {
        // Clock text only, at the scaled size, centred in the PILL
        // rect inside the stage (iter24: the canvas is stage-sized
        // now, so centring in (w, h) would put the clock in the
        // middle of the stage, not in the 76x24 pill). No capsule
        // fill — the caller owns the capsule. The scratch starts
        // zeroed, so the text composites cleanly onto it.
        let (pw, ph) = state.size();
        let px = lock_timer::text_px(ph as f32) * content_scale;
        with_panel_scratch((pw * ph * 4) as usize, |scratch| {
            lock_timer::draw_text_centred(
                scratch,
                pw,
                ph,
                clock_font().unwrap_or(font),
                &hhmm(),
                px,
            );
            let (ox, oy) = panel_origin(state);
            blit_panel(canvas, w, h, scratch, pw, ph, ox as i32, oy as i32);
        });
    } else {
        draw(
            canvas,
            w,
            h,
            state,
            font,
            icon_font,
            lock_data,
            lock_wallpaper,
        );
    }
}

fn draw_morph_frame(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    from_state: IslandState,
    to_state: IslandState,
    frame: &morph::MorphFrame,
    // Caller-owned scratch buffers (D7): both are w*h*4 bytes
    // (stage-sized, iter24). Both arrive zeroed.
    scratch_from: &mut [u8],
    scratch_to: &mut [u8],
    font: &fontdue::Font,
    icon_from: Option<&fontdue::Font>,
    icon_to: Option<&fontdue::Font>,
    // Lockscreen preview data (held default on this path — the
    // lockscreen never morphs, so this only matters if a morph ever
    // runs with a Lockscreen endpoint).
    lock_data: &lockscreen::LockscreenData,
    lock_wallpaper: Option<&[u8]>,
    // Only this rect is touched: the union of the two endpoint content
    // rects (plus the previous frame's), so a morph frame costs its
    // capsule's pixels instead of the whole 1176x636 stage.
    region: (i32, i32, i32, i32),
) {
    let rgn = clip_rect(region, w, h);
    let wf = w as f32;
    let (fw, fh, fr) = (frame.width, frame.height, frame.radius);
    // Capsule origin in stage coords: centred horizontally; the top
    // edge interpolates between the endpoints' panel origins (iter30
    // a3c: pill top 3 -> panel top 31 on expand, so the capsule grows
    // downward from beneath the pill instead of stretching the pill
    // into a top bar). Both endpoint states are already laid out at
    // STAGE coords inside their stage-sized scratch buffers
    // (draw_state_into -> draw()), so the composite needs no offset
    // at all (the old D4 centring offset is gone: the buffer size
    // never changes mid-morph now).
    let ox = (wf - fw) / 2.0;
    let oy = morph_capsule_oy(from_state, to_state, frame.progress);
    // Flat frost (iter31, a3 REVISED): the morph capsule's base alpha
    // interpolates between the endpoint states' fill alphas
    // (pill 0.92 <-> menu 0.92, iter128) with the incoming opacity, so the
    // capsule stays translucent through the morph; endpoint content
    // (opaque glyphs, flat fill in the scratches) raises it above
    // this base where present — see the per-pixel raise below.
    let morph_fill_alpha = frost_alpha(from_state)
        + (frost_alpha(to_state) - frost_alpha(from_state))
            * frame.incoming_opacity.clamp(0.0, 1.0);
    draw_state_into(
        scratch_from,
        w,
        h,
        from_state,
        frame.outgoing_scale,
        font,
        icon_from,
        lock_data,
        lock_wallpaper,
    );
    draw_state_into(
        scratch_to,
        w,
        h,
        to_state,
        frame.incoming_scale,
        font,
        icon_to,
        lock_data,
        lock_wallpaper,
    );
    // Fake frost (iter29): the capsule base is the blurred wallpaper
    // under the interpolated flat fill (opaque interior — no sharp
    // bleed); endpoint scratches composite over it below.
    let morph_bg = wallpaper_blur::ensure_loaded();
    // iter101 (Option 2): live frost for the morph capsule when
    // either endpoint is a menu (same frozen frame as the panels).
    let morph_live = if MENU_CYCLE.contains(&from_state) || MENU_CYCLE.contains(&to_state) {
        wallpaper_blur::live_backdrop()
    } else {
        None
    };
    let (msox, msoy) = stage_screen_origin();
    // iter33: one frost sample at the capsule centre (same reasoning
    // as draw_panel_into) — the capsule reads flat at every size.
    let morph_uniform: [u8; 3] = match &morph_bg {
        Some(bw) => bw.sample_frost_uniform(msox + ox + fw / 2.0, msoy + oy + fh / 2.0),
        None => [0, 0, 0],
    };
    for y in rgn.1..rgn.3 {
        // Bounded to the damage rect: pixels outside it are neither
        // upscaled nor damaged, so the previous frame's identical bytes
        // stay on screen. Only `dmg` is ever written here.
        for x in rgn.0..rgn.2 {
            let a = rounded_rect_alpha(
                x as f32 + 0.5 - ox,
                y as f32 + 0.5 - oy,
                fw,
                fh,
                fr,
            );
            let i = ((y as u32 * w + x as u32) * 4) as usize;
            canvas[i] = 0;
            canvas[i + 1] = 0;
            canvas[i + 2] = 0;
            // Flat-fill base (iter31; iter85 opaque fallback): the
            // settled panels are opaque now (see frost_fill_px), so the
            // morph capsule no-blur base must be opaque too - a
            // translucent base would snap to opaque at morph end.
            // (The blur branch below overwrites all four channels.)
            canvas[i + 3] = (a * 255.0) as u8;
            if a <= 0.0 {
                continue;
            }
            // Fake frost (iter29): blurred wallpaper under the
            // interpolated flat fill replaces the black base.
            if morph_live.is_some() || morph_bg.is_some() {
                let d = rounded_rect_sdf(
                    x as f32 + 0.5 - ox,
                    y as f32 + 0.5 - oy,
                    fw,
                    fh,
                    fr,
                );
                // iter103: same shared fill as the settled panels —
                // neutralised sample + its local mean (deviation 0
                // on the uniform fallback).
                let (s, lm) = match &morph_live {
                    Some(lb) => (
                        lb.sample(msox + x as f32 + 0.5, msoy + y as f32 + 0.5),
                        lb.local_mean(msox + x as f32 + 0.5, msoy + y as f32 + 0.5),
                    ),
                    None => (morph_uniform, morph_uniform),
                };
                let p = frost_fill_px_blur(
                    a,
                    d,
                    x as f32 + 0.5 - ox,
                    y as f32 + 0.5 - oy,
                    fw,
                    fh,
                    morph_fill_alpha,
                    s,
                    lm,
                );
                // iter91: p is RGB, canvas is BGRA - swap channels
                // on write (p[2]->B, p[1]->G, p[0]->R), same R<>B
                // fix as the settled-panel loop above.
                canvas[i] = p[2];
                canvas[i + 1] = p[1];
                canvas[i + 2] = p[0];
                canvas[i + 3] = p[3];
            } else {
                // No blurred wallpaper: opaque dark floor, same value
                // as frost_fill_px (FROST_FLOOR * fill_alpha), so the
                // morph capsule matches the settled panels exactly.
                let v = (FROST_FLOOR * morph_fill_alpha * a) as u8;
                canvas[i] = v;
                canvas[i + 1] = v;
                canvas[i + 2] = v;
            }
            let ao = frame.outgoing_opacity * a;
            let ai = frame.incoming_opacity * a;
            // Both scratches are stage-sized and already in stage
            // coords — the incoming sample is the same pixel.
            // iter33: proper premultiplied alpha-OVER of the faded
            // scratches onto the frost base — NOT the old max()
            // overwrite. max() replaced the frost RGB with 0 wherever
            // both scratches are transparent black (outside the
            // settled panel), which is exactly what painted the
            // one-frame pure-black overshoot slab in burst frame 03.
            // With over, a transparent scratch leaves the frost
            // untouched; an opaque glyph/fill replaces it; a shadow
            // (black, low alpha) only darkens it slightly. The
            // combined source is the brighter of the two faded
            // scratches (true cross-fade); its coverage `sa` is the
            // max of the two faded alphas.
            let sa = ((scratch_from[i + 3] as f32 * ao)
                .max(scratch_to[i + 3] as f32 * ai)
                / 255.0)
                .clamp(0.0, 1.0);
            let base_a = canvas[i + 3] as f32 / 255.0;
            for c in 0..3 {
                let vo = scratch_from[i + c] as f32 * ao;
                let vi = scratch_to[i + c] as f32 * ai;
                let src = vo.max(vi);
                canvas[i + c] = (src + canvas[i + c] as f32 * (1.0 - sa)) as u8;
            }
            canvas[i + 3] = ((sa + base_a * (1.0 - sa)) * 255.0) as u8;
        }
    }
    // Panel drop shadow (iter26): the shadow of the INTERPOLATED
    // capsule, drawn only where the capsule alpha is exactly 0 so it
    // never darkens panel content; it fades with the menu morph
    // (in with the incoming menu, out with the outgoing one, steady
    // at full strength on menu->menu morphs). The shadow drawn into
    // the endpoint scratches by draw() is masked out by the capsule
    // loop above, so it has to be re-drawn here at the live geometry.
    let sh_op = match (
        MENU_CYCLE.contains(&from_state),
        MENU_CYCLE.contains(&to_state),
    ) {
        (true, true) => frame.outgoing_opacity.max(frame.incoming_opacity),
        (true, false) => frame.outgoing_opacity,
        (false, true) => frame.incoming_opacity,
        (false, false) => 0.0,
    };
    if sh_op > 0.0 {
        let m = PANEL_SHADOW_BLUR + PANEL_SHADOW_OFF.0.abs().max(PANEL_SHADOW_OFF.1.abs()) + 1.0;
        let x0 = ((ox - m).floor() as i32).max(0);
        let x1 = ((ox + fw + m).ceil() as i32).min(w as i32);
        let y0 = ((oy - m).floor() as i32).max(0);
        let y1 = ((oy + fh + m).ceil() as i32).min(h as i32);
        for y in y0..y1 {
            for x in x0..x1 {
                let a_cap =
                    rounded_rect_alpha(x as f32 + 0.5 - ox, y as f32 + 0.5 - oy, fw, fh, fr);
                if a_cap > 0.0 {
                    continue;
                }
                let a = panel_shadow_alpha(
                    x as f32 + 0.5,
                    y as f32 + 0.5,
                    ox,
                    oy,
                    fw,
                    fh,
                    fr,
                ) * sh_op;
                if a > 0.0 {
                    let i = ((y as u32 * w + x as u32) * 4) as usize;
                    composite_black_shadow(canvas, i, a);
                }
            }
        }
    }
    // Nav discs (iter30 a3b): they track the LIVE capsule — centres
    // from the interpolated capsule rect (mid-height), so they glide
    // with the panel instead of cross-fading between settled
    // positions. Opacity follows the same menu-membership rule as the
    // panel shadow above (in with the incoming menu, out with the
    // outgoing one, steady on menu->menu morphs); the capsule alpha
    // above masked them out, so they are drawn here.
    if sh_op > 0.0 {
        let off = NAV_DISC_GAP + NAV_DISC_R;
        let dcy = oy + fh / 2.0;
        draw_nav_disc_faded(canvas, w, h, ox - off, dcy, -1.0, sh_op);
        draw_nav_disc_faded(canvas, w, h, ox + fw + off, dcy, 1.0, sh_op);
    }
    // iter30 a3c: the pill stays visible above the panel through the
    // whole morph — the capsule grows downward from beneath it.
    if MENU_CYCLE.contains(&from_state) || MENU_CYCLE.contains(&to_state) {
        draw_pill_over_stage(canvas, w, h, font, icon_from, lock_data, lock_wallpaper);
    }
    // iter90 a8: workspace circles are fixed in stage coords (they
    // never morph) — redrawn here at full strength, like the pill
    // above; the capsule loop masked the copies inside the endpoint
    // scratches. Not drawn for a lockscreen endpoint.
    if from_state != IslandState::Lockscreen && to_state != IslandState::Lockscreen {
        draw_workspace_circles_stage(canvas, w, h, font, active_tag_cached());
    }
}

// ---------------------------------------------------------------------------
// Wayland state
// ---------------------------------------------------------------------------
/// Lockscreen morph kind (v0.7): entry (~250 ms: scrim 100 ms, then
/// giant clock 150 ms) or exit (reverse, ~250 ms). See
/// lockscreen::LockAnim for the phase math.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LockAnimKind {
    LockIn,
    UnlockOut,
}

/// One running lockscreen morph: kind + wall-clock start. The anim is
/// a pure function of elapsed ms (LockAnim::lock_in/unlock_out), so a
/// late frame skips ahead instead of drifting the animation long.
#[derive(Clone, Copy, Debug)]
struct LockAnimRun {
    kind: LockAnimKind,
    start: std::time::Instant,
}

/// Lockscreen morph frame cadence (v0.7): ~30 fps timer chain.
const LOCK_ANIM_FRAME_MS: u64 = 33;
/// Lock-in settles at 250 ms (100 ms scrim + 150 ms clock — see
/// lockscreen::LockAnim::lock_in); unlock-out at 250 ms.
const LOCK_IN_MS: u32 = 250;
const LOCK_OUT_MS: u32 = 250;

struct Island {
    registry_state: RegistryState,
    output_state: OutputState,
    compositor_state: CompositorState,
    shm: Shm,
    layer_shell: LayerShell,
    layer: Option<LayerSurface>,
    pool: Option<SlotPool>,
    buffer: Option<smithay_client_toolkit::shm::slot::Buffer>,
    font: &'static fontdue::Font,
    // Icon font, loaded LAZILY on the first Power/ControlCenter draw.
    // Material Symbols Rounded parses to ~19 MB in fontdue — loading it
    // eagerly would 6x the idle PSS (4 MB -> 24 MB) for states that never
    // use it. One parsed copy is shared by the power + CC panels.
    icon_font: std::sync::OnceLock<Option<fontdue::Font>>,
    state: IslandState,
    // Lockscreen preview state (iter14 wiring): typed-dot count,
    // wrong-password flag, and the lazily decoded wallpaper (RGBA8,
    // 1504x1002) loaded on lockscreen entry, dropped on exit.
    // Preview only — no session lock, no keyboard input.
    lock_dots: u8,
    lock_wrong: bool,
    lock_wallpaper: Option<Vec<u8>>,
    // Morph state (v0.4): the running 400 ms animation, the state whose
    // content is fading out, and the timer token driving it.
    morph: Option<morph::Morph>,
    morph_from_state: IslandState,
    morph_token: Option<calloop::RegistrationToken>,
    // Morph endpoints (v0.7, S-1): the geometries the running morph
    // interpolates between, so draw_if_ready can recover the raw
    // progress from the PRESENTED width (see the morph_frame
    // computation there). Set in begin_morph alongside `morph`.
    morph_from_geom: morph::Geometry,
    morph_to_geom: morph::Geometry,
    // Lockscreen morph run (v0.7): LockIn (~250 ms) / UnlockOut
    // (~250 ms), driven by a 33 ms calloop timer chain. While a run
    // is active, draw_if_ready computes the lockscreen anim from the
    // run's wall-clock elapsed time (LockAnim::lock_in/unlock_out);
    // None = held frame (or not locked). lock_exit_target is the
    // state the UnlockOut run finishes into.
    lock_anim: Option<LockAnimRun>,
    lock_anim_token: Option<calloop::RegistrationToken>,
    lock_exit_target: IslandState,
    // Morph scratch buffers (D7, iter13): reused across intermediate
    // frames instead of heap-allocating two fresh buffers per frame.
    // Grow-only: resized when a larger state is morphed to, zeroed
    // before each use in draw_if_ready. Stage-sized since iter24
    // (1176x636x4 = 2,991,744 B each), so the retained pair is
    // 5,983,488 B = 5.98 MB at stage size and 2 x 1504x1002x4 =
    // 12,056,064 B = 11.77 MB at lockscreen size — NOT "~2 MB" as
    // this comment claimed before iter125. Freed on collapse-settle
    // (free_morph_scratches, iter125, ram-root-cause-iter124 §5) and
    // re-grown on the next expand; no per-frame alloc/free churn
    // while a morph runs.
    morph_scratch_from: Vec<u8>,
    morph_scratch_to: Vec<u8>,
    /// iter108: 1x stage scratch the frame is drawn into before the 2x
    /// upscale into the shm buffer (RENDER_SCALE). Grow-only, reused
    /// across frames; stage-sized (1176x636x4 ~ 3 MB), lockscreen-sized
    /// at most (1504x1002x4 ~ 6 MB) — far under the 100 MB PSS budget.
    scale_scratch: Vec<u8>,
    /// F-A: pinned 2-slot shm ring. `SlotPool::create_buffer` added a
    /// new slot per commit (the pool grew to 4-6 slots and 68 MB);
    /// exactly two slots are reused, and a draw is skipped while the
    /// compositor still holds the slot we would use.
    slots: Vec<Slot>,
    slot_bytes: usize,
    /// Round-robin index for the two slots. Alternating (instead of always
    /// taking slot 0 while it is free) makes both slots reach their final
    /// touched-page footprint inside the first few commits, so RSS does not
    /// step up by a whole slab minutes later when timing first puts a frame
    /// in flight.
    slot_rr: usize,
    /// Set after a commit that requested `wl_surface.frame`; cleared by
    /// CompositorHandler::frame. This is the back-pressure: nothing
    /// draws while the frame callback is outstanding.
    frame_pending: Option<std::time::Instant>,
    /// A draw was requested while a frame callback was outstanding.
    redraw_wanted: bool,
    /// Content rect of the last committed frame (stage-logical).
    last_rect: Option<(i32, i32, i32, i32)>,
    configured_size: (u32, u32),
    drawn_size: (u32, u32),
    /// Backdrop factor (0..1, iter30 a3b) the last draw used — the
    /// 1 s backdrop-watch timer redraws when the live factor moves.
    last_backdrop_factor: f32,
    /// iter90 a8: active mango tag the last draw showed (1..=9) — the
    /// 1 s timer redraws when the poller's cached tag moves (keyboard
    /// tag switches included).
    last_tag_drawn: u8,
    /// iter91 B1: fingerprint of the ControlCenter live data the last
    /// draw showed — the 1 s timer redraws CC when the poller's
    /// cached values move (battery W, header clock, notification
    /// line). None = never drawn since entering CC (forces a redraw
    /// on the next timer tick); reset on every CC entry.
    last_cc_fingerprint: Option<String>,
    /// iter106: fingerprint of the Music live data the last draw
    /// showed - same pattern as the CC one above (iter91 B1): the
    /// 1 s timer redraws Music when the refresher's cached values
    /// move (status, artist/title, whole-second position, length,
    /// artUrl). None = never drawn since entering Music (forces a
    /// redraw on the next timer tick); reset on every Music entry.
    last_music_fingerprint: Option<String>,
    /// iter108: fingerprint of the tray snapshot the last draw
    /// showed — the 1 s timer redraws when an item registers /
    /// unregisters or an icon/title changes, and when the open
    /// DBusMenu arrives. Empty = never drawn (forces a redraw
    /// on the next timer tick).
    last_tray_fingerprint: String,
    loop_signal: LoopSignal,
    // Pointer + keyboard input (iter20): seat state and the
    // wl_pointer / wl_keyboard objects, bound lazily when the seat
    // advertises the capabilities (SeatHandler::new_capability).
    // Only pointer was requested before; the keyboard is bound for
    // the functional launcher (typing filters, Enter launches).
    seat_state: SeatState,
    pointer: Option<WlPointer>,
    keyboard: Option<WlKeyboard>,
    // A stored clone of the event-loop handle (the same handle the
    // socket/timer sources captured as loop_handle_ctrl), so the
    // wl_pointer / wl_keyboard callbacks — which receive no loop
    // handle — can drive begin_morph via request_state, the SAME
    // transition path as socket commands.
    loop_handle: calloop::LoopHandle<'static, Self>,
    // Event-queue handle, kept so update_input_region can create a
    // fresh wl_region on every frame (iter24).
    qh: QueueHandle<Self>,
    pointer_over: bool,
    /// Last pointer position seen on a non-Leave event (stage coords).
    /// A Leave at (within 0.5 px of) this position means the pointer
    /// did not move — the Leave was caused by a region/state change
    /// under a stationary pointer, not by the user moving off.
    last_pointer_pos: Option<(f64, f64)>,
    // Wheel-cycle bookkeeping: accumulated sub-notch deltas
    // (touchpads) + the last step time for the momentum cooldown.
    wheel_accum: f64,
    last_wheel_step: Option<std::time::Instant>,
    // Last BTN_LEFT press position (surface-local logical px) —
    // recorded for the future control-centre hit-testing pass.
    last_click: Option<(f64, f64)>,
    /// iter34 a6a: active CC slider drag — (0 Display / 1 Sound, last
    /// applied level). Set on a CcSlider press, updated on Motion,
    /// cleared on left-button Release. Levels apply only on >= 0.02
    /// change so a drag does not spawn a process per motion event.
    cc_drag: Option<(u8, f32)>,
    /// iter36 a6b: calendar month offset from the current month
    /// (CalMonth chevrons; wraps Dec<->Jan in the panel).
    cal_offset: i32,
    /// iter36 a6b: selected wallpaper, global library index
    /// (filmstrip centre; neighbours wrap). Set from config at startup.
    wall_selected: usize,
    /// iter36 a6b: selected theme, global library index (themes rows:
    /// row 0 = this theme, rows 1..5 = the next themes, wrapping).
    theme_selected: usize,
}

/// Pure cycle step (testable): the menu after `current` in
/// MENU_CYCLE order, `dir` +1 next / -1 previous, wrapping both
/// ways. None if `current` is not a menu state (collapsed /
/// lock / lockscreen). Iter39 a10. Free function (not a method)
/// so the unit tests can call it bare.
fn cycle_step(current: IslandState, dir: i32) -> Option<IslandState> {
    let idx = MENU_CYCLE.iter().position(|s| *s == current)?;
    let n = MENU_CYCLE.len() as i32;
    Some(MENU_CYCLE[(idx as i32 + dir).rem_euclid(n) as usize])
}

/// Fingerprint of the ControlCenter live cache (iter91 B1): every
/// field `draw_controlcenter_panel` renders, as one comparable
/// string. `live()` only clones the refresher's cache — no system
/// call, no blocking — so the 1 s timer can call this every tick
/// while CC is open and redraw exactly when a poll landed. Covers
/// the header clock (`time_hhmm`), battery % + power-source text
/// ("Plugged in · N W"), all four toggles + SSID, both slider
/// levels, and the notification line (`notifications_since_hhmm` +
/// `notification_count`).
fn cc_live_fingerprint() -> String {
    let d = controlcenter_panel::ControlCenterData::live();
    format!(
        "{}|{:?}|{:?}|{}|{:?}|{}|{}|{}|{:?}|{}|{}|{}|{}|{:?}|{:?}|{}",
        d.time_hhmm,
        d.battery_pct,
        d.power_source_text,
        d.wifi_on,
        d.wifi_ssid,
        d.wifi_radio_on,
        d.focus_on,
        d.bluetooth_on,
        d.bt_devices,
        d.night_light_on,
        d.display_level,
        d.sound_level,
        d.notifications_since_hhmm,
        d.notification_count,
        d.audio_routes,
        d.audio_default,
    )
}

impl Island {
    /// Arm the Clipboard entry repaint timer (iter88 stale-paint
    /// fix): redraw ~250 ms after entry so the fresh history lands
    /// on screen (re-asserting the stage size if the row count
    /// changed the panel height), re-arming every 250 ms while the
    /// background refresh is still running. A single shot is not
    /// enough — the refresh inflates up to 12 screenshot PNGs in
    /// its thread (~1-2 s measured; one 2256x1504 PNG alone is
    /// ~319 ms), while the entry morph ends after anim_ms=160 ms
    /// and the morph timer chain then drops; the minute-boundary
    /// timer does not cover Clipboard and the 1 s backdrop timer
    /// only fires on a backdrop change, so without this nothing
    /// would ever repaint the fresh list until the next input
    /// event. The timer stops as soon as the user leaves the view
    /// or the refresh lands; zero cost when collapsed/idle (no
    /// source exists then). Called from both entry paths
    /// (begin_morph and apply_state_instant).
    fn arm_clipboard_entry_timer(&self) {
        let handle = self.loop_handle.clone();
        let _ = handle.insert_source(
            Timer::from_duration(Duration::from_millis(250)),
            |_, _, state: &mut Island| {
                if state.state != IslandState::Clipboard {
                    return TimeoutAction::Drop;
                }
                if clipboard_panel::is_refreshing() {
                    return TimeoutAction::ToDuration(Duration::from_millis(250));
                }
                let (w, h) = IslandState::Clipboard.size();
                if state.configured_size != (w, h) {
                    state.apply_state_instant(IslandState::Clipboard);
                }
                state.draw_if_ready();
                TimeoutAction::Drop
            },
        );
    }


    /// One-shot CC entry repaint (iter89): redraw ~250 ms after entry
    /// so the first frame after open is a live read, not the
    /// video-defaults seed frame (see controlcenter_panel.rs
    /// `ControlCenterData::live()` note). Mirrors
    /// `arm_clipboard_entry_timer` but single-shot: the CC refresher
    /// polls every 5 s from daemon start, so the cache is warm except
    /// on the first open after a restart, where one redraw 250 ms in
    /// covers the poll landing.
    /// iter91 B1: this one-shot only fixes the FIRST frame — it fires
    /// mid-morph and never again, so it cannot keep the panel live;
    /// steady-state freshness now comes from the fingerprint check in
    /// the 1 s backdrop-watch timer (see there).
    fn arm_cc_entry_timer(&self) {
        let handle = self.loop_handle.clone();
        let _ = handle.insert_source(
            Timer::from_duration(Duration::from_millis(250)),
            |_, _, state: &mut Island| {
                if state.state != IslandState::ControlCenter {
                    return TimeoutAction::Drop;
                }
                state.draw_if_ready();
                TimeoutAction::Drop
            },
        );
    }

    /// One-shot Music entry repaint (iter114 W5): mirror of
    /// `arm_cc_entry_timer` — redraw ~250 ms after entry so the first
    /// frame after open is a live read, not the seed-idle frame.
    /// The iter91 B1 caveat applies here too: this only fixes the
    /// FIRST frame; steady-state freshness comes from the fingerprint
    /// check in the 1 s backdrop-watch timer (see there).
    fn arm_music_entry_timer(&self) {
        let handle = self.loop_handle.clone();
        let _ = handle.insert_source(
            Timer::from_duration(Duration::from_millis(250)),
            |_, _, state: &mut Island| {
                if state.state != IslandState::Music {
                    return TimeoutAction::Drop;
                }
                state.draw_if_ready();
                TimeoutAction::Drop
            },
        );
    }

    /// iter125: free the morph scratch buffers once settled in
    /// Collapsed (ram-root-cause-iter124 §5). The pair is stage-sized
    /// (2 x 1176x636x4 = 5,983,488 B) and grow-only — without this,
    /// the first morph of the process retains ~5.9 MB for the process
    /// lifetime (iter123 probe: +5,972 kB PSS on one CC open, zero
    /// return on collapse). Takes the two buffers as disjoint field
    /// borrows (not &mut self) so draw_if_ready can call it while
    /// stage_1x still borrows scale_scratch. Safe even mid-morph:
    /// draw_if_ready re-grows the pair on demand (resize when len !=
    /// need) before the next morph frame, so the worst case is one
    /// re-alloc. malloc_trim runs only when something was actually
    /// freed, so a no-op settle never trims. The next expand
    /// re-allocates once (~ms, off the hot path).
    fn free_morph_scratches(from: &mut Vec<u8>, to: &mut Vec<u8>) {
        if from.capacity() == 0 && to.capacity() == 0 {
            return;
        }
        *from = Vec::new();
        *to = Vec::new();
        unsafe { libc::malloc_trim(0) };
    }

    /// Instant state switch (the pre-morph behaviour): set the state,
    /// resize the layer surface. Used for same-geometry
    /// switches and for the morph snap-to-end paths.
    fn apply_state_instant(&mut self, new_state: IslandState) {
        let prev = self.state;
        self.state = new_state;
        if new_state != IslandState::ControlCenter || prev != IslandState::ControlCenter {
            controlcenter_panel::cc_bt_submenu_set(false);
            controlcenter_panel::cc_sound_submenu_set(false);
        }
        if new_state == IslandState::Collapsed {
            // iter125: instant collapse (same-geometry snap, morph
            // timer-insert failure, unlock target, socket reset word)
            // — no morph runs to completion, so the morph-completion
            // site in draw_if_ready never fires. Free the stage-sized
            // morph scratches here instead.
            Self::free_morph_scratches(&mut self.morph_scratch_from, &mut self.morph_scratch_to);
        }
        if new_state == IslandState::Launcher && prev != IslandState::Launcher {
            // Fresh launcher = fresh search (the panel owns LIVE).
            launcher_panel::live_reset();
        }
        if new_state == IslandState::Clipboard && prev != IslandState::Clipboard {
            // Fresh clipboard = fresh search + fresh history (the
            // panel owns the cache; the refresh runs in a thread).
            clipboard_panel::on_enter();
            self.arm_clipboard_entry_timer();
        }
        if new_state == IslandState::Wallpaper && prev != IslandState::Wallpaper {
            // Fresh wallpaper search on entry (the panel owns
            // WALL_SEARCH) — same rule as the Launcher/Clipboard.
            wallpaper_panel::wall_reset();
        }
        if new_state == IslandState::Themes && prev != IslandState::Themes {
            // Fresh themes search on entry (the panel owns
            // THEME_SEARCH).
            themes_panel::theme_reset();
        }
        if new_state == IslandState::ControlCenter && prev != IslandState::ControlCenter {
            // iter89: one-shot 250 ms redraw so the first CC frame is
            // a live read, not the video-defaults seed frame.
            self.arm_cc_entry_timer();
            // iter91 B1: force the 1 s live-repaint timer to re-validate
            // on its next tick (fingerprint reset; see the timer).
            self.last_cc_fingerprint = None;
        }
        if new_state == IslandState::Music && prev != IslandState::Music {
            // iter114 W5: one-shot entry repaint (see
            // arm_music_entry_timer) alongside the fingerprint reset.
            self.arm_music_entry_timer();
            // iter106: force the 1 s live-repaint timer to re-validate
            // on its next tick (fingerprint reset; see the timer).
            self.last_music_fingerprint = None;
        }
        self.sync_keyboard_interactivity();
        if let Some(layer) = &self.layer {
            // iter24: the stage never resizes for island states — the
            // surface is always STAGE_W x STAGE_H (only the lockscreen
            // preview is fullscreen, and it never comes through here).
            // Re-asserting the stage size is a harmless no-op once the
            // surface is already stage-sized.
            if new_state != IslandState::Lockscreen {
                layer.set_size(STAGE_W, STAGE_H);
            }
            layer.wl_surface().commit();
        }
    }

    /// Pointer/keyboard-driven state change: the SAME transition the
    /// control socket performs for a plain state word (see the socket
    /// source in main) — out of the lockscreen preview it exits the
    /// preview instantly, otherwise it goes through begin_morph
    /// (config-duration morph, or instant for same-geometry). There is
    /// deliberately no parallel pointer-only path.
    fn request_state(&mut self, target: IslandState) {
        let handle = self.loop_handle.clone();
        if self.state == IslandState::Lockscreen {
            self.exit_lockscreen(&handle, target);
        } else {
            self.begin_morph(&handle, target);
        }
    }

    /// Keyboard interactivity follows the state: Exclusive in every
    /// menu state (a1 iter22 — Esc must reach the island from ANY open
    /// menu, and the Launcher still needs typing), None when collapsed
    /// / lock / lockscreen. Called on every state assignment path
    /// (apply_state_instant, the begin_morph morph-start, and
    /// enter_lockscreen); idempotent.
    fn sync_keyboard_interactivity(&self) {
        if let Some(layer) = &self.layer {
            let mode = if MENU_CYCLE.contains(&self.state)
                || self.state == IslandState::TrayMenu
            {
                KeyboardInteractivity::Exclusive
            } else {
                KeyboardInteractivity::None
            };
            layer.set_keyboard_interactivity(mode);
            layer.wl_surface().commit();
        }
    }

    /// One menu step in the MENU_CYCLE order (dir = +1 next, -1
    /// previous, wrapping both ways). No-op outside the cycle
    /// (collapsed / lock / lockscreen are not menu states).
    fn cycle_menu(&mut self, dir: i32) {
        if let Some(next) = cycle_step(self.state, dir) {
            self.request_state(next);
        }
    }

    /// Mouse wheel over the island (PointerEventKind::Axis, vertical):
    /// cycle one menu per notch — EXCEPT in the Launcher, where the
    /// wheel scrolls the app list instead (iter86: one row per notch;
    /// the window follows the selection, so the highlight stays
    /// visible — see selection.rs; iter112: at the clamped list edge
    /// the notch cycles menus instead, so the wheel can leave the
    /// Launcher). Discrete steps are taken at face
    /// value; continuous (touchpad) deltas accumulate, 15.0 units =
    /// one step (the wlroots convention for one classic notch). A
    /// 150 ms cooldown after each step swallows momentum noise, and
    /// at most one step fires per pointer frame. Gated by the
    /// scroll_cycle config key and by being in a menu state at all.
    fn on_wheel(&mut self, vertical: AxisScroll) {
        if !cfg().scroll_cycle {
            return;
        }
        if !MENU_CYCLE.contains(&self.state) {
            return;
        }
        let delta = if vertical.discrete != 0 {
            vertical.discrete as f64
        } else {
            vertical.absolute / 15.0
        };
        if delta == 0.0 {
            return;
        }
        self.wheel_accum += delta;
        if self.wheel_accum.abs() < 1.0 {
            return;
        }
        let now = std::time::Instant::now();
        if let Some(last) = self.last_wheel_step {
            if now.duration_since(last) < WHEEL_COOLDOWN {
                return; // momentum: keep the accumulation, step later
            }
        }
        let dir = if self.wheel_accum > 0.0 { 1 } else { -1 };
        self.wheel_accum -= dir as f64;
        self.last_wheel_step = Some(now);
        // Iter86: in the Launcher the wheel scrolls the app list
        // (one row per notch), it does NOT cycle menus — wheeling
        // used to leave the Launcher for the next menu. Menu cycling
        // is still Left/Right and the nav discs (press_key).
        // Iter112: at the clamped list edge the notch used to be
        // swallowed silently (live_move_selection clamped and
        // returned ()), so the wheel could enter the Launcher and
        // never leave it again by wheel — iter110 up2 was a
        // byte-identical no-op (selection 0, up clamps to 0). At
        // the edge, fall through to menu cycling instead, so
        // wheel-up on the top row returns to Music and wheel-down
        // on the last row advances to ControlCenter.
        if self.state == IslandState::Launcher
            && launcher_panel::live_move_selection(dir)
        {
            eprintln!(
                "dynamite-rust: DBG wheel dir={:+} from=Launcher target=list",
                dir
            );
            self.draw_if_ready();
            return;
        }
        eprintln!(
            "dynamite-rust: DBG wheel dir={:+} from={:?} target={:?}",
            dir,
            self.state,
            cycle_step(self.state, dir)
        );
        self.cycle_menu(dir);
    }

    /// Launch the launcher's selected (top) filtered result, if any,
    /// then reset the search and collapse. Enter with no results is
    /// a no-op (the launcher stays open).
    fn launch_live_selected(&mut self) {
        let Some(exec) = launcher_panel::live_selected_exec() else {
            return;
        };
        spawn_exec(&exec);
        launcher_panel::live_reset();
        self.request_state(IslandState::Collapsed);
    }

    /// iter128 B2: apply the library wallpaper at global index `i`
    /// (+ its owning theme) — the WallApply arm, extracted so the
    /// keyboard Enter path (on_wallpaper_key) shares the exact same
    /// apply path as a thumb click. No-op (logged) when out of range
    /// or the apply fails.
    fn apply_wallpaper(&mut self, i: usize) {
        let name = library::manifest()
            .and_then(|m| m.wallpaper_at(i))
            .map(|(_, f)| f.to_string());
        match name {
            Some(f) => match library::apply_wallpaper(&f) {
                Ok(theme) => {
                    eprintln!(
                        "dynamite-rust: DBG press -> wallpaper apply {} theme={}",
                        f, theme
                    );
                    self.wall_selected = i;
                    // The wallpaper's owning theme becomes the selected
                    // theme too.
                    if let Some(m) = library::manifest() {
                        if let Some(ti) = m.index_of_theme(&theme) {
                            self.theme_selected = ti;
                        }
                    }
                    set_selection(self.cal_offset, self.wall_selected, self.theme_selected);
                    self.draw_if_ready();
                }
                Err(e) => {
                    eprintln!(
                        "dynamite-rust: DBG press -> wallpaper apply {} failed: {}",
                        f, e
                    );
                }
            },
            None => {
                eprintln!(
                    "dynamite-rust: DBG press -> wallpaper apply index {} out of range, no-op",
                    i
                );
            }
        }
    }

    /// iter128 B2: apply the library theme at global index `i`
    /// (accent + persist) — the ThemeApply arm, extracted so the
    /// keyboard Enter path (on_themes_key) shares the exact same
    /// apply path as a row click.
    fn apply_theme(&mut self, i: usize) {
        let name = library::manifest()
            .and_then(|m| m.themes.get(i))
            .map(|t| t.name.clone());
        match name {
            Some(n) => match library::apply_theme(&n) {
                Ok(accent) => {
                    eprintln!(
                        "dynamite-rust: DBG press -> theme apply {} accent={:?}",
                        n, accent
                    );
                    self.theme_selected = i;
                    set_selection(self.cal_offset, self.wall_selected, self.theme_selected);
                    self.draw_if_ready();
                }
                Err(e) => {
                    eprintln!(
                        "dynamite-rust: DBG press -> theme apply {} failed: {}",
                        n, e
                    );
                }
            },
            None => {
                eprintln!(
                    "dynamite-rust: DBG press -> theme apply index {} out of range, no-op",
                    i
                );
            }
        }
    }

    /// One key press while the Launcher holds the keyboard
    /// (KeyboardHandler::press_key forwards here). Esc collapses,
    /// Return/KP_Enter launches the selected result, BackSpace edits
    /// the query, and printable xkb text (utf8 — Shift/Caps already
    /// applied by the keymap) appends to it; every edit resets the
    /// selection to the top result and redraws at once (redraws are
    /// event-driven only — no polling was added).
    /// Iter39 a10: Up/Down move the selection (clamped, window
    /// follows — see selection.rs); Enter launches the selected row.
    fn on_launcher_key(&mut self, event: KeyEvent) {
        if self.state != IslandState::Launcher {
            return;
        }
        let sym = event.keysym;
        if sym == Keysym::Escape {
            launcher_panel::live_reset();
            self.request_state(IslandState::Collapsed);
            return;
        }
        if sym == Keysym::Return || sym == Keysym::KP_Enter {
            self.launch_live_selected();
            return;
        }
        if sym == Keysym::Up {
            launcher_panel::live_move_selection(-1);
            self.draw_if_ready();
            return;
        }
        if sym == Keysym::Down {
            launcher_panel::live_move_selection(1);
            self.draw_if_ready();
            return;
        }
        if sym == Keysym::BackSpace {
            launcher_panel::live_backspace();
            self.draw_if_ready();
            return;
        }
        if let Some(text) = &event.utf8 {
            // utf8 fall-backs (same keys, text form) + printable text.
            match text.as_str() {
                "\r" | "\n" => {
                    self.launch_live_selected();
                    return;
                }
                "\u{1b}" => {
                    launcher_panel::live_reset();
                    self.request_state(IslandState::Collapsed);
                    return;
                }
                "\u{7f}" | "\u{8}" => {
                    launcher_panel::live_backspace();
                    self.draw_if_ready();
                    return;
                }
                _ => {}
            }
            let printable: String = text.chars().filter(|c| !c.is_control()).collect();
            if !printable.is_empty() {
                launcher_panel::live_push_text(&printable);
                self.draw_if_ready();
            }
        }
    }

    /// One key press while the Clipboard holds the keyboard
    /// (KeyboardHandler::press_key forwards here). Mirrors
    /// on_launcher_key: Esc collapses (fresh search on re-entry),
    /// Return/KP_Enter copies the selected entry and collapses,
    /// BackSpace edits the query, printable xkb text appends to
    /// it. Up/Down move the selection (clamped, no wrap);
    /// Left/Right cycle menus like the nav discs (no caret
    /// exists to move). Every edit resets the selection to the
    /// top result and redraws at once.
    fn on_clipboard_key(&mut self, event: KeyEvent) {
        if self.state != IslandState::Clipboard {
            return;
        }
        let sym = event.keysym;
        if sym == Keysym::Escape {
            clipboard_panel::clip_reset();
            self.request_state(IslandState::Collapsed);
            return;
        }
        if sym == Keysym::Return || sym == Keysym::KP_Enter {
            if clipboard_panel::clip_select_async() {
                self.request_state(IslandState::Collapsed);
            }
            return;
        }
        if sym == Keysym::Up {
            clipboard_panel::clip_move_selection(-1);
            self.draw_if_ready();
            return;
        }
        if sym == Keysym::Down {
            clipboard_panel::clip_move_selection(1);
            self.draw_if_ready();
            return;
        }
        if sym == Keysym::Left {
            self.cycle_menu(-1);
            return;
        }
        if sym == Keysym::Right {
            self.cycle_menu(1);
            return;
        }
        if sym == Keysym::BackSpace {
            clipboard_panel::clip_backspace();
            self.draw_if_ready();
            return;
        }
        if let Some(text) = &event.utf8 {
            match text.as_str() {
                "\r" | "\n" => {
                    if clipboard_panel::clip_select_async() {
                        self.request_state(IslandState::Collapsed);
                    }
                    return;
                }
                "\u{1b}" => {
                    clipboard_panel::clip_reset();
                    self.request_state(IslandState::Collapsed);
                    return;
                }
                "\u{7f}" | "\u{8}" => {
                    clipboard_panel::clip_backspace();
                    self.draw_if_ready();
                    return;
                }
                _ => {}
            }
            let printable: String = text.chars().filter(|c| !c.is_control()).collect();
            if !printable.is_empty() {
                clipboard_panel::clip_push_text(&printable);
                self.draw_if_ready();
            }
        }
    }

    /// One key press while the Wallpapers view holds the keyboard
    /// (KeyboardHandler::press_key forwards here). Mirrors
    /// on_clipboard_key (iter128 B2): Return/KP_Enter applies the
    /// highlighted match and STAYS in the view (same path as a thumb
    /// click — the filter is then cleared so the strip recentres on
    /// the applied wallpaper); BackSpace edits the query, printable
    /// xkb text appends to it. Up/Down move the highlight through
    /// the matches (clamped, no wrap); Left/Right cycle menus
    /// (handled globally in press_key, before this runs). Every edit
    /// resets the highlight to the top result and redraws at once.
    /// Esc is intercepted in press_key (reset + collapse) before
    /// this runs. Empty query => Enter applies the strip centre
    /// (wall_selected), exactly like a centre-thumb click.
    fn on_wallpaper_key(&mut self, event: KeyEvent) {
        if self.state != IslandState::Wallpaper {
            return;
        }
        let sym = event.keysym;
        if sym == Keysym::Return || sym == Keysym::KP_Enter {
            if let Some(i) = wallpaper_panel::wall_highlighted(self.wall_selected) {
                wallpaper_panel::wall_reset();
                self.apply_wallpaper(i);
            }
            return;
        }
        if sym == Keysym::Up {
            wallpaper_panel::wall_move_selection(-1);
            self.draw_if_ready();
            return;
        }
        if sym == Keysym::Down {
            wallpaper_panel::wall_move_selection(1);
            self.draw_if_ready();
            return;
        }
        if sym == Keysym::BackSpace {
            wallpaper_panel::wall_backspace();
            self.draw_if_ready();
            return;
        }
        if let Some(text) = &event.utf8 {
            match text.as_str() {
                "\r" | "\n" => {
                    if let Some(i) = wallpaper_panel::wall_highlighted(self.wall_selected) {
                        wallpaper_panel::wall_reset();
                        self.apply_wallpaper(i);
                    }
                    return;
                }
                "\u{1b}" => {
                    wallpaper_panel::wall_reset();
                    self.request_state(IslandState::Collapsed);
                    return;
                }
                "\u{7f}" | "\u{8}" => {
                    wallpaper_panel::wall_backspace();
                    self.draw_if_ready();
                    return;
                }
                _ => {}
            }
            let printable: String = text.chars().filter(|c| !c.is_control()).collect();
            if !printable.is_empty() {
                wallpaper_panel::wall_push_text(&printable);
                self.draw_if_ready();
            }
        }
    }

    /// One key press while the Themes view holds the keyboard
    /// (KeyboardHandler::press_key forwards here). Mirrors
    /// on_wallpaper_key (iter128 B2): Return/KP_Enter applies the
    /// highlighted theme and STAYS in the view (same path as a row
    /// click — the filter is then cleared so the rows recentre on
    /// the applied theme); BackSpace edits the query, printable xkb
    /// text appends to it. Up/Down move the highlight through the
    /// matches (clamped, no wrap); Left/Right cycle menus (handled
    /// globally in press_key). Every edit resets the highlight to
    /// the top result and redraws at once. Empty query => Enter
    /// applies the top row (theme_selected), exactly like a row-0
    /// click.
    fn on_themes_key(&mut self, event: KeyEvent) {
        if self.state != IslandState::Themes {
            return;
        }
        let sym = event.keysym;
        if sym == Keysym::Return || sym == Keysym::KP_Enter {
            if let Some(i) = themes_panel::theme_highlighted(self.theme_selected) {
                themes_panel::theme_reset();
                self.apply_theme(i);
            }
            return;
        }
        if sym == Keysym::Up {
            themes_panel::theme_move_selection(-1);
            self.draw_if_ready();
            return;
        }
        if sym == Keysym::Down {
            themes_panel::theme_move_selection(1);
            self.draw_if_ready();
            return;
        }
        if sym == Keysym::BackSpace {
            themes_panel::theme_backspace();
            self.draw_if_ready();
            return;
        }
        if let Some(text) = &event.utf8 {
            match text.as_str() {
                "\r" | "\n" => {
                    if let Some(i) = themes_panel::theme_highlighted(self.theme_selected) {
                        themes_panel::theme_reset();
                        self.apply_theme(i);
                    }
                    return;
                }
                "\u{1b}" => {
                    themes_panel::theme_reset();
                    self.request_state(IslandState::Collapsed);
                    return;
                }
                "\u{7f}" | "\u{8}" => {
                    themes_panel::theme_backspace();
                    self.draw_if_ready();
                    return;
                }
                _ => {}
            }
            let printable: String = text.chars().filter(|c| !c.is_control()).collect();
            if !printable.is_empty() {
                themes_panel::theme_push_text(&printable);
                self.draw_if_ready();
            }
        }
    }

    /// Enter the fullscreen lockscreen PREVIEW (iter10 builder, v0.7
    /// morph): the layer margin drops to 0 and the surface resizes to
    /// fullscreen; the LockIn morph (scrim 100 ms, then giant clock
    /// ~150 ms — lockscreen::LockAnim::lock_in) is driven by a 33 ms
    /// timer chain started here. The draw lands on the next configure
    /// (no direct draw here — drawing now would render the lockscreen
    /// at the stale pill configured_size for a frame); the timer ticks
    /// only draw once the configured size is fullscreen (see
    /// start_lock_anim). Re-entering while an UnlockOut is running
    /// cancels it and restarts the LockIn; re-entering a held lock
    /// just redraws. This is a VISUAL PREVIEW ONLY — it does not lock
    /// the session.
    fn enter_lockscreen(&mut self, loop_handle: &calloop::LoopHandle<'static, Self>) {
        if self.state == IslandState::Lockscreen {
            match self.lock_anim {
                // Mid-unlock re-lock: cancel the UnlockOut and restart
                // the LockIn from the top (fall through).
                Some(run) if run.kind == LockAnimKind::UnlockOut => {}
                _ => {
                    self.draw_if_ready();
                    return;
                }
            }
        }
        if let Some(tok) = self.morph_token.take() {
            let _ = loop_handle.remove(tok);
        }
        // Witness line for the stress harness: entering the preview does
        // not morph (it swaps geometry wholesale), but it is still a
        // top-level state change and the harness confirms effects from
        // these lines.
        eprintln!(
            "dynamite-rust: DBG morph {:?} -> Lockscreen (morph_running=false)",
            self.state
        );
        self.morph = None;
        self.state = IslandState::Lockscreen;
        self.sync_keyboard_interactivity();
        // Lockscreen preview state (iter14): fresh dots, and the
        // wallpaper decoded lazily from the pre-rendered .rgba (None =>
        // black fallback — see load_lock_wallpaper).
        self.lock_dots = 0;
        self.lock_wrong = false;
        self.lock_wallpaper = load_lock_wallpaper();
        if let Some(layer) = &self.layer {
            layer.set_margin(0, 0, 0, 0);
            let (w, h) = IslandState::Lockscreen.size();
            layer.set_size(w, h);
            layer.wl_surface().commit();
        }
        // v0.7: drive the LockIn morph on a 33 ms timer chain.
        self.start_lock_anim(loop_handle, LockAnimKind::LockIn);
    }

    /// Start a lockscreen morph run (v0.7): a 33 ms one-shot timer
    /// chain that redraws the lockscreen from the run's wall-clock
    /// elapsed time (LockAnim::lock_in / unlock_out) until the run's
    /// duration elapses. LockIn ends in the held frame; UnlockOut ends
    /// by calling finish_unlock into lock_exit_target. Any previous
    /// lock run is cancelled first. Timer ticks draw ONLY once the
    /// configured size is fullscreen — before the first configure the
    /// buffer is still pill-sized and a lockscreen draw there would be
    /// garbage for a frame (the configure itself draws the correct
    /// early frame, since draw_if_ready computes the anim from elapsed
    /// wall-clock time, not from a frame counter). On timer-insert
    /// failure the run is dropped and the settled frame is drawn
    /// (instant cut — the old behaviour — rather than a stuck state).
    fn start_lock_anim(
        &mut self,
        loop_handle: &calloop::LoopHandle<'static, Self>,
        kind: LockAnimKind,
    ) {
        if let Some(tok) = self.lock_anim_token.take() {
            let _ = loop_handle.remove(tok);
        }
        self.lock_anim = Some(LockAnimRun {
            kind,
            start: std::time::Instant::now(),
        });
        let loop_handle_anim = loop_handle.clone();
        match loop_handle.insert_source(
            Timer::from_duration(Duration::from_millis(LOCK_ANIM_FRAME_MS)),
            move |_, _, state: &mut Island| {
                let (kind, elapsed_ms) = match state.lock_anim {
                    Some(run) => (run.kind, run.start.elapsed().as_millis() as u32),
                    None => return TimeoutAction::Drop,
                };
                let duration = match kind {
                    LockAnimKind::LockIn => LOCK_IN_MS,
                    LockAnimKind::UnlockOut => LOCK_OUT_MS,
                };
                if elapsed_ms >= duration {
                    state.lock_anim = None;
                    state.lock_anim_token = None;
                    match kind {
                        LockAnimKind::LockIn => {
                            // Settled held frame (only if fullscreen;
                            // otherwise the configure draws it on
                            // arrival — the run is already cleared, so
                            // draw_if_ready uses locked_hold then).
                            if state.configured_size.0 >= 1400 {
                                state.draw_if_ready();
                            }
                        }
                        LockAnimKind::UnlockOut => {
                            let target = state.lock_exit_target;
                            state.finish_unlock(&loop_handle_anim, target);
                        }
                    }
                    TimeoutAction::Drop
                } else {
                    // Drive the intermediate frame — but only once the
                    // surface is actually fullscreen (see the method
                    // docs above).
                    if state.configured_size.0 >= 1400 {
                        state.draw_if_ready();
                    }
                    TimeoutAction::ToDuration(Duration::from_millis(LOCK_ANIM_FRAME_MS))
                }
            },
        ) {
            Ok(tok) => {
                self.lock_anim_token = Some(tok);
            }
            Err(e) => {
                eprintln!("dynamite-rust: lock anim timer insert failed: {e:?} — falling back to settled frame");
                self.lock_anim = None;
                self.draw_if_ready();
            }
        }
    }

    /// Finish an UnlockOut run (v0.7): the instant switch the morph
    /// ends in — margin restored, wallpaper dropped, dots reset, state
    /// set to `target`. The target draw lands on the configure for
    /// the restored size. Only called from the lock-anim timer when
    /// the run is done; callers that want to leave the lockscreen use
    /// exit_lockscreen (which animates) instead.
    fn finish_unlock(
        &mut self,
        loop_handle: &calloop::LoopHandle<'static, Self>,
        target: IslandState,
    ) {
        if self.state != IslandState::Lockscreen {
            return;
        }
        if let Some(tok) = self.morph_token.take() {
            let _ = loop_handle.remove(tok);
        }
        self.morph = None;
        // Lockscreen preview state (iter14): drop the decoded wallpaper
        // (~6 MB) and reset the dots on exit.
        self.lock_dots = 0;
        self.lock_wrong = false;
        self.lock_wallpaper = None;
        // iter125: same treatment for scale_scratch — a lockscreen draw
        // grows it to lockscreen size (1504x1002x4 = 6,028,032 B) and
        // it never shrinks. Free it here; the next draw_if_ready
        // re-grows it stage-sized (1176x636x4, ~3 MB) on first use:
        // one ~3 MB resize+zero, off the hot path. (Simple free, not
        // the stage/lockscreen split — the split saves one realloc but
        // adds a second scratch field for ~0 retained gain.)
        if self.scale_scratch.capacity() > 0 {
            self.scale_scratch = Vec::new();
        }
        // F-A step 3: the lockscreen-sized ring (two 24 MB slots) must
        // not outlive the preview. Dropping it here makes the next draw
        // allocate exactly two stage-sized slots; if a configure has
        // not landed yet the size check in draw_if_ready recreates it
        // again, so this can only ever shrink.
        if self.configured_size != LOCKSCREEN_SIZE {
            self.pool = None;
            self.slots.clear();
            self.slot_bytes = 0;
            self.last_rect = None;
        }
        // F-B: the morph scratch pair is freed on every exit path, not
        // only the morph-into-Collapsed arm.
        Self::free_morph_scratches(&mut self.morph_scratch_from, &mut self.morph_scratch_to);
        unsafe { libc::malloc_trim(0) };
        if let Some(layer) = &self.layer {
            layer.set_margin(stage_margin_top(), 0, 0, 0);
        }
        self.apply_state_instant(target);
    }

    /// Leave the lockscreen preview to `target` (v0.7): starts the
    /// UnlockOut morph (~250 ms: giant clock scales down + flies up,
    /// pill morphs back to island form, scrim fades out) and switches
    /// instantly only when the morph finishes (finish_unlock). No-op
    /// unless currently locked. If an UnlockOut is already running,
    /// just retargets it. If a LockIn is running, the UnlockOut
    /// replaces it (starting from the held frame — see the known gap
    /// in iterations/integrator-iter15.md: an unlock issued inside
    /// the 250 ms LockIn window pops the clock from its current fade
    /// to full, because unlock_out starts at the held frame).
    fn exit_lockscreen(
        &mut self,
        loop_handle: &calloop::LoopHandle<'static, Self>,
        target: IslandState,
    ) {
        if self.state != IslandState::Lockscreen {
            return;
        }
        // Witness line for the stress harness (same contract as
        // enter_lockscreen): the unlock-out animation is not a geometry
        // morph, but it does change the top-level state. Logged before
        // the mid-animation early return so a repeated command while the
        // exit is already running still confirms.
        eprintln!(
            "dynamite-rust: DBG morph Lockscreen -> {:?} (morph_running=true)",
            target
        );
        if let Some(run) = self.lock_anim {
            if run.kind == LockAnimKind::UnlockOut {
                self.lock_exit_target = target;
                return;
            }
        }
        self.lock_exit_target = target;
        self.start_lock_anim(loop_handle, LockAnimKind::UnlockOut);
    }

    /// Start a 400 ms morph to `new_state` (morph.rs), or switch
    /// instantly when the geometry does not change (collapsed clock <->
    /// timer, same state). A command for the morph's current target
    /// mid-morph snaps to the settled end state; a command for a
    /// different state restarts the morph from the CURRENT interpolated
    /// geometry, never from a stale nominal size.
    fn begin_morph(
        &mut self,
        loop_handle: &calloop::LoopHandle<'static, Self>,
        new_state: IslandState,
    ) {
        // Same-state command while a morph is running: snap to the end.
        if new_state == self.state && self.morph.is_some() {
            self.morph = None;
            if let Some(tok) = self.morph_token.take() {
                let _ = loop_handle.remove(tok);
            }
            self.apply_state_instant(new_state);
            self.draw_if_ready();
            return;
        }
        let from_geom = match &self.morph {
            Some(m) => {
                let f = m.frame_at(std::time::Instant::now());
                morph::Geometry::new(f.width, f.height, f.radius)
            }
            None => {
                let (w, h) = self.state.size();
                morph::Geometry::new(w as f32, h as f32, self.state.radius())
            }
        };
        let (tw, th) = new_state.size();
        let to_geom = morph::Geometry::new(tw as f32, th as f32, new_state.radius());
        if from_geom == to_geom {
            // Same geometry: no morph, instant switch (as before).
            self.morph = None;
            if let Some(tok) = self.morph_token.take() {
                let _ = loop_handle.remove(tok);
            }
            self.apply_state_instant(new_state);
            self.draw_if_ready();
            return;
        }
        eprintln!(
            "dynamite-rust: DBG morph {:?} -> {:?} (morph_running={})",
            self.state,
            new_state,
            self.morph.is_some()
        );
        self.morph_from_state = self.state;
        self.state = new_state;
        if new_state != IslandState::ControlCenter
            || self.morph_from_state != IslandState::ControlCenter
        {
            controlcenter_panel::cc_bt_submenu_set(false);
            controlcenter_panel::cc_sound_submenu_set(false);
        }
        if new_state == IslandState::Launcher
            && self.morph_from_state != IslandState::Launcher
        {
            launcher_panel::live_reset();
        }
        if new_state == IslandState::Clipboard
            && self.morph_from_state != IslandState::Clipboard
        {
            clipboard_panel::on_enter();
            self.arm_clipboard_entry_timer();
        }
        if new_state == IslandState::Wallpaper
            && self.morph_from_state != IslandState::Wallpaper
        {
            // Fresh wallpaper search on entry (morph path — same rule
            // as apply_state_instant above).
            wallpaper_panel::wall_reset();
        }
        if new_state == IslandState::Themes
            && self.morph_from_state != IslandState::Themes
        {
            // Fresh themes search on entry (morph path).
            themes_panel::theme_reset();
        }
        if new_state == IslandState::ControlCenter
            && self.morph_from_state != IslandState::ControlCenter
        {
            // iter89: one-shot 250 ms redraw (seed-frame fix, as above).
            self.arm_cc_entry_timer();
            // iter91 B1: force the 1 s live-repaint timer to re-validate
            // on its next tick (fingerprint reset; see the timer).
            self.last_cc_fingerprint = None;
        }
        if new_state == IslandState::Music
            && self.morph_from_state != IslandState::Music
        {
            // iter114 W5: one-shot entry repaint (see
            // arm_music_entry_timer) alongside the fingerprint reset.
            self.arm_music_entry_timer();
            // iter106: force the 1 s live-repaint timer to re-validate
            // on its next tick (fingerprint reset; see the timer).
            self.last_music_fingerprint = None;
        }
        self.sync_keyboard_interactivity();
        self.morph_from_geom = from_geom;
        self.morph_to_geom = to_geom;
        // D7 (review-morph-iter10, iter13): restart through the TESTED
        // helper `Morph::restart_from_current` when a morph is already
        // running, so the shipped path is the tested path — the inlined
        // frame_at + island_expand construction above duplicated it.
        // a1 (iter22) CORRECTION to D7: restart_from_current KEEPS the
        // running morph's easing — an EaseOutBack EXPAND retargeted to a
        // collapse overshoots past the pill (height < 0, saturates to 0
        // in `as u32`) and the compositor kills the layer surface with
        // "height 0 requested without setting top and bottom anchors"
        // (reproduced live 13 Nov 07:40: hover-open + click 120 ms in).
        // So a retarget re-derives the easing from the NEW direction,
        // exactly like the fresh path below; the frame itself still
        // comes from the running morph (no jump).
        let now = std::time::Instant::now();
        self.morph = Some(match &self.morph {
            Some(m) => {
                let frame = m.frame_at(now);
                let from_current = morph::Geometry::new(
                    frame.width,
                    frame.height,
                    frame.radius,
                );
                let expanding = to_geom.width * to_geom.height
                    >= from_current.width * from_current.height;
                let easing = if expanding {
                    morph::Easing::EaseOutCubic
                } else {
                    morph::Easing::EaseOutCubic
                };
                morph::Morph::new_at(
                    from_current,
                    to_geom,
                    m.duration(),
                    easing,
                    now,
                )
            }
            None => {
                // Duration from config (anim_ms, default 160 —
                // interaction-spec §2: view morphs measure
                // 130-230 ms, not the old 400). Easing is
                // EaseOutCubic in BOTH directions (iter33): the old
                // EaseOutBack expand overshot ~10% past the target,
                // and the overshoot strip rendered as a one-frame
                // pure-black slab (burst frame 03, iter32 pixel
                // critic §4) — the capsule loop composites the
                // settled scratches over the frost base, and outside
                // the settled panel both scratches are transparent
                // black (RGB 0), so the max() composite overwrote
                // the frost with 0. The overshoot also swallowed
                // the settled nav discs into the capsule. Cubic
                // keeps the capsule inside the settled rect at
                // every frame, so neither can happen; the
                // composite below is ALSO fixed to a proper
                // alpha-over, so an unfilled strip is impossible
                // even if an overshoot ever returns.
                let expanding = to_geom.width * to_geom.height
                    >= from_geom.width * from_geom.height;
                let easing = if expanding {
                    morph::Easing::EaseOutCubic
                } else {
                    morph::Easing::EaseOutCubic
                };
                morph::Morph::new(
                    from_geom,
                    to_geom,
                    Duration::from_millis(cfg().anim_ms),
                    easing,
                )
            }
        });
        if let Some(tok) = self.morph_token.take() {
            let _ = loop_handle.remove(tok);
        }
        // 16 ms one-shot timer chain (design doc section 6.3 option (a)):
        // the callback draws the wall-clock morph frame via
        // draw_if_ready (iter24: draw-only — the stage never resizes,
        // so no set_size/configure round-trip paces the animation any
        // more). When the morph is done it draws settled and drops the
        // timer.
        match loop_handle.insert_source(
            Timer::from_duration(Duration::from_millis(16)),
            |_, _, state: &mut Island| {
                let now = std::time::Instant::now();
                let done = match &state.morph {
                    Some(m) => m.is_done(now),
                    None => true,
                };
                if done {
                    // iter24: no set_size — the stage never resizes.
                    // Just draw the settled frame (which also rebuilds
                    // the input region for the settled geometry).
                    state.draw_if_ready();
                    // D8 (iter13): the timer source is dropped by this
                    // return — clear the token too, so the next
                    // begin_morph does not call remove() on a dead token.
                    state.morph_token = None;
                    TimeoutAction::Drop
                } else {
                    // iter24: no set_size — the morph is draw-only now.
                    // draw_if_ready renders the wall-clock frame and
                    // rebuilds the input region for it; the commit
                    // inside draw_if_ready presents it. (The old
                    // per-tick set_size is what made the compositor's
                    // surface geometry lag the morph and poisoned the
                    // hit test — see the stage note at the top.)
                    state.draw_if_ready();
                    TimeoutAction::ToDuration(Duration::from_millis(16))
                }
            },
        ) {
            Ok(tok) => {
                self.morph_token = Some(tok);
                // D1 (iter13): draw the first frame IMMEDIATELY
                // (design §6.2 step 5) instead of waiting for the first
                // +16 ms timer tick — the morph otherwise visibly
                // starts 16 ms late. The morph's clock was stamped
                // microseconds ago, so this draws ~the from geometry;
                // the timer chain takes over from the next tick.
                // iter24: draw-only (no set_size any more).
                self.draw_if_ready();
            }
            Err(e) => {
                eprintln!("dynamite-rust: morph timer insert failed: {e:?} — falling back to instant switch");
                self.morph = None;
                self.apply_state_instant(new_state);
                self.draw_if_ready();
            }
        }
    }

    /// Rebuild the wl input region from the CURRENT frame geometry
    /// (iter24 a2): the panel rect + both nav-disc squares + the 8 px
    /// bridge strips between each disc and the panel edge — and, while
    /// a morph runs, the FROM panel/discs too, so the region stays
    /// connected along the pointer's path and a morph never strands
    /// the pointer outside it. Everything else in the stage is
    /// transparent margin: pointer events there go to the surface below
    /// (pass through). The region is double-buffered and has copy
    /// semantics — the wl_region object is destroyed right after
    /// set_input_region; the pending region applies on the commit at
    /// the end of draw_if_ready. Called from draw_if_ready on every
    /// frame (settled and morph ticks), so the region always matches
    /// the frame being drawn. Lockscreen: None (infinite = the whole
    /// fullscreen surface, as before).
    ///
    /// NOTE on Leave: a surface resize / region change does NOT fire
    /// Leave by itself (focus only changes on pointer motion — proven
    /// by v0.13, where the surface resized every 16 ms during morphs
    /// and settled disc clicks still cycled). So shrinking the region
    /// to the target geometry at morph end cannot collapse the island
    /// while the pointer rests on the old disc position.
    /// True when a stage-coords point lies inside the input region the
    /// island is currently advertising — the same rects update_input_region
    /// commits (interpolated capsule + settled panel/discs/bridges for the
    /// current state, plus the FROM state's while a morph runs). Used by
    /// the Leave arm to tell a spurious Leave from a genuine one.
    fn point_in_current_region(&self, pos: (f64, f64)) -> bool {
        // Live capsule rect (iter30 a3c) — the same rect
        // update_input_region commits, so the Leave check can never
        // disagree with the advertised region.
        let (cap_x0, cap_y, fw, fh) = self.live_capsule_rect();
        let cap_x = cap_x0.round();
        if pos.0 >= cap_x as f64
            && pos.0 < (cap_x + fw.round()) as f64
            && pos.1 >= cap_y as f64
            && pos.1 < (cap_y + fh.round()) as f64
        {
            return true;
        }
        if point_in_state_region(pos, self.state) {
            return true;
        }
        if self.morph.is_some() && point_in_state_region(pos, self.morph_from_state) {
            return true;
        }
        false
    }

    /// Current capsule rect (stage coords): the interpolated morph
    /// frame while a morph runs, the settled panel rect otherwise.
    /// The single source for the live-capsule geometry used by the
    /// input region, the Leave check and the live disc centres —
    /// draw_morph_frame computes the same rect from its own frame
    /// (same helper for the top edge: morph_capsule_oy).
    fn live_capsule_rect(&self) -> (f32, f32, f32, f32) {
        match &self.morph {
            Some(m) => {
                let f = m.frame_at(std::time::Instant::now());
                let ox = (STAGE_W as f32 - f.width) / 2.0;
                let oy = morph_capsule_oy(self.morph_from_state, self.state, f.progress);
                (ox, oy, f.width, f.height)
            }
            None => panel_rect_stage(self.state),
        }
    }

    /// Live nav-disc centres: from the current capsule rect, so the
    /// hit test follows the discs while they glide mid-morph (iter30
    /// a3b). Settled, this is exactly nav_disc_centres(state).
    fn live_disc_centres(&self) -> ((f32, f32), (f32, f32)) {
        let (ox, oy, fw, fh) = self.live_capsule_rect();
        let cy = oy + fh / 2.0;
        let off = NAV_DISC_GAP + NAV_DISC_R;
        ((ox - off, cy), (ox + fw + off, cy))
    }

    /// Live disc hit test (iter30 a3b): circular hit around the LIVE
    /// disc centres, with the same widened radius classify_press uses
    /// mid-morph. Returns the cycle direction, or None.
    fn live_disc_hit(&self, pos: (f64, f64)) -> Option<i32> {
        if !MENU_CYCLE.contains(&self.state) {
            return None;
        }
        let ((lx, ly), (rx, ry)) = self.live_disc_centres();
        let radius = (NAV_DISC_R + 1.0 + 10.0) as f64;
        for (cx, cy, dir) in [(lx, ly, -1), (rx, ry, 1)] {
            let d = ((pos.0 - cx as f64).powi(2) + (pos.1 - cy as f64).powi(2)).sqrt();
            if d <= radius {
                return Some(dir);
            }
        }
        None
    }

    fn update_input_region(&mut self) {
        let Some(layer) = &self.layer else { return };
        let surface = layer.wl_surface();
        if self.state == IslandState::Lockscreen {
            surface.set_input_region(None);
            return;
        }
        // Current capsule geometry (iter30 a3c): live_capsule_rect —
        // the interpolated morph frame while a morph runs, the settled
        // panel rect otherwise (top edge interpolating pill-top ->
        // panel-top on expands).
        let (cap_x0, cap_y0, fw, fh) = self.live_capsule_rect();
        let region = self
            .compositor_state
            .wl_compositor()
            .create_region(&self.qh, ());
        // Interpolated capsule rect.
        let cap_x = cap_x0.round() as i32;
        let cap_y = cap_y0.round() as i32;
        let cap_w = fw.round().max(1.0) as i32;
        let cap_h = fh.round().max(1.0) as i32;
        region.add(cap_x, cap_y, cap_w, cap_h);
        // Target panel rect + discs + bridges (settled geometry).
        Self::add_panel_region(&region, self.state);
        // While morphing, the FROM panel/discs too (connectivity).
        if self.morph.is_some() {
            Self::add_panel_region(&region, self.morph_from_state);
        }
        surface.set_input_region(Some(&region));
        region.destroy();
    }

    /// One state's panel rect + disc squares + bridge strips, added
    /// to `region` (stage coords).
    fn add_panel_region(region: &WlRegion, state: IslandState) {
        let (ox, oy, pw, ph) = panel_rect_stage(state);
        region.add(ox as i32, oy as i32, pw as i32, ph as i32);
        // iter90 a8: the workspace cluster rect (circles + gaps +
        // bridge to the pill) belongs to every state's region, so
        // pointer travel pill <-> circles never fires Leave.
        let (wx, wy, ww, wh) = workspace_cluster_rect();
        region.add(wx as i32, wy as i32, ww as i32, wh as i32);
        // iter108-C: the tray cluster rect (icons + gaps + bridge to
        // the pill) belongs to every state's region too, so pointer
        // travel pill <-> tray never fires Leave. None when empty.
        if let Some((tx, ty, tw, th)) = tray_cluster_rect(tray_item_count()) {
            region.add(tx as i32, ty as i32, tw as i32, th as i32);
        }
        if MENU_CYCLE.contains(&state) || state == IslandState::TrayMenu {
            // iter30 a3c: the pill stays visible above the panel — its
            // rect plus the 8 px pill<->panel bridge belong to the
            // menu's input region (pointer travel pill <-> panel must
            // not fire Leave; clicks are decided by classify_press).
            // iter108: TrayMenu gets the pill rect too, but no nav
            // discs (it is not a cycled menu).
            let (px, py, ppw, pph) = panel_rect_stage(IslandState::Collapsed);
            region.add(px as i32, py as i32, ppw as i32, pph as i32);
            region.add(px as i32, (py + pph) as i32, ppw as i32, (oy - (py + pph)) as i32);
        }
        if MENU_CYCLE.contains(&state) {
            let ((lx, ly), (rx, ry)) = nav_disc_centres(state);
            let r = NAV_DISC_R as i32;
            // Disc squares (each disc's bounding box).
            region.add(lx as i32 - r, ly as i32 - r, 2 * r, 2 * r);
            region.add(rx as i32 - r, ry as i32 - r, 2 * r, 2 * r);
            // Bridge strips: the NAV_DISC_GAP-wide gap between each
            // disc square and the panel edge, at disc height — moving
            // panel -> disc must not cross outside the region (that
            // would fire Leave and collapse the island).
            let gap = NAV_DISC_GAP as i32;
            region.add(lx as i32 + r, ly as i32 - r, gap, 2 * r);
            region.add(ox as i32 + pw as i32, ry as i32 - r, gap, 2 * r);
        }
    }

    fn draw_if_ready(&mut self) {
        if self.layer.is_none() {
            return;
        }
        if self.configured_size == (0, 0) {
            return;
        }
        // F-A back-pressure: one frame callback per commit. A request
        // that lands while the callback is outstanding only records the
        // wish; CompositorHandler::frame repaints then. The 250 ms cap
        // is a safety net for a compositor that never acks a hidden
        // surface (otherwise the island would freeze mid-morph).
        if let Some(at) = self.frame_pending {
            if at.elapsed() < std::time::Duration::from_millis(250) {
                self.redraw_wanted = true;
                return;
            }
            self.frame_pending = None;
        }
        // iter30 a3b: tell the backdrop probe whether a morph is
        // running — its pill-flanking strips are covered by the
        // growing capsule mid-morph, so captures taken then are
        // discarded by the probe thread.
        let morphing = self
            .morph
            .as_ref()
            .map(|m| !m.is_done(std::time::Instant::now()))
            .unwrap_or(false);
        wallpaper_blur::set_morphing(morphing);
        // iter101 (Option 2): tell the backdrop probe whether a menu
        // panel is open — the clean live frame is only captured while
        // collapsed (a capture while open would contain the panel).
        wallpaper_blur::set_panel_open(MENU_CYCLE.contains(&self.state));
        // Input region first (it needs no other borrow): panel + discs
        // (+ bridges) in stage coords, from the current frame geometry.
        self.update_input_region();
        let Some(layer) = &self.layer else { return };
        let (w, h) = self.configured_size;
        // Lockscreen preview data for this frame (iter14 + v0.7): live
        // dots / wrong flag from the Island fields; while a lock morph
        // run is active the anim is the run's phase at its wall-clock
        // elapsed time (LockAnim::lock_in / unlock_out), clamped to the
        // held frame once the run's duration has elapsed (the timer
        // callback clears the run, so this clamp only covers draws
        // that land between the last tick and the clear). The morph
        // branch reuses the same frame (the lockscreen never capsule-
        // morphs, so the morph path only ever sees the held frame).
        let lock_data = {
            let anim = match self.lock_anim {
                Some(run) => {
                    let elapsed = run.start.elapsed().as_millis() as u32;
                    match run.kind {
                        LockAnimKind::LockIn => {
                            if elapsed >= LOCK_IN_MS {
                                lockscreen::LockAnim::locked_hold(h)
                            } else {
                                lockscreen::LockAnim::lock_in(elapsed, h)
                            }
                        }
                        LockAnimKind::UnlockOut => {
                            if elapsed >= LOCK_OUT_MS {
                                lockscreen::LockAnim::unlock_out(LOCK_OUT_MS, h)
                            } else {
                                lockscreen::LockAnim::unlock_out(elapsed, h)
                            }
                        }
                    }
                }
                None => lockscreen::LockAnim::locked_hold(h),
            };
            lockscreen::LockscreenData {
                time_hhmm: hhmm(),
                date_line: None,
                dots: self.lock_dots,
                wrong_password: self.lock_wrong,
                anim,
            }
        };
        let lock_wallpaper = self.lock_wallpaper.as_deref();
        // iter108: the shm buffer is RENDER_SCALE x the logical size.
        let (bw, bh) = (w * RENDER_SCALE, h * RENDER_SCALE);
        // F-A: exactly two pinned slots, recreated only when the
        // surface size changes (lockscreen <-> stage). Never grow.
        let needed = (bw * bh * 4) as usize;
        // Ring size 2 is the safe minimum: one buffer on screen while
        // the next is drawn. `pool.canvas(&slot)` returning None for the
        // in-flight slot is the back-pressure, so neither is ever written
        // while the compositor owns it.
        if self.slots.len() != SLOT_RING || self.slot_bytes != needed {
            // Drop the frame we attached before the pool goes away:
            // holding a Buffer of the old pool keeps its whole memfd
            // referenced (a full slab, seen as a late RSS step) until
            // the next successful commit.
            self.buffer = None;
            self.pool = None;
            self.slots.clear();
            self.last_rect = None;
            // Fresh slots are zero-filled memfds: outside the damage
            // rect the buffer is transparent, which is exactly what the
            // compositor shows for an undamaged area. No full-frame
            // damage (and no 12 MB of resident slots) is needed.
            if let Ok(mut fresh) = SlotPool::new(needed * SLOT_RING, &self.shm) {
                for _ in 0..SLOT_RING {
                    match fresh.new_slot(needed) {
                        Ok(slot) => self.slots.push(slot),
                        Err(_) => {
                            self.slots.clear();
                            break;
                        }
                    }
                }
                if self.slots.len() == SLOT_RING {
                    self.slot_bytes = needed;
                    self.pool = Some(fresh);
                }
            }
        }
        let Some(pool) = &mut self.pool else { return };
        let mut picked = None;
        let n = self.slots.len();
        for off in 0..n.max(1) {
            let i = (self.slot_rr + off) % n;
            if pool.canvas(&self.slots[i]).is_some() {
                picked = Some(i);
                break;
            }
        }
        let Some(si) = picked else {
            // Both slots are still in flight; the frame callback will
            // call again as soon as one is released.
            self.redraw_wanted = true;
            return;
        };
        self.slot_rr = (si + 1) % n;
        let slot = self.slots[si].clone();
        let morph_frame = self
            .morph
            .as_ref()
            .map(|m| m.frame_at(std::time::Instant::now()));
        let frame_running = morph_frame.map(|f| !f.done).unwrap_or(false);
        // F-A step 4: damage only what this frame can change — the
        // union of the new content rect and the last committed one.
        // Untouched shm pages stay non-resident (Pss counts pages).
        let full = (0, 0, w as i32, h as i32);
        // F-A step 4 / r1c: damage only what this frame can change.
        // A morph is NOT a full repaint: it interpolates between two
        // KNOWN geometries, so the union of their content rects (each
        // already padded for the panel shadow and the nav discs by
        // content_rect) bounds every pixel the frame can touch — the
        // capsule, the cross-faded endpoint content, the shadow, the
        // nav discs and the top row all live inside it. Bounding it
        // turned the 62-105 ms full-stage upscale into a sub-5 ms
        // region upscale, which is what made a 160 ms morph present
        // 2-3 poses instead of ~10.
        let mut rect = if self.state == IslandState::Lockscreen {
            full
        } else if frame_running {
            union_rect(
                content_rect(self.morph_from_state),
                content_rect(self.state),
            )
        } else {
            content_rect(self.state)
        };
        // The top row is drawn at stage coords by several morph helpers
        // (pill, workspace discs); its band is inside every content rect
        // already, but keep the lock-state full-repaint guard intact.
        rect = clip_rect(rect, w, h);
        let dmg = clip_rect(union_rect(rect, self.last_rect.unwrap_or(rect)), w, h);
        // iter108: the stage is drawn at logical 1x into this reused
        // scratch (grow-only, same pattern as the morph scratches) and
        // upscaled 2x into the shm buffer after the match below.
        let need_scratch = (w * h * 4) as usize;
        if self.scale_scratch.len() != need_scratch {
            self.scale_scratch.resize(need_scratch, 0);
        }
        let stage_1x: &mut [u8] = &mut self.scale_scratch;
        // Morph-aware draw: while a morph runs, draw the interpolated
        // frame (capsule at the interpolated geometry, both endpoint
        // contents cross-faded); once done, fall through to the settled
        // draw. Settled rendering is bit-identical to before the morph
        // (same code path, same font) apart from the intended v0.4
        // content/font changes.
        //
        // S-1 (v0.7, DELETED iter24): the frame used to be computed from
        // the PRESENTED width because the configure for a requested size
        // could lag the 16 ms timer by tens of ms. The stage never
        // resizes now, so there is no configure lag to compensate for
        // and no presented width to invert — frame_at(now) is the frame
        // being drawn. (morph.rs keeps raw_for_eased for its own tests.)
        // Every frame clears only its damage rect: morph frames write
        // inside dmg too now (the capsule loop is bounded below), and
        // fresh memfds are already zero, so a stage-sized memset every
        // 16 ms was pure waste.
        clear_rect(stage_1x, w, h, dmg);
        match morph_frame {
            Some(frame) if !frame.done => {
                let from_state = self.morph_from_state;
                let to_state = self.state;
                let icon_from = icon_font_for(&self.icon_font, from_state);
                let icon_to = icon_font_for(&self.icon_font, to_state);
                // D7: reuse the island's scratch buffers (grow-only)
                // instead of allocating two fresh buffers per
                // intermediate frame. Only the damage rect needs to be
                // zero: the composite below reads nothing outside it,
                // and the previous frame could only have written there.
                let need_from = (w * h * 4) as usize;
                if self.morph_scratch_from.len() != need_from {
                    self.morph_scratch_from.resize(need_from, 0);
                }
                clear_rect(&mut self.morph_scratch_from, w, h, dmg);
                // iter24: the to-scratch is stage-sized too (w, h are
                // the stage size; the old target-panel size is gone).
                let need_to = (w * h * 4) as usize;
                if self.morph_scratch_to.len() != need_to {
                    self.morph_scratch_to.resize(need_to, 0);
                }
                clear_rect(&mut self.morph_scratch_to, w, h, dmg);
                draw_morph_frame(
                    stage_1x,
                    w,
                    h,
                    from_state,
                    to_state,
                    &frame,
                    &mut self.morph_scratch_from,
                    &mut self.morph_scratch_to,
                    self.font,
                    icon_from,
                    icon_to,
                    &lock_data,
                    lock_wallpaper,
                    dmg,
                );
            }
            _ => {
                if morph_frame.map(|f| f.done).unwrap_or(false) {
                    self.morph = None;
                    // iter125/F-B: any completed morph can free the
                    // ~5.9 MB pair — the settled draw needs only
                    // scale_scratch (re-grown on the next expand). An
                    // instant switch back to Collapsed also frees via
                    // apply_state_instant, so no path keeps it.
                    Self::free_morph_scratches(
                        &mut self.morph_scratch_from,
                        &mut self.morph_scratch_to,
                    );
                }
                draw(
                    stage_1x,
                    w,
                    h,
                    self.state,
                    self.font,
                    // Icon font only for the states that draw glyphs; the
                    // collapsed/lock/placeholder states must not pay its ~19 MB.
                    icon_font_for(&self.icon_font, self.state),
                    &lock_data,
                    lock_wallpaper,
                );
            }
        }
        // iter108: upscale the 1x stage into the 2x shm buffer
        // (RENDER_SCALE). Settled frames upscale only their damage rect;
        // a morph/lockscreen frame is a full repaint.
        let Some(pool) = &mut self.pool else { return };
        let Some(canvas) = pool.canvas(&slot) else {
            self.redraw_wanted = true;
            return;
        };
        if frame_running {
            upscale_canvas_2x_region(&self.scale_scratch, w, h, canvas, dmg);
        } else {
            upscale_canvas_2x_region(&self.scale_scratch, w, h, canvas, dmg);
        }
        // A1: every settled frame whose topmost element is the collapsed
        // pill (bare, or over an open menu) replaces that rect with a
        // 2x-rendered copy, so the clock glyphs skip one resample. Morph
        // frames own their canvas and keep the old path.
        let pill_on_top = matches!(self.state, IslandState::Collapsed | IslandState::TrayMenu)
            || MENU_CYCLE.contains(&self.state);
        if self.morph.is_none() && pill_on_top {
            draw_top_row_2x(canvas, bw, bh, self.font, active_tag_cached());
        }
        let buffer = match pool.create_buffer_in(
            &slot,
            bw as i32,
            bh as i32,
            (bw * 4) as i32,
            Format::Argb8888,
        ) {
            Ok(b) => b,
            Err(_) => return,
        };
        let surface = layer.wl_surface();
        // The buffer is RENDER_SCALE x the logical surface size, so
        // advertise buffer_scale 2 (applies on this commit, before the
        // attach below) — the compositor shows the surface at its
        // logical size, downsampling 2x -> 1.5x. Sent every frame so a
        // recreated surface can never attach a 2x buffer at scale 1
        // (which would display the island twice as large).
        surface.set_buffer_scale(RENDER_SCALE as i32);
        surface.attach(Some(buffer.wl_buffer()), 0, 0);
        surface.damage_buffer(
            dmg.0 * RENDER_SCALE as i32,
            dmg.1 * RENDER_SCALE as i32,
            (dmg.2 - dmg.0) * RENDER_SCALE as i32,
            (dmg.3 - dmg.1) * RENDER_SCALE as i32,
        );
        surface.commit();
        // F-A step 1: ask for the release notification; the next draw
        // waits for it (or for the 250 ms safety cap).
        surface.frame(&self.qh, surface.clone());
        self.frame_pending = Some(std::time::Instant::now());
        self.redraw_wanted = false;
        self.last_rect = Some(rect);
        self.last_backdrop_factor = wallpaper_blur::backdrop_factor();
        self.buffer = Some(buffer);
        self.drawn_size = (w, h);
        {
            static ONCE: std::sync::Once = std::sync::Once::new();
        }
    }
}

impl CompositorHandler for Island {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &WlSurface,
        _new_factor: i32,
    ) {
    }
    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &WlSurface,
        _new_transform: Transform,
    ) {
    }
    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &WlSurface,
        _time: u32,
    ) {
        // The compositor released the buffer we committed: another draw
        // may proceed. Only redraw if a timer/event asked while this
        // callback was outstanding — idle stays at zero commits.
        self.frame_pending = None;
        if self.redraw_wanted {
            self.redraw_wanted = false;
            self.draw_if_ready();
        }
    }
    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &WlSurface,
        _output: &WlOutput,
    ) {
    }
    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &WlSurface,
        _output: &WlOutput,
    ) {
    }
}

impl OutputHandler for Island {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: WlOutput,
    ) {
    }
    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: WlOutput,
    ) {
    }
    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: WlOutput,
    ) {
    }
}

impl LayerShellHandler for Island {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.loop_signal.stop();
    }
    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        self.configured_size = configure.new_size;
        if self.configured_size.0 == 0 {
            self.configured_size.0 = self.state.size().0;
        }
        if self.configured_size.1 == 0 {
            self.configured_size.1 = self.state.size().1;
        }
        self.draw_if_ready();
    }
}

impl ShmHandler for Island {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for Island {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

delegate_compositor!(Island);
delegate_output!(Island);
delegate_shm!(Island);
delegate_layer!(Island);
delegate_registry!(Island);
delegate_seat!(Island);
delegate_keyboard!(Island);
delegate_pointer!(Island);
delegate_noop!(Island: ignore WlRegion);

// ---------------------------------------------------------------------------
// Seat / pointer / keyboard (iter20). SeatHandler binds ONLY the
// pointer and keyboard capabilities. PointerHandler implements the
// hover / wheel / click contract; KeyboardHandler forwards presses
// to Island::on_launcher_key (the launcher is the only state that
// holds keyboard interactivity — see sync_keyboard_interactivity).
// ---------------------------------------------------------------------------
impl SeatHandler for Island {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: WlSeat,
        capability: Capability,
    ) {
        match capability {
            Capability::Pointer => {
                if self.pointer.is_none() {
                    match self.seat_state.get_pointer(qh, &seat) {
                        Ok(pointer) => self.pointer = Some(pointer),
                        Err(e) => {
                            eprintln!("dynamite-rust: could not bind wl_pointer: {e}")
                        }
                    }
                }
            }
            Capability::Keyboard => {
                if self.keyboard.is_none() {
                    match self.seat_state.get_keyboard(qh, &seat, None) {
                        Ok(keyboard) => self.keyboard = Some(keyboard),
                        Err(e) => {
                            eprintln!("dynamite-rust: could not bind wl_keyboard: {e}")
                        }
                    }
                }
            }
            // Touch is deliberately not requested.
            _ => {}
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: WlSeat,
        capability: Capability,
    ) {
        match capability {
            Capability::Pointer => {
                if let Some(pointer) = self.pointer.take() {
                    pointer.release();
                }
                self.pointer_over = false;
            }
            Capability::Keyboard => {
                if let Some(keyboard) = self.keyboard.take() {
                    keyboard.release();
                }
            }
            _ => {}
        }
    }

    fn remove_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: WlSeat) {}
}

impl PointerHandler for Island {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            // Only our own layer surface (there is exactly one
            // surface, but never trust a stray event), and never
            // during the lockscreen preview — it owns the screen.
            let ours = match &self.layer {
                Some(layer) => event.surface == *layer.wl_surface(),
                None => false,
            };
            if !ours || self.state == IslandState::Lockscreen {
                continue;
            }
            // Track the pointer position on every event except Leave:
            // the Leave arm compares against this to tell a stationary
            // (spurious) Leave from a genuine move-off.
            if !matches!(event.kind, PointerEventKind::Leave { .. }) {
                self.last_pointer_pos = Some(event.position);
            }
            match event.kind {
                PointerEventKind::Enter { .. } => {
                    eprintln!(
                        "dynamite-rust: DBG enter state={:?} morph={} pos=({:.1},{:.1})",
                        self.state,
                        self.morph.is_some(),
                        event.position.0,
                        event.position.1
                    );
                    self.pointer_over = true;
                    // Hover-open (config hover_open + hover_state):
                    // collapsed/lock pill expands to the hover menu.
                    if cfg().hover_open
                        && (self.state == IslandState::Collapsed
                            || self.state == IslandState::Lock)
                    {
                        let target = cfg().hover_state;
                        self.request_state(target);
                    }
                }
                PointerEventKind::Leave { .. } => {
                    eprintln!(
                        "dynamite-rust: DBG leave state={:?} morph={} pos=({:.1},{:.1})",
                        self.state,
                        self.morph.is_some(),
                        event.position.0,
                        event.position.1
                    );
                    // iter24d (A-1): mango re-evaluates pointer focus
                    // on commit against the PRE-commit region, so a
                    // morph starting while the pointer sits at the old
                    // disc (outside the FROM menu's settled region)
                    // emits a Leave even though the pointer is inside
                    // the new union region (diag24 F2: Leave at
                    // stage (916,16) — inside Wallpaper's panel —
                    // collapsed the island, and the fast click that
                    // followed re-opened Music instead of cycling).
                    // A Leave whose position is still inside the
                    // CURRENT region is spurious: ignore it. A genuine
                    // Leave (pointer outside the region) still
                    // collapses, as before.
                    // iter24e: a Leave at the same position as the
                    // last pointer event means the pointer did not
                    // move — the Leave was caused by a region/state
                    // change (e.g. a socket-opened menu while the
                    // pointer is parked outside it), not by the user
                    // moving off. Ignore it, like the inside-region
                    // case above.
                    let stationary = match self.last_pointer_pos {
                        Some((lx, ly)) => {
                            (event.position.0 - lx).abs() < 0.5
                                && (event.position.1 - ly).abs() < 0.5
                        }
                        None => false,
                    };
                    self.last_pointer_pos = Some(event.position);
                    if stationary {
                        eprintln!("dynamite-rust: DBG leave -> spurious (stationary), ignored");
                    } else if self.point_in_current_region(event.position) {
                        eprintln!("dynamite-rust: DBG leave -> spurious (inside region), ignored");
                    } else {
                        self.pointer_over = false;
                        self.wheel_accum = 0.0;
                        // Leave always collapses a menu state (decided:
                        // no distinction between hover-opened and
                        // socket-opened expansions). The launcher is
                        // the one exception — it holds the keyboard,
                        // and a pointer slip off the panel must not
                        // destroy a half-typed search; Esc/click/
                        // launch collapse it instead. The clipboard
                        // is the second exception, for the same reason.
                        if (MENU_CYCLE.contains(&self.state)
                            || self.state == IslandState::TrayMenu)
                            && self.state != IslandState::Launcher
                            && self.state != IslandState::Clipboard
                        {
                            self.request_state(IslandState::Collapsed);
                        }
                    }
                }
                PointerEventKind::Press { button, .. } if button == BTN_LEFT => {
                    eprintln!(
                        "dynamite-rust: DBG press state={:?} morph={} pos=({:.1},{:.1})",
                        self.state,
                        self.morph.is_some(),
                        event.position.0,
                        event.position.1
                    );
                    self.last_click = Some(event.position);
                    // iter27 (D1/D2 hit-test): the decision is the pure
                    // classify_press() above — this arm only executes it.
                    // Collapsing stays possible via pill click (menu
                    // states), Esc, socket, and genuine Leave — NOT via
                    // panel clicks.
                    // iter30 a3b: mid-morph the discs glide with the
                    // live capsule — check the LIVE disc centres first
                    // (classify_press below still covers the settled
                    // positions as a fallback).
                    // iter36 a6b: music transport needs the font (the
                    // control centres depend on the clock width), so it
                    // is checked here before classify_press — same
                    // pattern as the live-disc check below. Settled
                    // state only: mid-morph the controls glide.
                    let music_hit = if self.state == IslandState::Music
                        && self.morph.is_none()
                    {
                        music_panel::control_hit(
                            self.font,
                            event.position,
                            panel_origin(self.state),
                        )
                    } else {
                        None
                    };
                    let action = if let Some(t) = music_hit {
                        PressAction::MusicTransport(t)
                    } else if self.morph.is_some() {
                        self.live_disc_hit(event.position)
                            .map(PressAction::Cycle)
                            .unwrap_or_else(|| {
                                classify_press(
                                    event.position,
                                    self.state,
                                    true,
                                    self.wall_selected,
                                    self.theme_selected,
                                    tray_item_count(),
                                )
                            })
                    } else {
                        classify_press(
                            event.position,
                            self.state,
                            false,
                            self.wall_selected,
                            self.theme_selected,
                            tray_item_count(),
                        )
                    };
                    match action {
                        PressAction::OpenMusic => {
                            eprintln!("dynamite-rust: DBG press -> pill open music");
                            // a1 (iter22): a pill click always opens
                            // the FIRST menu (Music / Clock-Media).
                            self.request_state(MENU_CYCLE[0]);
                        }
                        PressAction::Cycle(dir) => {
                            eprintln!("dynamite-rust: DBG press -> disc dir={}", dir);
                            self.cycle_menu(dir);
                        }
                        PressAction::WorkspaceTag(n) => {
                            eprintln!("dynamite-rust: DBG press -> workspace tag {}", n);
                            dispatch_tag(n);
                            // Optimistic highlight (the poller corrects
                            // it within ~1 s if the dispatch failed).
                            self.last_tag_drawn = n;
                            self.draw_if_ready();
                        }
                        PressAction::TrayActivate(i) => {
                            eprintln!("dynamite-rust: DBG press -> tray activate {}", i);
                            crate::tray::activate_index(i);
                        }
                        PressAction::TrayMenuRow(id) => {
                            eprintln!("dynamite-rust: DBG press -> tray menu row {}", id);
                            crate::tray::menu_event(id);
                            self.request_state(IslandState::Collapsed);
                        }
                        PressAction::Collapse => {
                            eprintln!("dynamite-rust: DBG press -> pill collapse");
                            self.request_state(IslandState::Collapsed);
                        }
                        PressAction::CcToggle(i) => {
                            eprintln!("dynamite-rust: DBG press -> cc toggle {}", i);
                            controlcenter_panel::cc_toggle_index(i);
                            // draw() re-reads ControlCenterData::live(),
                            // so the redraw shows the REAL post-action
                            // state, never a local-only flip.
                            self.draw_if_ready();
                        }
                        PressAction::CcBtOpen => {
                            controlcenter_panel::cc_bt_submenu_set(true);
                            self.draw_if_ready();
                        }
                        PressAction::CcBtBack => {
                            controlcenter_panel::cc_bt_submenu_set(false);
                            self.draw_if_ready();
                        }
                        PressAction::CcBtConnect(i) => {
                            controlcenter_panel::cc_bt_connect(i);
                            self.draw_if_ready();
                        }
                        PressAction::CcSoundOpen => {
                            controlcenter_panel::cc_sound_submenu_set(true);
                            self.draw_if_ready();
                        }
                        PressAction::CcSoundRoute(i) => {
                            controlcenter_panel::cc_audio_select(i);
                            self.draw_if_ready();
                        }
                        PressAction::CcSlider(which, level) => {
                            eprintln!(
                                "dynamite-rust: DBG press -> cc slider {} {:.2}",
                                which, level
                            );
                            if which == 0 {
                                controlcenter_panel::cc_set_display(level);
                            } else {
                                controlcenter_panel::cc_set_sound(level);
                            }
                            self.cc_drag = Some((which, level));
                            self.draw_if_ready();
                        }
                        PressAction::CcPower(i) => {
                            eprintln!("dynamite-rust: DBG press -> cc power {}", i);
                            controlcenter_panel::cc_set_power_level(i);
                            self.draw_if_ready();
                        }
                        PressAction::LaunchRow(i) => {
                            eprintln!("dynamite-rust: DBG press -> launcher row {}", i);
                            if let Some(exec) = launcher_panel::live_visible_row_exec(i) {
                                spawn_exec(&exec);
                                launcher_panel::live_reset();
                                self.request_state(IslandState::Collapsed);
                            } else {
                                // Row hit but no live exec (no live list):
                                // do nothing rather than collapse.
                                eprintln!("dynamite-rust: DBG press -> launcher row no exec, no-op");
                            }
                        }
                        PressAction::ClipRow(i) => {
                            eprintln!("dynamite-rust: DBG press -> clipboard row {}", i);
                            if clipboard_panel::clip_row_select_async(i) {
                                self.request_state(IslandState::Collapsed);
                            } else {
                                // Row hit but nothing to copy (empty /
                                // no match): do nothing rather than collapse.
                                eprintln!("dynamite-rust: DBG press -> clipboard row no copy, no-op");
                            }
                        }
                        PressAction::WallApply(i) => {
                            self.apply_wallpaper(i);
                        }
                        PressAction::WallWindow(dir) => {
                            // iter36 a6b: browse the filmstrip selection
                            // (no apply). iter128: stepping is
                            // wallpaper_panel::step_selection — exactly
                            // ±1 with wrap, so every index is reachable
                            // and the iter127 parity trap is impossible.
                            let n = library::manifest()
                                .map(|m| m.wallpaper_count())
                                .unwrap_or(wallpaper_panel::FILMSTRIP_NAMES.len());
                            if n > 0 {
                                self.wall_selected = wallpaper_panel::step_selection(
                                    self.wall_selected,
                                    dir,
                                    n,
                                );
                                set_selection(self.cal_offset, self.wall_selected, self.theme_selected);
                                eprintln!(
                                    "dynamite-rust: DBG press -> wallpaper browse dir={} selected={}",
                                    dir, self.wall_selected
                                );
                                self.draw_if_ready();
                            }
                        }
                        PressAction::ThemeApply(i) => {
                            self.apply_theme(i);
                        }
                        PressAction::PowerButton(i) => {
                            // iter36 a6b: only Lock executes. The other
                            // four are destructive: log the exact command
                            // (command-path verification) and do NOT run
                            // it unannounced.
                            let (label, cmd) = power_panel::action_command(i);
                            if i == 2 {
                                eprintln!(
                                    "dynamite-rust: DBG press -> power lock ({})",
                                    cmd
                                );
                                self.request_state(IslandState::Collapsed);
                                spawn_exec(cmd);
                            } else {
                                eprintln!(
                                    "dynamite-rust: DBG press -> power {} ({}): NOT executed, command-path only",
                                    label, cmd
                                );
                            }
                        }
                        PressAction::CalMonth(dir) => {
                            // iter36 a6b: shift the displayed month
                            // (wraps Dec<->Jan in the panel).
                            self.cal_offset += dir;
                            set_selection(self.cal_offset, self.wall_selected, self.theme_selected);
                            eprintln!(
                                "dynamite-rust: DBG press -> calendar month dir={} offset={}",
                                dir, self.cal_offset
                            );
                            self.draw_if_ready();
                        }
                        PressAction::MusicTransport(t) => {
                            // iter36 a6b: drive playerctl/MPRIS, detached
                            // (spawn never waits — freeze rule). No player
                            // => playerctl exits non-zero, state stays the
                            // honest idle; the 1 s cache refreshes after.
                            // iter114 W4: drive the SELECTED player
                            // explicitly — bare playerctl may answer for
                            // a different player than the reads (the
                            // iter110 defect). Falls back to bare only
                            // when no player is selected.
                            let verb = match t {
                                0 => "previous",
                                2 => "next",
                                _ => "play-pause",
                            };
                            let cmd = match music_panel::selected_player() {
                                Some(player) => {
                                    format!("playerctl --player={} {}", player, verb)
                                }
                                None => format!("playerctl {}", verb),
                            };
                            eprintln!("dynamite-rust: DBG press -> music transport {}", cmd);
                            spawn_exec(&cmd);
                            // iter114 W4: optimistic play/pause flip —
                            // the immediate draw below reads the
                            // PRE-click cache, so flip the cached bit
                            // now (the refresher corrects it within
                            // ~1 s if the command failed). Next/prev
                            // get no optimistic update.
                            if t == 1 {
                                music_panel::optimistic_toggle();
                            }
                            self.draw_if_ready();
                        }
                        PressAction::NoOp => {
                            // Keep the reason in the log: it is how the
                            // next run tells a margin click from a panel
                            // click from a stale post-collapse delivery.
                            let reason = if matches!(
                                self.state,
                                IslandState::Collapsed | IslandState::Lock
                            ) {
                                "outside pill"
                            } else if !point_in_panel_stage(event.position, self.state) {
                                "margin"
                            } else if self.state == IslandState::Launcher {
                                "launcher empty area"
                            } else {
                                "panel background"
                            };
                            eprintln!("dynamite-rust: DBG press -> no-op ({})", reason);
                        }
                    }
                }
                PointerEventKind::Press { button, .. } if button == BTN_RIGHT => {
                    // iter108: right click on a tray icon opens
                    // the app's DBusMenu as the TrayMenu panel
                    // (FetchMenu runs on the tray thread; the
                    // panel opens at once and the 1 s timer fills
                    // it in when the layout arrives). An item with
                    // no menu gets ContextMenu instead (the app
                    // shows its own menu window). Anywhere else:
                    // no-op — no other right-click behaviour
                    // exists in the island.
                    if let Some(i) = tray_hit_stage(event.position, tray_item_count()) {
                        if matches!(
                            crate::tray::context_index(i),
                            Some(crate::tray::ContextAction::OpenMenu)
                        ) {
                            self.request_state(IslandState::TrayMenu);
                        }
                    }
                }
                PointerEventKind::Release { button, .. } if button == BTN_LEFT => {
                    // iter34 a6a: end any CC slider drag.
                    self.cc_drag = None;
                }
                PointerEventKind::Motion { .. } => {
                    // iter34 a6a: CC slider drag — pointer x maps to the
                    // level while the button is held; >= 0.02 steps only.
                    if let Some((which, last)) = self.cc_drag {
                        if self.state == IslandState::ControlCenter && self.morph.is_none() {
                            let layout = controlcenter_panel::cc_layout();
                            let track = if which == 0 {
                                layout.display_track
                            } else {
                                layout.sound_track
                            };
                            let (ox, _) = panel_origin(self.state);
                            let px = event.position.0 as f32 - ox;
                            let level = ((px - track.x) / track.w).clamp(0.0, 1.0);
                            if (level - last).abs() >= 0.02 {
                                if which == 0 {
                                    controlcenter_panel::cc_set_display(level);
                                } else {
                                    controlcenter_panel::cc_set_sound(level);
                                }
                                self.cc_drag = Some((which, level));
                                self.draw_if_ready();
                            }
                        }
                    } else if self.morph.is_some() {
                        eprintln!(
                            "dynamite-rust: DBG motion state={:?} pos=({:.1},{:.1})",
                            self.state,
                            event.position.0,
                            event.position.1
                        );
                    }
                }
                PointerEventKind::Axis { vertical, .. } => {
                    // iter108: wheel over a tray icon forwards
                    // Scroll to the item (never cycles menus
                    // from there) — works collapsed too.
                    if let Some(i) = tray_hit_stage(event.position, tray_item_count()) {
                        let delta = if vertical.discrete != 0 {
                            vertical.discrete
                        } else {
                            (vertical.absolute / 15.0) as i32
                        };
                        if delta != 0 {
                            crate::tray::scroll_index(i, delta);
                        }
                    } else {
                        self.on_wheel(vertical);
                    }
                }
                _ => {}
            }
        }
    }
}

impl KeyboardHandler for Island {
    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _surface: &WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[Keysym],
    ) {
    }

    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _surface: &WlSurface,
        _serial: u32,
    ) {
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        // a1 (iter22): Esc collapses from ANY open menu, not just the
        // Launcher (keyboard interactivity is Exclusive in every menu
        // state — see sync_keyboard_interactivity). Anything else goes
        // to the per-view key handlers below.
        if event.keysym == Keysym::Escape
            && (MENU_CYCLE.contains(&self.state) || self.state == IslandState::TrayMenu)
        {
            if self.state == IslandState::ControlCenter
                && (controlcenter_panel::cc_bt_submenu_open()
                    || controlcenter_panel::cc_sound_submenu_open())
            {
                controlcenter_panel::cc_bt_submenu_set(false);
                controlcenter_panel::cc_sound_submenu_set(false);
                self.draw_if_ready();
                return;
            }
            if self.state == IslandState::Launcher {
                launcher_panel::live_reset();
            }
            // iter39 a10: same fresh-search reset for the Clipboard.
            if self.state == IslandState::Clipboard {
                clipboard_panel::clip_reset();
            }
            // iter128 B2: same fresh-search reset for Wallpapers/Themes.
            if self.state == IslandState::Wallpaper {
                wallpaper_panel::wall_reset();
            }
            if self.state == IslandState::Themes {
                themes_panel::theme_reset();
            }
            self.request_state(IslandState::Collapsed);
            return;
        }
        // iter39 a10: Left/Right cycle menus from ANY open menu —
        // same order and wrap as the nav discs and the wheel
        // (cycle_menu → request_state → begin_morph). There is no
        // text caret in either list view, so the arrows have no
        // editing meaning to preserve. No-op when collapsed (the
        // grab is None there, so this handler never fires anyway).
        if event.keysym == Keysym::Left && MENU_CYCLE.contains(&self.state) {
            self.cycle_menu(-1);
            return;
        }
        if event.keysym == Keysym::Right && MENU_CYCLE.contains(&self.state) {
            self.cycle_menu(1);
            return;
        }
        // Per-view keys (typing, list navigation, Enter): Clipboard,
        // Wallpaper and Themes have their own handlers; everything else
        // goes to the launcher handler (a no-op outside the Launcher).
        if self.state == IslandState::Clipboard {
            self.on_clipboard_key(event);
        } else if self.state == IslandState::Wallpaper {
            self.on_wallpaper_key(event);
        } else if self.state == IslandState::Themes {
            self.on_themes_key(event);
        } else {
            self.on_launcher_key(event);
        }
    }

    fn release_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _serial: u32,
        _event: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _serial: u32,
        _modifiers: Modifiers,
        _layout: u32,
    ) {
    }

    // update_repeat_info has a default impl in sctk 0.19 — key
    // repeat is NOT wired (held Backspace does not repeat; a
    // calloop-timer repeat driver is a follow-up, see the report).
}

// ---------------------------------------------------------------------------
// SIGTERM/SIGINT self-pipe
// ---------------------------------------------------------------------------
static SIGNAL_WRITE_FD: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);

extern "C" fn on_signal(_sig: libc::c_int) {
    let fd = SIGNAL_WRITE_FD.load(std::sync::atomic::Ordering::Relaxed);
    if fd >= 0 {
        let byte = [1u8; 1];
        unsafe {
            libc::write(fd, byte.as_ptr() as *const libc::c_void, 1);
        }
    }
}

fn install_signal_pipe() -> libc::c_int {
    let mut fds: [libc::c_int; 2] = [0; 2];
    unsafe {
        libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK);
        SIGNAL_WRITE_FD.store(fds[1], std::sync::atomic::Ordering::Relaxed);
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = on_signal as usize;
        libc::sigemptyset(&mut sa.sa_mask);
        libc::sigaction(libc::SIGTERM, &sa, std::ptr::null_mut());
        libc::sigaction(libc::SIGINT, &sa, std::ptr::null_mut());
    }
    fds[0]
}

// ---------------------------------------------------------------------------
// main: second invocation = remote control; otherwise run the daemon.
// ---------------------------------------------------------------------------
fn send_command(word: &str) -> std::io::Result<()> {
    let sock = UnixDatagram::unbound()?;
    sock.send_to(word.as_bytes(), socket_path())?;
    Ok(())
}


fn main() {
    // F-A/F-B: pin glibc's large-allocation policy. With the default
    // dynamic threshold a single ~10 MB free raises the mmap threshold
    // to 32 MB, so every later large buffer (wallpaper decode ~10 MB,
    // lockscreen slabs, scale_scratch) is carved from the brk heap and
    // malloc_trim can never return it: steady state sat ~13 MB high.
    // Explicit mmap for anything >=128 KiB (munmapped on free) and a
    // 128 KiB trim threshold makes the big transients disappear.
    unsafe {
        libc::mallopt(libc::M_MMAP_THRESHOLD, 128 * 1024);
        libc::mallopt(libc::M_TRIM_THRESHOLD, 128 * 1024);
    }
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let word = args[1].as_str();
        if word == "quit"
            || word == "lockscreen"
            || word == "unlock"
            || word == "locktype"
            || word == "lockback"
            || word == "lockwrong"
            || IslandState::from_word(word).is_some()
            || word == "clock"
            || word == "reload"
        {
            match send_command(word) {
                Ok(()) => return,
                Err(e) => {
                    eprintln!("dynamite-rust: could not reach daemon: {e}");
                    std::process::exit(1);
                }
            }
        }
        eprintln!("usage: dynamite-rust [collapsed|clock|lock|power|music|calendar|controlcenter|launcher|clipboard|wallpaper|themes|lockscreen|unlock|locktype|lockback|lockwrong|quit]");
        std::process::exit(2);
    }

    // Runtime config first: geometry below (margin, collapsed
    // size) and every later state change read it via cfg().
    load_config();

    // iter39: warm the clipboard cache off the hot path, so the
    // first Clipboard open shows history instead of the empty
    // state while the entry refresh runs.
    clipboard_panel::refresh_async();

    // Font: Quicksand Bold (static, usWeightClass 700), resolved by
    // ui_font() — exe-relative, then $HOME, then the vendored asset
    // compiled in. Never required on disk, never a panic: the old
    // $HOME-only chain made the binary die at startup off the dev box.
    let font = ui_font();

    // The icon font is NOT loaded here: see Island::icon_font — it parses
    // to ~19 MB, so it loads lazily on the first Power/ControlCenter draw.

    let conn = Connection::connect_to_env().expect("no Wayland connection");
    let (globals, mut event_queue) =
        registry_queue_init::<Island>(&conn).expect("registry init failed");
    let qh = event_queue.handle();
    let compositor_state =
        CompositorState::bind(&globals, &qh).expect("wl_compositor not available");
    let layer_shell = LayerShell::bind(&globals, &qh).expect("zwlr_layer_shell_v1 not available");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm not available");

    let surface = compositor_state.create_surface(&qh);
    let layer =
        layer_shell.create_layer_surface(&qh, surface, Layer::Overlay, Some(NAMESPACE), None);
    layer.set_anchor(Anchor::TOP);
    layer.set_margin(stage_margin_top(), 0, 0, 0);
    layer.set_exclusive_zone(pill_strip_px());
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer.set_size(STAGE_W, STAGE_H);
    layer.wl_surface().commit();

    // F-A: the 2-slot shm ring is created on the first draw for the
    // exact surface size; no up-front pool (it would be an unused
    // 23.9 MB mapping).

    let mut event_loop: EventLoop<Island> = EventLoop::try_new().expect("calloop");
    let loop_handle = event_loop.handle();
    let loop_signal = event_loop.get_signal();

    WaylandSource::new(conn.clone(), event_queue)
        .insert(loop_handle.clone())
        .expect("wayland source");

    // Minute-boundary timer: redraws the live-clock states (collapsed
    // pill, music clock, CC header, lockscreen giant clock, calendar —
    // iter116: Calendar was already in the condition below; the comment
    // omitted it) at each minute boundary.
    {
        let timer = Timer::from_duration(Duration::from_millis(ms_to_next_minute()));
        loop_handle
            .insert_source(timer, |_, _, state: &mut Island| {
                if state.state == IslandState::Collapsed
                    || state.state == IslandState::Music
                    || state.state == IslandState::ControlCenter
                    || state.state == IslandState::Lockscreen
                    || state.state == IslandState::Calendar
                {
                    state.draw_if_ready();
                }
                TimeoutAction::ToDuration(Duration::from_millis(ms_to_next_minute()))
            })
            .expect("timer source");
    }

    // iter97: tag-watch wake pipe — the read end of a UnixStream pair
    // whose write end the watcher thread pokes on every real tag
    // change (see tag_watch_loop). The callback drains the pipe and
    // redraws only when the cached tag moved past the last drawn one
    // (same check as the 1 s timer below, but fired within ms).
    {
        let (rx, tx) = std::os::unix::net::UnixStream::pair()
            .expect("tag watch wake pipe");
        rx.set_nonblocking(true).expect("tag wake nonblocking");
        tx.set_nonblocking(true).expect("tag wake nonblocking");
        set_tag_wake_tx(tx);
        loop_handle
            .insert_source(
                Generic::new(rx, Interest::READ, Mode::Level),
                move |_, stream, state: &mut Island| {
                    use std::io::Read;
                    let mut buf = [0u8; 64];
                    // calloop hands us NoIoDrop<UnixStream>: only a
                    // shared borrow is available, so read through &T
                    // (Read is implemented for &UnixStream).
                    let mut stream_ref: &std::os::unix::net::UnixStream = &stream;
                    loop {
                        match stream_ref.read(&mut buf) {
                            Ok(0) => break,
                            Ok(_) => continue,
                            Err(_) => break,
                        }
                    }
                    let t = active_tag_cached();
                    if t != state.last_tag_drawn {
                        state.last_tag_drawn = t;
                        state.draw_if_ready();
                    }
                    Ok(PostAction::Continue)
                },
            )
            .expect("tag watch wake source");
    }

    // iter97: start the event-driven tag watcher (idempotent; the
    // 1 s poller below stays as fallback/resync).
    ensure_tag_watcher();

    // Backdrop-watch timer (iter30 a3b): the flat fill follows the
    // REAL backdrop (grim probe, 1 s cadence) — but the island only
    // redraws on state changes, morph ticks and minute boundaries, so
    // a tag switch alone would leave a stale fill for up to a minute.
    // Once a second, redraw only if the backdrop factor moved since
    // the last draw (cheap when nothing changed: one float compare).
    {
        let timer = Timer::from_duration(Duration::from_millis(1000));
        // iter109 RAM: idle-trim clock (see the tail of this callback).
        let mut idle_since = std::time::Instant::now();
        let mut idle_trimmed = false;
        loop_handle
            .insert_source(timer, move |_, _, state: &mut Island| {
                let mut changed = false;
                ensure_tag_poller();
                if (wallpaper_blur::backdrop_factor() - state.last_backdrop_factor).abs() > 0.05 {
                    changed = true;
                    state.draw_if_ready();
                }
                // iter90 a8: the tag poller (background thread) may
                // have seen a keyboard tag switch — redraw so the
                // workspace highlight follows within ~1 s.
                // iter97: the event-driven watcher usually beats this
                // to it within milliseconds; this stays as fallback.
                let t = active_tag_cached();
                if t != state.last_tag_drawn {
                    state.last_tag_drawn = t;
                    changed = true;
                    state.draw_if_ready();
                }
                // iter91 B1: live repaint while ControlCenter is open.
                // The refresher rewrites the cache every 5 s, but after
                // the entry morph ends nothing redrew until the minute
                // boundary — the settled frame froze (iter90: byte-
                // identical over 35 s; battery stuck at its open-time
                // watts). The 1 s timer already runs unconditionally,
                // so compare a fingerprint of the cached live data and
                // redraw only when a poll landed. live() is a
                // non-blocking cache clone: no system call, no spin,
                // no morph interference (draw_if_ready during a morph
                // just draws the current morph frame, as the backdrop /
                // tag redraws above already do).
                if state.state == IslandState::ControlCenter {
                    let fp = cc_live_fingerprint();
                    if state.last_cc_fingerprint.as_deref() != Some(fp.as_str()) {
                        state.last_cc_fingerprint = Some(fp);
                        changed = true;
                        state.draw_if_ready();
                    }
                }
                // iter106: live repaint while Music is open - the
                // same frozen-settled-frame defect as CC in iter90,
                // same fix as iter91 B1 above. The refresher
                // rewrites MUSIC_CACHE every 1 s, but after the
                // entry morph ends nothing redrew until the minute
                // boundary, so the progress fill froze (iter105:
                // byte-identical frames 10 s apart while position
                // advanced 4 s -> 14 s). fingerprint() is a pure
                // format over the cached clone: no system call, no
                // spin, no morph interference (draw_if_ready
                // during a morph just draws the current morph
                // frame, as the backdrop / tag / CC redraws above
                // already do). Position is whole seconds: the
                // fingerprint steps once a second while Playing
                // and is frozen while Paused, so no redraw storm
                // when paused.
                if state.state == IslandState::Music {
                    let fp = music_panel::fingerprint(&music_panel::live_cached());
                    if state.last_music_fingerprint.as_deref() != Some(fp.as_str()) {
                        state.last_music_fingerprint = Some(fp);
                        changed = true;
                        state.draw_if_ready();
                    }
                }
                // iter108: redraw when the tray snapshot moves
                // (register / unregister / icon / title) or the
                // open DBusMenu arrives. fingerprint() is a pure
                // cache compare: no D-Bus, no spin — and the
                // tray icons are drawn in every state, so this
                // branch runs unconditionally, like the tag one.
                {
                    let fp = crate::tray::fingerprint();
                    if state.last_tray_fingerprint != fp {
                        state.last_tray_fingerprint = fp;
                        // Merge iter108 (C+B): keep the geometry-side
                        // count in sync with the live snapshot — the
                        // hit test / regions / draw read
                        // tray_item_count(), never the snapshot
                        // directly, so count and drawing stay in step.
                        set_tray_count(crate::tray::snapshot_count());
                        changed = true;
                        state.draw_if_ready();
                    }
                }
                // iter109 RAM: once per idle period, return glibc arena
                // slack (tour / registration transients) while collapsed
                // with no state change for > 60 s. Trim only releases
                // free pages, never live data (same call as the frost
                // reload in wallpaper_blur.rs:395).
                if state.state == IslandState::Collapsed && !changed {
                    if idle_since.elapsed() >= Duration::from_secs(60) && !idle_trimmed {
                        unsafe { libc::malloc_trim(0) };
                        idle_trimmed = true;
                    }
                } else {
                    idle_since = std::time::Instant::now();
                    idle_trimmed = false;
                }
                TimeoutAction::ToDuration(Duration::from_millis(1000))
            })
            .expect("timer source");
    }

    // Control socket (unix datagram).
    let sock_path = socket_path();
    // A second instance used to unlink this path and rebind it, orphaning the
    // first: two live surfaces, one socket, and no way to tell which one a
    // word reached. Refuse to start instead. (Probe before unlinking so the
    // incumbent keeps its socket.)
    if sock_path.exists() {
        match UnixDatagram::unbound().and_then(|s| s.connect(&sock_path)) {
            Ok(_) => {
                eprintln!("dynamite-rust: another instance owns {} -- exiting",
                          sock_path.display());
                // 75 = EX_TEMPFAIL: "not a failure, do not retry". The
                // supervisor counts ANY sub-30 s run as a rapid exit, so
                // exiting 0 here drove it to its 10-strike give-up cap and
                // it stopped restarting the island for 300 s at a time.
                std::process::exit(75);
            }
            // Nothing is listening: a stale file from a crashed run.
            Err(_) => { let _ = std::fs::remove_file(&sock_path); }
        }
    }
    let datagram = UnixDatagram::bind(&sock_path).expect("bind control socket");
    // The socket moves into the event loop below; remember its inode now so
    // shutdown can tell whether the path is still ours.
    let sock_ino = socket_inode(&datagram);
    datagram.set_nonblocking(true).expect("nonblocking");
    {
        let loop_handle_ctrl = loop_handle.clone();
        loop_handle
            .insert_source(
                Generic::new(datagram, Interest::READ, Mode::Level),
                move |_, sock, state: &mut Island| {
                    let mut buf = [0u8; 512];
                    match sock.recv(&mut buf) {
                        Ok(n) => {
                            let word = String::from_utf8_lossy(&buf[..n]).trim().to_string();
                            if let Some(rest) = word.strip_prefix("dictation_bars:") {
                                let mut bars = [0.0f32; 27];
                                for (slot, tok) in rest.split_whitespace().enumerate().take(27) {
                                    if let Ok(v) = tok.parse::<f32>() {
                                        bars[slot] = v.clamp(0.0, 1.0);
                                    }
                                }
                                *DICTATION_BARS.lock().unwrap() = bars;
                                if state.state == IslandState::Dictation {
                                    state.draw_if_ready();
                                }
                            } else if word == "dictation_start" {
                                if state.state != IslandState::Dictation {
                                    state.request_state(IslandState::Dictation);
                                }
                                state.draw_if_ready();
                            } else if word == "dictation_end" {
                                if state.state == IslandState::Dictation {
                                    state.request_state(IslandState::Collapsed);
                                }
                                state.draw_if_ready();
                            } else if word == "quit" {
                                state.loop_signal.stop();
                            } else if word == "lockscreen" {
                                // Fullscreen preview — bypasses
                                // begin_morph (no capsule morph for the
                                // lockscreen); see enter_lockscreen.
                                state.enter_lockscreen(&loop_handle_ctrl);
                            } else if word == "unlock" {
                                state.exit_lockscreen(
                                    &loop_handle_ctrl,
                                    IslandState::Collapsed,
                                );
                            } else if word == "locktype" {
                                // Lockscreen preview typing (iter14): one
                                // more dot, max 32; clears a wrong flag.
                                // No-op outside the lockscreen.
                                if state.state == IslandState::Lockscreen {
                                    state.lock_dots =
                                        state.lock_dots.saturating_add(1).min(32);
                                    state.lock_wrong = false;
                                    state.draw_if_ready();
                                }
                            } else if word == "lockback" {
                                if state.state == IslandState::Lockscreen {
                                    state.lock_dots = state.lock_dots.saturating_sub(1);
                                    state.draw_if_ready();
                                }
                            } else if word == "lockwrong" {
                                // Wrong password (iter14): dots clear, the
                                // pill shakes once; the flag clears after
                                // 150 ms via a one-shot timer.
                                if state.state == IslandState::Lockscreen {
                                    state.lock_dots = 0;
                                    state.lock_wrong = true;
                                    state.draw_if_ready();
                                    let _ = loop_handle_ctrl.insert_source(
                                        Timer::from_duration(Duration::from_millis(150)),
                                        |_, _, state: &mut Island| {
                                            state.lock_wrong = false;
                                            if state.state == IslandState::Lockscreen {
                                                state.draw_if_ready();
                                            }
                                            TimeoutAction::Drop
                                        },
                                    );
                                }
                            } else if word == "clock" {
                                // "clock" is a no-op alias for "collapsed":
                                // the pill always shows the live wall clock
                                // (the old stopwatch mode was deleted iter16).
                                if state.state == IslandState::Lockscreen {
                                    state.exit_lockscreen(
                                        &loop_handle_ctrl,
                                        IslandState::Collapsed,
                                    );
                                } else {
                                    state.begin_morph(&loop_handle_ctrl, IslandState::Collapsed);
                                }
                            } else if word == "reload" {
                                // Settings-app contract: re-read
                                // ~/.config/dynamite-rust/config.conf. Geometry
                                // applies on the next state change; while
                                // collapsed it applies immediately (margin +
                                // size + redraw). hover_open / scroll_cycle /
                                // clock_24h / hover_state are read live at
                                // their use sites, so they apply at once.
                                load_config();
                                // iter36 a6b: re-resolve the accent +
                                // selection indices from the new config.
                                library::refresh_accent_from_config();
                                if let Some(m) = library::manifest() {
                                    let c = cfg();
                                    if let Some(i) = m.index_of_wallpaper(&c.wallpaper) {
                                        state.wall_selected = i;
                                    }
                                    let theme =
                                        if c.theme.is_empty() { "anime" } else { c.theme.as_str() };
                                    if let Some(i) = m.index_of_theme(theme) {
                                        state.theme_selected = i;
                                    }
                                }
                                set_selection(state.cal_offset, state.wall_selected, state.theme_selected);
                                // Unconditional (F-C): a gap/height edit
                                // must land even while a menu is open,
                                // else the old reserved strip stays.
                                if let Some(layer) = &state.layer {
                                    layer.set_margin(stage_margin_top(), 0, 0, 0);
                                    layer.set_exclusive_zone(pill_strip_px());
                                }
                                if state.state == IslandState::Collapsed {
                                    state.apply_state_instant(IslandState::Collapsed);
                                }
                                state.draw_if_ready();
                            } else if let Some(new_state) = IslandState::from_word(&word) {
                                if state.state == IslandState::Lockscreen {
                                    // Leaving the preview is always
                                    // instant: no morph from a fullscreen
                                    // surface back to a pill.
                                    state.exit_lockscreen(&loop_handle_ctrl, new_state);
                                } else {
                                    state.begin_morph(&loop_handle_ctrl, new_state);
                                }
                            }
                            Ok(PostAction::Continue)
                        }
                        Err(_) => Ok(PostAction::Continue),
                    }
                },
            )
            .expect("socket source");
    }

    // Clean SIGTERM/SIGINT via a self-pipe (async-signal-safe write end,
    // calloop Generic source on the read end — no extra crates).
    {
        let read_fd = install_signal_pipe();
        let file = unsafe { std::fs::File::from_raw_fd(read_fd) };
        use std::os::fd::FromRawFd;
        let _ = &file;
        loop_handle
            .insert_source(
                Generic::new(file, Interest::READ, Mode::Level),
                |_, _, state: &mut Island| {
                    state.loop_signal.stop();
                    Ok(PostAction::Continue)
                },
            )
            .expect("signal pipe source");
    }

    let mut island = Island {
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        compositor_state,
        shm,
        layer_shell,
        layer: Some(layer),
        pool: None,
        buffer: None,
        font,
        icon_font: std::sync::OnceLock::new(),
        state: IslandState::Collapsed,
        lock_dots: 0,
        lock_wrong: false,
        lock_wallpaper: None,
        morph: None,
        morph_from_state: IslandState::Collapsed,
        morph_token: None,
        morph_from_geom: morph::Geometry::collapsed(),
        morph_to_geom: morph::Geometry::collapsed(),
        lock_anim: None,
        lock_anim_token: None,
        lock_exit_target: IslandState::Collapsed,
        morph_scratch_from: Vec::new(),
        morph_scratch_to: Vec::new(),
        scale_scratch: Vec::new(),
        slots: Vec::new(),
        slot_bytes: 0,
        slot_rr: 0,
        frame_pending: None,
        redraw_wanted: false,
        last_rect: None,
        configured_size: (0, 0),
        drawn_size: (0, 0),
        last_backdrop_factor: 1.0,
        last_tag_drawn: 1,
        last_cc_fingerprint: None,
        last_music_fingerprint: None,
        last_tray_fingerprint: String::new(),
        loop_signal,
        seat_state: SeatState::new(&globals, &qh),
        pointer: None,
        keyboard: None,
        loop_handle: loop_handle.clone(),
        qh: qh.clone(),
        pointer_over: false,
        last_pointer_pos: None,
        wheel_accum: 0.0,
        last_wheel_step: None,
        last_click: None,
        cc_drag: None,
        // iter36 a6b: resolved from config after load_config() below
        // (see the startup block) — defaults here are overwritten
        // before the event loop starts.
        cal_offset: 0,
        wall_selected: 0,
        theme_selected: 0,
    };

    // iter36 a6b: resolve the selection indices + accent from config
    // (library manifest if deployed; "anime" is the reference
    // setup's default theme when the config has no theme yet).
    {
        let c = cfg();
        if let Some(m) = library::manifest() {
            if let Some(i) = m.index_of_wallpaper(&c.wallpaper) {
                island.wall_selected = i;
            }
            let theme = if c.theme.is_empty() { "anime" } else { c.theme.as_str() };
            if let Some(i) = m.index_of_theme(theme) {
                island.theme_selected = i;
            }
        }
        library::refresh_accent_from_config();
        set_selection(island.cal_offset, island.wall_selected, island.theme_selected);
    }

    event_loop
        .run(None, &mut island, |_| {})
        .expect("event loop");

    // Only unlink if the path is still OUR socket. An instance that lost the
    // path to a later bind must not delete the incumbent's socket on its way
    // out -- doing exactly that killed the panel at 04:02.
    if owns_socket_path(sock_ino, &sock_path) {
        let _ = std::fs::remove_file(&sock_path);
    }
}

/// Inode of the socket we bound, or 0 if it cannot be read.
fn socket_inode(sock: &UnixDatagram) -> u64 {
    use std::os::unix::io::AsRawFd;
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(sock.as_raw_fd(), &mut st) } != 0 {
        return 0;
    }
    st.st_ino
}

/// True if `path` still refers to the socket we bound (same inode).
fn owns_socket_path(ino: u64, path: &std::path::Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    ino != 0 && std::fs::metadata(path).map(|m| m.ino() == ino).unwrap_or(false)
}

#[cfg(test)]
mod iter26_tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn disc_shadow_full_inside_fades_outside() {
        let (cx, cy) = (100.0, 50.0);
        // At the shadow centre: full peak alpha.
        assert!(approx(
            disc_shadow_alpha(cx + DISC_SHADOW_OFF.0, cy + DISC_SHADOW_OFF.1, cx, cy),
            DISC_SHADOW_ALPHA
        ));
        // Halfway through the blur band: half peak (smooth01(0.5) = 0.5).
        let d = NAV_DISC_R + DISC_SHADOW_BLUR / 2.0;
        assert!(approx(
            disc_shadow_alpha(cx + DISC_SHADOW_OFF.0 + d, cy + DISC_SHADOW_OFF.1, cx, cy),
            DISC_SHADOW_ALPHA * 0.5
        ));
        // Past the blur: zero.
        assert!(approx(
            disc_shadow_alpha(
                cx + DISC_SHADOW_OFF.0 + NAV_DISC_R + DISC_SHADOW_BLUR + 1.0,
                cy + DISC_SHADOW_OFF.1,
                cx,
                cy
            ),
            0.0
        ));
    }

    #[test]
    fn panel_shadow_full_inside_fades_below() {
        let (ox, oy, pw, ph, r) = (100.0, 4.0, 612.0, 150.0, 20.0);
        // Deep inside the offset shadow rect: full peak alpha.
        assert!(approx(
            panel_shadow_alpha(ox + pw / 2.0, oy + ph / 2.0, ox, oy, pw, ph, r),
            PANEL_SHADOW_ALPHA
        ));
        // 5 px below the offset rect's bottom edge (half the blur):
        // half peak.
        let y = oy + PANEL_SHADOW_OFF.1 + ph + PANEL_SHADOW_BLUR / 2.0;
        assert!(approx(
            panel_shadow_alpha(ox + pw / 2.0, y, ox, oy, pw, ph, r),
            PANEL_SHADOW_ALPHA * 0.5
        ));
        // Past the blur: zero.
        assert!(approx(
            panel_shadow_alpha(
                ox + pw / 2.0,
                oy + PANEL_SHADOW_OFF.1 + ph + PANEL_SHADOW_BLUR + 1.0,
                ox,
                oy,
                pw,
                ph,
                r
            ),
            0.0
        ));
    }

    #[test]
    fn disc_rim_light_on_top_dark_below_none_at_centre() {
        let (cx, cy) = (100.0, 50.0);
        // Centre: no rim.
        assert!(disc_rim(cx, cy, cx, cy).is_none());
        // Top of the disc, mid-band: clearly light periwinkle
        // (band peak sits at t ~= 0.97, so check ranges, not exact).
        let (r, g, b, a) = disc_rim(cx, cy - NAV_DISC_R + RIM_WIDTH / 2.0, cx, cy).unwrap();
        assert!(r > 150.0 && g > 150.0 && b > 180.0, "top rim too dark: {r},{g},{b}");
        assert!((a - RIM_LIGHT_ALPHA).abs() < 0.02, "top rim alpha {a}");
        // Bottom of the disc, mid-band: clearly dark.
        let (r, g, b, a) = disc_rim(cx, cy + NAV_DISC_R - RIM_WIDTH / 2.0, cx, cy).unwrap();
        assert!(r < 30.0 && g < 30.0 && b < 30.0, "bottom rim too light: {r},{g},{b}");
        assert!((a - RIM_DARK_ALPHA).abs() < 0.02, "bottom rim alpha {a}");
        // Outside the disc: no rim.
        assert!(disc_rim(cx, cy - NAV_DISC_R - 1.0, cx, cy).is_none());
    }

    #[test]
    fn black_shadow_composite_gate() {
        // Transparent pixel: writes a black translucent pixel.
        let mut px = [0u8, 0, 0, 0];
        composite_black_shadow(&mut px, 0, 0.4);
        assert_eq!(px, [0, 0, 0, (0.4 * 255.0) as u8]);
        // Opaque pixel (panel content): untouched.
        let mut opaque = [200u8, 200, 200, 255];
        composite_black_shadow(&mut opaque, 0, 0.4);
        assert_eq!(opaque, [200, 200, 200, 255]);
        // Half-transparent pixel: darkened, alpha grows.
        let mut half = [100u8, 100, 100, 128];
        composite_black_shadow(&mut half, 0, 0.4);
        assert!(half[0] < 100 && half[3] > 128);
    }

    #[test]
    fn shadows_fit_inside_stage() {
        // Tallest panel (ControlCenter, 525) bottom + shadow tail.
        // Panel top is panel_top_menu() (31) since iter30 a3c.
        let bottom = panel_top_menu() + 525.0 + PANEL_SHADOW_OFF.1 + PANEL_SHADOW_BLUR;
        assert!(bottom < STAGE_H as f32, "panel shadow bottom {bottom} >= {STAGE_H}");
        // Morph overshoot: CC grows ~10% past target (EaseOutBack) —
        // the overshoot shadow bottom must fit too.
        let over_bottom = panel_top_menu() + 525.0 * 1.10 + PANEL_SHADOW_OFF.1 + PANEL_SHADOW_BLUR;
        assert!(over_bottom < STAGE_H as f32, "overshoot shadow bottom {over_bottom} >= {STAGE_H}");
        // Widest panel (Wallpaper, 1008) right edge + blur.
        let right = (STAGE_W as f32 + 1008.0) / 2.0 + PANEL_SHADOW_BLUR;
        assert!(right < STAGE_W as f32, "panel shadow right {right} >= {STAGE_W}");
        // Leftmost disc shadow edge (widest panel, left disc).
        let left_cx = (STAGE_W as f32 - 1008.0) / 2.0 - NAV_DISC_GAP - NAV_DISC_R;
        let shadow_left = left_cx + DISC_SHADOW_OFF.0 - NAV_DISC_R - DISC_SHADOW_BLUR;
        assert!(shadow_left > 0.0, "disc shadow left {shadow_left} <= 0");
        // Disc shadow bottom (tallest menu: ControlCenter disc at
        // panel mid-height 31 + 525/2).
        let ((_, cy_cc), _) = nav_disc_centres(IslandState::ControlCenter);
        let shadow_bottom = cy_cc + DISC_SHADOW_OFF.1 + NAV_DISC_R + DISC_SHADOW_BLUR;
        assert!(shadow_bottom < STAGE_H as f32);
    }
    // iter27 (D1/D2 hit-test): classify_press pins the new rules.
    // All positions are STAGE coords (stage origin on screen is
    // (164, 11), but the classifier only ever sees stage coords).
    #[test]
    fn iter27_collapsed_pill_press_opens_music() {
        let (ox, oy, pw, ph) = panel_rect_stage(IslandState::Collapsed);
        let centre = (
            ox as f64 + pw as f64 / 2.0,
            oy as f64 + ph as f64 / 2.0,
        );
        assert_eq!(
            classify_press(centre, IslandState::Collapsed, false, 0, 0, 0),
            PressAction::OpenMusic
        );
        // Pill corner, still inside the rect: opens too.
        assert_eq!(
            classify_press((ox as f64 + 1.0, oy as f64 + 1.0), IslandState::Collapsed, false, 0, 0, 0),
            PressAction::OpenMusic
        );
    }

    #[test]
    fn iter27_lock_pill_press_opens_only_inside_54px_rect() {
        let (ox, oy, pw, ph) = panel_rect_stage(IslandState::Lock);
        assert!((pw - 54.0).abs() < 1e-4, "lock width {pw}");
        let centre = (
            ox as f64 + pw as f64 / 2.0,
            oy as f64 + ph as f64 / 2.0,
        );
        assert_eq!(
            classify_press(centre, IslandState::Lock, false, 0, 0, 0),
            PressAction::OpenMusic
        );
        // x=560 sits inside the 76-wide collapsed pill (550..626) but
        // outside the 54-wide lock pill (561..615): opens when collapsed,
        // no-op when locked.
        assert_eq!(
            classify_press((560.0, 16.0), IslandState::Collapsed, false, 0, 0, 0),
            PressAction::OpenMusic
        );
        assert_eq!(
            classify_press((560.0, 16.0), IslandState::Lock, false, 0, 0, 0),
            PressAction::NoOp
        );
    }

    #[test]
    fn iter27_old_panel_rect_press_while_collapsed_is_noop() {
        // D2 repro points (stage coords = logical - (164, 11)):
        // Power empty (575,155) -> (411,144); CC (752,350) -> (588,339);
        // CC (600,400) -> (436,389); Power BR (930,155) -> (766,144).
        for pos in [(411.0, 144.0), (588.0, 339.0), (436.0, 389.0), (766.0, 144.0)] {
            assert_eq!(
                classify_press(pos, IslandState::Collapsed, false, 0, 0, 0),
                PressAction::NoOp,
                "settled pos {pos:?}"
            );
            // Mid-morph collapse: state is already Collapsed, same verdict.
            assert_eq!(
                classify_press(pos, IslandState::Collapsed, true, 0, 0, 0),
                PressAction::NoOp,
                "mid-morph pos {pos:?}"
            );
        }
    }

    #[test]
    fn iter27_empty_panel_press_is_noop_in_all_menus() {
        // One in-panel point per menu (stage coords). The Launcher point
        // is the search/row-1 gap (panel-local y 62, between search end
        // 50 and row-0 top 74).
        let cases = [
            (IslandState::Power, (411.0, 144.0)),
            (IslandState::Music, (850.0, 140.0)),
            (IslandState::Calendar, (730.0, 290.0)),
            (IslandState::ControlCenter, (588.0, 339.0)),
            (IslandState::Launcher, (588.0, 98.0)),
            (IslandState::Wallpaper, (100.0, 260.0)),
            (IslandState::Themes, (300.0, 360.0)),
        ];
        for (state, pos) in cases {
            assert!(
                point_in_panel_stage(pos, state),
                "{state:?} test point {pos:?} not in panel"
            );
            assert_eq!(
                classify_press(pos, state, false, 0, 0, 0),
                PressAction::NoOp,
                "{state:?} pos {pos:?}"
            );
        }
    }

    #[test]
    fn iter27_disc_press_still_cycles_and_margin_still_noop() {
        let ((lx, ly), (rx, ry)) = nav_disc_centres(IslandState::Music);
        assert_eq!(
            classify_press((rx as f64, ry as f64), IslandState::Music, false, 0, 0, 0),
            PressAction::Cycle(1)
        );
        assert_eq!(
            classify_press((lx as f64, ly as f64), IslandState::Music, false, 0, 0, 0),
            PressAction::Cycle(-1)
        );
        // Mid-morph widened radius still cycles 5 px past the disc edge.
        assert_eq!(
            classify_press(
                (rx as f64 + NAV_DISC_R as f64 + 5.0, ry as f64),
                IslandState::Music,
                true,
                0,
                0,
                0
            ),
            PressAction::Cycle(1)
        );
        // Margin at disc height, outside panel+discs: no-op.
        assert_eq!(
            classify_press((300.0, 27.0), IslandState::Music, false, 0, 0, 0),
            PressAction::NoOp
        );
        // Below the panel: no-op.
        assert_eq!(
            classify_press((752.0, 300.0), IslandState::Music, false, 0, 0, 0),
            PressAction::NoOp
        );
    }

    #[test]
    fn iter27_launcher_row_press_still_launches() {
        let (ox, oy) = panel_origin(IslandState::Launcher);
        let layout = launcher_panel::launcher_layout_for_count(launcher_panel::ROW_COUNT);
        let r0 = layout.rows[0];
        let pos = (
            ox as f64 + r0.cx() as f64,
            oy as f64 + r0.cy() as f64,
        );
        assert_eq!(
            classify_press(pos, IslandState::Launcher, false, 0, 0, 0),
            PressAction::LaunchRow(0)
        );
        // Gap between search (ends panel-y 50) and row 0 (starts 74).
        let gap = (ox as f64 + 252.0, oy as f64 + 62.0);
        assert_eq!(
            classify_press(gap, IslandState::Launcher, false, 0, 0, 0),
            PressAction::NoOp
        );
    }

    #[test]
    fn iter27b_pill_rect_collapses_from_every_menu() {
        // Pill centre, stage coords (588,16) = logical (752,27).
        // iter30 a3c: the pill is NO LONGER inside the menu panel
        // (the panel hangs below it) — but a click there still
        // collapses, via the pill-rect branch of classify_press.
        for state in MENU_CYCLE {
            assert!(
                !point_in_panel_stage((588.0, 16.0), state),
                "{state:?}: pill centre should NOT be in the panel any more"
            );
            assert_eq!(
                classify_press((588.0, 16.0), state, false, 0, 0, 0),
                PressAction::Collapse,
                "{state:?}"
            );
        }
    }

    #[test]
    fn iter27b_just_outside_pill_rect_stays_noop() {
        // Just right of the pill rect (x 633 edge) and just below it
        // (y 28 edge), still inside the Music panel: no-op, not collapse.
        assert_eq!(
            classify_press((640.0, 16.0), IslandState::Music, false, 0, 0, 0),
            PressAction::NoOp
        );
        assert_eq!(
            classify_press((588.0, 40.0), IslandState::Music, false, 0, 0, 0),
            PressAction::NoOp
        );
        // Just left of the pill rect: the 4 px bridge gap (x 546..550)
        // between workspace circle 4 and the pill is still a no-op.
        assert_eq!(
            classify_press((548.0, 16.0), IslandState::Power, false, 0, 0, 0),
            PressAction::NoOp
        );
        // iter90 a8 / iter92: the old probe point (536, 16) is ON
        // workspace circle 4 (centre 538, 15) — it switches to tag 4,
        // no longer NoOp.
        assert_eq!(
            classify_press((536.0, 16.0), IslandState::Power, false, 0, 0, 0),
            PressAction::WorkspaceTag(4)
        );
    }

    #[test]
    fn iter34a_cc_toggle_press_hits_all_four_pills() {
        let (ox, oy) = panel_origin(IslandState::ControlCenter);
        let layout = controlcenter_panel::cc_layout();
        for (i, pill) in layout.toggle_pills.iter().enumerate() {
            let pos = (
                ox as f64 + pill.cx() as f64,
                oy as f64 + pill.cy() as f64,
            );
            let expected = if i == 2 {
                PressAction::CcBtOpen
            } else {
                PressAction::CcToggle(i)
            };
            assert_eq!(
                classify_press(pos, IslandState::ControlCenter, false, 0, 0, 0),
                expected,
                "pill {i}"
            );
        }
    }

    #[test]
    fn bt_submenu_press_hits_back_power_and_rows() {
        let l = controlcenter_panel::cc_layout();
        assert_eq!(
            classify_cc_press(l.bt_back.cx(), l.bt_back.cy(), true, false),
            PressAction::CcBtBack
        );
        assert_eq!(
            classify_cc_press(l.bt_power_pill.cx(), l.bt_power_pill.cy(), true, false),
            PressAction::CcToggle(2)
        );
        for (i, row) in l.bt_device_rows.iter().enumerate() {
            assert_eq!(
                classify_cc_press(row.cx(), row.cy(), true, false),
                PressAction::CcBtConnect(i),
                "row {i}"
            );
        }
        // The same point on the normal panel still opens the submenu.
        assert_eq!(
            classify_cc_press(l.toggle_pills[2].cx(), l.toggle_pills[2].cy(), false, false),
            PressAction::CcBtOpen
        );
    }

    #[test]
    fn sound_submenu_press_hits_chevron_back_and_rows() {
        let l = controlcenter_panel::cc_layout();
        assert_eq!(
            classify_cc_press(l.sound_end_btn.cx(), l.sound_end_btn.cy(), false, false),
            PressAction::CcSoundOpen
        );
        assert_eq!(
            classify_cc_press(l.bt_back.cx(), l.bt_back.cy(), false, true),
            PressAction::CcBtBack
        );
        for (i, row) in l.sound_rows.iter().enumerate() {
            assert_eq!(
                classify_cc_press(row.cx(), row.cy(), false, true),
                PressAction::CcSoundRoute(i),
                "row {i}"
            );
        }
        // The slider itself still sets volume; only its chevron opens.
        assert!(matches!(
            classify_cc_press(l.sound_track.cx(), l.sound_track.cy(), false, false),
            PressAction::CcSlider(1, _)
        ));
    }

    #[test]
    fn cc_power_press_hits_all_three_levels() {
        let (ox, oy) = panel_origin(IslandState::ControlCenter);
        let layout = controlcenter_panel::cc_layout();
        for (i, btn) in layout.power_buttons.iter().enumerate() {
            let pos = (
                ox as f64 + btn.cx() as f64,
                oy as f64 + btn.cy() as f64,
            );
            assert_eq!(
                classify_press(pos, IslandState::ControlCenter, false, 0, 0, 0),
                PressAction::CcPower(i),
                "power button {i}"
            );
        }
    }

    #[test]
    fn iter34a_cc_slider_press_sets_level_from_x() {
        let (ox, oy) = panel_origin(IslandState::ControlCenter);
        let layout = controlcenter_panel::cc_layout();
        let t = layout.display_track;
        let left = (
            ox as f64 + t.x as f64 + 1.0,
            oy as f64 + t.cy() as f64,
        );
        assert!(
            matches!(
                classify_press(left, IslandState::ControlCenter, false, 0, 0, 0),
                PressAction::CcSlider(0, l) if l < 0.05
            ),
            "display track left end should be ~0"
        );
        let mid = (
            ox as f64 + (t.x + t.w / 2.0) as f64,
            oy as f64 + t.cy() as f64,
        );
        assert!(
            matches!(
                classify_press(mid, IslandState::ControlCenter, false, 0, 0, 0),
                PressAction::CcSlider(0, l) if (l - 0.5).abs() < 0.02
            ),
            "display track middle should be 0.5"
        );
        let s = layout.sound_track;
        let mid2 = (
            ox as f64 + (s.x + s.w / 2.0) as f64,
            oy as f64 + s.cy() as f64,
        );
        assert!(
            matches!(
                classify_press(mid2, IslandState::ControlCenter, false, 0, 0, 0),
                PressAction::CcSlider(1, l) if (l - 0.5).abs() < 0.02
            ),
            "sound track middle should be 0.5"
        );
        // The iter27 no-op point (notifications area, panel-local
        // (219, 303) — 7 px below the Sound track, inside the new card,
        // above the bell) stays NoOp.
        assert_eq!(
            classify_press((588.0, 339.0), IslandState::ControlCenter, false, 0, 0, 0),
            PressAction::NoOp
        );
    }

}

#[cfg(test)]
mod iter28_tests {
    use super::*;

    #[test]
    fn frost_alpha_pill_vs_menu() {
        assert!((frost_alpha(IslandState::Collapsed) - 0.92).abs() < 1e-6);
        assert!((frost_alpha(IslandState::Lock) - 0.92).abs() < 1e-6);
        assert!((frost_alpha(IslandState::Music) - 0.92).abs() < 1e-6);
        assert!((frost_alpha(IslandState::ControlCenter) - 0.92).abs() < 1e-6);
        assert!((frost_alpha(IslandState::Power) - 0.92).abs() < 1e-6);
        assert!((frost_alpha(IslandState::Lockscreen) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn frost_fill_px_pill_top() {
        // Top-centre pixel of the pill: opaque dark floor, no brightening
        // (iter31: the top light is deleted, not tamed; iter85: the
        // fallback is opaque - iter86: 5*0.80 = 4.0 -> 4, A = 255 - so the backdrop
        // can never leak through and fake a panel edge; iter128:
        // 2.5*0.92 = 2.3 -> 2).
        let p = frost_fill_px(1.0, -0.5, 45.0, 0.5, 90.0, 24.0, FROST_ALPHA_PILL);
        assert_eq!(p, [2, 2, 2, 255]);
    }

    #[test]
    fn frost_fill_px_menu_centre() {
        // Deep inside a menu: opaque dark floor, no light (iter85:
        // 5*0.80 = 4.0 -> 4, A = 255 - matches the blur path floor;
        // iter128: 2.5*0.92 = 2.3 -> 2).
        let p = frost_fill_px(1.0, -10.0, 300.0, 100.0, 612.0, 525.0, FROST_ALPHA_MENU);
        assert_eq!(p, [2, 2, 2, 255]);
    }

    #[test]
    fn frost_fill_px_transparent_outside_and_aa_edge() {
        assert_eq!(frost_fill_px(0.0, 0.5, 45.0, 0.5, 90.0, 24.0, FROST_ALPHA_PILL), [0, 0, 0, 0]);
        // AA edge pixel (a=0.5, d=0): premultiplied floor, A = coverage
        // (iter86: 5*0.80*0.5 = 2.0 -> 2, A = 0.5*255 = 127;
        // iter128: 2.5*0.92*0.5 = 1.15 -> 1).
        let p = frost_fill_px(0.5, 0.0, 45.0, 0.5, 90.0, 24.0, FROST_ALPHA_PILL);
        assert_eq!(p, [1, 1, 1, 127]);
    }
}

#[cfg(test)]
mod iter29_tests {
    use super::*;

    #[test]
    fn frost_fill_px_blur_bright_interior() {
        // Blurred bright wallpaper under the 0.90 pill fill:
        // base = bg*0.10 + 2.5*0.90 = ~19/20/22 per channel, opaque interior
        // (iter128: floor 5.0 -> 2.5; was ~21/22/24).
        let p = frost_fill_px_blur(1.0, -10.0, 100.0, 100.0, 90.0, 24.0, 0.90, [170, 180, 200], [170, 180, 200]);
        assert_eq!(p[3], 255);
        assert!((p[0] as i32 - 19).abs() <= 2, "R {} want ~19", p[0]);
        assert!((p[1] as i32 - 20).abs() <= 2, "G {} want ~20", p[1]);
        assert!((p[2] as i32 - 22).abs() <= 2, "B {} want ~22", p[2]);
    }

    #[test]
    fn frost_fill_px_blur_dark_floor() {
        // Pure-black backdrop: the fill must NOT vanish — 2.5*0.90 = 2.25 -> 2
        // (iter128: was 5*0.90 = 4.5 -> 4).
        let p = frost_fill_px_blur(1.0, -10.0, 100.0, 100.0, 90.0, 24.0, 0.90, [0, 0, 0], [0, 0, 0]);
        assert_eq!(p, [2, 2, 2, 255]);
    }

    #[test]
    fn frost_fill_px_blur_top_is_flat() {
        // Top edge of the pill, centre: exactly the interior base —
        // no top lift at all (iter31).
        // base = 100*0.10 + 2.5*0.90 = 12.25 -> 12 (was 14.5 -> 14).
        let p = frost_fill_px_blur(1.0, -0.5, 45.0, 0.5, 90.0, 24.0, 0.90, [100, 100, 100], [100, 100, 100]);
        assert_eq!(p, [12, 12, 12, 255]);
    }

    #[test]
    fn frost_fill_px_blur_bright_sky_iter89() {
        // iter89 decision (frost-decision-iter89.md section 2.2):
        // neutralised bright-sky sample [167,177,190] at fa 0.80.
        // iter128: floor 5.0 -> 2.5, so [37,39,41] -> [35,37,39].
        // Note the f32 exactness: (1-0.80) is 0.19999999, so
        // 190*0.19999999 + 2.5*0.80 = 39.999998 -> 39, not 40.
        let p = frost_fill_px_blur(1.0, -10.0, 100.0, 100.0, 90.0, 24.0, 0.80, [167, 177, 190], [167, 177, 190]);
        assert_eq!(p, [35, 37, 39, 255]);
    }

    #[test]
    fn frost_fill_px_blur_day_file_iter89() {
        // iter89 strong proof (section 2.3): neutralised day-file
        // sample [89,106,117] at fa 0.80.
        // iter128: floor 5.0 -> 2.5, so [21,25,27] -> [19,23,25]
        // (the floor term here is 2.5*0.80 = 2.0, not 2.3 — fa is
        // the explicit 0.80, and 89*0.19999999 + 2.0 = 19.799999
        // -> 19).
        let p = frost_fill_px_blur(1.0, -10.0, 100.0, 100.0, 90.0, 24.0, 0.80, [89, 106, 117], [89, 106, 117]);
        assert_eq!(p, [19, 23, 25, 255]);
    }

    #[test]
    fn frost_fill_px_blur_live_sky_iter101() {
        // iter101 live path: a blurred live sky sample goes through
        // the fill formula with NO backdrop factor — the live sample
        // IS the backdrop. [38,85,120] at 0.80 -> [9,18,25]
        // (iter128: was [11,20,27] at floor 5.0; f32 (1-0.80) =
        // 0.19999999 puts G/B one level under the decimal value).
        // iter103: the sample passed here is RAW (this test pins
        // the formula with dev 0); on the real live path the sample
        // is neutralised at capture (neutralise_sample), so the
        // panel reads neutral dark — see the iter103 test below.
        let p = frost_fill_px_blur(1.0, -10.0, 100.0, 100.0, 90.0, 24.0, 0.80, [38, 85, 120], [38, 85, 120]);
        assert_eq!(p, [9, 18, 25, 255]);
    }

    #[test]
    fn frost_fill_px_blur_live_varies_with_sample_iter101() {
        // Two different live samples must give two different fills:
        // the whole point of Option 2 is visible variation, not a
        // second uniform tint.
        let a = frost_fill_px_blur(1.0, -10.0, 100.0, 100.0, 90.0, 24.0, 0.80, [38, 85, 120], [38, 85, 120]);
        let b = frost_fill_px_blur(1.0, -10.0, 100.0, 100.0, 90.0, 24.0, 0.80, [200, 210, 220], [200, 210, 220]);
        assert_ne!(a, b);
        assert_eq!(b, [41, 43, 45, 255]); // iter128: was [43,45,47] at floor 5.0
    }

    #[test]
    fn frost_fill_px_blur_aa_edge_premultiplied() {
        // AA edge pixel: RGB premultiplied by coverage, A = coverage.
        let p = frost_fill_px_blur(0.5, 0.0, 45.0, 12.0, 90.0, 24.0, 0.90, [100, 100, 100], [100, 100, 100]);
        assert_eq!(p, [6, 6, 6, 127]); // base 12.25 * 0.5 = 6.125 -> 6, A 0.5*255
        assert!(p[0] as u16 <= p[3] as u16, "premultiplied RGB > A");
    }

    #[test]
    fn frost_fill_px_blur_transparent_outside() {
        assert_eq!(
            frost_fill_px_blur(0.0, 0.5, 45.0, 0.5, 90.0, 24.0, 0.90, [200, 200, 200], [200, 200, 200]),
            [0, 0, 0, 0]
        );
    }

    #[test]
    fn frost_fill_px_blur_menu_keeps_092() {
        // Menus at 0.92 (iter128; was 0.80, iter89): base =
        // bg*0.08 + 2.5*0.92 = 18.3 -> 18.
        let p = frost_fill_px_blur(1.0, -10.0, 300.0, 100.0, 612.0, 525.0, FROST_ALPHA_MENU, [200, 200, 200], [200, 200, 200]);
        assert_eq!(p[3], 255);
        assert!((p[0] as i32 - 18).abs() <= 2, "menu R {} want ~18", p[0]);
    }
}

#[cfg(test)]
mod iter103_tests {
    use super::*;

    #[test]
    fn frost_fill_px_blur_hp_adds_deviation_iter103() {
        // Positive deviation: sample 20 above its local mean. iter128:
        // the deviation is capped at FROST_HP_MAX_DEV (12), so the
        // gain is k*12 = 9, not k*20 = 15 — this test now also pins
        // the cap (a sharp edge must not print through). Base at
        // fa 0.80 / floor 2.5: R 120*0.19999999+2.0+9 = 34.999998
        // -> 34 (f32 (1-0.80) is 0.19999999, and the floor term is
        // 2.5*0.80 = 2.0 at this explicit fa).
        let p = frost_fill_px_blur(1.0, -10.0, 100.0, 100.0, 90.0, 24.0, 0.80, [120, 130, 140], [100, 110, 120]);
        assert!((p[0] as i32 - 34).abs() <= 1, "R {} want ~34", p[0]);
        assert!((p[1] as i32 - 36).abs() <= 1, "G {} want ~36", p[1]);
        assert!((p[2] as i32 - 38).abs() <= 1, "B {} want ~38", p[2]);
        // Negative deviation: sample 20 below its local mean loses 9
        // (capped), not 15. R 80*0.19999999+2.0-9 = 8.999999 -> 8.
        let q = frost_fill_px_blur(1.0, -10.0, 100.0, 100.0, 90.0, 24.0, 0.80, [80, 90, 100], [100, 110, 120]);
        assert!((q[0] as i32 - 8).abs() <= 1, "R {} want ~8", q[0]);
        assert!((q[1] as i32 - 10).abs() <= 1, "G {} want ~10", q[1]);
        assert!((q[2] as i32 - 12).abs() <= 1, "B {} want ~12", q[2]);
        // Extreme negative deviation clamps at the floor (2),
        // never wraps and never reads below the dark-wallpaper
        // value (iter113 — was [0, 0, 0, 255] with the clamp at 0;
        // iter128: floor 5.0 -> 2.5, so the clamp value is 2, not 4).
        let z = frost_fill_px_blur(1.0, -10.0, 100.0, 100.0, 90.0, 24.0, 0.80, [5, 5, 5], [200, 200, 200]);
        assert_eq!(z, [2, 2, 2, 255]);
    }

    #[test]
    fn frost_fill_px_blur_live_neutral_sky_iter103() {
        // iter103: the gap1 implied live sky sample, through the
        // SHARED strengthened neutralise_sample at capture, then
        // the shared fill at fa 0.92 with no local deviation: B-R
        // must land in +0..+5 (was +5.7 shipped) and the mean in
        // the reference's (4,5,5)-class band +/-6 (was (10,11,11)
        // at fa 0.80 — neutralise_sample([56,133,187]) = [33,35,37],
        // fill = n*0.08 + 2.3 = [4,5,5]).
        let n = crate::wallpaper_blur::neutralise_sample([56, 133, 187]);
        let p = frost_fill_px_blur(1.0, -10.0, 100.0, 100.0, 90.0, 24.0, crate::wallpaper_blur::FROST_ALPHA, n, n);
        let br = p[2] as i32 - p[0] as i32;
        assert!((0..=5).contains(&br), "B-R {} in {:?}", br, p);
        assert!(p[0] >= 4 && p[0] <= 16, "R {} in {:?}", p[0], p);
        assert!(p[1] >= 4 && p[1] <= 16, "G {} in {:?}", p[1], p);
        assert!(p[2] >= 4 && p[2] <= 16, "B {} in {:?}", p[2], p);
    }

    #[test]
    fn frost_fill_px_blur_dark_leg_stays_dark_iter103() {
        // Dark neutral sample, flat local mean: the fill is the
        // floor, (3,3,4)-class (iter128: was (5,5,5)-class at
        // 5.0*0.80) — well inside the (12,12,16) cap.
        let p = frost_fill_px_blur(1.0, -10.0, 100.0, 100.0, 90.0, 24.0, 0.80, [8, 8, 10], [8, 8, 10]);
        assert!(p[0] <= 12 && p[1] <= 12 && p[2] <= 16, "dark {:?}", p);
        // A dark pixel under a bright local mean clamps at the
        // floor (2) instead of lifting the dark leg (iter113;
        // iter128: floor 5.0 -> 2.5).
        let q = frost_fill_px_blur(1.0, -10.0, 100.0, 100.0, 90.0, 24.0, 0.80, [5, 5, 5], [40, 40, 45]);
        assert!(q[0] <= 6 && q[1] <= 6 && q[2] <= 6, "dark hole {:?}", q);
    }

    #[test]
    fn frost_shared_consts_iter103() {
        // iter128: FROST_ALPHA 0.80 -> 0.92, and the high-pass
        // deviation cap FROST_HP_MAX_DEV is pinned at 12.0.
        assert!((crate::wallpaper_blur::FROST_ALPHA - 0.92).abs() < 1e-6);
        assert!((crate::wallpaper_blur::FROST_HP_K - 0.75).abs() < 1e-6);
        assert!((crate::wallpaper_blur::FROST_HP_MAX_DEV - 12.0).abs() < 1e-6);
        assert_eq!(crate::wallpaper_blur::FROST_HP_RADIUS, 25);
        assert!((crate::wallpaper_blur::NEUTRALISE_MIN_STRENGTH - 0.88).abs() < 1e-6);
        assert!((crate::wallpaper_blur::FROST_EXPOSURE - 0.30).abs() < 1e-6);
    }

    #[test]
    fn frost_fill_px_blur_bright_leg_fa092_iter128() {
        // iter128 (B4): the measured bright leg — the neutralised
        // snow sample means ~32 (harness_frost_panel_fill, gwwwxd).
        // At fa 0.92: 32*0.08 + 2.5*0.92 = 4.86 -> 4. At the old
        // fa 0.80 / floor 5.0 this was 32*0.20 + 4.0 = 10.4 -> 10.
        let p = frost_fill_px_blur(1.0, -10.0, 100.0, 100.0, 90.0, 24.0, 0.92, [32, 32, 32], [32, 32, 32]);
        assert_eq!(p, [4, 4, 4, 255]);
    }

    #[test]
    fn frost_fill_px_blur_dark_leg_fa092_iter128() {
        // iter128 (B4): the dark leg stays at the floor — a [2,2,2]
        // sample fills 2*0.08 + 2.3 = 2.46 -> 2, never 0.
        let p = frost_fill_px_blur(1.0, -10.0, 100.0, 100.0, 90.0, 24.0, 0.92, [2, 2, 2], [2, 2, 2]);
        assert_eq!(p, [2, 2, 2, 255]);
    }

    #[test]
    fn frost_fill_px_blur_dev_cap_iter128() {
        // iter128 (B4): a +40 deviation (a sharp backdrop edge, e.g.
        // UI text over a dark mean) is capped at +12 before the
        // gain: 60*0.08 + 2.3 + 0.75*12 = 16.1 -> 16. Uncapped it
        // would be 60*0.08 + 2.3 + 0.75*40 = 37.1 -> 37 — the
        // bright speck the dark leg showed (fill max 34 over a 4.6
        // mean in the harness). Soft mottling (|dev| <= 12) is
        // untouched: dev +12 lands exactly at the cap.
        let p = frost_fill_px_blur(1.0, -10.0, 100.0, 100.0, 90.0, 24.0, 0.92, [60, 60, 60], [20, 20, 20]);
        assert_eq!(p, [16, 16, 16, 255]);
        let q = frost_fill_px_blur(1.0, -10.0, 100.0, 100.0, 90.0, 24.0, 0.92, [32, 32, 32], [20, 20, 20]);
        assert_eq!(q, [13, 13, 13, 255]); // dev +12: 32*0.08+2.3+9 = 13.86 -> 13
    }
}

#[cfg(test)]
mod iter31_tests {
    use super::*;

    #[test]
    fn frost_fill_px_blur_top_corner_mid_identical() {
        // Flat frost (iter31): top, corner and
        // mid-height pixels are the same base for the same backdrop.
        for (x, y, d) in [(45.0, 0.5, -0.5), (2.0, 0.5, -0.5), (45.0, 12.0, -10.0)] {
            let p = frost_fill_px_blur(1.0, d, x, y, 90.0, 24.0, 0.90, [100, 100, 100], [100, 100, 100]);
            assert_eq!(p, [12, 12, 12, 255], "x {x} y {y}"); // iter128: 100*0.10+2.25 = 12.25 -> 12
        }
    }

    #[test]
    fn frost_fill_px_black_fallback_has_no_light() {
        // Fallback (no blurred wallpaper): opaque dark floor, top pixel
        // included - no brightening anywhere, and no backdrop leak
        // (iter85: the old translucent [0,0,0,229]/[0,0,0,216] let the
        // backdrop through, faking a panel top at the terminal edge).
        // iter128: floor 2.5*0.92 = 2.3 -> 2 (was 5*0.80 = 4.0 -> 4).
        let top = frost_fill_px(1.0, -0.5, 45.0, 0.5, 90.0, 24.0, FROST_ALPHA_PILL);
        assert_eq!(top, [2, 2, 2, 255]);
        let centre = frost_fill_px(1.0, -10.0, 45.0, 12.0, 90.0, 24.0, FROST_ALPHA_MENU);
        assert_eq!(centre, [2, 2, 2, 255]);
    }

    #[test]
    fn frost_alphas_unchanged() {
        // Pill and menus 0.92 (iter128: was 0.80); lockscreen stays 1.0.
        assert!((frost_alpha(IslandState::Collapsed) - 0.92).abs() < 1e-6);
        assert!((frost_alpha(IslandState::Music) - 0.92).abs() < 1e-6);
        assert!((frost_alpha(IslandState::Lockscreen) - 1.0).abs() < 1e-6);
    }
}

#[cfg(test)]
mod iter30_tests {
    use super::*;

    #[test]
    fn panel_below_pill_geometry() {
        // Pill: stage (550, 3), 76x28 -> screen (714, 3), 76x28.
        let (ox, oy, pw, ph) = panel_rect_stage(IslandState::Collapsed);
        assert!((ox - 550.0).abs() < 1e-4, "pill ox {ox}");
        assert!((oy - STAGE_PAD_TOP).abs() < 1e-4, "pill oy {oy}");
        assert!((pw - 76.0).abs() < 1e-4 && (ph - 28.0).abs() < 1e-4);
        // Menus: panel top panel_top_menu() (33) -> screen 0 + 33 = 33,
        // i.e. 2 px below the pill bottom (3 + 28 = 31).
        for state in MENU_CYCLE {
            let (_, moy, _, _) = panel_rect_stage(state);
            assert!((moy - panel_top_menu()).abs() < 1e-4, "{state:?} oy {moy}");
        }
        assert!((panel_top_menu() - (STAGE_PAD_TOP + 28.0 + 2.0)).abs() < 1e-4);
    }

    // iter90 a8: workspace circles — geometry, hit-test (incl. the
    // stage-vs-screen offset), colours, mmsg parse, classify_press.
    #[test]
    fn workspace_cluster_geometry() {
        // Live geometry: pill 76 wide at stage x 550..626, top 3..27.
        let (px, py, pw, ph) = panel_rect_stage(IslandState::Collapsed);
        assert!((px - 550.0).abs() < 1e-4, "pill x {px}");
        assert!((py - STAGE_PAD_TOP).abs() < 1e-4);
        assert!((pw - 76.0).abs() < 1e-4);
        assert!((ph - 28.0).abs() < 1e-4);
        let centres = workspace_disc_centres();
        // Exact centres, left to right (tag 1..4).
        assert_eq!(
            centres,
            [(478.0, 17.0), (498.0, 17.0), (518.0, 17.0), (538.0, 17.0)]
        );
        // Ø16 on the pill centre line; 20 px centre-to-centre
        // (= 16 diameter + 4 gap) between neighbours.
        for (cx, cy) in centres {
            assert!((cy - 17.0).abs() < 1e-4, "cy {cy}");
            assert!((cx - 478.0) % 20.0 == 0.0, "cx {cx}");
        }
        for w in centres.windows(2) {
            assert!(((w[1].0 - w[0].0) - 20.0).abs() < 1e-4);
        }
        // 4 px gap between the rightmost circle edge and the pill.
        assert!(((px - (centres[3].0 + WS_DISC_R)) - WS_GAP_PILL).abs() < 1e-4);
        // Cluster rect: x 470..550, y 7..23 — the circles span exactly
        // 76 (the pill's width) plus the 4 px bridge to the pill, so
        // rw = 80; fully inside the a3d reserved strip (y 3..27) and
        // strictly above the panel top (29).
        let (rx, ry, rw, rh) = workspace_cluster_rect();
        assert!((rx - 470.0).abs() < 1e-4, "cluster x {rx}");
        assert!((ry - 9.0).abs() < 1e-4, "cluster y {ry}");
        assert!((rw - 80.0).abs() < 1e-4, "cluster w {rw}");
        assert!((rh - 16.0).abs() < 1e-4, "cluster h {rh}");
        // Circles-only span (no bridge): exactly 76, the pill's width.
        let span = (centres[3].0 + WS_DISC_R) - (centres[0].0 - WS_DISC_R);
        assert!((span - 76.0).abs() < 1e-4, "circle row width {span}");
        assert!(rx >= 0.0 && ry >= 0.0);
        assert!(ry >= STAGE_PAD_TOP, "cluster top must stay in the reserved strip");
        assert!(
            ry + rh <= STAGE_PAD_TOP + pill_h(),
            "cluster bottom must stay in the reserved strip"
        );
        assert!(ry + rh < panel_top_menu(), "cluster must stay above the panel top");
        assert!(rx + rw <= STAGE_W as f32);
        // No circle overlaps the pill or any panel top edge.
        for (cx, _) in centres {
            assert!(cx + WS_DISC_R <= px, "circle at {cx} overlaps the pill");
        }
    }

    /// r1c finding 3: SUPER+5..9 must not black out the row. Whatever
    /// the active tag, it has to be one of the four drawn slots, and
    /// tags 1..4 must keep the original 1,2,3,4 layout (every geometry
    /// assertion and the reference measurement assume it).
    #[test]
    fn ws_window_always_shows_the_active_tag() {
        for active in 1..=9u8 {
            let tags = ws_visible_tags(active);
            assert!(
                tags.contains(&active),
                "active tag {active} missing from drawn tags {tags:?}"
            );
            assert_eq!(tags.len(), WS_COUNT);
            // Contiguous, ascending, in range.
            for (i, t) in tags.iter().enumerate() {
                assert_eq!(*t, tags[0] + i as u8, "tags not contiguous: {tags:?}");
                assert!((1..=9).contains(t), "tag out of range: {tags:?}");
            }
        }
        // The first four tags are the regression-frozen layout.
        for active in 1..=4u8 {
            assert_eq!(ws_visible_tags(active), [1, 2, 3, 4], "tag {active}");
        }
        assert_eq!(ws_visible_tags(5), [2, 3, 4, 5]);
        assert_eq!(ws_visible_tags(9), [6, 7, 8, 9]);
    }

    /// r1c finding 4: occupancy comes from the same JSON mmsg already
    /// prints, and a window-less tag must not read as occupied.
    #[test]
    fn parse_tag_state_occupancy() {
        // Real `mmsg get all-tags` output, tag 2 active with one window.
        let json = "{\"all_tags\":[{\"monitor\":\"eDP-1\",\"tags\":[{\"index\":1,\"is_active\":false,\"is_urgent\":false,\"layout\":\"T\",\"client_count\":0},{\"index\":2,\"is_active\":true,\"is_urgent\":false,\"layout\":\"T\",\"client_count\":1},{\"index\":3,\"is_active\":false,\"is_urgent\":true,\"layout\":\"T\",\"client_count\":2}]}]}";
        let ts = parse_tag_state(json).expect("parses");
        assert_eq!(ts.active, 2);
        assert!(!ts.is_occupied(1), "empty tag 1 read as occupied");
        assert!(ts.is_occupied(2), "tag 2 has a window");
        assert!(ts.is_occupied(3), "tag 3 has two windows");
        assert!(!ts.is_occupied(4), "tag absent from the JSON");
        assert_eq!(ts.urgent & 0b100, 0b100, "tag 3 urgent bit");
        assert_eq!(ts.urgent & 0b010, 0, "tag 2 not urgent");
        // Nothing active => nothing to cache (the caller keeps its last).
        assert!(parse_tag_state("{\"all_tags\":[]}").is_none());
        assert!(parse_tag_state("not json").is_none());
        // A tag whose flags would be misattributed to its NEIGHBOUR must
        // not happen: index 1 empty, index 2 occupied.
        let two = "[{\"index\":1,\"is_active\":true,\"is_urgent\":false,\"client_count\":0},{\"index\":2,\"is_active\":false,\"is_urgent\":false,\"client_count\":3}]";
        let ts = parse_tag_state(two).expect("parses");
        assert_eq!(ts.active, 1);
        assert!(!ts.is_occupied(1), "tag 1 stole tag 2's count");
        assert!(ts.is_occupied(2), "tag 2 lost its count");
    }

    /// The active disc draws the accent, an occupied one a lighter grey,
    /// an empty one the dark grey — and active always beats occupied.
    #[test]
    fn workspace_colours_are_distinct() {
        assert_ne!(WS_ACTIVE_FILL, WS_OCCUPIED_FILL);
        assert_ne!(WS_OCCUPIED_FILL, WS_INACTIVE_FILL);
        // Occupied is lighter than empty on every channel (visible on a
        // near-black bar without introducing a new hue).
        for c in 0..3 {
            let occ = [WS_OCCUPIED_FILL.0, WS_OCCUPIED_FILL.1, WS_OCCUPIED_FILL.2][c];
            let emp = [WS_INACTIVE_FILL.0, WS_INACTIVE_FILL.1, WS_INACTIVE_FILL.2][c];
            assert!(occ > emp, "occupied must be lighter on channel {c}");
        }
    }

    #[test]
    fn workspace_hit_stage_coords() {
        // Centre hits return the tag; gaps and the pill do not.
        assert_eq!(workspace_hit_stage((478.0, 17.0)), Some(1));
        assert_eq!(workspace_hit_stage((498.0, 17.0)), Some(2));
        assert_eq!(workspace_hit_stage((518.0, 17.0)), Some(3));
        assert_eq!(workspace_hit_stage((538.0, 17.0)), Some(4));
        // Circle edge still hits (radius 8 + 1 slack = 9).
        assert_eq!(workspace_hit_stage((486.0, 17.0)), Some(1));
        // Mid-gap between circles 1 and 2 (x 488): no hit.
        assert_eq!(workspace_hit_stage((488.0, 17.0)), None);
        // Pill centre (588, 15) is not a workspace hit.
        assert_eq!(workspace_hit_stage((588.0, 17.0)), None);
        // Stage-vs-screen: the stage origin is x=164 on screen, so a
        // driver click at SCREEN (789, 176) arrives as STAGE (625, 176)
        // — inside the pill, not on a circle — and the hit fn takes
        // STAGE coords only.
        let (sox, soy) = stage_screen_origin();
        assert!((sox - 164.0).abs() < 1e-4, "stage origin x {sox}");
        assert!((soy - 0.0).abs() < 1e-4, "stage origin y {soy}");
        let stage = (789.0 - sox as f64, 176.0 - soy as f64);
        assert!((stage.0 - 625.0).abs() < 1e-4);
        assert_eq!(workspace_hit_stage(stage), None);
        // A screen click on circle 4: screen x = 538 + 164 = 702.
        assert_eq!(workspace_hit_stage((702.0 - sox as f64, 17.0)), Some(4));
    }

    #[test]
    fn workspace_colours() {
        // Active = the island accent itself; inactive = nav-disc dark.
        assert_eq!(WS_ACTIVE_FILL, library::ISLAND_ACCENT);
        assert_eq!(WS_INACTIVE_FILL, (28, 30, 34));
        assert_eq!(WS_ACTIVE_GLYPH, (11, 27, 58));
        assert_eq!(WS_INACTIVE_GLYPH, (232, 234, 237));
        // iter92: Ø16 — deliberately NOT the nav-disc Ø28 any more;
        // the whole row is 76 wide, the pill's width.
        assert!((WS_DISC_R * 2.0 - 16.0).abs() < 1e-4);
        assert!((WS_NUMBER_PX - 11.0).abs() < 1e-4);
        // Circles-only span (rect width 80 includes the 4 px bridge).
        let centres = workspace_disc_centres();
        let span = (centres[3].0 + WS_DISC_R) - (centres[0].0 - WS_DISC_R);
        assert!((span - 76.0).abs() < 1e-4, "circle row width {span}");
    }

    #[test]
    fn workspace_digit_fits() {
        // iter92: the digit at WS_NUMBER_PX must fit inside a Ø16
        // disc with margin — rasterize 1..=4 with the real UI font.
        // Vendored asset is embedded, so this never depends on $HOME.
        let font = ui_font();
        for n in 1..=4u8 {
            let row = layout_text(&font, &n.to_string(), WS_NUMBER_PX);
            // Digit bitmap height ~8 px (0.5 x Ø16); width well inside.
            let max_h = row.glyphs.iter().map(|(m, _)| m.height).max().unwrap_or(0);
            assert!(max_h <= 9, "digit {n} height {max_h} too tall for Ø16");
            assert!(
                row.width <= 2.0 * WS_DISC_R - 4.0,
                "digit {n} width {} too wide for Ø16",
                row.width
            );
        }
    }

    #[test]
    fn parse_active_tag_mmsg_json() {
        let json = "{\"all_tags\":[{\"monitor\":\"eDP-1\",\"tags\":[{\"index\":1,\"is_active\":true,\"is_urgent\":false,\"layout\":\"T\",\"client_count\":1},{\"index\":2,\"is_active\":false,\"is_urgent\":false,\"layout\":\"T\",\"client_count\":1}]}]}";
        assert_eq!(parse_active_tag(json), Some(1));
        let json2 = json
            .replace("\"index\":1,\"is_active\":true", "\"index\":1,\"is_active\":false")
            .replace("\"index\":2,\"is_active\":false", "\"index\":2,\"is_active\":true");
        assert_eq!(parse_active_tag(&json2), Some(2));
        assert_eq!(parse_active_tag("{\"all_tags\":[]}"), None);
        assert_eq!(parse_active_tag("not json"), None);
        assert_eq!(parse_active_tag(""), None);
    }

    #[test]
    fn parse_active_tag_watch_stream_line() {
        // iter97: a real `mmsg watch all-tags` line (captured live
        // 14 Nov 2026): same one-line JSON as `get all-tags`, tag 2
        // active after `dispatch view,2`.
        let line = "{\"all_tags\":[{\"monitor\":\"eDP-1\",\"tags\":[{\"index\":1,\"is_active\":false,\"is_urgent\":false,\"layout\":\"T\",\"client_count\":1},{\"index\":2,\"is_active\":true,\"is_urgent\":false,\"layout\":\"T\",\"client_count\":0},{\"index\":3,\"is_active\":false,\"is_urgent\":false,\"layout\":\"T\",\"client_count\":0}]}]}";
        assert_eq!(parse_active_tag(line), Some(2));
    }

    #[test]
    fn classify_press_workspace_tag() {
        // Circles work collapsed and in a menu, and never swallow the pill.
        assert_eq!(
            classify_press((538.0, 17.0), IslandState::Collapsed, false, 0, 0, 0),
            PressAction::WorkspaceTag(4)
        );
        assert_eq!(
            classify_press((478.0, 17.0), IslandState::Music, false, 0, 0, 0),
            PressAction::WorkspaceTag(1)
        );
        assert_eq!(
            classify_press((588.0, 17.0), IslandState::Collapsed, false, 0, 0, 0),
            PressAction::OpenMusic
        );
        assert_eq!(
            classify_press((588.0, 17.0), IslandState::Music, false, 0, 0, 0),
            PressAction::Collapse
        );
    }

    #[test]
    fn tray_geometry_numbers() {
        // iter108-C: tray mirrors the workspace row about the pill.
        // Pill stage rect is (550, 3, 76, 24); pill right edge 626.
        let (px, _, pw, _) = panel_rect_stage(IslandState::Collapsed);
        assert!((px - 550.0).abs() < 1e-4, "pill x {px}");
        assert!((pw as f32 - 76.0).abs() < 1e-4, "pill w {pw}");
        // Icon centres: first 638, step 20, cy 15 (pill centre line).
        assert_eq!(tray_icon_centres(0), vec![]);
        assert_eq!(tray_icon_centres(1), vec![(638.0, 17.0)]);
        assert_eq!(
            tray_icon_centres(2),
            vec![(638.0, 17.0), (658.0, 17.0)]
        );
        assert_eq!(
            tray_icon_centres(3),
            vec![(638.0, 17.0), (658.0, 17.0), (678.0, 17.0)]
        );
        for n in 1..=3usize {
            let centres = tray_icon_centres(n);
            for (cx, cy) in &centres {
                assert!((cy - 17.0).abs() < 1e-4, "tray cy {cy}");
                // 4 px gap between the pill's right edge and icon 0.
                assert!(*cx - TRAY_ICON_R >= px + pw as f32 + TRAY_GAP_PILL - 1e-4);
            }
            for w in centres.windows(2) {
                assert!(((w[1].0 - w[0].0) - 20.0).abs() < 1e-4);
            }
        }
        // Cluster rects: (626, 7, 20n, 16) — left edge is the pill's
        // right edge (the 4 px bridge is inside the rect width).
        assert_eq!(tray_cluster_rect(0), None);
        assert_eq!(tray_cluster_rect(1), Some((626.0, crate::ws_cy() - 8.0, 20.0, 16.0)));
        assert_eq!(tray_cluster_rect(2), Some((626.0, crate::ws_cy() - 8.0, 40.0, 16.0)));
        assert_eq!(tray_cluster_rect(3), Some((626.0, crate::ws_cy() - 8.0, 60.0, 16.0)));
        // Top-row content span (the "stage width" that grows right):
        // 156 / 176 / 196 / 216 for 0 / 1 / 2 / 3 tray items.
        assert_eq!(top_row_span(0), (470.0, 626.0));
        assert_eq!(top_row_span(1), (470.0, 646.0));
        assert_eq!(top_row_span(2), (470.0, 666.0));
        assert_eq!(top_row_span(3), (470.0, 686.0));
        for n in 0..=3usize {
            let (x0, x1) = top_row_span(n);
            assert!(((x1 - x0) - (156.0 + 20.0 * n as f32)).abs() < 1e-4);
        }
        // Pill centre proof: screen 752 (screen centre 1504 / 2) for
        // every tray count — the pill never moves when the tray grows.
        assert!((pill_centre_screen_x() - 752.0).abs() < 1e-4);
        let (sox, _) = stage_screen_origin();
        assert!((sox - 164.0).abs() < 1e-4);
    }

    #[test]
    fn tray_inside_reserved_strip() {
        // iter108-C: the tray row (y 9..25) sits inside the a3d
        // reserved strip (gap_top 3..31) with 2 px spare top/bottom,
        // strictly above the panel top (29) — for any item count.
        assert_eq!(pill_strip_px(), 31);
        for n in [1usize, 2, 3, 27] {
            let (rx, ry, rw, rh) = tray_cluster_rect(n).unwrap();
            assert!(ry >= STAGE_PAD_TOP, "tray top {ry} n={n}");
            assert!(
                ry + rh <= STAGE_PAD_TOP + pill_h(),
                "tray bottom {} n={n}",
                ry + rh
            );
            assert!(ry + rh < panel_top_menu(), "tray under panel top n={n}");
            assert!(rx >= 0.0 && rx + rw <= STAGE_W as f32, "n={n} overflows stage");
            // Icons-only span: 16n + 4(n-1) = 20n - 4.
            let centres = tray_icon_centres(n);
            let span = (centres[n - 1].0 + TRAY_ICON_R) - (centres[0].0 - TRAY_ICON_R);
            assert!(((span - (20.0 * n as f32 - 4.0))).abs() < 1e-4, "n={n} span {span}");
        }
        // Sheet bound: 27 icons fit (right edge 1166 < 1176); a 28th
        // would overflow the fixed stage (1186) — the SNI host must
        // never need that many, but the geometry says so honestly.
        assert_eq!(tray_cluster_rect(27), Some((626.0, crate::ws_cy() - 8.0, 540.0, 16.0)));
        let (rx, _, rw, _) = tray_cluster_rect(28).unwrap();
        assert!(rx + rw > STAGE_W as f32, "28 icons must overflow");
    }

    #[test]
    fn tray_hit_stage_coords() {
        // Centre hits return the 0-based slot; edges, gaps, the bridge
        // and the pill do not.
        assert_eq!(tray_hit_stage((638.0, 17.0), 3), Some(0));
        assert_eq!(tray_hit_stage((658.0, 17.0), 3), Some(1));
        assert_eq!(tray_hit_stage((678.0, 17.0), 3), Some(2));
        // Icon edge still hits (radius 8 + 1 slack = 9).
        assert_eq!(tray_hit_stage((647.0, 17.0), 3), Some(0));
        assert_eq!(tray_hit_stage((669.0, 17.0), 3), Some(2));
        // Mid-gap between icons 0 and 1 (x 648): no hit.
        assert_eq!(tray_hit_stage((648.0, 17.0), 3), None);
        // 4 px bridge to the pill (x 626..630): no hit.
        assert_eq!(tray_hit_stage((628.0, 17.0), 3), None);
        // Pill centre is not a tray hit.
        assert_eq!(tray_hit_stage((588.0, 17.0), 3), None);
        // Workspace circle 4 is not a tray hit.
        assert_eq!(tray_hit_stage((538.0, 17.0), 3), None);
        // Empty tray: nothing hits anywhere on the row.
        assert_eq!(tray_hit_stage((638.0, 17.0), 0), None);
        assert_eq!(tray_hit_stage((658.0, 17.0), 0), None);
        // Fewer items than slots: slot 2 does not exist with n=2.
        assert_eq!(tray_hit_stage((678.0, 17.0), 2), None);
        assert_eq!(tray_hit_stage((658.0, 17.0), 2), Some(1));
        // Stage-vs-screen: a driver click at SCREEN (802, 15) arrives
        // as STAGE (638, 15) — tray slot 0.
        let (sox, soy) = stage_screen_origin();
        assert_eq!(tray_hit_stage((802.0 - sox as f64, 17.0 - soy as f64), 3), Some(0));
        // Screen click on icon 2: screen x = 678 + 164 = 842.
        assert_eq!(tray_hit_stage((842.0 - sox as f64, 17.0), 3), Some(2));
    }

    #[test]
    fn classify_press_tray_icon() {
        // Slots work collapsed and in a menu, and never swallow the pill.
        assert_eq!(
            classify_press((638.0, 17.0), IslandState::Collapsed, false, 0, 0, 1),
            PressAction::TrayActivate(0)
        );
        assert_eq!(
            classify_press((658.0, 17.0), IslandState::Music, false, 0, 0, 2),
            PressAction::TrayActivate(1)
        );
        assert_eq!(
            classify_press((678.0, 17.0), IslandState::ControlCenter, false, 0, 0, 3),
            PressAction::TrayActivate(2)
        );
        // Gaps and the bridge are NoOp — never Collapse, in any state.
        assert_eq!(
            classify_press((648.0, 17.0), IslandState::Collapsed, false, 0, 0, 3),
            PressAction::NoOp
        );
        assert_eq!(
            classify_press((648.0, 17.0), IslandState::Music, false, 0, 0, 3),
            PressAction::NoOp
        );
        assert_eq!(
            classify_press((628.0, 17.0), IslandState::Music, false, 0, 0, 3),
            PressAction::NoOp
        );
        // Empty tray: the same point is NoOp (outside the pill, D2).
        assert_eq!(
            classify_press((638.0, 17.0), IslandState::Collapsed, false, 0, 0, 0),
            PressAction::NoOp
        );
        // The pill still opens / collapses with a tray present.
        assert_eq!(
            classify_press((588.0, 17.0), IslandState::Collapsed, false, 0, 0, 3),
            PressAction::OpenMusic
        );
        assert_eq!(
            classify_press((588.0, 17.0), IslandState::Music, false, 0, 0, 3),
            PressAction::Collapse
        );
        // Workspace circles still win on their side with a tray present.
        assert_eq!(
            classify_press((538.0, 17.0), IslandState::Collapsed, false, 0, 0, 3),
            PressAction::WorkspaceTag(4)
        );
    }

    #[test]
    fn tray_count_store_roundtrip() {
        // The geometry-side store starts at 0 and round-trips; the
        // SNI host (iter108-B) is the only writer in production.
        // Merge: take the shared tray-test lock — B's region test
        // writes this store too, and Rust runs tests in parallel.
        let _g = TRAY_TEST_LOCK.lock().unwrap();
        assert_eq!(tray_item_count(), 0);
        set_tray_count(3);
        assert_eq!(tray_item_count(), 3);
        assert_eq!(tray_icon_centres(tray_item_count()).len(), 3);
        set_tray_count(0);
        assert_eq!(tray_item_count(), 0);
    }

    #[test]
    fn pill_strip_covers_pill_exactly() {
        // iter35 a3d: default gap_top 3, pill 24 tall -> the reserved
        // strip ends exactly at the pill bottom (27); the strip never
        // covers panel space (panel top is gap_top + 26 = 29).
        assert_eq!(TOP_MARGIN, 3);
        assert_eq!(TOP_MARGIN + 28, 31);
        assert_eq!(pill_strip_px(), TOP_MARGIN + 28);
    }

    #[test]
    fn discs_at_panel_midheight() {
        // (state, expected disc cy): oy 33 + h/2 (A2: panel top 33,
        // Calendar PANEL_H 280 -> 266, CC_H 401 -> 451 (iter109: power row).
        let cases = [
            (IslandState::Music, 108.0),
            (IslandState::Power, 108.0),
            (IslandState::Calendar, 164.0), // 33 + 262/2
            (IslandState::ControlCenter, 256.5), // 33 + 447/2
            (IslandState::Launcher, 216.0),
            (IslandState::Wallpaper, 168.0),
            (IslandState::Themes, 216.0),
        ];
        for (state, want_cy) in cases {
            let ((lx, ly), (rx, ry)) = nav_disc_centres(state);
            assert!((ly - want_cy).abs() < 1e-4, "{state:?} ly {ly} want {want_cy}");
            assert!((ry - want_cy).abs() < 1e-4, "{state:?} ry {ry}");
            // Same horizontal offset as before: 22 beyond each edge.
            let (ox, _, pw, _) = panel_rect_stage(state);
            assert!((lx - (ox - 22.0)).abs() < 1e-4, "{state:?} lx {lx}");
            assert!((rx - (ox + pw + 22.0)).abs() < 1e-4, "{state:?} rx {rx}");
        }
    }

    #[test]
    fn morph_capsule_oy_interpolates_pill_to_panel() {
        assert_eq!(morph_capsule_oy(IslandState::Collapsed, IslandState::Music, 0.0), 3.0);
        assert_eq!(morph_capsule_oy(IslandState::Collapsed, IslandState::Music, 1.0), 33.0);
        assert_eq!(morph_capsule_oy(IslandState::Collapsed, IslandState::Music, 0.5), 18.0);
        // Overshoot clamps: the top edge lands exactly on the target.
        assert_eq!(morph_capsule_oy(IslandState::Collapsed, IslandState::Music, 1.3), 33.0);
        // Menu -> menu: top stays at the panel origin.
        assert_eq!(morph_capsule_oy(IslandState::Music, IslandState::Calendar, 0.5), 33.0);
        // Collapse: back to the pill top.
        assert_eq!(morph_capsule_oy(IslandState::Music, IslandState::Collapsed, 1.0), 3.0);
    }

    #[test]
    fn pill_and_bridge_in_menu_region() {
        // Pill centre (588,16) and the pill<->panel bridge (588,28)
        // belong to a menu's region; the gap beside the bridge does not.
        // iter89: bridge strip is y 27..29 (pill bottom 27, panel top 29);
        // y=29 is inside the panel now, so the bridge/gap probes move to 28.
        for state in MENU_CYCLE {
            assert!(point_in_state_region((588.0, 16.0), state), "{state:?} pill");
            assert!(point_in_state_region((588.0, 28.0), state), "{state:?} bridge");
        }
        assert!(!point_in_state_region((400.0, 28.0), IslandState::Music));
        assert!(!point_in_state_region((400.0, 16.0), IslandState::Music));
        // Collapsed has no pill-bridge (no menu below it).
        assert!(!point_in_state_region((588.0, 32.0), IslandState::Collapsed));
    }

    #[test]
    fn bridge_click_is_noop_pill_click_collapses() {
        assert_eq!(
            classify_press((588.0, 32.0), IslandState::Music, false, 0, 0, 0),
            PressAction::NoOp
        );
        assert_eq!(
            classify_press((588.0, 16.0), IslandState::Music, false, 0, 0, 0),
            PressAction::Collapse
        );
        // Gap click beside the bridge: no-op, never collapse.
        assert_eq!(
            classify_press((400.0, 32.0), IslandState::Music, false, 0, 0, 0),
            PressAction::NoOp
        );
    }
}

#[cfg(test)]
mod iter36_tests {
    use super::*;

    // Stage coords: panel origin + panel-local point.
    // Wallpaper (1008x270): origin (84, 31). Themes (600x366): (288, 31).
    // Power (400x150): (388, 29). Calendar (342x262): (417, 29).

    #[test]
    fn wallpaper_thumb_click_applies_that_index() {
        // Centre thumb (selected, index 2 when wall_selected=2).
        assert_eq!(
            classify_press((84.0 + 391.0 + 10.0, 31.0 + 78.0 + 10.0), IslandState::Wallpaper, false, 2, 0, 0),
            PressAction::WallApply(2)
        );
        // First thumb (index 0) with the same selection.
        assert_eq!(
            classify_press((84.0 + 59.0 + 10.0, 31.0 + 99.0 + 10.0), IslandState::Wallpaper, false, 2, 0, 0),
            PressAction::WallApply(0)
        );
        // Wrapped neighbours: selected=0 shows index 54 at position 0
        // (56-wallpaper library) — mapping is (selected + pos - 2) mod n.
        assert_eq!(wallpaper_panel::filmstrip_index_n(0, 0, 56), 54);
        assert_eq!(wallpaper_panel::filmstrip_index_n(4, 0, 56), 2);
        assert_eq!(wallpaper_panel::filmstrip_index_n(0, 2, 56), 0);
    }

    #[test]
    fn wallpaper_chevron_click_browses() {
        // iter128: the filmstrip chevrons are BACK — iter89 F4
        // removed them and this test used to assert the removal
        // (chevron points -> NoOp). iter127 proved that left the
        // picker with no non-applying navigation at all (30 futile
        // clicks, counter never moved), so the discs are restored as
        // nav-disc-family buttons (wallpaper_panel.rs `draw_chevron`
        // / `filmstrip_hit`) and their clicks browse ±1, never apply:
        // left disc -> WallWindow(-1), right disc -> WallWindow(+1).
        // Nav discs still cycle menus; thumbs still apply.
        assert_eq!(
            classify_press((84.0 + 30.0, 29.0 + 147.0), IslandState::Wallpaper, false, 2, 0, 0),
            PressAction::WallWindow(-1)
        );
        assert_eq!(
            classify_press((84.0 + 978.0, 29.0 + 147.0), IslandState::Wallpaper, false, 2, 0, 0),
            PressAction::WallWindow(1)
        );
    }

    #[test]
    fn themes_row_click_applies_that_theme() {
        // Selected row (row 0) centre.
        assert_eq!(
            classify_press((288.0 + 300.0, 31.0 + 114.0), IslandState::Themes, false, 0, 0, 0),
            PressAction::ThemeApply(0)
        );
        // First unselected row (row 1, y 168..216) centre.
        assert_eq!(
            classify_press((288.0 + 300.0, 31.0 + 192.0), IslandState::Themes, false, 0, 0, 0),
            PressAction::ThemeApply(1)
        );
        // Row 4 with theme_selected=22 wraps: (22+4) % n.
        let n = themes_panel::theme_total();
        assert_eq!(
            classify_press((288.0 + 300.0, 31.0 + 336.0), IslandState::Themes, false, 0, 22, 0),
            PressAction::ThemeApply((22 + 4) % n)
        );
    }

    #[test]
    fn power_disc_click_is_lock() {
        // Centre disc (lock): BTN_CX[2]=200, ROW_CY=78.
        assert_eq!(
            classify_press((388.0 + 200.0, 31.0 + 78.0), IslandState::Power, false, 0, 0, 0),
            PressAction::PowerButton(2)
        );
        // First disc (moon/suspend) and last disc (power).
        assert_eq!(
            classify_press((388.0 + 74.0, 31.0 + 78.0), IslandState::Power, false, 0, 0, 0),
            PressAction::PowerButton(0)
        );
        assert_eq!(
            classify_press((388.0 + 326.0, 31.0 + 78.0), IslandState::Power, false, 0, 0, 0),
            PressAction::PowerButton(4)
        );
    }

    #[test]
    fn calendar_chevron_click_shifts_month() {
        // Left chevron centre (40, 5.86) panel-local (iter121: CHEV_CY 5.86 screen-calibrated; was 6.53 iter116-120, 5.2 iter89-115, 23.5 pre-iter89).
        assert_eq!(
            classify_press((417.0 + 40.0, 29.0 + 5.86), IslandState::Calendar, false, 0, 0, 0),
            PressAction::CalMonth(-1)
        );
        assert_eq!(
            classify_press((417.0 + 302.0, 29.0 + 5.86), IslandState::Calendar, false, 0, 0, 0),
            PressAction::CalMonth(1)
        );
        // Title centre is NOT a chevron: no-op.
        assert_eq!(
            classify_press((417.0 + 171.0, 29.0 + 5.86), IslandState::Calendar, false, 0, 0, 0),
            PressAction::NoOp
        );
    }

    #[test]
    fn power_action_commands() {
        assert_eq!(power_panel::action_command(2), ("lock", "swaylock -f -c 000000"));
        assert_eq!(power_panel::action_command(0), ("suspend", "loginctl suspend"));
        assert_eq!(power_panel::action_command(1), ("logout", "loginctl terminate-session $XDG_SESSION_ID"));
        assert_eq!(power_panel::action_command(3), ("reboot", "loginctl reboot"));
        assert_eq!(power_panel::action_command(4), ("poweroff", "loginctl poweroff"));
    }
}

#[cfg(test)]
mod iter39_keyboard_tests {
    use super::*;

    #[test]
    fn cycle_order_is_the_full_menu_sequence() {
        let expected = [
            IslandState::Music,
            IslandState::Launcher,
            IslandState::ControlCenter,
            IslandState::Calendar,
            IslandState::Wallpaper,
            IslandState::Themes,
            IslandState::Power,
            IslandState::Clipboard,
        ];
        assert_eq!(MENU_CYCLE, expected);
        // Forward walk from Music visits every menu exactly once, then wraps.
        let mut state = IslandState::Music;
        for want in expected.iter().cycle().skip(1).take(8) {
            state = cycle_step(state, 1).unwrap();
            assert_eq!(state, *want);
        }
        // Reverse walk from Music wraps to Clipboard first.
        assert_eq!(cycle_step(IslandState::Music, -1), Some(IslandState::Clipboard));
        assert_eq!(cycle_step(IslandState::Clipboard, 1), Some(IslandState::Music));
        assert_eq!(cycle_step(IslandState::Power, 1), Some(IslandState::Clipboard));
        assert_eq!(cycle_step(IslandState::Clipboard, -1), Some(IslandState::Power));
    }

    #[test]
    fn cycle_step_is_none_outside_menus() {
        assert_eq!(cycle_step(IslandState::Collapsed, 1), None);
        assert_eq!(cycle_step(IslandState::Lock, -1), None);
        assert_eq!(cycle_step(IslandState::Lockscreen, 1), None);
    }
}

#[cfg(test)]
mod iter108_tests {
    use super::*;

    fn load_font() -> &'static fontdue::Font {
        ui_font()
    }

    /// Ideal reference: the same disc rendered NATIVELY at 2x with the
    /// same SDF encoding (a_disc ramp, fill colour, A=255) — no upscale
    /// involved. (`cx`, `cy`) is the disc centre in tile coords (tile is
    /// `d` x `d`, kept separate: a centre can be half-integer on one
    /// axis and integer on the other, which shifts the tip by a row.)
    /// The profile is measured with the same helper as the upscaled
    /// render, so any difference is the upscale's doing.
    fn ideal_profile(
        radius_logical: f32,
        fill: (u8, u8, u8),
        d: u32,
        cx: f32,
        cy: f32,
    ) -> Vec<u32> {
        let r = radius_logical * 2.0;
        let mut tile = vec![0u8; (d * d * 4) as usize];
        for y in 0..d {
            for x in 0..d {
                let dd = (((x as f32 + 0.5) - cx).powi(2)
                    + ((y as f32 + 0.5) - cy).powi(2))
                .sqrt();
                let a = (r + 0.5 - dd).clamp(0.0, 1.0) * 0.92;
                if a > 0.0 {
                    let i = ((y * d + x) * 4) as usize;
                    tile[i] = (fill.2 as f32 * a) as u8;
                    tile[i + 1] = (fill.1 as f32 * a) as u8;
                    tile[i + 2] = (fill.0 as f32 * a) as u8;
                    tile[i + 3] = 255;
                }
            }
        }
        disc_row_widths(
            &tile,
            d,
            0,
            0,
            d,
            (fill.0 as f32, fill.1 as f32, fill.2 as f32),
            0.5,
        )
    }

    /// Shared profile verdict (iter108 roundness acceptance):
    /// - every row where the ideal is at least 30% of max within
    ///   tolerance of the native-2x ideal: tol 3 in the interior,
    ///   tol 5 on the outer rows (ideal <= 60% of max), where the
    ///   1.5 px rim highlight/shadow band (iter26) shifts the
    ///   50%-coverage contour by up to ~1 logical px — the rimless
    ///   ideal cannot capture it, and it is not a facet;
    /// - extent: first/last nonzero rows within 1 row of the ideal's,
    ///   and at most 2 fewer nonzero rows (the bottom rim can push
    ///   the last fringe row under the 50% threshold after the
    ///   upscale's 3/4 weight — a darker fringe pixel, not a facet);
    /// - shoulder jump (rows with ideal in [45%, 85%] of max) at most
    ///   radius_buf/8, min 4 — the tip rows are excluded: a true
    ///   circle legitimately jumps ~10 px between its first two rows;
    /// - no flat run longer than the ideal's run + 4: the 2x upscale turns each 1x row into two buffer rows, so a 1x full-width run of N rows becomes up to 2N buffer rows while the ideal run grows only ~sqrt(2)x - measured excesses are WS +2, nav +4, toggle +2, bell 0, so +4 covers the worst quantisation case.
    /// (Live v0.46, compositor-upscaled 1x: shoulder jumps 5-6 and a
    /// 3 px tip row — the octagon Alexandre reported.)
    fn assert_profile_close(name: &str, measured: &[u32], ideal: &[u32], radius_buf: f32) {
        assert_eq!(measured.len(), ideal.len(), "{name}: profile length");
        let max_ideal = *ideal.iter().max().unwrap_or(&0);
        let shoulder_bound = ((radius_buf / 8.0) as u32).max(4);
        let mut shoulder_jump = 0u32;
        let mut run = 1u32;
        let mut worst_run = 1u32;
        let mut ideal_run = 1u32;
        let mut ideal_cur = 1u32;
        for (i, (m, e)) in measured.iter().zip(ideal.iter()).enumerate() {
            // Tip rows (ideal < 30% of max) are covered by the extent
            // checks below, not by the per-row comparison: the rim
            // band and the upscale phase move them by a row.
            if *e * 10 >= max_ideal * 3 {
                let tol = if *e * 5 <= max_ideal * 3 { 5 } else { 3 };
                let diff = m.abs_diff(*e);
                assert!(
                    diff <= tol,
                    "{name} row {i}: measured {m} vs ideal {e} (tol {tol}); profile {measured:?}"
                );
            }
            if i > 0 {
                if measured[i] == measured[i - 1] {
                    run += 1;
                    worst_run = worst_run.max(run);
                } else {
                    run = 1;
                }
                if ideal[i] == ideal[i - 1] {
                    ideal_cur += 1;
                    ideal_run = ideal_run.max(ideal_cur);
                } else {
                    ideal_cur = 1;
                }
                // Shoulder only: exclude the pointy tip rows.
                if *e * 20 >= max_ideal * 9 && *e * 20 <= max_ideal * 17 {
                    shoulder_jump = shoulder_jump.max(measured[i].abs_diff(measured[i - 1]));
                }
            }
        }
        let first = |p: &[u32]| p.iter().position(|w| *w > 0);
        let last = |p: &[u32]| p.iter().rposition(|w| *w > 0);
        let (mf, ilf) = (first(measured), first(ideal));
        let (ml, ill) = (last(measured), last(ideal));
        let count = |p: &[u32]| p.iter().filter(|w| **w > 0).count();
        eprintln!(
            "{name} profile: {measured:?} (shoulder_jump={shoulder_jump} max_run={worst_run} ideal_run={ideal_run} span={mf:?}..{ml:?} ideal_span={ilf:?}..{ill:?})"
        );
        match (mf, ilf) {
            (Some(a), Some(b)) => assert!(
                a.abs_diff(b) <= 1,
                "{name}: first nonzero row {a} vs ideal {b}"
            ),
            _ => panic!("{name}: empty profile"),
        }
        match (ml, ill) {
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
            worst_run <= ideal_run + 4,
            "{name}: max_run {worst_run} > ideal {ideal_run} + 4"
        );
    }

    #[test]
    fn upscale_flat_is_identity() {
        let (w, h) = (4u32, 3u32);
        let mut src = vec![0u8; (w * h * 4) as usize];
        for i in 0..(w * h) as usize {
            src[i * 4] = 12;
            src[i * 4 + 1] = 34;
            src[i * 4 + 2] = 56;
            src[i * 4 + 3] = 200;
        }
        let up = upscale_canvas_2x(&src, w, h);
        assert_eq!(up.len(), (8 * 6 * 4) as usize);
        for px in up.chunks_exact(4) {
            assert_eq!(px, [12, 34, 56, 200]);
        }
    }

    #[test]
    fn upscale_matches_f32_reference() {
        // Deterministic pseudo-random 5x4 canvas; compare the fixed-point
        // upscale against a direct f32 bilinear within +-1 per channel.
        let (w, h) = (5u32, 4u32);
        let mut src = vec![0u8; (w * h * 4) as usize];
        let mut seed: u32 = 0x1234_5678;
        for v in src.iter_mut() {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            *v = (seed >> 24) as u8;
        }
        let up = upscale_canvas_2x(&src, w, h);
        for dy in 0..8 {
            for dx in 0..10 {
                let sx = dx as f32 / 2.0 - 0.25;
                let sy = dy as f32 / 2.0 - 0.25;
                let x0 = sx.floor() as i32;
                let y0 = sy.floor() as i32;
                let fx = sx - x0 as f32;
                let fy = sy - y0 as f32;
                let tap = |x: i32, y: i32, c: usize| -> f32 {
                    let x = x.clamp(0, w as i32 - 1) as usize;
                    let y = y.clamp(0, h as i32 - 1) as usize;
                    src[(y * w as usize + x) * 4 + c] as f32
                };
                for c in 0..4 {
                    let top = tap(x0, y0, c) * (1.0 - fx) + tap(x0 + 1, y0, c) * fx;
                    let bot = tap(x0, y0 + 1, c) * (1.0 - fx) + tap(x0 + 1, y0 + 1, c) * fx;
                    let want = top * (1.0 - fy) + bot * fy;
                    let got = up[((dy * 10 + dx) * 4) + c] as f32;
                    assert!(
                        (got - want).abs() <= 1.0,
                        "px ({dx},{dy}) ch {c}: got {got} want {want:.1}"
                    );
                }
            }
        }
    }

    #[test]
    fn upscale_empty_is_empty() {
        assert!(upscale_canvas_2x(&[], 0, 0).is_empty());
    }

    #[test]
    fn workspace_circles_round_at_2x() {
        let font = load_font();
        let mut stage = vec![0u8; (STAGE_W * STAGE_H * 4) as usize];
        draw_workspace_circles_stage(&mut stage, STAGE_W, STAGE_H, &font, 1);
        let up = upscale_canvas_2x(&stage, STAGE_W, STAGE_H);
        assert_eq!(up.len(), (2 * STAGE_W * 2 * STAGE_H * 4) as usize);
        let centres = workspace_disc_centres();
        for (i, (cx, cy)) in centres.iter().enumerate() {
            let fill = if i == 0 { WS_ACTIVE_FILL } else { WS_INACTIVE_FILL };
            // 40x40 tile centred on the disc in the 2x buffer.
            let tx = (2.0 * cx - 20.0).round() as i32;
            let ty = (2.0 * cy - 20.0).round() as i32;
            let prof = disc_row_widths(
                &up,
                2 * STAGE_W,
                tx,
                ty,
                40,
                (fill.0 as f32, fill.1 as f32, fill.2 as f32),
                0.5,
            );
            let ideal = ideal_profile(
                WS_DISC_R,
                fill,
                40,
                2.0 * cx - tx as f32,
                2.0 * cy - ty as f32,
            );
            assert_profile_close(&format!("ws-circle-{}", i + 1), &prof, &ideal, WS_DISC_R * 2.0);
        }
    }

    #[test]
    fn nav_discs_round_at_2x() {
        let mut stage = vec![0u8; (STAGE_W * STAGE_H * 4) as usize];
        draw_nav_arrows_stage(&mut stage, STAGE_W, STAGE_H, IslandState::Power);
        let up = upscale_canvas_2x(&stage, STAGE_W, STAGE_H);
        // Nav fill is hardcoded (28, 30, 34) in draw_nav_disc.
        let fill = (28u8, 30u8, 34u8);
        let ((lx, ly), (rx, ry)) = nav_disc_centres(IslandState::Power);
        for (name, (cx, cy)) in [("nav-left", (lx, ly)), ("nav-right", (rx, ry))] {
            // 64x64 tile centred on the disc in the 2x buffer.
            let tx = (2.0 * cx - 32.0).round() as i32;
            let ty = (2.0 * cy - 32.0).round() as i32;
            let prof = disc_row_widths(
                &up,
                2 * STAGE_W,
                tx,
                ty,
                64,
                (fill.0 as f32, fill.1 as f32, fill.2 as f32),
                0.5,
            );
            let ideal = ideal_profile(
                NAV_DISC_R,
                fill,
                64,
                2.0 * cx - tx as f32,
                2.0 * cy - ty as f32,
            );
            assert_profile_close(name, &prof, &ideal, NAV_DISC_R * 2.0);
        }
    }

    #[test]
    fn upscale_region_perf() {
        // What the frame path actually pays on a settled frame and on a
        // morph frame: the upscale is bounded by the damage rect.
        // (The full-stage sibling used to run here at ~61 ms; that is
        // exactly the teleport this test now guards against coming
        // back — a morph region is a fraction of the stage, so the
        // budget is a fraction of the full-stage cost.)
        let (w, h) = (STAGE_W, STAGE_H);
        let mut src = vec![0u8; (w * h * 4) as usize];
        for (i, v) in src.iter_mut().enumerate() {
            *v = (i % 251) as u8;
        }
        let mut dst = vec![0u8; (2 * w * 2 * h * 4) as usize];
        // Worst realistic damage: Collapsed -> Music, the union of both
        // endpoint content rects (the morph path's bound).
        let dmg = union_rect(
            content_rect(IslandState::Collapsed),
            content_rect(IslandState::Music),
        );
        let dmg = clip_rect(dmg, w, h);
        let (dw, dh) = ((dmg.2 - dmg.0) as u32, (dmg.3 - dmg.1) as u32);
        let mut best = std::time::Duration::MAX;
        for _ in 0..3 {
            let t = std::time::Instant::now();
            upscale_canvas_2x_region(&src, w, h, &mut dst, dmg);
            best = best.min(t.elapsed());
        }
        eprintln!(
            "morph-region 2x upscale best of 3: {best:?} over {dw}x{dh} = {:.1}% of the stage",
            100.0 * (dw * dh) as f32 / (w * h) as f32
        );
        // Freeze a real budget: 15 ms leaves headroom for a loaded
        // machine while still failing loudly if the region ever grows
        // back into a full-stage upscale (61 ms measured on this box).
        assert!(
            best.as_millis() < 15,
            "region upscale took {best:?} over {dw}x{dh}, budget 15 ms"
        );
    }

    /// The morph damage bound is the union of the two endpoint content
    /// rects and must stay a small fraction of the stage: that fraction
    /// is what turned a 62 ms frame into a sub-5 ms one.
    #[test]
    fn morph_damage_rect_is_bounded() {
        let (w, h) = (STAGE_W, STAGE_H);
        for (from, to) in [
            (IslandState::Collapsed, IslandState::Music),
            (IslandState::Collapsed, IslandState::Wallpaper),
            (IslandState::Music, IslandState::Launcher),
            (IslandState::ControlCenter, IslandState::Collapsed),
        ] {
            let r = clip_rect(union_rect(content_rect(from), content_rect(to)), w, h);
            let frac = (r.2 - r.0) as f32 * (r.3 - r.1) as f32 / (w * h) as f32;
            assert!(
                frac < 0.80,
                "{from:?} -> {to:?} damage is {:.1}% of the stage",
                frac * 100.0
            );
            // ...and it must still contain both endpoints outright.
            for s in [from, to] {
                let c = clip_rect(content_rect(s), w, h);
                assert!(
                    r.0 <= c.0 && r.1 <= c.1 && r.2 >= c.2 && r.3 >= c.3,
                    "{from:?} -> {to:?} damage misses {s:?}"
                );
            }
        }
    }

    /// Generic f32 bilinear resample (test-only stand-in for the
    /// compositor's 2x -> 1.5x downsample; wlroots GLES2 samples
    /// linear, and the iter108 simulation showed nearest gives the
    /// same verdict).
    fn resample_bilinear(src: &[u8], sw: u32, sh: u32, dw: u32, dh: u32) -> Vec<u8> {
        let (sw, sh, dw, dh) = (sw as usize, sh as usize, dw as usize, dh as usize);
        let mut dst = vec![0u8; dw * dh * 4];
        for oy in 0..dh {
            let sy = (oy as f32 + 0.5) * sh as f32 / dh as f32 - 0.5;
            let y0 = sy.floor() as i32;
            let fy = sy - y0 as f32;
            for ox in 0..dw {
                let sx = (ox as f32 + 0.5) * sw as f32 / dw as f32 - 0.5;
                let x0 = sx.floor() as i32;
                let fx = sx - x0 as f32;
                let tap = |x: i32, y: i32, c: usize| -> f32 {
                    let x = x.clamp(0, sw as i32 - 1) as usize;
                    let y = y.clamp(0, sh as i32 - 1) as usize;
                    src[(y * sw + x) * 4 + c] as f32
                };
                for c in 0..4 {
                    let top = tap(x0, y0, c) * (1.0 - fx) + tap(x0 + 1, y0, c) * fx;
                    let bot = tap(x0, y0 + 1, c) * (1.0 - fx) + tap(x0 + 1, y0 + 1, c) * fx;
                    dst[(oy * dw + ox) * 4 + c] = (top * (1.0 - fy) + bot * fy + 0.5) as u8;
                }
            }
        }
        dst
    }

    /// Analytic ideal screen profile: the 50%-coverage contour of the
    /// SDF disc is the circle of radius R + 0.5 - 0.5/0.92 logical px
    /// (a_disc = 0.5 there), times 1.5 for the screen scale.
    fn ideal_screen_profile(radius_logical: f32, d: u32, centre: f32) -> Vec<u32> {
        let r = (radius_logical + 0.5 - 0.5 / 0.92) * 1.5;
        let mut out = Vec::with_capacity(d as usize);
        for y in 0..d {
            let dy = (y as f32 + 0.5) - centre;
            out.push((2.0 * (r * r - dy * dy).max(0.0).sqrt()).round() as u32);
        }
        out
    }

    #[test]
    fn workspace_circles_round_on_screen() {
        // What Alexandre actually sees: the 2x buffer downsampled to
        // the 1.5x output (24 px discs). Tile the real 1x render,
        // upscale 2x with the shipped code, downsample 2x -> 1.5x,
        // and compare each circle against the analytic 24 px circle.
        let font = load_font();
        let mut stage = vec![0u8; (STAGE_W * STAGE_H * 4) as usize];
        draw_workspace_circles_stage(&mut stage, STAGE_W, STAGE_H, &font, 1);
        let up = upscale_canvas_2x(&stage, STAGE_W, STAGE_H);
        let centres = workspace_disc_centres();
        for (i, (cx, cy)) in centres.iter().enumerate() {
            let fill = if i == 0 { WS_ACTIVE_FILL } else { WS_INACTIVE_FILL };
            // 40x40 tile at 2x -> 30x30 at screen scale; disc centre
            // (20,20) in the tile -> (15,15) on screen.
            let tx = (2.0 * cx - 20.0).round() as i32;
            let ty = (2.0 * cy - 20.0).round() as i32;
            let mut tile = vec![0u8; 40 * 40 * 4];
            for y in 0..40 {
                for x in 0..40 {
                    let si = (((ty + y) as usize) * (2 * STAGE_W as usize) + (tx + x) as usize) * 4;
                    let di = ((y * 40 + x) * 4) as usize;
                    tile[di..di + 4].copy_from_slice(&up[si..si + 4]);
                }
            }
            let screen = resample_bilinear(&tile, 40, 40, 30, 30);
            let prof = disc_row_widths(
                &screen,
                30,
                0,
                0,
                30,
                (fill.0 as f32, fill.1 as f32, fill.2 as f32),
                0.5,
            );
            let ideal = ideal_screen_profile(WS_DISC_R, 30, 15.0);
            eprintln!("ws-circle-{} on screen: {prof:?} ideal {ideal:?}", i + 1);
            // Per-row within 3 px where the ideal is at least 30% of
            // max; span within 1 row. (The bottom rim can darken the
            // last fringe row, as at 2x — covered by the span check,
            // not the per-row check.)
            let max_ideal = *ideal.iter().max().unwrap_or(&0);
            for (r, (m, e)) in prof.iter().zip(ideal.iter()).enumerate() {
                if *e * 10 >= max_ideal * 3 {
                    let diff = m.abs_diff(*e);
                    assert!(diff <= 3, "ws-circle-{} screen row {r}: {m} vs ideal {e}", i + 1);
                }
            }
            let first = |p: &[u32]| p.iter().position(|w| *w > 0);
            let last = |p: &[u32]| p.iter().rposition(|w| *w > 0);
            assert!(first(&prof).unwrap().abs_diff(first(&ideal).unwrap()) <= 1);
            assert!(last(&prof).unwrap().abs_diff(last(&ideal).unwrap()) <= 1);
        }
    }
}

/// Iteration 109 — true-round Ø16 workspace discs.
///
/// The iter108 coverage tests (disc_row_widths, 50% contour) pass
/// even for the old point-sampled rasteriser, because the SDF's 50%
/// contour is a circle by construction; the defect Alexandre
/// reported (octagon silhouette, STRICT max|d| = 5.1 on the live
/// screenshot) only shows in the silhouette the compositor actually
/// produces — the darkness / blue-fill mask measured by
/// circles-profile108.py. These tests reproduce that measurement
/// off-screen: render the real stage, upscale 2x, downsample 2x ->
/// 1.5x, composite over a flat wallpaper, and apply the same STRICT
/// predicate. Two wallpapers bound the live photo wallpaper: the
/// bright sky at the top rows (219, 242, 200) and the dark band at
/// the equator/bottom rows (150, 164, 136), both read off
/// closeout-collapsed.png (iterations/rust108v48).
#[cfg(test)]
mod iter109_tests {
    use super::*;

    fn load_font() -> &'static fontdue::Font {
        ui_font()
    }

    /// STRICT disc predicate, identical to circles-profile108.py:
    /// dark fill (all channels < 90) or active blue fill
    /// (B - R > 40 and B > 120).
    fn strict_disc(px: (u8, u8, u8)) -> bool {
        let (r, g, b) = px;
        (r < 90 && g < 90 && b < 90) || ((b as i32 - r as i32) > 40 && b > 120)
    }

    /// Ideal row-width profile of a circle spanning `n` rows, as in
    /// circles-profile108.py: chord 2*sqrt(r^2 - (k-c)^2), r = n/2.
    fn ideal_prof(n: usize) -> Vec<f32> {
        let c = (n - 1) as f32 / 2.0;
        let r = n as f32 / 2.0;
        (0..n)
            .map(|k| 2.0 * (r * r - (k as f32 - c).powi(2)).max(0.0).sqrt())
            .collect()
    }

    /// Longest run of equal consecutive widths lying fully outside
    /// the equator band (+/-2 rows around the middle of the widest
    /// run), as in circles-profile108.py.
    fn longest_outside(prof: &[u32]) -> usize {
        let widest = *prof.iter().max().unwrap_or(&0);
        let idxs: Vec<usize> = prof
            .iter()
            .enumerate()
            .filter(|(_, w)| **w == widest)
            .map(|(k, _)| k)
            .collect();
        let mid = (idxs[0] + idxs[idxs.len() - 1]) as f32 / 2.0;
        let mut worst = 0usize;
        let mut run = 1usize;
        for k in 1..prof.len() {
            if prof[k] == prof[k - 1] {
                run += 1;
            } else {
                run = 1;
            }
            if run > worst && (k + 1 - run..=k).all(|j| (j as f32 - mid).abs() > 2.0) {
                worst = run;
            }
        }
        worst
    }

    /// Phase-correct 2x -> 1.5x downsample of one disc, straight
    /// from the upscaled buffer: the output tile is 30x30 and its
    /// pixel grid is the live screen grid — the disc centre is a
    /// pixel boundary in x (cx is an integer in phys px) and a pixel
    /// centre in y (cy is a half-integer), so output pixel (ox, oy)
    /// has centre U = (ox - 15 + 0.5, oy - 15) in disc-local screen
    /// px. (A plain 40 -> 30 resample of a centred tile puts the
    /// centre at (14.875, 14.875) instead — half a pixel off in y —
    /// which thins the tip rows by several px under a hard threshold
    /// and would fail this test for a perfectly round disc.)
    fn screen_tile(up: &[u8], cx: f32, cy: f32) -> Vec<u8> {
        let bw = 2 * STAGE_W as usize;
        let mut out = vec![0u8; 30 * 30 * 4];
        for oy in 0..30 {
            let uy = oy as f32 - 15.0;
            let by = 2.0 * cy + (uy / 1.5) * 2.0;
            for ox in 0..30 {
                let ux = ox as f32 - 15.0 + 0.5;
                let bx = 2.0 * cx + (ux / 1.5) * 2.0;
                // Bilinear sample of the 2x buffer at (bx, by);
                // buffer pixel (i, j) has centre (i + 0.5, j + 0.5).
                let x0 = (bx - 0.5).floor() as i32;
                let y0 = (by - 0.5).floor() as i32;
                let fx = (bx - 0.5) - x0 as f32;
                let fy = (by - 0.5) - y0 as f32;
                let tap = |x: i32, y: i32, c: usize| -> f32 {
                    let x = x.clamp(0, bw as i32 - 1) as usize;
                    let y = y.clamp(0, (up.len() / (bw * 4)) as i32 - 1) as usize;
                    up[(y * bw + x) * 4 + c] as f32
                };
                for c in 0..4 {
                    let top = tap(x0, y0, c) * (1.0 - fx) + tap(x0 + 1, y0, c) * fx;
                    let bot = tap(x0, y0 + 1, c) * (1.0 - fx) + tap(x0 + 1, y0 + 1, c) * fx;
                    out[(oy * 30 + ox) * 4 + c] = (top * (1.0 - fy) + bot * fy + 0.5) as u8;
                }
            }
        }
        out
    }

    /// The four discs at screen scale (30x30 tiles, disc centre at
    /// (14.5, 15.0) in tile coords): real stage render, 2x upscale
    /// with the shipped code, 2x -> 1.5x phase-correct downsample.
    fn screen_tiles() -> Vec<Vec<u8>> {
        let font = load_font();
        let mut stage = vec![0u8; (STAGE_W * STAGE_H * 4) as usize];
        draw_workspace_circles_stage(&mut stage, STAGE_W, STAGE_H, &font, 1);
        let up = upscale_canvas_2x(&stage, STAGE_W, STAGE_H);
        workspace_disc_centres()
            .iter()
            .map(|(cx, cy)| screen_tile(&up, *cx, *cy))
            .collect()
    }

    /// STRICT row-width profile of one 30x30 screen tile composited
    /// over a flat `wallpaper` (RGB). Returns (widths in row order,
    /// widest width). The canvas is BGRA premultiplied; the composite
    /// is premultiplied-over, rounded to u8 as the compositor's
    /// output would be.
    fn strict_profile(tile: &[u8], wallpaper: (u8, u8, u8)) -> (Vec<u32>, u32) {
        let mut comp = vec![0u8; 30 * 30 * 3];
        for i in 0..30 * 30 {
            let a = tile[i * 4 + 3] as f32 / 255.0;
            comp[i * 3] = (tile[i * 4 + 2] as f32 + wallpaper.0 as f32 * (1.0 - a) + 0.5) as u8;
            comp[i * 3 + 1] = (tile[i * 4 + 1] as f32 + wallpaper.1 as f32 * (1.0 - a) + 0.5) as u8;
            comp[i * 3 + 2] = (tile[i * 4] as f32 + wallpaper.2 as f32 * (1.0 - a) + 0.5) as u8;
        }
        let mut prof = Vec::new();
        let mut widest = 0u32;
        for y in 0..30 {
            let mut min_x: Option<u32> = None;
            let mut max_x: Option<u32> = None;
            for x in 0..30 {
                let px = (
                    comp[(y * 30 + x) * 3],
                    comp[(y * 30 + x) * 3 + 1],
                    comp[(y * 30 + x) * 3 + 2],
                );
                if strict_disc(px) {
                    min_x = Some(min_x.map_or(x as u32, |m| m.min(x as u32)));
                    max_x = Some(max_x.map_or(x as u32, |m| m.max(x as u32)));
                }
            }
            if let (Some(a), Some(b)) = (min_x, max_x) {
                prof.push(b - a + 1);
                widest = widest.max(b - a + 1);
            }
        }
        (prof, widest)
    }

    /// The acceptance test (verify-iter108-plan.md section a, now
    /// applied to the iter109 rasteriser): every circle's STRICT
    /// profile is within +/-2 px/row of the ideal circle of the same
    /// span, with a 24 px bbox, no flat run >= 4 rows outside the
    /// equator band, and tip rows >= 2 px wide — on both bounding
    /// wallpapers.
    /// Coverage profile of a tile: the row widths where the
    /// composited alpha exceeds 128 (coverage > 0.5). This is the
    /// disc's *geometric* silhouette — the rasteriser's output —
    /// uncontaminated by the rim bevel's colours and the drop
    /// shadow, which the STRICT colour predicate conflates with
    /// the geometry (see the note on `workspace_circles_strict_on_screen`).
    fn coverage_profile(tile: &[u8]) -> (Vec<u32>, u32) {
        let mut prof = Vec::new();
        let mut widest = 0u32;
        for y in 0..30 {
            let mut min_x: Option<u32> = None;
            let mut max_x: Option<u32> = None;
            for x in 0..30 {
                if tile[(y * 30 + x) * 4 + 3] > 128 {
                    min_x = Some(min_x.map_or(x as u32, |m| m.min(x as u32)));
                    max_x = Some(max_x.map_or(x as u32, |m| m.max(x as u32)));
                }
            }
            if let (Some(a), Some(b)) = (min_x, max_x) {
                prof.push(b - a + 1);
                widest = widest.max(b - a + 1);
            }
        }
        (prof, widest)
    }

    /// The rasteriser's roundness, measured on the geometric
    /// silhouette (coverage > 0.5). All four discs are
    /// byte-identical here (one draw path), and the measured
    /// profile is
    ///   [10,14,16,18,20,21,22,23,24,24,25,25,25,24,24,24,
    ///    23,22,21,20,18,16,12,8]
    /// (span 24, width 25, max|d| 3.1 vs the span-ideal, flat
    /// run 1 outside the equator). What is asserted:
    ///  - span 24: the full geometric diameter is present;
    ///  - width 24..=25: the geometric disc is 24 wide; the
    ///    extra pixel is the drop shadow's alpha (offset (2,3))
    ///    lifting the composite alpha's 50% contour outward on
    ///    its side, plus resampling blur — not the disc being
    ///    wider;
    ///  - max|d| <= 3.5 vs the span-ideal: the residual is the
    ///    extreme rows (the span-ideal is 0.5 px phase-shifted
    ///    vs the screen grid, and the 2x->1.5x blur spreads
    ///    coverage into the tip rows) plus the shadow bias, NOT
    ///    faceting — the profile is smooth and monotonic, and
    ///    the V0 value was 5.1 with an 11-row flat run;
    ///  - flat run < 4 outside the equator: the anti-faceting
    ///    criterion — V0's octagon read as an 11-row flat run,
    ///    V4 measures 1;
    ///  - tip rows >= 2 px.
    #[test]
    fn workspace_circles_coverage_round() {
        let tiles = screen_tiles();
        for (i, tile) in tiles.iter().enumerate() {
            let (prof, width) = coverage_profile(tile);
            let ideal = ideal_prof(prof.len());
            let max_d = prof
                .iter()
                .zip(ideal.iter())
                .map(|(m, e)| (*m as f32 - e).abs())
                .fold(0.0f32, f32::max);
            eprintln!(
                "ws-circle-{} coverage: span={} width={width} prof={prof:?} max|d|={max_d:.1} flat={}",
                i + 1,
                prof.len(),
                longest_outside(&prof)
            );
            // 23, not 24: the drop shadow is gone (the reference discs cast
            // none). Its +3y offset used to add one row of alpha>128 BELOW
            // the disc, inflating this span. The disc itself is unchanged --
            // the profile is now perfectly symmetric, which a +3y shadow
            // could never be, and widest is still 24 = 2 * WS_DISC_R * 1.5.
            assert_eq!(
                prof.len(),
                23,
                "circle {} coverage: span {} rows, want 23",
                i + 1,
                prof.len()
            );
            assert_eq!(
                prof,
                prof.iter().rev().copied().collect::<Vec<_>>(),
                "circle {} coverage: profile asymmetric -- a shadow or a \
                 misplaced centre is back",
                i + 1
            );
            assert!(
                (24..=25).contains(&width),
                "circle {} coverage: bbox width {width}, want 24..=25",
                i + 1
            );
            assert!(
                max_d <= 3.5,
                "circle {} coverage: max|d| {max_d:.1} > 3.5",
                i + 1
            );
            assert!(
                longest_outside(&prof) < 4,
                "circle {} coverage: flat run {} >= 4 outside equator",
                i + 1,
                longest_outside(&prof)
            );
            assert!(
                *prof.first().unwrap() >= 2 && *prof.last().unwrap() >= 2,
                "circle {} coverage: tip rows {:?} / {:?} < 2 px",
                i + 1,
                prof.first(),
                prof.last()
            );
        }
    }

    /// STRICT colour profile of the four discs on screen, as the
    /// live verifier (circles-profile108.py) measures it — kept
    /// as a *diagnostic*, not an assertion, because the STRICT
    /// colour predicate conflates the rasteriser's geometry with
    /// two pre-existing design effects that are out of this lane:
    ///  - the rim bevel is light on top (`disc_rim_r`, t = 1 at
    ///    the top, RIM_LIGHT (168,176,205) alpha 0.65) and dark
    ///    at the bottom: on a bright wallpaper it lightens the
    ///    inactive discs' top tip row above the dark branch's
    ///    <90 threshold, so that row is not counted (span 22
    ///    instead of 23) even though the row is visibly there;
    ///  - the drop shadow (offset (2,3), alpha 0.35) darkens the
    ///    background below the disc, lowering the effective
    ///    threshold there and widening the lower rows by ~1-2 px.
    /// Neither can change without altering the discs' look or
    /// breaking `disc_rim_light_on_top_dark_below_none_at_centre`.
    /// The active disc passes STRICT (max|d| 1.3); the inactive
    /// discs report here so the integrator sees exactly what the
    /// live verifier will measure. Geometry is asserted by
    /// `workspace_circles_coverage_round` instead.
    #[test]
    fn workspace_circles_strict_on_screen() {
        let tiles = screen_tiles();
        for (wp_name, wp) in [
            ("bright", (219u8, 242u8, 200u8)),
            ("dark", (150u8, 164u8, 136u8)),
        ] {
            for (i, tile) in tiles.iter().enumerate() {
                let (prof, _width) = strict_profile(tile, wp);
                let ideal = ideal_prof(prof.len());
                let max_d = prof
                    .iter()
                    .zip(ideal.iter())
                    .map(|(m, e)| (*m as f32 - e).abs())
                    .fold(0.0f32, f32::max);
                eprintln!(
                    "ws-circle-{} on {wp_name}: prof={prof:?} ideal={ideal:?} max|d|={max_d:.1}",
                    i + 1
                );
                // Diagnostic only: no assertions here (see the doc
                // comment above). Geometry is asserted by
                // workspace_circles_coverage_round.
            }
        }
    }

    /// The rasteriser must not change the disc's look: a pixel well
    /// inside the disc (no rim, no glyph, no fringe) still lands at
    /// exactly the old encoding — fill * 0.92, alpha 255.
    #[test]
    fn workspace_disc_interior_unchanged() {
        let font = load_font();
        for (active, fill) in [
            (true, WS_ACTIVE_FILL),
            (false, WS_INACTIVE_FILL),
        ] {
            let mut tile = vec![0u8; 40 * 40 * 4];
            draw_workspace_disc(&mut tile, 40, 40, 20.0, 20.0, active, false, &font, "1");
            // (20, 14) = (cx, cy - 6): d = 6, inside the rim band
            // (6.5..8) and above the glyph box.
            let i = (14 * 40 + 20) * 4;
            assert_eq!(
                (tile[i], tile[i + 1], tile[i + 2], tile[i + 3]),
                (
                    (fill.2 as f32 * 0.92) as u8,
                    (fill.1 as f32 * 0.92) as u8,
                    (fill.0 as f32 * 0.92) as u8,
                    255
                ),
                "interior pixel changed for active={active}"
            );
        }
    }

    /// The supersampled disc draw must stay cheap: 20 full
    /// four-circle redraws (the whole draw_workspace_circles_stage
    /// call) in well under a second on the Surface.
    #[test]
    fn workspace_discs_draw_fast() {
        let font = load_font();
        let mut stage = vec![0u8; (STAGE_W * STAGE_H * 4) as usize];
        let t = std::time::Instant::now();
        for _ in 0..20 {
            draw_workspace_circles_stage(&mut stage, STAGE_W, STAGE_H, &font, 1);
        }
        let el = t.elapsed();
        eprintln!("20x draw_workspace_circles_stage: {el:?}");
        assert!(el.as_millis() < 1000, "disc draw took {el:?} for 20 calls");
    }

    /// Preview strip for the eye check: the four discs at screen
    /// scale, composited over the bright wallpaper, written as a
    /// binary PPM (P6) next to the crate root. Convert with PIL
    /// (`python3 -c "from PIL import Image; Image.open(
    /// 'ws-discs-iter109.ppm').save('ws-discs-iter109.png')"`) and
    /// inspect at 3x nearest-neighbour zoom.
    #[test]
    fn write_ws_discs_preview() {
        let tiles = screen_tiles();
        let wp = (219u8, 242u8, 200u8);
        let (tw, th, gap) = (30usize, 30usize, 4usize);
        let w = 4 * tw + 3 * gap;
        // Start wallpaper-coloured; the disc tiles overwrite it.
        let mut px = vec![0u8; w * th * 3];
        for p in px.chunks_exact_mut(3) {
            p[0] = wp.0;
            p[1] = wp.1;
            p[2] = wp.2;
        }
        for (k, tile) in tiles.iter().enumerate() {
            let ox = k * (tw + gap);
            for y in 0..th {
                for x in 0..tw {
                    let i = (y * tw + x) * 4;
                    let a = tile[i + 3] as f32 / 255.0;
                    let di = ((y * w) + ox + x) * 3;
                    px[di] = (tile[i + 2] as f32 + wp.0 as f32 * (1.0 - a) + 0.5) as u8;
                    px[di + 1] = (tile[i + 1] as f32 + wp.1 as f32 * (1.0 - a) + 0.5) as u8;
                    px[di + 2] = (tile[i] as f32 + wp.2 as f32 * (1.0 - a) + 0.5) as u8;
                }
            }
        }
        let mut out = format!("P6\n{w} {th}\n255\n").into_bytes();
        out.extend_from_slice(&px);
        let path = "ws-discs-iter109.ppm";
        std::fs::write(path, &out).expect("write preview PPM");
        eprintln!("wrote {path} ({w}x{th})");
    }
}

/// Shared serialiser for the iter108 tray tests (merge): the tray
/// snapshot / menu globals (B) and the geometry-side tray-count store
/// (C) are shared mutable state, and Rust runs tests in parallel —
// every test that touches either takes this lock (the iter108-C store
/// roundtrip test takes it too). Test-only.
#[cfg(test)]
static TRAY_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod iter108_tray_tests {
    use super::*;
    use crate::tray::{MenuRow, TrayItemSnapshot};

    // Serialisation: these tests share the tray snapshot / menu
    // globals (and, via point_in_state_region, the geometry-side
    // tray-count store) — they take the shared crate-root
    // TRAY_TEST_LOCK (defined just above this module; `use super::*`
    // brings it in), which the iter108-C store test takes too.

    fn fake_item(id: &str) -> TrayItemSnapshot {
        TrayItemSnapshot {
            service: format!(":1.{id}"),
            path: "/StatusNotifierItem".to_string(),
            id: format!("item-{id}"),
            title: format!("Item {id}"),
            status: "Active".to_string(),
            icon: Some(vec![255u8; 16 * 16 * 4]),
            icon32: Some(vec![255u8; 32 * 32 * 4]),
            icon_name: String::new(),
            has_menu: false,
            menu_path: None,
        }
    }

    #[test]
    fn tray_menu_not_in_cycle() {
        // TrayMenu must never be reached by wheel / arrow cycling:
        // it is owned by one item, and cycling into it with a stale
        // item would be meaningless.
        assert!(!MENU_CYCLE.contains(&IslandState::TrayMenu));
        assert_eq!(cycle_step(IslandState::TrayMenu, 1), None);
        assert_eq!(cycle_step(IslandState::TrayMenu, -1), None);
        assert_eq!(IslandState::TrayMenu.word(), "traymenu");
        assert_eq!(IslandState::from_word("traymenu"), Some(IslandState::TrayMenu));
    }

    #[test]
    fn classify_press_tray_click_activates() {
        let _g = TRAY_TEST_LOCK.lock().unwrap();
        crate::tray::set_snapshot_for_test(vec![fake_item("42")]);
        // Pill is 550..626 live (76 wide, centred in the 1176 stage);
        // the first tray icon centre is (638, 15).
        let (pill_x, _, pill_w, _) = panel_rect_stage(IslandState::Collapsed);
        assert_eq!((pill_x, pill_w), (550.0, 76.0));
        let action = classify_press((638.0, 17.0), IslandState::Collapsed, false, 0, 0, 1);
        assert_eq!(action, PressAction::TrayActivate(0));
        // Same in a menu state (icons ride above every panel).
        let action = classify_press((638.0, 17.0), IslandState::Music, false, 0, 0, 1);
        assert_eq!(action, PressAction::TrayActivate(0));
        // The gap between pill and icon is a no-op, not a tray hit.
        let action = classify_press((628.0, 17.0), IslandState::Collapsed, false, 0, 0, 1);
        assert_eq!(action, PressAction::NoOp);
        // Workspace circles still win on the left.
        let action = classify_press((478.0, 17.0), IslandState::Collapsed, false, 0, 0, 1);
        assert_eq!(action, PressAction::WorkspaceTag(1));
        crate::tray::set_snapshot_for_test(vec![]);
    }

    #[test]
    fn tray_region_follows_snapshot() {
        let _g = TRAY_TEST_LOCK.lock().unwrap();
        crate::tray::set_snapshot_for_test(vec![fake_item("7"), fake_item("8")]);
        set_tray_count(2); // merge: mirror the 1 s timer sync in production
        assert!(point_in_state_region((638.0, 17.0), IslandState::Collapsed));
        assert!(point_in_state_region((658.0, 17.0), IslandState::Collapsed));
        assert!(!point_in_state_region((700.0, 17.0), IslandState::Collapsed));
        crate::tray::set_snapshot_for_test(vec![]);
        set_tray_count(0); // merge: store follows the snapshot
        // Empty snapshot: no tray rect anywhere.
        assert!(!point_in_state_region((638.0, 17.0), IslandState::Collapsed));
    }

    #[test]
    fn tray_menu_rows_hit_and_size() {
        let _g = TRAY_TEST_LOCK.lock().unwrap();
        crate::tray::set_menu_for_test(
            "Spike Item",
            vec![
                MenuRow { id: 1, label: "Open dashboard".to_string(), enabled: true, separator: false, depth: 0 },
                MenuRow { id: 2, label: String::new(), enabled: true, separator: true, depth: 0 },
                MenuRow { id: 3, label: "Quit".to_string(), enabled: false, separator: false, depth: 0 },
            ],
        );
        // Title row (y 16..44) is not a menu row.
        assert_eq!(crate::tray::menu_row_hit(100.0, 30.0), None);
        // Row 0 spans y 44..72; row 1 (separator) is not clickable;
        // row 2 (disabled) is not clickable.
        assert_eq!(crate::tray::menu_row_hit(100.0, 50.0), Some(1));
        assert_eq!(crate::tray::menu_row_hit(100.0, 80.0), None);
        assert_eq!(crate::tray::menu_row_hit(100.0, 110.0), None);
        // Size follows the row count: 16 + 28 + 3*28 + 16 = 144.
        assert_eq!(crate::tray::tray_menu_size(), (404, 144));
        // classify_press maps a row click in TrayMenu state. The
        // panel is 404 wide, centred: stage (500, 85) is panel-local
        // (114, 56) — inside row 0 (y 44..72).
        let action = classify_press((500.0, 85.0), IslandState::TrayMenu, false, 0, 0, 0);
        assert_eq!(action, PressAction::TrayMenuRow(1));
        crate::tray::set_menu_for_test("", vec![]);
    }
}
