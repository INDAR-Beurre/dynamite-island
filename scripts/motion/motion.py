#!/usr/bin/env python3
"""Motion smoothness instrument for the dynamite-rust island.

Drives one known state transition, captures a burst of cropped frames with
`grim`, and emits per-frame geometry plus smoothness metrics judged against
explicit bounds. See README.md for metric meaning, bounds and blind spots.

The island is a COMPOUND object: a ~106 px capsule (y 5..45) plus a panel body
that appears below it (y ~50..126). For `controlcenter` the capsule width does
NOT change -- only the body appears. Measuring capsule width therefore proves
nothing; the morph signal used here is `total_h` (capsule top -> body bottom),
which travels 41 -> 121 device px.
"""
import argparse
import json
import os
import re
import shutil
import socket
import statistics
import subprocess
import sys
import threading
import time

import numpy as np
from PIL import Image

# ---- detector / capture constants (device px unless named *logical*) -------
DARK = 40          # luminance threshold for "island dark"
HALF = 110         # +- window around island centre used to find dark rows
CAP_MAX_H = 70     # a capsule is never taller than this
BODY_WMIN = 250    # narrower than this below the capsule = text, not a body
BODY_WMAX = 1250   # wider = terminal window / wallpaper, not the island
BODY_MAX_GAP = 12  # rows the body may start below the capsule (one rounded rect)
# 220 was sized when the panel HUNG BELOW the pill. Since r1d §0 the panel
# grows INTO the pill band, so the measured object is the whole capsule:
# ControlCenter is 447 logical x 1.5 = 670 device px. A 220 cap truncated it
# at 147 and read the residual 76 as "overshoot_frac 1.75".
BODY_MAX_H = 700
PAD_DEV = 620      # crop half-width around island centre
BAND_DEV = 760     # crop height: must contain the tallest panel (670) + top
                   # offset + shadow. 190 was the pre-§0 pill+hang-below band.
SCALE = 1.5
# scratch frames: /tmp here is a small tmpfs and a burst of PPMs is ~0.5 GB
SHM = "/dev/shm" if os.path.isdir("/dev/shm") else "/tmp"

BOUNDS = {
    "capture_median_interval_ms": 100.0,  # >= 10 fps
    "interval_p95_over_median": 3.0,
    "dropped_frame_ratio": 0.15,
    "duplicate_ratio_fast": 0.10,         # median interval < 30 ms
    "duplicate_ratio_slow": 0.50,         # median interval >= 30 ms
    "reversals_max": 1,                   # EaseOutCubic is monotone; 1 = noise
    "overshoot_min": 0.0,
    "overshoot_max": 0.05,
    "travel_min_px": 20.0,
    "settle_bound_ms": 260.0,             # anim_ms + one capture interval + slack
    "settle_duration_ms": 230.0,          # motion only: anim_ms + interval + 30
    "latency_bound_ms": 260.0,            # command -> first visible change
    "edge_ramp_px": 3.0,                  # reference ramp ~1.5 px
    "min_window_samples": 8,
    # Morph smoothness is judged on POSE COUNT, not on geometry continuity.
    # wf-recorder at 60 Hz proved the panel emits only 2-3 distinct poses in a
    # 160 ms morph, so a "backwards" step in the width series is a symptom, and
    # pose count is the quantity the user actually perceives ("frames of the
    # animation"). 160 ms at 60 Hz = 9.6 frame slots; 9 poses in 160 ms means
    # every display frame was distinct.
    "min_morph_poses": 8,        # distinct poses inside the morph window
    "min_pose_hold_ms": 8.0,     # a pose must persist this long to count
    "min_dt_bins": 5,
    "dt_bin_ms": 10.0,
    "pixel_delta_epsilon": 0.001,
    "pixel_delta_level": 2,
    "settle_tolerance_px": 2.0,
}


# --------------------------------------------------------------------------- #
class Cmd:
    def __init__(self):
        self.path = os.path.join(
            os.environ.get("XDG_RUNTIME_DIR", "/run/user/1000"),
            "dynamite-rust.sock")
        self.s = socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM)
        self.s.setblocking(False)
        self.failures = 0

    def send(self, word):
        """Non-blocking send with backpressure retry -> (ok, perf_counter)."""
        for _ in range(120):
            try:
                self.s.sendto(word.encode(), self.path)
                return True, time.perf_counter()
            except OSError as e:
                self.failures += 1
                if e.errno not in (11, 105):
                    raise
                time.sleep(0.02)
        return False, None


# --------------------------------------------------------------------------- #
def runs_of(mask_row, lo, hi):
    idx = np.flatnonzero(mask_row[lo:hi])
    if idx.size == 0:
        return []
    brk = np.flatnonzero(np.diff(idx) > 1)
    st = np.concatenate(([0], brk + 1))
    en = np.concatenate((brk, [idx.size - 1]))
    return [(int(idx[a]) + lo, int(idx[b]) + lo) for a, b in zip(st, en)]


def centre_run(mask_row, cx, lo, hi):
    """Widest contiguous dark run bracketing the island centre."""
    best = None
    for a, b in runs_of(mask_row, lo, hi):
        if a <= cx <= b and (best is None or b - a > best[1] - best[0]):
            best = (a, b)
    return best


def background_level(lum, cx):
    """Luminance of the backdrop immediately left/right of the island.

    A fixed `lum <= 40` cut works only while the wallpaper below the pill is
    BRIGHT. Measured live: with a dark-navy wallpaper the backdrop is 21 and the
    pill is 2, so an absolute cut sees no island at all and the instrument
    reports "nothing detected" on a perfectly good frame. The threshold is
    therefore derived from the actual backdrop each frame.
    """
    H, W = lum.shape
    rows = lum[:min(H, 80)]
    strips = []
    if W > 120:
        strips.append(rows[:, :40])
        strips.append(rows[:, W - 40:])
    if not strips:
        return float(np.median(rows))
    v = np.concatenate([x.ravel() for x in strips])
    return float(np.median(v))


def dark_threshold(bg):
    """Absolute cut for background `bg`: 40 while the backdrop is bright,
    else 8 below it (the pill is authored near-black, RGB 0-8)."""
    return DARK if bg >= 60 else max(4.0, bg - 8.0)


def edge_ramps(lum_row, a, b):
    """Antialiasing width of both edges of a dark run, in px."""
    out = []
    if b - a < 8:
        return out
    for side in ("l", "r"):
        inner = float(np.median(lum_row[a + 1:a + 5])) if side == "l" \
            else float(np.median(lum_row[b - 3:b + 1]))
        bg_slice = lum_row[max(0, a - 14):max(1, a - 4)] if side == "l" \
            else lum_row[min(len(lum_row), b + 5):min(len(lum_row), b + 15)]
        if bg_slice.size == 0:
            continue
        bg = float(np.median(bg_slice))
        # relative, not absolute: a dark backdrop (21) against a black pill (2)
        # has only 19 levels of contrast in total, and 40 would skip it.
        floor = max(6.0, (bg - inner) * 0.08)
        if bg - inner < floor:
            continue
        n = 1
        if side == "l":
            p = a
            while p - 1 >= 0 and inner + floor < lum_row[p - 1] < bg - floor:
                p -= 1
                n += 1
        else:
            q = b
            while q + 1 < len(lum_row) and inner + floor < lum_row[q + 1] < bg - floor:
                q += 1
                n += 1
        out.append(float(n))
    return out


def analyse_frame(path, cx, prev_path=None):
    """Island geometry for one frame, or None if nothing detected.

    Model (rewritten after the height series proved the old one wrong): the
    island is ONE rounded capsule, and since r1d §0 an expanded panel grows
    INTO the pill band, so there is no "pill then body" pair to separate.
    The measured object is therefore the widest dark run per row and the
    contiguous vertical span of those rows.

    Why not the centre column: a glyph, toggle or icon crossing screen centre
    narrows that run to ~190 px while the panel is still 656 px wide, and the
    old scan stopped there. Measured on a settled ControlCenter: real bottom
    607 device px, old detector reported 81, so the morph's target height
    read 77 instead of ~600 and every derived gate (latency, settle,
    overshoot) was measuring the detector's failure, not the animation.
    """
    a = np.asarray(Image.open(path).convert("RGB"), dtype=np.int32)
    lum = (a[:, :, 0] * 299 + a[:, :, 1] * 587 + a[:, :, 2] * 114) // 1000
    H, W = lum.shape
    bg = background_level(lum, cx)
    thr = dark_threshold(bg)
    dark = lum < thr
    cx = int(min(max(cx, 0), W - 1))

    # Widest dark run per row was tried twice and is wrong in both
    # directions: stopping at a bright row truncates the panel at its first
    # label (ControlCenter read 275 of 670), and bridging gaps to the end of
    # the crop measures the whole wallpaper (collapsed read 1484 of 37).
    #
    # What works is CONNECTIVITY. Close the dark mask so glyph/label holes
    # join their own panel, label the components, and take the one containing
    # the pill. Verified against hand-measured ground truth:
    #   settled ControlCenter  h 670 w 658 top 7
    #   settled themes         h 549 w 900 top 20
    #   settled music          h 225 w 918 top 20
    #   collapsed pill         h  37 w 100 top 20
    from scipy import ndimage
    closed = ndimage.binary_closing(dark, structure=np.ones((15, 5)))
    lab, _n = ndimage.label(closed)
    # seed: the dark run nearest the top on the centre-ish column
    seed = None
    for y in range(min(80, H)):
        if closed[y, cx]:
            seed = lab[y, cx]
            break
    if not seed:
        # centre column may be bright; take the largest component in the band
        band = lab[:80]
        ids, counts = np.unique(band[band > 0], return_counts=True)
        if ids.size == 0:
            return None
        seed = int(ids[np.argmax(counts)])
    ys, xs = np.where(lab == seed)
    if ys.size == 0:
        return None
    cap_y0 = int(ys.min())
    bottom = int(ys.max())
    x0, x1 = int(xs.min()), int(xs.max())
    cap_w = x1 - x0 + 1
    # A component spanning the crop is the desktop behind the island, not the
    # island: a black panel over a black app window is ONE dark region. No
    # pixel heuristic can split two identical blacks (a difference-against-
    # baseline detector was tried and fails in the same place, because black
    # over black does not change either). Flag the geometry unresolved and
    # let the pose metric, which counts change in the band, still stand.
    merged = cap_w > min(PANEL_WMAX_DEV, W - 8)

    g = {
        "cap_y0": cap_y0,
        "cap_h": int(bottom - cap_y0 + 1),
        "cap_w": int(cap_w),
        "body_top": None,
        "body_h": 0,
        "body_w": int(cap_w),
        "body_area": int(dark[cap_y0:bottom + 1].sum()),
        "region_left": x0,
        "region_right": x1,
    }
    g["bottom"] = bottom
    g["total_h"] = int(bottom - cap_y0 + 1)

    ramps = []
    # AA ramp on the island's own left/right edges, measured on the widest
    # row of the detected component. A settled capsule is a single run there;
    # an expanded panel has glyphs crossing centre, so use the component's
    # actual bounds rather than the centre run.
    best_row = cap_y0
    best_w = 0
    for y in range(cap_y0, bottom + 1):
        row = np.where(lab[y] == seed)[0]
        if row.size and row[-1] - row[0] + 1 > best_w:
            best_w = int(row[-1] - row[0] + 1)
            best_row = y
    wr = centre_run(dark[best_row], cx, max(0, cx - HALF), min(W, cx + HALF))
    if wr:
        ramps = edge_ramps(lum[best_row], wr[0], wr[1])
    g["edge_ramp"] = round(float(np.mean(ramps)), 2) if ramps else None
    g["bg_lum"] = round(bg, 1)
    g["geometry_merged"] = bool(merged)
    g["dark_thr"] = round(thr, 1)

    if prev_path is not None:
        p = np.asarray(Image.open(prev_path).convert("RGB"), dtype=np.int32)
        if p.shape == a.shape:
            pl = (p[:, :, 0] * 299 + p[:, :, 1] * 587 + p[:, :, 2] * 114) // 1000
            g["delta"] = round(float(
                (np.abs(pl - lum) > BOUNDS["pixel_delta_level"]).mean()), 5)
        else:
            g["delta"] = None
    return g



# --------------------------------------------------------------------------- #
# wf-recorder source: real compositor PTS at ~58.8 fps instead of ~28 fps grim.
# Use it for every RELATIVE timing question (motion shape, settle duration,
# reversals, overshoot, duplicate frames, frame pacing). Do NOT trust its
# absolute command->first-pixel latency to better than one frame interval: PTS
# is relative to the recorder's own start, so the wall->PTS offset can only be
# recovered from "when did the last frame arrive", which is worth ~1 interval.
WF_STARTUP_S = 0.35   # wf-recorder needs ~0.1 s to init; wait generously


def wf_record(path, region, seconds, trig_delay, sock, word):
    """Record one burst; returns (list of (pts_s, ppm_path), meta).

    PTS comes from the file, so intervals are the compositor's, not our loop's.
    """
    p = subprocess.Popen(["wf-recorder", "-g", region, "-f", path,
                          "-D", "-c", "ffv1"],
                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    t_pop = time.perf_counter()
    deadline = t_pop + WF_STARTUP_S + 5.0
    while time.perf_counter() < deadline:
        if os.path.exists(path) and os.path.getsize(path) > 0:
            break
        time.sleep(0.01)
    # Fire as soon as the recorder is live. `-D` makes capture DAMAGE-driven:
    # on a quiet screen (empty tag, settled pill) nothing damages the output,
    # so the recorder goes silent after a fraction of a second. A phase delay
    # of 60-130 ms then lands the trigger AFTER the last captured frame and
    # every frame is pre-motion — measured: 14 frames spanning 0.219 s while
    # the burst ran 1.17 s, travel 0, scored as a product FAIL.
    time.sleep(min(trig_delay, 0.02))
    ok, t_trig = sock.send(word) if sock else (False, None)
    trig_rel = time.perf_counter() - t_pop
    time.sleep(seconds)
    p.send_signal(2)                       # SIGINT = clean flush
    t_sig = time.perf_counter()
    try:
        p.wait(20)
    except subprocess.TimeoutExpired:
        p.kill()
        p.wait(5)
    pts = subprocess.run(["ffprobe", "-v", "error", "-select_streams", "v:0",
                          "-show_entries", "frame=pts_time", "-of", "csv=p=0",
                          path], capture_output=True, text=True).stdout
    tv = [float(x.strip().rstrip(",")) for x in pts.split()
          if x.strip().rstrip(",")]
    if len(tv) < 4:
        return [], {"error": "no frames", "n_pts": len(tv),
                    "absolute_latency_resolved": False}
    # PTS -> wall clock cannot be recovered reliably from the recorder's
    # final frame. The old `t_sig - last_pts` mapping sometimes placed the
    # trigger BEFORE the first PTS (negative pts_trig) and manufactured a
    # ~340 ms flat prefix. Keep the relative PTS series for motion shape,
    # but mark absolute command->first-pixel latency unresolved instead of
    # turning a recorder-startup offset into a product defect.
    t_sig_rel = t_sig - t_pop
    offset = t_sig_rel - tv[-1]
    pts_trig = None
    # Did the capture still run when the word was sent? If the recorder's
    # last frame predates the trigger, the burst never saw the morph and the
    # trial is INVALID, not evidence about the panel.
    covered = tv[-1] + offset >= trig_rel
    return tv, {"offset_s": offset, "pts_trig": pts_trig, "n_pts": len(tv),
                "span_s": tv[-1] - tv[0], "sigint_rel_s": t_sig_rel,
                "trigger_rel_s": trig_rel, "trigger_covered": covered,
                "send_ok": ok,
                "absolute_latency_resolved": False}


def wf_extract(path, outdir, trial, pts_list):
    """Decode every frame to PPM, index-aligned with pts_list.

    Decoding the whole file (not a -ss window) is what keeps output index and
    ffprobe PTS index aligned; a seek could drop or duplicate the boundary
    frame and silently shift every timestamp by one interval.
    """
    os.makedirs(outdir, exist_ok=True)
    pat = os.path.join(outdir, f"w{trial:02d}_%05d.ppm")  # short-lived scratch
    # /tmp is a 7.8 G tmpfs on this box and a 700-frame burst is ~600 MB, so
    # these scratch PPMs live in /dev/shm; only the kept PNGs go to outdir.
    subprocess.run(["ffmpeg", "-v", "error", "-i", path,
                    "-fps_mode", "passthrough",
                    "-pix_fmt", "rgb24", pat], check=False)
    got = sorted(f for f in os.listdir(outdir)
                 if f.startswith(f"w{trial:02d}_") and f.endswith(".ppm"))
    if len(got) != len(pts_list):
        return []
    return [(pts_list[i], os.path.join(outdir, g)) for i, g in enumerate(got)]



# --------------------------------------------------------------------------- #
# Distinct-pose counting: the real smoothness signal.
#
# Why this and not geometry continuity: the panel's morph is authored as a
# geometry lerp (morph.rs frame_for_raw), but it only PRESENTS poses when the
# compositor releases a frame callback. Measured at 60 Hz, a 160 ms morph
# yields 2-3 distinct images, i.e. ~15 poses/sec, while the display refreshes
# 60 times/sec. Counting distinct poses in the morph window is therefore the
# number that must go up; the width series only says where the jumps are.
# --------------------------------------------------------------------------- #
POSE_EPS = 12          # per-channel delta that counts as a changed pixel
# Widest panel the crop can hold: 2*PAD_DEV = 1240 device px. A dark component
# reaching the crop edges is the desktop behind the island, not the island.
PANEL_WMAX_DEV = 1180
POSE_MIN_PX = 60       # changed pixels needed to call it a new pose
# Rows the island can occupy: tallest panel (ControlCenter 447 logical x 1.5
# = 670 device px) + its top offset. Must track BAND_DEV, not a second guess.
POSE_BAND_DEV = 700


def pose_delta(a_path, b_path, cx, half=None):
    """Changed-pixel count between two frames, island band only.

    Band-limited to the rows the island can occupy. Since r1d §0 the panel
    GROWS INTO the pill band, so that extent is the tallest panel (670 device
    px measured), not the old 120 logical px: with the old cap every change
    below row 180 was invisible and the pose counter went blind to the second
    half of every expansion, under-reporting smoothness.

    Contamination is held off by the x crop, the morph time window and
    POSE_MIN_PX — a blinking cursor is a handful of pixels, not 60.
    """
    half = half or 620
    la = Image.open(a_path).convert("L")
    lb = Image.open(b_path).convert("L")
    w, h = la.size
    x0, x1 = max(0, cx - half), min(w, cx + half)
    rows = min(h, POSE_BAND_DEV)
    na = np.asarray(la.crop((x0, 0, x1, rows)), dtype=np.int16)
    nb = np.asarray(lb.crop((x0, 0, x1, rows)), dtype=np.int16)
    if na.shape != nb.shape:
        return None
    return int((np.abs(na - nb) > POSE_EPS).sum())


def count_poses(frames, cx, t_from, t_to):
    """Distinct poses presented in [t_from, t_to] (ms, relative to command).

    Returns dict with n_poses, the pose change times, and the longest stretch
    of identical frames (the "frozen" interval a viewer sees as a hitch).
    """
    win = [f for f in frames if f.get("dt") is not None and t_from <= f["dt"] * 1000.0 <= t_to]
    if len(win) < 3:
        return {"n_poses": None, "changes_ms": [], "longest_hold_ms": None,
                "resolved": False, "frames_in_window": len(win),
                "window_ms": [round(t_from, 1), round(t_to, 1)]}
    changes, holds = [], []
    run_start = win[0]["dt"]
    prev = win[0]
    for f in win[1:]:
        d = pose_delta(prev["path"], f["path"], cx)
        if d is None:
            return {"n_poses": None, "changes_ms": [], "longest_hold_ms": None,
                    "resolved": False, "frames_in_window": len(win)}
        if d >= POSE_MIN_PX:
            changes.append(round((f["dt"] - prev["dt"]) * 1000.0 + prev["dt"] * 1000.0, 1))
            holds.append((f["dt"] - run_start) * 1000.0)
            run_start = f["dt"]
        prev = f
    holds.append((win[-1]["dt"] - run_start) * 1000.0)
    return {"n_poses": len(changes) + 1, "changes_ms": changes,
            "longest_hold_ms": round(max(holds), 1), "resolved": True,
            "frames_in_window": len(win),
            "window_ms": [round(t_from, 1), round(t_to, 1)],
            "pose_hold_median_ms": round(float(np.median(holds)), 1)}

# --------------------------------------------------------------------------- #
def layout_origin():
    """The output's LAYOUT ORIGIN, which is not (0,0).

    eDP-1 sits at y=1543 here. grim/wf-recorder take LAYOUT coordinates, so a
    region built from screen-relative pixels misses the output entirely:
    "supplied geometry did not intersect with any outputs", zero frames.
    run_all.sh already queried this; motion.py did not, so stage 00 died
    while stages 01-05 ran fine.
    """
    try:
        mons = json.loads(subprocess.run(
            ["mmsg", "get", "all-monitors"],
            capture_output=True, text=True, check=True).stdout)["monitors"]
        m = next((m for m in mons if m.get("active")), mons[0])
        return int(m["x"]), int(m["y"])
    except Exception:
        return 0, 0


ORIGIN_X, ORIGIN_Y = layout_origin()


def region_for(cx_dev, cap_y0_dev):
    x = int(round((cx_dev - PAD_DEV) / SCALE)) + ORIGIN_X
    y = int(round((cap_y0_dev - 4) / SCALE)) + ORIGIN_Y
    w = int(round(2 * PAD_DEV / SCALE))
    h = int(round(BAND_DEV / SCALE))
    return f"{max(0, x)},{max(0, y)} {w}x{h}", max(0, x - ORIGIN_X)


def grab(region, path):
    t0 = time.perf_counter()
    r = subprocess.run(["grim", "-g", region, "-t", "ppm", path],
                       capture_output=True)
    t1 = time.perf_counter()
    if r.returncode != 0:
        raise RuntimeError("grim: " + r.stderr.decode()[:200])
    return t0, t1


# --------------------------------------------------------------------------- #
def outcome(frames, pre, bounds, allow_relative=False):
    """Per-trial verdict: settled geometry, latency, reversals, overshoot.

    `allow_relative` keeps a wf-recorder trial usable after its absolute
    wall-clock trigger was rejected. The series is then indexed from the
    first captured frame, so only relative motion metrics are meaningful;
    `latency_ms` and command-relative `settle_ms` stay None.
    """
    if allow_relative:
        for i, f in enumerate(frames):
            f["dt"] = f["t0"] - frames[0]["t0"]
    w = [f for f in frames if f["dt"] is not None and f["geom"]]
    after = sorted([f for f in w if f["dt"] >= 0], key=lambda f: f["dt"])
    if len(after) < 3:
        return None
    # h0 must be THIS trial's start, not the calibration's: another agent can
    # leave the panel expanded, and using the calibrated h0 then turns the
    # starting state itself into "overshoot" (measured 1.167 of travel).
    before = [f["geom"]["total_h"] for f in w if f["dt"] is not None and f["dt"] < 0]
    h0 = statistics.median(before[-3:]) if before else pre["total_h"]
    last = after[-3:]
    target = statistics.median([f["geom"]["total_h"] for f in last])
    travel = target - h0
    # None = the change was never observed inside the burst. Reporting a large
    # number here would be an extrapolation, so absence stays absent.
    lat = None if allow_relative else next(
        (f["dt"] * 1000 for f in after
         if abs(f["geom"]["total_h"] - h0) > bounds["settle_tolerance_px"]),
        None)
    if lat is not None and lat > 900:
        lat = None
    tol = bounds["settle_tolerance_px"]
    settle = None
    # Observed onset, in this trial's own time base. The pose window and the
    # duplicate-frame window both need it: anchoring either at t=0 grades the
    # idle frames before the animation, which on the relative (wf) path is
    # wherever the recorder happened to start.
    onset_ms = next((f["dt"] * 1000 for f in after
                     if abs(f["geom"]["total_h"] - h0) > tol), None)
    if not allow_relative:
        for i, f in enumerate(after):
            if abs(f["geom"]["total_h"] - target) <= tol and \
                    all(abs(g["geom"]["total_h"] - target) <= tol for g in after[i:]):
                settle = f["dt"] * 1000
                break
    else:
        # Relative settle duration only: time from the first changed pose to
        # the final stable pose. It never claims command-to-settle latency.
        first_change = next((i for i, f in enumerate(after)
                             if abs(f["geom"]["total_h"] - h0) > tol), None)
        if first_change is not None:
            for i in range(first_change, len(after)):
                if abs(after[i]["geom"]["total_h"] - target) <= tol and \
                        all(abs(g["geom"]["total_h"] - target) <= tol
                            for g in after[i:]):
                    settle = None
                    settle_dur = (after[i]["dt"] - after[first_change]["dt"]) * 1000
                    break
            else:
                settle_dur = None
        else:
            settle_dur = None
    if not allow_relative:
        settle_dur = (settle - lat) if (settle is not None and lat is not None) else None
    stats_trial = [f["geom"]["total_h"] for f in after]
    rev = 0
    for i in range(1, len(stats_trial) - 1):
        d1 = stats_trial[i] - stats_trial[i - 1]
        d2 = stats_trial[i + 1] - stats_trial[i]
        if d1 * d2 < 0 and abs(d1) > 2 and abs(d2) > 2:
            rev += 1
    # overshoot is direction-aware: a shrinking morph overshoots BELOW target.
    peak = max(stats_trial) if travel >= 0 else min(stats_trial)
    over = max(0.0, (peak - target) if travel >= 0 else (target - peak))
    # how many distinct geometry values did we actually see in flight? with <3
    # the reversal and overshoot questions were never really asked.
    in_flight = [v for v in stats_trial if abs(v - h0) > tol and abs(v - target) > tol]
    return {
        "target_total_h": target,
        "travel_px": travel,
        "not_observed": lat is None,
        "onset_ms": onset_ms,
        "latency_ms": lat,
        "settle_ms": settle,
        "settle_duration_ms": settle_dur,
        "reversals": rev,
        "peak_total_h": peak,
        "start_total_h": h0,
        "overshoot_px": over,
        "overshoot_frac": (over / abs(travel)) if travel else None,
        "samples": len(after),
        "in_flight_samples": len(in_flight),
        "series_total_h": [int(v) for v in stats_trial],
        "series_dt_ms": [round(f["dt"] * 1000, 1) for f in after],
    }


def pct(v, q):
    if not v:
        return None
    s = sorted(v)
    k = (len(s) - 1) * q / 100.0
    f = int(k)
    c = min(f + 1, len(s) - 1)
    return s[f] + (s[c] - s[f]) * (k - f)


# --------------------------------------------------------------------------- #
def measure_on_empty_tag():
    """Move to an empty tag for the whole measurement, restore afterwards.

    The island is anchored over the owner's maximised dark editor window, and
    below y~90 the panel and that window are the same colour, so no pixel
    heuristic can separate them (measured: ControlCenter read 82 px of a real
    670, themes read 770 by merging the window). On an empty tag there is no
    window and the detector reads the true extents (CC 670, themes 549).
    The owner's tag is restored in a finally block: leaving them on an empty
    workspace would be a hostile side effect of running a test.
    """
    try:
        raw = subprocess.run(["mmsg", "get", "all-tags"], capture_output=True,
                             text=True, check=True).stdout
        tags = json.loads(raw)["all_tags"][0]["tags"]
        current = next(t["index"] for t in tags if t["is_active"])
        # Prefer a HIGH empty tag. Mango keeps the previously focused client
        # visible for a moment after `view,N`, and tag 2 still showed the
        # owner's editor while reporting client_count 0 — the panel then
        # merged with that dark window and read 746x1236. Tags 7-9 are far
        # from anything the owner works in and measured clean.
        empties = [t["index"] for t in tags
                   if not t["is_active"] and t["client_count"] == 0]
        empty = max(empties) if empties else None
    except Exception:
        return None
    if empty is None:
        return None
    subprocess.run(["mmsg", "dispatch", f"view,{empty}"], capture_output=True)
    time.sleep(0.8)
    return current


def product_timing(src=None):
    """Read the live spring duration and settle tail from src/morph.rs."""
    path = src or os.path.join(os.path.dirname(os.path.dirname(
        os.path.dirname(os.path.abspath(__file__)))), "src", "morph.rs")
    text = open(path).read()
    out = []
    for name in ("SPRING_DURATION_MS", "SETTLE_MS"):
        m = re.search(r"pub const " + name + r"\s*:\s*u64\s*=\s*(\d+)", text)
        if not m:
            raise SystemExit(f"motion: cannot read {name} from {path}")
        out.append(float(m.group(1)))
    return out[0], out[1]


def run():
    ap = argparse.ArgumentParser()
    ap.add_argument("--state", default="controlcenter")
    ap.add_argument("--pre", default="collapsed")
    ap.add_argument("--trials", type=int, default=10)
    ap.add_argument("--burst", type=float, default=0.85)
    # Read the product's real timing, do not hardcode it: these two constants
    # drifted once already (the instrument kept grading a 160 ms morph long
    # after src/morph.rs moved to 233 + 150).
    ap.add_argument("--anim-ms", type=float, default=None,
                    help="morph duration; default: SPRING_DURATION_MS from src/morph.rs")
    ap.add_argument("--settle-ms", type=float, default=None,
                    help="settle tail; default: SETTLE_MS from src/morph.rs")
    ap.add_argument("--out", default=None)
    ap.add_argument("--json-out", default=None)
    ap.add_argument("--quiet", action="store_true")
    ap.add_argument("--source", choices=("grim", "wf"), default="grim",
                    help="grim: ~28 fps, absolute latency valid. wf: wf-recorder "
                         "~59 fps with real PTS, relative timing only")
    args = ap.parse_args()
    if args.anim_ms is None or args.settle_ms is None:
        d, s = product_timing()
        if args.anim_ms is None:
            args.anim_ms = d
        if args.settle_ms is None:
            args.settle_ms = s
    b = dict(BOUNDS)
    # Two DIFFERENT budgets. settle_duration measures the motion itself
    # (MORPH_DURATION_MS); settle_ms measures command-to-rest, which also
    # pays the settle tail (SETTLE_MS). Conflating them made every motion
    # reading look late by the length of the tail.
    b["settle_duration_ms"] = args.anim_ms + b["capture_median_interval_ms"] + 30.0
    b["settle_bound_ms"] = (args.anim_ms + args.settle_ms
                            + b["capture_median_interval_ms"] + 30.0)
    b["latency_bound_ms"] = b["settle_bound_ms"]

    outdir = args.out or (time.strftime("/tmp/motion-runs/%Y%m%d-%H%M%S-")
                          + args.state)
    os.makedirs(outdir, exist_ok=True)
    sock = Cmd()
    if not os.path.exists(sock.path):
        print(f"FATAL: no control socket at {sock.path}", file=sys.stderr)
        return 2

    # ---- calibration ------------------------------------------------------
    # The pre-state must be VERIFIED, not assumed: another agent drives the same
    # socket, and a stray launcher/controlcenter left open makes "collapsed"
    # measure as a 220 px wide, 70 px tall slab (cap_h pinned at CAP_MAX_H).
    # Calibrating on that yields a crop the capsule overflows, which then reads
    # as "no edge" for every frame. Retry, then refuse rather than measure.
    pre, cal_tries = None, []
    for attempt in range(5):
        sock.send(args.pre)
        time.sleep(0.9)
        cal = []
        for i in range(3):
            p = os.path.join(outdir, f"cal_{attempt}_{i}.ppm")
            probe_region, probe_x = region_for(2256 // 2, 5)
            grab(probe_region, p)
            g = analyse_frame(p, 1128 - int(probe_x * SCALE))
            if g:
                cal.append(g)
            time.sleep(0.06)
        if not cal:
            cal_tries.append("not_detected")
            continue
        cand = {k: int(round(np.median([c[k] for c in cal])))
                for k in ("cap_y0", "cap_h", "cap_w", "total_h", "body_h")}
        # a settled collapsed pill is ~106x41; anything at the height cap or with
        # a body is a different state, i.e. someone else's command won the race
        bad = (cand["cap_h"] >= CAP_MAX_H or cand["body_h"] > 0
               or cand["cap_w"] > 260 or cand["cap_h"] < 20)
        cal_tries.append({"pre": cand, "accepted": not bad})
        if not bad:
            pre = cand
            break
    if pre is None:
        print(f"FATAL: could not settle '{args.pre}' for calibration "
              f"({cal_tries}). Another agent is probably driving the socket; "
              f"retry when it is idle.", file=sys.stderr)
        return 2
    cx_dev = 1128
    region, crop_x_logical = region_for(cx_dev, pre["cap_y0"])
    cx_crop = int(round(cx_dev - crop_x_logical * SCALE))
    if not args.quiet:
        print(f"calibrated '{args.pre}': {pre}  crop='{region}'  "
              f"cx_crop={cx_crop}  trials={args.trials}", file=sys.stderr)

    # ---- trials -----------------------------------------------------------
    all_frames, trial_log, trials = [], [], []
    for ti in range(args.trials):
        sock.send(args.pre)
        time.sleep(0.75)              # let the pre-state settle
        phase = 0.06 + ((ti * 0.6180339887) % 1.0) * 0.07
        trig = {"t": None, "ok": False}

        def fire():
            ok, t = sock.send(args.state)
            trig["ok"], trig["t"] = ok, t

        frames = []
        if args.source == "wf":
            pv = os.path.join(outdir, f"w{ti:02d}.nut")
            tv, meta = wf_record(pv, region, args.burst, phase, sock, args.state)
            trig["t"] = meta.get("pts_trig")
            trig["ok"] = trig["t"] is not None
            scratch = os.path.join(SHM, f"motion-frames-{os.getpid()}")
            pairs = wf_extract(pv, scratch, ti, tv)
            for i, (pt, pp) in enumerate(pairs):
                frames.append({"i": i, "path": pp, "t0": pt, "t1": pt,
                               "trial": ti, "geom": None,
                               "dt": (pt - trig["t"])
                               if trig["t"] is not None else None})
            try:
                os.remove(pv)
            except OSError:
                pass
        else:
            timer = threading.Timer(phase, fire)
            timer.start()
            t0 = time.perf_counter()
            try:
                # capture loop ONLY: no image analysis here, or the sampler
                # interval becomes capture+analysis and the rate is understated.
                while time.perf_counter() - t0 < args.burst:
                    p = os.path.join(outdir, f"t{ti:02d}_{len(frames):03d}.ppm")
                    ta, tb = grab(region, p)
                    frames.append({"i": len(frames), "path": p, "t0": ta,
                                   "t1": tb, "trial": ti, "geom": None,
                                   "dt": (((ta + tb) / 2) - trig["t"])
                                   if trig["t"] is not None else None})
            finally:
                timer.cancel()
        # analysis happens after the burst so it cannot slow the capture down
        prev = None
        for f in frames:
            f["geom"] = analyse_frame(f["path"], cx_crop, prev)
            prev = f["path"]
        # Relative PTS is still valid for pose shape, pacing and settle
        # duration. Absolute command->first-pixel latency is unresolved for
        # wf-recorder (see wf_record), so keep the trial and its relative
        # series but do not invent a command-relative timestamp.
        o = outcome(frames, pre, b, allow_relative=(args.source == "wf"))
        if args.source == "wf" and o:
            o["absolute_latency_resolved"] = False
            # A damage-driven recorder that stopped before the trigger cannot
            # see the morph. Say so instead of scoring travel 0 as a defect.
            if meta and not meta.get("trigger_covered", True):
                o["recorder_starved"] = True
        # pose count over the morph window: command -> anim_ms + 2 intervals.
        # This is THE smoothness number: how many distinct images the panel
        # actually presented while it was supposed to be animating.
        # The morph window must be anchored to the OBSERVED first change, not to
        # the command: dispatch latency measured 200-450 ms, so a window of
        # [command, command+anim_ms] contains no motion at all and counts only
        # unrelated content ticks (clock, live data). Anchor on the measured
        # latency and settle, falling back to the whole burst when neither was
        # seen (then the pose count is genuinely unresolved).
        iv = b["capture_median_interval_ms"]
        anchor = o["latency_ms"] if (o and o["latency_ms"] is not None) \
            else (o["onset_ms"] if o else None)
        if anchor is not None:
            t_a = anchor - iv
            t_b = (o["settle_ms"] + iv) if o["settle_ms"] is not None \
                else (anchor + args.anim_ms + args.settle_ms + 2 * iv)
            pos = count_poses(frames, cx_crop, t_a, t_b)
            pos["window_anchored"] = True
        else:
            # Whole-burst fallback: the count then includes pre-motion idle
            # frames and cannot fail a build that pauses and jumps. Mark it
            # unresolved rather than reporting it as a smoothness headline.
            pos = count_poses(frames, cx_crop, 0.0, args.burst * 1000.0)
            pos["window_anchored"] = False
            pos["resolved"] = False
        if o:
            o["poses"] = pos
            # Geometry unreadable (panel merged with a dark window behind it):
            # the pose count still stands, the extents do not.
            o["geometry_merged"] = any(
                f["geom"] and f["geom"].get("geometry_merged") for f in frames)
        trials.append({k: v for k, v in o.items() if k != "series_total_h"}
                      | {"series_total_h": o["series_total_h"]})
        all_frames.extend(frames)
        trial_log.append({
            "trial": ti, "phase_s": round(phase, 3), "trigger_ok": trig["ok"],
            "onset_ms": round(o["onset_ms"], 1)
            if (o and o.get("onset_ms") is not None) else None,
            "frames": len(frames),
            "no_island": sum(1 for f in frames if not f["geom"]),
            "samples_after_trigger": o["samples"] if o else 0,
            # Audit the wall->PTS offset: absolute latency is only as good as
            # this mapping, and a long flat prefix must be attributable.
            "wf_meta": meta if args.source == "wf" else None,
            "series_dt_ms": [round(f["dt"] * 1000, 1) for f in frames
                             if f["dt"] is not None],
            "total_h_series": o["series_total_h"] if o else None,
            "start_total_h": o["start_total_h"] if o else None,
            "target_total_h": o["target_total_h"] if o else None,
            "travel_px": o["travel_px"] if o else None,
            "latency_ms": round(o["latency_ms"], 1)
            if o and o["latency_ms"] is not None else None,
            "settle_ms": round(o["settle_ms"], 1)
            if o and o["settle_ms"] is not None else None,
            "in_flight_samples": o["in_flight_samples"] if o else None,
            "morph_poses": (o["poses"]["n_poses"] if o and o["poses"]["resolved"]
                            else None),
            "pose_hold_median_ms": (o["poses"]["pose_hold_median_ms"]
                                    if o and o["poses"]["resolved"] else None),
            "pose_window_ms": (o["poses"].get("window_ms") if o else None),
            "not_observed": o["not_observed"] if o else None,
        })
        if not args.quiet:
            print(f"  trial {ti:2d}: {len(frames):2d} frames, "
                  f"h={o['series_total_h'] if o else 'n/a'}"
                  f" poses={o['poses']['n_poses'] if o else 'n/a'}",
                  file=sys.stderr)
    sock.send(args.pre)               # leave the panel where we found it

    ok = [t for t in trials if t]
    # ---- contamination screen -------------------------------------------
    # Another agent drives the same socket; a stray `collapsed` from them shows
    # up as a trial that settles somewhere other than where the others settled.
    targets = [t["target_total_h"] for t in ok]
    ref_target = statistics.median(targets) if targets else None
    for t in ok:
        # Two independent ways a trial is not ours: it settled somewhere else
        # (target spread) OR it did not START where we parked it. The second one
        # matters because a contaminated start with a correct target looks like
        # a morph in the wrong direction (measured travel_px -100 on a trial
        # that began at total_h 183 while the calibration said 36).
        off_target = bool(ref_target is not None and
                          abs(t["target_total_h"] - ref_target) > 2)
        off_start = bool(abs(t["start_total_h"] - pre["total_h"]) > 2)
        starved = bool(t.get("recorder_starved"))
        # A merged component means the geometry is unreadable, NOT that the
        # trial belongs to someone else. Keep the trial for its pose count
        # (change-in-band is immune to a static dark window) and mark only
        # the geometry-derived metrics unresolved.
        t["geometry_merged"] = bool(t.get("geometry_merged"))
        t["suspect"] = off_target or off_start or starved
        t["suspect_why"] = (" ".join((["target"] if off_target else [])
                                     + (["start"] if off_start else [])
                                     + (["recorder-starved"] if starved else []))
                            or None)
    clean = [t for t in ok if not t["suspect"]]
    # With fewer than 3 trials the median is not a reference: two contaminated
    # trials are each other's "median" and both escape the screen. Say so
    # instead of reporting a number that a stray socket command produced.
    screening_weak = len(clean) < 3
    if screening_weak and len(ok) >= 3:
        # Every trial was driven by somebody else. Reporting bounds from these
        # numbers would call the panel broken (measured travel -2 px, overshoot
        # 27x) when in fact the screen was not ours. Refuse, and say why.
        why = ", ".join(f"trial {i}: start {t['start_total_h']} "
                        f"target {t['target_total_h']} "
                        f"({t.get('suspect_why') or 'clean'})"
                        for i, t in enumerate(ok))
        print(f"FATAL: only {len(clean)} of {len(ok)} trials were clean — the "
              f"screen is not ours to measure right now ({why}). Another agent "
              f"is driving this socket; re-run when it is quiet.",
              file=sys.stderr)
        return 2
    if screening_weak:
        # Never silently fall back to contaminated trials: one suspicious
        # trial would then supply every order statistic (r2 critic #4).
        # A starved recorder is an instrument problem, so say which it was.
        starved_n = sum(1 for t in ok if t.get("recorder_starved"))
        print(f"FATAL: only {len(clean)} clean trial(s) of {len(ok)} "
              f"({starved_n} recorder-starved). wf-recorder -D is damage-driven: "
              f"on a quiet screen it stops capturing before the morph. Re-run "
              f"with more trials, or use --source grim.", file=sys.stderr)
        return 2
    # ---- global capture stats --------------------------------------------
    gaps, costs = [], []
    for ti in range(args.trials):
        fr = [f for f in all_frames if f["trial"] == ti]
        for i in range(len(fr) - 1):
            gaps.append((fr[i + 1]["t0"] - fr[i]["t0"]) * 1000.0)
        costs += [(f["t1"] - f["t0"]) * 1000.0 for f in fr]
    med = statistics.median(gaps) if gaps else 0.0
    p95 = pct(gaps, 95) or 0.0
    dropped = sum(1 for g in gaps if g > 2 * med) if gaps else 0

    # ---- duplicate frames (morph window only: before the command nothing
    # is expected to change, so those pairs are not evidence of a stalled anim)
    # Window = the OBSERVED motion, per trial: onset .. onset + anim + tail.
    # A fixed 0.45 s window on a 233 ms morph scored ~217 ms of idle frames
    # either side, so the ratio graded the waiting room, not the animation.
    dups = pairs = 0
    span = args.anim_ms + args.settle_ms
    windows = []
    for ti in range(args.trials):
        tr = next((t for t in trial_log if t["trial"] == ti), None)
        onset = None
        if tr:
            onset = tr.get("latency_ms")
            if onset is None:
                onset = tr.get("onset_ms")
        if onset is None:
            continue
        lo, hi = onset, onset + span
        windows.append([round(lo, 1), round(hi, 1)])
        fr = [f for f in all_frames if f["trial"] == ti]
        for a, c in zip(fr, fr[1:]):
            if c["geom"] is None or c["geom"].get("delta") is None:
                continue
            if c["dt"] is None or not (lo <= c["dt"] * 1000.0 <= hi):
                continue
            pairs += 1
            if c["geom"]["delta"] < b["pixel_delta_epsilon"]:
                dups += 1
    window = span / 1000.0
    dup_ratio = dups / pairs if pairs else None
    dup_bound = (b["duplicate_ratio_fast"] if med and med < 30
                 else b["duplicate_ratio_slow"])

    # ---- time-aligned equivalent-time series -----------------------------
    aligned = sorted([(round(f["dt"] * 1000, 1), f["geom"]["total_h"],
                       f["trial"]) for f in all_frames
                      if f["dt"] is not None and f["geom"] and f["dt"] >= 0
                      and f["dt"] <= 0.7], key=lambda x: x[0])
    bins = set(int(dt / b["dt_bin_ms"]) for dt, _, _ in aligned)

    # ---- aggregate the per-trial verdicts (contaminated trials excluded) --
    def agg(key, fn=max):
        vals = [t[key] for t in clean if t[key] is not None]
        return fn(vals) if vals else None

    travel = agg("travel_px", min)
    overshoot_frac = agg("overshoot_frac")
    worst_rev = agg("reversals")
    settle_max = agg("settle_ms")
    settle_dur_max = agg("settle_duration_ms")
    lat_max = agg("latency_ms")
    flight = [t["in_flight_samples"] for t in clean]
    # a reversal / overshoot claim needs at least 2 intermediate geometries in
    # some trial; otherwise the question was never asked and must not PASS
    # If any clean trial's geometry merged with the desktop, every extent-
    # derived number is suspect: refuse to score them rather than report a
    # height that belongs to the app window.
    geometry_ok = not any(t.get("geometry_merged") for t in clean)
    resolved_flight = any(v >= 2 for v in flight) and geometry_ok
    # One rule for every timing metric, not just wf's latency: the sampler
    # must resolve the individual steps of the animation, not merely span it.
    # A 233 ms morph at 60 Hz has ~14 steps, so anything coarser than three
    # display intervals (50 ms) cannot tell a smooth ramp from a slideshow.
    # grim at 33.7 ms passes; 30 fps and slower does not. These used to be
    # graded as hard FAILs, which punished the sampler for the panel's sake.
    timing_resolved = bool(med) and med <= 3.0 * (1000.0 / 60.0)
    ramps = [f["geom"]["edge_ramp"] for f in all_frames
             if f["geom"] and f["geom"].get("edge_ramp") is not None]
    ramp_med = statistics.median(ramps) if ramps else None
    soft = sum(1 for r in ramps if r > b["edge_ramp_px"])
    never_settled = sum(1 for t in clean if t["settle_ms"] is None)

    checks = []

    def add(name, value, passed, bound, why, resolved=True):
        checks.append({"metric": name, "value": value, "bound": bound,
                       "pass": bool(passed), "resolved": bool(resolved),
                       "why": why})

    add("capture_effective_fps", round(1000.0 / med, 1) if med else None,
        med and med <= b["capture_median_interval_ms"],
        f">= {1000.0 / b['capture_median_interval_ms']:.0f} fps",
        "below 10 fps a 160 ms morph gets <2 samples")
    add("interval_p95_over_median", round(p95 / med, 2) if med else None,
        med and p95 / med <= b["interval_p95_over_median"],
        f"<= {b['interval_p95_over_median']}x median",
        "uneven spacing smears the time-aligned series")
    add("dropped_frame_ratio", round(dropped / len(gaps), 3) if gaps else None,
        gaps and dropped / len(gaps) <= b["dropped_frame_ratio"],
        f"<= {b['dropped_frame_ratio']}", "sampler stalls bias every metric")
    add("travel_px", round(travel, 1) if travel is not None else None,
        travel is not None and travel >= b["travel_min_px"],
        f">= {b['travel_min_px']} px",
        "no travel means the morph never happened / wrong signal. UNRESOLVED "
        "when the panel merged with a dark window behind it",
        resolved=geometry_ok)
    absolute_latency = args.source != "wf"
    add("latency_ms", round(lat_max, 1) if lat_max is not None else None,
        lat_max is not None and lat_max <= b["latency_bound_ms"],
        f"<= {b['latency_bound_ms']:.0f} ms (worst trial)",
        "UNRESOLVED for wf-recorder: its PTS has no trustworthy wall-clock "
        "anchor. Relative pose timing remains measured.",
        resolved=absolute_latency and lat_max is not None)
    add("settle_ms", round(settle_max, 1) if settle_max is not None else None,
        settle_max is not None and never_settled <= len(clean) // 4
        and settle_max <= b["settle_bound_ms"],
        f"<= {b['settle_bound_ms']:.0f} ms from command (anim {args.anim_ms:.0f} + "
        f"1 interval + slack), never-settled <= {len(clean) // 4} trials",
        "UNRESOLVED for wf-recorder: command-relative settle would inherit "
        "the same invalid wall-clock offset. Use settle_duration_ms instead.",
        resolved=absolute_latency and settle_max is not None)
    add("settle_duration_ms",
        round(settle_dur_max, 1) if settle_dur_max is not None else None,
        settle_dur_max is not None and settle_dur_max <= b["settle_duration_ms"],
        f"<= {b['settle_duration_ms']:.0f} ms of MOTION (anim {args.anim_ms:.0f} + "
        f"1 capture interval + 30 ms)",
        "the animation's own cost, independent of dispatch. UNRESOLVED whenever "
        "no intermediate geometry was captured: a 0.0 ms reading then means the "
        "sampler missed the motion, not that it was instantaneous. UNRESOLVED "
        "too when the capture interval exceeds a third of the morph: at that "
        "rate the reading is the sampler's, not the panel's",
        resolved=resolved_flight and timing_resolved)
    add("width_reversals", worst_rev,
        worst_rev is not None and worst_rev <= b["reversals_max"],
        f"<= {b['reversals_max']} (worst trial)",
        "the easing is EaseOutCubic (src/main.rs:4835), which is monotone; a "
        "reversal is stutter or a re-triggered morph",
        resolved=resolved_flight)
    add("overshoot_frac",
        round(overshoot_frac, 3) if overshoot_frac is not None else None,
        overshoot_frac is not None
        and b["overshoot_min"] <= overshoot_frac <= b["overshoot_max"],
        f"{b['overshoot_min']:.2f}..{b['overshoot_max']:.2f} of travel",
        "EaseOutCubic does not overshoot by design, so anything past ~2 px "
        "(0.05 of 48 px travel) is quantisation noise or a bounce",
        resolved=resolved_flight)
    add("in_flight_samples", max(flight) if flight else None,
        max(flight) >= 2 if flight else False,
        ">= 2 intermediate geometries in at least one trial",
        "below this the curve shape is not observable at all. UNRESOLVED too "
        "when the panel merged with a dark window behind it: the extent then "
        "belongs to the desktop, not the island",
        resolved=resolved_flight)
    add("duplicate_ratio", round(dup_ratio, 3) if dup_ratio is not None else None,
        dup_ratio is not None and dup_ratio <= dup_bound,
        f"<= {dup_bound} of {pairs} pairs inside the observed motion windows "
        f"{windows} (onset .. onset + {span:.0f} ms)",
        "identical frames while a morph is in flight = animation not advancing. "
        "UNRESOLVED when no in-flight geometry was captured: at <60 fps the "
        "still frames before and after the morph dominate the ratio, so it "
        "grades the sampler, not the panel. UNRESOLVED too when no trial had "
        "an observed onset: the window would then be guesswork",
        resolved=resolved_flight and pairs >= 20 and bool(windows))
    add("edge_ramp_px", round(ramp_med, 2) if ramp_med is not None else None,
        ramp_med is not None and ramp_med <= b["edge_ramp_px"],
        f"<= {b['edge_ramp_px']} px (reference ~1.5 px)",
        "wide ramp = soft/blurred capsule edges. UNRESOLVED with 0 measurable "
        "edges: that means the capsule overflowed the crop, not that it is sharp",
        resolved=bool(ramps))
    add("soft_edge_frames", soft, soft <= max(1, len(ramps) // 4),
        f"<= 25 % of {len(ramps)} frames", "edge softness must not be systematic",
        resolved=bool(ramps))
    # ---- the head-line smoothness metric ---------------------------------
    poses_by_trial = [t["poses"]["n_poses"] for t in clean
                      if t.get("poses") and t["poses"]["resolved"]
                      and t["poses"]["n_poses"] is not None]
    poses_max = max(poses_by_trial) if poses_by_trial else None
    # A pose count from the whole-burst fallback includes pre-motion idle
    # frames and cannot fail a build that pauses then jumps, so it is not a
    # smoothness result at all.
    poses_anchored = any(t.get("poses", {}).get("window_anchored")
                         for t in clean if t.get("poses"))
    holds = [t["poses"]["pose_hold_median_ms"] for t in clean
             if t.get("poses") and t["poses"]["resolved"]
             and t["poses"]["pose_hold_median_ms"] is not None]
    hold_med = statistics.median(holds) if holds else None
    add("morph_poses", poses_max,
        poses_max is not None and poses_max >= b["min_morph_poses"],
        f">= {b['min_morph_poses']} distinct poses in the morph window "
        f"({args.anim_ms:.0f} ms @ 60 Hz = {args.anim_ms / (1000.0 / 60.0):.1f} slots)",
        "THE smoothness number: distinct images presented while animating. "
        "2-3 poses = the animation steps, one per display frame = smooth. "
        "UNRESOLVED when the window fell back to the whole burst: that count "
        "includes idle frames and cannot fail a paused-then-jump build",
        resolved=poses_anchored)
    add("pose_hold_median_ms", round(hold_med, 1) if hold_med is not None else None,
        hold_med is not None and hold_med <= b["min_pose_hold_ms"] * 3,
        f"<= {b['min_pose_hold_ms'] * 3:.0f} ms (3 display intervals)",
        f"how long one pose is held; a hold longer than ~3 display intervals "
        f"on a {args.anim_ms:.0f} ms morph is a visible step",
        resolved=hold_med is not None and timing_resolved and poses_anchored)
    add("dt_coverage", len(bins),
        len(aligned) >= b["min_window_samples"] * 5
        and len(bins) >= b["min_dt_bins"],
        f">= {b['min_window_samples'] * 5} aligned samples in "
        f"{b['min_dt_bins']} x {b['dt_bin_ms']:.0f} ms bins",
        "without spread the morph curve is a single point")

    res = {
        "instrument": "scripts/motion/motion.py",
        "state": args.state, "pre_state": args.pre,
        "anim_ms_config": args.anim_ms,
        "settle_ms_config": args.settle_ms,
        "screen": {"device": "2256x1504", "scale": SCALE, "refresh_hz": 60},
        "capture_method": ("wf-recorder -D (no-damage) ffv1, PTS from the "
                           "compositor" if args.source == "wf" else
                           "repeated grim -t ppm of a cropped region"),
        "source": args.source,
        "crop_grim": region,
        "smoothness": {
            "morph_poses_max": poses_max,
            "morph_poses_per_trial": poses_by_trial,
            "pose_hold_median_ms": round(hold_med, 1) if hold_med is not None else None,
            "poses_per_trial_detail": [t.get("poses") for t in clean],
            "expected_poses_60hz": round(args.anim_ms / (1000.0 / 60.0), 1),
        },
        "capture": {
            "frames": len(all_frames),
            "median_interval_ms": round(med, 1),
            "p95_interval_ms": round(p95, 1),
            "effective_fps": round(1000.0 / med, 1) if med else None,
            "grim_median_ms": (round(statistics.median(costs), 1)
                               if costs else None),
            # grim: the frame existed somewhere inside the capture call, so the
            # uncertainty is half the call. wf: PTS is exact, but the wall<->PTS
            # offset is only pinned to ~1 interval (see WF note above).
            "timestamp_uncertainty_ms": (
                round(statistics.median(costs) / 2, 1) if costs else
                (round(statistics.median(gaps) / 2, 1) if gaps else None)),
            "dropped_frames": dropped,
            "dropped_ratio": round(dropped / len(gaps), 3) if gaps else None,
            "no_island_frames": sum(1 for f in all_frames if not f["geom"]),
        },
        "pre_geometry": pre,
        "trials": trial_log,
        "summary": {
            "trials_run": len(ok),
            "screening_weak": screening_weak,
            "screening_clean": len([t for t in ok if not t["suspect"]]),
            "trials_suspect": sum(1 for t in ok if t["suspect"]),
            "trials_used": len(clean),
            "ref_target_total_h": ref_target,
            "travel_px": round(travel, 1) if travel is not None else None,
            "latency_ms_worst": round(lat_max, 1) if lat_max is not None else None,
            "settle_ms_worst": round(settle_max, 1) if settle_max is not None else None,
            "settle_duration_ms_worst": round(settle_dur_max, 1)
            if settle_dur_max is not None else None,
            "reversals_worst": worst_rev,
            "overshoot_frac": round(overshoot_frac, 3)
            if overshoot_frac is not None else None,
            "duplicate_ratio": round(dup_ratio, 3) if dup_ratio is not None else None,
            "edge_ramp_px_median": round(ramp_med, 2) if ramp_med is not None else None,
            "settled_trials": len(clean) - never_settled,
            "curve_resolved": resolved_flight,
            "in_flight_samples_max": max(flight) if flight else None,
            "dt_bins_10ms": len(bins),
            "aligned_samples": len(aligned),
        },
        "aligned_series": [{"dt_ms": d, "total_h": h, "trial": t}
                           for d, h, t in aligned],
        "checks": checks,
        "bounds": b,
        "bounds_note": {
            "settle_ms": f"threshold {b['settle_bound_ms']:.0f} ms = morph "
                         f"{args.anim_ms:.0f} + settle tail {args.settle_ms:.0f} "
                         f"+ one capture interval + 30 ms slack. Command to rest, "
                         f"so it pays BOTH phases (src/morph.rs)",
            "settle_duration_ms": f"threshold {b['settle_duration_ms']:.0f} ms = "
                                  f"morph {args.anim_ms:.0f} + one interval + 30 ms "
                                  f"slack. The MOTION only — it must not be charged "
                                  f"for the settle tail, which is a separate phase",
            "latency_ms": f"same {b['latency_bound_ms']:.0f} ms budget for "
                          f"command->first-pixel; this is the number a user reads "
                          f"as laggy",
            "timing_resolution": f"capture interval {med:.1f} ms: timing metrics "
                                 f"are UNRESOLVED above 50 ms (three 60 Hz "
                                 f"display intervals) — coarser than that cannot "
                                 f"separate a ramp from a slideshow",
            "timing_source": "MORPH_DURATION_MS and SETTLE_MS read from "
                             "src/morph.rs at run time, never hardcoded",
            "reversals_max": "the morph easing is EaseOutCubic in BOTH "
                             "directions (src/main.rs:4835-4850, iter33 removed "
                             "the old EaseOutBack), so the intended curve is "
                             "monotone. One reversal is allowed for the 1-2 px "
                             "quantisation of a 48 px travel; two or more is "
                             "stutter or a re-triggered morph. NOTE: the task "
                             "brief assumed an OutBack 1.15 overshoot was "
                             "intended -- that is stale for this codebase.",
            "overshoot_max": "same reason: EaseOutCubic has no overshoot to "
                             "intend. Measured 0.021 (1 px on 48 px travel) is "
                             "quantisation, not a bounce; 0.05 is ~2 px.",
            "edge_ramp_px": "reference frames measure 1.5 px; grim is lossless "
                            "so a wider ramp is real softness",
        },
        "socket_backpressure_failures": sock.failures,
        "loadavg": os.getloadavg(),
        "outdir": outdir,
    }
    # an UNRESOLVED metric is a limit of this instrument, not a defect of the
    # panel, so it must not condemn the run -- but it stays visible in the JSON
    res["pass"] = all(c["pass"] for c in checks if c["resolved"])
    # (a metric is either resolved-and-passing, resolved-and-failing, or
    # unresolved; it can never be both failing and unresolved, because the two
    # lists are what a fixer loop reads and a name in both is ambiguous)
    res["failures"] = [c["metric"] for c in checks
                       if c["resolved"] and not c["pass"]]
    res["unresolved"] = [c["metric"] for c in checks if not c["resolved"]]

    # keep only the interesting frames as PNG
    for f in all_frames:
        keep = f["geom"] and f["dt"] is not None and -0.05 <= f["dt"] <= 0.7
        if keep:
            f["png"] = os.path.join(
                outdir, os.path.basename(f["path"]).replace(".ppm", ".png"))
            Image.open(f["path"]).save(f["png"], compress_level=1)
        try:
            os.remove(f["path"])
        except OSError:
            pass
    shutil.rmtree(os.path.join(SHM, f"motion-frames-{os.getpid()}"),
                  ignore_errors=True)
    for i in range(3):
        try:
            os.remove(os.path.join(outdir, f"cal_{i}.ppm"))
        except OSError:
            pass

    js = json.dumps(res, indent=2)
    print(js)
    if args.json_out:
        d = os.path.dirname(os.path.abspath(args.json_out))
        os.makedirs(d, exist_ok=True)
        with open(args.json_out, "w") as fh:
            fh.write(js + "\n")

    print("\n" + "=" * 92, file=sys.stderr)
    for c in checks:
        mark = "PASS" if c["pass"] else "FAIL"
        if not c["resolved"]:
            mark = "UNRESOLVED"
        print(f"{mark:10s}  {c['metric']:26s} "
              f"{str(c['value']):>9s}   {c['bound']}", file=sys.stderr)
    print("=" * 92, file=sys.stderr)
    if screening_weak:
        print(f"NOTE  contamination screen is WEAK: only "
              f"{len([t for t in ok if not t['suspect']])} unsuspicious "
              f"trial(s). Another agent drives this socket; treat the "
              f"{len(clean)} used trial(s) as unverified.", file=sys.stderr)
    print(f"{'PASS' if res['pass'] else 'FAIL'}  overall  ({len(res['failures'])} "
          f"failing: {', '.join(res['failures']) or 'none'}; "
          f"{len(res['unresolved'])} unresolved: "
          f"{', '.join(res['unresolved']) or 'none'})", file=sys.stderr)
    return 0 if res["pass"] else 1


def main():
    previous_tag = measure_on_empty_tag()
    try:
        return run()
    finally:
        if previous_tag is not None:
            subprocess.run(["mmsg", "dispatch", f"view,{previous_tag}"],
                           capture_output=True)
            time.sleep(0.3)


if __name__ == "__main__":
    sys.exit(main())
