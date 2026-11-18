#!/usr/bin/env python3
"""analyze.py <outdir> [--region x0,y0,x1,y1] [--json]

Measures a capture.sh sequence. Prints:
  - frames captured vs requested (drop rate)
  - inter-capture interval: median / mean / p95 (+ grim's own duration)
  - histogram of consecutive-frame mean-absolute-pixel-delta
  - frames with delta ~0  -> duplicated frame = a stalled redraw
  - frames with delta spike -> a jump = a skipped intermediate frame

Caveat that governs how to read this: the capture loop is grim-based, so the
INTERVAL numbers describe how fast grim could go, not how the compositor
clocked. The DELTA numbers are the trustworthy part: they say whether the
panel actually put new pixels on screen. A run where the clocky pill shows
many zero-delta frames while nothing is animating is expected (the pill is
static); zero-delta frames DURING an animation are a stalled redraw.
"""
import json
import statistics
import sys
from pathlib import Path

import numpy as np
from PIL import Image

ZERO_EPS = 0.05      # mean-abs delta below this = same frame (8-bit, AA noise)
# A spike is a step big enough to have swallowed an intermediate frame, i.e.
# roughly TWO typical steps of motion in one interval. Stated as a ratio of the
# median non-zero delta rather than an absolute value so it works at any scale.
# (A 3x rule was tried first and missed real jumps when only a handful of
# non-zero deltas existed: median 100, step 185 -> threshold 300, no flag.)
SPIKE_RATIO = 1.8
PILL_ROWS = 300      # default crop: top 300 device rows = 200 logical px
                     # at scale 1.5, i.e. the band the island lives in


def load_any(p: Path) -> np.ndarray:
    im = Image.open(p)
    if im.mode not in ("RGB", "L"):
        im = im.convert("RGB")
    return np.asarray(im, dtype=np.uint8)


def parse_region(s: str):
    x0, y0, x1, y1 = (int(v) for v in s.split(","))
    return x0, y0, x1, y1


def main() -> int:
    args = [a for a in sys.argv[1:]]
    if not args:
        print(__doc__.strip().splitlines()[0])
        return 2
    outdir = Path(args.pop(0))
    region = None
    as_json = False
    while args:
        a = args.pop(0)
        if a == "--region":
            region = parse_region(args.pop(0))
        elif a == "--json":
            as_json = True
        else:
            print(f"unknown arg: {a}", file=sys.stderr)
            return 2

    meta = {}
    mp = outdir / "meta.txt"
    if mp.exists():
        for line in mp.read_text().splitlines():
            if "=" in line:
                k, v = line.split("=", 1)
                meta[k.strip()] = v.strip()

    frames = sorted(p for p in outdir.glob("frame_*") if p.suffix in (".ppm", ".png"))
    if not frames:
        print(f"no frames in {outdir}", file=sys.stderr)
        return 1

    # --- timing ---------------------------------------------------------
    starts, ends = [], []
    tp = outdir / "timestamps.txt"
    if tp.exists():
        for line in tp.read_text().splitlines():
            parts = line.split()
            if len(parts) >= 3:
                try:
                    starts.append(int(parts[1]))
                    ends.append(int(parts[2]))
                except ValueError:
                    pass
    intervals = [(b - a) / 1e6 for a, b in zip(starts, starts[1:])]
    grim_ms = [(e - s) / 1e6 for s, e in zip(starts, ends)]

    requested = int(float(meta.get("requested_seconds", 0)) * float(meta.get("requested_fps", 0)) or 0)
    captured = len(frames)
    drop_rate = (1 - captured / requested) * 100 if requested else float("nan")

    med = round(statistics.median(intervals), 1) if intervals else None
    mean = round(statistics.fmean(intervals), 1) if intervals else None
    p95 = round(float(np.percentile(intervals, 95)), 1) if intervals else None
    gmed = round(statistics.median(grim_ms), 1) if grim_ms else None

    # --- pixel deltas ---------------------------------------------------
    probe = Image.open(frames[0])
    iw, ih = probe.size
    if region:
        x0, y0, x1, y1 = region
        # --region is given in LOGICAL px (the same space grim -g takes) and
        # scaled to device px here: frames are 1.5x the logical screen.
        lw = meta.get("logical_width")
        scale = (iw / int(lw)) if lw else 1.0
        x0, y0, x1, y1 = (int(round(v * scale)) for v in (x0, y0, x1, y1))
        x0, y0 = max(0, x0), max(0, y0)
        x1, y1 = min(iw, x1), min(ih, y1)
    else:
        x0, y0, x1, y1 = 0, 0, iw, min(ih, PILL_ROWS)

    deltas = []
    prev = None
    kept = 0
    for p in frames:
        a = load_any(p)[y0:y1, x0:x1]
        if a.shape[:2] != (y1 - y0, x1 - x0):
            continue
        kept += 1
        if prev is not None and prev.shape == a.shape:
            d = np.abs(a.astype(np.int16) - prev.astype(np.int16)).mean()
            deltas.append(float(d))
        prev = a

    zero = [i for i, d in enumerate(deltas) if d <= ZERO_EPS]
    nonzero = [d for d in deltas if d > ZERO_EPS]
    # median_LOW, not median: with an even (or tiny) count of non-zero deltas
    # the midpoint sits half-way to the outlier, which hides the very jump we
    # are looking for. median_low reports a real observed step instead.
    med_nz = statistics.median_low(nonzero) if nonzero else 0.0
    # No non-zero deltas at all => the screen never changed; a spike is
    # undefined rather than infinite, so nothing is flagged.
    thresh = SPIKE_RATIO * med_nz if med_nz > ZERO_EPS else float("inf")
    spikes = [(i, d) for i, d in enumerate(deltas) if d > thresh]

    # --- morph window ----------------------------------------------------
    # If capture.sh ran with --trigger, trigger.log holds the absolute time the
    # command went out. Frames landing inside [trigger, trigger + MORPH_MS] are
    # the ones where a stalled redraw or a jump is a real animation fault;
    # zero-delta frames outside that window are just a still pill.
    MORPH_MS = 160
    win = None
    tlog = outdir / "trigger.log"
    if tlog.exists() and starts:
        for line in tlog.read_text().splitlines():
            if line.startswith("trigger ") and "FAILED" in line:
                win = {"failed": True}
                continue
            if line.startswith("trigger ") and " at " in line:
                try:
                    tns = int(line.rsplit(" at ", 1)[1])
                except ValueError:
                    continue
                lo = (tns - starts[0]) / 1e6
                hi = lo + MORPH_MS
                idx_in = [i for i in range(len(starts))
                          if lo <= (starts[i] - starts[0]) / 1e6 <= hi]
                d_in = [deltas[i - 1] for i in idx_in if 1 <= i <= len(deltas)]
                win = {
                    "trigger_ms": round(lo, 0),
                    "window_ms": [round(lo, 0), round(hi, 0)],
                    "frames_in_window": len(idx_in),
                    "distinct_in_window": sum(1 for d in d_in if d > ZERO_EPS),
                    "duplicates_in_window": sum(1 for d in d_in if d <= ZERO_EPS),
                    "spikes_in_window": sum(1 for d in d_in if d > thresh),
                }
                break

    # histogram: log-ish buckets that keep zero distinct from "tiny"
    edges = [0.0, 0.05, 0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 1e9]
    hist = []
    for lo, hi in zip(edges, edges[1:]):
        n = sum(1 for d in deltas if (d <= lo + 1e-12 if lo == 0 else lo < d <= hi))
        hist.append((f"{lo:g}-{'inf' if hi >= 1e9 else format(hi, 'g')}", n))

    res = {
        "outdir": str(outdir),
        "region_device_px": [x0, y0, x1, y1],
        "frames_requested": requested,
        "frames_captured": captured,
        "frames_compared": kept,
        "drop_rate_pct": round(drop_rate, 1) if requested else None,
        "interval_ms": {"median": med, "mean": mean, "p95": p95},
        "grim_ms_median": gmed,
        "achieved_fps": round(1000.0 / mean, 1) if mean else None,
        "loadavg_start": meta.get("loadavg_start"),
        "nproc": meta.get("nproc"),
        "delta_stats": {
            "median_nonzero": round(med_nz, 3),
            "max": round(max(deltas), 3) if deltas else None,
            "spike_threshold": (None if thresh == float("inf")
                                else round(thresh, 3)),
        },
        "zero_delta_frames": zero,
        "zero_delta_count": len(zero),
        "spike_frames": [{"idx": i, "delta": round(d, 3)} for i, d in spikes],
        "spike_count": len(spikes),
        "morph_window": win,
        "histogram": hist,
        "meta": meta,
    }

    if as_json:
        print(json.dumps(res, indent=2))
        return 0

    print(f"== motion capture analysis: {outdir}")
    if meta.get("trigger"):
        print(f"   trigger: {meta['trigger']}")
    print(f"\nFRAMES      requested {requested or '?'}  captured {captured}  "
          f"drop {res['drop_rate_pct'] if res['drop_rate_pct'] is not None else '?'}%")
    if intervals:
        print(f"INTERVAL    median {med} ms   mean {mean} ms   p95 {p95} ms   "
              f"(grim itself: {gmed} ms median)  => {res['achieved_fps']} fps achieved")
    else:
        print("INTERVAL    no timestamps.txt (run capture.sh to get them)")
    load = meta.get("loadavg_start", "")
    if load:
        nproc = meta.get("nproc", "?")
        la1 = float(load.split()[0])
        print(f"LOAD        1-min loadavg at start {la1} on {nproc} cores"
              + ("  <-- CONTENDED: interval stats are an upper bound on lateness"
                 if nproc != "?" and la1 > 1.0 * int(nproc) else ""))
    print(f"CROP        device px {x0},{y0} -> {x1},{y1}  ({x1-x0}x{y1-y0})\n")
    print(f"DELTA       median(nonzero) {res['delta_stats']['median_nonzero']}  "
          f"max {res['delta_stats']['max']}  spike thr {res['delta_stats']['spike_threshold']}")
    print("HISTOGRAM of consecutive-frame mean-abs delta")
    for label, n in hist:
        bar = "#" * min(60, n)
        print(f"  {label:>14}  {n:4d} {bar}")
    print(f"\nDUPLICATES  {len(zero)} frame(s) identical to predecessor "
          f"(stalled redraw if mid-animation)")
    if zero:
        print("            idx: " + ", ".join(str(i) for i in zero[:60])
              + (" ..." if len(zero) > 60 else ""))
    thr_disp = ("n/a (static)" if thresh == float("inf")
                else f"{res['delta_stats']['spike_threshold']}")
    print(f"SPIKES      {len(spikes)} jump(s) > {thr_disp} "
          f"(skipped intermediate frame)")
    for s in res["spike_frames"][:20]:
        print(f"            idx {s['idx']:4d}  delta {s['delta']}")

    if win and win.get("failed"):
        print("\nMORPH WINDOW  n/a — the trigger command FAILED to send; this "
              "capture contains no animation")
    elif win:
        print(f"\nMORPH WINDOW  t={win['window_ms'][0]:.0f}..{win['window_ms'][1]:.0f} ms "
              f"(160 ms morph), {win['frames_in_window']} frame(s) landed in it")
        print(f"            distinct {win['distinct_in_window']}  "
              f"duplicate {win['duplicates_in_window']}  "
              f"spike {win['spikes_in_window']}")
        if win["frames_in_window"] < 4:
            print("            WARNING: too few frames inside the morph to judge "
                  "smoothness — raise fps or shrink --region")
    return 0


if __name__ == "__main__":
    sys.exit(main())
