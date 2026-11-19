#!/usr/bin/env python3
"""analyze.py <outdir> [--region x0,y0,x1,y1] [--json]

Measures a captured frame sequence, from either front end:
  record.sh  wf-recorder -> real compositor PTS, 60 Hz sampling (meta says
             capture_clock=compositor_pts); the honest pacing path.
  capture.sh grim loop    -> wall-clock interval, ~20 fps ceiling (no
             capture_clock key); intervals are grim-limited, pixels are not.

Prints, all from measured pixels plus the frame timestamps:
  - frames captured vs requested (drop rate)
  - inter-frame interval: median / mean / p95
  - histogram of consecutive-frame mean-absolute-pixel-delta
  - MOTION WINDOW: the span where the screen was actually changing, found
    from the pixels (anchored on --trigger when one was sent)
  - poses      how many DISTINCT shapes reached the screen in that span
  - duplicates frames inside that span identical to their predecessor
               = a display slot that carried no new content = stalled redraw
  - spikes     a step > SPIKE_RATIO x the window's median step
               = a swallowed intermediate pose = jump
  - SCORE      one number, formula printed next to it

Caveat that governs how to read this: with the grim front end the INTERVAL
numbers describe how fast grim could go, not how the compositor clocked, so a
score computed there is CONTENT-ONLY. With record.sh the timestamps are the
compositor's, and in cfr mode every display slot is sampled, so a duplicate
frame is genuinely a slot without new content. The DELTA and POSE numbers are
trustworthy in both cases: they say whether the panel put new pixels on screen.
A run where the pill is static shows many zero-delta frames and NO motion
window — that is not smooth, it is unmeasurable, and the score says so.
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

# --- smoothness score inputs ------------------------------------------------
# The panel animates a 160 ms morph on a 16 ms timer (src/main.rs:4546,4572),
# i.e. it aims for ~10 poses at 60 Hz. That is the reference "complete" morph.
POSE_TARGET = 10
PANEL_HZ = 60.0
# A gap longer than this between two changed frames ends one animation and
# starts another; without it a stalled redraw would split a morph in two.
MOTION_GAP_MS = 100.0
# Everything the panel paints within this long after the trigger is ONE
# transition. The morph is designed for 160 ms; a build that spreads the same
# transition over 300 ms is still the same transition to the user, and splitting
# it into "separate animations" would flatter it (each fragment looks smooth).
TRANSITION_MS = 400.0
# A window spanning more than this is sustained motion, not one transition.
# Still scored, but a 3 s window is not "the morph" — see docs/.
SUSTAINED_MS = 1500.0
# A delta this large means most of the crop was replaced at once (workspace
# switch, a window swapping, wallpaper change), not an animation. Measured
# range: panel morph steps 15-40, a workspace switch 180-215.
CONTAMINATION_DELTA = 80.0


def load_any(p: Path) -> np.ndarray:
    im = Image.open(p)
    if im.mode not in ("RGB", "L"):
        im = im.convert("RGB")
    return np.asarray(im, dtype=np.uint8)


def parse_region(s: str):
    x0, y0, x1, y1 = (int(v) for v in s.split(","))
    return x0, y0, x1, y1


def analyse_motion(run, rel, deltas, frames, x0, y0, x1, y1):
    """Metrics for one contiguous spell of screen change.

    `run` is the list of frame indices that carried a new pose. Everything here
    is derived from pixels plus timestamps, so it means the same thing whether
    the frames came from a grim loop or a real recording.
    """
    w0, w1 = run[0], run[-1]
    frames_in_window = w1 - w0 + 1
    poses = len(run)
    duplicates = frames_in_window - poses
    # Span runs from the last static frame before the change to the last
    # changed frame: the time the screen spent morphing, not the time it spent
    # finished.
    lo_ms = rel[w0 - 1] if w0 >= 1 else rel[w0]
    span_ms = rel[w1] - lo_ms

    win_deltas = deltas[w0 - 1:w1]          # entry step + internal steps
    nz = [d for d in win_deltas if d > ZERO_EPS]
    med_nz = statistics.median_low(nz) if nz else 0.0
    thr = SPIKE_RATIO * med_nz if med_nz > ZERO_EPS else float("inf")
    spikes = [d for d in win_deltas if d > thr]

    pose_rate = poses / (span_ms / 1000.0) if span_ms > 0 else None
    # Completeness: the fraction of the ideal 60 Hz pose stream that actually
    # arrived. Two independent caps, because either alone is gameable: a
    # 2-pose teleport has a huge pose RATE but almost no poses, and a slow
    # 20-pose morph has plenty of poses but a low rate.
    if pose_rate is None:
        completeness = None
    else:
        # Use UNIQUE poses, not the raw change count. A two-state strobe
        # (A->B->A->B...) changes every frame and would otherwise collect a
        # perfect completeness while showing no motion at all — critic #23b
        # built exactly that and scored 100.0. Unique shapes is the honest
        # numerator: a real ramp has one per frame, a strobe has two.
        uniq = count_unique_frames(run, frames, x0, y0, x1, y1)
        completeness = min(1.0, uniq / POSE_TARGET, pose_rate / PANEL_HZ)
    dup_rate = duplicates / max(1, frames_in_window - 1)
    # A strobe's steps are all the SAME size, so the spike test sees nothing
    # unusual (threshold is 1.8x the median) and J came out 0.0 -- scoring a
    # two-state flicker 68, above a real slideshow's 48.7. What is wrong with
    # a strobe is not step size but that it REVISITS shapes instead of
    # advancing. Count steps that fail to show a new shape as jumps too.
    spike_rate = (len(spikes) / max(1, poses - 1)) if poses >= 2 else None
    revisit_rate = ((poses - uniq) / max(1, poses - 1)) if poses >= 2 else None
    jump_rate = max(spike_rate or 0.0, revisit_rate or 0.0) \
        if poses >= 2 else None
    return {
        "measurable": True,
        "window_ms": [round(lo_ms, 0), round(rel[w1], 0)],
        "span_ms": round(span_ms, 1),
        "frames_in_window": frames_in_window,
        "poses": poses,
        "poses_unique": uniq,
        "duplicates": duplicates,
        "duplicate_rate": round(dup_rate, 3),
        "spikes": len(spikes),
        "revisit_rate": None if revisit_rate is None else round(revisit_rate, 3),
        "jump_rate": None if jump_rate is None else round(min(1.0, jump_rate), 3),
        "spike_threshold": (None if thr == float("inf") else round(thr, 3)),
        "pose_rate_hz": None if pose_rate is None else round(pose_rate, 1),
        "completeness": None if completeness is None else round(completeness, 3),
        "sustained": span_ms > SUSTAINED_MS,
    }


def count_unique_frames(run, frames, x0, y0, x1, y1):
    """How many of the window's frames are distinct from EVERY other frame in
    the window, not just from the frame before it. A->B->A is three frames but
    two unique poses, and a bounce that never leaves the first pose is not
    motion — the detector must not report it as such.
    """
    seen = []
    for i in run:
        a = load_any(frames[i])[y0:y1, x0:x1]
        if not any(np.abs(a.astype(np.int16) - b.astype(np.int16)).mean() <= ZERO_EPS
                   for b in seen):
            seen.append(a)
    return len(seen)


def score_of(motion):
    """Headline number. Formula is printed with the number, always.

    S = 100 * (0.40*C + 0.35*(1-D) + 0.25*(1-J))
      C = completeness (see analyse_motion; capped by BOTH pose count and
          pose rate, so neither a teleport nor a slow slideshow can score high)
      D = duplicate-during-motion rate (stalled redraws)
      J = jump rate (swallowed intermediate poses)

    Pacing (dt median/p95) is deliberately NOT a term: in cfr mode every
    display slot is sampled, so dt is an imposed 16.7 ms grid that would hand
    out free points; in grim mode dt is grim's own speed. Pacing failures do
    reach the score through D and J, which are measured from pixels in both
    modes. A perfect 60 Hz 10-pose morph scores exactly 100.
    """
    if not motion or not motion.get("measurable"):
        return {"smoothness": None, "kind": "unmeasurable",
                "formula": SCORE_FORMULA,
                "why": "no motion window: the screen never changed beyond "
                       "ZERO_EPS, so there is nothing to score"}
    if motion["poses"] < 2:
        # One pose in the window: the whole transition landed in a single
        # painted frame. Nothing fresh and nothing continuous was shown, so
        # the failure terms are pinned to 1 rather than left undefined (a
        # window with one frame has no duplicate RATE, and calling that 0
        # would hand a teleport 35 free points).
        c = motion["completeness"]
        d, j = 1.0, 1.0
    else:
        c, d = motion["completeness"], motion["duplicate_rate"]
        j = motion["jump_rate"] if motion["jump_rate"] is not None else 1.0
    terms = {"completeness": round(c, 3), "fresh_slots": round(1 - d, 3),
             "continuous_steps": round(1 - j, 3)}
    s = 100.0 * (0.40 * c + 0.35 * (1 - d) + 0.25 * (1 - j))
    return {"smoothness": round(s, 1),
            "kind": "sustained" if motion.get("sustained") else "transition",
            "formula": SCORE_FORMULA, "terms": terms}


SCORE_FORMULA = "100*(0.40*C + 0.35*(1-D) + 0.25*(1-J))"


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
    # record.sh crops at capture time, so its frames are already the region:
    # re-cropping with --region would slice the wrong pixels (and iw would no
    # longer scale to the logical screen).
    pre_cropped = meta.get("crop_applied_by_recorder") == "1"
    if pre_cropped:
        x0, y0, x1, y1 = 0, 0, iw, ih
    elif region:
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

    # deltas[i] is the step INTO frame i+1, so report frame numbers, not
    # list offsets (a reviewer reading "idx 8" means frame_0008).
    zero = [i for i, d in enumerate(deltas, start=1) if d <= ZERO_EPS]
    nonzero = [d for d in deltas if d > ZERO_EPS]
    # median_LOW, not median: with an even (or tiny) count of non-zero deltas
    # the midpoint sits half-way to the outlier, which hides the very jump we
    # are looking for. median_low reports a real observed step instead.
    med_nz = statistics.median_low(nonzero) if nonzero else 0.0
    # No non-zero deltas at all => the screen never changed; a spike is
    # undefined rather than infinite, so nothing is flagged.
    thresh = SPIKE_RATIO * med_nz if med_nz > ZERO_EPS else float("inf")
    spikes = [(i, d) for i, d in enumerate(deltas, start=1) if d > thresh]

    # --- motion window ---------------------------------------------------
    # Where was the screen actually changing? Found from the pixels, not from
    # an assumed 160 ms. `rel` is each frame's time offset from the first
    # frame; deltas[i-1] is the transition INTO frame i, so a frame index in
    # `moving` is a frame that carried a new pose.
    rel = [(s - starts[0]) / 1e6 for s in starts] if starts else []
    anchor = None
    win = None
    tlog = outdir / "trigger.log"
    if tlog.exists() and starts:
        for line in tlog.read_text().splitlines():
            if line.startswith("trigger ") and "FAILED" in line:
                win = {"failed": True}
                continue
            if line.startswith("trigger ") and " at " in line:
                try:
                    anchor = (int(line.rsplit(" at ", 1)[1]) - starts[0]) / 1e6
                except ValueError:
                    anchor = None
                break

    motion = None
    if len(rel) >= 2 and len(deltas) == len(frames) - 1:
        moving = [i for i, d in enumerate(deltas, start=1) if d > ZERO_EPS]
        if moving:
            runs = [[moving[0]]]
            for i in moving[1:]:
                if rel[i] - rel[runs[-1][-1]] <= MOTION_GAP_MS:
                    runs[-1].append(i)
                else:
                    runs.append([i])
            if anchor is not None:
                # Not the run nearest the trigger: on a stalled build the same
                # transition arrives as several one-frame runs separated by
                # 100-150 ms of nothing, and picking one of them reports a
                # single pose and hides the stall. Take every frame the panel
                # painted inside the transition band instead.
                band = [i for i in moving
                        if anchor - 100.0 <= rel[i] <= anchor + TRANSITION_MS]
                pick = band or [min(moving, key=lambda i: abs(rel[i] - anchor))]
            else:
                pick = max(runs, key=len)
            motion = analyse_motion(pick, rel, deltas, frames, x0, y0, x1, y1)

    score = score_of(motion)

    # histogram: log-ish buckets that keep zero distinct from "tiny"
    edges = [0.0, 0.05, 0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 1e9]
    hist = []
    for lo, hi in zip(edges, edges[1:]):
        # first bucket is inclusive of ZERO_EPS too: frames the run already
        # counts as duplicates must appear here or the bars do not add up
        n = sum(1 for d in deltas if (d <= hi if lo == 0 else lo < d <= hi))
        hist.append((f"{lo:g}-{'inf' if hi >= 1e9 else format(hi, 'g')}", n))

    clock = meta.get("capture_clock", "wall_clock")
    # This screen is shared with other agents and with the user. Anything that
    # moved BEFORE our trigger, or replaced most of the crop in one step, was
    # not our animation, and a score built on it would be a phantom.
    pre = ([d for i, d in enumerate(deltas, start=1) if rel[i] < anchor - 100.0]
           if anchor is not None else None)
    # A big step INSIDE the motion window is the morph itself: an expanding
    # panel replaces most of a tight crop. Only outside the window does a big
    # step mean somebody else touched the screen.
    w_lo = anchor - 100.0 if anchor is not None else None
    w_hi = (motion["window_ms"][1] + 200.0) if motion else None
    swaps = [i for i, d in enumerate(deltas, start=1)
             if d > CONTAMINATION_DELTA
             and (w_lo is None or rel[i] < w_lo or (w_hi is not None and rel[i] > w_hi))]
    pre_n = None if pre is None else sum(1 for d in pre if d > ZERO_EPS)
    contamination = {
        "pre_trigger_changes": pre_n,
        "off_window_swaps": len(swaps),
        "clean": (pre_n == 0 and not swaps),
    }
    res = {
        "outdir": str(outdir),
        "capture_clock": clock,
        "recorder_mode": meta.get("recorder_mode"),
        "region_device_px": [x0, y0, x1, y1],
        "frames_requested": requested,
        "frames_captured": captured,
        "frames_compared": kept,
        "drop_rate_pct": round(drop_rate, 1) if requested else None,
        "interval_ms": {"median": med, "mean": mean, "p95": p95},
        "interval_source": ("compositor_pts" if clock == "compositor_pts"
                            else "wall_clock_grim_limited"),
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
        "trigger_ms": None if anchor is None else round(anchor, 0),
        "contamination": contamination,
        "motion": motion,
        "score": score,
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
    clock_note = ("compositor PTS" if clock == "compositor_pts"
                  else "grim wall clock — describes grim's speed, not the compositor's")
    print(f"\nCLOCK       {clock_note}")
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
        print("\nMOTION WINDOW  n/a — the trigger command FAILED to send; this "
              "capture contains no animation")
    elif motion:
        kind = "sustained" if motion["sustained"] else "one transition"
        print(f"\nMOTION WINDOW  t={motion['window_ms'][0]:.0f}..{motion['window_ms'][1]:.0f} ms "
              f"({kind}, span {motion['span_ms']:.0f} ms, "
              f"{motion['frames_in_window']} frame(s) inside)")
        print(f"            poses {motion['poses']} distinct "
              f"({motion['poses_unique']} unique shapes)  "
              f"duplicate {motion['duplicates']}  spike {motion['spikes']}")
        print(f"            pose rate {motion['pose_rate_hz']}/s   "
              f"completeness {motion['completeness']} "
              f"(target {POSE_TARGET} poses = 160 ms @ {PANEL_HZ:.0f} Hz)")
        print(f"            duplicate-during-motion {motion['duplicate_rate']}   "
              f"jump rate {motion['jump_rate']} (step thr {motion['spike_threshold']})")
        if motion["frames_in_window"] < 4:
            print("            WARNING: too few frames inside the motion to judge "
                  "smoothness — capture faster or shrink --region")
    else:
        print("\nMOTION WINDOW  none — no consecutive frames differed by more than "
              f"{ZERO_EPS}; nothing animated, so smoothness is UNMEASURABLE "
              "here (not perfect)")

    c = res["contamination"]
    print(f"\nCLEAN       {'yes' if c['clean'] else 'NO — do not trust this trial'}"
          f"  (changes before the trigger {c['pre_trigger_changes']}, "
          f"big steps outside the window {c['off_window_swaps']} "
          f"> {CONTAMINATION_DELTA:g})")
    sc = res["score"]
    if sc["smoothness"] is None:
        print(f"\nSCORE       n/a — {sc['why']}")
    else:
        t = sc["terms"]
        print(f"\nSCORE       {sc['smoothness']} / 100   [{sc['kind']}]")
        print(f"            = {SCORE_FORMULA}")
        print(f"            = 100*(0.40*{t['completeness']} + 0.35*{t['fresh_slots']}"
              f" + 0.25*{t['continuous_steps']})")
    # A capture with no animation in it is not a smoothness measurement.
    # Printing a tidy summary and exiting 0 let a dead trial read as data
    # (the 20261118-213403 morph capture: trigger ENOTCONN, all 80 deltas 0).
    # Fail loudly so no caller can score it.
    if win and win.get("failed"):
        print("\nRESULT FAIL: the trigger never reached the panel — this "
              "capture contains NO animation and must not be scored.")
        print("             (Errno 107 ENOTCONN usually means the panel was "
              "restarting; re-run in a quiet window.)")
        return 3
    return 0


if __name__ == "__main__":
    sys.exit(main())
