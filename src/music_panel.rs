//! Compact MPRIS player: artwork, track details and one transport row.
//! Player discovery, commands and artwork decoding stay on the existing worker.

pub const PANEL_W: u32 = 612;
pub const PANEL_H: u32 = 150;

const WHITE: [u8; 3] = [235, 237, 240];
const SECONDARY: [u8; 3] = [161, 165, 174];
const DIM_GREY: [u8; 3] = [86, 89, 96];
const ART_BORDER: [u8; 3] = [30, 31, 35];
const PROGRESS_TRACK: [u8; 3] = [45, 47, 53];
const CONTROL_BG: [u8; 3] = [24, 25, 29];
const LEFT_INSET: f32 = 24.0;
const RIGHT_INSET: f32 = PANEL_W as f32 - LEFT_INSET;
const ART_SIZE: f32 = 96.0;
const ART_RADIUS: f32 = 10.0;
const TILE_TOP: f32 = 27.0;
const ART_THUMB: usize = 96;
const TEXT_WITH_ART: f32 = 140.0;
const TRACK_PX: f32 = 17.0;
const ARTIST_PX: f32 = 12.0;
const PROGRESS_END: f32 = 439.0;
const PROGRESS_CY: f32 = 108.0;
const CTRL_CY: f32 = 112.0;
const CTRL_HALF_SPREAD: f32 = 44.0;
const CTRL_CX: f32 = 527.0;
const CTRL_ICON_EM_SKIP: f32 = 30.0;
const CTRL_ICON_EM_PLAY: f32 = 32.0;
const ICON_SKIP_PREV: char = '\u{E045}';
const ICON_PLAY: char = '\u{E037}';
const ICON_SKIP_NEXT: char = '\u{E044}';

const MAX_ART_BYTES: usize = 8 * 1024 * 1024;
const MAX_JPEG_PIXELS: usize = 4 * 1024 * 1024;
const MAX_PNG_PIXELS: usize = 64 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Media state from the selected MPRIS player.
// ---------------------------------------------------------------------------
/// Current MPRIS metadata. Missing metadata produces an explicit idle state.
#[derive(Clone)]
pub struct MusicData {
    pub artist: Option<String>,
    pub title: Option<String>,
    /// 0.0–1.0 playback position fraction. Ignored when idle.
    pub progress: f32,
    pub playing: bool,
    /// Whole-second playback position (MPRIS Position, us -> s,
    /// truncated). Drives the iter106 live-repaint fingerprint:
    /// it advances once a second while Playing and is frozen while
    /// Paused, so the 1 s timer redraws exactly when the fill moved.
    pub position_secs: u64,
    /// Whole-second track length (MPRIS mpris:length, us -> s).
    /// In the fingerprint so a same-position replay of a different
    /// length cannot alias.
    pub length_secs: u64,
    /// MPRIS `mpris:artUrl` as polled (None = the player exposes none).
    /// Set by `parse_playerctl`; never decoded there.
    pub art_url: Option<String>,
    /// Decoded artwork, pre-scaled to ART_THUMB x ART_THUMB RGBA8.
    /// `Some` only when `art_url` resolved AND decoded (background
    /// refresher, one-entry cache). Each image occupies 36 KiB.
    pub artwork: Option<Vec<u8>>,
}

impl MusicData {
    pub fn has_track(&self) -> bool {
        self.artist.is_some() || self.title.is_some()
    }

    /// No player: show an honest empty state and inactive controls.
    pub fn idle() -> Self {
        Self {
            artist: None,
            title: None,
            progress: 0.0,
            playing: false,
            position_secs: 0,
            length_secs: 0,
            art_url: None,
            artwork: None,
        }
    }

    /// A playing track. `progress` is clamped to 0.0–1.0 at draw time.
    /// No artwork attached — the refresher fills `art_url`/`artwork`
    /// from the live poll; tests use `with_artwork` to inject a thumb.
    pub fn playing(artist: &str, title: &str, progress: f32) -> Self {
        Self {
            artist: Some(artist.to_string()),
            title: Some(title.to_string()),
            progress,
            playing: true,
            position_secs: 0,
            length_secs: 0,
            art_url: None,
            artwork: None,
        }
    }

    /// Attach a decoded artwork thumb. Test-only
    /// constructor helper — `playing()` deliberately starts without
    /// artwork, and the refresher sets the field directly.
    #[cfg(test)]
    pub fn with_artwork(mut self, artwork: Option<Vec<u8>>) -> Self {
        self.artwork = artwork;
        self
    }
}

// ---------------------------------------------------------------------------
// Live player state (iter36 a6b): a background thread polls playerctl
// once a second and writes the cache; draw() only ever reads the
// cache, so a slow or missing player can never stall a frame (freeze
// rule: no blocking external command on the render path). No player
// => the honest idle (artist/title None, playing false) — never a
// fake track. Transport commands run in a bounded background worker.
// ---------------------------------------------------------------------------

static MUSIC_CACHE: std::sync::OnceLock<std::sync::RwLock<MusicData>> =
    std::sync::OnceLock::new();

/// The player the refresher last selected (`--player=<name>`), if
/// any — written by the refresher thread on selection change only,
/// read by main.rs's transport dispatch so a click drives the SAME
/// player the reads observe. None until the refresher has selected
/// once (no player yet, or no poll yet). (iter114 W3.)
static SELECTED_PLAYER: std::sync::OnceLock<std::sync::RwLock<Option<String>>> =
    std::sync::OnceLock::new();

/// The currently selected player, if any — for the transport click
/// dispatch in main.rs. None => no player selected (dispatch still
/// fires bare `playerctl <cmd>`).
pub fn selected_player() -> Option<String> {
    SELECTED_PLAYER
        .get_or_init(|| std::sync::RwLock::new(None))
        .read()
        .unwrap()
        .clone()
}

/// Optimistic play/pause flip for the click path (mirrors CC's
/// `cache_set_toggle`): the click already dispatched its command on
/// a detached thread, so flip the cached bit now and the immediate
/// `draw_if_ready()` in main.rs shows the new glyph/brightness at
/// once; the refresher corrects it from the real player within ~1 s
/// if the command failed. Only flips when a track is known — with
/// no player there is nothing to flip.
pub fn optimistic_toggle() {
    if let Some(cache) = MUSIC_CACHE.get() {
        let mut d = cache.write().unwrap();
        if d.artist.is_some() || d.title.is_some() {
            d.playing = !d.playing;
        }
    }
}

/// Send a transport command to the same player whose metadata is displayed.
/// Direct argv keeps player names out of the shell; idle controls do nothing.
pub fn dispatch_transport(kind: u8) -> bool {
    if !live_cached().has_track() { return false; }
    let Some(player) = selected_player() else { return false; };
    let verb = match kind { 0 => "previous", 1 => "play-pause", 2 => "next", _ => return false };
    let worker = std::thread::Builder::new().name("music-control".into()).spawn(move || {
        let target = format!("--player={player}");
        if run_cmd_timeout("playerctl", &[&target, verb], CMD_TIMEOUT).is_none() {
            eprintln!("dynamite-rust: music {verb} failed");
        }
    });
    if let Err(error) = worker {
        eprintln!("dynamite-rust: music command worker: {error}");
        return false;
    }
    if kind == 1 { optimistic_toggle(); }
    true
}

/// Hard timeout for any single playerctl call on the refresher
/// thread. Local playerctl answers in 5-50 ms; 1500 ms is generous
/// and still bounds a hung MPRIS player to a stall the user never
/// sees (off the hot path entirely). Port of the
/// controlcenter_panel.rs pattern via music_feed.rs (iter114 W1):
/// the old blocking `.output()` with NO timeout let a wedged MPRIS
/// player wedge the refresher thread forever and freeze the cache
/// at seed-idle.
const CMD_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(1500);

fn run_cmd_timeout(
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
        let _ = std::io::Read::read_to_end(&mut stdout, &mut buf);
        let _ = tx.send(buf);
    });
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let buf = rx
                    .recv_timeout(std::time::Duration::from_millis(200))
                    .unwrap_or_default();
                let out = std::process::Output {
                    status,
                    stdout: buf,
                    stderr: Vec::new(),
                };
                return if out.status.success() { Some(out) } else { None };
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

fn cmd_out(args: &[&str]) -> Option<String> {
    let out = run_cmd_timeout("playerctl", args, CMD_TIMEOUT)?;
    String::from_utf8(out.stdout)
        .ok()
        .map(|s| s.trim().to_string())
}

/// One playerctl poll: status + `artist|title|position|mpris:length|artUrl`
/// (position AND mpris:length are both microseconds — playerctl 2.4.1
/// returns {{position}} in µs, NOT seconds; iter86 fix). Any failure =>
/// idle; an empty artist+title => idle too. Artwork is NOT fetched
/// here — `parse_playerctl` keeps only the URL string; the refresher
/// resolves it (fetch + decode) with its 1-entry cache.
/// `playerctl --list-all`: one player name per line, in playerctl's
/// own order. Empty / failing => no players.
fn list_players() -> Vec<String> {
    match cmd_out(&["--list-all"]) {
        Some(s) => s
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        None => Vec::new(),
    }
}

/// Score for selection: Playing wins over Paused, Paused over
/// Stopped/other. A player with no status (error) scores 0 and can
/// never win while any scored player exists.
fn score_status(status: Option<&str>) -> u8 {
    match status {
        Some("Playing") => 3,
        Some("Paused") => 2,
        Some("Stopped") => 1,
        _ => 0,
    }
}

/// Pick the player for this poll. `sticky` is the player selected on
/// the previous poll (None on the first poll). Rules, in order:
///   1. Highest status score wins (a Playing player always beats a
///      Paused one — this is what keeps a real player ahead of an
///      idle tray app like NectarLink).
///   2. Ties break toward the STICKY player (no flapping when two
///      players share a status — e.g. stub Paused + NectarLink
///      Paused: whichever was selected stays selected).
///   3. Otherwise the first player in `playerctl --list-all` order.
/// `statuses` maps player name -> `playerctl status` result (None =
/// the status call failed). Returns None when the list is empty or
/// every status failed (score 0 across the board AND no sticky
/// player to keep — a lone failing player is not selected).
fn select_player(
    sticky: &Option<String>,
    players: &[String],
    statuses: &std::collections::HashMap<String, Option<String>>,
) -> Option<String> {
    let mut best: Option<(u8, String)> = None;
    for p in players {
        let score = score_status(statuses.get(p).and_then(|s| s.as_deref()));
        let better = match &best {
            None => score > 0,
            Some((bs, bp)) => {
                score > *bs
                    || (score == *bs
                        && score > 0
                        && sticky.as_deref() == Some(p.as_str())
                        && sticky.as_deref() != Some(bp.as_str()))
            }
        };
        if better {
            best = Some((score, p.clone()));
        }
    }
    // Sticky keep-alive: if the sticky player is still listed but
    // its status call failed this poll (score 0), keep it rather
    // than dropping to idle for one poll — the next poll re-scores.
    // Only when nothing else scored: a scored player always wins.
    if best.is_none() {
        if let Some(s) = sticky {
            if players.iter().any(|p| p == s) {
                return Some(s.clone());
            }
        }
    }
    best.map(|(_, p)| p)
}

/// One full poll. Returns:
///   * `Some((data, selected))` — fresh data (including the honest
///     idle when the player list is empty: `selected == None`);
///   * `None` — TRANSIENT failure (a status/metadata spawn timed out
///     / failed, or no player could be selected). The caller keeps
///     the last good snapshot; the panel never flashes to idle on a
///     single slow call.
/// `sticky` is the previously selected player (for the tie-break).
/// Re-derived to the live 9-field MusicData (artist, title,
/// progress, playing, position_secs, length_secs, art_url, artwork)
/// via parse_playerctl — position/length/artUrl are carried through,
/// artwork stays None here (the refresher resolves it).
/// Whether a failed metadata call for the selected player means
/// "this player has no track" (honest idle) rather than a transient
/// failure (keep the last good snapshot). A Stopped player has no
/// current track by definition -- playerctl answers rc=1 ("No player
/// could handle this command") -- so the panel must go idle instead
/// of sticking on a stale track when the only player left is a
/// Stopped one (e.g. NectarLink after the stub exits). Playing/Paused
/// (or an unknown status) means a track exists and the call just
/// failed, so the caller keeps the last good snapshot.
fn no_track_on_metadata_failure(status: Option<&str>) -> bool {
    matches!(status, Some("Stopped"))
}

fn read_playerctl(sticky: &Option<String>) -> Option<(MusicData, Option<String>)> {
    let players = list_players();
    if players.is_empty() {
        return Some((MusicData::idle(), None));
    }
    // One status call per listed player (2 players = 2 fast calls;
    // this is the only way to know which one is Playing).
    let mut statuses: std::collections::HashMap<String, Option<String>> =
        std::collections::HashMap::new();
    for p in &players {
        statuses.insert(p.clone(), cmd_out(&["--player", p.as_str(), "status"]));
    }
    let selected = select_player(sticky, &players, &statuses)?;
    // The winner's status is known from the scoring pass; a missing
    // entry here means the status call failed but the sticky
    // keep-alive selected it — parse_playerctl treats status None
    // as idle, so the metadata below still has to carry a track
    // for anything to show.
    let status = statuses.get(&selected).and_then(|s| s.clone());
    let meta = match cmd_out(&[
        "--player",
        selected.as_str(),
        "metadata",
        "--format",
        "{{artist}}|{{title}}|{{position}}|{{mpris:length}}|{{mpris:artUrl}}",
    ]) {
        Some(m) => m,
        None => {
            // iter114 fix-up: a Stopped player has no track (its
            // metadata call fails with rc=1) -- that is a stable
            // "no track" state, not a transient failure. Return the
            // honest idle so the panel does not stick on a stale
            // track forever when the only player left is Stopped.
            // Playing/Paused/unknown: a track exists, the call just
            // failed -- None (transient, caller keeps last good).
            if no_track_on_metadata_failure(status.as_deref()) {
                return Some((MusicData::idle(), Some(selected)));
            }
            return None;
        }
    };
    let data = parse_playerctl(status.as_deref(), &meta);
    Some((data, Some(selected)))
}

/// Pure parse of one playerctl poll (testable without a player).
/// `status` is the `playerctl status` line (None = no player).
fn parse_playerctl(status: Option<&str>, meta: &str) -> MusicData {
    if status.is_none() {
        return MusicData::idle();
    }
    let playing = matches!(status, Some("Playing"));
    let mut parts = meta.split('|');
    let artist = parts.next().unwrap_or("").trim();
    let title = parts.next().unwrap_or("").trim();
    if artist.is_empty() && title.is_empty() {
        return MusicData::idle();
    }
    // iter86: playerctl {{position}} is MICROSECONDS (2.4.1), same as
    // {{mpris:length}} — NOT seconds as previously assumed. The old code
    // divided pos (µs) by (len_us / 1e6) (seconds), so progress read full
    // for any position over 1 s.
    let pos_us: f64 = parts.next().unwrap_or("").trim().parse().unwrap_or(0.0);
    let len_us: f64 = parts.next().unwrap_or("").trim().parse().unwrap_or(0.0);
    let progress = if len_us > 0.0 {
        (pos_us / len_us) as f32
    } else {
        0.0
    };
    // iter106: whole-second position/length for the live-repaint
    // fingerprint (see `fingerprint`). Truncated, not rounded, so
    // the value is stable within a second and steps exactly once
    // per second of playback.
    let position_secs = (pos_us / 1_000_000.0) as u64;
    let length_secs = (len_us / 1_000_000.0) as u64;
    // 5th field (iter104): MPRIS artUrl — usually file://…jpg/png or
    // https://…. Empty / absent (old 4-field format) = None. The URL
    // string only: decoding happens on the refresher thread, never
    // in this pure parser, so `artwork` stays None here.
    let art_url = parts.next().unwrap_or("").trim();
    MusicData {
        artist: if artist.is_empty() {
            None
        } else {
            Some(artist.to_string())
        },
        title: if title.is_empty() {
            None
        } else {
            Some(title.to_string())
        },
        progress: progress.clamp(0.0, 1.0),
        playing,
        position_secs,
        length_secs,
        art_url: if art_url.is_empty() {
            None
        } else {
            Some(art_url.to_string())
        },
        artwork: None,
    }
}


// ---------------------------------------------------------------------------
// Artwork: artUrl -> bounded RGBA thumbnail, background thread only.
// ---------------------------------------------------------------------------

/// Percent-decode a file:// path (`%20` -> space, ...). Lossy UTF-8:
/// a malformed path decodes to replacement chars and the fs::read
/// below fails -> None, never a panic.
fn percent_decode_path(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
            if let (Some(hi), Some(lo)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push(hi * 16 + lo);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Fetch one artUrl's bytes. file:// via std::fs; http(s):// via the
/// `curl` binary (no HTTP client in Cargo.toml) with a hard 5 s
/// timeout and an 8 MB cap. Anything else (spotify:, data:, ...) has
/// no fetch path -> None. Called ONLY from the refresher thread —
/// a slow server can never stall a frame (freeze rule).
fn fetch_artwork_bytes(url: &str) -> Option<Vec<u8>> {
    if let Some(rest) = url.strip_prefix("file://") {
        let rest = rest.strip_prefix("localhost").unwrap_or(rest);
        let bytes = std::fs::read(percent_decode_path(rest)).ok()?;
        if bytes.is_empty() || bytes.len() > MAX_ART_BYTES {
            return None;
        }
        Some(bytes)
    } else if url.starts_with("http://") || url.starts_with("https://") {
        let out = std::process::Command::new("curl")
            .args([
                "-fsSL",
                "--max-time",
                "5",
                "--max-filesize",
                "8388608",
                url,
            ])
            .output()
            .ok()?;
        if !out.status.success() || out.stdout.is_empty() || out.stdout.len() > MAX_ART_BYTES {
            return None;
        }
        Some(out.stdout)
    } else {
        None
    }
}

/// Fetch + decode one artUrl to a bounded RGBA thumb. None on any
/// failure (unreachable URL, oversize payload, undecodable bytes) —
/// the caller then draws NO tile, never a placeholder.
fn fetch_and_decode_artwork(url: &str) -> Option<Vec<u8>> {
    let bytes = fetch_artwork_bytes(url)?;
    decode_artwork_thumb(&bytes)
}

/// Sniff the magic bytes and dispatch. PNG or JPEG only — anything
/// else (GIF, WebP, SVG, garbage) is None.
fn decode_artwork_thumb(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        decode_png_artwork(bytes)
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        decode_jpeg_artwork(bytes)
    } else {
        None
    }
}

/// Convert one `png`-crate output row (after `normalize_to_color8`:
/// 8-bit Grayscale / GrayscaleAlpha / Rgb / Rgba) to RGBA8. Same
/// conversion as clipboard's `png_row_to_rgba`, duplicated per this
/// module's drop-in convention. `out` must hold exactly `w * 4` bytes.
fn art_png_row_to_rgba(row: &[u8], ct: png::ColorType, w: usize, out: &mut [u8]) {
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

/// Decode PNG bytes to an ART_THUMB-square RGBA thumb (cover-fit,
/// centre-square crop, area-averaged). Rows are streamed so the full
/// source image is never materialised. None on any failure.
fn decode_png_artwork(bytes: &[u8]) -> Option<Vec<u8>> {
    use std::io::Cursor;
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let info = reader.info();
    let (w, h) = (info.width as usize, info.height as usize);
    if w == 0 || h == 0 || info.interlaced {
        return None;
    }
    if w.checked_mul(h)? > MAX_PNG_PIXELS {
        return None;
    }
    let (out_ct, _) = reader.output_color_type();
    let line_size = reader.output_line_size(info.width);
    if line_size == 0 {
        return None;
    }
    // Cover-fit: centre square crop of side s.
    let s = w.min(h);
    let ox = (w - s) / 2;
    let oy = (h - s) / 2;
    let t = ART_THUMB;
    let mut acc = vec![0f32; t * t * 4];
    let mut wsum = vec![0f32; t * t];
    let mut row_rgba = vec![0u8; w * 4];
    for y in 0..h {
        let row = reader.next_row().ok()??;
        let data = row.data();
        if data.len() < line_size {
            return None;
        }
        if y < oy || y >= oy + s {
            continue; // outside the centre-square crop: consumed, not used
        }
        art_png_row_to_rgba(&data[..line_size], out_ct, w, &mut row_rgba);
        let ry = y - oy; // crop-relative row
        let ty0 = ry * t / s;
        let ty1 = ((ry + 1) * t / s).min(t).max(ty0 + 1);
        for ty in ty0..ty1 {
            let wy = ((ty + 1) as f32 * s as f32 / t as f32)
                .min((ry + 1) as f32)
                - (ty as f32 * s as f32 / t as f32).max(ry as f32);
            if wy <= 0.0 {
                continue;
            }
            for x in ox..ox + s {
                let rx = x - ox; // crop-relative column
                let tx0 = rx * t / s;
                let tx1 = ((rx + 1) * t / s).min(t).max(tx0 + 1);
                let si = x * 4;
                let px = [
                    row_rgba[si] as f32,
                    row_rgba[si + 1] as f32,
                    row_rgba[si + 2] as f32,
                    row_rgba[si + 3] as f32,
                ];
                for tx in tx0..tx1 {
                    let wx = ((tx + 1) as f32 * s as f32 / t as f32)
                        .min((rx + 1) as f32)
                        - (tx as f32 * s as f32 / t as f32).max(rx as f32);
                    if wx <= 0.0 {
                        continue;
                    }
                    let wt = wx * wy;
                    let di = (ty * t + tx) * 4;
                    for c in 0..4 {
                        acc[di + c] += px[c] * wt;
                    }
                    wsum[ty * t + tx] += wt;
                }
            }
        }
    }
    let mut thumb = vec![0u8; t * t * 4];
    for i in 0..t * t {
        let ws = wsum[i];
        if ws > 0.0 {
            for c in 0..4 {
                thumb[i * 4 + c] = (acc[i * 4 + c] / ws).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    Some(thumb)
}

/// Decode JPEG bytes to an ART_THUMB-square RGBA thumb (cover-fit: centre-square
/// crop, box-averaged; alpha 255). zune-jpeg decodes the full frame,
/// so sources are capped at MAX_JPEG_PIXELS (4M px = 12 MB RGB
/// transient, freed right after downscale) — same pattern as
/// wallpaper_blur's `decode_jpeg_to_logical`, but down to a thumb.
/// None on any failure (caller draws NO tile).
fn decode_jpeg_artwork(bytes: &[u8]) -> Option<Vec<u8>> {
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
    let s = w.min(h);
    let ox = (w - s) / 2;
    let oy = (h - s) / 2;
    let t = ART_THUMB;
    let mut thumb = vec![0u8; t * t * 4];
    for ty in 0..t {
        let y0 = oy + ty * s / t;
        let y1 = (oy + (ty + 1) * s / t).max(y0 + 1);
        for tx in 0..t {
            let x0 = ox + tx * s / t;
            let x1 = (ox + (tx + 1) * s / t).max(x0 + 1);
            let mut acc = [0u32; 3];
            let mut n = 0u32;
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let si = (sy * w + sx) * 3;
                    for c in 0..3 {
                        acc[c] += rgb[si + c] as u32;
                    }
                    n += 1;
                }
            }
            let di = (ty * t + tx) * 4;
            for c in 0..3 {
                thumb[di + c] = (acc[c] / n.max(1)) as u8;
            }
            thumb[di + 3] = 255;
        }
    }
    Some(thumb)
}

/// Blit the bounded RGBA thumbnail at device scale, clipped to the artwork tile.
fn blit_artwork(canvas: &mut [u8], w: u32, h: u32, rgba: &[u8], x0: f32, y0: f32, cx: f32, cy: f32) {
    if rgba.len() != ART_THUMB * ART_THUMB * 4 { return; }
    let s = scale_of(w);
    let size = (ART_SIZE * s).round() as usize;
    for dy in 0..size {
        for dx in 0..size {
            let x = (x0 * s).round() as i32 + dx as i32;
            let y = (y0 * s).round() as i32 + dy as i32;
            let d = sd_round_box(x as f32 + 0.5, y as f32 + 0.5, cx*s, cy*s,
                (ART_SIZE/2.0-1.0)*s, (ART_SIZE/2.0-1.0)*s, (ART_RADIUS-1.0)*s);
            let edge = (0.5-d).clamp(0.0, 1.0);
            if edge == 0.0 { continue; }
            let si = ((dy * ART_THUMB / size) * ART_THUMB + dx * ART_THUMB / size) * 4;
            blend_px(canvas, w, h, x, y, [rgba[si], rgba[si+1], rgba[si+2]],
                f32::from(rgba[si+3]) / 255.0 * edge);
        }
    }
}

fn ensure_refresher() {
    static STARTED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    STARTED.get_or_init(|| {
        std::thread::spawn(|| {
            // Cache the current URL, including failed fetches, until the track
            // changes. The decoded and live copies occupy at most 72 KiB.
            let mut last_url: Option<String> = None;
            let mut last_art: Option<Vec<u8>> = None;
            // iter114 W3: sticky selection — the player the last
            // poll selected, carried across polls for the tie-break
            // and the keep-alive in select_player().
            let mut sticky: Option<String> = None;
            loop {
                // r2 #5b: 27.2 ms CPU of playerctl per second, forever,
                // even with zero players and the pill collapsed (~2.7% of
                // a core). Only poll while the panel is actually shown.
                if !crate::wallpaper_blur::park_while_closed(
                    std::time::Duration::from_secs(1),
                ) {
                    continue;
                }
                // iter114 W3: a transient failure (None) keeps the
                // last good snapshot — a single slow playerctl call
                // must never blank a playing track to idle.
                let Some((mut data, selected)) = read_playerctl(&sticky) else {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    continue;
                };
                // Log ONLY on selection change: the deploy run's
                // proof that the right player won (and that
                // NectarLink did not). Steady state is silent —
                // no log spam at 1 Hz.
                if selected != sticky {
                    eprintln!(
                        "dynamite-rust: DBG music player selected={}",
                        selected.as_deref().unwrap_or("(none)")
                    );
                    sticky = selected.clone();
                    *SELECTED_PLAYER
                        .get_or_init(|| std::sync::RwLock::new(None))
                        .write()
                        .unwrap() = selected.clone();
                }
                match &data.art_url {
                    Some(url) if Some(url) == last_url.as_ref() => {
                        data.artwork = last_art.clone();
                    }
                    Some(url) => {
                        let art = fetch_and_decode_artwork(url);
                        last_url = Some(url.clone());
                        last_art = art.clone();
                        data.artwork = art;
                    }
                    None => {
                        last_url = None;
                        last_art = None;
                    }
                }
                if let Some(cache) = MUSIC_CACHE.get() {
                    *cache.write().unwrap() = data;
                }
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        });
    });
}

/// The cached live player state (spawns the 1 s refresher on first
/// call). Cheap clone; never runs a command on the calling thread.
pub fn live_cached() -> MusicData {
    let cache = MUSIC_CACHE.get_or_init(|| std::sync::RwLock::new(MusicData::idle()));
    ensure_refresher();
    cache.read().unwrap().clone()
}

/// Cheap fingerprint of the cached player state for the 1 s
/// backdrop-watch timer in main.rs (iter106, same pattern as the
/// iter91 ControlCenter fingerprint): status + artist + title +
/// whole-second position + length + artUrl. Pure function over a
/// `MusicData` value - no cache read, no playerctl spawn, no
/// system call - so the timer can call it every tick while Music
/// is open. Advances once a second while Playing (position_secs
/// steps); frozen while Paused (position fixed, so no redraw
/// storm); changes on track change, play/pause, and artwork-URL
/// change (the refresher writes art_url + decoded thumb in the
/// same cache write, so the URL covers artwork arrival).
pub fn fingerprint(data: &MusicData) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}",
        data.playing,
        data.artist.as_deref().unwrap_or(""),
        data.title.as_deref().unwrap_or(""),
        data.position_secs,
        data.length_secs,
        data.art_url.as_deref().unwrap_or(""),
    )
}

/// Transport centres in panel-local logical coordinates, independent of metadata.
const CONTROL_CENTRES: [(f32, f32); 3] = [
    (CTRL_CX - CTRL_HALF_SPREAD, CTRL_CY), (CTRL_CX, CTRL_CY),
    (CTRL_CX + CTRL_HALF_SPREAD, CTRL_CY),
];

/// Non-overlapping 40 px targets matching the visible transport buttons.
pub fn control_hit(stage_pos: (f64, f64), origin: (f32, f32)) -> Option<u8> {
    let (x, y) = (stage_pos.0 as f32 - origin.0, stage_pos.1 as f32 - origin.1);
    CONTROL_CENTRES.iter().position(|&(cx, cy)| {
        (x - cx).powi(2) + (y - cy).powi(2) <= 20.0 * 20.0
    }).map(|i| i as u8)
}

fn elapsed_text(seconds: u64) -> String {
    if seconds >= 3600 {
        format!("{}:{:02}:{:02}", seconds / 3600, seconds / 60 % 60, seconds % 60)
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}

fn text_left(data: &MusicData) -> f32 {
    if data.artwork.as_ref().is_some_and(|a| a.len() == ART_THUMB * ART_THUMB * 4) {
        TEXT_WITH_ART
    } else {
        LEFT_INSET
    }
}

// ---------------------------------------------------------------------------
// Pixel helpers — same style as calendar_panel.rs / power_panel.rs:
// premultiplied Argb8888 canvas, BGRA byte order, src-over onto the
// (opaque) black panel.
// ---------------------------------------------------------------------------
thread_local! {
    /// r1f (2x): logical -> device scale for the CURRENT draw, set by the
    /// public entry point. Thread-local because the tests draw these
    /// primitives at arbitrary canvas sizes in parallel: it stays 1.0
    /// outside a draw, so w == PANEL_W is byte-identical to the 1x path.
    static SCALE: std::cell::Cell<f32> = const { std::cell::Cell::new(1.0) };
}
fn scale_of(_w: u32) -> f32 {
    SCALE.with(|s| s.get())
}
/// Begin a draw: set the logical -> device scale (see `SCALE`).
fn set_scale(w: u32) {
    SCALE.with(|c| c.set(w as f32 / PANEL_W as f32));
}

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

fn sd_round_box(px: f32, py: f32, cx: f32, cy: f32, hx: f32, hy: f32, r: f32) -> f32 {
    let qx = (px - cx).abs() - (hx - r);
    let qy = (py - cy).abs() - (hy - r);
    let ax = qx.max(0.0);
    let ay = qy.max(0.0);
    (ax * ax + ay * ay).sqrt() + qx.max(qy).min(0.0) - r
}

fn fill_round_box(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    cx: f32,
    cy: f32,
    hx: f32,
    hy: f32,
    r: f32,
    rgb: [u8; 3],
) {
    let s = scale_of(w);
    let (cx, cy, hx, hy, r) = (cx * s, cy * s, hx * s, hy * s, r * s);
    let x0 = (cx - hx - 1.0).floor() as i32;
    let x1 = (cx + hx + 1.0).ceil() as i32;
    let y0 = (cy - hy - 1.0).floor() as i32;
    let y1 = (cy + hy + 1.0).ceil() as i32;
    for y in y0..y1 {
        for x in x0..x1 {
            let d = sd_round_box(x as f32 + 0.5, y as f32 + 0.5, cx, cy, hx, hy, r);
            blend_px(canvas, w, h, x, y, rgb, (0.5 - d).clamp(0.0, 1.0));
        }
    }
}

fn fill_rect(canvas: &mut [u8], w: u32, h: u32, x0: f32, y0: f32, x1: f32, y1: f32, rgb: [u8; 3]) {
    let s = scale_of(w);
    let (x0, x1, y0, y1) = (x0 * s, x1 * s, y0 * s, y1 * s);
    let xa = x0.floor() as i32;
    let xb = x1.ceil() as i32;
    let ya = y0.floor() as i32;
    let yb = y1.ceil() as i32;
    for y in ya..yb {
        for x in xa..xb {
            blend_px(canvas, w, h, x, y, rgb, 1.0);
        }
    }
}

fn fill_circle(canvas: &mut [u8], w: u32, h: u32, cx: f32, cy: f32, r: f32, rgb: [u8; 3]) {
    let s = scale_of(w);
    let (cx, cy, r) = (cx * s, cy * s, r * s);
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

/// Solid triangle (ax,ay) (bx,by) (cx,cy), hard edge — used only by the
/// drawn control fallbacks (no icon font). 1 px AA via the sign test at
/// pixel centres is skipped: at 20 px the icons are small and the panel
/// is upscaled x1.5 anyway; keep the fallback honest and simple.
fn fill_triangle(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    ax: f32,
    ay: f32,
    bx: f32,
    by: f32,
    cx: f32,
    cy: f32,
    rgb: [u8; 3],
) {
    fn sign(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
        (px - bx) * (ay - by) - (ax - bx) * (py - by)
    }
    let s = scale_of(w);
    let ((ax, ay), (bx, by), (cx, cy)) = ((ax * s, ay * s), (bx * s, by * s), (cx * s, cy * s));
    let x0 = ax.min(bx).min(cx).floor() as i32;
    let x1 = ax.max(bx).max(cx).ceil() as i32;
    let y0 = ay.min(by).min(cy).floor() as i32;
    let y1 = ay.max(by).max(cy).ceil() as i32;
    for y in y0..y1 {
        for x in x0..x1 {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let d1 = sign(px, py, ax, ay, bx, by);
            let d2 = sign(px, py, bx, by, cx, cy);
            let d3 = sign(px, py, cx, cy, ax, ay);
            let neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
            let pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);
            if !(neg && pos) {
                blend_px(canvas, w, h, x, y, rgb, 1.0);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Text — same layout/blit approach as calendar_panel.rs (duplicated:
// main.rs's helpers are private and this module must stay drop-in).
// ---------------------------------------------------------------------------
struct TextRow {
    glyphs: Vec<(fontdue::Metrics, std::sync::Arc<[u8]>)>,
    width: f32,
}

fn layout_text(font: &fontdue::Font, text: &str, px: f32) -> TextRow {
    let mut glyphs = Vec::new();
    let mut width = 0.0f32;
    for ch in text.chars() {
        let (m, cov) = crate::lock_timer::rasterize_cached(font, ch, px);
        width += m.advance_width;
        glyphs.push((m, cov));
    }
    TextRow { glyphs, width }
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

/// Blit a laid-out row starting at pen x `x0`, baseline `baseline`.
fn blit_row(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    row: &TextRow,
    x0: f32,
    baseline: f32,
    rgb: [u8; 3],
) {
    let mut pen = x0;
    for (m, cov) in &row.glyphs {
        let gx0 = (pen + m.xmin as f32).round() as i32;
        let gy0 = (baseline - m.ymin as f32 - m.height as f32).round() as i32;
        blit_glyph(canvas, w, h, m, cov, gx0, gy0, rgb);
        pen += m.advance_width;
    }
}

/// Draw `text` with its LEFT edge at `x` and its cap-height centre at
/// `cy`. Quicksand cap height is ~0.71em, so the glyph band top sits
/// ~0.36em above the centre line (same convention as calendar_panel.rs).
fn draw_text_left_at(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &fontdue::Font,
    text: &str,
    px: f32,
    x: f32,
    cy: f32,
    rgb: [u8; 3],
) {
    let s = scale_of(w);
    let row = layout_text(font, text, px * s);
    // Anchor fix (iter13 — iter10 D1/D5 root cause): the old formula
    // `baseline = (cy - 0.36*px) + ascent` added the full line ascent
    // (Quicksand-Bold hhea ascent = 1.00em) below the cap top, so every
    // line rendered ~0.29*px too low (clock cap centre 64.7 vs s04 58.8
    // at 38 px). The baseline sits half a cap height (0.36em) below the
    // cap centre — no ascent term. (The iter10 fixer documented this fix
    // but only changed the constants; the code still had the old formula
    // until this edit — deploying the constants alone would have pushed
    // the clock to ~68.7.)
    let baseline = (cy + px * 0.36) * s;
    blit_row(canvas, w, h, &row, x * s, baseline, rgb);
}

/// Draw `text` with its RIGHT edge at `x` and its cap-height centre at `cy`.
fn draw_text_right_at(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &fontdue::Font,
    text: &str,
    px: f32,
    x: f32,
    cy: f32,
    rgb: [u8; 3],
) {
    let s = scale_of(w);
    let row = layout_text(font, text, px * s);
    // Same anchor fix as draw_text_left_at (iter13, iter10 D1/D5).
    let baseline = (cy + px * 0.36) * s;
    blit_row(canvas, w, h, &row, x * s - row.width, baseline, rgb);
}

/// Truncate text to a logical width, appending an ellipsis when needed.
fn truncate_to_width(font: &fontdue::Font, text: &str, px: f32, max_w: f32) -> String {
    if layout_text(font, text, px).width <= max_w {
        return text.to_string();
    }
    let ell_w = layout_text(font, "\u{2026}", px).width;
    let mut out = String::new();
    let mut w = 0.0f32;
    for ch in text.chars() {
        let adv = crate::lock_timer::rasterize_cached(font, ch, px).0.advance_width;
        if w + adv + ell_w > max_w {
            break;
        }
        out.push(ch);
        w += adv;
    }
    out.push('\u{2026}');
    out
}

/// Rasterize one Material Symbols glyph centred at (cx, cy).
fn draw_icon_glyph(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    icon: &fontdue::Font,
    ch: char,
    px: f32,
    cx: f32,
    cy: f32,
    rgb: [u8; 3],
) {
    let s = scale_of(w);
    let (cx, cy) = (cx * s, cy * s);
    let (m, cov) = crate::lock_timer::rasterize_cached(icon, ch, px * s);
    if m.width == 0 || m.height == 0 {
        return;
    }
    // Centre the glyph bitmap on (cx, cy) — same convention as
    // power_panel.rs draw_icon_glyph: the blit origin is the bitmap's
    // top-left, so xmin/ymin are NOT subtracted again here.
    let gx0 = (cx - m.width as f32 / 2.0).round() as i32;
    let gy0 = (cy - m.height as f32 / 2.0).round() as i32;
    blit_glyph(canvas, w, h, &m, &cov, gx0, gy0, rgb);
}

/// Pause as two SOLID bars (iter104 defect 6). The font pause glyph is
/// two hollow "00" rings, so it is never rasterised on the font path:
/// the bars below replace it. Geometry keeps the glyph's own footprint
/// scaled to 20.0 tall: each ring is 240/560 of the 560-wide bbox, so
/// each bar is 240/560 * 20 = 8.6 wide; the gap between the rings is
/// 80/560 * 20 = 2.9, i.e. bars at cx-10.0..cx-1.4 and cx+1.4..cx+10.0.
/// Height is exactly 20 pixel rows centred on cy: fill_rect
/// floors/ceils its float bounds, so y0 = cy-9.5 / y1 = cy+10.5 gives
/// 20 rows centred on cy (at CTRL_CY 130.5: rows 121..140). Colour is
/// the caller's ctrl_colour (WHITE when loaded, DIM_GREY when idle —
/// the idle state never reaches here, pause only shows while playing).
fn draw_pause_bars(canvas: &mut [u8], w: u32, h: u32, cx: f32, cy: f32, rgb: [u8; 3]) {
    fill_round_box(canvas, w, h, cx - 4.0, cy, 2.0, 8.0, 1.0, rgb);
    fill_round_box(canvas, w, h, cx + 4.0, cy, 2.0, 8.0, 1.0, rgb);
}

// ---------------------------------------------------------------------------
// Drawn fallbacks (used only when the icon font is not passed in)
// ---------------------------------------------------------------------------
/// Prev / play / pause / next as drawn shapes (no icon font).
/// `kind`: 0 = prev, 1 = play/pause, 2 = next. `playing` selects the pause
/// bars for kind 1. Icons span ~18 px around (cx, cy).
fn draw_control_fallback(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    kind: usize,
    playing: bool,
    cx: f32,
    cy: f32,
    rgb: [u8; 3],
) {
    let half = 8.0;
    match kind {
        0 => {
            // Prev: bar on the left + left-pointing triangle.
            fill_rect(canvas, w, h, cx - 10.0, cy - half, cx - 7.5, cy + half, rgb);
            fill_triangle(
                canvas, w, h, cx + 7.0, cy - half, cx + 7.0, cy + half, cx - 6.0, cy, rgb,
            );
        }
        2 => {
            // Next: right-pointing triangle + bar on the right.
            fill_triangle(
                canvas, w, h, cx - 7.0, cy - half, cx - 7.0, cy + half, cx + 6.0, cy, rgb,
            );
            fill_rect(canvas, w, h, cx + 7.5, cy - half, cx + 10.0, cy + half, rgb);
        }
        _ => {
            if playing {
                draw_pause_bars(canvas, w, h, cx, cy, rgb);
            } else {
                // Play: right-pointing triangle.
                fill_triangle(
                    canvas, w, h, cx - 7.0, cy - half, cx - 7.0, cy + half, cx + 9.0, cy, rgb,
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Panel content
// ---------------------------------------------------------------------------
pub fn draw_music_panel(
    canvas: &mut [u8], w: u32, h: u32, font: &fontdue::Font,
    icon_font: Option<&fontdue::Font>, data: &MusicData,
) {
    set_scale(w);
    let loaded = data.has_track();
    let left = text_left(data);
    let (r, g, b) = crate::library::theme_accent();
    let accent = [r, g, b];
    draw_text_left_at(canvas, w, h, font,
        if !loaded { "Music" } else if data.playing { "Playing" } else { "Paused" },
        10.5, left, 28.0, SECONDARY);
    draw_text_right_at(canvas, w, h, font, &crate::hhmm(), 12.0, RIGHT_INSET, 28.0, SECONDARY);

    if left == TEXT_WITH_ART {
        let cx = LEFT_INSET + ART_SIZE / 2.0;
        let cy = TILE_TOP + ART_SIZE / 2.0;
        fill_round_box(canvas, w, h, cx, cy, ART_SIZE / 2.0, ART_SIZE / 2.0, ART_RADIUS, ART_BORDER);
        if let Some(art) = &data.artwork {
            blit_artwork(canvas, w, h, art, LEFT_INSET, TILE_TOP, cx, cy);
        }
    }
    let title = data.title.as_deref().filter(|s| !s.is_empty()).unwrap_or(
        if loaded { "Unknown track" } else { "Nothing playing" });
    let artist = data.artist.as_deref().filter(|s| !s.is_empty()).unwrap_or(
        if loaded { "Unknown artist" } else { "No active media session" });
    draw_text_left_at(canvas, w, h, font,
        &truncate_to_width(font, title, TRACK_PX, RIGHT_INSET - left), TRACK_PX, left, 53.0, WHITE);
    draw_text_left_at(canvas, w, h, font,
        &truncate_to_width(font, artist, ARTIST_PX, RIGHT_INSET - left), ARTIST_PX, left, 76.0, SECONDARY);

    if loaded {
        fill_round_box(canvas, w, h, (left + PROGRESS_END) / 2.0, PROGRESS_CY,
            (PROGRESS_END - left) / 2.0, 1.5, 1.5, PROGRESS_TRACK);
        let fraction = if data.progress.is_finite() { data.progress.clamp(0.0, 1.0) } else { 0.0 };
        let end = left + (PROGRESS_END - left) * fraction;
        if end > left {
            fill_round_box(canvas, w, h, (left + end) / 2.0, PROGRESS_CY,
                (end - left) / 2.0, 1.5, 1.5, accent);
        }
        draw_text_left_at(canvas, w, h, font, &elapsed_text(data.position_secs), 10.0, left, 129.0, SECONDARY);
        if data.length_secs > 0 {
            draw_text_right_at(canvas, w, h, font, &elapsed_text(data.length_secs), 10.0,
                PROGRESS_END, 129.0, SECONDARY);
        }
    }

    for (i, &(cx, cy)) in CONTROL_CENTRES.iter().enumerate() {
        let bg = if i == 1 && loaded { accent } else { CONTROL_BG };
        let ink = if !loaded { DIM_GREY } else if i == 1 {
            let (r, g, b) = crate::theme::ink_on((bg[0], bg[1], bg[2]), (0, 0, 0));
            [r, g, b]
        } else { WHITE };
        fill_circle(canvas, w, h, cx, cy, if i == 1 { 20.0 } else { 18.0 }, bg);
        match icon_font {
            Some(_) if i == 1 && data.playing => draw_pause_bars(canvas, w, h, cx, cy, ink),
            Some(icon) => draw_icon_glyph(canvas, w, h, icon,
                [ICON_SKIP_PREV, ICON_PLAY, ICON_SKIP_NEXT][i],
                if i == 1 { CTRL_ICON_EM_PLAY } else { CTRL_ICON_EM_SKIP }, cx, cy, ink),
            None => draw_control_fallback(canvas, w, h, i, data.playing, cx, cy, ink),
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
        panic!("no Quicksand font found under ~/.local/share/fonts");
    }

    #[test]
    fn parse_playerctl_playing_and_paused() {
        let d = parse_playerctl(Some("Playing"), "Nirvana|Come As You Are|60000000|180000000");
        assert!(d.playing);
        assert_eq!(d.artist.as_deref(), Some("Nirvana"));
        assert_eq!(d.title.as_deref(), Some("Come As You Are"));
        assert!((d.progress - 1.0 / 3.0).abs() < 0.01);
        let d = parse_playerctl(Some("Paused"), "Nirvana|Come As You Are|90000000|180000000");
        assert!(!d.playing);
        assert!((d.progress - 0.5).abs() < 0.01);
    }

    #[test]
    fn parse_playerctl_position_is_microseconds() {
        // iter86 (scout bug): playerctl 2.4.1 returns {{position}} in
        // MICROSECONDS. 30,000,000 us of 180,000,000 us = 1/6; the old
        // seconds assumption read it as 30,000,000 s and clamped to 1.0.
        let d = parse_playerctl(Some("Playing"), "Nirvana|Come As You Are|30000000|180000000");
        assert!((d.progress - 1.0 / 6.0).abs() < 0.01);
    }

    #[test]
    fn parse_playerctl_idle_cases() {
        assert_eq!(parse_playerctl(None, "X|Y|1.0|1000000").artist, None);
        assert_eq!(parse_playerctl(Some("Stopped"), "||0.0|0").artist, None);
        assert_eq!(parse_playerctl(Some("Playing"), "||0.0|0").title, None);
    }

    fn px_at(canvas: &[u8], w: u32, x: u32, y: u32) -> [u8; 3] {
        let i = ((y * w + x) * 4) as usize;
        [canvas[i + 2], canvas[i + 1], canvas[i]]
    }

    #[test]
    fn transport_hits_match_the_three_visible_buttons() {
        let origin = (282.0, 3.0);
        for (i, &(cx, cy)) in CONTROL_CENTRES.iter().enumerate() {
            assert_eq!(control_hit((f64::from(origin.0 + cx), f64::from(origin.1 + cy)), origin), Some(i as u8));
        }
        assert_eq!(control_hit((787.0, 115.0), origin), None);
        assert_eq!(control_hit((809.0, 80.0), origin), None);
    }

    #[test]
    fn music_panel_draws_one_set_of_scaled_controls_and_artwork() {
        let font = load_test_font();
        let icon = crate::load_icon_font().expect("embedded icon font");
        let data = MusicData::playing("Artist", "Title", 0.65)
            .with_artwork(Some([200, 30, 40, 255].repeat(ART_THUMB * ART_THUMB)));
        for scale in [1, 2] {
            let (w, h) = (PANEL_W * scale, PANEL_H * scale);
            let mut canvas = vec![0; (w * h * 4) as usize];
            draw_music_panel(&mut canvas, w, h, &font, Some(&icon), &MusicData::idle());
            assert_eq!(px_at(&canvas, w, 200 * scale, 108 * scale), [0; 3]);
            canvas.fill(0);
            draw_music_panel(&mut canvas, w, h, &font, Some(&icon), &data);
            for (x, y) in [(36, 42), (108, 110)] {
                assert_eq!(px_at(&canvas, w, x * scale, y * scale), [200, 30, 40]);
            }
            assert_eq!(px_at(&canvas, w, 24 * scale, 27 * scale), [0; 3]);
            assert_eq!(px_at(&canvas, w, 420 * scale, 108 * scale), PROGRESS_TRACK);
            for (i, &(cx, cy)) in CONTROL_CENTRES.iter().enumerate() {
                let (cx, cy) = (cx as u32 * scale, cy as u32 * scale);
                let ink = if i == 1 {
                    let (r, g, b) = crate::theme::ink_on(crate::library::theme_accent(), (0, 0, 0));
                    [r, g, b]
                } else { WHITE };
                let count = (cy - 10 * scale..cy + 10 * scale).flat_map(|y| {
                    (cx - 10 * scale..cx + 10 * scale).map(move |x| (x, y))
                }).filter(|&(x, y)| px_at(&canvas, w, x, y) == ink).count();
                assert!(count > (24 * scale * scale) as usize, "glyph {i} at {scale}x: {count} pixels");
            }
        }
    }

    #[test]
    fn long_title_fits_its_column() {
        let font = load_test_font();
        set_scale(PANEL_W);
        let title = truncate_to_width(&font, "A very long title that cannot fit into the track area", TRACK_PX, 150.0);
        assert!(title.ends_with('\u{2026}'));
        assert!(layout_text(&font, &title, TRACK_PX).width <= 150.0);
    }

    #[test]
    fn parse_playerctl_art_url() {
        let d = parse_playerctl(
            Some("Playing"),
            "Nirvana|Come As You Are|60000000|180000000|file:///home/alex/cover.jpg",
        );
        assert!(d.playing);
        assert_eq!(
            d.art_url.as_deref(),
            Some("file:///home/alex/cover.jpg")
        );
        assert!(d.artwork.is_none());
        // Empty artUrl = no art (player exposes none).
        let d = parse_playerctl(
            Some("Playing"),
            "Nirvana|Come As You Are|0|180000000|",
        );
        assert_eq!(d.art_url, None);
        // Old 4-field format (no artUrl) still parses.
        let d = parse_playerctl(Some("Playing"), "Nirvana|Come As You Are|0|180000000");
        assert_eq!(d.art_url, None);
        // Idle has no artUrl either.
        assert_eq!(
            parse_playerctl(None, "X|Y|0|1|file:///x.png").art_url,
            None
        );
    }

    /// Encode a small RGBA image as PNG (test fixture builder).
    fn encode_test_png_rgba(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut buf, w, h);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut wtr = enc.write_header().unwrap();
            wtr.write_image_data(rgba).unwrap();
        }
        buf
    }

    #[test]
    fn decode_png_artwork_cover_fit() {
        // 4x2 RGBA: left half red, right half blue -> cover-fit
        // centre-square crop (2x2: red | blue) then 36x36: left
        // edge red, right edge blue, fully opaque.
        let mut rgba = Vec::new();
        for _ in 0..2 {
            rgba.extend_from_slice(&[
                255, 0, 0, 255, 255, 0, 0, 255, 0, 0, 255, 255, 0, 0, 255, 255,
            ]);
        }
        let png = encode_test_png_rgba(4, 2, &rgba);
        let thumb = decode_artwork_thumb(&png).expect("png thumb");
        assert_eq!(thumb.len(), ART_THUMB * ART_THUMB * 4);
        let mid = (ART_THUMB / 2 * ART_THUMB) * 4;
        assert_eq!(&thumb[mid..mid + 3], &[255, 0, 0]);
        assert_eq!(&thumb[mid + (ART_THUMB - 1) * 4..mid + (ART_THUMB - 1) * 4 + 3], &[0, 0, 255]);
        assert_eq!(thumb[mid + 3], 255);
        // Non-image bytes decode to None (no tile, never a stamp).
        assert!(decode_artwork_thumb(b"not an image").is_none());
        assert!(decode_artwork_thumb(&[0xff, 0xd8, 0xff, 0xe0, 0, 1, 2, 3]).is_none());
    }

    #[test]
    fn decode_jpeg_artwork_fixture() {
        // 16x16 solid (200,30,40) JPEG, quality 95 — generated with
        // PIL on the Surface (see the iter104-B build note); lossy,
        // so assert with tolerance, not exact bytes.
        let jpg = include_bytes!("test-fixtures/artwork-test.jpg");
        let thumb = decode_artwork_thumb(jpg).expect("jpeg thumb");
        assert_eq!(thumb.len(), ART_THUMB * ART_THUMB * 4);
        let mid = (ART_THUMB / 2 * ART_THUMB + ART_THUMB / 2) * 4;
        let (r, g, b, a) = (
            thumb[mid] as i32,
            thumb[mid + 1] as i32,
            thumb[mid + 2] as i32,
            thumb[mid + 3],
        );
        assert!((r - 200).abs() <= 12, "red {r}");
        assert!((g - 30).abs() <= 12, "green {g}");
        assert!((b - 40).abs() <= 12, "blue {b}");
        assert_eq!(a, 255);
    }

    // iter106: the live-repaint fingerprint - pure over fixtures.
    // parse_playerctl is a pure parse and fingerprint() never
    // touches MUSIC_CACHE / live_cached() (which would spawn the
    // refresher thread and its playerctl polls), so no test below
    // ever spawns playerctl.
    #[test]
    fn fingerprint_position_seconds() {
        // 4 s of 200 s vs 5 s of 200 s: the position stepped.
        let a = parse_playerctl(Some("Playing"), "Alpha|One|4000000|200000000");
        let b = parse_playerctl(Some("Playing"), "Alpha|One|5000000|200000000");
        assert_eq!(a.position_secs, 4);
        assert_eq!(b.position_secs, 5);
        assert_eq!(a.length_secs, 200);
        assert_ne!(fingerprint(&a), fingerprint(&b));
    }

    #[test]
    fn fingerprint_stable_within_one_second() {
        // Same whole second while Playing: 4.1 s vs 4.9 s.
        let a = parse_playerctl(Some("Playing"), "Alpha|One|4100000|200000000");
        let b = parse_playerctl(Some("Playing"), "Alpha|One|4900000|200000000");
        assert_eq!(fingerprint(&a), fingerprint(&b));
        // Paused at a fixed position: two identical polls are
        // stable, so the 1 s timer never redraw-storms paused.
        let c = parse_playerctl(Some("Paused"), "Alpha|One|14000000|200000000");
        let d = parse_playerctl(Some("Paused"), "Alpha|One|14000000|200000000");
        assert_eq!(fingerprint(&c), fingerprint(&d));
    }

    #[test]
    fn fingerprint_changes_on_track_status_and_art() {
        let a = parse_playerctl(Some("Playing"), "Alpha|One|4000000|200000000");
        let b = parse_playerctl(Some("Playing"), "Beta|Two|4000000|200000000");
        assert_ne!(fingerprint(&a), fingerprint(&b), "track change");
        let c = parse_playerctl(Some("Paused"), "Alpha|One|4000000|200000000");
        assert_ne!(fingerprint(&a), fingerprint(&c), "play -> pause");
        // Same track, same second, different artUrl: artwork
        // arrival must redraw even if nothing else moved.
        let d = parse_playerctl(
            Some("Playing"),
            "Alpha|One|4000000|200000000|file:///cover.jpg",
        );
        assert_ne!(fingerprint(&a), fingerprint(&d), "artUrl change");
    }

    #[test]
    fn fingerprint_idle_stable() {
        assert_eq!(
            fingerprint(&MusicData::idle()),
            fingerprint(&MusicData::idle())
        );
        // No player at all parses to the same idle fingerprint.
        let d = parse_playerctl(None, "X|Y|1000000|200000000");
        assert_eq!(fingerprint(&d), fingerprint(&MusicData::idle()));
    }

    // iter114 W2: player selection — pure functions only (no
    // playerctl spawn; cf. the fingerprint tests above).
    fn statuses(
        pairs: &[(&str, Option<&str>)],
    ) -> std::collections::HashMap<String, Option<String>> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.map(|s| s.to_string())))
            .collect()
    }

    #[test]
    fn score_status_ordering() {
        assert_eq!(score_status(Some("Playing")), 3);
        assert_eq!(score_status(Some("Paused")), 2);
        assert_eq!(score_status(Some("Stopped")), 1);
        assert_eq!(score_status(Some("Other")), 0);
        assert_eq!(score_status(None), 0);
        assert!(score_status(Some("Playing")) > score_status(Some("Paused")));
        assert!(score_status(Some("Paused")) > score_status(Some("Stopped")));
    }

    #[test]
    fn select_playing_beats_paused() {
        // NectarLink Paused + stub Playing => stub wins (the iter110
        // defect: bare playerctl could pick either; explicit
        // selection must pick the Playing one).
        let players = vec!["aaa_stub".to_string(), "nectarlink".to_string()];
        let st = statuses(&[
            ("aaa_stub", Some("Playing")),
            ("nectarlink", Some("Paused")),
        ]);
        assert_eq!(
            select_player(&None, &players, &st).as_deref(),
            Some("aaa_stub")
        );
        // Reversed list order must not change the winner.
        let players = vec!["nectarlink".to_string(), "aaa_stub".to_string()];
        assert_eq!(
            select_player(&None, &players, &st).as_deref(),
            Some("aaa_stub")
        );
    }

    #[test]
    fn select_sticky_wins_ties() {
        // Both Paused: the sticky player stays selected (no flapping
        // between two idle players on every poll).
        let players = vec!["aaa_stub".to_string(), "nectarlink".to_string()];
        let st = statuses(&[
            ("aaa_stub", Some("Paused")),
            ("nectarlink", Some("Paused")),
        ]);
        assert_eq!(
            select_player(&Some("nectarlink".to_string()), &players, &st).as_deref(),
            Some("nectarlink")
        );
        // No sticky: first in list order.
        assert_eq!(
            select_player(&None, &players, &st).as_deref(),
            Some("aaa_stub")
        );
    }

    #[test]
    fn select_sticky_keep_alive() {
        // Both status calls failed this poll, but the sticky player
        // is still listed: keep it rather than dropping to idle for
        // one poll (the next poll re-scores).
        let players = vec!["aaa_stub".to_string(), "nectarlink".to_string()];
        let st = statuses(&[("aaa_stub", None), ("nectarlink", None)]);
        assert_eq!(
            select_player(&Some("nectarlink".to_string()), &players, &st).as_deref(),
            Some("nectarlink")
        );
        // No sticky to keep alive => None (transient, caller keeps
        // the last good snapshot).
        assert_eq!(select_player(&None, &players, &st), None);
        // Sticky NOT listed anymore => no keep-alive either.
        let players = vec!["aaa_stub".to_string()];
        assert_eq!(
            select_player(&Some("nectarlink".to_string()), &players, &st),
            None
        );
    }

    #[test]
    fn select_vanished_player_reselects() {
        // Sticky player gone from the list => fall back to the
        // remaining player.
        let players = vec!["nectarlink".to_string()];
        let st = statuses(&[("nectarlink", Some("Paused"))]);
        assert_eq!(
            select_player(&Some("aaa_stub".to_string()), &players, &st).as_deref(),
            Some("nectarlink")
        );
        // Empty list => None (honest idle).
        let empty: Vec<String> = vec![];
        assert_eq!(select_player(&None, &empty, &st), None);
    }

    #[test]
    fn metadata_failure_stopped_means_idle() {
        // A Stopped player whose metadata call failed has no track:
        // honest idle, not keep-last-good (else the panel sticks on
        // a stale track forever once the stub exits and only
        // NectarLink Stopped is left).
        assert!(no_track_on_metadata_failure(Some("Stopped")));
        // Playing/Paused/unknown: a track exists, the call just
        // failed -- transient, keep the last good snapshot.
        assert!(!no_track_on_metadata_failure(Some("Playing")));
        assert!(!no_track_on_metadata_failure(Some("Paused")));
        assert!(!no_track_on_metadata_failure(None));
    }
}
