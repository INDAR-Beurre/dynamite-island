//! Wallpaper + theme library (iter36, item a6b).
//!
//! Reads `~/.config/dynamite-rust/library/themes.json` (v2 grouped
//! schema — 24 themes, 56 wallpapers, deployed from
//! `~/workspace/hyprland-replica/wallpaper-library/`) plus the full-size
//! images under `library/images/` and pre-rendered RGBA thumbs under
//! `library/thumbs/` (`<stem>-154.rgba` 154x96, `<stem>-225.rgba`
//! 225x138, cover-fit LANCZOS, rounded-corner alpha baked in — same
//! format as the compiled-in `wallpaper-thumbs/*.rgba`).
//!
//! No new crates: themes.json is parsed by a minimal JSON reader for
//! this fixed schema (objects / arrays / strings only). All file I/O
//! happens on the press / socket path, never per frame: the manifest is
//! loaded once (OnceLock), thumbs are cached in memory.
//!
//! Applying a wallpaper (the `wallpaper.png` mechanism) copies the
//! chosen library image over `~/.config/mango/wallpaper/wallpaper.png`
//! (the current one is preserved in `wallpaper.png.bak-iter36` on the
//! first apply), restarts swaybg detached, persists `wallpaper=` +
//! `theme=` in `config.conf`. The island accent is a FIXED periwinkle
//! #7CA2F5 (see `ISLAND_ACCENT`) — it is NOT derived from any theme
//! palette: palette slot 3 is arbitrary per theme (everblush green
//! #8CCF7E) and only feeds the themes-panel dot rows. Applying a theme
//! persists `theme=`; it does NOT change the wallpaper.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

/// Fixed island accent — periwinkle (121, 161, 245) #79A1F5.
/// NOT derived from any theme palette: theme palette slot 3 is
/// arbitrary per theme (everblush green #8CCF7E, violet-tide slate
/// #2E4580) and must never drive the island again. This is both the
/// fallback and the live accent (see [`ISLAND_ACCENT`]) — keep the two
/// equal: ONE accent value, not one per theme.
/// r1d finding 4: the reference's active power disc reads
/// (121, 161, 245); the old (124, 162, 245) was 3/1/0 too bright.
pub const DEFAULT_ACCENT: (u8, u8, u8) = (121, 161, 245);

/// Alias for the fixed island accent — the single source of truth for
/// the live accent colour. [`accent_for_theme`] returns this for every
/// theme name, known or unknown.
pub const ISLAND_ACCENT: (u8, u8, u8) = DEFAULT_ACCENT;

/// Live island accent, always the fixed periwinkle [`ISLAND_ACCENT`].
/// Panels read it via [`theme_accent()`] (power centre disc,
/// Control Centre toggle-on + slider fills, wallpaper outline).
static ACCENT: RwLock<(u8, u8, u8)> = RwLock::new(DEFAULT_ACCENT);

pub fn theme_accent() -> (u8, u8, u8) {
    *ACCENT.read().unwrap()
}

pub fn set_theme_accent(rgb: (u8, u8, u8)) {
    *ACCENT.write().unwrap() = rgb;
}

pub fn library_dir() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    home.join(".config/dynamite-rust/library")
}

fn wallpaper_png_path() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    home.join(".config/mango/wallpaper/wallpaper.png")
}

// ---------------------------------------------------------------------------
// Minimal JSON reader (objects / arrays / strings only — the themes.json
// schema has no numbers, booleans or nulls).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum JVal {
    Str(String),
    Arr(Vec<JVal>),
    Obj(Vec<(String, JVal)>),
    /// Numbers / true / false / null — carried, never read by the
    /// manifest (e.g. `"version": 2` in themes.json).
    Other(String),
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.s.get(self.i).copied()
    }

    fn expect(&mut self, b: u8) -> Option<()> {
        if self.peek() == Some(b) {
            self.i += 1;
            Some(())
        } else {
            None
        }
    }

    fn string(&mut self) -> Option<String> {
        self.expect(b'"')?;
        let mut out = String::new();
        while let Some(&c) = self.s.get(self.i) {
            self.i += 1;
            match c {
                b'"' => return Some(out),
                b'\\' => {
                    let e = *self.s.get(self.i)?;
                    self.i += 1;
                    out.push(match e {
                        b'n' => '\n',
                        b't' => '\t',
                        b'r' => '\r',
                        b'\\' => '\\',
                        b'"' => '"',
                        b'/' => '/',
                        _ => return None,
                    });
                }
                _ => out.push(c as char),
            }
        }
        None
    }

    fn value(&mut self) -> Option<JVal> {
        match self.peek()? {
            b'"' => Some(JVal::Str(self.string()?)),
            b'0'..=b'9' | b'-' => {
                let start = self.i;
                while self.i < self.s.len()
                    && matches!(self.s[self.i], b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-')
                {
                    self.i += 1;
                }
                Some(JVal::Other(
                    String::from_utf8_lossy(&self.s[start..self.i]).into_owned(),
                ))
            }
            b't' | b'f' | b'n' => {
                let start = self.i;
                while self.i < self.s.len() && self.s[self.i].is_ascii_alphabetic() {
                    self.i += 1;
                }
                Some(JVal::Other(
                    String::from_utf8_lossy(&self.s[start..self.i]).into_owned(),
                ))
            }
            b'[' => {
                self.i += 1;
                let mut items = Vec::new();
                if self.peek() == Some(b']') {
                    self.i += 1;
                    return Some(JVal::Arr(items));
                }
                loop {
                    items.push(self.value()?);
                    match self.peek()? {
                        b',' => {
                            self.i += 1;
                        }
                        b']' => {
                            self.i += 1;
                            return Some(JVal::Arr(items));
                        }
                        _ => return None,
                    }
                }
            }
            b'{' => {
                self.i += 1;
                let mut fields = Vec::new();
                if self.peek() == Some(b'}') {
                    self.i += 1;
                    return Some(JVal::Obj(fields));
                }
                loop {
                    let k = self.string()?;
                    self.expect(b':')?;
                    let v = self.value()?;
                    fields.push((k, v));
                    match self.peek()? {
                        b',' => {
                            self.i += 1;
                        }
                        b'}' => {
                            self.i += 1;
                            return Some(JVal::Obj(fields));
                        }
                        _ => return None,
                    }
                }
            }
            _ => None,
        }
    }
}

fn parse_hex6(s: &str) -> Option<(u8, u8, u8)> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(h, 16).ok()?;
    Some(((n >> 16) as u8, ((n >> 8) & 0xff) as u8, (n & 0xff) as u8))
}

// ---------------------------------------------------------------------------
// Manifest
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    /// 6-colour palette; slot 3 (index 2) feeds the themes-panel dot
    /// row only — it is NOT the island accent (see [`ISLAND_ACCENT`]).
    pub palette: [(u8, u8, u8); 6],
    pub wallpapers: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Manifest {
    pub themes: Vec<Theme>,
}

impl Manifest {
    /// Global wallpaper order: themes in file order, wallpapers in
    /// listed order. Index `i` -> (theme name, filename).
    pub fn wallpaper_at(&self, i: usize) -> Option<(&str, &str)> {
        let mut n = i;
        for t in &self.themes {
            if n < t.wallpapers.len() {
                return Some((t.name.as_str(), t.wallpapers[n].as_str()));
            }
            n -= t.wallpapers.len();
        }
        None
    }

    pub fn wallpaper_count(&self) -> usize {
        self.themes.iter().map(|t| t.wallpapers.len()).sum()
    }

    /// Owning theme of a wallpaper filename.
    pub fn theme_of_wallpaper(&self, filename: &str) -> Option<&str> {
        self.themes
            .iter()
            .find(|t| t.wallpapers.iter().any(|w| w == filename))
            .map(|t| t.name.as_str())
    }

    pub fn theme(&self, name: &str) -> Option<&Theme> {
        self.themes.iter().find(|t| t.name == name)
    }

    /// Global index of a wallpaper filename, or None.
    pub fn index_of_wallpaper(&self, filename: &str) -> Option<usize> {
        let mut base = 0;
        for t in &self.themes {
            if let Some(pos) = t.wallpapers.iter().position(|w| w == filename) {
                return Some(base + pos);
            }
            base += t.wallpapers.len();
        }
        None
    }

    pub fn index_of_theme(&self, name: &str) -> Option<usize> {
        self.themes.iter().position(|t| t.name == name)
    }
}

/// Parse themes.json text (v2 grouped schema) into a [`Manifest`].
/// Returns None on any shape error — callers fall back to the
/// compiled-in panels instead of crashing on a bad file.
pub fn parse_manifest(text: &str) -> Option<Manifest> {
    let mut p = Parser {
        s: text.as_bytes(),
        i: 0,
    };
    let root = p.value()?;
    let JVal::Obj(fields) = root else {
        return None;
    };
    let themes_val = fields.iter().find(|(k, _)| k == "themes")?.1.clone();
    let JVal::Obj(theme_fields) = themes_val else {
        return None;
    };
    let mut themes = Vec::new();
    for (name, def) in theme_fields {
        let JVal::Obj(def_fields) = def else {
            return None;
        };
        let palette_val = def_fields.iter().find(|(k, _)| k == "palette")?.1.clone();
        let wallpapers_val = def_fields
            .iter()
            .find(|(k, _)| k == "wallpapers")?
            .1
            .clone();
        let JVal::Arr(palette_items) = palette_val else {
            return None;
        };
        let JVal::Arr(wallpaper_items) = wallpapers_val else {
            return None;
        };
        if palette_items.len() != 6 {
            return None;
        }
        let mut palette = [(0u8, 0u8, 0u8); 6];
        for (i, item) in palette_items.iter().enumerate() {
            let JVal::Str(hex) = item else {
                return None;
            };
            palette[i] = parse_hex6(hex)?;
        }
        let mut wallpapers = Vec::new();
        for item in &wallpaper_items {
            let JVal::Str(f) = item else {
                return None;
            };
            wallpapers.push(f.clone());
        }
        themes.push(Theme {
            name,
            palette,
            wallpapers,
        });
    }
    if themes.is_empty() {
        return None;
    }
    Some(Manifest { themes })
}

/// The loaded manifest, or None when the library is not deployed (yet).
/// Loaded once; the island restarts to pick up a new library.
pub fn manifest() -> Option<&'static Manifest> {
    static MANIFEST: OnceLock<Option<Manifest>> = OnceLock::new();
    MANIFEST
        .get_or_init(|| {
            std::fs::read_to_string(library_dir().join("themes.json"))
                .ok()
                .and_then(|t| parse_manifest(&t))
        })
        .as_ref()
}

/// Fixed island accent for a theme name — always [`ISLAND_ACCENT`]
/// (#7CA2F5), regardless of the theme. The name is accepted (and
/// ignored) so the `apply_theme` / `apply_wallpaper` / refresh call
/// sites need no edits; theme palettes still render in the themes
/// panel dot rows, but they never drive the island accent.
pub fn accent_for_theme(name: &str) -> (u8, u8, u8) {
    let _ = name;
    ISLAND_ACCENT
}

/// Apply the fixed island accent (startup / `reload`). The config
/// theme is still read (empty resolves to "anime") so the selected
/// theme persists, but it no longer affects the accent colour.
pub fn refresh_accent_from_config() {
    let theme = crate::cfg().theme.clone();
    let theme = if theme.is_empty() { "anime" } else { theme.as_str() };
    set_theme_accent(accent_for_theme(theme));
}

// ---------------------------------------------------------------------------
// Thumbnails — raw RGBA files under library/thumbs/, cached in memory.
// ---------------------------------------------------------------------------

/// Bounded thumbnail cache (iter85 RAM fix): the old cache was an
/// unbounded `HashMap` — every wallpaper thumb ever shown stayed
/// resident. LRU, max 64 entries and max 4 MiB total (a 225 thumb is
/// 124 KB, a 154 thumb 59 KB, so the byte cap binds first when the
/// library mixes sizes). `order` is LRU order, front = oldest.
struct ThumbCache {
    map: HashMap<(String, u32), Vec<u8>>,
    order: VecDeque<(String, u32)>,
    bytes: usize,
}

impl ThumbCache {
    fn new() -> Self {
        ThumbCache {
            map: HashMap::new(),
            order: VecDeque::new(),
            bytes: 0,
        }
    }

    /// Move `key` to the back (most-recently-used end).
    fn touch(&mut self, key: &(String, u32)) {
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            self.order.remove(pos);
        }
        self.order.push_back(key.clone());
    }

    /// Insert, evicting least-recently-used entries until both caps
    /// hold. A single thumb is far below the byte cap, so eviction
    /// always terminates with the new entry kept.
    fn insert_bounded(&mut self, key: (String, u32), bytes: Vec<u8>) {
        if let Some(old) = self.map.insert(key.clone(), bytes) {
            self.bytes -= old.len();
        }
        self.bytes += self.map[&key].len();
        self.touch(&key);
        while self.map.len() > MAX_THUMB_ENTRIES || self.bytes > MAX_THUMB_BYTES {
            let Some(old) = self.order.pop_front() else {
                break;
            };
            if let Some(v) = self.map.remove(&old) {
                self.bytes -= v.len();
            }
        }
    }
}

/// Max cached thumbnails (iter85 RAM fix).
const MAX_THUMB_ENTRIES: usize = 64;
/// Max cached thumbnail bytes total (iter85 RAM fix).
const MAX_THUMB_BYTES: usize = 4 * 1024 * 1024;

static THUMBS: OnceLock<RwLock<ThumbCache>> = OnceLock::new();

fn thumbs() -> &'static RwLock<ThumbCache> {
    THUMBS.get_or_init(|| RwLock::new(ThumbCache::new()))
}

/// RGBA bytes for wallpaper `stem` at `size` (154 or 225). The file must
/// hold exactly size*height*4 bytes (154x96 / 225x138); anything else
/// is rejected so a truncated transfer can never render as garbage.
pub fn thumb_rgba(stem: &str, size: u32) -> Option<Vec<u8>> {
    let key = (stem.to_string(), size);
    {
        let mut cache = thumbs().write().unwrap();
        if let Some(hit) = cache.map.get(&key) {
            let hit = hit.clone();
            cache.touch(&key);
            return Some(hit);
        }
    }
    let want: usize = match size {
        154 => 154 * 96 * 4,
        225 => 225 * 138 * 4,
        _ => return None,
    };
    let path = library_dir()
        .join("thumbs")
        .join(format!("{stem}-{size}.rgba"));
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() != want {
        return None;
    }
    let mut cache = thumbs().write().unwrap();
    // Another thread may have inserted it while we read the file.
    if let Some(hit) = cache.map.get(&key) {
        let hit = hit.clone();
        cache.touch(&key);
        return Some(hit);
    }
    cache.insert_bounded(key, bytes.clone());
    Some(bytes)
}

// ---------------------------------------------------------------------------
// Config persistence — edit key=value lines in place, preserving the
// rest of the file (the settings app owns the other keys).
// ---------------------------------------------------------------------------

/// Pure string transform: replace or append `key=value` lines.
pub fn save_config_keys_str(text: &str, pairs: &[(&str, &str)]) -> String {
    let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    for (k, v) in pairs {
        let prefix = format!("{k}=");
        if let Some(line) = lines.iter_mut().find(|l| l.starts_with(&prefix)) {
            *line = format!("{k}={v}");
        } else {
            lines.push(format!("{k}={v}"));
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

pub fn save_config_keys(pairs: &[(&str, &str)]) -> std::io::Result<()> {
    let path = crate::config_path();
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    std::fs::write(path, save_config_keys_str(&text, pairs))
}

// ---------------------------------------------------------------------------
// Apply paths
// ---------------------------------------------------------------------------

/// Restart swaybg on the current wallpaper.png, detached: the island
/// never waits on it (freeze rule — no blocking command on the input
/// or render path).
fn restart_swaybg() {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let png = home.join(".config/mango/wallpaper/wallpaper.png");
    // r2 #10c: `pkill -x swaybg` killed ANY swaybg, including one the user
    // started with a different wallpaper or mode. Only restart when every
    // running swaybg is ours; if a foreign one is present, leave the
    // desktop alone rather than fight it (the file is already written, so
    // the user's own setter picks it up on its next reload).
    let listed = std::process::Command::new("pgrep")
        .args(["-a", "-x", "swaybg"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    if !may_restart_swaybg(&listed, &png.display().to_string()) {
        eprintln!(
            "dynamite-rust: another swaybg is running a different wallpaper; \
             leaving it alone (wallpaper.png updated)"
        );
        return;
    }
    let cmd = format!(
        "pkill -x swaybg; sleep 0.3; exec swaybg -i {} -m fill",
        png.display()
    );
    let _ = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("setsid -f sh -c {cmd:?} >/dev/null 2>&1"))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

/// True when restarting swaybg would only touch instances of OURS.
/// Pure so the ownership rule is testable without spawning processes
/// (a fake swaybg can't be simulated: `pkill -f` also matches the test
/// shell's own command line).
fn may_restart_swaybg(pgrep_output: &str, our_png: &str) -> bool {
    let needle = format!("-i {our_png}");
    pgrep_output.trim().is_empty() || pgrep_output.lines().all(|l| l.contains(&needle))
}

#[cfg(test)]
mod swaybg_ownership_tests {
    use super::may_restart_swaybg;
    const OURS: &str = "/home/u/.config/mango/wallpaper/wallpaper.png";

    #[test]
    fn no_swaybg_running_means_start_ours() {
        assert!(may_restart_swaybg("", OURS));
    }

    #[test]
    fn only_our_instance_may_be_restarted() {
        let out = format!("4242 swaybg -i {OURS} -m fill\n");
        assert!(may_restart_swaybg(&out, OURS));
    }

    #[test]
    fn a_foreign_wallpaper_is_left_alone() {
        assert!(!may_restart_swaybg("4242 swaybg -i /home/u/other.png -m fill\n", OURS));
    }

    #[test]
    fn a_mix_still_leaves_the_desktop_alone() {
        let out = format!("4242 swaybg -i {OURS} -m fill\n4343 swaybg -i /x.png\n");
        assert!(!may_restart_swaybg(&out, OURS));
    }
}

/// Apply library wallpaper `filename` for real: copy it over
/// wallpaper.png atomically (tmp + fsync + rename, iter89 F1;
/// current one preserved in `wallpaper.png.bak-iter36` on the first
/// apply), verify bytes (F3), persist `wallpaper=` + `theme=`
/// (owning theme, pure function of the filename — F5: the manifest
/// lookup is canonical, e.g. 3zmr6y => violet-tide), restart swaybg
/// strictly after the rename (F2), set the (fixed) accent. Returns the theme.
pub fn apply_wallpaper(filename: &str) -> Result<String, String> {
    let m = manifest().ok_or_else(|| "library manifest not loaded".to_string())?;
    let theme = m
        .theme_of_wallpaper(filename)
        .ok_or_else(|| format!("{filename} is not in the library"))?
        .to_string();
    let src = library_dir().join("images").join(filename);
    if !src.is_file() {
        return Err(format!("library image missing: {}", src.display()));
    }
    let dst = wallpaper_png_path();
    let bak = dst.with_file_name("wallpaper.png.bak-iter36");
    if !bak.exists() {
        let _ = std::fs::copy(&dst, &bak);
    }
    // iter89 F1: atomic copy — write to a temp file in the same
    // directory, fsync, then rename over wallpaper.png, so swaybg can
    // only ever read a complete file (a truncated JPEG read mid-copy
    // painted the iter37 grey strip).
    let tmp = dst.with_file_name(format!("wallpaper.png.tmp-{}", std::process::id()));
    std::fs::copy(&src, &tmp).map_err(|e| format!("copy failed: {e}"))?;
    let atomic = (|| -> std::io::Result<()> {
        let f = std::fs::File::open(&tmp)?;
        f.sync_all()?;
        std::fs::rename(&tmp, &dst)?;
        Ok(())
    })();
    if let Err(e) = atomic {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("atomic copy failed: {e}"));
    }
    // iter89 F3: verify the renamed file is byte-identical before
    // touching config or restarting swaybg; on failure leave config
    // untouched and report like the missing-file error.
    let same = std::fs::read(&dst)
        .map_err(|e| format!("verify read failed: {e}"))?
        == std::fs::read(&src).map_err(|e| format!("verify read failed: {e}"))?;
    if !same {
        return Err("post-copy verify mismatch".to_string());
    }
    // iter89 F2: strict order — rename, then config, then restart.
    let _ = save_config_keys(&[("wallpaper", filename), ("theme", theme.as_str())]);
    restart_swaybg();
    set_theme_accent(accent_for_theme(&theme));
    Ok(theme)
}

/// Apply theme `name` for real: set the fixed island accent
/// ([`ISLAND_ACCENT`]) and persist `theme=`. Does NOT change the
/// wallpaper.
pub fn apply_theme(name: &str) -> Result<(u8, u8, u8), String> {
    let m = manifest().ok_or_else(|| "library manifest not loaded".to_string())?;
    if m.theme(name).is_none() {
        return Err(format!("{name} is not in the library"));
    }
    let accent = accent_for_theme(name);
    set_theme_accent(accent);
    let _ = save_config_keys(&[("theme", name)]);
    Ok(accent)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r##"{
      "version": 2,
      "themes": {
        "anime": {
          "palette": ["#0A2A5E", "#0066CC", "#2B7DE9", "#66CCCC", "#A9CCF0", "#FFFFFF"],
          "wallpapers": ["wallhaven-k8jgl6.png", "wallhaven-72j21e.jpg"]
        },
        "sakura": {
          "palette": ["#111111", "#222222", "#FF88AA", "#444444", "#555555", "#666666"],
          "wallpapers": ["wallhaven-9mjoy1.png"]
        }
      }
    }"##;

    #[test]
    fn manifest_parses_grouped_schema() {
        let m = parse_manifest(SAMPLE).expect("parse");
        assert_eq!(m.themes.len(), 2);
        assert_eq!(m.themes[0].name, "anime");
        assert_eq!(m.themes[0].palette[2], (0x2B, 0x7D, 0xE9));
        assert_eq!(m.wallpaper_count(), 3);
        assert_eq!(m.wallpaper_at(0), Some(("anime", "wallhaven-k8jgl6.png")));
        assert_eq!(m.wallpaper_at(2), Some(("sakura", "wallhaven-9mjoy1.png")));
        assert_eq!(m.wallpaper_at(3), None);
        assert_eq!(m.theme_of_wallpaper("wallhaven-9mjoy1.png"), Some("sakura"));
        assert_eq!(m.index_of_wallpaper("wallhaven-72j21e.jpg"), Some(1));
        assert_eq!(m.index_of_theme("sakura"), Some(1));
    }

    #[test]
    fn manifest_rejects_bad_shapes() {
        assert!(parse_manifest("{}").is_none());
        assert!(parse_manifest("not json").is_none());
        assert!(parse_manifest(r#"{"themes": {}}"#).is_none());
        assert!(parse_manifest(
            r##"{"themes": {"x": {"palette": ["#FFFFFF"], "wallpapers": []}}}"##
        )
        .is_none());
    }

    #[test]
    fn hex_parses() {
        assert_eq!(parse_hex6("#2B7DE9"), Some((0x2B, 0x7D, 0xE9)));
        assert_eq!(parse_hex6("#ffffff"), Some((255, 255, 255)));
        assert_eq!(parse_hex6("2B7DE9"), None);
        assert_eq!(parse_hex6("#12345"), None);
    }

    #[test]
    fn accent_is_fixed_periwinkle_for_any_theme() {
        // iter96: the island accent is decoupled from theme palettes —
        // everblush slot 3 is mint green #8CCF7E and must never surface.
        // r1d: re-measured from the reference at (121,161,245) = #79A1F5.
        assert_eq!(accent_for_theme("everblush"), (121, 161, 245));
        assert_eq!(accent_for_theme("anime"), (121, 161, 245));
        assert_eq!(accent_for_theme("nonexistent"), (121, 161, 245));
        assert_eq!(ISLAND_ACCENT, DEFAULT_ACCENT);
    }

    #[test]
    fn config_keys_replace_and_append() {
        let text = "collapsed_width=76\ngap_top=3\n";
        let out = save_config_keys_str(text, &[("theme", "sakura"), ("gap_top", "3")]);
        assert!(out.contains("theme=sakura\n"));
        assert_eq!(out.matches("gap_top=").count(), 1);
        let out2 = save_config_keys_str(&out, &[("wallpaper", "wallhaven-9mjoy1.png")]);
        assert!(out2.contains("wallpaper=wallhaven-9mjoy1.png\n"));
        assert!(out2.contains("theme=sakura\n"));
    }
}
