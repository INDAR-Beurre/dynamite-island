//! Iteration 108 — system-tray host (StatusNotifierWatcher) for the island.
//!
//! The island itself becomes the tray host: it owns
//! `org.kde.StatusNotifierWatcher` on the session bus, accepts
//! `RegisterStatusNotifierItem` from any app (current or later — no
//! fixed list, no restart), and renders every item's icon at pill
//! height on the RIGHT of the pill, mirroring the workspace circles
//! on the left (icon ~16 logical, gaps ~4, 4 px gap after the pill's
//! right edge; the cluster grows rightward).
//!
//! Protocol (freedesktop StatusNotifierItem spec, KDE extensions):
//! * Watcher object at `/StatusNotifierWatcher`, interface
//!   `org.kde.StatusNotifierWatcher`: `RegisterStatusNotifierItem`,
//!   properties `RegisteredStatusNotifierItems` /
//!   `IsStatusNotifierHostRegistered` / `ProtocolVersion`.
//! * Item interface `org.kde.StatusNotifierItem` at the item's object
//!   path (`/StatusNotifierItem` for service-form registrations, the
//!   given path for path-form ones): properties `Id`, `Title`,
//!   `Status`, `IconName`, `IconPixmap` (`a(iiay)` ARGB32), `Menu`.
//! * Methods: `Activate(x, y)`, `ContextMenu(x, y)`,
//!   `Scroll(delta, orientation)`.
//! * DBusMenu (`org.freedesktop.DBusMenu.GetLayout`) renders the
//!   app's menu as the island's TrayMenu panel; row click sends
//!   `Event(id, "clicked", ...)`. Left click always Activates first
//!   (the app shows its own mini dashboard window); right click
//!   opens the DBusMenu panel when the item exposes one, else calls
//!   `ContextMenu` on the item.
//!
//! Threading (the island's hard rule: NO blocking D-Bus call on the
//! render/input path): one background tray thread owns the zbus
//! blocking connection. The watcher interface methods running on
//! zbus's executor thread only push `(service, sender)` pairs into a
//! Mutex queue and return. The worker thread drains the queue,
//! fetches properties/icons (theme lookup incl. rsvg-convert for SVG
//! IconNames, IconPixmap ARGB decode), sweeps `ListNames` every 1 s
//! to drop dead items, and executes UI-thread commands (Activate /
//! ContextMenu / Scroll / MenuEvent / FetchMenu) received over an
//! mpsc channel — each command runs in a short-lived thread joined
//! with a 3 s timeout so a hung app can stall the worker, never the
//! island. The UI thread only ever clones the snapshot
//! (`RwLock`, small) and sends commands; the 1 s calloop timer
//! redraws when `fingerprint()` moves (same pattern as the iter91 CC
//! / iter106 Music fingerprints).
//!
//! Verified before integration (spike `~/tray-spike`, 15 Nov 2026):
//! zbus 5.19.0 + zvariant 5.15.0 resolve and build `--offline` from
//! the Surface cargo cache; the full handshake works against the
//! live session bus (Register -> props -> Activate/ContextMenu ->
//! GetLayout); `#[zbus(header)]` is REQUIRED on the Header param
//! (without it dispatch fails silently); GetLayout must deserialize
//! into `OwnedValue` (borrowed `Value` fails DynamicDeserialize);
//! watcher idle PSS ~2.9 MB. Live finding: `magpie-desktop`
//! (:1.591) registers its item with the path form
//! (`/StatusNotifierItem` on its unique name) the moment a watcher
//! name appears — the island gains a real first tray icon for free.

use std::collections::HashMap;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::Duration;

use zbus::{blocking::Connection, interface, message::Header};

// ---------------------------------------------------------------------------
// Geometry — OWNED BY main.rs (iter108-C, merge): tray_icon_centres /
// tray_cluster_rect / tray_hit_stage + the TRAY_* constants live there
// as the single source of truth (pill-anchored stage geometry, incl.
// the 4 px bridge inside the cluster rect and the circular +1 px hit
// test). This file must never duplicate them: draw_tray_cluster_stage
// below calls crate::tray_icon_centres, and the hit/region callers in
// main.rs call crate::tray_hit_stage / crate::tray_cluster_rect.
// (Merge note: B's own pill-right-parameterised copies were removed —
// same icon centres, but B's cluster rect omitted the bridge and its
// hit test was square, not circular.)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Snapshot — what the UI thread reads (clone of small data only).
// ---------------------------------------------------------------------------

/// One tray item, as the island draws it. `icon` is 16x16 RGBA8
/// straight alpha (decoded IconPixmap, or IconName theme lookup);
/// None = no usable icon (drawn as a placeholder disc + initial).
#[derive(Clone, Debug)]
pub struct TrayItemSnapshot {
    pub service: String,
    pub path: String,
    pub id: String,
    pub title: String,
    pub status: String,
    pub icon: Option<Vec<u8>>,
    /// Native 32x32 variant for the 2x top row (F-G). The old path
    /// box-averaged the app's 44x44 IconPixmap down to 16 px and then
    /// upscaled it, which is why the tray glyph read as a muddy blob.
    pub icon32: Option<Vec<u8>>,
    pub icon_name: String,
    pub has_menu: bool,
    pub menu_path: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct TrayState {
    items: Vec<TrayItemSnapshot>,
    /// Bumped on every real change (register / unregister / icon /
    /// title / status) — the 1 s timer redraws when it moves.
    seq: u64,
}

static TRAY: OnceLock<RwLock<TrayState>> = OnceLock::new();
fn tray_state() -> &'static RwLock<TrayState> {
    TRAY.get_or_init(|| RwLock::new(TrayState::default()))
}

/// Clone the current item list (UI thread: draw + hit-test + clicks).
pub fn snapshot() -> Vec<TrayItemSnapshot> {
    tray_state().read().unwrap().items.clone()
}

/// Item count without cloning icons (input region / hit-test).
pub fn snapshot_count() -> usize {
    tray_state().read().unwrap().items.len()
}

// NOTE (merge iter108): cluster_rect / hit_stage lived here in B's
// tree but are gone — main.rs owns the cluster rect
// (tray_cluster_rect) and the hit test (tray_hit_stage) over the
// shared count store now.

#[cfg(test)]
pub fn set_menu_for_test(title: &str, rows: Vec<MenuRow>) {
    let mut m = tray_menu().write().unwrap();
    m.service = "test".to_string();
    m.menu_path = "/Menu".to_string();
    m.title = title.to_string();
    m.rows = rows;
    m.seq += 1;
}

/// Fingerprint for the 1 s timer: seq + service list + menu seq.
/// Cheap (one RwLock read, no D-Bus).
pub fn fingerprint() -> String {
    let st = tray_state().read().unwrap();
    let menu_seq = tray_menu().read().unwrap().seq;
    let mut s = format!("seq={} menu={} [", st.seq, menu_seq);
    for it in &st.items {
        s.push_str(&format!("{}:{}:{}|", it.service, it.id, it.icon.is_some()));
    }
    s.push(']');
    s
}

#[cfg(test)]
pub fn set_snapshot_for_test(items: Vec<TrayItemSnapshot>) {
    let mut st = tray_state().write().unwrap();
    st.items = items;
    st.seq += 1;
}

// ---------------------------------------------------------------------------
// Commands — UI thread -> tray worker thread (mpsc, never blocking).
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum TrayCmd {
    Activate { service: String, path: String },
    ContextMenu { service: String, path: String },
    Scroll { service: String, path: String, delta: i32, orientation: String },
    MenuEvent { service: String, menu_path: String, id: i32 },
    FetchMenu { service: String, menu_path: String, title: String },
}

static TRAY_CMD_TX: OnceLock<Mutex<std::sync::mpsc::Sender<TrayCmd>>> = OnceLock::new();

fn send_cmd(cmd: TrayCmd) -> bool {
    if let Some(tx) = TRAY_CMD_TX.get() {
        tx.lock().unwrap().send(cmd).is_ok()
    } else {
        false
    }
}

/// Left click on item `i`: Activate (the app shows its own mini
/// dashboard window). Fire-and-forget from the UI thread.
pub fn activate_index(i: usize) -> bool {
    let Some(it) = snapshot().into_iter().nth(i) else {
        return false;
    };
    eprintln!("dynamite-rust: tray activate {} ({})", it.id, it.service);
    send_cmd(TrayCmd::Activate { service: it.service, path: it.path })
}

/// Scroll over item `i`: forward as Scroll(delta, "vertical").
pub fn scroll_index(i: usize, delta: i32) -> bool {
    let Some(it) = snapshot().into_iter().nth(i) else {
        return false;
    };
    send_cmd(TrayCmd::Scroll {
        service: it.service,
        path: it.path,
        delta,
        orientation: "vertical".to_string(),
    })
}

/// What a right click on item `i` does: open the island's DBusMenu
/// panel when the item exposes a menu, else call ContextMenu on the
/// item (the app shows its own menu window).
pub enum ContextAction {
    OpenMenu,
    ContextMenu,
}

pub fn context_index(i: usize) -> Option<ContextAction> {
    let it = snapshot().into_iter().nth(i)?;
    match (&it.has_menu, &it.menu_path) {
        (true, Some(menu_path)) => {
            send_cmd(TrayCmd::FetchMenu {
                service: it.service.clone(),
                menu_path: menu_path.clone(),
                title: it.title.clone(),
            });
            Some(ContextAction::OpenMenu)
        }
        _ => {
            send_cmd(TrayCmd::ContextMenu { service: it.service, path: it.path });
            Some(ContextAction::ContextMenu)
        }
    }
}

/// Click on DBusMenu row `id` in the open TrayMenu panel: send
/// Event(id, "clicked") to the item.
pub fn menu_event(id: i32) -> bool {
    let m = tray_menu().read().unwrap();
    if m.service.is_empty() || m.menu_path.is_empty() {
        return false;
    }
    send_cmd(TrayCmd::MenuEvent {
        service: m.service.clone(),
        menu_path: m.menu_path.clone(),
        id,
    })
}

// ---------------------------------------------------------------------------
// DBusMenu panel state (open menu: service + rows + seq).
// ---------------------------------------------------------------------------

/// One rendered menu row. `depth` 0 = top level; children are
/// flattened one level deeper (indented). Separators and disabled
/// rows are drawn but never clickable.
#[derive(Clone, Debug)]
pub struct MenuRow {
    pub id: i32,
    pub label: String,
    pub enabled: bool,
    pub separator: bool,
    pub depth: usize,
}

#[derive(Clone, Debug, Default)]
struct TrayMenuState {
    service: String,
    menu_path: String,
    title: String,
    rows: Vec<MenuRow>,
    seq: u64,
}

static TRAY_MENU: OnceLock<RwLock<TrayMenuState>> = OnceLock::new();
fn tray_menu() -> &'static RwLock<TrayMenuState> {
    TRAY_MENU.get_or_init(|| RwLock::new(TrayMenuState::default()))
}

pub fn open_menu_snapshot() -> (String, Vec<MenuRow>) {
    let m = tray_menu().read().unwrap();
    (m.title.clone(), m.rows.clone())
}

/// TrayMenu panel size: fixed 404 wide; height follows the open
/// menu's row count (title + rows), clamped to a sane range.
/// Default (no menu yet / loading) is the title-only height.
pub fn tray_menu_size() -> (u32, u32) {
    let rows = tray_menu().read().unwrap().rows.len();
    let h = 16 + 28 + rows as u32 * 28 + 16;
    (404, h.clamp(96, 480))
}

/// Hit-test a panel-local point against the open menu's rows:
/// Some(row id) for an enabled, non-separator row; None otherwise.
pub fn menu_row_hit(px: f32, py: f32) -> Option<i32> {
    let m = tray_menu().read().unwrap();
    for (i, row) in m.rows.iter().enumerate() {
        let (x, y, w, h) = (14.0, 44.0 + i as f32 * 28.0, 404.0 - 28.0, 28.0);
        if row.enabled && !row.separator && px >= x && px < x + w && py >= y && py < y + h {
            return Some(row.id);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Icon decoding (pure — unit-tested).
// ---------------------------------------------------------------------------

/// Pick the pixmap to render: the largest one no bigger than 64 px
/// on a side (tray icons are small; a 512 px pixmap is wasted RAM
/// and downscale time); if every pixmap is bigger, the smallest of
/// them. None when the list is empty or all entries are degenerate.
pub fn pick_pixmap(pixmaps: &[(i32, i32, Vec<u8>)]) -> Option<&(i32, i32, Vec<u8>)> {
    let mut best: Option<&(i32, i32, Vec<u8>)> = None;
    for p in pixmaps {
        if p.0 <= 0 || p.1 <= 0 || p.2.len() < (p.0 * p.1 * 4) as usize {
            continue;
        }
        let fits = p.0 <= 64 && p.1 <= 64;
        best = match best {
            None => Some(p),
            Some(b) => {
                let b_fits = b.0 <= 64 && b.1 <= 64;
                if fits && !b_fits {
                    Some(p) // a fitting pixmap beats any oversize one
                } else if fits != b_fits {
                    Some(b)
                } else if fits && (p.0 * p.1) > (b.0 * b.1) {
                    Some(p) // both fit: the larger
                } else if !fits && (p.0 * p.1) < (b.0 * b.1) {
                    Some(p) // both oversize: the smaller
                } else {
                    Some(b)
                }
            }
        };
    }
    best
}

/// Convert one ARGB32 pixmap (D-Bus IconPixmap: big-endian ARGB32 —
/// the wire bytes for one pixel are A,R,G,B in memory order, per the
/// SNI spec and as emitted by real apps) to 16x16 RGBA8 straight
/// alpha. Sources already 16x16 convert directly; anything else is
/// box-averaged down.
pub fn pixmap_to_rgba16(p: &(i32, i32, Vec<u8>)) -> Option<Vec<u8>> {
    pixmap_to_rgba(p, 16)
}

/// 32x32 variant for the 2x top row (F-G).
pub fn pixmap_to_rgba32(p: &(i32, i32, Vec<u8>)) -> Option<Vec<u8>> {
    pixmap_to_rgba(p, 32)
}

fn pixmap_to_rgba(p: &(i32, i32, Vec<u8>), n: usize) -> Option<Vec<u8>> {
    let (w, h) = (p.0 as usize, p.1 as usize);
    if w == 0 || h == 0 || p.2.len() < w * h * 4 {
        return None;
    }
    // First to straight RGBA at native size.
    let mut rgba = vec![0u8; w * h * 4];
    for i in 0..w * h {
        let v = u32::from_be_bytes([p.2[i * 4], p.2[i * 4 + 1], p.2[i * 4 + 2], p.2[i * 4 + 3]]);
        let (a, r, g, b) = (
            ((v >> 24) & 0xff) as u8,
            ((v >> 16) & 0xff) as u8,
            ((v >> 8) & 0xff) as u8,
            (v & 0xff) as u8,
        );
        rgba[i * 4] = r;
        rgba[i * 4 + 1] = g;
        rgba[i * 4 + 2] = b;
        rgba[i * 4 + 3] = a;
    }
    if w == n && h == n {
        return Some(rgba);
    }
    Some(downscale_rgba(&rgba, w, h, n, n))
}

/// Box-average downscale of straight-alpha RGBA8 to (dw, dh).
/// Nearest-sample when the source is already smaller.
pub fn downscale_rgba(src: &[u8], w: usize, h: usize, dw: usize, dh: usize) -> Vec<u8> {
    let mut out = vec![0u8; dw * dh * 4];
    for dy in 0..dh {
        for dx in 0..dw {
            let (x0, x1) = (dx * w / dw, ((dx + 1) * w / dw).max(dx * w / dw + 1));
            let (y0, y1) = (dy * h / dh, ((dy + 1) * h / dh).max(dy * h / dh + 1));
            let mut acc = [0u32; 4];
            let mut n = 0u32;
            for sy in y0..y1.min(h) {
                for sx in x0..x1.min(w) {
                    let si = (sy * w + sx) * 4;
                    for c in 0..4 {
                        acc[c] += src[si + c] as u32;
                    }
                    n += 1;
                }
            }
            let oi = (dy * dw + dx) * 4;
            for c in 0..4 {
                out[oi + c] = if n > 0 { (acc[c] / n) as u8 } else { 0 };
            }
        }
    }
    out
}

/// Decode PNG bytes to straight RGBA8 (any size). Palette /
/// grayscale / 16-bit sources are expanded by the png crate's
/// EXPAND transform; RGB sources get an opaque alpha. None on any
/// decode failure.
pub fn decode_png_bytes(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width, info.height);
    // Bytes per pixel depend on the TRANSFORMED colour type: Rgba
    // is 4, Rgb is 3 — slicing w*h*4 unconditionally would over-read
    // (and panic) on RGB sources.
    let bpp = match info.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        _ => return None,
    };
    let px = &buf[..(w * h * bpp) as usize];
    match info.color_type {
        png::ColorType::Rgba => Some((w, h, px.to_vec())),
        png::ColorType::Rgb => {
            let mut rgba = vec![0u8; (w * h * 4) as usize];
            for i in 0..(w * h) as usize {
                rgba[i * 4] = px[i * 3];
                rgba[i * 4 + 1] = px[i * 3 + 1];
                rgba[i * 4 + 2] = px[i * 3 + 2];
                rgba[i * 4 + 3] = 255;
            }
            Some((w, h, rgba))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// IconName theme lookup (tray worker thread only; cached).
// ---------------------------------------------------------------------------

/// Candidate files for an IconName, in lookup priority order:
/// absolute path as-is; else Adwaita then hicolor, sizes
/// 16/22/24/32/48 then scalable, under the standard icon dirs,
/// plus /usr/share/pixmaps. Mirrors the iter86 launcher rule
/// (Adwaita per gtk-icon-theme-name, hicolor fallback).
pub fn theme_icon_candidates(name: &str) -> Vec<PathBuf> {
    if name.starts_with('/') {
        return vec![PathBuf::from(name)];
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut bases: Vec<PathBuf> = vec![
        PathBuf::from("/usr/share/icons"),
        PathBuf::from("/usr/local/share/icons"),
        PathBuf::from("/usr/share/pixmaps"),
    ];
    if let Some(h) = &home {
        bases.insert(0, h.join(".local/share/icons"));
        bases.push(h.join(".icons"));
    }
    let mut out = Vec::new();
    for theme in ["Adwaita", "hicolor"] {
        for size in ["16x16", "22x22", "24x24", "32x32", "48x48", "scalable"] {
            for base in &bases {
                if base.ends_with("pixmaps") {
                    continue; // pixmaps has no theme/size layout
                }
                for ext in ["png", "svg"] {
                    out.push(base.join(theme).join(size).join("apps").join(format!("{name}.{ext}")));
                }
            }
        }
    }
    for base in &bases {
        for ext in ["png", "svg"] {
            out.push(base.join(format!("{name}.{ext}")));
        }
    }
    out
}

/// Run a command with a hard timeout, capturing stdout bytes
/// (binary-safe — unlike the main.rs mmsg helper, which returns a
/// lossy String). Used for rsvg-convert only.
fn run_cmd_bytes_timeout(args: &[&str], timeout: Duration) -> Option<Vec<u8>> {
    let mut child = std::process::Command::new(args[0])
        .args(&args[1..])
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
                return rx.recv_timeout(Duration::from_millis(200)).ok();
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(_) => return None,
        }
    }
}

/// Resolve an IconName to 16x16 RGBA8: first usable candidate wins
/// (PNG decoded directly; SVG rasterized via rsvg-convert 16x16,
/// the same tool the iter86 extraction used). `cache` is the
/// worker thread's own map (negative results cached too).
pub fn lookup_theme_icon(
    name: &str,
    cache: &mut HashMap<String, Option<Vec<u8>>>,
) -> Option<Vec<u8>> {
    if name.is_empty() {
        return None;
    }
    if let Some(hit) = cache.get(name) {
        return hit.clone();
    }
    let mut found = None;
    for path in theme_icon_candidates(name) {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext == "png" {
            if let Ok(bytes) = std::fs::read(&path) {
                if let Some((w, h, rgba)) = decode_png_bytes(&bytes) {
                    found = Some(if w == 16 && h == 16 {
                        rgba
                    } else {
                        downscale_rgba(&rgba, w as usize, h as usize, 16, 16)
                    });
                    break;
                }
            }
        } else if ext == "svg" {
            if let Some(png_bytes) = run_cmd_bytes_timeout(
                &[
                    "rsvg-convert",
                    "-w",
                    "16",
                    "-h",
                    "16",
                    path.to_str()?,
                ],
                Duration::from_secs(2),
            ) {
                if let Some((w, h, rgba)) = decode_png_bytes(&png_bytes) {
                    found = Some(if w == 16 && h == 16 {
                        rgba
                    } else {
                        downscale_rgba(&rgba, w as usize, h as usize, 16, 16)
                    });
                    break;
                }
            }
        }
    }
    cache.insert(name.to_string(), found.clone());
    found
}

// ---------------------------------------------------------------------------
// DBusMenu layout parsing (pure — unit-tested).
// ---------------------------------------------------------------------------

fn owned_string(v: &zvariant::OwnedValue) -> Option<String> {
    match &**v {
        zvariant::Value::Str(s) => Some(s.to_string()),
        _ => None,
    }
}

fn owned_bool(v: &zvariant::OwnedValue) -> Option<bool> {
    match &**v {
        zvariant::Value::Bool(b) => Some(*b),
        _ => None,
    }
}

/// Parse one GetLayout node `(id, props, children)` into rows,
/// flattening children one level deeper (indented). Rows whose
/// `type` is `separator`, or with an empty label and no children,
/// become separator rows. Unparseable children are skipped, never
/// fatal — a half-parsed menu still renders.
fn parse_menu_node(
    node: &(i32, HashMap<String, zvariant::OwnedValue>, Vec<zvariant::OwnedValue>),
    depth: usize,
    rows: &mut Vec<MenuRow>,
) {
    let (id, props, children) = node;
    let label = props.get("label").and_then(owned_string).unwrap_or_default();
    let enabled = props.get("enabled").and_then(owned_bool).unwrap_or(true);
    let visible = props.get("visible").and_then(owned_bool).unwrap_or(true);
    let is_sep = props
        .get("type")
        .and_then(owned_string)
        .map(|t| t == "separator")
        .unwrap_or(false)
        || (label.is_empty() && children.is_empty());
    // The root node (id 0) is the menu itself, not a row.
    if *id != 0 && visible {
        rows.push(MenuRow { id: *id, label, enabled, separator: is_sep, depth });
    }
    if *id == 0 || !is_sep {
        for child in children {
            if let Ok(node) = child.downcast_ref::<(
                i32,
                HashMap<String, zvariant::OwnedValue>,
                Vec<zvariant::OwnedValue>,
            )>() {
                parse_menu_node(&node, if *id == 0 { 0 } else { depth + 1 }, rows);
            }
        }
    }
}

pub fn parse_menu_layout(
    layout: &(i32, HashMap<String, zvariant::OwnedValue>, Vec<zvariant::OwnedValue>),
) -> Vec<MenuRow> {
    let mut rows = Vec::new();
    parse_menu_node(layout, 0, &mut rows);
    rows
}

// ---------------------------------------------------------------------------
// Watcher object + worker thread.
// ---------------------------------------------------------------------------

/// A registration as it arrives on the watcher object: the raw
/// argument plus the sender's unique name. Path-form arguments
/// (`/StatusNotifierItem`, the form real apps like magpie-desktop
/// use) resolve against the sender; service-form arguments carry
/// the service name and imply path `/StatusNotifierItem`.
struct PendingReg {
    arg: String,
    sender: String,
}

struct Watcher {
    pending: Arc<Mutex<Vec<PendingReg>>>,
}

#[interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    fn register_status_notifier_item(
        &mut self,
        service: &str,
        #[zbus(header)] hdr: Header<'_>,
    ) {
        // Runs on zbus's executor thread: record only, never block.
        let sender = hdr
            .sender()
            .map(|u| u.to_string())
            .unwrap_or_default();
        eprintln!(
            "dynamite-rust: tray register {service} sender={sender}"
        );
        self.pending.lock().unwrap().push(PendingReg {
            arg: service.to_string(),
            sender,
        });
    }

    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        snapshot().iter().map(|i| i.service.clone()).collect()
    }

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        0
    }
}

/// Fetch one item's properties + icon and insert it into the
/// snapshot. A fetch failure (item died mid-handshake, no such
/// object) drops the registration silently — never a phantom icon.
fn register_item(
    conn: &Connection,
    service: &str,
    path: &str,
    icon_cache: &mut HashMap<String, Option<Vec<u8>>>,
) {
    let proxy = match zbus::blocking::Proxy::new(
        conn,
        service,
        path,
        "org.kde.StatusNotifierItem",
    ) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("dynamite-rust: tray proxy for {service} failed: {e}");
            return;
        }
    };
    let id: String = proxy.get_property("Id").unwrap_or_default();
    let title: String = proxy.get_property("Title").unwrap_or_default();
    let status: String = proxy.get_property("Status").unwrap_or_default();
    // Menu is D-Bus type `o` (object path), not `s` — a String
    // get_property would fail against real (Qt/GTK) items.
    let menu: String = proxy
        .get_property::<zvariant::ObjectPath>("Menu")
        .map(|o| o.to_string())
        .unwrap_or_default();
    // Icon: IconPixmap first (authoritative), IconName lookup second.
    let mut icon = None;
    let mut icon32 = None;
    if let Ok(pixmaps) = proxy.get_property::<Vec<(i32, i32, Vec<u8>)>>("IconPixmap") {
        if let Some(p) = pick_pixmap(&pixmaps) {
            icon = pixmap_to_rgba16(p);
            icon32 = pixmap_to_rgba32(p);
        }
    }
    let icon_name: String = proxy.get_property("IconName").unwrap_or_default();
    if icon.is_none() && !icon_name.is_empty() {
        icon = lookup_theme_icon(&icon_name, icon_cache);
    }
    // An item with neither id nor title nor icon is not provably an
    // item (e.g. a stale registration) — drop it; the next sweep
    // would drop it anyway.
    if id.is_empty() && title.is_empty() && icon.is_none() {
        eprintln!("dynamite-rust: tray ignoring empty item {service} {path}");
        return;
    }
    let has_menu = !menu.is_empty() && menu != "/";
    let item = TrayItemSnapshot {
        service: service.to_string(),
        path: path.to_string(),
        id,
        title,
        status,
        icon,
        icon32,
        icon_name,
        has_menu,
        menu_path: if has_menu { Some(menu) } else { None },
    };
    let mut st = tray_state().write().unwrap();
    if let Some(old) = st.items.iter_mut().find(|i| i.service == service && i.path == path) {
        *old = item;
    } else {
        st.items.push(item);
    }
    st.seq += 1;
}

/// Run one D-Bus command in a short-lived thread joined with a
/// 3 s timeout, so a hung app stalls the worker briefly, never the
/// island. The abandoned thread (if any) just exits when its call
/// returns; the connection is Arc-shared.
fn run_cmd_guarded<F>(conn: &Connection, f: F)
where
    F: FnOnce(&Connection) + Send + 'static,
{
    let conn = conn.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        f(&conn);
        let _ = tx.send(());
    });
    let _ = rx.recv_timeout(Duration::from_secs(3));
}

fn call_item_method(conn: &Connection, service: &str, path: &str, method: &str) {
    let service = service.to_string();
    let path = path.to_string();
    let method = method.to_string();
    run_cmd_guarded(conn, move |c| {
        if let Ok(p) = zbus::blocking::Proxy::new(
            c,
            service.as_str(),
            path.as_str(),
            "org.kde.StatusNotifierItem",
        ) {
            let _: Result<(), zbus::Error> = p.call(method.as_str(), &(0i32, 0i32));
        }
    });
}

fn handle_cmd(conn: &Connection, cmd: TrayCmd) {
    match cmd {
        TrayCmd::Activate { service, path } => {
            call_item_method(conn, &service, &path, "Activate")
        }
        TrayCmd::ContextMenu { service, path } => {
            call_item_method(conn, &service, &path, "ContextMenu")
        }
        TrayCmd::Scroll { service, path, delta, orientation } => {
            let service = service.clone();
            let path = path.clone();
            run_cmd_guarded(conn, move |c| {
                if let Ok(p) = zbus::blocking::Proxy::new(
                    c,
                    service.as_str(),
                    path.as_str(),
                    "org.kde.StatusNotifierItem",
                ) {
                    let _: Result<(), zbus::Error> =
                        p.call("Scroll", &(delta, orientation.as_str()));
                }
            });
        }
        TrayCmd::MenuEvent { service, menu_path, id } => {
            let service = service.clone();
            let menu_path = menu_path.clone();
            run_cmd_guarded(conn, move |c| {
                if let Ok(p) = zbus::blocking::Proxy::new(
                    c,
                    service.as_str(),
                    menu_path.as_str(),
                    "org.freedesktop.DBusMenu",
                ) {
                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs() as u32)
                        .unwrap_or(0);
                    let _: Result<(), zbus::Error> = p.call(
                        "Event",
                        &(id, "clicked", zvariant::Value::new(""), ts),
                    );
                }
            });
        }
        TrayCmd::FetchMenu { service, menu_path, title } => {
            let svc = service.clone();
            let mp = menu_path.clone();
            run_cmd_guarded(conn, move |c| {
                if let Ok(p) = zbus::blocking::Proxy::new(
                    c,
                    svc.as_str(),
                    mp.as_str(),
                    "org.freedesktop.DBusMenu",
                ) {
                    let layout: Result<
                        (
                            u32,
                            (
                                i32,
                                HashMap<String, zvariant::OwnedValue>,
                                Vec<zvariant::OwnedValue>,
                            ),
                        ),
                        zbus::Error,
                    > = p.call("GetLayout", &(0i32, -1i32, Vec::<String>::new()));
                    if let Ok((_rev, root)) = layout {
                        let rows = parse_menu_layout(&root);
                        let mut m = tray_menu().write().unwrap();
                        // Clone: the proxy above still borrows svc/mp.
                        m.service = svc.clone();
                        m.menu_path = mp.clone();
                        m.title = title;
                        m.rows = rows;
                        m.seq += 1;
                    }
                }
            });
        }
    }
}

/// One connected run of the tray worker: own the watcher name,
/// serve registrations, execute commands, sweep every 1 s.
/// Returns Err only on a connection failure (caller retries);
/// a taken watcher name exits the thread permanently (another
/// host owns the tray — never steal it).
fn tray_run(
    pending: Arc<Mutex<Vec<PendingReg>>>,
    rx: &std::sync::mpsc::Receiver<TrayCmd>,
) -> Result<(), String> {
    let conn = Connection::session().map_err(|e| format!("session bus: {e}"))?;
    conn.object_server()
        .at(
            "/StatusNotifierWatcher",
            Watcher { pending: pending.clone() },
        )
        .map_err(|e| format!("serve watcher: {e}"))?;
    if let Err(e) = conn.request_name("org.kde.StatusNotifierWatcher") {
        eprintln!("dynamite-rust: tray watcher name unavailable ({e}) — another host owns the tray, tray thread exiting");
        return Ok(());
    }
    eprintln!("dynamite-rust: tray host live (org.kde.StatusNotifierWatcher owned)");
    let dbus = zbus::blocking::Proxy::new(
        &conn,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
    )
    .map_err(|e| format!("dbus proxy: {e}"))?;
    let mut icon_cache: HashMap<String, Option<Vec<u8>>> = HashMap::new();
    let mut last_sweep = std::time::Instant::now();
    loop {
        // 1) Drain pending registrations (normalise + fetch).
        let regs: Vec<PendingReg> = {
            let mut p = pending.lock().unwrap();
            std::mem::take(&mut *p)
        };
        let n_regs = regs.len();
        for reg in regs {
            let (service, path) = if let Some(stripped) = reg.arg.strip_prefix('/') {
                // Path form: the item lives at this path on the
                // sender's unique name (magpie-desktop does this).
                (reg.sender.clone(), format!("/{stripped}"))
            } else {
                (reg.arg.clone(), "/StatusNotifierItem".to_string())
            };
            if service.is_empty() {
                continue;
            }
            register_item(&conn, &service, &path, &mut icon_cache);
        }
        // iter109 RAM: a registration transient fetch/draw buffers land on
        // the heap and stay as arena slack (measured +2.1-3.2 MB per real-app
        // registration, 0 released on unregister). Return the freed pages now;
        // registrations are rare (app start), so this is off the hot path.
        if n_regs > 0 {
            unsafe { libc::malloc_trim(0) };
        }
        // 2) One command (200 ms wait keeps the sweep cadence ~1 s).
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(cmd) => handle_cmd(&conn, cmd),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                // The sender lives in a OnceLock and is never dropped
                // while the process runs; treat as fatal.
                return Err("tray command channel closed".to_string());
            }
        }
        // 3) 1 s sweep: drop dead items, refresh light properties.
        // A failed ListNames skips the sweep — an empty default
        // would wipe every live item (a transient bus error must
        // never look like "all items quit").
        if last_sweep.elapsed() >= Duration::from_secs(1) {
            last_sweep = std::time::Instant::now();
            let Ok(names): Result<Vec<String>, zbus::Error> =
                dbus.call("ListNames", &())
            else {
                continue;
            };
            let live: std::collections::HashSet<&str> =
                names.iter().map(|s| s.as_str()).collect();
            let mut st = tray_state().write().unwrap();
            let before = st.items.len();
            st.items.retain(|i| live.contains(i.service.as_str()));
            if st.items.len() != before {
                st.seq += 1;
            }
            let services: Vec<(String, String, String)> = st
                .items
                .iter()
                .map(|i| (i.service.clone(), i.path.clone(), i.icon_name.clone()))
                .collect();
            drop(st);
            for (service, path, old_icon_name) in services {
                let Ok(p) = zbus::blocking::Proxy::new(
                    &conn,
                    service.as_str(),
                    path.as_str(),
                    "org.kde.StatusNotifierItem",
                ) else {
                    continue;
                };
                // Fetch everything BEFORE taking the snapshot lock:
                // a blocking property call must never hold the lock
                // the UI thread reads for every draw.
                let status: Result<String, _> = p.get_property("Status");
                let title: Result<String, _> = p.get_property("Title");
                let icon_name: Result<String, _> = p.get_property("IconName");
                let mut new_icon: Option<(Option<Vec<u8>>, Option<Vec<u8>>)> = None;
                if let Ok(n) = &icon_name {
                    if *n != old_icon_name {
                        let mut icon = None;
                        let mut icon32 = None;
                        if let Ok(pix) =
                            p.get_property::<Vec<(i32, i32, Vec<u8>)>>("IconPixmap")
                        {
                            if let Some(px) = pick_pixmap(&pix) {
                                icon = pixmap_to_rgba16(px);
                                icon32 = pixmap_to_rgba32(px);
                            }
                        }
                        if icon.is_none() && !n.is_empty() {
                            icon = lookup_theme_icon(n, &mut icon_cache);
                        }
                        new_icon = Some((icon, icon32));
                    }
                }
                let mut st = tray_state().write().unwrap();
                if let Some(it) = st
                    .items
                    .iter_mut()
                    .find(|i| i.service == service && i.path == path)
                {
                    let mut changed = false;
                    if let Ok(s) = status {
                        if s != it.status {
                            it.status = s;
                            changed = true;
                        }
                    }
                    if let Ok(t) = title {
                        if t != it.title {
                            it.title = t;
                            changed = true;
                        }
                    }
                    if let Ok(n) = icon_name {
                        if n != old_icon_name {
                            it.icon_name = n;
                            changed = true;
                        }
                    }
                    if let Some((icon, icon32)) = new_icon {
                        it.icon = icon;
                        it.icon32 = icon32;
                        changed = true;
                    }
                    if changed {
                        st.seq += 1;
                    }
                }
            }
        }
    }
}

/// Spawn the tray host thread once (idempotent; safe to call from
/// draw() / the 1 s timer, like ensure_tag_poller). The command
/// channel is created here, so it survives connection retries —
/// the worker thread owns the receiver across tray_run calls.
/// The thread retries the session-bus connection forever (5 s
/// backoff); a taken watcher name exits it permanently with a log
/// line.
pub fn ensure_tray_host() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<TrayCmd>();
        let _ = TRAY_CMD_TX.set(Mutex::new(tx));
        let pending = Arc::new(Mutex::new(Vec::<PendingReg>::new()));
        std::thread::spawn(move || loop {
            match tray_run(pending.clone(), &rx) {
                Ok(()) => return, // name taken (or clean exit): stop
                Err(e) => {
                    eprintln!("dynamite-rust: tray host error ({e}); retrying in 5 s");
                    std::thread::sleep(Duration::from_secs(5));
                }
            }
        });
    });
}

// ---------------------------------------------------------------------------
// Drawing — stage cluster + TrayMenu panel (UI thread, snapshot only).
// Canvas is BGRA premultiplied (Format::Argb8888), same as main.rs.
// ---------------------------------------------------------------------------

/// Blit a 16x16 straight-alpha RGBA icon onto the stage canvas at
/// (x0, y0): same alpha-over convention as
/// launcher_icons::blit_icon (source RGB, dest BGRA, A forced 255).
fn blit_rgba16(canvas: &mut [u8], w: u32, h: u32, x0: i32, y0: i32, rgba: &[u8]) {
    debug_assert_eq!(rgba.len(), 16 * 16 * 4);
    for sy in 0..16i32 {
        let y = y0 + sy;
        if y < 0 || y >= h as i32 {
            continue;
        }
        for sx in 0..16i32 {
            let x = x0 + sx;
            if x < 0 || x >= w as i32 {
                continue;
            }
            let si = (sy as usize * 16 + sx as usize) * 4;
            let sa = rgba[si + 3] as f32 / 255.0;
            if sa <= 0.0 {
                continue;
            }
            let i = ((y as u32) * w + x as u32) as usize * 4;
            if i + 3 >= canvas.len() {
                continue;
            }
            let inv = 1.0 - sa;
            canvas[i] = (rgba[si + 2] as f32 * sa + canvas[i] as f32 * inv) as u8;
            canvas[i + 1] = (rgba[si + 1] as f32 * sa + canvas[i + 1] as f32 * inv) as u8;
            canvas[i + 2] = (rgba[si] as f32 * sa + canvas[i + 2] as f32 * inv) as u8;
            canvas[i + 3] = 255;
        }
    }
}

/// Placeholder for an item with no usable icon: a Ø16 dark disc in
/// the workspace-disc material (fill #1C1E22 at 0.92) with the
/// item's first letter centred (near-white, 11 px — the same size
/// as the workspace numbers).
fn draw_placeholder_disc(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    cx: f32,
    cy: f32,
    font: &fontdue::Font,
    label: &str,
) {
    let (fr, fg, fb) = (28u8, 30u8, 34u8);
    let x0 = ((cx - 8.0 - 1.0).floor() as i32).max(0);
    let x1 = ((cx + 8.0 + 1.0).ceil() as i32).min(w as i32);
    let y0 = ((cy - 8.0 - 1.0).floor() as i32).max(0);
    let y1 = ((cy + 8.0 + 1.0).ceil() as i32).min(h as i32);
    for y in y0..y1 {
        for x in x0..x1 {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
            let a = (8.0 + 0.5 - d).clamp(0.0, 1.0) * 0.92;
            if a > 0.0 {
                let i = (((y as u32 * w) + x as u32) * 4) as usize;
                let inv = 1.0 - a;
                canvas[i] = (fb as f32 * a + canvas[i] as f32 * inv) as u8;
                canvas[i + 1] = (fg as f32 * a + canvas[i + 1] as f32 * inv) as u8;
                canvas[i + 2] = (fr as f32 * a + canvas[i + 2] as f32 * inv) as u8;
                canvas[i + 3] = 255;
            }
        }
    }
    let initial: String = label.chars().next().unwrap_or('?').to_uppercase().collect();
    let row = crate::layout_text(font, &initial, 11.0);
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
                canvas[i] = (237.0 * cc + canvas[i] as f32 * inv) as u8;
                canvas[i + 1] = (234.0 * cc + canvas[i + 1] as f32 * inv) as u8;
                canvas[i + 2] = (232.0 * cc + canvas[i + 2] as f32 * inv) as u8;
                canvas[i + 3] = 255;
            }
        }
        pen += gm.advance_width;
    }
}

/// Flat two-tone composite of a square RGBA icon at device
/// resolution (F-G): the source colour is discarded, alpha becomes
/// coverage, and one near-white ink is laid over the background. The
/// tray glyph then matches the clock's monochrome treatment instead of
/// introducing a second, painted look.
fn blit_rgba_flat(
    canvas: &mut [u8],
    bw: u32,
    bh: u32,
    x0: i32,
    y0: i32,
    rgba: &[u8],
    n: usize,
) {
    const INK: (f32, f32, f32) = (232.0, 234.0, 237.0);
    if rgba.len() < n * n * 4 {
        return;
    }
    for sy in 0..n as i32 {
        let y = y0 + sy;
        if y < 0 || y >= bh as i32 {
            continue;
        }
        for sx in 0..n as i32 {
            let x = x0 + sx;
            if x < 0 || x >= bw as i32 {
                continue;
            }
            let si = (sy as usize * n + sx as usize) * 4;
            let sa = rgba[si + 3] as f32 / 255.0;
            if sa <= 0.0 {
                continue;
            }
            let i = ((y as u32 * bw + x as u32) * 4) as usize;
            let inv = 1.0 - sa;
            canvas[i] = (INK.2 * sa + canvas[i] as f32 * inv) as u8;
            canvas[i + 1] = (INK.1 * sa + canvas[i + 1] as f32 * inv) as u8;
            canvas[i + 2] = (INK.0 * sa + canvas[i + 2] as f32 * inv) as u8;
            canvas[i + 3] = 255;
        }
    }
}

/// 16 px theme icon scaled to device resolution by nearest sample —
/// only used when a theme lookup (not a native IconPixmap) supplied
/// the icon.
fn blit_rgba16_scaled(canvas: &mut [u8], bw: u32, bh: u32, x0: i32, y0: i32, rgba: &[u8], s: f32) {
    if rgba.len() < 16 * 16 * 4 {
        return;
    }
    let n = (16.0 * s).round() as i32;
    for dy in 0..n {
        for dx in 0..n {
            let sx = ((dx as f32 / s) as usize).min(15);
            let sy = ((dy as f32 / s) as usize).min(15);
            let si = (sy * 16 + sx) * 4;
            let sa = rgba[si + 3] as f32 / 255.0;
            if sa <= 0.0 {
                continue;
            }
            let x = x0 + dx;
            let y = y0 + dy;
            if x < 0 || y < 0 || x >= bw as i32 || y >= bh as i32 {
                continue;
            }
            let i = ((y as u32 * bw + x as u32) * 4) as usize;
            let inv = 1.0 - sa;
            canvas[i] = (232.0 * sa + canvas[i] as f32 * inv) as u8;
            canvas[i + 1] = (234.0 * sa + canvas[i + 1] as f32 * inv) as u8;
            canvas[i + 2] = (237.0 * sa + canvas[i + 2] as f32 * inv) as u8;
            canvas[i + 3] = 255;
        }
    }
}

/// Placeholder at device resolution: flat Ø16 disc (no shadow, no rim)
/// with the item's initial, same treatment as the workspace numbers.
fn draw_placeholder_disc_scaled(
    canvas: &mut [u8],
    bw: u32,
    bh: u32,
    cx: f32,
    cy: f32,
    font: &fontdue::Font,
    label: &str,
    s: f32,
) {
    let r = 8.0 * s;
    let (fr, fg, fb) = (28u8, 30u8, 34u8);
    let x0 = ((cx - r - 1.0).floor() as i32).max(0);
    let x1 = ((cx + r + 1.0).ceil() as i32).min(bw as i32);
    let y0 = ((cy - r - 1.0).floor() as i32).max(0);
    let y1 = ((cy + r + 1.0).ceil() as i32).min(bh as i32);
    for y in y0..y1 {
        for x in x0..x1 {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
            let a = (r + 0.5 - d).clamp(0.0, 1.0);
            if a <= 0.0 {
                continue;
            }
            let i = ((y as u32 * bw + x as u32) * 4) as usize;
            let inv = 1.0 - a;
            canvas[i] = (fb as f32 * a + canvas[i] as f32 * inv) as u8;
            canvas[i + 1] = (fg as f32 * a + canvas[i + 1] as f32 * inv) as u8;
            canvas[i + 2] = (fr as f32 * a + canvas[i + 2] as f32 * inv) as u8;
            canvas[i + 3] = ((a + canvas[i + 3] as f32 / 255.0 * inv) * 255.0) as u8;
        }
    }
    let initial: String = label.chars().next().unwrap_or('?').to_uppercase().collect();
    let row = crate::layout_text(font, &initial, 11.0 * s);
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
                let i = ((px_y as u32 * bw + px_x as u32) * 4) as usize;
                let inv = 1.0 - cc;
                canvas[i] = (237.0 * cc + canvas[i] as f32 * inv) as u8;
                canvas[i + 1] = (234.0 * cc + canvas[i + 1] as f32 * inv) as u8;
                canvas[i + 2] = (232.0 * cc + canvas[i + 2] as f32 * inv) as u8;
                canvas[i + 3] = 255;
            }
        }
        pen += gm.advance_width;
    }
}

/// Device-resolution tray cluster for the settled top row (F-G): the
/// same centres as draw_tray_cluster_stage, at `scale` device px per
/// logical px, with the native 32x32 IconPixmap when available.
pub fn draw_tray_cluster_2x(canvas: &mut [u8], bw: u32, bh: u32, font: &fontdue::Font, scale: f32) {
    let items = snapshot();
    if items.is_empty() {
        return;
    }
    for (i, (cx, cy)) in crate::tray_icon_centres(items.len()).iter().enumerate() {
        let it = &items[i];
        let (dcx, dcy) = (cx * scale, cy * scale);
        let (x0, y0) = (
            (cx - 8.0).round() as i32 * 2,
            (cy - 8.0).round() as i32 * 2,
        );
        match (&it.icon32, &it.icon) {
            (Some(rgba), _) => blit_rgba_flat(canvas, bw, bh, x0, y0, rgba, 32),
            (None, Some(rgba)) => blit_rgba16_scaled(canvas, bw, bh, x0, y0, rgba, scale),
            (None, None) => draw_placeholder_disc_scaled(
                canvas,
                bw,
                bh,
                dcx,
                dcy,
                font,
                if it.title.is_empty() { &it.id } else { &it.title },
                scale,
            ),
        }
    }
}

/// Draw the tray cluster into the stage canvas (called from
/// draw(), after the workspace circles, in every non-lockscreen
/// state). Empty snapshot = nothing drawn.
pub fn draw_tray_cluster_stage(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &fontdue::Font,
) {
    let items = snapshot();
    if items.is_empty() {
        return;
    }
    // Centres come from main.rs (crate::tray_icon_centres, iter108-C).
    for (i, (cx, cy)) in crate::tray_icon_centres(items.len()).iter().enumerate() {
        let it = &items[i];
        match &it.icon {
            Some(rgba) => blit_rgba16(
                canvas,
                w,
                h,
                (cx - 8.0).round() as i32,
                (cy - 8.0).round() as i32,
                rgba,
            ),
            None => draw_placeholder_disc(
                canvas,
                w,
                h,
                *cx,
                *cy,
                font,
                if it.title.is_empty() { &it.id } else { &it.title },
            ),
        }
    }
}

/// Draw the open DBusMenu panel content (panel-local coords; the
/// capsule background is drawn by draw_panel_into first). Title
/// row + one 28 px row per menu entry (indented by depth);
/// separators are 1 px lines; disabled rows are dimmed. A menu
/// still loading shows the title with no rows yet.
pub fn draw_tray_menu_panel(canvas: &mut [u8], w: u32, h: u32, font: &fontdue::Font) {
    let (title, rows) = open_menu_snapshot();
    let title = if title.is_empty() { "Tray menu".to_string() } else { title };
    draw_menu_text(canvas, w, h, font, &title, 14.0, 16.0, 15.0, (232, 234, 237));
    for (i, row) in rows.iter().enumerate() {
        let y = 44.0 + i as f32 * 28.0;
        if row.separator {
            let sy = (y + 13.0).round() as i32;
            if sy >= 0 && (sy as u32) < h {
                for x in 14..(w as i32 - 14) {
                    if x < 0 || x >= w as i32 {
                        continue;
                    }
                    let idx = ((sy as u32) * w + x as u32) as usize * 4;
                    if idx + 3 < canvas.len() {
                        canvas[idx] = 60;
                        canvas[idx + 1] = 60;
                        canvas[idx + 2] = 60;
                        canvas[idx + 3] = 255;
                    }
                }
            }
            continue;
        }
        let rgb = if row.enabled { (232, 234, 237) } else { (110, 112, 116) };
        let x = 14.0 + row.depth as f32 * 14.0;
        draw_menu_text(canvas, w, h, font, &row.label, x, y, 14.0, rgb);
    }
}

fn draw_menu_text(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &fontdue::Font,
    text: &str,
    x: f32,
    y: f32,
    px: f32,
    rgb: (u8, u8, u8),
) {
    let row = crate::layout_text(font, text, px);
    let baseline = y + row.ascent;
    let mut pen = x;
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
                let idx = (((px_y as u32) * w + px_x as u32) * 4) as usize;
                let inv = 1.0 - cc;
                canvas[idx] = (rgb.2 as f32 * cc + canvas[idx] as f32 * inv) as u8;
                canvas[idx + 1] = (rgb.1 as f32 * cc + canvas[idx + 1] as f32 * inv) as u8;
                canvas[idx + 2] = (rgb.0 as f32 * cc + canvas[idx + 2] as f32 * inv) as u8;
                canvas[idx + 3] = 255;
            }
        }
        pen += gm.advance_width;
    }
}

// ---------------------------------------------------------------------------
// Tests.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Test fixture: emits the SNI wire format — big-endian ARGB32,
    // bytes A,R,G,B in memory order per pixel. Do NOT "fix" this to
    // to_le_bytes(): the old LE fixture is what baked the decoder bug
    // into the test (iter111: magenta rendered as teal on the live island).
    fn pixmap_argb(w: i32, h: i32, argb: u32) -> (i32, i32, Vec<u8>) {
        let mut bytes = Vec::with_capacity((w * h * 4) as usize);
        for _ in 0..w * h {
            bytes.extend_from_slice(&argb.to_be_bytes());
        }
        (w, h, bytes)
    }

    #[test]
    fn geometry_mirrors_workspace_cluster() {
        // Pill 550..626 (live collapsed 76 wide). First icon centre
        // 638 = 626 + 4 + 8; step 20; cluster rect starts at 626
        // (merge iter108: C's rect includes the 4 px bridge to the
        // pill — B's old (630, 7, 56, 16) omitted it). Centres/rect
        // come from main.rs (iter108-C), the single source of truth.
        let cy = crate::ws_cy();
        let centres = crate::tray_icon_centres(3);
        assert_eq!(
            centres,
            vec![(638.0, cy), (658.0, cy), (678.0, cy)]
        );
        let rect = crate::tray_cluster_rect(3).unwrap();
        assert_eq!(rect, (626.0, cy - 8.0, 60.0, 16.0));
        assert_eq!(crate::tray_cluster_rect(0), None);
        // The cluster sits inside the reserved strip (pill_h tall, from
        // STAGE_PAD_TOP) like the workspace cluster.
        let strip = crate::STAGE_PAD_TOP + crate::pill_h();
        assert!(rect.1 >= crate::STAGE_PAD_TOP && rect.1 + rect.3 <= strip);
    }

    #[test]
    fn hit_test_circular_with_slack() {
        // Merge iter108: the hit test is C's circular one (Ø16 + 1 px
        // slack, like the workspace circles), via main.rs — B's old
        // square test is re-pointed at it. Every assertion below still
        // holds: all eight points are on-axis, where square and
        // circular agree (the corner case is what differs, and the
        // circular form is the specified one).
        let cy = crate::ws_cy();
                let cy = crate::ws_cy();
        let cy64 = cy as f64;
assert_eq!(crate::tray_hit_stage((638.0, cy64), 2), Some(0));
        assert_eq!(crate::tray_hit_stage((658.0, cy64), 2), Some(1));
        assert_eq!(crate::tray_hit_stage((647.0, cy64), 2), Some(0)); // +9 slack edge
        assert_eq!(crate::tray_hit_stage((648.0, cy64), 2), None); // gap between icons
        assert_eq!(crate::tray_hit_stage((700.0, cy64), 2), None);
        assert_eq!(crate::tray_hit_stage((638.0, cy64), 0), None);
        assert_eq!(crate::tray_hit_stage((638.0, cy64 + 9.0), 2), Some(0)); // slack edge, inclusive
        assert_eq!(crate::tray_hit_stage((638.0, cy64 + 10.0), 2), None); // below icon+slack
    }

    #[test]
    fn pixmap_pick_prefers_largest_fitting() {
        let small = pixmap_argb(16, 16, 0xFF000000);
        let big = pixmap_argb(48, 48, 0xFF000000);
        let huge = pixmap_argb(128, 128, 0xFF000000);
        let list = vec![small.clone(), big.clone(), huge.clone()];
        let picked = pick_pixmap(&list).unwrap();
        assert_eq!((picked.0, picked.1), (48, 48));
        // All oversize: smallest wins.
        let oversize = vec![huge.clone(), pixmap_argb(256, 256, 0xFF000000)];
        let picked = pick_pixmap(&oversize).unwrap();
        assert_eq!((picked.0, picked.1), (128, 128));
        assert!(pick_pixmap(&[]).is_none());
        assert!(pick_pixmap(&[(0, 0, vec![])]).is_none());
    }

    #[test]
    fn argb_to_rgba16_exact_channels() {
        // Opaque red ARGB 0xFFFF0000 -> RGBA (255, 0, 0, 255).
        let p = pixmap_argb(16, 16, 0xFFFF0000);
        let rgba = pixmap_to_rgba16(&p).unwrap();
        assert_eq!(rgba.len(), 16 * 16 * 4);
        assert_eq!(&rgba[0..4], &[255, 0, 0, 255]);
        // Semi-transparent green-blue: A=0x80, R=0x10, G=0x20, B=0x30.
        let p = pixmap_argb(16, 16, 0x80102030);
        let rgba = pixmap_to_rgba16(&p).unwrap();
        assert_eq!(&rgba[0..4], &[0x10, 0x20, 0x30, 0x80]);
    }

    #[test]
    fn downscale_box_average() {
        // 2x2 -> 1x1: mean of (255,0,0,255), (0,255,0,255),
        // (0,0,255,255), (255,255,255,255) = (127,127,127,255).
        let src: Vec<u8> = vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ];
        let out = downscale_rgba(&src, 2, 2, 1, 1);
        assert_eq!(out, vec![127, 127, 127, 255]);
    }

    #[test]
    fn png_roundtrip_decode() {
        // Encode a 2x2 RGBA PNG with the png crate, decode it back.
        let mut buf = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut buf, 2, 2);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().unwrap();
            w.write_image_data(&[
                10, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 255, 100, 110, 120, 255,
            ])
            .unwrap();
            w.finish().unwrap();
        }
        let (w, h, rgba) = decode_png_bytes(&buf).unwrap();
        assert_eq!((w, h), (2, 2));
        assert_eq!(&rgba[0..4], &[10, 20, 30, 255]);
        assert_eq!(&rgba[12..16], &[100, 110, 120, 255]);
        assert!(decode_png_bytes(b"not a png").is_none());
    }

    #[test]
    fn theme_candidates_order() {
        let c = theme_icon_candidates("firefox");
        // Adwaita 16x16 PNG first, hicolor later, pixmaps last.
        assert!(c[0].to_str().unwrap().contains("Adwaita/16x16/apps/firefox.png"));
        assert!(c.iter().any(|p| p.to_str().unwrap().contains("hicolor/scalable/apps/firefox.svg")));
        assert!(c.iter().any(|p| p.to_str().unwrap().ends_with("pixmaps/firefox.png")));
        // Absolute path passes through untouched.
        assert_eq!(
            theme_icon_candidates("/opt/app/icon.png"),
            vec![PathBuf::from("/opt/app/icon.png")]
        );
    }

    fn owned_str(s: &str) -> zvariant::OwnedValue {
        zvariant::Value::new(s.to_string()).try_to_owned().unwrap()
    }

    fn owned_bool(b: bool) -> zvariant::OwnedValue {
        zvariant::Value::new(b).try_to_owned().unwrap()
    }

    fn menu_value(
        id: i32,
        label: &str,
        enabled: bool,
        sep: bool,
        kids: Vec<zvariant::OwnedValue>,
    ) -> zvariant::OwnedValue {
        let mut props: HashMap<String, zvariant::OwnedValue> = HashMap::new();
        props.insert("label".to_string(), owned_str(label));
        props.insert("enabled".to_string(), owned_bool(enabled));
        props.insert("visible".to_string(), owned_bool(true));
        if sep {
            props.insert("type".to_string(), owned_str("separator"));
        }
        zvariant::Value::new((id, props, kids)).try_to_owned().unwrap()
    }

    fn layout_root(
        kids: Vec<zvariant::OwnedValue>,
    ) -> (
        i32,
        HashMap<String, zvariant::OwnedValue>,
        Vec<zvariant::OwnedValue>,
    ) {
        let mut root_props = HashMap::new();
        root_props.insert("label".to_string(), owned_str("Spike"));
        (0, root_props, kids)
    }

    #[test]
    fn menu_layout_parses_rows_and_separators() {
        let kids = vec![
            menu_value(1, "Open dashboard", true, false, vec![]),
            menu_value(2, "", true, true, vec![]),
            menu_value(3, "Quit", false, false, vec![]),
        ];
        let root = layout_root(kids);
        let rows = parse_menu_layout(&root);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].label, "Open dashboard");
        assert!(rows[0].enabled && !rows[0].separator);
        assert!(rows[1].separator);
        assert_eq!(rows[2].label, "Quit");
        assert!(!rows[2].enabled);
    }

    #[test]
    fn menu_layout_flattens_children() {
        let sub = menu_value(11, "Sub item", true, false, vec![]);
        let kids = vec![menu_value(10, "Parent", true, false, vec![sub])];
        let root = layout_root(kids);
        let rows = parse_menu_layout(&root);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].depth, 0);
        assert_eq!(rows[1].depth, 1);
        assert_eq!(rows[1].label, "Sub item");
    }
}
