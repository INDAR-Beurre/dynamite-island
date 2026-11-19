//! clipboard_panel — Clipboard view (item a9, iter39) for dynamite-rust.
//!
//! A faithful Rust port of `analysis/clipboard-manager-original.py`
//! (Alexandre's Python clipboard manager; that file stays on disk
//! untouched, and `cliphist` stays the data backend). Behaviour copied
//! exactly where the script defines it:
//!   * history = `cliphist list`, newest first, capped at 100 entries;
//!   * text entries: whitespace-collapsed, truncated to 90 chars
//!     (87 + "..."); single-line rows;
//!   * image entries (`[[ binary data ... ]]`): label `Bild (<details>)`
//!     with `<details>` = the script's bracket-stripped content;
//!   * selection: `cliphist decode` (fed the FULL original line on
//!     stdin, exactly like the script) piped to `wl-copy` — images
//!     with the MIME sniffed exactly like the script (PNG default;
//!     `\xff\xd8` -> image/jpeg; `RIFF....WEBP` -> image/webp);
//!   * empty history: the script's "Keine Einträge vorhanden!" —
//!     shown as an honest empty state INSIDE the panel (no notify-send
//!     from the island; the island never spawns notifications).
//!
//! Differences from the script, all deliberate and documented:
//!   * No rofi, no ffmpeg, no notify-send: the island IS the picker
//!     (launcher-like list view, type-to-search, arrows + Enter/click).
//!   * No 64x64 ffmpeg thumbnails: image thumbnails stream rows
//!     through the `png` crate (iter85 RAM fix — one reusable row
//!     buffer, area-averaged straight into the 32x32 thumb, never a
//!     full-res buffer) and are downscaled to 32x32, aspect
//!     preserved. PNG is the only format decoded — screenshots
//!     pasted via grim/wl-copy are PNG.
//!     JPEG/WebP images still list, select and copy correctly — they
//!     just get a procedural image placeholder tile instead of a
//!     real thumbnail. That is an honest fallback, not a silent lie.
//!   * Text entries get a procedural document glyph in the thumb slot
//!     (the script generated a 64x64 SVG->PNG text icon via ffmpeg;
//!     same colours/shapes, drawn directly).
//!
//! THREADING / HOT PATH (hard constraint: nothing blocking on the
//! render/input path, <=100 MB PSS):
//!   * `cliphist list` + every `cliphist decode` + every PNG decode
//!     run in a background std::thread (`refresh_async`), never on the
//!     event loop. The draw path only reads the Mutex-guarded cache.
//!   * Selection also copies in a fire-and-forget thread; the island
//!     collapses immediately, it does not wait for wl-copy.
//!   * Cache bounds: <=100 entries; thumbnails for at most the first
//!     12 image entries per refresh (visible window is 8 rows);
//!     each thumb is 32x32x4 = 4 KB, so <=48 KB of thumbs total.
//!     Entry labels are <=~100 chars; the whole cache is tens of KB.
//!   * A refresh already running is never duplicated (flag in cache).
//!     First entry after daemon start uses the startup-warmed cache
//!     (main.rs calls `refresh_async()` once at startup — see the
//!     wiring report); a stale cache is still shown while a refresh
//!     runs, and the entry timer in main.rs redraws ~250 ms after
//!     entry (resizing the panel if the row count changed),
//!     re-arming every 250 ms until the refresh lands (iter88:
//!     a single shot fires before the thread finishes — the morph
//!     ends after anim_ms and nothing else redraws while idle in
//!     Clipboard — so the fresh list stayed stale until the next
//!     input event).
//!
//! WIRING (integrator — see iterations/builder-iter39-clipboard.md
//! for the full diffs; `main.rs` is untouched by this file):
//! ```ignore
//! // main.rs: mod clipboard_panel;
//! // IslandState::Clipboard, from_word "clipboard", size() =>
//! //   clipboard_panel::panel_size(), MENU_CYCLE += Clipboard
//! //   (at the END, after Power — the Exclusive grab follows
//! //   automatically, since sync_keyboard_interactivity keys off
//! //   MENU_CYCLE membership), draw() arm =>
//! //   clipboard_panel::draw_clipboard_panel(...),
//! // press_key dispatches on_clipboard_key, classify_press maps
//! //   row hits to PressAction::ClipRow, begin_morph /
//! //   apply_state_instant call clipboard_panel::on_enter().
//! ```
//!
//! TYPECHECK HONESTY: written on a VM with NO Rust toolchain
//! (`which cargo rustc` -> nothing), so this file has NOT been
//! typechecked or test-run here. The integrator MUST run
//! `cargo test --offline` on the Surface before wiring it in; the
//! pure-logic tests below are dependency-free (std only) so they run
//! with no font files and no cliphist/wl-copy present.

use fontdue::Font;
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// Constants — geometry family shared with launcher_panel.rs
// (504 wide; search line 14..50; rows 42 px, pitch 47.5, first row
// top 74; bottom pad 12.5). The panel height is content-sized:
// H(n) = 74 + (n-1)*47.5 + 42 + 12.5 (n = visible rows, 1..=8);
// n = 6 gives 366, exactly the launcher height.
// ---------------------------------------------------------------------------
pub const PANEL_W: u32 = 504;
pub const MAX_ENTRIES: usize = 100;
/// Max rows drawn at once (launcher caps at 6; clipboard shows 8 —
/// single-line rows, no second text line, so 8 still fit a 461 px
/// panel; more entries are reachable through the search filter).
pub const MAX_VISIBLE_ROWS: usize = 8;
/// Thumbnail slot: 32x32, aspect-preserved fit, centred.
pub const THUMB_SIZE: usize = 32;
/// Max image entries thumbnailed per refresh (visible window + margin).
pub const MAX_THUMBS_PER_REFRESH: usize = 12;
/// Label truncation: the script's 90 chars (87 + "..."), CHAR-based
/// (Python len() counts chars — so does this, never bytes).
pub const MAX_LABEL_CHARS: usize = 90;

pub const C_PRIMARY: (u8, u8, u8) = (255, 255, 255);
pub const C_SECONDARY: (u8, u8, u8) = (192, 196, 201); // #C0C4C9
pub const C_HIGHLIGHT: (u8, u8, u8) = (5, 22, 60); // #05163C, selected row - muted periwinkle tint at the old lift's lightness (anchor #7CA2F5), iter96
/// Scroll thumb (iter39 a10): subtle dark grey, same as the Launcher.
pub const C_SCROLLBAR: (u8, u8, u8) = (64, 66, 70); // #404246
/// Empty-state / placeholder tile colours (music-panel family).
pub const C_TILE_FILL: (u8, u8, u8) = (20, 20, 20); // #141414
pub const C_TILE_BORDER: (u8, u8, u8) = (46, 46, 46); // #2E2E2E
pub const C_TILE_GLYPH: (u8, u8, u8) = (74, 74, 74); // #4A4A4A
/// Text-icon colours, copied from the script's inline SVG.
pub const C_DOC_FILL: (u8, u8, u8) = (32, 33, 56); // #202138
pub const C_DOC_STROKE: (u8, u8, u8) = (122, 162, 247); // #7aa2f7
pub const C_DOC_LINE1: (u8, u8, u8) = (187, 154, 247); // #bb9af7
pub const C_DOC_LINE2: (u8, u8, u8) = (125, 207, 255); // #7dcfff
pub const C_DOC_LINE3: (u8, u8, u8) = (224, 228, 252); // #e0e4fc

const SEARCH_Y: f32 = 14.0;
const SEARCH_H: f32 = 36.0;
const SEARCH_TEXT_X: f32 = 49.0;
const SEARCH_TEXT_TOP: f32 = 24.0;
const MAGNIFIER_CX: f32 = 32.0;
const MAGNIFIER_CY: f32 = SEARCH_Y + SEARCH_H / 2.0; // 32
const ROW_X: f32 = 14.0;
const ROW_W: f32 = PANEL_W as f32 - 2.0 * ROW_X; // 476
const ROW_Y0: f32 = 74.0;
const ROW_H: f32 = 42.0;
const ROW_PITCH: f32 = 47.5;
const ROW_R: f32 = 9.0;
const THUMB_X: f32 = 24.0;
const TEXT_X: f32 = 66.0; // thumb right (56) + 10
const TEXT_MAX_W: f32 = PANEL_W as f32 - ROW_X - TEXT_X; // 424
const EMPTY_PANEL_H: u32 = 200;
const EMPTY_MSG_TOP: f32 = 110.0;

// ---------------------------------------------------------------------------
// Pure parsing / labelling — exactly the script's semantics.
// ---------------------------------------------------------------------------
/// One history entry. `raw_line` is the FULL `cliphist list` line
/// (`<id>\t<content>`); `cliphist decode` must be fed exactly this
/// line on stdin, like the script does.
#[derive(Clone, Debug)]
pub struct ClipEntry {
    pub id: String,
    pub raw_line: String,
    pub label: String,
    pub is_image: bool,
    /// 32x32 RGBA thumbnail (straight alpha) for decodable PNGs;
    /// None for text entries and for images that did not decode.
    pub thumb: Option<Vec<u8>>,
}

/// Parse one `cliphist list` line. Mirrors the script:
/// `parts = l.split("\t", 1)`; missing tab => content "".
pub fn parse_cliphist_line(line: &str) -> ClipEntry {
    let (id, content) = match line.split_once('\t') {
        Some((id, content)) => (id, content),
        None => (line, ""),
    };
    let is_image = content.starts_with("[[ binary data");
    let label = if is_image {
        image_label(content)
    } else {
        text_label(content)
    };
    ClipEntry {
        id: id.to_string(),
        raw_line: line.to_string(),
        label,
        is_image,
        thumb: None,
    }
}

/// Parse a whole `cliphist list` stdout, newest first, capped at 100
/// (the script: `[l for l in stdout.strip().split("\n") if l][:100]`).
pub fn parse_cliphist_list(stdout: &str) -> Vec<ClipEntry> {
    stdout
        .lines()
        .filter(|l| !l.is_empty())
        .take(MAX_ENTRIES)
        .map(parse_cliphist_line)
        .collect()
}

/// Text label: whitespace-collapse (`" ".join(content.split())`)
/// then truncate to 90 chars (87 + "..."). Char-based, like Python.
pub fn text_label(content: &str) -> String {
    let clean = content.split_whitespace().collect::<Vec<&str>>().join(" ");
    truncate_chars(&clean, MAX_LABEL_CHARS)
}

/// Image label: `Bild (<details>)`, details = content with the
/// script's two replaces applied in order.
pub fn image_label(content: &str) -> String {
    let details = content
        .replace("[[ binary data ", "")
        .replace(" ]]", "");
    format!("Bild ({details})")
}

/// Truncate to `max_chars` chars, appending "..." when cut, so the
/// result is at most `max_chars` chars long. `max_chars` must be >= 3.
pub fn truncate_chars(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        return s.to_string();
    }
    let keep = max_chars.saturating_sub(3);
    let mut out: String = s.chars().take(keep).collect();
    out.push_str("...");
    out
}

/// MIME sniff, exactly the script's order: PNG default, JPEG magic,
/// then RIFF....WEBP. Anything else (incl. empty / short input)
/// stays image/png.
pub fn sniff_mime(img_bytes: &[u8]) -> &'static str {
    if img_bytes.len() >= 2 && img_bytes[0] == 0xff && img_bytes[1] == 0xd8 {
        "image/jpeg"
    } else if img_bytes.len() >= 12
        && &img_bytes[0..4] == b"RIFF"
        && &img_bytes[8..12] == b"WEBP"
    {
        "image/webp"
    } else {
        "image/png"
    }
}

/// Panel height for `n` visible rows (0 => empty-state panel).
/// n = 6 -> 366 (launcher height); n = 8 -> 461; n = 1 -> 129.
pub fn panel_height_for(visible_rows: usize) -> u32 {
    if visible_rows == 0 {
        return EMPTY_PANEL_H;
    }
    let n = visible_rows as f32;
    (ROW_Y0 + (n - 1.0) * ROW_PITCH + ROW_H + 12.5).round() as u32
}

// ---------------------------------------------------------------------------
// Live state — one Mutex, same pattern as launcher_panel::LIVE.
// Every caller is on the island's single event-loop thread except
// the refresh thread, which only ever writes through this Mutex.
// ---------------------------------------------------------------------------
struct ClipboardCache {
    entries: Vec<ClipEntry>,
    query: String,
    selected: usize,
    /// First visible row: the drawn window is `offset .. offset +
    /// MAX_VISIBLE_ROWS` of the filtered list. Follows the selection
    /// (see selection.rs); 0 until the selection scrolls past row 7.
    /// Iter39 a10 — without this, selected >= 8 has no visible
    /// highlight (the draw only renders the first 8 filtered rows).
    offset: usize,
    /// A refresh thread is running; a second one is never spawned.
    refreshing: bool,
}

static CACHE: Mutex<ClipboardCache> = Mutex::new(ClipboardCache {
    entries: Vec::new(),
    query: String::new(),
    selected: 0,
    offset: 0,
    refreshing: false,
});

fn with_cache<R>(f: impl FnOnce(&mut ClipboardCache) -> R) -> R {
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut guard)
}

/// Entries matching the current query (case-insensitive substring
/// on the label, launcher `filtered()` model). Empty query returns
/// all entries, in history order (newest first), capped at the
/// visible window by the caller. Private on purpose: the cache
/// type itself is private, so a `pub` here would only buy a
/// `private_interfaces` warning.
fn filtered_indices(cache: &ClipboardCache) -> Vec<usize> {
    let q = cache.query.trim().to_lowercase();
    cache
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| q.is_empty() || e.label.to_lowercase().contains(&q))
        .map(|(i, _)| i)
        .collect()
}

/// Number of rows actually drawn (filtered, capped at 8).
pub fn visible_row_count() -> usize {
    with_cache(|c| filtered_indices(c).len().min(MAX_VISIBLE_ROWS))
}

/// Panel size for the CURRENT cache (dynamic height — read at
/// state-entry time by `IslandState::size()`; the 250 ms entry
/// timer in main.rs re-applies it if a refresh lands mid-morph).
pub fn panel_size() -> (u32, u32) {
    (PANEL_W, panel_height_for(visible_row_count()))
}

/// Entry hook: fresh search + fresh history. Called from
/// begin_morph / apply_state_instant when Clipboard is entered.
pub fn on_enter() {
    with_cache(|c| {
        c.query.clear();
        c.selected = 0;
        c.offset = 0;
    });
    refresh_async();
}

/// True while a refresh thread is running (set by `refresh_async`,
/// cleared when it writes the new entries). The island's entry
/// timer re-arms on this until it clears, so the fresh list is
/// repainted even when the refresh outruns the morph (stale-paint
/// bug, iter84/88: up to 12 screenshot PNGs inflate in the thread,
/// ~1-2 s, while the morph ends after anim_ms and nothing else
/// redraws while idle in Clipboard).
pub fn is_refreshing() -> bool {
    with_cache(|c| c.refreshing)
}

/// Fresh search (Esc / re-entry), without re-reading history.
pub fn clip_reset() {
    with_cache(|c| {
        c.query.clear();
        c.selected = 0;
        c.offset = 0;
    });
}

/// Append typed text; selection resets to the top result.
pub fn clip_push_text(s: &str) {
    with_cache(|c| {
        c.query.push_str(s);
        c.selected = 0;
        c.offset = 0;
    });
}

/// Delete the last query character (char-safe); selection resets.
pub fn clip_backspace() {
    with_cache(|c| {
        c.query.pop();
        c.selected = 0;
        c.offset = 0;
    });
}

/// Move the selection: dir = -1 up (previous), +1 down (next).
/// Clamped at both ends (no wrap — matches the launcher's
/// top-anchored selection model; a wrap would hide the top row).
/// The window follows so the selected row stays visible (shared
/// math with the Launcher: `crate::selection`, page 8). Iter39 a10.
pub fn clip_move_selection(dir: i32) {
    with_cache(|c| {
        let n = filtered_indices(c).len();
        let (sel, off) = crate::selection::move_by(
            c.selected,
            c.offset,
            dir,
            n,
            MAX_VISIBLE_ROWS,
        );
        c.selected = sel;
        c.offset = off;
    });
}

// ---------------------------------------------------------------------------
// Subprocess + background refresh. ALL of this runs in a spawned
// thread — never on the event loop. `timeout` is coreutils (present
// on Artix); it bounds a hung cliphist exactly like the script's
// timeout=2.0. Missing cliphist / timeout => empty result, never a
// panic: the panel then honestly shows the empty state.
// ---------------------------------------------------------------------------
fn run_cliphist_list() -> Vec<ClipEntry> {
    let out = std::process::Command::new("timeout")
        .arg("2")
        .arg("cliphist")
        .arg("list")
        .output();
    let Ok(out) = out else { return Vec::new() };
    if !out.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&out.stdout);
    parse_cliphist_list(&text)
}

fn run_cliphist_decode(raw_line: &str) -> Vec<u8> {
    use std::io::Write;
    let mut child = match std::process::Command::new("timeout")
        .arg("2")
        .arg("cliphist")
        .arg("decode")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(raw_line.as_bytes());
    }
    match child.wait_with_output() {
        Ok(o) if o.status.success() => o.stdout,
        _ => Vec::new(),
    }
}

/// Stream `cliphist decode` straight into the PNG decoder: the
/// child's stdout is read row-by-row by `decode_png_thumb`, so the
/// refresh thread never holds a full decoded image (largest seen:
/// 3.65 MB). Same spawn/timeout/stdin contract as
/// `run_cliphist_decode` — only the collection step is gone.
/// `run_cliphist_decode` stays for the wl-copy path, which needs
/// the bytes. (iter128 RAM: the collecting form grew the refresh
/// thread's glibc arena to ~11 MB across two refreshes; the freed
/// image buffers sat below the retained thumbs, so `malloc_trim`
/// could not release them.)
fn decode_image_thumb(raw_line: &str) -> Option<Vec<u8>> {
    use std::io::Write;
    let mut child = match std::process::Command::new("timeout")
        .arg("2")
        .arg("cliphist")
        .arg("decode")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return None,
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(raw_line.as_bytes());
    }
    // Decode from the live pipe; a killed/slow child closes the
    // pipe and the decoder returns None. Always reap the child:
    // it exits on SIGPIPE once we stop reading, or the `timeout 2`
    // wrapper kills it (same 2 s bound as `run_cliphist_decode`).
    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            let _ = child.wait();
            return None;
        }
    };
    let thumb = decode_png_thumb(stdout);
    let _ = child.wait();
    thumb
}

/// Refresh history in a background thread. No-op while one runs.
/// Thumbnails are decoded in the SAME thread, right after the list
/// parse, for at most the first 12 image entries (visible window +
/// margin) — decode cost never touches the draw path.
pub fn refresh_async() {
    let already = with_cache(|c| {
        if c.refreshing {
            true
        } else {
            c.refreshing = true;
            false
        }
    });
    if already {
        return;
    }
    std::thread::spawn(|| {
        let mut entries = run_cliphist_list();
        let mut thumbs_done = 0;
        for e in entries.iter_mut() {
            if !e.is_image || thumbs_done >= MAX_THUMBS_PER_REFRESH {
                continue;
            }
            // Stream the decode: the full image bytes are never
            // held (iter128 RAM — the collecting form retained
            // ~11 MB in this thread's arena across two refreshes).
            e.thumb = decode_image_thumb(&e.raw_line);
            thumbs_done += 1;
        }
        with_cache(|c| {
            // Replace first, THEN clamp the selection into the NEW
            // list (query kept). (Computing the count before the
            // replace would clamp against the stale list.)
            c.entries = entries;
            c.refreshing = false;
            let n = filtered_indices(c).len();
            let (sel, off) =
                crate::selection::clamp(c.selected, c.offset, n, MAX_VISIBLE_ROWS);
            c.selected = sel;
            c.offset = off;
        });
        // iter91 (B3 RAM) + iter128: the refresh used to hold up to
        // 12 decoded PNG buffers (largest 3.65 MB); the decode now
        // streams row-by-row, so the transient is ~200 KB. Keep the
        // trim — it returns whatever the small accumulators freed
        // (same pattern as wallpaper_blur.rs ensure_loaded).
        unsafe {
            libc::malloc_trim(0);
        }
    });
}

/// Copy the selected (filtered) entry to the system clipboard in a
/// fire-and-forget thread, exactly like the script (decode the full
/// raw line, MIME-sniff images, `wl-copy [--type mime]`). Returns
/// false when there is nothing to copy (empty / no match) — the
/// caller then leaves the panel open. The caller collapses the
/// island immediately after a true return; it never waits.
pub fn clip_select_async() -> bool {
    let raw_line = with_cache(|c| {
        let idx = filtered_indices(c).get(c.selected).copied();
        idx.map(|i| (c.entries[i].raw_line.clone(), c.entries[i].is_image))
    });
    let Some((raw_line, is_image)) = raw_line else {
        return false;
    };
    std::thread::spawn(move || {
        copy_raw_line_to_clipboard(&raw_line, is_image);
    });
    true
}

/// Copy drawn row `idx` (click path; index space = the DRAWN rows:
/// drawn row i is filtered index `offset + i`). Returns false for
/// an out-of-range row. Iter39 a10: the offset mapping lives here
/// so main.rs's click hunk needs no change.
pub fn clip_row_select_async(idx: usize) -> bool {
    let raw_line = with_cache(|c| {
        let i = filtered_indices(c).get(c.offset + idx).copied();
        i.map(|j| (c.entries[j].raw_line.clone(), c.entries[j].is_image))
    });
    let Some((raw_line, is_image)) = raw_line else {
        return false;
    };
    std::thread::spawn(move || {
        copy_raw_line_to_clipboard(&raw_line, is_image);
    });
    true
}

fn copy_raw_line_to_clipboard(raw_line: &str, is_image: bool) {
    use std::io::Write;
    let bytes = run_cliphist_decode(raw_line);
    if bytes.is_empty() {
        return;
    }
    let mut cmd = std::process::Command::new("wl-copy");
    if is_image {
        cmd.arg("--type").arg(sniff_mime(&bytes));
    }
    let mut child = match cmd
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return,
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(&bytes);
    }
    // wl-copy forks to serve the clipboard; wait() returns when the
    // parent exits, so this thread ends promptly either way.
    let _ = child.wait();
}

// ---------------------------------------------------------------------------
// Minimal PNG decoder (dependency-free — the island has no image
// crate). NOTE (iter85): production thumbnails no longer use this
// decoder — decode_png_thumb streams rows through the `png` crate
// instead. This decoder stays as the unit-test oracle for PNG
// semantics (filters, palette, CRC); it never runs on the refresh
// path. Scope, stated plainly: 8-bit samples (16-bit takes the
// high byte), colour types 0/2/3/4/6, filters 0-4, NON-interlaced
// only (Adam7 => None => placeholder tile). CRC32 of every chunk
// and the zlib Adler32 are verified; anything malformed => None.
// DEFLATE: stored + fixed + dynamic Huffman blocks (RFC 1951).
// Output is RGBA8, straight alpha. Hard caps: raw scanline data
// over 64 MB is rejected before inflation, so a hostile clipboard
// image cannot grow the island past the RAM gate.
// ---------------------------------------------------------------------------
const MAX_PNG_RAW_BYTES: usize = 64 * 1024 * 1024;

/// Hard cap on decoded pixels for a thumbnail (iter85): bounds the
/// refresh thread's CPU on a hostile clipboard image. Memory is
/// already bounded (rows stream); this bounds work. 64M pixels is
/// far above any real screenshot (2256x1504 = 3.4M).
const MAX_PNG_PIXELS: usize = 64 * 1024 * 1024;

const fn crc32_table() -> [u32; 256] {
    let mut t = [0u32; 256];
    let mut i = 0usize;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
}
const CRC_TABLE: [u32; 256] = crc32_table();

fn crc32(data: &[u8]) -> u32 {
    let mut c: u32 = 0xFFFF_FFFF;
    for &b in data {
        c = CRC_TABLE[((c ^ b as u32) & 0xff) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for chunk in data.chunks(5552) {
        for &x in chunk {
            a += x as u32;
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}

struct BitReader<'a> {
    data: &'a [u8],
    idx: usize,
    buf: u32,
    cnt: u32,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, idx: 0, buf: 0, cnt: 0 }
    }
    fn bits(&mut self, n: u32) -> Option<u32> {
        if n == 0 {
            return Some(0);
        }
        while self.cnt < n {
            let b = *self.data.get(self.idx)? as u32;
            self.idx += 1;
            self.buf |= b << self.cnt;
            self.cnt += 8;
        }
        let v = self.buf & ((1u32 << n) - 1);
        self.buf >>= n;
        self.cnt -= n;
        Some(v)
    }
    /// Drop the rest of the current byte (stored-block alignment).
    fn align_byte(&mut self) {
        self.buf = 0;
        self.cnt = 0;
    }
}

struct Huffman {
    counts: [u16; 16],
    symbols: Vec<u16>,
}

fn build_huffman(lengths: &[u8]) -> Huffman {
    let mut counts = [0u16; 16];
    for &l in lengths {
        counts[l as usize] += 1;
    }
    counts[0] = 0;
    let mut offs = [0u16; 16];
    for i in 1..16 {
        offs[i] = offs[i - 1] + counts[i - 1];
    }
    let total: usize = lengths.iter().filter(|&&l| l > 0).count();
    let mut symbols = vec![0u16; total];
    for (sym, &l) in lengths.iter().enumerate() {
        if l > 0 {
            symbols[offs[l as usize] as usize] = sym as u16;
            offs[l as usize] += 1;
        }
    }
    Huffman { counts, symbols }
}

impl Huffman {
    /// Canonical-code decode (zlib puff.c loop, verbatim logic).
    fn decode(&self, r: &mut BitReader) -> Option<u16> {
        let mut code: i32 = 0;
        let mut first: i32 = 0;
        let mut index: i32 = 0;
        for len in 1..16 {
            code |= r.bits(1)? as i32;
            let count = self.counts[len] as i32;
            if code - count < first {
                // Incomplete / adversarial code lengths can make
                // this index land past the table — None, never panic.
                let si = index + (code - first);
                if si < 0 || si as usize >= self.symbols.len() {
                    return None;
                }
                return Some(self.symbols[si as usize]);
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        None
    }
}

const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
    131, 163, 195, 227, 258,
];
const LENGTH_EXTRA: [u32; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u32; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
    13, 13,
];

fn inflate_block(
    r: &mut BitReader,
    lit: &Huffman,
    dist: &Huffman,
    out: &mut Vec<u8>,
    out_cap: usize,
) -> Option<()> {
    loop {
        let sym = lit.decode(r)? as usize;
        if sym < 256 {
            if out.len() >= out_cap {
                return None;
            }
            out.push(sym as u8);
        } else if sym == 256 {
            return Some(());
        } else {
            let li = sym - 257;
            if li >= 29 {
                return None;
            }
            let len = LENGTH_BASE[li] as usize + r.bits(LENGTH_EXTRA[li])? as usize;
            let dsym = dist.decode(r)? as usize;
            if dsym >= 30 {
                return None;
            }
            let d = DIST_BASE[dsym] as usize + r.bits(DIST_EXTRA[dsym])? as usize;
            if d == 0 || d > out.len() || out.len() + len > out_cap {
                return None;
            }
            let start = out.len() - d;
            for i in 0..len {
                let b = out[start + i];
                out.push(b);
            }
        }
    }
}

/// Raw DEFLATE stream -> bytes. `out_cap` is the exact expected
/// size (PNG scanlines); exceeding it is an error.
fn inflate(data: &[u8], out_cap: usize) -> Option<Vec<u8>> {
    let mut r = BitReader::new(data);
    let mut out: Vec<u8> = Vec::with_capacity(out_cap.min(1 << 20));
    loop {
        let bfinal = r.bits(1)?;
        let btype = r.bits(2)?;
        match btype {
            0 => {
                r.align_byte();
                let len = (r.bits(8)? | (r.bits(8)? << 8)) as usize;
                let nlen = (r.bits(8)? | (r.bits(8)? << 8)) as usize;
                if len != (!nlen & 0xffff) || out.len() + len > out_cap {
                    return None;
                }
                for _ in 0..len {
                    out.push(*r.data.get(r.idx)?);
                    r.idx += 1;
                }
            }
            1 => {
                let mut lit_lengths = [0u8; 288];
                for (i, l) in lit_lengths.iter_mut().enumerate() {
                    *l = match i {
                        0..=143 => 8,
                        144..=255 => 9,
                        256..=279 => 7,
                        _ => 8,
                    };
                }
                let dist_lengths = [5u8; 30];
                let lit = build_huffman(&lit_lengths);
                let dist = build_huffman(&dist_lengths);
                inflate_block(&mut r, &lit, &dist, &mut out, out_cap)?;
            }
            2 => {
                let hlit = r.bits(5)? as usize + 257;
                let hdist = r.bits(5)? as usize + 1;
                let hclen = r.bits(4)? as usize + 4;
                const ORDER: [usize; 19] = [
                    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
                ];
                let mut cl_lengths = [0u8; 19];
                for i in 0..hclen {
                    cl_lengths[ORDER[i]] = r.bits(3)? as u8;
                }
                let cl = build_huffman(&cl_lengths);
                let total = hlit + hdist;
                let mut lengths = vec![0u8; total];
                let mut i = 0;
                while i < total {
                    let sym = cl.decode(&mut r)? as usize;
                    match sym {
                        0..=15 => {
                            lengths[i] = sym as u8;
                            i += 1;
                        }
                        16 => {
                            if i == 0 {
                                return None;
                            }
                            let prev = lengths[i - 1];
                            let rep = r.bits(2)? as usize + 3;
                            if i + rep > total {
                                return None;
                            }
                            for _ in 0..rep {
                                lengths[i] = prev;
                                i += 1;
                            }
                        }
                        17 => {
                            let rep = r.bits(3)? as usize + 3;
                            if i + rep > total {
                                return None;
                            }
                            i += rep;
                        }
                        18 => {
                            let rep = r.bits(7)? as usize + 11;
                            if i + rep > total {
                                return None;
                            }
                            i += rep;
                        }
                        _ => return None,
                    }
                }
                let lit = build_huffman(&lengths[..hlit]);
                let dist = build_huffman(&lengths[hlit..]);
                inflate_block(&mut r, &lit, &dist, &mut out, out_cap)?;
            }
            _ => return None,
        }
        if bfinal == 1 {
            break;
        }
    }
    Some(out)
}

fn paeth(a: i32, b: i32, c: i32) -> i32 {
    let p = a + b - c;
    let pa = (p - a).abs();
    let pb = (p - b).abs();
    let pc = (p - c).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

fn read_be32(b: &[u8]) -> u32 {
    ((b[0] as u32) << 24) | ((b[1] as u32) << 16) | ((b[2] as u32) << 8) | (b[3] as u32)
}

/// Decode PNG bytes to RGBA8. Returns (rgba, width, height).
/// See the module header for the supported scope.
///
/// NOTE (iter85): kept as the unit-test oracle only — production
/// thumbnails stream rows via `decode_png_thumb`. This function
/// holds inflated + unfiltered + RGBA buffers at once (~40 MB peak
/// for a 2256x1504 screenshot), which is the RAM-fix target.
#[allow(dead_code)]
pub fn decode_png_rgba(bytes: &[u8]) -> Option<(Vec<u8>, usize, usize)> {
    const SIG: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
    if bytes.len() < 8 || bytes[0..8] != SIG {
        return None;
    }
    let mut pos = 8usize;
    let (mut w, mut h) = (0usize, 0usize);
    let (mut bit_depth, mut color_type, mut interlace) = (0u8, 0u8, 0u8);
    let mut palette: Vec<u8> = Vec::new();
    let mut trns: Vec<u8> = Vec::new();
    let mut idat: Vec<u8> = Vec::new();
    let mut seen_ihdr = false;
    while pos + 8 <= bytes.len() {
        let len = read_be32(&bytes[pos..pos + 4]) as usize;
        let ctype = &bytes[pos + 4..pos + 8];
        let data_start = pos + 8;
        let data_end = data_start.checked_add(len)?;
        if data_end + 4 > bytes.len() {
            return None;
        }
        let data = &bytes[data_start..data_end];
        // Verify the chunk CRC (type + data).
        let mut crc_input = Vec::with_capacity(4 + len);
        crc_input.extend_from_slice(ctype);
        crc_input.extend_from_slice(data);
        if crc32(&crc_input) != read_be32(&bytes[data_end..data_end + 4]) {
            return None;
        }
        match ctype {
            b"IHDR" => {
                if len != 13 || seen_ihdr {
                    return None;
                }
                seen_ihdr = true;
                w = read_be32(&data[0..4]) as usize;
                h = read_be32(&data[4..8]) as usize;
                bit_depth = data[8];
                color_type = data[9];
                if data[10] != 0 || data[11] != 0 {
                    return None; // compression / filter method must be 0
                }
                interlace = data[12];
            }
            b"PLTE" => palette = data.to_vec(),
            b"tRNS" => trns = data.to_vec(),
            b"IDAT" => idat.extend_from_slice(data),
            b"IEND" => break,
            _ => {} // ancillary chunks ignored (gAMA, pHYs, tEXt, ...)
        }
        pos = data_end + 4;
    }
    if !seen_ihdr || w == 0 || h == 0 || interlace != 0 {
        return None;
    }
    let channels: usize = match color_type {
        0 => 1,
        2 => 3,
        3 => 1,
        4 => 2,
        6 => 4,
        _ => return None,
    };
    if !matches!(bit_depth, 1 | 2 | 4 | 8 | 16) {
        return None;
    }
    if color_type == 2 || color_type == 4 || color_type == 6 {
        if bit_depth != 8 && bit_depth != 16 {
            return None;
        }
    }
    if color_type == 3 && palette.len() % 3 != 0 {
        return None;
    }
    // Bits per pixel in the packed scanline data.
    let bits_per_pixel = channels * bit_depth as usize;
    let row_bits = w * bits_per_pixel;
    let stride = (row_bits + 7) / 8;
    let raw_len = h.checked_mul(1 + stride)?;
    if raw_len > MAX_PNG_RAW_BYTES {
        return None;
    }
    // zlib stream: 2-byte header (CMF low nibble must be 8 = deflate),
    // deflate data, 4-byte Adler32 trailer.
    if idat.len() < 6 || idat[0] & 0x0f != 8 {
        return None;
    }
    let raw = inflate(&idat[2..idat.len() - 4], raw_len)?;
    if raw.len() != raw_len {
        return None;
    }
    let adler = read_be32(&idat[idat.len() - 4..]);
    if adler32(&raw) != adler {
        return None;
    }
    // Unfilter (bytes-per-pixel for the filter = ceil(bpp/8), min 1).
    let filter_bpp = ((bits_per_pixel + 7) / 8).max(1);
    let mut img = vec![0u8; raw_len];
    for y in 0..h {
        let row_start = y * (1 + stride);
        let f = raw[row_start];
        if f > 4 {
            return None;
        }
        let cur = &raw[row_start + 1..row_start + 1 + stride];
        // Index `img` directly per pixel (no held slices): the
        // previous row is already fully written, and each read
        // finishes before the write to this row's pixel.
        let base = y * stride;
        for x in 0..stride {
            let a: i32 = if x >= filter_bpp {
                img[base + x - filter_bpp] as i32
            } else {
                0
            };
            let b: i32 = if y == 0 { 0 } else { img[base - stride + x] as i32 };
            let c: i32 = if x >= filter_bpp && y > 0 {
                img[base - stride + x - filter_bpp] as i32
            } else {
                0
            };
            let v = cur[x] as i32;
            let recon = match f {
                0 => v,
                1 => v + a,
                2 => v + b,
                3 => v + (a + b) / 2,
                4 => v + paeth(a, b, c),
                _ => unreachable!(),
            };
            img[base + x] = (recon & 0xff) as u8;
        }
    }
    // Expand packed samples to RGBA8. `sample_raw` returns the
    // UNscaled sample value (0..2^depth-1; 16-bit returns the high
    // byte). Greyscale scales it to 0..255; palette uses it RAW as
    // the index (scaling it would corrupt the lookup).
    let sample_raw = |row: &[u8], px: usize, ch: usize| -> u32 {
        let bitpos = (px * channels + ch) * bit_depth as usize;
        if bit_depth >= 8 {
            row[bitpos / 8] as u32
        } else {
            let byte = row[bitpos / 8] as u32;
            let shift = 8 - bit_depth as u32 - (bitpos % 8) as u32;
            let max = (1u32 << bit_depth) - 1;
            (byte >> shift) & max
        }
    };
    // Greyscale sample scaled to a full 0..255 byte.
    let grey = |row: &[u8], px: usize, ch: usize| -> u8 {
        if bit_depth < 8 {
            let max = (1u32 << bit_depth) - 1;
            (sample_raw(row, px, ch) * 255 / max) as u8
        } else {
            sample_raw(row, px, ch) as u8
        }
    };
    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        let row = &img[y * stride..(y + 1) * stride];
        for x in 0..w {
            let o = (y * w + x) * 4;
            match color_type {
                0 => {
                    let g = grey(row, x, 0);
                    rgba[o] = g;
                    rgba[o + 1] = g;
                    rgba[o + 2] = g;
                    rgba[o + 3] = 255;
                }
                2 => {
                    rgba[o] = sample_raw(row, x, 0) as u8;
                    rgba[o + 1] = sample_raw(row, x, 1) as u8;
                    rgba[o + 2] = sample_raw(row, x, 2) as u8;
                    rgba[o + 3] = 255;
                }
                3 => {
                    let idx = sample_raw(row, x, 0) as usize;
                    let p = palette.get(idx * 3..idx * 3 + 3).unwrap_or(&[0, 0, 0]);
                    rgba[o] = p[0];
                    rgba[o + 1] = p[1];
                    rgba[o + 2] = p[2];
                    rgba[o + 3] = trns.get(idx).copied().unwrap_or(255);
                }
                4 => {
                    let g = grey(row, x, 0);
                    rgba[o] = g;
                    rgba[o + 1] = g;
                    rgba[o + 2] = g;
                    rgba[o + 3] = sample_raw(row, x, 1) as u8;
                }
                6 => {
                    rgba[o] = sample_raw(row, x, 0) as u8;
                    rgba[o + 1] = sample_raw(row, x, 1) as u8;
                    rgba[o + 2] = sample_raw(row, x, 2) as u8;
                    rgba[o + 3] = sample_raw(row, x, 3) as u8;
                }
                _ => unreachable!(),
            }
        }
    }
    Some((rgba, w, h))
}

/// Downscale RGBA to a 32x32 thumbnail, aspect preserved (fit inside
/// the square, centred on transparency — the script's
/// `scale=58:58:force_original_aspect_ratio=decrease,pad=64:64`,
/// at island scale). Box-average sampling, like wallpaper_blur.rs.
///
/// NOTE (iter85): kept for its unit test only — production thumbs
/// area-average inside `decode_png_thumb` while streaming rows.
#[allow(dead_code)]
pub fn downscale_to_thumb(rgba: &[u8], w: usize, h: usize) -> Option<Vec<u8>> {
    if w == 0 || h == 0 || rgba.len() != w * h * 4 {
        return None;
    }
    let (tw, th) = if w >= h {
        (THUMB_SIZE, ((h * THUMB_SIZE) / w).max(1))
    } else {
        (((w * THUMB_SIZE) / h).max(1), THUMB_SIZE)
    };
    let ox = (THUMB_SIZE - tw) / 2;
    let oy = (THUMB_SIZE - th) / 2;
    let mut thumb = vec![0u8; THUMB_SIZE * THUMB_SIZE * 4];
    for ty in 0..th {
        let y0 = ty * h / th;
        let y1 = ((ty + 1) * h / th).max(y0 + 1);
        for tx in 0..tw {
            let x0 = tx * w / tw;
            let x1 = ((tx + 1) * w / tw).max(x0 + 1);
            let mut acc = [0u32; 4];
            let mut n = 0u32;
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let si = (sy * w + sx) * 4;
                    for c in 0..4 {
                        acc[c] += rgba[si + c] as u32;
                    }
                    n += 1;
                }
            }
            let di = ((oy + ty) * THUMB_SIZE + (ox + tx)) * 4;
            for c in 0..4 {
                thumb[di + c] = (acc[c] / n.max(1)) as u8;
            }
        }
    }
    Some(thumb)
}

/// Convert one `png`-crate output row (after
/// `normalize_to_color8`: 8-bit, Grayscale / GrayscaleAlpha / Rgb /
/// Rgba) to RGBA8. `out` must hold exactly `w * 4` bytes.
fn png_row_to_rgba(row: &[u8], ct: png::ColorType, w: usize, out: &mut [u8]) {
    use png::ColorType;
    debug_assert_eq!(out.len(), w * 4);
    match ct {
        ColorType::Grayscale => {
            for x in 0..w {
                let g = row[x];
                let o = x * 4;
                out[o] = g;
                out[o + 1] = g;
                out[o + 2] = g;
                out[o + 3] = 255;
            }
        }
        ColorType::GrayscaleAlpha => {
            for x in 0..w {
                let s = x * 2;
                let g = row[s];
                let o = x * 4;
                out[o] = g;
                out[o + 1] = g;
                out[o + 2] = g;
                out[o + 3] = row[s + 1];
            }
        }
        ColorType::Rgb => {
            for x in 0..w {
                let s = x * 3;
                let o = x * 4;
                out[o] = row[s];
                out[o + 1] = row[s + 1];
                out[o + 2] = row[s + 2];
                out[o + 3] = 255;
            }
        }
        ColorType::Rgba => {
            out[..w * 4].copy_from_slice(&row[..w * 4]);
        }
        ColorType::Indexed => {
            // normalize_to_color8 expands palettes, so this is
            // unreachable — stay total rather than panicking.
            for x in 0..w {
                let o = x * 4;
                out[o] = 0;
                out[o + 1] = 0;
                out[o + 2] = 0;
                out[o + 3] = 255;
            }
        }
    }
}

/// Decode PNG bytes straight to a 32x32 thumbnail (None when the
/// bytes are not a decodable PNG — the caller draws the placeholder).
///
/// Iter85 RAM fix: streams rows through the `png` crate
/// (`Transformations::normalize_to_color8`, one reusable row buffer)
/// and area-averages each row straight into the 32x32 thumb — the
/// old path decoded the FULL image to RGBA first (`decode_png_rgba`:
/// inflated + unfiltered + RGBA buffers alive at once, ~40 MB peak
/// for a 2256x1504 screenshot). Peak here is one row (~9 KB) +
/// 32x32 f32 accumulators (~20 KB). Interlaced PNGs stay rejected
/// (placeholder tile), exactly like the old path — `next_row` yields
/// Adam7 pass rows, not display rows, for those.
/// Decode a PNG thumbnail from ANY reader — a byte slice (tests)
/// or a live child stdout (production, `decode_image_thumb`).
/// Rows stream through the `png` crate; the full image bytes are
/// never held in memory (iter128 RAM: the old `&[u8]` form forced
/// `run_cliphist_decode` to collect the whole PNG — up to 3.65 MB
/// per image — into the refresh thread's arena, where it was
/// retained across refreshes as fragmented arena growth).
pub fn decode_png_thumb<R: std::io::Read>(reader: R) -> Option<Vec<u8>> {
    let mut decoder = png::Decoder::new(reader);
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let info = reader.info();
    let (w, h) = (info.width as usize, info.height as usize);
    if w == 0 || h == 0 || info.interlaced {
        return None;
    }
    // Bound the work: a hostile clipboard image cannot make the
    // refresh thread chew through gigapixels (memory is already
    // bounded — rows stream — this bounds CPU).
    if w.checked_mul(h)? > MAX_PNG_PIXELS {
        return None;
    }
    let (out_ct, _) = reader.output_color_type();
    let line_size = reader.output_line_size(info.width);
    if line_size == 0 {
        return None;
    }
    let (tw, th) = if w >= h {
        (THUMB_SIZE, ((h * THUMB_SIZE) / w).max(1))
    } else {
        (((w * THUMB_SIZE) / h).max(1), THUMB_SIZE)
    };
    let ox = (THUMB_SIZE - tw) / 2;
    let oy = (THUMB_SIZE - th) / 2;
    // Area-average accumulators: one f32 sum per thumb pixel per
    // channel + one f32 weight per thumb pixel (~20 KB total).
    let mut acc = vec![0f32; THUMB_SIZE * THUMB_SIZE * 4];
    let mut wsum = vec![0f32; THUMB_SIZE * THUMB_SIZE];
    // One reusable RGBA row buffer (w*4, ~9 KB at 2256 px).
    let mut row_rgba = vec![0u8; w * 4];
    for y in 0..h {
        let row = reader.next_row().ok()??;
        let data = row.data();
        if data.len() < line_size {
            return None;
        }
        png_row_to_rgba(&data[..line_size], out_ct, w, &mut row_rgba);
        // Thumb rows this source row overlaps (usually exactly one).
        let ty0 = y * th / h;
        let ty1 = ((y + 1) * th / h).min(th).max(ty0 + 1);
        for ty in ty0..ty1 {
            let wy = ((ty + 1) as f32 * h as f32 / th as f32)
                .min((y + 1) as f32)
                - (ty as f32 * h as f32 / th as f32).max(y as f32);
            if wy <= 0.0 {
                continue;
            }
            for x in 0..w {
                let tx0 = x * tw / w;
                let tx1 = ((x + 1) * tw / w).min(tw).max(tx0 + 1);
                let si = x * 4;
                let px = [
                    row_rgba[si] as f32,
                    row_rgba[si + 1] as f32,
                    row_rgba[si + 2] as f32,
                    row_rgba[si + 3] as f32,
                ];
                for tx in tx0..tx1 {
                    let wx = ((tx + 1) as f32 * w as f32 / tw as f32)
                        .min((x + 1) as f32)
                        - (tx as f32 * w as f32 / tw as f32).max(x as f32);
                    if wx <= 0.0 {
                        continue;
                    }
                    let wt = wx * wy;
                    let di = ((oy + ty) * THUMB_SIZE + (ox + tx)) * 4;
                    for c in 0..4 {
                        acc[di + c] += px[c] * wt;
                    }
                    wsum[(oy + ty) * THUMB_SIZE + (ox + tx)] += wt;
                }
            }
        }
    }
    let mut thumb = vec![0u8; THUMB_SIZE * THUMB_SIZE * 4];
    for i in 0..THUMB_SIZE * THUMB_SIZE {
        let ws = wsum[i];
        if ws > 0.0 {
            for c in 0..4 {
                thumb[i * 4 + c] = (acc[i * 4 + c] / ws).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    Some(thumb)
}

// ---------------------------------------------------------------------------
// Layout + drawing — same SDF/Argb8888 style as launcher_panel.rs
// (private helpers duplicated here, per project convention).
// The panel background is NOT filled here: draw() in main.rs fills
// the black capsule for every state except ControlCenter/Launcher/
// Lockscreen, and Clipboard stays in that default. Nav discs are
// also drawn by main.rs after the state arm (MENU_CYCLE membership).
// ---------------------------------------------------------------------------
/// Panel-relative rect, for hit-testing in main.rs.
#[derive(Clone, Copy, Debug)]
pub struct ClipRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl ClipRect {
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }
    pub fn cy(&self) -> f32 {
        self.y + self.h / 2.0
    }
}

/// Row rects for the rows currently drawn (filtered, capped at 8).
/// Single source of truth for draw + click hit-testing.
pub fn clipboard_row_rects() -> Vec<ClipRect> {
    let n = visible_row_count();
    (0..n)
        .map(|i| ClipRect {
            x: ROW_X,
            y: ROW_Y0 + i as f32 * ROW_PITCH,
            w: ROW_W,
            h: ROW_H,
        })
        .collect()
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

fn fill_rounded_rect(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    rect: ClipRect,
    radius: f32,
    rgb: (u8, u8, u8),
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
            blend_pixel(canvas, w, x, y, rgb, a);
        }
    }
}

fn stroke_rounded_rect(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    rect: ClipRect,
    radius: f32,
    t: f32,
    rgb: (u8, u8, u8),
) {
    let inner = ClipRect {
        x: rect.x + t,
        y: rect.y + t,
        w: rect.w - 2.0 * t,
        h: rect.h - 2.0 * t,
    };
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

fn fill_rect(canvas: &mut [u8], w: u32, h: u32, x0: f32, y0: f32, x1: f32, y1: f32, rgb: (u8, u8, u8)) {
    for y in (y0.floor() as i32)..(y1.ceil() as i32) {
        for x in (x0.floor() as i32)..(x1.ceil() as i32) {
            blend_pixel(canvas, w, x, y, rgb, 1.0);
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
            blend_pixel(canvas, w, x, y, rgb, (0.5 - d).clamp(0.0, 1.0));
        }
    }
}

fn stroke_circle(canvas: &mut [u8], w: u32, h: u32, cx: f32, cy: f32, r: f32, t: f32, rgb: (u8, u8, u8)) {
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

fn stroke_segment(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    (x1, y1): (f32, f32),
    (x2, y2): (f32, f32),
    t: f32,
    rgb: (u8, u8, u8),
) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len2 = dx * dx + dy * dy;
    let pad = t + 1.0;
    let x0 = (x1.min(x2) - pad).floor() as i32;
    let x1i = (x1.max(x2) + pad).ceil() as i32;
    let y0 = (y1.min(y2) - pad).floor() as i32;
    let y1i = (y1.max(y2) + pad).ceil() as i32;
    for y in y0..y1i {
        for x in x0..x1i {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let s = if len2 == 0.0 {
                0.0
            } else {
                ((px - x1) * dx + (py - y1) * dy) / len2
            }
            .clamp(0.0, 1.0);
            let qx = x1 + dx * s;
            let qy = y1 + dy * s;
            let d = ((px - qx) * (px - qx) + (py - qy) * (py - qy)).sqrt();
            let a = (t / 2.0 + 0.5 - d).clamp(0.0, 1.0);
            if a <= 0.0 {
                continue;
            }
            blend_pixel(canvas, w, x, y, rgb, a);
        }
    }
}

fn fill_triangle(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    (ax, ay): (f32, f32),
    (bx, by): (f32, f32),
    (cx, cy): (f32, f32),
    rgb: (u8, u8, u8),
) {
    let x0 = ax.min(bx).min(cx).floor() as i32 - 1;
    let x1 = ax.max(bx).max(cx).ceil() as i32 + 1;
    let y0 = ay.min(by).min(cy).floor() as i32 - 1;
    let y1 = ay.max(by).max(cy).ceil() as i32 + 1;
    let edge = |px: f32, py: f32, x1: f32, y1: f32, x2: f32, y2: f32| -> f32 {
        (px - x1) * (y2 - y1) - (py - y1) * (x2 - x1)
    };
    let area = edge(ax, ay, bx, by, cx, cy);
    if area == 0.0 {
        return;
    }
    for y in y0..y1 {
        for x in x0..x1 {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let w0 = edge(px, py, bx, by, cx, cy) / area;
            let w1 = edge(px, py, cx, cy, ax, ay) / area;
            let w2 = edge(px, py, ax, ay, bx, by) / area;
            let inside = w0.min(w1).min(w2);
            if inside >= 0.0 {
                blend_pixel(canvas, w, x, y, rgb, 1.0);
            } else if inside > -0.06 {
                blend_pixel(canvas, w, x, y, rgb, (1.0 + inside / 0.06).clamp(0.0, 1.0));
            }
        }
    }
}

struct TextRow {
    glyphs: Vec<(fontdue::Metrics, std::sync::Arc<[u8]>)>,
    width: f32,
    ascent: f32,
}

fn layout_text(font: &Font, text: &str, px: f32) -> TextRow {
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

fn text_width(font: &Font, text: &str, px: f32) -> f32 {
    text.chars()
        .map(|ch| crate::lock_timer::rasterize_cached(font, ch, px).0.advance_width)
        .sum()
}

fn truncate_to_width(font: &Font, text: &str, px: f32, max_w: f32) -> String {
    if text_width(font, text, px) <= max_w {
        return text.to_string();
    }
    let ellipsis_w = text_width(font, "…", px);
    let mut out = String::new();
    let mut w = 0.0f32;
    for ch in text.chars() {
        let cw = crate::lock_timer::rasterize_cached(font, ch, px).0.advance_width;
        if w + cw + ellipsis_w > max_w {
            break;
        }
        out.push(ch);
        w += cw;
    }
    out.push('…');
    out
}

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
) {
    if text.is_empty() {
        return;
    }
    let row = layout_text(font, text, px);
    let baseline = y_top + row.ascent;
    let mut pen = x;
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
) {
    let tw = text_width(font, text, px);
    draw_text(canvas, w, h, font, text, px, cx - tw / 2.0, y_top, rgb);
}

/// Search magnifier (launcher model — procedural, no guessed
/// Material codepoint): ring r 6.5, 2 px stroke, handle lower-right.
fn draw_magnifier(canvas: &mut [u8], w: u32, h: u32, cx: f32, cy: f32, rgb: (u8, u8, u8)) {
    stroke_circle(canvas, w, h, cx - 1.0, cy - 1.0, 6.0, 2.0, rgb);
    stroke_segment(canvas, w, h, (cx + 3.2, cy + 3.2), (cx + 8.0, cy + 8.0), 2.0, rgb);
}

/// Blit a 32x32 straight-alpha RGBA thumb onto the BGRA canvas,
/// top-left at (x0, y0). Same blend as launcher_icons::blit_icon.
fn blit_thumb(canvas: &mut [u8], w: u32, h: u32, x0: i32, y0: i32, rgba: &[u8]) {
    debug_assert_eq!(rgba.len(), THUMB_SIZE * THUMB_SIZE * 4);
    for sy in 0..THUMB_SIZE as i32 {
        let y = y0 + sy;
        if y < 0 || y >= h as i32 {
            continue;
        }
        for sx in 0..THUMB_SIZE as i32 {
            let x = x0 + sx;
            if x < 0 || x >= w as i32 {
                continue;
            }
            let si = (sy as usize * THUMB_SIZE + sx as usize) * 4;
            let sa = rgba[si + 3] as f32 / 255.0;
            if sa <= 0.0 {
                continue;
            }
            let i = ((y as u32) * w + x as u32) as usize * 4;
            if i + 3 >= canvas.len() {
                continue;
            }
            let inv = 1.0 - sa;
            canvas[i] = (rgba[si + 2] as f32 * sa + canvas[i] as f32 * inv) as u8; // B
            canvas[i + 1] = (rgba[si + 1] as f32 * sa + canvas[i + 1] as f32 * inv) as u8; // G
            canvas[i + 2] = (rgba[si] as f32 * sa + canvas[i + 2] as f32 * inv) as u8; // R
            canvas[i + 3] = 255; // A
        }
    }
}

/// Text-entry icon: the script's SVG document, drawn procedurally
/// into the 32x32 slot (rect x 6..26, y 4..28, rx 4; three lines).
/// Same colours as the script's inline SVG.
fn draw_text_icon(canvas: &mut [u8], w: u32, h: u32, cx: f32, cy: f32) {
    let rect = ClipRect {
        x: cx - 10.0,
        y: cy - 12.0,
        w: 20.0,
        h: 24.0,
    };
    fill_rounded_rect(canvas, w, h, rect, 4.0, C_DOC_FILL);
    stroke_rounded_rect(canvas, w, h, rect, 4.0, 1.5, C_DOC_STROKE);
    let t = 1.5;
    stroke_segment(canvas, w, h, (cx - 5.0, cy - 5.0), (cx + 5.0, cy - 5.0), t, C_DOC_LINE1);
    stroke_segment(canvas, w, h, (cx - 5.0, cy), (cx + 5.0, cy), t, C_DOC_LINE2);
    stroke_segment(canvas, w, h, (cx - 5.0, cy + 5.0), (cx + 1.0, cy + 5.0), t, C_DOC_LINE3);
}

/// Image placeholder tile: dark rounded square + mountain glyph
/// (music-panel artwork-slot family). Used for JPEG/WebP images
/// and for PNGs that failed to decode.
fn draw_image_placeholder(canvas: &mut [u8], w: u32, h: u32, cx: f32, cy: f32) {
    let rect = ClipRect {
        x: cx - 16.0,
        y: cy - 16.0,
        w: 32.0,
        h: 32.0,
    };
    fill_rounded_rect(canvas, w, h, rect, 4.0, C_TILE_FILL);
    stroke_rounded_rect(canvas, w, h, rect, 4.0, 1.0, C_TILE_BORDER);
    fill_circle(canvas, w, h, cx + 6.0, cy - 6.0, 3.0, C_TILE_GLYPH);
    fill_triangle(
        canvas, w, h,
        (cx - 11.0, cy + 10.0),
        (cx - 2.0, cy - 3.0),
        (cx + 4.0, cy + 10.0),
        C_TILE_GLYPH,
    );
    fill_triangle(
        canvas, w, h,
        (cx - 1.0, cy + 10.0),
        (cx + 6.0, cy + 1.0),
        (cx + 12.0, cy + 10.0),
        C_TILE_GLYPH,
    );
}

/// Draw the full clipboard panel into `canvas` (504 wide, height =
/// `panel_size().1`, Argb8888). `font` is the UI font (Quicksand
/// Bold); `font_regular` is the body font (Regular 400 — search
/// placeholder + empty states, iter18 rule). The caller (draw() in
/// main.rs) owns the black capsule background; this only draws
/// content. Rows are single-line: thumb + 13 px label, selected
/// row highlighted. While a refresh is still running and no data
/// has ever loaded, the panel shows the empty state — the honest
/// "not loaded yet" read is the same string the script shows for
/// an empty history, and the 250 ms entry timer redraws once the
/// refresh lands.
pub fn draw_clipboard_panel(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &Font,
    font_regular: &Font,
) {
    debug_assert_eq!(w, PANEL_W);
    // Search line: flat on black, launcher model — magnifier +
    // placeholder, or the query as plain white text (no caret, no box).
    draw_magnifier(canvas, w, h, MAGNIFIER_CX, MAGNIFIER_CY, C_SECONDARY);
    // Draw under one cache lock (launcher live_draw_data pattern):
    // no entry/thumb cloning per frame, and the refresh thread only
    // ever waits out one draw (~ms). Row rects are computed inline
    // here — clipboard_row_rects() would re-lock the same Mutex.
    with_cache(|c| {
        if c.query.trim().is_empty() {
            draw_text(
                canvas, w, h, font_regular, "Search...", 13.0,
                SEARCH_TEXT_X, SEARCH_TEXT_TOP, C_SECONDARY,
            );
        } else {
            let q = truncate_to_width(font, c.query.trim(), 13.0, TEXT_MAX_W);
            draw_text(canvas, w, h, font, &q, 13.0, SEARCH_TEXT_X, SEARCH_TEXT_TOP, C_PRIMARY);
        }
        let idx = filtered_indices(c);
        if idx.is_empty() {
            draw_text_centered(
                canvas, w, h, font_regular, "Keine Einträge vorhanden!", 12.0,
                PANEL_W as f32 / 2.0, EMPTY_MSG_TOP, C_SECONDARY,
            );
            return;
        }
        let selected = c.selected.min(idx.len() - 1);
        // Window the filtered list by the scroll offset (iter39 a10):
        // without this, selected >= 8 is drawn nowhere and the
        // highlight vanishes. `selected` indexes the FULL filtered
        // list; the visible slot of the selected row is
        // `selected - offset`.
        let start = c.offset.min(idx.len());
        let end = (start + MAX_VISIBLE_ROWS).min(idx.len());
        for (i, &ei) in idx[start..end].iter().enumerate() {
            let entry = &c.entries[ei];
            let rect = ClipRect {
                x: ROW_X,
                y: ROW_Y0 + i as f32 * ROW_PITCH,
                w: ROW_W,
                h: ROW_H,
            };
            if i + c.offset == selected {
                fill_rounded_rect(canvas, w, h, rect, ROW_R, C_HIGHLIGHT);
            }
            let thumb_cx = THUMB_X + THUMB_SIZE as f32 / 2.0; // 40
            let thumb_cy = rect.cy();
            match (&entry.thumb, entry.is_image) {
                (Some(thumb), true) => {
                    blit_thumb(
                        canvas,
                        w,
                        h,
                        (thumb_cx - THUMB_SIZE as f32 / 2.0).round() as i32,
                        (thumb_cy - THUMB_SIZE as f32 / 2.0).round() as i32,
                        thumb,
                    );
                }
                (None, true) => draw_image_placeholder(canvas, w, h, thumb_cx, thumb_cy),
                _ => draw_text_icon(canvas, w, h, thumb_cx, thumb_cy),
            }
            // Single-line 13 px label, pixel-truncated so it never
            // runs past the right padding (the label is already
            // char-truncated to 90 by the parser; this only guards
            // wide glyphs).
            let label = truncate_to_width(font, &entry.label, 13.0, TEXT_MAX_W);
            draw_text(
                canvas, w, h, font, &label, 13.0,
                TEXT_X, rect.y + 14.5, C_PRIMARY,
            );
        }
        // Scrollbar (iter39 a10): a thin 4 px thumb in the right
        // padding gutter (x 496..500), only when scrolling is
        // possible. It sits clear of the rows (which end at x 490);
        // the nav disc crosses it at mid-height and is drawn over it
        // (main.rs draws discs last). Inside this guard the window
        // is always full (8 rows), so the track spans all 8 rows and
        // `max_off > 0` — no division by zero.
        if idx.len() > MAX_VISIBLE_ROWS {
            let track_y0 = ROW_Y0;
            let track_y1 =
                ROW_Y0 + MAX_VISIBLE_ROWS as f32 * ROW_PITCH - (ROW_PITCH - ROW_H);
            let track_h = track_y1 - track_y0;
            let max_off = (idx.len() - MAX_VISIBLE_ROWS) as f32;
            let thumb_h =
                (track_h * MAX_VISIBLE_ROWS as f32 / idx.len() as f32).max(20.0);
            let thumb_y =
                track_y0 + (track_h - thumb_h) * (c.offset as f32 / max_off);
            fill_rounded_rect(
                canvas,
                w,
                h,
                ClipRect {
                    x: PANEL_W as f32 - 8.0,
                    y: thumb_y,
                    w: 4.0,
                    h: thumb_h,
                },
                2.0,
                C_SCROLLBAR,
            );
        }
    });
}

// ---------------------------------------------------------------------------
// Tests — pure logic only (std + this module), so they run under
// `cargo test --offline` with no font files and no cliphist/wl-copy.
// Global-state helpers (clip_push_text / clip_move_selection /
// refresh_async / clip_select_async) are NOT unit-tested here on
// purpose: they mutate the shared CACHE static, and cargo runs
// tests in parallel threads — asserting on them would be flaky.
// They are covered by the live verification plan in the iter39
// report instead (seed cliphist, open the view, type / arrow /
// Enter / click, wl-paste proof).
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    // --- parsing / labelling (the script's semantics) ---

    #[test]
    fn parse_text_line_collapses_whitespace() {
        let e = parse_cliphist_line("42\tHello   \t  world\n  foo");
        assert_eq!(e.id, "42");
        assert!(!e.is_image);
        // tab inside the content collapses like any whitespace
        assert_eq!(e.label, "Hello world foo");
        assert_eq!(e.raw_line, "42\tHello   \t  world\n  foo");
    }

    #[test]
    fn parse_line_without_tab_has_empty_content() {
        // Script: parts = l.split("\t", 1); content = parts[1] if
        // len(parts) > 1 else "".
        let e = parse_cliphist_line("7");
        assert_eq!(e.id, "7");
        assert_eq!(e.label, "");
        assert!(!e.is_image);
    }

    #[test]
    fn parse_image_line_label() {
        let e = parse_cliphist_line("3\t[[ binary data 800x600 image/png ]]");
        assert!(e.is_image);
        assert_eq!(e.label, "Bild (800x600 image/png)");
    }

    #[test]
    fn parse_image_line_empty_details() {
        // The script's exact replace order: the first replace eats
        // the space, so the second (" ]]") no longer matches and
        // the empty-details edge keeps "]]" — the port matches the
        // script byte-for-byte (real cliphist lines always carry
        // details, so this edge never occurs live).
        let e = parse_cliphist_line("3\t[[ binary data ]]");
        assert_eq!(e.label, "Bild (]])");
    }

    #[test]
    fn parse_list_caps_at_100_and_skips_blank_lines() {
        let mut s = String::new();
        for i in 0..105 {
            s.push_str(&format!("{i}\tentry {i}\n"));
        }
        s.push_str("\n");
        let entries = parse_cliphist_list(&s);
        assert_eq!(entries.len(), 100);
        assert_eq!(entries[0].id, "0");
        assert_eq!(entries[99].id, "99");
    }

    #[test]
    fn text_label_truncates_to_90_chars() {
        // 30 words collapse to "word word ... word" (149 chars).
        let long = "word ".repeat(30);
        let label = text_label(&long);
        assert_eq!(label.chars().count(), 90);
        assert!(label.ends_with("..."));
        // 87 kept chars = 17 x "word " (85) + "wo", all ASCII.
        let expected_prefix = format!("{}wo", "word ".repeat(17));
        assert_eq!(&label[..87], expected_prefix);
    }

    #[test]
    fn truncate_chars_is_char_based_not_byte_based() {
        // 91 multi-byte chars: byte length is 182, char length 91.
        let s = "é".repeat(91);
        let t = truncate_chars(&s, 90);
        assert_eq!(t.chars().count(), 90);
        assert!(t.ends_with("..."));
        assert_eq!(t.chars().take(87).count(), 87);
        // Exactly 90 chars is untouched (no "...").
        let exact = "é".repeat(90);
        assert_eq!(truncate_chars(&exact, 90), exact);
    }

    #[test]
    fn truncate_chars_short_string_unchanged() {
        assert_eq!(truncate_chars("hello", 90), "hello");
        assert_eq!(truncate_chars("", 90), "");
    }

    #[test]
    fn image_label_strips_brackets_like_script() {
        assert_eq!(
            image_label("[[ binary data 1234x567 image/jpeg ]]"),
            "Bild (1234x567 image/jpeg)"
        );
    }

    // --- MIME sniffing (script order: jpeg, webp, else png) ---

    #[test]
    fn sniff_mime_defaults_png() {
        assert_eq!(sniff_mime(&[0x89, b'P', b'N', b'G']), "image/png");
        assert_eq!(sniff_mime(&[]), "image/png");
        assert_eq!(sniff_mime(&[0xff]), "image/png");
    }

    #[test]
    fn sniff_mime_jpeg_magic() {
        assert_eq!(sniff_mime(&[0xff, 0xd8, 0xff, 0xe0]), "image/jpeg");
    }

    #[test]
    fn sniff_mime_webp_magic() {
        let mut b = b"RIFF____WEBP".to_vec();
        b[4..8].copy_from_slice(&1234u32.to_le_bytes());
        assert_eq!(sniff_mime(&b), "image/webp");
    }

    #[test]
    fn sniff_mime_riff_without_webp_stays_png() {
        assert_eq!(sniff_mime(b"RIFF____AVI "), "image/png");
    }

    // --- panel sizing ---

    #[test]
    fn panel_height_matches_launcher_at_6_rows() {
        assert_eq!(panel_height_for(0), 200);
        assert_eq!(panel_height_for(1), 129); // 74 + 42 + 12.5 = 128.5
        assert_eq!(panel_height_for(6), 366); // launcher height
        assert_eq!(panel_height_for(8), 461);
    }

    // --- filtering on a local cache (no global state) ---

    fn cache_with(labels: &[&str]) -> ClipboardCache {
        ClipboardCache {
            entries: labels
                .iter()
                .enumerate()
                .map(|(i, l)| ClipEntry {
                    id: i.to_string(),
                    raw_line: format!("{i}\t{l}"),
                    label: l.to_string(),
                    is_image: l.starts_with("Bild"),
                    thumb: None,
                })
                .collect(),
            query: String::new(),
            selected: 0,
            offset: 0,
            refreshing: false,
        }
    }

    #[test]
    fn filter_matches_label_case_insensitively() {
        let mut c = cache_with(&["Hello World", "Bild (800x600 image/png)", "other"]);
        c.query = "bild".to_string();
        assert_eq!(filtered_indices(&c), vec![1]);
        c.query = "HELLO".to_string();
        assert_eq!(filtered_indices(&c), vec![0]);
        c.query = String::new();
        assert_eq!(filtered_indices(&c), vec![0, 1, 2]);
        c.query = "zzz".to_string();
        assert!(filtered_indices(&c).is_empty());
    }

    // --- PNG decoder: test-only stored-block encoder ---

    fn png_chunk(out: &mut Vec<u8>, ctype: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(ctype);
        out.extend_from_slice(data);
        let mut crc_in = Vec::with_capacity(4 + data.len());
        crc_in.extend_from_slice(ctype);
        crc_in.extend_from_slice(data);
        out.extend_from_slice(&crc32(&crc_in).to_be_bytes());
    }

    /// Build a PNG with a stored (uncompressed) zlib stream.
    /// `raw` = scanlines INCLUDING one filter byte per row.
    fn encode_test_png(
        w: u32,
        h: u32,
        color_type: u8,
        bit_depth: u8,
        raw: &[u8],
        palette: Option<&[u8]>,
        interlace: u8,
    ) -> Vec<u8> {
        let mut out = vec![137, 80, 78, 71, 13, 10, 26, 10];
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&w.to_be_bytes());
        ihdr.extend_from_slice(&h.to_be_bytes());
        ihdr.push(bit_depth);
        ihdr.push(color_type);
        ihdr.push(0);
        ihdr.push(0);
        ihdr.push(interlace);
        png_chunk(&mut out, b"IHDR", &ihdr);
        if let Some(p) = palette {
            png_chunk(&mut out, b"PLTE", p);
        }
        // zlib: header 0x78 0x01, stored blocks, Adler32 trailer.
        let mut z = vec![0x78, 0x01];
        for (i, block) in raw.chunks(65535).enumerate() {
            let last = i == (raw.len() + 65534) / 65535 - 1 || raw.is_empty();
            z.push(if last { 1 } else { 0 });
            z.extend_from_slice(&(block.len() as u16).to_le_bytes());
            z.extend_from_slice(&(!block.len() as u16).to_le_bytes());
            z.extend_from_slice(block);
        }
        if raw.is_empty() {
            z.push(1);
            z.extend_from_slice(&0u16.to_le_bytes());
            z.extend_from_slice(&0xffffu16.to_le_bytes());
        }
        z.extend_from_slice(&adler32(raw).to_be_bytes());
        png_chunk(&mut out, b"IDAT", &z);
        png_chunk(&mut out, b"IEND", &[]);
        out
    }

    #[test]
    fn png_roundtrip_rgba_filter_none() {
        // 3x2 RGBA, distinct pixels incl. a transparent one.
        let raw: Vec<u8> = vec![
            0, 255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, // row 0
            0, 10, 20, 30, 40, 50, 60, 70, 80, 0, 0, 0, 0, // row 1
        ];
        let png = encode_test_png(3, 2, 6, 8, &raw, None, 0);
        let (rgba, w, h) = decode_png_rgba(&png).expect("decode our own PNG");
        assert_eq!((w, h), (3, 2));
        assert_eq!(&rgba[0..12], &raw[1..13]);
        assert_eq!(&rgba[12..24], &raw[14..26]);
    }

    #[test]
    fn png_roundtrip_rgb() {
        let raw: Vec<u8> = vec![
            0, 200, 100, 50, 1, 2, 3, // row 0: two RGB pixels
            0, 9, 8, 7, 6, 5, 4, // row 1
        ];
        let png = encode_test_png(2, 2, 2, 8, &raw, None, 0);
        let (rgba, w, h) = decode_png_rgba(&png).expect("decode RGB PNG");
        assert_eq!((w, h), (2, 2));
        assert_eq!(&rgba[0..8], &[200, 100, 50, 255, 1, 2, 3, 255]);
        assert_eq!(&rgba[8..16], &[9, 8, 7, 255, 6, 5, 4, 255]);
    }

    #[test]
    fn png_sub_filter_decodes() {
        // 4x1 RGB, filter 1 (Sub): stored = value - left (bpp 3).
        let px: [u8; 12] = [10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120];
        let mut raw = vec![1u8];
        for i in 0..12 {
            let left = if i < 3 { 0 } else { px[i - 3] as i16 };
            raw.push(px[i].wrapping_sub(left as u8));
        }
        let png = encode_test_png(4, 1, 2, 8, &raw, None, 0);
        let (rgba, _, _) = decode_png_rgba(&png).expect("decode Sub PNG");
        for i in 0..4 {
            assert_eq!(&rgba[i * 4..i * 4 + 3], &px[i * 3..i * 3 + 3]);
        }
    }

    #[test]
    fn png_up_filter_decodes() {
        // 2x2 grey, row 1 stored with filter 2 (Up): value - above.
        let raw: Vec<u8> = vec![0, 50, 60, 2, 10, 10];
        let png = encode_test_png(2, 2, 0, 8, &raw, None, 0);
        let (rgba, _, _) = decode_png_rgba(&png).expect("decode Up PNG");
        assert_eq!(&rgba[0..4], &[50, 50, 50, 255]);
        assert_eq!(&rgba[4..8], &[60, 60, 60, 255]);
        assert_eq!(&rgba[8..12], &[60, 60, 60, 255]); // 50+10
        assert_eq!(&rgba[12..16], &[70, 70, 70, 255]); // 60+10
    }

    #[test]
    fn png_palette_decodes() {
        // 2x1 palette, indices 0 and 1.
        let raw: Vec<u8> = vec![0, 0, 1];
        let palette: [u8; 6] = [255, 0, 0, 0, 0, 255];
        let png = encode_test_png(2, 1, 3, 8, &raw, Some(&palette), 0);
        let (rgba, _, _) = decode_png_rgba(&png).expect("decode palette PNG");
        assert_eq!(&rgba[0..4], &[255, 0, 0, 255]);
        assert_eq!(&rgba[4..8], &[0, 0, 255, 255]);
    }

    #[test]
    fn png_rejects_bad_magic_truncated_and_interlaced() {
        assert!(decode_png_rgba(&[1, 2, 3]).is_none());
        let raw: Vec<u8> = vec![0, 1, 2, 3, 4];
        let mut png = encode_test_png(1, 1, 0, 8, &raw, None, 0);
        png.truncate(png.len() - 10); // cut the IEND off mid-chunk
        assert!(decode_png_rgba(&png).is_none());
        let interlaced = encode_test_png(1, 1, 0, 8, &raw, None, 1);
        assert!(decode_png_rgba(&interlaced).is_none());
    }

    #[test]
    fn png_rejects_corrupt_crc() {
        let raw: Vec<u8> = vec![0, 9, 9, 9, 9];
        let mut png = encode_test_png(1, 1, 0, 8, &raw, None, 0);
        // Flip a byte inside the IDAT data (CRC must catch it).
        let flip_at = png.len() - 20;
        png[flip_at] ^= 0xff;
        assert!(decode_png_rgba(&png).is_none());
    }

    #[test]
    fn thumb_downscale_preserves_aspect_and_colours() {
        // 2x1: red | blue -> 32x16 centred in the 32x32 square.
        let rgba: Vec<u8> = vec![255, 0, 0, 255, 0, 0, 255, 255];
        let thumb = downscale_to_thumb(&rgba, 2, 1).expect("thumb");
        assert_eq!(thumb.len(), 32 * 32 * 4);
        // Top padding rows are transparent.
        assert_eq!(&thumb[0..4], &[0, 0, 0, 0]);
        // Middle row: left edge red, right edge blue.
        let mid = (8 * 32) * 4;
        assert_eq!(&thumb[mid..mid + 3], &[255, 0, 0]);
        assert_eq!(&thumb[mid + 31 * 4..mid + 31 * 4 + 3], &[0, 0, 255]);
    }

    #[test]
    fn decode_png_thumb_rejects_jpeg_bytes() {
        // JPEG magic is not a PNG: thumb is None (placeholder path).
        assert!(decode_png_thumb(&[0xff, 0xd8, 0xff, 0xe0, 0, 1, 2, 3][..]).is_none());
    }

    #[test]
    fn decode_png_thumb_streams_small_png() {
        // 2x1 RGB: red | blue -> 32x16 centred in the 32x32 square,
        // through the streaming path (same expectations as the
        // downscale_to_thumb test above).
        let raw: Vec<u8> = vec![0, 255, 0, 0, 0, 0, 255];
        let png = encode_test_png(2, 1, 2, 8, &raw, None, 0);
        let thumb = decode_png_thumb(&png[..]).expect("thumb");
        assert_eq!(thumb.len(), 32 * 32 * 4);
        // Top padding rows are transparent.
        assert_eq!(&thumb[0..4], &[0, 0, 0, 0]);
        // Middle row: left edge red, right edge blue.
        let mid = (8 * 32) * 4;
        assert_eq!(&thumb[mid..mid + 3], &[255, 0, 0]);
        assert_eq!(&thumb[mid + 31 * 4..mid + 31 * 4 + 3], &[0, 0, 255]);
    }
}
