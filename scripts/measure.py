#!/usr/bin/env python3
"""Objective acceptance instrument for the dynamite pill.

Every council critic must run this and quote the numbers. It measures the four
gates recorded in COUNCIL_LOG.md: sharpness, geometry, CPU, RAM.

Usage:  ./scripts/measure.py [--png PATH]   (no --png => runs grim)
Output: one JSON object on stdout.
"""
import json, os, subprocess, sys, time
from pathlib import Path
import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
PROC = "dynamite-rust"
GAP = 4  # max dark-pixel gap inside one cluster (device px)


def shot(path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(["grim", str(path)], check=True)


def pid() -> int | None:
    # -x matches the executable name exactly: `-f` also matched the
    # measuring shell itself (its argv contains the project path), which
    # made rss_mb/cpu_pct report the *shell's* numbers.
    out = subprocess.run(["pgrep", "-n", "-x", PROC], capture_output=True, text=True)
    s = out.stdout.strip()
    return int(s) if s else None


def proc_stats(p: int | None) -> dict:
    if not p:
        return {"rss_mb": None, "cpu_pct": None}
    rss_kb = None
    for line in Path(f"/proc/{p}/status").read_text().splitlines():
        if line.startswith("VmRSS:"):
            rss_kb = int(line.split()[1])
    def ticks() -> int:
        f = Path(f"/proc/{p}/stat").read_text().split()
        return int(f[13]) + int(f[14])
    a = ticks(); t0 = time.monotonic()
    time.sleep(2.0)
    b = ticks(); dt = max(time.monotonic() - t0, 1e-6)
    hz = os.sysconf("SC_CLK_TCK")
    return {"rss_mb": round(rss_kb / 1024, 1) if rss_kb else None,
            "cpu_pct": round(100.0 * (b - a) / hz / dt, 2)}


def pill_bbox(a: np.ndarray) -> dict | None:
    """Find the pure-black capsule near the top centre. a = HxWx3 uint8.

    Only accepts a narrow, centred run (the pill is ~100px wide at 1.5 scale),
    and stops at the first gap after it starts — otherwise a dark browser page
    below the bar gets merged into the bbox.
    """
    H, W, _ = a.shape
    dark = a.max(axis=2) <= 24
    lo, hi = max(0, W // 2 - 110), min(W, W // 2 + 110)
    rows = []
    started = False
    for y in range(0, min(H, 220)):
        # Extent of dark pixels in a centred window: glyphs punch bright holes
        # in the capsule, so a single contiguous run is not the right signal.
        # Split on gaps wider than GAP (4 px): the glyph holes are 1-3 px, the
        # wallpaper gap between the capsule and a tray icon is 6 px, so the
        # cluster containing the screen centre is the capsule alone instead of
        # capsule+neighbour merged into one bbox.
        xs = np.flatnonzero(dark[y, lo:hi])
        if xs.size < 10:
            if started:
                break
            continue
        cut = np.flatnonzero(np.diff(xs) > GAP)
        starts = np.concatenate(([0], cut + 1))
        ends = np.concatenate((cut, [xs.size - 1]))
        centre = W // 2 - lo
        run = None
        for a, b in zip(starts, ends):
            if xs[a] <= centre <= xs[b]:
                run = (a, b)
                break
        if run is None:
            run = max(zip(starts, ends), key=lambda ab: xs[ab[1]] - xs[ab[0]])
        started = True
        rows.append((y, int(xs[run[0]]) + lo, int(xs[run[1]]) + lo))
    if len(rows) < 6:
        return None
    ys = [r[0] for r in rows]
    x0 = min(r[1] for r in rows); x1 = max(r[2] for r in rows)
    y0, y1 = min(ys), max(ys)
    w, h = x1 - x0 + 1, y1 - y0 + 1
    return {"x": x0, "y": y0, "w": w, "h": h,
            "cx": (x0 + x1) / 2, "cy": (y0 + y1) / 2,
            "aspect": round(w / h, 3) if h else None,
            "center_offset_px": round((x0 + x1) / 2 - W / 2, 1),
            "height_pct": round(100.0 * h / H, 3)}


def lapvar(a: np.ndarray, box) -> dict | None:
    """Sharpness of the glyph area: variance of the 4-neighbour Laplacian."""
    x0, y0, x1, y1 = [int(v) for v in box]
    g = a[y0:y1, x0:x1].astype(np.float32).mean(axis=2)
    if g.shape[0] < 4 or g.shape[1] < 4:
        return None
    lap = np.abs(4 * g[1:-1, 1:-1] - g[:-2, 1:-1] - g[2:, 1:-1] - g[1:-1, :-2] - g[1:-1, 2:])
    return {"lapvar": round(float(lap.var()), 1), "lapmean": round(float(lap.mean()), 2)}


def ink_metrics(a: np.ndarray, bb: dict) -> dict | None:
    """Clock ink measured INSIDE the capsule, never on a bare rectangle.

    Two earlier measurements were wrong because a rectangular crop of a
    rounded capsule leaks bright wallpaper through the corners. Mask the
    capsule interior first: distance to the horizontal centre segment must
    be under r-3. Everything here is scale-free so it compares across the
    reference's 2560x1440 and our 2256x1504.
    """
    x, y, w, h = bb["x"], bb["y"], bb["w"], bb["h"]
    g = a[y:y + h, x:x + w].astype(np.float32).mean(axis=2)
    if g.shape[0] < 8 or g.shape[1] < 8:
        return None
    H, W = g.shape
    r = h / 2.0
    yy, xx = np.mgrid[0:H, 0:W]
    d = np.hypot(xx - np.clip(xx, r, W - r), yy - H / 2.0)
    ink = (g > 110) & (d < r - 3.0)
    if not ink.any():
        return None
    rr = np.flatnonzero(ink.any(axis=1)); cc = np.flatnonzero(ink.any(axis=0))
    cap = int(rr[-1] - rr[0] + 1); tw = int(cc[-1] - cc[0] + 1)
    runs = []
    for row in ink:
        k = 0
        for v in row:
            if v:
                k += 1
            elif k:
                runs.append(k); k = 0
        if k:
            runs.append(k)
    med = float(np.median(runs)) if runs else 0.0
    return {"ink_cap_h": cap, "ink_text_w": tw,
            "cap_over_pill_h": round(cap / h, 3),
            "text_over_pill_w": round(tw / W, 3),
            "margin_left": int(cc[0]), "margin_right": int(W - 1 - cc[-1]),
            "stroke_over_cap": round(med / cap, 3) if cap else None}


def edge_ramp(a: np.ndarray, box) -> dict | None:
    """Left-edge AA width: how many px the luminance takes to go dark."""
    x0, y0, x1, y1 = [int(v) for v in box]
    yc = (y0 + y1) // 2
    lum = a[yc, :, :].astype(np.float32).mean(axis=1)
    scan = lum[max(0, x0 - 8): min(a.shape[1], x0 + 4)]
    return {"left_edge_lum": [round(float(v), 1) for v in scan]}


def main() -> int:
    png = None
    if "--png" in sys.argv:
        png = Path(sys.argv[sys.argv.index("--png") + 1])
    if png is None:
        png = Path("/tmp/dynamite-measure.png")
        shot(png)
    a = np.array(Image.open(png).convert("RGB"))
    H, W, _ = a.shape
    out = {"png": str(png), "screen": {"w": W, "h": H}}
    bb = pill_bbox(a)
    out["pill"] = bb
    if bb:
        gx0 = bb["x"] + int(bb["w"] * 0.22); gx1 = bb["x"] + int(bb["w"] * 0.78)
        gy0 = bb["y"] + int(bb["h"] * 0.25); gy1 = bb["y"] + int(bb["h"] * 0.78)
        out["glyphs"] = lapvar(a, (gx0, gy0, gx1, gy1))
        out["ink"] = ink_metrics(a, bb)
        out["edge"] = edge_ramp(a, (bb["x"], bb["y"], 0, bb["y"] + bb["h"]))
    out.update(proc_stats(pid()))
    print(json.dumps(out, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
