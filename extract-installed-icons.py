#!/usr/bin/env python3
"""Iter86: extract REAL 24x24 RGBA8 icons for all 37 installed launcher apps.

Run ON the Surface:  python3 extract-installed-icons.py
Output: /tmp/iter86-icons/<slug>.rgba  (2304 bytes each) + sha256 manifest.
Also prints a per-icon saturation report (opaque px, saturated px).

Sources: each app's Icon= resolved via the icon theme in use (Adwaita,
per ~/.config/gtk-3.0/settings.ini gtk-icon-theme-name=Adwaita) with
hicolor / AdwaitaLegacy / ~/.local/share/icons / /usr/share/pixmaps as
fallbacks, plus absolute-path Icon= values. Largest raster wins; a
scalable SVG wins over any raster (rsvg-convert renders it at 48x48).
PNG -> PIL RGBA -> LANCZOS 24x24. SVG -> rsvg-convert 48x48 -> same.

Apps with NO resolvable icon (genuinely missing on this machine):
  Hardware Locality lstopo (Icon=hwloc — no file anywhere)
  Keyguard (no Icon= key in its .desktop, no icon file anywhere)
Those two keep the drawn-folder fallback and are listed in the report.

Re-run after any app install / icon-theme change, copy the .rgba files
into src/icons/, and update launcher_icons.rs digests.
"""
import hashlib
import os
import subprocess
import sys
from PIL import Image

OUT = "/tmp/iter86-icons"
os.makedirs(OUT, exist_ok=True)

# slug -> (display name(s) for the report, source file)
SOURCES = {
    "about-xfce":       (["About Xfce"], "/usr/share/icons/hicolor/scalable/apps/org.xfce.about.svg"),
    "antigravity":      (["Antigravity"], "/usr/share/pixmaps/antigravity.png"),
    "btop":             (["btop++"], "/usr/share/icons/hicolor/scalable/apps/btop.svg"),
    "cmake":            (["CMake"], "/usr/share/icons/hicolor/128x128/apps/CMakeSetup.png"),
    "deckcraft":        (["DeckCraft"], "/home/alex/.local/share/icons/hicolor/scalable/apps/ai.storyteller.deckcraft.svg"),
    "display-scaler":   (["Display Scaler"], "/usr/share/icons/AdwaitaLegacy/48x48/legacy/preferences-desktop-display.png"),
    "dynamite-settings": (["Dynamite Settings"], "/usr/share/icons/AdwaitaLegacy/48x48/legacy/preferences-system.png"),
    "foot":             (["Foot", "Foot Client", "Foot Server"], "/usr/share/icons/hicolor/scalable/apps/foot.svg"),
    "gridcraft":        (["GridCraft"], "/home/alex/.local/share/icons/hicolor/256x256/apps/ai.storyteller.gridcraft.png"),
    "handy":            (["Handy"], "/home/alex/.local/share/icons/hicolor/128x128/apps/handy.png"),
    "herdr-tui":        (["Herdr TUI"], "/home/alex/.local/share/icons/hicolor/scalable/apps/herdr-tui.svg"),
    "java21-openjdk":   (["OpenJDK Java 21 Console", "OpenJDK Java 21 Shell"], "/usr/share/icons/hicolor/48x48/apps/java21-openjdk.png"),
    "magpie":           (["magpie"], "/home/alex/.local/share/icons/hicolor/256x256/apps/magpie.png"),
    "cups":             (["Manage Printing"], "/usr/share/icons/hicolor/128x128/apps/cups.png"),
    "meiro":            (["Meiro"], "/home/alex/.local/share/icons/meiro.png"),
    "nectarlink":       (["NectarLink"], "/home/alex/.local/share/icons/hicolor/256x256/apps/nectarlink.png"),
    "preferences-desktop": (["Preferred Applications"], "/usr/share/icons/AdwaitaLegacy/48x48/legacy/preferences-desktop.png"),
    "proton-vpn":       (["Proton VPN"], "/usr/share/icons/hicolor/scalable/apps/proton-vpn-logo.svg"),
    "qv4l2":            (["Qt V4L2 test Utility"], "/usr/share/icons/hicolor/scalable/apps/qv4l2.svg"),
    "qvidcap":          (["Qt V4L2 video capture utility"], "/usr/share/icons/hicolor/scalable/apps/qvidcap.svg"),
    "rofi":             (["Rofi", "Rofi Theme Selector"], "/usr/share/icons/hicolor/scalable/apps/rofi.svg"),
    "t3-code":          (["T3 Code"], "/home/alex/.local/share/icons/hicolor/512x512/apps/t3-code.png"),
    "vscodium":         (["VSCodium"], "/usr/share/icons/hicolor/scalable/apps/vscodium.svg"),
    "waterfox":         (["Waterfox"], "/home/artix/.local/opt/waterfox/browser/chrome/icons/default/default128.png"),
    "wordcraft":        (["WordCraft"], "/home/alex/.local/share/icons/hicolor/scalable/apps/ai.storyteller.wordcraft.svg"),
    "xgps":             (["xgps", "xgpsspeed"], "/usr/share/gpsd/icons/gpsd-logo.png"),
    "yazi":             (["Yazi File Manager"], "/usr/share/icons/hicolor/512x512/apps/yazi.png"),
    "zapfast":          (["ZapFast"], "/home/alex/.local/share/icons/hicolor/scalable/apps/zapfast.svg"),
    "zeron":            (["Zeron"], "/home/artix/.zeron/app/current/zeron.png"),
}
# "Files" reuses the existing files.rgba (same source, org.gnome.Nautilus.svg);
# it is verified below against a fresh extraction instead of being rewritten.

def load_48(src):
    if src.endswith(".svg"):
        tmp = "/tmp/iter86-icons/_tmp48.png"
        subprocess.run(["rsvg-convert", "-w", "48", "-h", "48", "-o", tmp, src],
                       check=True)
        return Image.open(tmp).convert("RGBA")
    return Image.open(src).convert("RGBA")

manifest = []
for slug in sorted(SOURCES):
    names, src = SOURCES[slug]
    assert os.path.exists(src), f"MISSING SOURCE {slug}: {src}"
    im = load_48(src)
    im = im.resize((24, 24), Image.LANCZOS)
    raw = im.tobytes()  # RGBA8, 24*24*4 = 2304
    assert len(raw) == 2304, slug
    path = os.path.join(OUT, slug + ".rgba")
    with open(path, "wb") as f:
        f.write(raw)
    sha = hashlib.sha256(raw).hexdigest()
    opaque = sum(1 for i in range(3, 2304, 4) if raw[i] > 128)
    sat = 0
    for i in range(0, 2304, 4):
        if raw[i + 3] > 128:
            r, g, b = raw[i], raw[i + 1], raw[i + 2]
            if max(r, g, b) - min(r, g, b) > 60:
                sat += 1
    manifest.append((slug, sha, opaque, sat, ", ".join(names), src))
    print(f"{slug:22s} sha256={sha} opaque={opaque:3d}/576 sat={sat:3d}  {', '.join(names)}")

# Verify Files: fresh extraction of Nautilus SVG must equal existing files.rgba
nautilus = "/usr/share/icons/hicolor/scalable/apps/org.gnome.Nautilus.svg"
im = load_48(nautilus).resize((24, 24), Image.LANCZOS)
fresh = im.tobytes()
existing = open("/home/alex/rust-island-build/src/icons/files.rgba", "rb").read()
print("files.rgba fresh==existing:", fresh == existing,
      "fresh sha256:", hashlib.sha256(fresh).hexdigest())

with open(os.path.join(OUT, "MANIFEST.txt"), "w") as f:
    for slug, sha, opaque, sat, names, src in manifest:
        f.write(f"{slug} {sha} opaque={opaque} sat={sat} [{names}] {src}\n")
print(f"WROTE {len(manifest)} icons to {OUT}")
