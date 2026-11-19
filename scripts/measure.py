#!/usr/bin/env python3
"""Objective acceptance instrument for the dynamite pill.

Every council critic must run this and quote the numbers. It measures the four
gates recorded in COUNCIL_LOG.md: sharpness, geometry, CPU, RAM.

Usage:  ./scripts/measure.py [--png PATH]   (no --png => runs grim)
Output: one JSON object on stdout.
"""
import json, os, re, subprocess, sys, time
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
    out = subprocess.run(["pgrep", "-x", PROC], capture_output=True, text=True)
    pids = out.stdout.split()
    if not pids:
        return None
    if len(pids) > 1:
        # Two instances share one socket path: the newest pid need not be the
        # one that owns the surface we are photographing. Refuse rather than
        # attribute one process's pixels to another's RSS.
        return None
    return int(pids[0])


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
        for s, e in zip(starts, ends):   # not a/b: `a` is the image array
            if xs[s] <= centre <= xs[e]:
                run = (s, e)
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
    # A box that starts at y=0 and fills the scan window is a fullscreen dark
    # surface (lockscreen, a dark app), not the capsule. Scoring it as a pill
    # produced a bogus "220x220, aspect 1.0" reading that critics would quote.
    if y0 == 0 and x0 <= lo and x1 >= hi - 1:
        return {"not_collapsed": True, "x": x0, "y": y0, "w": w, "h": h,
                "reason": "dark surface fills the scan window from y=0"}
    # A dark app window merges into the bbox too, and it neither starts at y=0
    # nor fills the window, so the guard above misses it. Height cannot separate
    # them (bar_height clamps to 80 logical = 120 device px). The capsule's
    # distinguishing property is its MATERIAL: interior luminance 0-2, where
    # dark UI themes sit at 18-30.
    # Use a low percentile of the WHOLE box, not the left/right padding: the
    # capsule is a stadium, so its outer 12% is entirely rounded cap and samples
    # the wallpaper through the corners (that read 5.0 on a bright wallpaper and
    # refused a genuine pill). Glyph ink is the bright minority, so p25 is fill.
    # Threshold 12, not 4: the reference fill measures 0-2 but the live panel
    # fills at exactly 5, and both are the capsule. Dark UI themes sit at 18-30,
    # so 12 separates material from window without refusing a real pill.
    interior = float(np.percentile(a[y0:y1 + 1, x0:x1 + 1], 25))
    if interior > 12.0:
        return {"not_collapsed": True, "x": x0, "y": y0, "w": w, "h": h,
                "reason": f"interior luminance p25 {interior:.1f} > 12 (not the black capsule)"}
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
    # Raw lapvar scales with the SQUARE of contrast, so it cannot be compared
    # across sources: live ink clips at 255 while the video-sourced reference
    # peaks at 225.7 and never reaches 255. Critic #1 measured that clamping the
    # live ink to 225 alone drops lapvar 12100 -> 9436, i.e. roughly half the
    # apparent 1.86x gap was amplitude, not edge structure. Normalise by the
    # crop's own floor-to-peak range so the number describes edges only.
    span = float(g.max() - g.min())
    out = {"lapvar": round(float(lap.var()), 1), "lapmean": round(float(lap.mean()), 2),
           "ink_peak": round(float(g.max()), 1), "ink_span": round(span, 1)}
    if span > 1.0:
        ln = lap / span
        out["lapvar_norm"] = round(float(ln.var()) * 1e4, 2)
        out["lapmean_norm"] = round(float(ln.mean()) * 1e2, 3)
    return out


def screen_relative(a: np.ndarray, bb: dict) -> dict:
    """Position/size as fractions of the SCREEN, not of the pill.

    Every earlier metric was pill-relative, so a pill hugging the top edge
    scored perfectly. Reference: gap 1.319% of height, width 4.336% of width.
    """
    H, W = a.shape[:2]
    return {"gap_top_pct": round(bb["y"] / H * 100, 3),
            "width_pct": round(bb["w"] / W * 100, 3),
            "height_pct_screen": round(bb["h"] / H * 100, 3)}


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


def panel_state() -> dict | None:
    """The panel's OWN last reported state, read from its stderr witness.

    Pixel heuristics have now mis-scored six times (discs swallowed, dark page
    below, dark window at y=0, dark window mid-screen, dark wallpaper, and a
    lock surface measured as a pill). The last one defeated every pixel guard:
    the box was genuinely the panel's own black material, just the wrong
    surface. So ask the panel instead of guessing.

    TWO witnesses, because the first one proved fragile. 'DBG enter state=' is
    printed only on POINTER ENTER (src/main.rs:5653), so a panel driven purely
    over the socket never emits it -- this guard silently returned None and fell
    back to the pixel heuristics it exists to replace. 'DBG morph A -> B' is
    printed on every state change (src/main.rs:4764) and is already a contract
    the stress harness depends on, so it is the reliable one.

    A morph line reports the state being LEFT and the one being ENTERED, and is
    printed BEFORE the morph runs. `morph_running` says whether an animation
    follows. There is no completion line, so a morph that has since finished
    still reads morph=true; callers must allow for settling (see settled_for).
    """
    p = pid()
    if p is None:
        return None
    try:
        target = os.readlink(f"/proc/{p}/fd/2")
    except OSError:
        return None
    if not (target.startswith("/") and os.path.isfile(target)):
        return None
    try:
        with open(target, "rb") as f:
            f.seek(0, 2)
            size = f.tell()
            f.seek(max(0, size - 65536))
            lines = f.read().decode("utf-8", "replace").splitlines()
    except OSError:
        return None
    for ln in reversed(lines):
        m = re.search(r"DBG enter state=(\w+) morph=(\w+)", ln)
        if m:
            return {"state": m.group(1), "morph": m.group(2) == "true",
                    "witness": ln.strip(), "via": "pointer-enter"}
        # `DBG morph Lockscreen -> Collapsed (morph_running=true)`
        m = re.search(r"DBG morph (\w+) -> (\w+) \(morph_running=(\w+)\)", ln)
        if m:
            return {"state": m.group(2), "morph": m.group(3) == "true",
                    "witness": ln.strip(), "via": "morph"}
        # Lock transitions use a fixed target and no morph_running flag.
        m = re.search(r"DBG morph (\w+) -> Lockscreen \(morph_running=false\)", ln)
        if m:
            return {"state": "Lockscreen", "morph": False,
                    "witness": ln.strip(), "via": "morph"}
    return None


def settled(png: Path, tries: int = 3, allow_capture: bool = True) -> bool:
    """True when two captures `anim_ms` apart are pixel-identical.

    The morph witness is printed BEFORE the animation runs and there is no
    completion line, so `morph_running=true` stays true forever after a morph
    finishes. Trusting it would refuse every measurement. Instead ask the
    pixels the one question they CAN answer reliably: is anything still moving?
    This is not a return to the old heuristics -- it does not identify the pill,
    it only detects change.
    """
    import time
    # Compare only the BAR STRIP, not the whole screen: a terminal cursor or a
    # video elsewhere on the desktop keeps changing forever and would make this
    # refuse every measurement. The strip is where the island lives.
    strip = 220
    if not allow_capture:
        # A supplied PNG is a still: it cannot settle, and re-capturing over
        # it would DESTROY the reference frame. That happened once -- s17.png
        # was overwritten with a live capture and every critic then scored
        # our own panel as "the reference".
        return False
    try:
        prev = np.array(Image.open(png).convert("RGB"))[:strip]
    except OSError:
        return False
    for _ in range(tries):
        time.sleep(0.20)          # > anim_ms (160 ms)
        shot(png)
        cur = np.array(Image.open(png).convert("RGB"))[:strip]
        if cur.shape == prev.shape and not np.any(cur != prev):
            return True
        prev = cur
    return False


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
    # A supplied PNG is a reference frame, not our live panel. The running
    # panel's state says nothing about it -- consulting it once made this
    # tool overwrite the reference row with live geometry.
    st = None if "--png" in sys.argv else panel_state()
    if st:
        out["panel"] = st
    # The panel's own witness outranks any pixel heuristic: a lock surface or
    # an open panel is black too, and no luminance test can tell them apart.
    moving = st and st["state"] == "Collapsed" and st["morph"]
    if moving:
        # A finished morph still reads morph=true. Only refuse if it is
        # genuinely still animating.
        if settled(png, allow_capture="--png" not in sys.argv):
            st["morph"] = False
            st["settled_by"] = "two identical captures 200ms apart"
        else:
            out["state"] = "not-collapsed"
            out["note"] = ("panel is mid-morph and still changing between "
                           "captures; wait and re-measure")
            out["pill"] = None
            out.update(proc_stats(pid()))
            print(json.dumps(out, indent=2))
            return 0
    if st and (st["state"] != "Collapsed" or st["morph"]):
        out["state"] = "not-collapsed"
        out["note"] = (f"panel reports state={st['state']} morph={st['morph']}; "
                       "no geometry/sharpness gates emitted. Drive `collapsed`, "
                       "wait for the morph to finish, and re-measure.")
        out["pill"] = None
        out.update(proc_stats(pid()))
        print(json.dumps(out, indent=2))
        return 0
    if bb and bb.get("not_collapsed"):
        # Refuse to score a frame that is not the collapsed pill: geometry and
        # sharpness gates are meaningless on a lockscreen or an open panel.
        out["state"] = "not-collapsed"
        out["note"] = ("no geometry/sharpness gates emitted; drive `collapsed` "
                       "and re-measure in a quiet window")
    elif bb:
        gx0 = bb["x"] + int(bb["w"] * 0.22); gx1 = bb["x"] + int(bb["w"] * 0.78)
        gy0 = bb["y"] + int(bb["h"] * 0.25); gy1 = bb["y"] + int(bb["h"] * 0.78)
        out["glyphs"] = lapvar(a, (gx0, gy0, gx1, gy1))
        out["ink"] = ink_metrics(a, bb)
        out["screen"] = screen_relative(a, bb)
        out["edge"] = edge_ramp(a, (bb["x"], bb["y"], 0, bb["y"] + bb["h"]))
    out.update(proc_stats(pid()))
    print(json.dumps(out, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
