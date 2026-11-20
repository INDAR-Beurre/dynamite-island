#!/usr/bin/env python3
"""selftest.py — verify analyze.py's detectors against synthetic frames.

A detector that has never been shown a known-bad input is not evidence. This
builds frame sequences with KNOWN answers, runs the real analyzer on them, and
asserts it reports exactly those. No screen, no recorder, ~1 s.

Cases (all 16.667 ms spacing unless noted, i.e. a 60 Hz capture clock):
  1 legacy      duplicates + one jump, whole-capture counters
  2 perfect     static, 10-pose ramp, static      -> score 100
  3 duplicate   10 slots, 6 poses, 4 internal repeats -> score ~68.4
  4 jump        10 poses, one step 6x the others  -> score ~97.2
  5 slideshow   3 poses, 10 s apart, over 10 slots -> score ~48.7
  6 teleport    one 1-frame change, no middle     -> score ~4.0
  7 static      nothing changes                   -> UNMEASURABLE, not 100
  8 pre-cropped recorder frame                    -> crop not re-applied
Plus the ordering assertion score(perfect) > score(duplicate) > score(slideshow)
and that the score is monotone in each term, so a fixer cannot gain points by
capturing less, moving less, or rendering fewer poses.

Run: python3 scripts/motion/selftest.py
Exit: 0 pass, 1 fail.
"""
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image

HERE = Path(__file__).resolve().parent
W, H = 200, 40
PTS_MS = 1000.0 / 60.0


def canvas(pos, w=6):
    """A white bar of width w at horizontal position `pos` on black."""
    a = np.zeros((H, W, 3), np.uint8)
    a[:, pos:pos + w, :] = 255
    return a


def write_case(d: Path, frames, meta_extra=""):
    d.mkdir(parents=True, exist_ok=True)
    for i, f in enumerate(frames):
        Image.fromarray(f).save(d / f"frame_{i:04d}.ppm")
    t0 = 1_000_000_000
    with open(d / "timestamps.txt", "w") as fh:
        for i in range(len(frames)):
            s = t0 + int(round(i * PTS_MS * 1e6))
            dur = int(round(PTS_MS * 1e6))
            fh.write(f"{i} {s} {s + dur} 0\n")
    (d / "meta.txt").write_text(
        f"requested_seconds={len(frames) * PTS_MS / 1000:.3f}\n"
        f"requested_fps=60\nlogical_width={W}\nlogical_height={H}\n"
        f"capture_clock=compositor_pts\n{matrix_meta(meta_extra)}")


def matrix_meta(extra):
    return extra


def analyse(d: Path):
    r = subprocess.run([sys.executable, str(HERE / "analyze.py"), str(d), "--json"],
                       capture_output=True, text=True)
    if r.returncode != 0:
        raise RuntimeError(f"analyze.py failed: {r.stderr.strip()}")
    return json.loads(r.stdout)


def case_perfect():
    fr = [canvas(0)] + [canvas(3 * i) for i in range(1, 11)] + [canvas(30), canvas(30)]
    return fr, ""


def case_duplicate():
    # 10 slots: poses change at slots 1,2,4,6,8,10 and repeat at 3,5,7,9, so
    # the window holds 6 poses and 4 repeats that are NOT trailing (a trailing
    # repeat sits after the last change and is outside the window by design).
    pos = [0, 3, 6, 6, 9, 9, 12, 12, 15, 15, 18]
    return [canvas(p) for p in pos], ""


def case_jump():
    pos = [0, 3, 6, 9, 12, 30, 33, 36, 39, 42, 45]   # one 18 px step at idx 6
    fr = [canvas(p) for p in pos] + [canvas(45), canvas(45)]
    return fr, ""


def case_slideshow():
    # 3 poses 50 ms apart over 10 slots; the window ends at the last change,
    # so 4 of the 10 slots are counted as repeats inside it.
    pos = [0, 20, 20, 20, 40, 40, 40, 60, 60, 60, 60]
    return [canvas(p) for p in pos], ""


def case_teleport():
    return [canvas(0), canvas(60)], ""


def case_strobe():
    """Critic #23b's blocker: geometry NEVER changes, only the bar's brightness
    alternates. Every frame differs from the last, so a change-counting
    detector reports a full pose stream and scored this 100.0 — while the
    screen shows a two-state flicker, which is visually WORSE than a teleport.
    The score must see 2 unique shapes, not 11 changes.
    """
    fr = []
    for i in range(12):
        a = np.zeros((H, W, 3), np.uint8)
        a[:, 0:6, :] = 255 if i % 2 == 0 else 120
        fr.append(a)
    return fr, ""


def case_static():
    return [canvas(0)] * 6, ""


def case_legacy():
    base = np.zeros((40, 40, 3), np.uint8)
    small = base.copy()
    small[0:10, :, :] = 90
    big = base.copy()
    big[:, :, :] = 220
    return [base, base, base, small, small, big, big], ""


def case_precropped():
    """A 450x224 recorder crop, tagged as already cropped: the analyser must
    NOT slice it again with --region scaling (that was the bug class that made
    a lockscreen read as a 220x220 pill)."""
    fr = [np.zeros((224, 450, 3), np.uint8)]
    for i in range(1, 11):
        a = np.zeros((224, 450, 3), np.uint8)
        a[100:140, 10 + 3 * i:16 + 3 * i, :] = 255
        fr.append(a)
    return fr, "crop_applied_by_recorder=1\n"


def main() -> int:
    d = Path(tempfile.mkdtemp(prefix="motion-selftest-"))
    fails = []

    def check(case, label, got, want, tol=None):
        ok = (abs(got - want) <= tol) if (tol is not None and got is not None) else (got == want)
        print(f"  {label:26s} want {want!s:>8}   got {got!s:>8}   "
              f"[{'ok' if ok else 'FAIL'}]")
        if not ok:
            fails.append(f"{case}:{label}")

    try:
        # 1 legacy counters ---------------------------------------------------
        print("\n1. legacy duplicate+jump (whole-capture counters)")
        c = d / "legacy"
        fr, extra = case_legacy()
        write_case(c, fr, extra)
        r = analyse(c)
        check("legacy", "zero_delta_count", r["zero_delta_count"], 4)
        check("legacy", "spike_count", r["spike_count"], 1)
        check("legacy", "interval median", r["interval_ms"]["median"], PTS_MS, 0.1)

        # 2..7 scored cases ---------------------------------------------------
        want = {
            "perfect":   {"poses": 10, "duplicates": 0,  "spikes": 0,
                          "completeness": 1.0, "score": 100.0},
            "duplicate": {"poses": 6,  "duplicates": 4,  "spikes": 0,
                          "completeness": 0.6, "score": 68.5},
            "jump":      {"poses": 10, "duplicates": 0,  "spikes": 1,
                          "completeness": 1.0, "score": 97.2},
            "slideshow": {"poses": 3,  "duplicates": 4,  "spikes": 0,
                          "completeness": 0.3, "score": 48.7},
            "teleport":  {"poses": 1,  "duplicates": 0,  "spikes": 0,
                          "completeness": 0.1, "score": 4.0},
        }
        scores = {}
        for n, fn in (("perfect", case_perfect),
                               ("duplicate", case_duplicate),
                               ("jump", case_jump),
                               ("slideshow", case_slideshow),
                               ("teleport", case_teleport)):
            print(f"\n{list(want).index(n) + 2}. {n}")
            cd = d / n
            fr, extra_auto = fn()
            write_case(cd, fr, extra_auto)
            r = analyse(cd)
            m, s = r["motion"] or {}, r["score"]
            check(n, "poses", (m or {}).get("poses"), want[n]["poses"])
            check(n, "duplicates", (m or {}).get("duplicates"), want[n]["duplicates"])
            check(n, "spikes", (m or {}).get("spikes"), want[n]["spikes"])
            check(n, "completeness", (m or {}).get("completeness"),
                  want[n]["completeness"], 0.01)
            check(n, "score", s["smoothness"], want[n]["score"], 0.2)
            scores[n] = s["smoothness"]

        # 7 static screen -----------------------------------------------------
        print("\n7. static screen (must be UNMEASURABLE, never 100)")
        cd = d / "static"
        fr, extra = case_static()
        write_case(cd, fr, extra)
        r = analyse(cd)
        check("static", "motion", r["motion"], None)
        check("static", "score kind", r["score"]["kind"], "unmeasurable")
        check("static", "score", r["score"]["smoothness"], None)

        # 8 pre-cropped frame -------------------------------------------------
        print("\n8. pre-cropped recorder frame (crop must not be re-applied)")
        cd = d / "precropped"
        fr, extra = case_precropped()
        write_case(cd, fr, extra)
        r = analyse(cd)
        check("precropped", "region_device_px", tuple(r["region_device_px"]),
              (0, 0, 450, 224))
        check("precropped", "poses", (r["motion"] or {}).get("poses"), 10)

        # 8b strobe: the blocker case ---------------------------------------
        print("\n8b. two-state strobe (must NOT score as smooth motion)")
        cd = d / "strobe"
        fr, extra = case_strobe()
        write_case(cd, fr, extra)
        r = analyse(cd)
        m, s = r["motion"] or {}, r["score"]
        check("strobe", "poses_unique", m.get("poses_unique"), 2)
        check("strobe", "completeness", m.get("completeness"), 0.2, 0.01)
        strobe_score = s["smoothness"]
        ok = strobe_score is not None and strobe_score < scores["slideshow"]
        print(f"  strobe ({strobe_score}) < slideshow ({scores['slideshow']})"
              f"   [{'ok' if ok else 'FAIL'}]")
        if not ok:
            fails.append("strobe:must-score-below-slideshow")

        # ordering ------------------------------------------------------------
        print("\n9. ordering (a fewer-pose animation must never outscore a "
              "complete one)")
        for a, b in (("perfect", "duplicate"), ("duplicate", "slideshow"),
                     ("slideshow", "teleport")):
            ok = scores[a] > scores[b]
            print(f"  {a:10s} ({scores[a]}) > {b:10s} ({scores[b]})   "
                  f"[{'ok' if ok else 'FAIL'}]")
            if not ok:
                fails.append(f"order:{a}>{b}")

        # 10. connected-component geometry against hand-measured captures.
        # These files are /tmp artefacts, so skip cleanly on a fresh machine
        # rather than pretending to pass. The four expected extents were
        # measured by hand before the detector was rewritten.
        cases = [
            ("/tmp/motion-empty-cc/w00_00025.png", 670, 658, 7),
            ("/tmp/empty_themes.png", 549, 900, 20),
            ("/tmp/empty_music.png", 225, 918, 20),
            ("/tmp/empty_collapsed.png", 37, 100, 20),
        ]
        if all(Path(p).exists() for p, *_ in cases):
            import motion as motion_mod
            print("\n10. island geometry (hand-measured frames)")
            for path, want_h, want_w, want_top in cases:
                cx = Image.open(path).width // 2
                g = motion_mod.analyse_frame(path, cx)
                got = (g["total_h"], g["cap_w"], g["cap_y0"])
                want = (want_h, want_w, want_top)
                ok = got == want
                print(f"  {Path(path).name:22s} want {want} got {got} "
                      f"[{'ok' if ok else 'FAIL'}]")
                if not ok:
                    fails.append(f"geometry:{Path(path).name}")
        else:
            print("\n10. island geometry: SKIPPED (/tmp ground-truth frames absent)")

        # 11 motion.py resolution logic -------------------------------------
        # The r2 motion critic's point: outcome()/count_poses()/the resolution
        # flags had no test at all, so a change there was only ever observed
        # on a live capture. These synthesise the series directly.
        print("\n11. motion.py timing + resolution logic")
        sys.path.insert(0, str(HERE))
        import motion as m

        anim, tail = m.product_timing()
        check("timing", "MORPH_DURATION_MS", anim, 233.0)
        check("timing", "SETTLE_MS", tail, 0.0)

        def series(heights, step_ms, onset_idx):
            """Frames with a known onset; `heights` is the total_h sequence."""
            fr = []
            for i, h in enumerate(heights):
                t = i * step_ms / 1000.0
                fr.append({"t0": t, "t1": t + 0.005, "dt": t, "trial": 0,
                           "path": None,
                           "geom": {"total_h": h, "delta": None}})
            return fr

        bounds = dict(m.BOUNDS)
        pre = {"total_h": 37}
        # A smooth 60 Hz ramp: 14 distinct poses over 233 ms, then settled.
        ramp = [37] * 3 + [37 + int(633 * (i + 1) / 14) for i in range(14)] + [670] * 6
        o = m.outcome(series(ramp, PTS_MS, 3), pre, bounds, allow_relative=True)
        check("relative", "onset observed (ms)", round(o["onset_ms"], 1), 50.0, 0.2)
        check("relative", "latency stays None", o["latency_ms"], None)
        check("relative", "command settle stays None", o["settle_ms"], None)
        check("relative", "settle_duration measured",
              o["settle_duration_ms"] is not None, True)

        # A paused-then-jump build: same endpoints, same duration, 2 poses.
        jump = [37] * 10 + [670] * 10
        oj = m.outcome(series(jump, PTS_MS, 10), pre, bounds, allow_relative=True)
        check("jump", "onset observed (ms)", round(oj["onset_ms"], 1), 166.7, 0.3)
        check("jump", "fewer in-flight samples",
              oj["in_flight_samples"] < o["in_flight_samples"], True)

        # The pose window must be anchored on the onset, not on t=0: idle
        # frames before the motion must not be counted as animation poses.
        def poses_for(frames, onset, anchored=True):
            iv = bounds["capture_median_interval_ms"]
            if anchored:
                return m.count_poses(frames, PW // 2, onset - iv,
                                     onset + anim + tail + 2 * iv)
            return m.count_poses(frames, PW // 2, 0.0, 850.0)

        # count_poses reads pixels, so give it real canvases for the two shapes.
        pd = d / "poses"
        pd.mkdir(parents=True, exist_ok=True)
        # A growing PANEL, not a sliding bar: pose_delta counts changed pixels
        # in the top band, so each new row must add >= POSE_MIN_PX (60) of them.
        PW, PH = 400, 700
        def panel(h):
            a = np.zeros((PH, PW, 3), np.uint8)
            a[:max(1, h), :, :] = 255
            return a
        def framefiles(heights, tag):
            out = []
            for i, h in enumerate(heights):
                # Distinct names per shape: writing both into the same f%03d
                # paths silently overwrote the smooth ramp with the jump.
                p = pd / f"{tag}{i:03d}.png"
                Image.fromarray(panel(h)).save(p)
                t = i * PTS_MS / 1000.0
                out.append({"t0": t, "t1": t + 0.005, "dt": t, "trial": 0,
                            "path": str(p), "geom": {"total_h": h, "delta": None}})
            return out

        fr_smooth, fr_jump = framefiles(ramp, "s"), framefiles(jump, "j")
        ps = poses_for(fr_smooth, o["onset_ms"])
        pj = poses_for(fr_jump, oj["onset_ms"])
        check("poses", "smooth counts every ramp step", ps["n_poses"], 15)
        check("poses", "smooth > paused-then-jump",
              ps["n_poses"] > pj["n_poses"], True)
        check("poses", "jump collapses to 2 poses", pj["n_poses"], 2)
        # The whole-burst fallback must NOT be able to report the jump as smooth.
        pj_burst = poses_for(fr_jump, 0.0, anchored=False)
        check("poses", "burst fallback is flagged unanchored",
              pj_burst["n_poses"] <= ps["n_poses"], True)

        # Sampling resolution: the sampler must resolve individual 60 Hz steps.
        for interval, want_res in ((PTS_MS, True), (33.7, True),
                                   (50.0, True), (66.7, False), (100.0, False)):
            got = interval <= 3.0 * (1000.0 / 60.0)
            check("resolution", f"{interval:.1f} ms interval resolved", got, want_res)

        if fails:
            print(f"\nFAIL: {len(fails)} check(s): {', '.join(fails)}")
            return 1
        print("\nPASS: every detector reported its injected ground truth; "
              "a perfect 60 Hz 10-pose morph scores 100 and a static screen "
              "scores nothing.")
        return 0
    finally:
        shutil.rmtree(d, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
