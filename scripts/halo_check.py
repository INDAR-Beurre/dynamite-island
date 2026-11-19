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
    r = max(4, (y1 - y0) // 8)
    sub = a[y0 + r:y1 - r + 1, x0 + r:x1 - r + 1]
    wall = (sub[:, :, 2] > sub[:, :, 0] + 20) & (sub.mean(axis=2) > 60)
    n = int(wall.sum())
    out = {"state": state, "panel": [x0, y0, x1, y1], "hole_px": n}
    if n:
        ys, xs = np.where(wall)
        out["hole_bbox"] = [int(xs.min() + x0 + r), int(ys.min() + y0 + r),
                            int(xs.max() + x0 + r), int(ys.max() + y0 + r)]
    return out


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
