#!/usr/bin/env python3
"""Does each expanded state actually RENDER, not just log a state change?

The stress suite's 246 state-machine tests read `DBG morph A -> B` lines.
That proves the state machine moved; it does not prove anything was drawn.
A panel could log `-> Themes` and paint nothing and every test would pass.

This checks pixels per state: a dark panel rect of plausible size, with
content inside it (glyphs/icons), and no wallpaper hole. It is the visual
half of the state-machine gate.

Usage: render_check.py [state ...]        (default: all expanded states)
Exit 1 if any state fails.
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
STATES = ["power", "music", "calendar", "controlcenter", "displays", "launcher",
          "wallpaper", "themes", "clipboard", "traymenu"]

# Expected panel widths, LOGICAL px, read from IslandState::size()
# (src/main.rs:199). This gate asks "did the state paint its own surface",
# so the code's declared size is the right expectation; whether that size
# matches the REFERENCE is a separate question the visual critics score.
# Guessing reference bands here produced four false failures on a correct
# build (music measured 0.406 against a made-up 0.40 ceiling).
LOGICAL_W = {
    "power": 316, "music": 612, "calendar": 342, "controlcenter": 438, "displays": 438,
    "launcher": 504, "wallpaper": 1008, "themes": 600,
    "clipboard": 504, "traymenu": 404,
}
SCALE = 1.5
TOL = 0.12          # +-12%: rounded corners and AA shave a few px per side

# Declared logical heights, same source (IslandState::size). Used only to
# bound the ink window: the row span cannot see past the 88-row band cap.
# Clipboard and TrayMenu size themselves from live content (clipboard:
# 74 + (n-1)*47.5 + 42 + 12.5 for n rows, min 200 empty; tray: 60 + 28*n,
# clamped to 440), so their entry is the MINIMUM, and the ink window for a
# dynamic state is capped by the measured span's own bottom instead.
LOGICAL_H = {
    "power": 118, "music": 150, "calendar": 262, "controlcenter": 424, "displays": 284,
    "launcher": 129, "wallpaper": 270, "themes": 366,
    "clipboard": 200, "traymenu": 88,
}
DYNAMIC_H = {"launcher", "clipboard", "traymenu"}


def send(word):
    d = socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM)
    d.sendto(word.encode(), SOCK)
    d.close()


def measure(state, settle=1.5):
    send(state)
    time.sleep(settle)
    png = f"/tmp/render_{state}.png"
    if subprocess.run(["grim", png], capture_output=True).returncode != 0:
        return {"state": state, "ok": False, "why": "grim failed"}
    a = np.asarray(Image.open(png).convert("RGB")).astype(int)
    H, W, _ = a.shape

    # The island lives in the top band. Below it the app window is also dark
    # and far wider, so cap the scan (same trap halo_check.py documents).
    # Measured on this desktop: the owner's window starts at y=90 at x=400,
    # so a 200-row band always included it and the wallpaper panel (1512 px)
    # merged with the window and read 0.84 of screen width.
    band = a[:88]
    dark = band.max(axis=2) <= 24
    cx = W // 2
    # Take the CONTIGUOUS dark run containing screen centre on each row, not
    # the min/max of all dark pixels: the app window's title bar is dark too
    # and spans the full width, which reported every panel as 0.977 of screen.
    def centre_run(y):
        row = dark[y]
        if not row[cx]:
            return None
        l = cx
        while l > 0 and row[l - 1]:
            l -= 1
        r = cx
        while r < W - 1 and row[r + 1]:
            r += 1
        return l, r

    # Keep only rows whose run is PLAUSIBLY the panel: wide enough to be a
    # panel rather than a glyph gap, narrow enough not to be the app window.
    # Measured on a live power capture: y20-135 gives 422..473 px (the panel),
    # then y136+ jumps to 1097..1893 (the T3 title bar, also dark, also
    # containing screen centre). Taking every row unioned them into 0.84 of
    # screen width. Rows crossing a disc or glyph shrink to 6..279 px, so a
    # lower bound is needed too.
    # Width bounds alone do not separate them: the window's runs are
    # 1097..1893 px, inside any wmax generous enough for the wallpaper panel.
    # The island is anchored TOP-CENTRE, so its run is SYMMETRIC about screen
    # centre; the window's is not (measured centres 1284..1427 vs cx 1128).
    wmin, wmax = int(0.05 * W), int(0.78 * W)
    runs = []
    for y in range(band.shape[0]):
        r = centre_run(y)
        if not r:
            continue
        w = r[1] - r[0] + 1
        if not (wmin <= w <= wmax):
            continue
        if abs((r[0] + r[1]) / 2 - cx) > 0.05 * W:
            continue
        runs.append((y, r))
    if not runs:
        return {"state": state, "ok": False, "why": "no panel-width dark run"}
    y0, y1 = runs[0][0], runs[-1][0]
    # Width = the 90th percentile run, not the union and not the median.
    #   union: one row bleeding into the app window drags it (CC y136 read
    #          1540 px against a 657 px panel).
    #   median: most rows CROSS content, so the dark run stops at a disc or
    #          a glyph (CC p50 265 px, power p50 472, music p50 916).
    # The widest rows are the panel's own fill; p90 reaches them while still
    # ignoring the handful of rows that merged with something else.
    widths = sorted(r[1] - r[0] + 1 for _, r in runs)
    pw_typ = widths[min(len(widths) - 1, int(len(widths) * 0.90))]
    # x-extent = the median run's edges, NOT the union over rows. A single
    # row that bleeds into the app window drags the union by hundreds of px
    # (calendar read 738 px against a 513 px panel, launcher 827 against
    # 756). The panel is centred, so the median run is the honest rect;
    # the p90 width above is still what guards against glyph-crossed rows.
    lefts = sorted(r[0] for _, r in runs)
    rights = sorted(r[1] for _, r in runs)
    x0 = lefts[len(lefts) // 2]
    x1 = rights[len(rights) // 2]
    # Trust the p90 width when the median run is narrower: rounded corners
    # shave the top rows and content cuts the middle ones, so the median can
    # under-read the true fill. Re-centre it on the median run's centre.
    if pw_typ > x1 - x0 + 1:
        mid = (x0 + x1) // 2
        x0 = mid - pw_typ // 2
        x1 = x0 + pw_typ - 1
    pw, ph = x1 - x0 + 1, y1 - y0 + 1
    frac = pw / W

    # Content: bright pixels strictly inside, away from the rounded corners.
    # The height comes from the DECLARED logical size, not the row span: the
    # band cap (88 rows, to keep the app window out) truncates the span of
    # every state taller than ~46 logical px, so measuring ink inside the
    # span reads 0.00% on a perfectly drawn music panel. Width is measured
    # from pixels; height is verified separately by the containment test in
    # src/main.rs (stage_extent_tests).
    r = 8
    want_h = int(round(LOGICAL_H.get(state, 0) * SCALE))
    if want_h:
        top = runs[0][0]
        bottom = runs[-1][0]
        if state in DYNAMIC_H:
            # Dynamic states: the real height is whatever the live content
            # made it. Use the span when it is taller than the minimum, and
            # still give a short span enough rows to contain its rows.
            y1 = top + max(want_h, bottom - top + 1) - 1
        else:
            y1 = top + want_h - 1
        y0 = top
    inner = a[y0 + r:min(y1 - r + 1, a.shape[0]), x0 + r:x1 - r + 1]
    lum = inner.mean(axis=2)
    ink = float((lum > 110).mean()) if lum.size else 0.0

    want = LOGICAL_W.get(state)
    why = []
    if want:
        expect = want * SCALE
        if abs(pw - expect) > TOL * expect:
            why.append(f"width {pw} px vs declared {expect:.0f} "
                       f"({want} logical x {SCALE})")
    if ph < 24:
        why.append(f"height {ph} px too short for an expanded panel")
    if ink < 0.005:
        why.append(f"no content: only {ink*100:.2f}% bright pixels inside")
    # `h` is a LOWER BOUND on panel height, not the height: rows whose centre
    # column crosses a disc or a glyph have no dark centre run and are
    # skipped, so the span stops early (power reads 66 of a 177 px panel).
    # Width and ink are the load-bearing checks; height only catches "nothing
    # tall was drawn at all". Do not score panel proportions from this field.
    return {"state": state, "ok": not why, "why": "; ".join(why),
            "panel": [x0, y0, x1, y1], "w": pw, "h_lower_bound": ph,
            "width_frac": round(frac, 4), "ink_frac": round(ink, 4)}


def main():
    states = sys.argv[1:] or STATES
    send("collapsed")
    time.sleep(0.8)
    rows = [measure(s) for s in states]
    send("collapsed")
    bad = [r for r in rows if not r["ok"]]
    print(json.dumps({"rows": rows, "failed": len(bad)}, indent=1))
    print("RESULT", "PASS" if not bad else "FAIL: " + ", ".join(r["state"] for r in bad))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
