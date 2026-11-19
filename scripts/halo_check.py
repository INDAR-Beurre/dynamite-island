#!/usr/bin/env python3
"""Detect the square wallpaper hole inside an open panel (brief §19).

The panel fill is (0,0,0). Wallpaper is bright and blue-biased. A hole is
any run of wallpaper-coloured pixels strictly inside the panel's rounded
rect. Returns a JSON verdict; exit 1 if a hole is found.

Usage: halo_check.py [state ...]      (default: all MENU_CYCLE states)
"""
import json
import os
import socket
import subprocess
import sys
import time

import numpy as np
from PIL import Image

SOCK = os.path.join(os.environ.get("XDG_RUNTIME_DIR", "/run/user/1000"),
                    "dynamite-rust.sock")
STATES = ["power", "music", "calendar", "controlcenter", "launcher",
          "wallpaper", "themes", "clipboard"]


def send(word):
    d = socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM)
    d.sendto(word.encode(), SOCK)
    d.close()


def panel_bbox(a):
    """Panel rect in the top band.

    Two traps this avoids:
      * the app window below y~98 is also dark and much wider, so the scan
        is capped at the band the island can occupy;
      * the hole itself SPLITS the panel's dark run on the affected rows,
        so the width must come from the unbroken lower rows, not the top.
    """
    band = a[:95]
    dark = band.max(axis=2) <= 24

    def widest(y):
        xs = np.where(dark[y])[0]
        if len(xs) < 80:
            return None
        runs, s, p = [], xs[0], xs[0]
        for x in xs[1:]:
            if x - p > 25:
                runs.append((s, p))
                s = x
            p = x
        runs.append((s, p))
        return max(runs, key=lambda r: r[1] - r[0])

    best = None
    for y in range(58, band.shape[0]):
        w = widest(y)
        if w and (best is None or w[1] - w[0] > best[1] - best[0]):
            best = w
    if best is None:
        return None
    x0, x1 = best
    cols = dark[:, x0:x1 + 1].sum(axis=1)
    ys = np.where(cols > (x1 - x0) * 0.30)[0]
    if len(ys) == 0:
        return None
    return int(x0), int(ys.min()), int(x1), int(ys.max())


def check(state, settle=1.5):
    send(state)
    time.sleep(settle)
    png = f"/tmp/halo_{state}.png"
    if subprocess.run(["grim", png], capture_output=True).returncode != 0:
        return {"state": state, "error": "grim failed"}
    a = np.asarray(Image.open(png).convert("RGB")).astype(int)
    bb = panel_bbox(a)
    if bb is None:
        return {"state": state, "error": "no panel found"}
    x0, y0, x1, y1 = bb
    # Inset past the rounded corners so the capsule's own AA is not counted.
    # Inset past the rounded corners AND the panel's own content margin.
    r = 12
    sub = a[y0 + r:y1 - r + 1, x0 + r:x1 - r + 1]
    lum = sub.mean(axis=2)
    # Threshold alone cannot work: the frost blur desaturates the leaked
    # wallpaper to (129,131,135) and the cloud behind it reaches 215, so it
    # overlaps both the accent icons and the white glyphs. Shape separates
    # them instead -- glyphs are thin strokes and die under a 7x7 erosion,
    # while a 258x35 hole survives it.
    from scipy import ndimage
    er = ndimage.binary_erosion(lum > 40, np.ones((7, 7)))
    lab, nlab = ndimage.label(er)
    blobs = []
    for i, sl in enumerate(ndimage.find_objects(lab)):
        size = int((lab[sl] == i + 1).sum())
        if size > 150:
            blobs.append({
                "px": size,
                "bbox": [int(sl[1].start + x0 + r), int(sl[0].start + y0 + r),
                         int(sl[1].stop + x0 + r), int(sl[0].stop + y0 + r)],
            })
    return {"state": state, "panel": [x0, y0, x1, y1],
            "hole_px": sum(b["px"] for b in blobs), "blobs": blobs}


def main():
    states = sys.argv[1:] or STATES
    send("collapsed")
    time.sleep(0.6)
    rows = [check(s) for s in states]
    send("collapsed")
    bad = [r for r in rows if r.get("hole_px", 0) > 0]
    print(json.dumps({"rows": rows, "holes": len(bad)}, indent=1))
    print("RESULT", "FAIL" if bad else "PASS")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
