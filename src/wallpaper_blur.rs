//! Fake frosted-glass backdrop (iter29, a3 retry).
//!
//! The compositor blur provably does NOT affect this layer surface
//! (iter28 A/B test: phase A vs B pixel-identical, mean|Δ| = 0.00), so
//! uniform translucency just shows SHARP wallpaper bleeding through —
//! the left tip of the pill reads 130–202 over a bright cloud while the
//! interior reads ~96. The fix is faked in-buffer: the island reads the
//! live wallpaper file itself, downsamples it to logical screen size,
//! box-blurs it (radius 12 logical px), and composites the blurred crop
//! UNDER the dark glass fill, so the surface pixel is (near-)opaque and
//! no sharp wallpaper ever leaks through the compositor.
//!
//! Cache: one blurred logical-size RGB buffer (1504x1002x3 ≈ 4.5 MB),
//! keyed by the wallpaper file's (mtime, size). `ensure_loaded()` stats
//! the file (cheap) on every draw and only re-decodes/re-blurs when the
//! key changes, so a wallpaper change is picked up on the next draw.
//! Unreadable/undecodable wallpaper → `None`, and callers fall back to
//! the iter28 black-fill behaviour.
//!
//! Iter85 RAM fix: the PNG is decoded row-by-row straight to logical
//! size (one reusable row buffer + a sliding window of a few source
//! rows) — no full-res frame buffer is ever held — and `malloc_trim`
//! runs after a wallpaper (re)load so glibc returns the transient
//! arenas instead of sitting on them.
//!
//! Iter88 frost fix: `wallpaper.png` is whatever
//! `library::apply_wallpaper` byte-copied there — including JPEGs
//! (wallhaven-*.jpg). The PNG-only decoder returned `None` for those,
//! so the frost has been dead since the file became JPEG on 13 Nov
//! 21:54 (iter37) and every panel fell back to the flat (4,4,4)
//! floor. `load_and_blur` now sniffs the magic bytes and decodes
//! JPEGs with zune-jpeg (pure Rust, offline-cached); PNGs take the
//! unchanged streaming path.
//!
//! Iter101 (Option 2, live-backdrop frost): the wallpaper FILE is the
//! wrong backdrop — over a live desktop the panels never show the
//! window behind them. The probe thread's grim capture is now also
//! streamed into a half-logical (752x501) blurred "clean frame"
//! (see LIVE_BACKDROP below) while the island is collapsed; panels
//! sample that frame per pixel instead of the wallpaper file. While a
//! panel is open the frame is frozen: the panel occludes its own rect
//! in every later grim, so the pixels behind an open panel are
//! unknowable to any client (mangoWM exposes no capture-without-layer
//! protocol); re-sampling while open would feed the island its own
//! output. Frost is therefore <=1 s stale at open, frozen for the
//! open session, refreshed on the next open. No neutralise and no
//! backdrop factor on the live path — the live sample IS the backdrop
//! and its colour is the point.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc, Mutex,
};
use std::time::SystemTime;

/// Logical screen size (mango eDP-1: 1504x1002 @ scale 1.5).
pub const LOGICAL_W: usize = 1504;
pub const LOGICAL_H: usize = 1002;
/// Box-blur radius in logical px (task range ~8–12).
pub const BLUR_RADIUS: usize = 12;

/// Hard cap on wallpaper pixels (iter85): bounds decode work; the
/// 2256x1504 panel wallpaper is 3.4M. Memory is already bounded
/// (rows stream); this bounds CPU.
const MAX_WALLPAPER_PIXELS: usize = 64 * 1024 * 1024;

/// Hard cap on one interlaced fallback frame (iter85): the fallback
/// holds a single full-frame buffer, so refuse frames over 64 MB
/// rather than allocating them.
const MAX_PNG_FRAME_BYTES: usize = 64 * 1024 * 1024;

/// Hard cap on JPEG pixels (iter88): zune-jpeg has no streaming row
/// API, so a JPEG decodes to one full RGB frame (3 bytes/px) before
/// the downsample — 16 MP caps that transient at ~48 MB, in line
/// with the 64 MB interlaced-PNG cap above. Real wallpapers are
/// ~3.4 MP. Frames over the cap fall back to the flat fill.
/// iter128: raised 16 -> 24 MP. The live wallpaper wallhaven-gwwwxd
/// is 6034x3394 = 20.5 MP, over the old cap — so since it was applied,
/// `ensure_loaded()` returned None and the whole wallpaper-file frost
/// path (pill, lock, every no-live fallback) silently read the flat
/// (4,4,4) floor. 24 MP covers gwwwxd and 52/56 library files; the
/// transient frame for gwwwxd is ~61 MB, freed before the blur, and
/// `ensure_loaded` runs `malloc_trim` after a reload. Four library
/// JPEGs are still over the cap (qzdrzl 44.7 MP, n6em3q 40.0 MP,
/// gpmg53 34.2 MP, q2qm5r 32.1 MP) — they still fall back to the flat
/// fill until the apply path downscales them or a streaming JPEG
/// decoder lands; the cap is a transient-RAM bound, not a format
/// limit, so do not raise it past 24 MP without re-measuring the
/// transient against the 100 MB PSS cap.
const MAX_JPEG_PIXELS: usize = 24 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Live-backdrop probe (iter30, a3 dark-fill fix).
// ---------------------------------------------------------------------------
// The fake frost samples the wallpaper FILE, so on a dark backdrop
// (tag 1: a fullscreen black window) it serves the bright wallpaper's
// warm cloud and the pill reads milk-chocolate brown (~115) over pure
// black. The island cannot see the real backdrop through the
// compositor — so it measures it: a background thread screenshots two
// strips flanking the pill at pill height with `grim -t ppm -` once a
// second and caches their median luminance. The strips sit just
// outside the pill (logical x 600..700 and 800..900, y 16..38 —
// physical x 900..1050 and 1200..1350, y 24..57 at scale 1.5) and
// above every panel (top 47), so no SETTLED island pixel ever covers
// them; during a morph the growing capsule CAN cover them, so the
// thread discards any capture taken while a morph is running
// (MORPHING, checked before AND after the capture — a morph lasts
// 400 ms, a capture ~50 ms, so a morph starting mid-capture is still
// running at the after-check).
//
// The probe is read-only (grim), runs in its own thread, and draws
// read only the cached value — a slow or failing grim can never block
// a frame. Default luminance is 255 (bright): before the first probe
// lands (~200 ms after the first draw) the island behaves like v0.19.

/// Probe strips in PHYSICAL pixels: (x0, y0, w, h).
const PROBE_STRIPS: [(i32, i32, i32, i32); 2] = [(900, 24, 150, 33), (1200, 24, 150, 33)];
/// Physical screen size the strips are defined for (eDP-1 2256x1504).
const PROBE_PHYS_W: i32 = 2256;
const PROBE_PHYS_H: i32 = 1504;

/// Cached median strip luminance x1000 (default 255000 = bright).
static PROBE_LUM_MILLI: AtomicU32 = AtomicU32::new(255_000);
/// True while a morph is running (set by main.rs draw_if_ready).
static MORPHING: AtomicBool = AtomicBool::new(false);
static PROBE_ONCE: std::sync::Once = std::sync::Once::new();

/// Live-backdrop clean frame (iter101, Option 2): half-logical
/// (752x501) blurred RGB of the last grim captured while the island
/// was collapsed and not morphing. 752 = 2256/3; 501 covers 1504
/// with integer row boundaries (see probe_read_full_frame). Blur
/// radius 6 at half res is ~12 logical px, the same as BLUR_RADIUS.
/// Buffer is 752*501*3 = 1,130,256 bytes (~1.1 MB).
/// iter103: the stored frame is neutralised at capture
/// (neutralise_sample — the same function the cards and the
/// wallpaper path use, so all callers follow one strengthening)
/// and carries a second 1.1 MB buffer with its BoxBlur(25) local
/// mean, the high-pass term's reference.
pub const LIVE_W: usize = 752;
pub const LIVE_H: usize = 501;
pub const LIVE_BLUR_RADIUS: usize = 6;

/// iter103 (frost visibility): shared frost tuning knobs — one place
/// for the deployer (see iterations/builder-fill-iter103.md).
/// FROST_ALPHA: fill alpha fa, 0.92 (iter128: was 0.80).
/// FROST_HP_K: high-pass gain k, 0.75 (Builder B used 0.70 in the
/// cards branch; 0.75 is this branch's start value — same 0.6-0.9
/// tuning range, predicted clean-patch std ~4.5-5 either way).
/// FROST_HP_RADIUS: local-mean BoxBlur radius on the half-res live
/// frame, 25 half-res px (approx 50 logical px).
/// NEUTRALISE_MIN_STRENGTH: minimum desaturation in
/// neutralise_sample, 0.88 (the shipped strength left a bright sky
/// at sample B-R ~+29, panel ~(28,32,34)).
/// FROST_EXPOSURE: multiplicative exposure in neutralise_sample,
/// 0.30 — iter113 replaces iter103's subtractive offset (100).
/// The offset was a hard threshold: every sample below lum ~100
/// collapsed to 0, so the dark leg (and all mid-tones) read flat
/// at the floor and the frost was invisible over the real dark
/// wallpaper. A scale preserves the backdrop's structure at every
/// level: a sky of lum ~117-135 still neutralises to lum ~35-42
/// (panel ~(10,10,11)-class at fa 0.80, the iter103 target),
/// while a dark cloud keeps its mottling instead of collapsing
/// to 0. Still plain frost, not glass: same neutral desaturated
/// blur, just uniformly darker instead of thresholded to black.
/// iter128 (B4, R5 re-grade): FROST_ALPHA 0.80 -> 0.92. Measured on
/// the real bright leg (harness_frost_panel_fill, gwwwxd snow):
/// the neutralised live sample means ~32, so at fa 0.80 the fill
/// meant ~11.3 — a grey slab against the reference's pure-black
/// ~2-3. At fa 0.92 the same sample fills ~5.5 and the dark leg
/// ~2.6, with the high-pass texture untouched (it is not scaled by
/// fa). FROST_HP_MAX_DEV caps |sample - local mean| at 12 so sharp
/// backdrop edges (UI text) cannot print through as bright specks:
/// the dark leg's max dev was 29 (-> +22 fill spike, max 34); the
/// soft mottling lives at |dev| ~1-6 and passes untouched.
pub const FROST_ALPHA: f32 = 0.92;
pub const FROST_HP_K: f32 = 0.75;
/// Cap on |sample - local mean| in the high-pass term (iter128):
/// a frost transmits low-frequency luminance variation, not sharp
/// edges. The fill clamps the deviation to +/- this before applying
/// FROST_HP_K, in every fill implementation (main.rs, CC panel +
/// cards, launcher).
pub const FROST_HP_MAX_DEV: f32 = 12.0;
pub const FROST_HP_RADIUS: usize = 25;
pub const NEUTRALISE_MIN_STRENGTH: f32 = 0.88;
pub const FROST_EXPOSURE: f32 = 0.30;

/// True while any menu panel is open (set by main.rs draw_if_ready
/// from MENU_CYCLE). While true the probe thread reads strips only
/// and the clean frame stays frozen (see module docs).
static PANEL_OPEN: AtomicBool = AtomicBool::new(false);

/// Cached clean frame, swapped in by the probe thread.
static LIVE_CACHE: Mutex<Option<Arc<LiveBackdrop>>> = Mutex::new(None);
/// Unix seconds when the cached clean frame was captured (0 = none).
static LIVE_CAPTURED_SECS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// Tell the probe thread whether a menu panel is open.
pub fn set_panel_open(open: bool) {
    PANEL_OPEN.store(open, Ordering::Relaxed);
}

/// The cached live-backdrop clean frame, if one has been captured.
/// Also starts the probe thread lazily (no-op without Wayland).
/// None until the first clean capture lands (~1 s after start).
pub fn live_backdrop() -> Option<Arc<LiveBackdrop>> {
    ensure_probe_running();
    LIVE_CACHE.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Age of the cached clean frame in seconds (None = never captured).
pub fn live_backdrop_age_secs() -> Option<u64> {
    let captured = LIVE_CAPTURED_SECS.load(Ordering::Relaxed);
    if captured == 0 {
        return None;
    }
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(captured);
    Some(now.saturating_sub(captured))
}

/// Blurred live backdrop in half-logical coords: RGB, LIVE_W x LIVE_H.
/// iter103: `data` is the NEUTRALISED frame (neutralise_sample at
/// capture — the panel reads neutral dark, B-R approx +3, not +27)
/// and `mean` is its BoxBlur(FROST_HP_RADIUS) local mean, the
/// high-pass term's reference. Same dims, +1.1 MB.
pub struct LiveBackdrop {
    pub data: Vec<u8>,
    pub mean: Vec<u8>,
}

impl LiveBackdrop {
    /// Nearest sample at logical screen coords (clamped to the screen).
    /// The buffer is already blurred, so nearest is smooth enough.
    /// Half-res: one buffer pixel covers 2x2 logical px.
    pub fn sample(&self, sx: f32, sy: f32) -> [u8; 3] {
        let x = ((sx * 0.5).floor() as i32).clamp(0, LIVE_W as i32 - 1) as usize;
        let y = ((sy * 0.5).floor() as i32).clamp(0, LIVE_H as i32 - 1) as usize;
        let i = (y * LIVE_W + x) * 3;
        [self.data[i], self.data[i + 1], self.data[i + 2]]
    }

    /// Nearest local-mean sample at logical screen coords (same
    /// mapping as `sample`). Equals `sample` on a flat backdrop, so
    /// the high-pass term is ~0 on a dark leg by construction.
    pub fn local_mean(&self, sx: f32, sy: f32) -> [u8; 3] {
        let x = ((sx * 0.5).floor() as i32).clamp(0, LIVE_W as i32 - 1) as usize;
        let y = ((sy * 0.5).floor() as i32).clamp(0, LIVE_H as i32 - 1) as usize;
        let i = (y * LIVE_W + x) * 3;
        [self.mean[i], self.mean[i + 1], self.mean[i + 2]]
    }
}

/// Tell the probe thread whether a morph is running.
pub fn set_morphing(m: bool) {
    MORPHING.store(m, Ordering::Relaxed);
}

/// Median luminance behind the pill (0..255), cached by the probe thread.
pub fn backdrop_luminance() -> f32 {
    PROBE_LUM_MILLI.load(Ordering::Relaxed) as f32 / 1000.0
}

/// Backdrop factor from a strip luminance: 0 at/below 15 (black window),
/// 1 at/above 75 (bright wallpaper sky). Measured: tag 5 strips read
/// ~100-218, tag 1 reads ~0 (medians), so the band 15..75 separates
/// them with room on both sides.
pub fn factor_for_lum(lum: f32) -> f32 {
    ((lum - 15.0) / 60.0).clamp(0.0, 1.0)
}

/// Current backdrop factor (0 = dark backdrop, 1 = bright).
pub fn backdrop_factor() -> f32 {
    factor_for_lum(backdrop_luminance())
}

/// Neutralise a blurred wallpaper sample toward grey (iter30 a3): the
/// warm brown cast is the cloud's tone amplified by the tint, so the
/// sample is desaturated toward its own luminance — 0.55 base, up to
/// 0.90 for strongly chromatic/warm samples, harder still when the
/// sample is dark (a dark warm sample is the milk-chocolate tell).
/// iter103: STRENGTHENED — the shipped strength is NOT enough. The
/// reference frame (f0210) and the v0.43 gaps prove the panel must
/// be neutral dark ~(10,9,10)-class, but the shipped function left a
/// bright sky at sample B-R ~+29 (panel ~(28,32,34), Builder B's
/// measurement). Two changes, both inside this one function so
/// every caller (live frame, wallpaper path, CC cards) follows
/// automatically:
/// (a) strength floor NEUTRALISE_MIN_STRENGTH (0.88) — the panel
/// B-R lands approx +3 instead of +5.7;
/// (b) multiplicative exposure FROST_EXPOSURE (0.30) — iter113
/// replaces the subtractive offset (100). Desaturation alone
/// preserves luminance, so at fa 0.80 a sky of lum ~117-135 can
/// never fill darker than ~(28,32,34); the exposure brings the
/// neutral sample to lum ~35-42 so the panel lands
/// ~(10,10,11)-class. Per-channel (preserves B-R), clamps at 0.
/// Unlike the offset, a scale preserves the backdrop's structure
/// below lum 100 instead of collapsing it to the floor — that is
/// what makes the frost visible over the real dark wallpaper.
/// Luminance is therefore NO LONGER preserved — that is the point.
/// The pill darkens on bright backdrops about as before (day sky
/// ~(11,11,12)-class) — the verifier must check the collapsed
/// pill still reads correctly.
pub fn neutralise_sample(bg: [u8; 3]) -> [u8; 3] {
    let r = bg[0] as f32;
    let g = bg[1] as f32;
    let b = bg[2] as f32;
    let lum = 0.299 * r + 0.587 * g + 0.114 * b;
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let chroma = (mx - mn) / 255.0;
    let warm = ((r - b) / 255.0).max(0.0);
    let mut s = 0.55 + 0.45 * chroma.max(warm);
    if lum < 80.0 {
        s = (s + 0.10).min(0.95);
    }
    let s = s.max(NEUTRALISE_MIN_STRENGTH).clamp(0.0, 0.95);
    let k = 1.0 - s;
    [
        ((lum + (r - lum) * k) * FROST_EXPOSURE).round().clamp(0.0, 255.0) as u8,
        ((lum + (g - lum) * k) * FROST_EXPOSURE).round().clamp(0.0, 255.0) as u8,
        ((lum + (b - lum) * k) * FROST_EXPOSURE).round().clamp(0.0, 255.0) as u8,
    ]
}

/// Apply the backdrop factor to a neutralised sample: on a dark
/// backdrop the frost collapses toward black, so the glass fill lands
/// on its ~18 floor instead of serving bright wallpaper over black.
pub fn apply_backdrop_factor(n: [u8; 3], f: f32) -> [u8; 3] {
    [
        (n[0] as f32 * f).round() as u8,
        (n[1] as f32 * f).round() as u8,
        (n[2] as f32 * f).round() as u8,
    ]
}

fn wallpaper_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/alex".to_string());
    PathBuf::from(home).join(".config/mango/wallpaper/wallpaper.png")
}

/// Blurred wallpaper in logical screen coords: RGB, LOGICAL_W x LOGICAL_H.
pub struct BlurredWallpaper {
    pub data: Vec<u8>,
}

impl BlurredWallpaper {
    /// Nearest sample at logical screen coords (clamped to the screen).
    /// The buffer is already blurred, so nearest is smooth enough.
    pub fn sample(&self, sx: f32, sy: f32) -> [u8; 3] {
        let x = (sx.floor() as i32).clamp(0, LOGICAL_W as i32 - 1) as usize;
        let y = (sy.floor() as i32).clamp(0, LOGICAL_H as i32 - 1) as usize;
        let i = (y * LOGICAL_W + x) * 3;
        [self.data[i], self.data[i + 1], self.data[i + 2]]
    }

    /// Backdrop-aware frost sample (iter30): the blurred sample,
    /// neutralised toward grey and scaled by the live backdrop factor.
    /// Glass fills use this instead of `sample`.
    pub fn sample_frost(&self, sx: f32, sy: f32) -> [u8; 3] {
        let n = neutralise_sample(self.sample(sx, sy));
        apply_backdrop_factor(n, backdrop_factor())
    }

    /// Uniform frost sample for one surface (iter33, a3 blur-only):
    /// the same pipeline as `sample_frost`, sampled ONCE at the
    /// surface's centre. Callers hoist this out of their fill loops so
    /// a panel/pill reads perfectly flat: a box blur cannot flatten a
    /// smooth wallpaper gradient (its symmetric window averages a
    /// linear ramp to the centre value), so per-pixel sampling left
    /// the pill tip ~17% brighter than its interior. This is blur
    /// taken to its limit (fully homogenised frost), not a tint or
    /// glass effect — the fill formula is unchanged.
    pub fn sample_frost_uniform(&self, cx: f32, cy: f32) -> [u8; 3] {
        self.sample_frost(cx, cy)
    }
}

struct CacheEntry {
    mtime_secs: u64,
    mtime_nanos: u32,
    len: u64,
    /// None = the load failed for this key (do not retry every frame).
    blurred: Option<Arc<BlurredWallpaper>>,
}

static CACHE: Mutex<Option<CacheEntry>> = Mutex::new(None);

/// Stat the wallpaper; (re)load + (re)blur only when (mtime, size)
/// changed. Cheap after the first load: one stat + one mutex lock.
/// Returns None when the file is unreadable or undecodable.
pub fn ensure_loaded() -> Option<Arc<BlurredWallpaper>> {
    // Backdrop probe (iter30): start the grim thread lazily on the
    // first draw; it keeps the cached strip luminance fresh so the
    // frost can follow the real backdrop (see sample_frost).
    ensure_probe_running();
    let path = wallpaper_path();
    let md = std::fs::metadata(&path).ok()?;
    let mtime = md.modified().ok()?;
    let (secs, nanos) = match mtime.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => (d.as_secs(), d.subsec_nanos()),
        Err(_) => (0, 0),
    };
    let len = md.len();
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(e) = guard.as_ref() {
        if e.mtime_secs == secs && e.mtime_nanos == nanos && e.len == len {
            return e.blurred.clone();
        }
    }
    let blurred = load_and_blur(&path).map(Arc::new);
    *guard = Some(CacheEntry {
        mtime_secs: secs,
        mtime_nanos: nanos,
        len,
        blurred: blurred.clone(),
    });
    // Iter85 RAM fix: the reload transiently held row buffers and
    // the old blurred buffer; return the freed arenas to the OS now
    // instead of letting glibc sit on them until the next wallpaper
    // change. Only on a real reload, never per frame.
    drop(guard);
    unsafe {
        libc::malloc_trim(0);
    }
    blurred
}

/// Start the backdrop probe thread (once, lazily — no-op without a
/// Wayland session, e.g. under `cargo test`).
pub fn ensure_probe_running() {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return;
    }
    PROBE_ONCE.call_once(|| {
        let _ = std::thread::Builder::new()
            .name("backdrop-probe".to_string())
            .spawn(probe_loop);
    });
}

fn probe_loop() {
    loop {
        let before = MORPHING.load(Ordering::Relaxed);
        if !before {
            if PANEL_OPEN.load(Ordering::Relaxed) {
                // Panel open: strips only (existing path). The clean
                // frame stays frozen — a capture now would contain
                // the island's own panel (feedback loop).
                if let Some(lum) = probe_backdrop() {
                    let after = MORPHING.load(Ordering::Relaxed);
                    if !after {
                        PROBE_LUM_MILLI.store((lum * 1000.0) as u32, Ordering::Relaxed);
                    }
                }
            } else if let Some((lum, frame)) = probe_full_frame() {
                let after = MORPHING.load(Ordering::Relaxed);
                if !after {
                    PROBE_LUM_MILLI.store((lum * 1000.0) as u32, Ordering::Relaxed);
                    {
                        let mut guard =
                            LIVE_CACHE.lock().unwrap_or_else(|e| e.into_inner());
                        *guard = Some(Arc::new(frame));
                    }
                    let now = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    LIVE_CAPTURED_SECS.store(now, Ordering::Relaxed);
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(1000));
    }
}

/// One probe capture: grim the full screen as PPM to stdout, read the
/// two strip bands, return their combined median luminance.
fn probe_backdrop() -> Option<f32> {
    let mut child: Child = Command::new("grim")
        .args(["-t", "ppm", "-"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let result = probe_read_strips(&mut child);
    let _ = child.kill();
    let _ = child.wait();
    result
}

fn probe_read_strips(child: &mut Child) -> Option<f32> {
    use std::io::{BufReader, Read};
    let stdout = child.stdout.take()?;
    let mut rdr = BufReader::new(stdout);
    // P6 header: magic, width, height, maxval (whitespace-separated,
    // `#` comments allowed), then exactly one whitespace byte before
    // the binary raster. Read byte-by-byte until 4 tokens are done;
    // the whitespace that terminates the 4th token IS that byte.
    let mut tokens: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut buf = [0u8; 1];
    let mut in_comment = false;
    while tokens.len() < 4 {
        let n = rdr.read(&mut buf).ok()?;
        if n == 0 {
            return None;
        }
        let c = buf[0] as char;
        if in_comment {
            if c == '\n' {
                in_comment = false;
            }
            continue;
        }
        if c == '#' {
            in_comment = true;
            continue;
        }
        if c.is_whitespace() {
            if !cur.is_empty() {
                tokens.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(c);
        }
    }
    if tokens.first().map(|s| s.as_str()) != Some("P6") {
        return None;
    }
    let w: i32 = tokens.get(1)?.parse().ok()?;
    let h: i32 = tokens.get(2)?.parse().ok()?;
    let maxval: i32 = tokens.get(3)?.parse().ok()?;
    if w != PROBE_PHYS_W || h != PROBE_PHYS_H || maxval != 255 {
        return None;
    }
    // Both strips share one band: rows y0..y0+hh, two x-ranges per row.
    let row_bytes = (w * 3) as usize;
    let mut row = vec![0u8; row_bytes];
    let mut skip = vec![0u8; row_bytes];
    let (_, sy, sw, sh) = PROBE_STRIPS[0];
    for _ in 0..sy {
        rdr.read_exact(&mut skip).ok()?;
    }
    let mut lums: Vec<f32> = Vec::with_capacity((sw * sh * 2) as usize);
    for _ in 0..sh {
        rdr.read_exact(&mut row).ok()?;
        for (x0, _, _, _) in PROBE_STRIPS {
            for x in x0..x0 + sw {
                let i = (x * 3) as usize;
                let (r, g, b) = (row[i] as f32, row[i + 1] as f32, row[i + 2] as f32);
                lums.push(0.299 * r + 0.587 * g + 0.114 * b);
            }
        }
    }
    lums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    lums.get(lums.len() / 2).copied()
}

/// One full-frame probe capture (iter101): grim the full screen as
/// PPM, stream it row by row, and (a) compute the same strip median
/// luminance as probe_read_strips and (b) area-average downsample
/// the whole frame to half-logical (LIVE_W x LIVE_H) and box-blur
/// it — the "clean frame" panels sample per pixel. Only called while
/// collapsed and not morphing (see probe_loop). Returns None on any
/// read/parse failure; the caller keeps the previous cached values.
fn probe_full_frame() -> Option<(f32, LiveBackdrop)> {
    let mut child: Child = Command::new("grim")
        .args(["-t", "ppm", "-"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let result = probe_read_full_frame(&mut child);
    let _ = child.kill();
    let _ = child.wait();
    result
}

fn probe_read_full_frame(child: &mut Child) -> Option<(f32, LiveBackdrop)> {
    use std::io::{BufReader, Read};
    let stdout = child.stdout.take()?;
    let mut rdr = BufReader::new(stdout);
    // P6 header (same parse as probe_read_strips — kept in sync).
    let mut tokens: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut buf = [0u8; 1];
    let mut in_comment = false;
    while tokens.len() < 4 {
        let n = rdr.read(&mut buf).ok()?;
        if n == 0 {
            return None;
        }
        let c = buf[0] as char;
        if in_comment {
            if c == '\n' {
                in_comment = false;
            }
            continue;
        }
        if c == '#' {
            in_comment = true;
            continue;
        }
        if c.is_whitespace() {
            if !cur.is_empty() {
                tokens.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(c);
        }
    }
    if tokens.first().map(|s| s.as_str()) != Some("P6") {
        return None;
    }
    let w: i32 = tokens.get(1)?.parse().ok()?;
    let h: i32 = tokens.get(2)?.parse().ok()?;
    let maxval: i32 = tokens.get(3)?.parse().ok()?;
    if w != PROBE_PHYS_W || h != PROBE_PHYS_H || maxval != 255 {
        return None;
    }
    // Stream rows: strip luminances + half-res area average in one pass.
    // Horizontal: exactly 3 physical px per dest px (2256 = 752*3).
    // Vertical: dest row dy covers source rows [dy*1504/501,
    // (dy+1)*1504/501) — 3 rows each, one dest row gets 4
    // (1504 = 501*3 + 1); the per-row divisor handles both.
    let row_bytes = (w * 3) as usize;
    let mut row = vec![0u8; row_bytes];
    let (_, sy, sw, sh) = PROBE_STRIPS[0];
    let mut lums: Vec<f32> = Vec::with_capacity((sw * sh * 2) as usize);
    let mut out = vec![0u8; LIVE_W * LIVE_H * 3];
    let mut acc = vec![0u32; LIVE_W * 3];
    let mut cur_dy: i32 = -1;
    let mut rows_in_dest: u32 = 0;
    for y in 0..h {
        rdr.read_exact(&mut row).ok()?;
        let dy = (y * LIVE_H as i32) / h;
        if dy != cur_dy {
            if cur_dy >= 0 {
                flush_live_dest_row(&mut out, cur_dy, &acc, rows_in_dest);
            }
            acc.fill(0);
            cur_dy = dy;
            rows_in_dest = 0;
        }
        rows_in_dest += 1;
        for dx in 0..LIVE_W {
            let si = dx * 9;
            let di = dx * 3;
            acc[di] += row[si] as u32 + row[si + 3] as u32 + row[si + 6] as u32;
            acc[di + 1] += row[si + 1] as u32 + row[si + 4] as u32 + row[si + 7] as u32;
            acc[di + 2] += row[si + 2] as u32 + row[si + 5] as u32 + row[si + 8] as u32;
        }
        if y >= sy && y < sy + sh {
            for (x0, _, _, _) in PROBE_STRIPS {
                for x in x0..x0 + sw {
                    let i = (x * 3) as usize;
                    let (r, g, b) = (row[i] as f32, row[i + 1] as f32, row[i + 2] as f32);
                    lums.push(0.299 * r + 0.587 * g + 0.114 * b);
                }
            }
        }
    }
    if cur_dy >= 0 {
        flush_live_dest_row(&mut out, cur_dy, &acc, rows_in_dest);
    }
    lums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let lum = lums.get(lums.len() / 2).copied()?;
    box_blur_rgb(&mut out, LIVE_W, LIVE_H, LIVE_BLUR_RADIUS);
    Some((lum, finish_live_frame(out)))
}

/// iter103: finish a clean frame — neutralise the r6-blurred frame
/// (neutralise_sample, the shared strengthened function: desaturate
/// with strength >= 0.88, then the subtractive exposure offset) and
/// precompute its BoxBlur(FROST_HP_RADIUS) local mean for the fill's
/// high-pass term. Cost: one 1.1 MB clone + one separable blur per
/// clean capture (approx 1/s, probe thread only, while collapsed);
/// the per-pixel draw cost grows by three f32 ops. The deviation
/// term is sample minus local mean of this neutralised frame, so it
/// is ~0 on a dark backdrop by construction (the offset crushes
/// dark samples to 0, sample and mean alike).
fn finish_live_frame(mut data: Vec<u8>) -> LiveBackdrop {
    debug_assert_eq!(data.len(), LIVE_W * LIVE_H * 3);
    for px in data.chunks_exact_mut(3) {
        let n = neutralise_sample([px[0], px[1], px[2]]);
        px[0] = n[0];
        px[1] = n[1];
        px[2] = n[2];
    }
    let mut mean = data.clone();
    box_blur_rgb(&mut mean, LIVE_W, LIVE_H, FROST_HP_RADIUS);
    LiveBackdrop { data, mean }
}

/// Divide one accumulated dest row into the live buffer: each dest
/// pixel averages rows*3 source pixels (3 across, `rows` down).
fn flush_live_dest_row(out: &mut [u8], dy: i32, acc: &[u32], rows: u32) {
    let div = rows * 3;
    if div == 0 {
        return;
    }
    let base = (dy as usize) * LIVE_W * 3;
    for i in 0..LIVE_W * 3 {
        out[base + i] = (acc[i] / div) as u8;
    }
}

/// Wallpaper image format, sniffed from the magic bytes (iter88):
/// the filename says `.png` but the bytes are whatever
/// `library::apply_wallpaper` copied there.
#[derive(Debug, PartialEq, Eq)]
enum ImgFormat {
    Png,
    Jpeg,
}

/// Sniff the image format from the first 8 bytes: PNG signature
/// `89 50 4E 47 0D 0A 1A 0A`, JPEG SOI `FF D8 FF`. Anything else
/// (corrupt, truncated, unknown) -> `None`, and callers fall back to
/// the flat fill exactly as before.
fn file_format(path: &Path) -> Option<ImgFormat> {
    use std::io::Read;
    let mut f = File::open(path).ok()?;
    let mut magic = [0u8; 8];
    f.read_exact(&mut magic).ok()?;
    if magic == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
        Some(ImgFormat::Png)
    } else if magic[0] == 0xFF && magic[1] == 0xD8 && magic[2] == 0xFF {
        Some(ImgFormat::Jpeg)
    } else {
        None
    }
}

fn load_and_blur(path: &Path) -> Option<BlurredWallpaper> {
    let (mut logical, w, h) = match file_format(path)? {
        ImgFormat::Png => decode_png_to_logical(path)?,
        ImgFormat::Jpeg => decode_jpeg_to_logical(path)?,
    };
    debug_assert_eq!((w, h), (LOGICAL_W, LOGICAL_H));
    box_blur_rgb(&mut logical, w, h, BLUR_RADIUS);
    // iter91 (B3 RAM): free the reusable blur scratch (~4.3 MB) now
    // that the blur is done - it is only needed DURING the blur and
    // is re-grown on the next one. ensure_loaded's malloc_trim after
    // a reload returns the pages to the OS.
    {
        let mut scratch = BLUR_SCRATCH.lock().unwrap_or_else(|e| e.into_inner());
        scratch.clear();
        scratch.shrink_to_fit();
    }
    Some(BlurredWallpaper { data: logical })
}

/// Decode a JPEG wallpaper straight to logical screen size (iter88):
/// same output contract as `decode_png_to_logical` (RGB, LOGICAL_W x
/// LOGICAL_H). zune-jpeg decodes the full frame (RGB8 — its default
/// output colorspace is RGB); the frame is refused unless it is
/// exactly w*h*3 bytes, so non-RGB output (CMYK, ...) falls back to
/// the flat fill instead of misinterpreting bytes. The transient
/// full frame (~11 MB for 2560x1440) is freed before the blur runs,
/// and `ensure_loaded` runs `malloc_trim` after a reload, so
/// steady-state RAM is unchanged. Returns (rgb, LOGICAL_W, LOGICAL_H).
fn decode_jpeg_to_logical(path: &Path) -> Option<(Vec<u8>, usize, usize)> {
    let bytes = std::fs::read(path).ok()?;
    let mut dec = zune_jpeg::JpegDecoder::new(std::io::Cursor::new(bytes));
    dec.decode_headers().ok()?;
    let info = dec.info()?;
    let (w, h) = (info.width as usize, info.height as usize);
    if w == 0 || h == 0 || w.checked_mul(h)? > MAX_JPEG_PIXELS {
        return None;
    }
    let rgb = dec.decode().ok()?;
    if rgb.len() != w * h * 3 {
        return None;
    }
    let logical = downsample_to_logical(&rgb, w, h);
    Some((logical, LOGICAL_W, LOGICAL_H))
}

/// Convert one `png`-crate output row (after
/// `normalize_to_color8`) to packed RGB8. `out` must hold exactly
/// `w * 3` bytes.
fn png_row_to_rgb(row: &[u8], ct: png::ColorType, w: usize, out: &mut [u8]) {
    use png::ColorType;
    debug_assert_eq!(out.len(), w * 3);
    match ct {
        ColorType::Grayscale => {
            for x in 0..w {
                let g = row[x];
                let o = x * 3;
                out[o] = g;
                out[o + 1] = g;
                out[o + 2] = g;
            }
        }
        ColorType::GrayscaleAlpha => {
            for x in 0..w {
                let s = x * 2;
                let g = row[s];
                let o = x * 3;
                out[o] = g;
                out[o + 1] = g;
                out[o + 2] = g;
            }
        }
        ColorType::Rgb => {
            out[..w * 3].copy_from_slice(&row[..w * 3]);
        }
        ColorType::Rgba => {
            for x in 0..w {
                let s = x * 4;
                let o = x * 3;
                out[o] = row[s];
                out[o + 1] = row[s + 1];
                out[o + 2] = row[s + 2];
            }
        }
        ColorType::Indexed => {
            // normalize_to_color8 expands palettes, so this is
            // unreachable — stay total rather than panicking.
            for x in 0..w {
                let o = x * 3;
                out[o] = 0;
                out[o + 1] = 0;
                out[o + 2] = 0;
            }
        }
    }
}

/// Decode a PNG wallpaper straight to logical screen size (iter85
/// RAM fix). Rows stream through the `png` crate
/// (`normalize_to_color8`) and are area-averaged into the logical
/// buffer through a sliding window of at most a few source rows —
/// the old path held the full decoded frame (~10–14 MB) AND the
/// full RGB copy AND the logical buffer at once. Peak here is the
/// 4.5 MB logical buffer + a few rows. Interlaced PNGs take the
/// single-buffer `next_frame` fallback below (rare; the island's
/// own pipeline writes non-interlaced files).
/// Returns (rgb, LOGICAL_W, LOGICAL_H).
fn decode_png_to_logical(path: &Path) -> Option<(Vec<u8>, usize, usize)> {
    use std::collections::VecDeque;
    let file = File::open(path).ok()?;
    let mut decoder = png::Decoder::new(file);
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let (w, h, interlaced) = {
        let info = reader.info();
        (
            info.width as usize,
            info.height as usize,
            info.interlaced,
        )
    };
    if w == 0 || h == 0 {
        return None;
    }
    if w.checked_mul(h)? > MAX_WALLPAPER_PIXELS {
        return None;
    }
    if interlaced {
        return decode_png_to_logical_interlaced(&mut reader, w, h);
    }
    let (out_ct, _) = reader.output_color_type();
    let line_size = reader.output_line_size(w as u32);
    if line_size == 0 {
        return None;
    }
    let sx = w as f32 / LOGICAL_W as f32;
    let sy = h as f32 / LOGICAL_H as f32;
    let mut out = vec![0u8; LOGICAL_W * LOGICAL_H * 3];
    // Sliding window of source rows (packed RGB8, w*3 each) covering
    // the logical row being built; at most ceil(sy)+1 rows are ever
    // held (3 rows, ~20 KB, for the 2256x1504 wallpaper).
    let mut window: VecDeque<(usize, Vec<u8>)> = VecDeque::new();
    let mut next_y = 0usize;
    for ly in 0..LOGICAL_H {
        let y0 = ((ly as f32 * sy).floor() as usize).min(h - 1);
        let y1 = (((ly + 1) as f32 * sy).ceil() as usize).min(h).max(y0 + 1);
        while next_y < y1 {
            let row = reader.next_row().ok()??;
            let data = row.data();
            if data.len() < line_size {
                return None;
            }
            let mut buf = vec![0u8; w * 3];
            png_row_to_rgb(&data[..line_size], out_ct, w, &mut buf);
            window.push_back((next_y, buf));
            next_y += 1;
        }
        while window.front().map(|(y, _)| *y < y0).unwrap_or(false) {
            window.pop_front();
        }
        let y0f = ly as f32 * sy;
        let y1f = (ly + 1) as f32 * sy;
        for lx in 0..LOGICAL_W {
            let x0f = lx as f32 * sx;
            let x1f = (lx + 1) as f32 * sx;
            let sx0 = (x0f.floor() as usize).min(w - 1);
            let sx1 = ((x1f.ceil() as usize).min(w)).max(sx0 + 1);
            let mut acc = [0f32; 3];
            let mut wsum = 0f32;
            for (ry, rdata) in window.iter() {
                let wy = y1f.min((*ry + 1) as f32) - y0f.max(*ry as f32);
                if wy <= 0.0 {
                    continue;
                }
                for sxi in sx0..sx1 {
                    let wx = x1f.min((sxi + 1) as f32) - x0f.max(sxi as f32);
                    if wx <= 0.0 {
                        continue;
                    }
                    let wt = wx * wy;
                    let si = sxi * 3;
                    acc[0] += rdata[si] as f32 * wt;
                    acc[1] += rdata[si + 1] as f32 * wt;
                    acc[2] += rdata[si + 2] as f32 * wt;
                    wsum += wt;
                }
            }
            let o = (ly * LOGICAL_W + lx) * 3;
            if wsum > 0.0 {
                out[o] = (acc[0] / wsum).round() as u8;
                out[o + 1] = (acc[1] / wsum).round() as u8;
                out[o + 2] = (acc[2] / wsum).round() as u8;
            }
        }
    }
    Some((out, LOGICAL_W, LOGICAL_H))
}

/// Interlaced fallback for `decode_png_to_logical`: one full-frame
/// buffer via `next_frame`, converted to RGB and downsampled with
/// the existing area-average path. Only for interlaced wallpapers
/// (the island's own pipeline writes non-interlaced files); a
/// frame over 64 MB is refused rather than allocated.
fn decode_png_to_logical_interlaced(
    reader: &mut png::Reader<File>,
    w: usize,
    h: usize,
) -> Option<(Vec<u8>, usize, usize)> {
    let frame_bytes = reader.output_buffer_size();
    if frame_bytes == 0 || frame_bytes > MAX_PNG_FRAME_BYTES {
        return None;
    }
    let (out_ct, _) = reader.output_color_type();
    let mut buf = vec![0u8; frame_bytes];
    let frame = reader.next_frame(&mut buf).ok()?;
    let line_size = frame.line_size;
    if line_size == 0 {
        return None;
    }
    let mut rgb = vec![0u8; w * h * 3];
    let mut row_rgb = vec![0u8; w * 3];
    for y in 0..h {
        let row = buf.get(y * line_size..(y + 1) * line_size)?;
        png_row_to_rgb(row, out_ct, w, &mut row_rgb);
        rgb[y * w * 3..(y + 1) * w * 3].copy_from_slice(&row_rgb);
    }
    // Drop the full frame before the downsample allocates the
    // logical buffer, so peak stays frame + rgb, never + logical.
    drop(buf);
    let logical = downsample_to_logical(&rgb, w, h);
    Some((logical, LOGICAL_W, LOGICAL_H))
}

/// Area-average downsample of an RGB image to logical screen size.
/// Each logical pixel averages the source rect it covers (weights =
/// overlap area), so a 1.5x factor stays exact and alias-free.
fn downsample_to_logical(rgb: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut out = vec![0u8; LOGICAL_W * LOGICAL_H * 3];
    let sx = w as f32 / LOGICAL_W as f32;
    let sy = h as f32 / LOGICAL_H as f32;
    for ly in 0..LOGICAL_H {
        let y0 = ly as f32 * sy;
        let y1 = (ly + 1) as f32 * sy;
        let sy0 = (y0.floor() as usize).min(h.saturating_sub(1));
        let sy1 = ((y1.ceil() as usize).min(h)).max(sy0 + 1);
        for lx in 0..LOGICAL_W {
            let x0 = lx as f32 * sx;
            let x1 = (lx + 1) as f32 * sx;
            let sx0 = (x0.floor() as usize).min(w.saturating_sub(1));
            let sx1 = ((x1.ceil() as usize).min(w)).max(sx0 + 1);
            let mut acc = [0f32; 3];
            let mut wsum = 0f32;
            for syi in sy0..sy1 {
                let wy = (y1.min(syi as f32 + 1.0) - y0.max(syi as f32)).max(0.0);
                if wy <= 0.0 {
                    continue;
                }
                for sxi in sx0..sx1 {
                    let wx = (x1.min(sxi as f32 + 1.0) - x0.max(sxi as f32)).max(0.0);
                    if wx <= 0.0 {
                        continue;
                    }
                    let wt = wx * wy;
                    let si = (syi * w + sxi) * 3;
                    acc[0] += rgb[si] as f32 * wt;
                    acc[1] += rgb[si + 1] as f32 * wt;
                    acc[2] += rgb[si + 2] as f32 * wt;
                    wsum += wt;
                }
            }
            let o = (ly * LOGICAL_W + lx) * 3;
            if wsum > 0.0 {
                out[o] = (acc[0] / wsum).round() as u8;
                out[o + 1] = (acc[1] / wsum).round() as u8;
                out[o + 2] = (acc[2] / wsum).round() as u8;
            }
        }
    }
    out
}

/// Reusable blur scratch (iter85): one logical-size buffer shared by
/// every `box_blur_rgb` call instead of a fresh 4.5 MB allocation
/// per wallpaper apply.
static BLUR_SCRATCH: Mutex<Vec<u8>> = Mutex::new(Vec::new());

/// Separable box blur (sliding window, O(n) per pass): one horizontal
/// pass into a temp buffer, then one vertical pass back into `buf`.
/// Edge pixels are clamped (replicated), so a uniform image is unchanged
/// and a step spreads over exactly 2r+1 px.
fn box_blur_rgb(buf: &mut Vec<u8>, w: usize, h: usize, r: usize) {
    if r == 0 || w == 0 || h == 0 {
        return;
    }
    let win = (2 * r + 1) as u32;
    // Reusable scratch (iter85 RAM fix): the horizontal pass needs a
    // second logical-size buffer (4.5 MB); allocating it per
    // wallpaper apply churned the heap. The Mutex also serialises
    // concurrent test calls to this function.
    let mut scratch = BLUR_SCRATCH.lock().unwrap_or_else(|e| e.into_inner());
    if scratch.len() != buf.len() {
        scratch.resize(buf.len(), 0);
    }
    let tmp = &mut *scratch;
    // Horizontal pass.
    for y in 0..h {
        let mut sum = [0u32; 3];
        for i in 0..2 * r + 1 {
            let xi = (i as i32 - r as i32).clamp(0, w as i32 - 1) as usize;
            let si = (y * w + xi) * 3;
            sum[0] += buf[si] as u32;
            sum[1] += buf[si + 1] as u32;
            sum[2] += buf[si + 2] as u32;
        }
        for x in 0..w {
            let di = (y * w + x) * 3;
            tmp[di] = (sum[0] / win) as u8;
            tmp[di + 1] = (sum[1] / win) as u8;
            tmp[di + 2] = (sum[2] / win) as u8;
            let rem = (x as i32 - r as i32).clamp(0, w as i32 - 1) as usize;
            let add = (x as i32 + r as i32 + 1).clamp(0, w as i32 - 1) as usize;
            let si_r = (y * w + rem) * 3;
            let si_a = (y * w + add) * 3;
            for c in 0..3 {
                sum[c] = sum[c] - buf[si_r + c] as u32 + buf[si_a + c] as u32;
            }
        }
    }
    // Vertical pass (tmp -> buf).
    for x in 0..w {
        let mut sum = [0u32; 3];
        for i in 0..2 * r + 1 {
            let yi = (i as i32 - r as i32).clamp(0, h as i32 - 1) as usize;
            let si = (yi * w + x) * 3;
            sum[0] += tmp[si] as u32;
            sum[1] += tmp[si + 1] as u32;
            sum[2] += tmp[si + 2] as u32;
        }
        for y in 0..h {
            let di = (y * w + x) * 3;
            buf[di] = (sum[0] / win) as u8;
            buf[di + 1] = (sum[1] / win) as u8;
            buf[di + 2] = (sum[2] / win) as u8;
            let rem = (y as i32 - r as i32).clamp(0, h as i32 - 1) as usize;
            let add = (y as i32 + r as i32 + 1).clamp(0, h as i32 - 1) as usize;
            let si_r = (rem * w + x) * 3;
            let si_a = (add * w + x) * 3;
            for c in 0..3 {
                sum[c] = sum[c] - tmp[si_r + c] as u32 + tmp[si_a + c] as u32;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- iter101 live-backdrop tests ---
    #[test]
    fn live_sample_maps_logical_to_half_res() {
        let mut data = vec![10u8, 20, 30].repeat(LIVE_W * LIVE_H);
        let i = (50 * LIVE_W + 100) * 3;
        data[i] = 200;
        data[i + 1] = 150;
        data[i + 2] = 100;
        let lb = LiveBackdrop {
            data: data.clone(),
            mean: data,
        };
        // Logical (201, 101) -> half (100, 50).
        assert_eq!(lb.sample(201.0, 101.0), [200, 150, 100]);
        assert_eq!(lb.sample(0.0, 0.0), [10, 20, 30]);
        // iter103: the local-mean buffer uses the same mapping.
        assert_eq!(lb.local_mean(201.0, 101.0), [200, 150, 100]);
        assert_eq!(lb.local_mean(0.0, 0.0), [10, 20, 30]);
        // Clamped at the far edge: logical (1503.9, 1001.9) -> (751, 500).
        assert_eq!(lb.sample(1503.9, 1001.9), [10, 20, 30]);
        assert_eq!(lb.sample(9999.0, -5.0), [10, 20, 30]);
    }

    #[test]
    fn finish_live_frame_flat_stays_flat_and_mean_matches() {
        // A flat frame neutralises flat and its local mean equals
        // the sample everywhere, so the high-pass term is 0.
        let flat = vec![100u8, 120, 140].repeat(LIVE_W * LIVE_H);
        let lb = finish_live_frame(flat);
        assert_eq!(lb.sample(400.0, 300.0), lb.local_mean(400.0, 300.0));
        assert_eq!(lb.sample(0.0, 0.0), lb.local_mean(1503.0, 1001.0));
        // Neutralised flat sky sample: still near-grey (B-R <= 6).
        let s = lb.sample(400.0, 300.0);
        assert!((s[2] as i32 - s[0] as i32).abs() <= 6, "flat {s:?}");
    }

    #[test]
    fn finish_live_frame_step_local_mean_lags_sample() {
        // Left half dark, right half bright: at the step the local
        // mean sits between the two, so sample - mean != 0 (the
        // mottling signal); deep inside a uniform half it is ~0.
        let mut data = vec![0u8; LIVE_W * LIVE_H * 3];
        for y in 0..LIVE_H {
            for x in LIVE_W / 2..LIVE_W {
                let i = (y * LIVE_W + x) * 3;
                data[i] = 200;
                data[i + 1] = 200;
                data[i + 2] = 200;
            }
        }
        let lb = finish_live_frame(data);
        // Deep in the bright half (logical x 1400 -> half 700):
        let s = lb.sample(1400.0, 500.0);
        let m = lb.local_mean(1400.0, 500.0);
        assert_eq!(s, m);
        // At the step (logical x 760 -> half 380): mean < sample.
        let s2 = lb.sample(760.0, 500.0);
        let m2 = lb.local_mean(760.0, 500.0);
        assert!(m2[0] < s2[0], "step mean {m2:?} vs sample {s2:?}");
        assert!((25..=45).contains(&m2[0]), "step mean {m2:?}");
    }

    #[test]
    fn live_dest_row_flush_averages() {
        // 3 source rows x 3 source px per dest px: 9 px of 20 -> 20.
        let mut out = vec![0u8; LIVE_W * LIVE_H * 3];
        let acc = vec![180u32; LIVE_W * 3];
        flush_live_dest_row(&mut out, 0, &acc, 3);
        assert!(out[..LIVE_W * 3].iter().all(|&v| v == 20));
        assert!(out[LIVE_W * 3..].iter().all(|&v| v == 0));
    }

    #[test]
    fn live_dest_row_mapping_covers_all_rows() {
        // dy = y*501/1504: first dest row gets 4 source rows (the +1),
        // last source row lands in dest row 500, every dest row hit.
        let mut seen = [false; LIVE_H];
        for y in 0..PROBE_PHYS_H {
            seen[((y * LIVE_H as i32) / PROBE_PHYS_H) as usize] = true;
        }
        assert!(seen.iter().all(|&s| s));
        assert_eq!((0 * LIVE_H as i32) / PROBE_PHYS_H, 0);
        assert_eq!((3 * LIVE_H as i32) / PROBE_PHYS_H, 0);
        assert_eq!((4 * LIVE_H as i32) / PROBE_PHYS_H, 1);
        assert_eq!((1503 * LIVE_H as i32) / PROBE_PHYS_H, 500);
    }

    #[test]
    fn live_backdrop_none_without_wayland() {
        // Under cargo test there is no Wayland session, so the probe
        // never runs and no clean frame exists.
        if std::env::var_os("WAYLAND_DISPLAY").is_none() {
            assert!(live_backdrop().is_none());
        }
    }

    #[test]
    fn box_blur_uniform_unchanged() {
        let mut buf = vec![200u8; 8 * 6 * 3];
        box_blur_rgb(&mut buf, 8, 6, 3);
        assert!(buf.iter().all(|&v| v == 200));
    }

    #[test]
    fn box_blur_step_spreads_over_2r_plus_1() {
        // 9x1 image: 0s then 255s, step between x=3 and x=4, radius 2.
        let mut buf = vec![0u8; 9 * 3];
        for x in 4..9 {
            buf[x * 3] = 255;
            buf[x * 3 + 1] = 255;
            buf[x * 3 + 2] = 255;
        }
        box_blur_rgb(&mut buf, 9, 1, 2);
        // Window is 5 wide: x=1 sees 0/5 white, x=2 sees 1/5, x=3 2/5,
        // x=4 3/5, x=5 4/5, x=6+ 5/5.
        assert_eq!(buf[1 * 3], 0);
        assert_eq!(buf[2 * 3], 51);
        assert_eq!(buf[3 * 3], 102);
        assert_eq!(buf[4 * 3], 153);
        assert_eq!(buf[5 * 3], 204);
        assert_eq!(buf[6 * 3], 255);
        assert_eq!(buf[8 * 3], 255);
    }

    #[test]
    fn box_blur_radius_12_kills_a_narrow_spike() {
        // A 1-px white line on black, radius 12: peak must drop below
        // 255/25 + margin — the smudge mechanism (a sharp bright edge)
        // cannot survive the blur.
        let w = 64usize;
        let mut buf = vec![0u8; w * 3];
        buf[32 * 3] = 255;
        box_blur_rgb(&mut buf, w, 1, 12);
        let peak = buf.chunks_exact(3).map(|p| p[0]).max().unwrap();
        assert!(peak <= 20, "spike peak {peak} after r=12 blur");
    }

    #[test]
    fn downsample_uniform_stays_uniform() {
        let rgb = vec![77u8; 6 * 4 * 3];
        let out = downsample_to_logical(&rgb, 6, 4);
        assert_eq!(out.len(), LOGICAL_W * LOGICAL_H * 3);
        assert!(out.iter().all(|&v| v == 77));
    }

    #[test]
    fn downsample_two_by_two_averages() {
        // 2x2 source, one white pixel: the first logical pixel covers
        // the whole source only if the source is 2x2 mapped onto a
        // 1504x1002 grid — instead verify the averaging kernel directly
        // on a 3008x2004 source (exactly 2x logical): logical (0,0)
        // averages source (0..2, 0..2).
        let mut rgb = vec![0u8; 3008 * 2004 * 3];
        rgb[0] = 200;
        rgb[1] = 100;
        rgb[2] = 50;
        let out = downsample_to_logical(&rgb, 3008, 2004);
        assert_eq!([out[0], out[1], out[2]], [50, 25, 13]); // 12.5 rounds to 13
    }

    #[test]
    fn sample_clamps_to_screen() {
        let bw = BlurredWallpaper {
            data: vec![9u8; LOGICAL_W * LOGICAL_H * 3],
        };
        assert_eq!(bw.sample(-5.0, -5.0), [9, 9, 9]);
        assert_eq!(bw.sample(99999.0, 99999.0), [9, 9, 9]);
        assert_eq!(bw.sample(0.4, 0.4), [9, 9, 9]);
    }

    #[test]
    fn decode_png_to_logical_roundtrip() {
        // Encode a tiny 4x3 RGB PNG, decode it straight to logical
        // size through the streaming path.
        use std::io::Cursor;
        let mut encoded = Vec::new();
        {
            let mut enc = png::Encoder::new(Cursor::new(&mut encoded), 4, 3);
            enc.set_color(png::ColorType::Rgb);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().unwrap();
            let mut rows = Vec::new();
            for y in 0..3u8 {
                for x in 0..4u8 {
                    rows.push(x * 40);
                    rows.push(y * 60);
                    rows.push(200);
                }
            }
            w.write_image_data(&rows).unwrap();
            w.finish().unwrap();
        }
        let path = std::env::temp_dir().join("iter85-png-roundtrip.png");
        std::fs::write(&path, &encoded).unwrap();
        let (rgb, w, h) = decode_png_to_logical(&path).expect("decode our own PNG");
        assert_eq!((w, h), (LOGICAL_W, LOGICAL_H));
        assert_eq!(rgb.len(), LOGICAL_W * LOGICAL_H * 3);
        // Logical (0,0) covers only source (0,0); logical (1500,1000)
        // covers only source (3,2).
        assert_eq!(&rgb[0..3], &[0, 0, 200]);
        let i = (1000 * LOGICAL_W + 1500) * 3;
        assert_eq!(&rgb[i..i + 3], &[120, 120, 200]);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn neutralise_grey_stays_grey_darkened_by_exposure() {
        // iter113: grey is still grey, but the multiplicative
        // exposure (0.30) darkens it — luminance is NO LONGER
        // preserved (that is the point: a bright sample must land
        // near lum ~35 so the panel fills ~(10,10,11)-class at
        // fa 0.80). Unlike iter103's subtractive offset, a mid
        // grey keeps its level instead of collapsing to black.
        assert_eq!(neutralise_sample([100, 100, 100]), [30, 30, 30]);
        assert_eq!(neutralise_sample([0, 0, 0]), [0, 0, 0]);
        assert_eq!(neutralise_sample([255, 255, 255]), [77, 77, 77]);
        assert_eq!(neutralise_sample([200, 200, 200]), [60, 60, 60]);
    }

    #[test]
    fn neutralise_warm_cloud_desaturates_and_darkens() {
        // Warm cloud [210, 180, 150]: spread 60 -> ~3 (strength
        // floor 0.88), luminance ~186 -> ~55 (exposure 0.30).
        let n = neutralise_sample([210, 180, 150]);
        let spread = n[0].max(n[1]).max(n[2]) - n[0].min(n[1]).min(n[2]);
        assert!(spread <= 10, "spread {spread} still warm: {n:?}");
        let lum = 0.299 * n[0] as f32 + 0.587 * n[1] as f32 + 0.114 * n[2] as f32;
        assert!((lum - 55.5).abs() < 3.0, "luminance moved to {lum}");
    }

    #[test]
    fn neutralise_dark_warm_pulls_harder() {
        // Dark warm [90, 60, 40]: desaturated hard, then the
        // exposure keeps it at ~20 instead of collapsing it to
        // the floor — iter113: a dark sample keeps its structure
        // so the frost stays visible over a dark backdrop.
        let n = neutralise_sample([90, 60, 40]);
        assert_eq!(n, [21, 20, 19]);
    }

    #[test]
    fn neutralise_bright_sky_hits_reference_mean_iter103() {
        // iter103 (iter113: exposure 0.30 instead of offset 100):
        // the v0.43 bright-leg sky samples (gap-implied live
        // samples) must neutralise to lum ~35-42, B-R ~4, so the
        // shared fill at fa 0.80 lands the panel in the reference's
        // (10,10,11)-class band with B-R ~+1 (see main.rs
        // frost_fill_px_blur_live_neutral_sky_iter103 for the fill).
        for (bg, want) in [
            ([56, 133, 187], [33, 35, 37]),
            ([62, 142, 196], [35, 38, 40]),
            ([81, 158, 210], [40, 43, 45]),
        ] {
            let n = neutralise_sample(bg);
            for c in 0..3 {
                assert!((n[c] as i32 - want[c] as i32).abs() <= 1,
                    "neutralise {bg:?} -> {n:?}, want ~{want:?}");
            }
            assert!((n[2] as i32 - n[0] as i32) <= 17, "B-R in {n:?}");
        }
        // Dark sample: keeps its level (~2) instead of collapsing
        // to the floor — the dark leg keeps its structure.
        assert_eq!(neutralise_sample([8, 8, 10]), [2, 2, 3]);
    }

    #[test]
    fn factor_for_lum_mapping() {
        assert_eq!(factor_for_lum(0.0), 0.0);
        assert_eq!(factor_for_lum(15.0), 0.0);
        assert!((factor_for_lum(45.0) - 0.5).abs() < 1e-6);
        assert_eq!(factor_for_lum(75.0), 1.0);
        assert_eq!(factor_for_lum(255.0), 1.0);
    }

    #[test]
    fn apply_backdrop_factor_scales_to_black() {
        assert_eq!(apply_backdrop_factor([190, 180, 170], 0.0), [0, 0, 0]);
        assert_eq!(apply_backdrop_factor([190, 180, 170], 1.0), [190, 180, 170]);
        assert_eq!(apply_backdrop_factor([200, 100, 50], 0.5), [100, 50, 25]);
    }

    #[test]
    fn sample_frost_bright_matches_neutralised() {
        // Default probe luminance is bright (255) -> factor 1.
        let mut data = vec![0u8; LOGICAL_W * LOGICAL_H * 3];
        for px in data.chunks_exact_mut(3) {
            px[0] = 210;
            px[1] = 180;
            px[2] = 150;
        }
        let bw = BlurredWallpaper { data };
        assert_eq!(bw.sample_frost(752.0, 27.0), neutralise_sample([210, 180, 150]));
    }

    #[test]
    fn decode_jpeg_roundtrip_fixture() {
        // Tiny 4x3 JPEG fixture (same pixel pattern as the PNG
        // roundtrip test: (x*40, y*60, 200), q95 4:4:4). JPEG is
        // lossy, so values are asserted within +-8 (PIL decodes them
        // within +-4 of source; zune-jpeg's IDCT may differ by a
        // couple more levels).
        let dir = std::env::temp_dir().join("iter88-jpeg-fixture");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("tiny-4x3.jpg");
        std::fs::write(&path, include_bytes!("test-assets/tiny-4x3.jpg")).unwrap();
        let (rgb, w, h) =
            decode_jpeg_to_logical(&path).expect("decode our own JPEG fixture");
        assert_eq!((w, h), (LOGICAL_W, LOGICAL_H));
        assert_eq!(rgb.len(), LOGICAL_W * LOGICAL_H * 3);
        let close = |got: &[u8], want: [u8; 3]| {
            got.iter()
                .zip(want.iter())
                .all(|(g, w)| (*g as i32 - *w as i32).abs() <= 8)
        };
        assert!(close(&rgb[0..3], [0, 0, 200]), "corner {:?}", &rgb[0..3]);
        let i = (1000 * LOGICAL_W + 1500) * 3;
        assert!(
            close(&rgb[i..i + 3], [120, 120, 200]),
            "far corner {:?}",
            &rgb[i..i + 3]
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_and_blur_accepts_jpeg_named_png() {
        // The exact iter88 regression: a JPEG in a `.png`-named file
        // (what `library::apply_wallpaper` produces) must load, not
        // fall back to the flat fill — and an unknown format must
        // still fall back (file_format -> None).
        let dir = std::env::temp_dir().join("iter88-jpeg-named-png");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("wallpaper.png");
        std::fs::write(&path, include_bytes!("test-assets/tiny-4x3.jpg")).unwrap();
        assert_eq!(file_format(&path), Some(ImgFormat::Jpeg));
        let bw = load_and_blur(&path).expect("JPEG bytes in wallpaper.png must load");
        assert_eq!(bw.data.len(), LOGICAL_W * LOGICAL_H * 3);
        let bad = dir.join("wallpaper.bin");
        std::fs::write(&bad, b"not an image at all").unwrap();
        assert_eq!(file_format(&bad), None);
        assert!(load_and_blur(&bad).is_none());
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&bad);
    }
}
