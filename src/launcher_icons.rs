//! ITER98 — KEYGUARD REAL ICON; HWLOC CONFIRMED UNRESOLVABLE (14 Nov 2026, ~23:30 CET).
//! Keyguard now blits its REAL icon: 36/37 rows. The installed .desktop files
//! (/usr/share/applications/keyguard.desktop and ~/.local/share/applications/
//! keyguard.desktop) carry NO Icon= key, which is why iter86 found nothing —
//! but Keyguard ships as an AppImage (/home/artix/.local/opt/keyguard/
//! Keyguard.AppImage, v3.5.0) and the AppImage embeds its upstream icon:
//! Icon=com.artemchep.keyguard in its own com.artemchep.keyguard.desktop,
//! SVG usr/share/icons/hicolor/scalable/apps/com.artemchep.keyguard.svg
//! (5,353 bytes; .DirIcon / usr/lib/Keyguard.png 512x512 is the same logo
//! as a raster — SVG wins per the iter86 rule, so the blob below is the
//! SVG route: rsvg-convert 48x48 -> PIL LANCZOS 24x24, sha256 e1de214b...,
//! opaque 396/576, saturated 321). Blue rounded square, white padlock,
//! black shackle — visually verified from the 48 px render. No change to
//! the installed .desktop files was made (the island reads the embedded
//! blob, not Icon=, at runtime); installing the AppImage's own .desktop
//! would fix other launchers too, but that is out of scope here.
//!
//! Hardware Locality lstopo stays the SOLE fallback (1/37). Icon=hwloc
//! resolves nowhere: exhaustive 14 Nov 2026 sweep — no hwloc.* under
//! /usr/share/icons, /usr/share/pixmaps, ~/.local/share/icons, ~/.icons;
//! no hwloc*.png/.svg/.xpm/.ico anywhere on the filesystem (find / -xdev);
//! nothing in any icon-theme index.theme; the hwloc package itself ships
//! no icon (pacman -Ql hwloc: only binaries, headers, DTDs, a systemd
//! service and hwloc-ps.www assets). Upstream hwloc ships no icon in any
//! distro — the .desktop Icon=hwloc reference is dangling. Do NOT invent
//! a glyph for it; it keeps the drawn-folder fallback via app_icon_for().
//!
//! ITER86 — REAL ICONS FOR THE 37 INSTALLED APPS (14 Nov 2026).
//! The live launcher scans 37 .desktop files (67 files under
//! /usr/share/applications + ~/.local/share/applications, filtered to
//! 37 by scan_desktop_apps); before iter86 only the 6 video-set icons
//! above existed, so 36 of the 37 live rows fell back to the same
//! drawn white folder glyph. Now 35/37 rows blit their app's REAL
//! icon file (36/37 since iter98 — Keyguard resolved, see above): each Icon= resolved via the icon theme in use (Adwaita,
//! per gtk-icon-theme-name=Adwaita; hicolor / AdwaitaLegacy /
//! ~/.local/share/icons / /usr/share/pixmaps as fallbacks; absolute
//! Icon= paths used as-is), rasterized to 24x24 RGBA8 on the Surface
//! by `extract-installed-icons.py` (SVG -> rsvg-convert 48x48, PNG ->
//! PIL LANCZOS 24x24; scalable SVG preferred over any raster, largest
//! raster otherwise). The 29 new blobs live in src/icons/ alongside
//! the 6 video ones; Files reuses the existing files.rgba — a fresh
//! extraction of org.gnome.Nautilus.svg is byte-identical (sha256
//! 276f3dae...), so it was not rewritten.
//!
//! APPROACH — build-time extraction, NOT a runtime loader. The png
//! crate (0.17, vendored) IS a dependency since iter85, so a runtime
//! PNG loader is possible in principle — but 9 of the 36 icons are
//! SVG-only (Files/Nautilus, Proton VPN, qv4l2, qvidcap, Rofi,
//! VSCodium, Herdr TUI, ZapFast, Keyguard) and there is no SVG decoder in the
//! offline cargo cache, so a runtime loader would still need a second
//! build-time path for SVGs. One uniform path wins: 36 blobs,
//! 36 * 2304 = 82,944 bytes .rodata, demand-paged, zero runtime I/O
//! or heap. Cost of the choice: a newly installed app needs a
//! re-extraction run (script is checked in); the launcher already
//! rescans .desktop files, so the new row appears at once and only
//! its icon waits for the next extraction.
//!
//! STILL FALLBACK (1 row — no icon file exists anywhere on this
//! machine, verified by exhaustive filesystem search 14 Nov 2026,
//! see the ITER98 note above):
//!   Hardware Locality lstopo — Icon=hwloc, no hwloc.* under
//!     /usr/share/icons, /usr/share/pixmaps, ~/.local/share/icons,
//!     and no hwloc icon file anywhere on the filesystem at all.
//! It keeps the drawn-folder fallback via app_icon_for() in
//! launcher_panel.rs. Do NOT invent an icon for it. (Keyguard was
//! the second fallback in iter86 and is resolved since iter98.)
//!
//! MONOCHROME NOTE — six installed icons are near-zero saturation by
//! nature (their upstream icons are black/white line art), same as
//! auto-cpufreq/gimp in the video set: foot (sat 0, 19 shade bins),
//! herdr-tui (0), magpie (0), preferences-desktop (0), rofi (0),
//! zeron (0). Low saturation there is fidelity, not a defect — each
//! has 322–407 opaque px with real internal structure. qv4l2 and
//! qvidcap are byte-identical upstream (same sha256) — the two Qt
//! utilities genuinely share one icon file.
//!
//! launcher_icons — REAL app icons for the launcher, 24x24 RGBA8.
//!
//! The six icons are the apps in the VIDEO launcher frame (s06 /
//! f0180, settled launcher, seconds 5–6 of Iv8OVf-4Yxw), in display
//! order: auto-cpufreq, Bluetooth Manager, Brave, File Roller, Files,
//! GNU Image Manipulation Program (GIMP). This REPLACES the previous
//! QS-set version of this file (Foot / VSCodium / Files / magpie /
//! Display Scaler / NectarLink), which was for the wrong app set —
//! only Files overlapped — and was never referenced by any draw path
//! (critic-rust-launcher-iter10.md D1, CONDITIONAL FAIL).
//!
//! Each icon is the app's ACTUAL icon file, rasterized/downscaled to
//! 24x24 on the Surface and embedded here as raw bytes via
//! `include_bytes!`. The decode happened once, at extraction time,
//! and the island only blits bytes. (The png crate IS a dependency
//! since iter85, for clipboard thumbnails — but see the ITER86 note
//! above: SVG-only icons rule out a runtime-only loader.)
//!
//! EXTRACTION (13 Nov 2026 ~00:48–00:51 Surface time, run on the
//! Surface): script `rust-island/icons-src/extract-video-icons.py`
//! (checked in). Pipeline: SVG -> rsvg-convert 48x48 -> PIL RGBA ->
//! LANCZOS 24x24 -> /tmp/dynamite-icons-video/<name>.rgba (2304 bytes
//! each). The six .rgba files were copied back over SSH into
//! `rust-island/src/icons/` and their sha256 digests verified against
//! the Surface-side digests — every digest below matched on both
//! sides (re-verified 13 Nov 2026 ~01:00 CET by the iter13 fixer).
//! The upstream source files are still on the Surface in
//! /tmp/video-icons-src/ (autocpufreq.png, blueman.png, brave.png +
//! brave.svg, fileroller.svg, gimp.svg); Files uses the Surface's own
//! icon theme. Re-run the script after any icon change and update the
//! digests here.
//!
//! SOURCES — five of the six video apps are NOT installed on the
//! Surface (only org.gnome.Nautilus.desktop exists under
//! /usr/share/applications/), so their icons come from the apps'
//! OFFICIAL upstream icon files, not from the Surface theme:
//!   auto-cpufreq  upstream icon.png (AdnanHodzic/auto-cpufreq,
//!                 images/icon.png) — dark square, green graph line
//!   bluetooth     blueman.png (blueman-project/blueman,
//!                 data/icons/hicolor/48x48/apps/blueman.png) —
//!                 blue disc, white Bluetooth rune
//!   brave         product_logo_48.png (brave/brave-core,
//!                 app/theme/brave/product_logo_48.png) — orange/red
//!                 lion head. (The docs/images/brave.svg tried first
//!                 is a white-tile lion+wordmark lockup, rejected.)
//!   file-roller   org.gnome.FileRoller.svg (GNOME/file-roller,
//!                 data/icons/hicolor/scalable/apps/) — TAN archive
//!                 box. See HUE NOTE below.
//!   files         /usr/share/icons/hicolor/scalable/apps/
//!                 org.gnome.Nautilus.svg (Surface icon theme,
//!                 via rsvg-convert 48x48) — BLUE cabinet. Same
//!                 source and digest as the old QS-set files entry.
//!                 See HUE NOTE below.
//!   gimp          The_GIMP_icon_-_gnome.svg (Wikimedia Commons,
//!                 GNOME hicolor Wilber) — grey/brown Wilber face
//!
//! SATURATION (measured locally from the embedded blobs, opaque px
//! only, max-min channel > 60 — the critic's test):
//!   auto-cpufreq  opaque 170 / 576, saturated   5 (dark icon; the
//!                 green graph is thin lines, mostly washed out by
//!                 the 512->24 downscale — matches the video, whose
//!                 row-1 icon is also near-monochrome dark grey:
//!                 f0180 mean (38,39,43), sat 0 in the slot sample)
//!   bluetooth     opaque 318 / 576, saturated 243
//!   brave         opaque 363 / 576, saturated 265
//!   file-roller   opaque 396 / 576, saturated 351
//!   files         opaque 320 / 576, saturated 296
//!   gimp          opaque 178 / 576, saturated   8 (Wilber is
//!                 grey/brown line art — matches the video, whose
//!                 row-6 icon is grey-brown: f0180 mean (86,81,80))
//! A blit of any of these shows colour in the slot; the iter10
//! screenshot showed 0 saturated px in all six slots because the
//! panel drew glyphs instead. After wiring, the critic's per-slot
//! test must show >100 saturated px for bluetooth / brave /
//! file-roller / files, and a visible coloured mark for
//! auto-cpufreq / gimp (their real icons are near-monochrome by
//! nature — low saturation there is fidelity, not a defect).
//!
//! HUE NOTE — two icons are the app's real upstream icon but NOT the
//! exact hue the video shows (measured from frames/all/f0180.png,
//! tight 8x7 px icon boxes; the video's icon theme is Papirus per
//! analysis/saneaspect-research.md, which recolours both):
//!   file-roller: upstream is TAN (SVG fills #cdab8f #b5835a
//!     #986a44 — verified from the SVG on the Surface); the video
//!     shows GREEN (f0180 centre px e.g. (122,182,139),
//!     (159,230,168), G dominant by 40–70).
//!   files: upstream Nautilus is BLUE (mean (83,149,228)); the
//!     video shows a PALE GREY cabinet (f0180 centre px e.g.
//!     (191,196,157), (151,153,146), no blue at all).
//! The blobs below are still the right call for this iteration: they
//! are real icon files (the real-icons gate is about real vs drawn,
//! and both pass the saturated-px test), while drawing a green box /
//! grey cabinet by hand would put drawn glyphs back in. The exact
//! fix is a Papirus follow-up: fetch Papirus's
//! org.gnome.FileRoller + org.gnome.Nautilus SVGs, rasterize with
//! the same script, and swap those two blobs — do NOT guess the
//! Papirus hues from the 640x360 frame. The other four icons match
//! the video's hues: auto-cpufreq dark grey, bluetooth blue,
//! brave orange-red, gimp grey-brown.
//!
//! WIRING (integrator — see also launcher_panel.rs, which calls
//! `icon_bytes_for()` + `blit_icon()`):
//! ```ignore
//! // main.rs, with the other state-content mods:
//! mod launcher_icons;
//! // No call-site change: launcher_panel::draw_launcher_panel keeps
//! // its signature; it looks the icon up by app name and falls back
//! // to its drawn glyphs only when no blob exists for a row.
//! ```
//!
//! RAM: these are `&'static [u8; 2304]` in .rodata — 13,824 bytes
//! for the six video icons, 82,944 bytes for all 36 distinct blobs
//! (video 6 + installed 30; Files shared). They are demand-paged from
//! the executable, cost ~0 PSS until the launcher is first drawn, and
//! never grow: no heap allocation, no lazy loader, no file I/O at
//! runtime. Worst case the launcher state costs ~81 KB PSS over the
//! collapsed baseline — nothing against the ≤100 MB gate (live v0.28
//! PSS is ~58 MB). Do NOT "optimize" this into a runtime PNG loader:
//! that would ADD heap buffers and disk reads for zero visible
//! benefit — and it could not cover the SVG-only icons anyway.

pub const ICON_W: usize = 24;
pub const ICON_H: usize = 24;
/// Bytes per icon: 24 * 24 px * 4 (RGBA8, straight alpha).
pub const ICON_BYTES: usize = ICON_W * ICON_H * 4; // 2304

/// auto-cpufreq — upstream icon.png (512x512 source).
/// sha256 478ff9a1dbff89abc8201ef03d0c365d673950ffc2b05658f482a992478cbcee
pub const ICON_AUTO_CPUFREQ: &[u8; ICON_BYTES] =
    include_bytes!("icons/auto-cpufreq.rgba");
/// Bluetooth Manager (Blueman) — upstream blueman.png.
/// sha256 00f3bf1f1f87461ffcc235a874fbfd6c7426d884ae9c884cdf1be06a8f19a666
pub const ICON_BLUETOOTH: &[u8; ICON_BYTES] = include_bytes!("icons/bluetooth.rgba");
/// Brave — upstream product_logo_48.png (genuine Brave product icon).
/// sha256 837ea0afad0fad3ba1b5ee9b792179fa9c063a94f0040702b097ebab78b862b9
pub const ICON_BRAVE: &[u8; ICON_BYTES] = include_bytes!("icons/brave.rgba");
/// File Roller — upstream org.gnome.FileRoller.svg, rasterized.
/// TAN box (see HUE NOTE above — video shows the Papirus green).
/// sha256 f2319d03919f87db014e6e9c100b2e06dc3a2012c17327960734550bfe14edbc
pub const ICON_FILE_ROLLER: &[u8; ICON_BYTES] =
    include_bytes!("icons/file-roller.rgba");
/// Files (Nautilus) — Surface hicolor org.gnome.Nautilus.svg.
/// BLUE cabinet (see HUE NOTE above — video shows a pale grey one).
/// sha256 276f3daeebf874faa2cf15eaeed52601c048e5dffaf0296d9e533497d6a3d803
pub const ICON_FILES: &[u8; ICON_BYTES] = include_bytes!("icons/files.rgba");
/// GIMP (Wilber) — Wikimedia "The_GIMP_icon_-_gnome.svg".
/// sha256 437e1e66eb814c4e3272f6ad0012cb74cae5fde6ea257f6eaea2d5f578354689
pub const ICON_GIMP: &[u8; ICON_BYTES] = include_bytes!("icons/gimp.rgba");

/// Installed app (About Xfce) — /usr/share/icons/hicolor/scalable/apps/org.xfce.about.svg.
/// sha256 8cc0630708b1681211f9bec78c54a33032bdc67d4b21ce7cf7b329bc45445c29 (opaque 232/576, saturated 232)
pub const ICON_ABOUT_XFCE: &[u8; ICON_BYTES] =
    include_bytes!("icons/about-xfce.rgba");

/// Installed app (Antigravity) — /usr/share/pixmaps/antigravity.png.
/// sha256 8044b6b814374227dcccf9664350833b0d2fb00baf1b0a7f9cf0f098a6d7469c (opaque 368/576, saturated 71)
pub const ICON_ANTIGRAVITY: &[u8; ICON_BYTES] =
    include_bytes!("icons/antigravity.rgba");

/// Installed app (btop++) — /usr/share/icons/hicolor/scalable/apps/btop.svg.
/// sha256 72c315449446c0e147aae84fdb6e96b2b479fd370ca5774f26a27e48b4871433 (opaque 295/576, saturated 205)
pub const ICON_BTOP: &[u8; ICON_BYTES] =
    include_bytes!("icons/btop.rgba");

/// Installed app (CMake) — /usr/share/icons/hicolor/128x128/apps/CMakeSetup.png.
/// sha256 f02ee6c7c8ea2b97f414be176c10251d38e77dddbb194ce70808e26744cfbdd8 (opaque 280/576, saturated 260)
pub const ICON_CMAKE: &[u8; ICON_BYTES] =
    include_bytes!("icons/cmake.rgba");

/// Installed app (Manage Printing) — /usr/share/icons/hicolor/128x128/apps/cups.png.
/// sha256 a552d66fd4673b1a9b1fd0e8a9ac0eeea06917dbc6cb311e91d3893b020a64cf (opaque 572/576, saturated 500)
pub const ICON_CUPS: &[u8; ICON_BYTES] =
    include_bytes!("icons/cups.rgba");

/// Installed app (DeckCraft) — /home/alex/.local/share/icons/hicolor/scalable/apps/ai.storyteller.deckcraft.svg.
/// sha256 e55ddbde328c02702f2ccaa837024740fae815e6d7edbb5c86d4bbff89d35609 (opaque 472/576, saturated 319)
pub const ICON_DECKCRAFT: &[u8; ICON_BYTES] =
    include_bytes!("icons/deckcraft.rgba");

/// Installed app (Display Scaler) — /usr/share/icons/AdwaitaLegacy/48x48/legacy/preferences-desktop-display.png.
/// sha256 55312f9a1e084142d958cdbe7a53902ef116c1191114b68161c01fc4cd5d4abf (opaque 405/576, saturated 235)
pub const ICON_DISPLAY_SCALER: &[u8; ICON_BYTES] =
    include_bytes!("icons/display-scaler.rgba");

/// Installed app (Dynamite Settings) — /usr/share/icons/AdwaitaLegacy/48x48/legacy/preferences-system.png.
/// sha256 4554fb3faa60044ebd55d1c37043a4e55bc8ef3e2a28dd3d0b760df24762e7dd (opaque 162/576, saturated 41)
pub const ICON_DYNAMITE_SETTINGS: &[u8; ICON_BYTES] =
    include_bytes!("icons/dynamite-settings.rgba");

/// Installed app (Foot, Foot Client, Foot Server) — /usr/share/icons/hicolor/scalable/apps/foot.svg.
/// sha256 a4344c1ac93d84dbdc63d5ebfc16150cae96c374999aed9e24c865aba8a93bf0 (opaque 396/576, saturated 0)
pub const ICON_FOOT: &[u8; ICON_BYTES] =
    include_bytes!("icons/foot.rgba");

/// Installed app (GridCraft) — /home/alex/.local/share/icons/hicolor/256x256/apps/ai.storyteller.gridcraft.png.
/// sha256 7a36a31f51e5e11ecf7d03cd0a34077c08f07aa0d8edced8b067643edb9c41a4 (opaque 552/576, saturated 353)
pub const ICON_GRID_CRAFT: &[u8; ICON_BYTES] =
    include_bytes!("icons/gridcraft.rgba");

/// Installed app (Handy) — /home/alex/.local/share/icons/hicolor/128x128/apps/handy.png.
/// sha256 bdf9f5bf11e4613941c5a70843c806fae8e0db33a68984fae11103e384679351 (opaque 370/576, saturated 203)
pub const ICON_HANDY: &[u8; ICON_BYTES] =
    include_bytes!("icons/handy.rgba");

/// Installed app (Herdr TUI) — /home/alex/.local/share/icons/hicolor/scalable/apps/herdr-tui.svg.
/// sha256 62a752f65aa90a3efe14b95972de1529f987cbd8888baa795d521fedf18417e4 (opaque 396/576, saturated 0)
pub const ICON_HERDR_TUI: &[u8; ICON_BYTES] =
    include_bytes!("icons/herdr-tui.rgba");

/// Installed app (OpenJDK Java 21 Console, OpenJDK Java 21 Shell) — /usr/share/icons/hicolor/48x48/apps/java21-openjdk.png.
/// sha256 3a18bde09d226fc491e36e422acac17b9ebcd62ea49efbea78d1bcf1a3eb9586 (opaque 572/576, saturated 25)
pub const ICON_JAVA21: &[u8; ICON_BYTES] =
    include_bytes!("icons/java21-openjdk.rgba");

/// Installed app (Keyguard) — AppImage-embedded com.artemchep.keyguard.svg
/// (Keyguard.AppImage 3.5.0, usr/share/icons/hicolor/scalable/apps/).
/// sha256 e1de214b607cf3d4f66d50392308a711435d5f4f2c8ffa307ebc5f0a681cec5a (opaque 396/576, saturated 321)
pub const ICON_KEYGUARD: &[u8; ICON_BYTES] =
    include_bytes!("icons/keyguard.rgba");

/// Installed app (magpie) — /home/alex/.local/share/icons/hicolor/256x256/apps/magpie.png.
/// sha256 5b6c8e154e9bef2f4ad404b68ef67b750169ddcff5faa046036d92d9e8619a9c (opaque 368/576, saturated 0)
pub const ICON_MAGPIE: &[u8; ICON_BYTES] =
    include_bytes!("icons/magpie.rgba");

/// Installed app (Meiro) — /home/alex/.local/share/icons/meiro.png.
/// sha256 62ed6fbd0278b843129cf05d6eaede3b704a60e01c7b16e60cf1d83caa30998d (opaque 370/576, saturated 320)
pub const ICON_MEIRO: &[u8; ICON_BYTES] =
    include_bytes!("icons/meiro.rgba");

/// Installed app (NectarLink) — /home/alex/.local/share/icons/hicolor/256x256/apps/nectarlink.png.
/// sha256 1978828f9432166a87813ae1843a62257dd4ea5d86ec59a396b150800b433626 (opaque 460/576, saturated 131)
pub const ICON_NECTARLINK: &[u8; ICON_BYTES] =
    include_bytes!("icons/nectarlink.rgba");

/// Installed app (Preferred Applications) — /usr/share/icons/AdwaitaLegacy/48x48/legacy/preferences-desktop.png.
/// sha256 2e3da628d8f39f27a4f73695a7c5e13a7786914613f1d9e3b51738d8fbbcb437 (opaque 322/576, saturated 0)
pub const ICON_PREFERRED_APPS: &[u8; ICON_BYTES] =
    include_bytes!("icons/preferences-desktop.rgba");

/// Installed app (Proton VPN) — /usr/share/icons/hicolor/scalable/apps/proton-vpn-logo.svg.
/// sha256 797564573d2ff356901b17dd8000d19d7317154b23d3f8c0a2c5b0a9cee158f7 (opaque 536/576, saturated 105)
pub const ICON_PROTON_VPN: &[u8; ICON_BYTES] =
    include_bytes!("icons/proton-vpn.rgba");

/// Installed app (Qt V4L2 test Utility) — /usr/share/icons/hicolor/scalable/apps/qv4l2.svg.
/// sha256 c75a68f729945a01ec4482ca4f85278bd8abdc83705e247db191560fd4f78b0e (opaque 397/576, saturated 5)
pub const ICON_QV4L2: &[u8; ICON_BYTES] =
    include_bytes!("icons/qv4l2.rgba");

/// Installed app (Qt V4L2 video capture utility) — /usr/share/icons/hicolor/scalable/apps/qvidcap.svg.
/// sha256 c75a68f729945a01ec4482ca4f85278bd8abdc83705e247db191560fd4f78b0e (opaque 397/576, saturated 5)
pub const ICON_QVIDCAP: &[u8; ICON_BYTES] =
    include_bytes!("icons/qvidcap.rgba");

/// Installed app (Rofi, Rofi Theme Selector) — /usr/share/icons/hicolor/scalable/apps/rofi.svg.
/// sha256 9788f65b45c082a65c1387825212d23cc4a56ed36b7396f8a905d0534f4dcba0 (opaque 407/576, saturated 0)
pub const ICON_ROFI: &[u8; ICON_BYTES] =
    include_bytes!("icons/rofi.rgba");

/// Installed app (T3 Code) — /home/alex/.local/share/icons/hicolor/512x512/apps/t3-code.png.
/// sha256 3ad2e7fb5f23c9b05e25a8b689b4c77d2d19e58da9081d027691f78ee10578f9 (opaque 544/576, saturated 187)
pub const ICON_T3_CODE: &[u8; ICON_BYTES] =
    include_bytes!("icons/t3-code.rgba");

/// Installed app (VSCodium) — /usr/share/icons/hicolor/scalable/apps/vscodium.svg.
/// sha256 e11311b8e87744b722de3e42aefb58ca871ba59abcbfb731b95cd470463cee84 (opaque 400/576, saturated 113)
pub const ICON_VSCODIUM: &[u8; ICON_BYTES] =
    include_bytes!("icons/vscodium.rgba");

/// Installed app (Waterfox) — /home/artix/.local/opt/waterfox/browser/chrome/icons/default/default128.png.
/// sha256 4f76e00f8aad6ee8f94699e432f2129ed14b784561a66f1141d3b20bb6fff1b5 (opaque 265/576, saturated 257)
pub const ICON_WATERFOX: &[u8; ICON_BYTES] =
    include_bytes!("icons/waterfox.rgba");

/// Installed app (WordCraft) — /home/alex/.local/share/icons/hicolor/scalable/apps/ai.storyteller.wordcraft.svg.
/// sha256 4a021e143baef6bbc6430bb8721ae3755079753ecbf49946dc193599c012ebae (opaque 552/576, saturated 369)
pub const ICON_WORDCRAFT: &[u8; ICON_BYTES] =
    include_bytes!("icons/wordcraft.rgba");

/// Installed app (xgps, xgpsspeed) — /usr/share/gpsd/icons/gpsd-logo.png.
/// sha256 185d2a8a4783317a3048458083992126c0df3d5cf00d2c49f9c6474eae664637 (opaque 226/576, saturated 51)
pub const ICON_XGPS: &[u8; ICON_BYTES] =
    include_bytes!("icons/xgps.rgba");

/// Installed app (Yazi File Manager) — /usr/share/icons/hicolor/512x512/apps/yazi.png.
/// sha256 b12a58dfc02fa0fe928b0605d51e049bba011c34b5b87854c5734f5645b460a9 (opaque 420/576, saturated 367)
pub const ICON_YAZI: &[u8; ICON_BYTES] =
    include_bytes!("icons/yazi.rgba");

/// Installed app (ZapFast) — /home/alex/.local/share/icons/hicolor/scalable/apps/zapfast.svg.
/// sha256 26dbb5538c29008b2b4ba6e253f269ed040a683ceef2c06f2b5ce900af92cad3 (opaque 432/576, saturated 377)
pub const ICON_ZAPFAST: &[u8; ICON_BYTES] =
    include_bytes!("icons/zapfast.rgba");

/// Installed app (Zeron) — /home/artix/.zeron/app/current/zeron.png.
/// sha256 92784a708810ec0884124901721ef51f3af233d2ce6ac07d6991e0e2bce64009 (opaque 388/576, saturated 0)
pub const ICON_ZERON: &[u8; ICON_BYTES] =
    include_bytes!("icons/zeron.rgba");

/// Look up an icon by the row's display name, as shown in the
/// launcher panel. VIDEO SET (first six arms): the six apps from the
/// video frame — only "Files" is installed on this machine, the other
/// five keep their upstream icons for the video-defaults screenshot
/// diff. INSTALLED SET (remaining arms): the 37 apps actually scanned
/// from /usr/share/applications + ~/.local/share/applications —
/// 36 of them resolve here; the one that does not is listed in the
/// module header (Hardware Locality lstopo) and falls back
/// to the caller's drawn glyph. Shared icons map several names to one
/// blob: Foot / Foot Client / Foot Server, OpenJDK Console / Shell,
/// Rofi / Rofi Theme Selector, xgps / xgpsspeed. Returns None for any
/// other name — the caller then falls back to its drawn glyph, and
/// must never claim a drawn glyph is a real icon.
pub fn icon_bytes_for(name: &str) -> Option<&'static [u8; ICON_BYTES]> {
    Some(match name {
        "auto-cpufreq" => ICON_AUTO_CPUFREQ,
        "Bluetooth Manager" => ICON_BLUETOOTH,
        "Brave" => ICON_BRAVE,
        "File Roller" => ICON_FILE_ROLLER,
        "Files" => ICON_FILES,
        "GNU Image Manipulation Program" | "GIMP" => ICON_GIMP,
        "About Xfce" => ICON_ABOUT_XFCE,
        "Antigravity" => ICON_ANTIGRAVITY,
        "CMake" => ICON_CMAKE,
        "DeckCraft" => ICON_DECKCRAFT,
        "Display Scaler" => ICON_DISPLAY_SCALER,
        "Dynamite Settings" => ICON_DYNAMITE_SETTINGS,
        "Foot" => ICON_FOOT,
        "Foot Client" => ICON_FOOT,
        "Foot Server" => ICON_FOOT,
        "GridCraft" => ICON_GRID_CRAFT,
        "Handy" => ICON_HANDY,
        "Herdr TUI" => ICON_HERDR_TUI,
        "Keyguard" => ICON_KEYGUARD,
        "Manage Printing" => ICON_CUPS,
        "Meiro" => ICON_MEIRO,
        "NectarLink" => ICON_NECTARLINK,
        "OpenJDK Java 21 Console" => ICON_JAVA21,
        "OpenJDK Java 21 Shell" => ICON_JAVA21,
        "Preferred Applications" => ICON_PREFERRED_APPS,
        "Proton VPN" => ICON_PROTON_VPN,
        "Qt V4L2 test Utility" => ICON_QV4L2,
        "Qt V4L2 video capture utility" => ICON_QVIDCAP,
        "Rofi" => ICON_ROFI,
        "Rofi Theme Selector" => ICON_ROFI,
        "T3 Code" => ICON_T3_CODE,
        "VSCodium" => ICON_VSCODIUM,
        "Waterfox" => ICON_WATERFOX,
        "WordCraft" => ICON_WORDCRAFT,
        "Yazi File Manager" => ICON_YAZI,
        "ZapFast" => ICON_ZAPFAST,
        "Zeron" => ICON_ZERON,
        "btop++" => ICON_BTOP,
        "magpie" => ICON_MAGPIE,
        "xgps" => ICON_XGPS,
        "xgpsspeed" => ICON_XGPS,
        _ => return None,
    })
}

/// Blit one 24x24 straight-alpha RGBA icon onto the BGRA
/// premultiplied canvas, with its top-left corner at (`x0`, `y0`).
/// Source-over blend; destination is opaque inside the panel, so
/// alpha is left at 255 — the same shortcut `blend_pixel` in
/// launcher_panel.rs uses. Fully transparent source pixels are
/// skipped; out-of-bounds pixels are clipped, never wrapped.
/// The caller centres the 24 px icon on the 20 px slot (x0 = cx-12,
/// y0 = cy-12); the 2 px overshoot stays inside the 42 px row.
pub fn blit_icon(canvas: &mut [u8], w: u32, h: u32, x0: i32, y0: i32, rgba: &[u8]) {
    debug_assert_eq!(rgba.len(), ICON_BYTES);
    for sy in 0..ICON_H as i32 {
        let y = y0 + sy;
        if y < 0 || y >= h as i32 {
            continue;
        }
        for sx in 0..ICON_W as i32 {
            let x = x0 + sx;
            if x < 0 || x >= w as i32 {
                continue;
            }
            let si = (sy as usize * ICON_W + sx as usize) * 4;
            let sa = rgba[si + 3] as f32 / 255.0;
            if sa <= 0.0 {
                continue;
            }
            let i = ((y as u32) * w + x as u32) as usize * 4;
            if i + 3 >= canvas.len() {
                continue;
            }
            // Source is straight-alpha RGBA; dest is BGRA, opaque.
            let inv = 1.0 - sa;
            canvas[i] = (rgba[si + 2] as f32 * sa + canvas[i] as f32 * inv) as u8; // B
            canvas[i + 1] = (rgba[si + 1] as f32 * sa + canvas[i + 1] as f32 * inv) as u8; // G
            canvas[i + 2] = (rgba[si] as f32 * sa + canvas[i + 2] as f32 * inv) as u8; // R
            canvas[i + 3] = 255; // A
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyguard_resolves_to_real_icon() {
        let rgba = icon_bytes_for("Keyguard").expect("Keyguard must resolve since iter98");
        assert_eq!(rgba.len(), ICON_BYTES);
        // Real icon, not a blank/empty blob: mostly opaque, strongly
        // saturated (blue padlock on white). Guards against a bad re-extraction.
        let opaque = rgba.iter().skip(3).step_by(4).filter(|&&a| a > 128).count();
        assert!(opaque >= 300, "opaque px: {opaque}");
    }

    #[test]
    fn hardware_locality_still_falls_back() {
        assert!(icon_bytes_for("Hardware Locality lstopo").is_none());
    }
}
