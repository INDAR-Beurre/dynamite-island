#!/usr/bin/env python3
"""Does the leak gate tell a LEAK from a RELOAD?

Three successive threshold tunings all failed a wallpaper/lock reload, because
they judged an excursion by its DURATION and a reload can last arbitrarily
long. The shape that actually separates them: a leak never returns to its
baseline, a reload always does. So the verdict compares the tail against the
early post-warmup baseline, and per-iteration excursions are reported as
observations rather than failures.

This pins that behaviour against synthetic series of known shape.
"""
import statistics
import sys

import os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from leak_gate import verdict, M, BOUND   # ONE implementation, not a copy


def S(*parts):
    out = []
    for n, v in parts:
        out += [v * M] * n
    return out


def ramp(n, start, step):
    return [(start + i * step) * M for i in range(n)]


CASES = []
# --- must PASS: flat, and reloads of every duration ---------------------
CASES.append(("flat 85MB", S((1, 58), (199, 85)), 0))
CASES.append(("one-time step at i49 (real curve)",
              S((1, 57), (48, 59), (151, 64)), 0))
for n, at in ((2, 50), (5, 146), (25, 126), (60, 60), (100, 60)):
    CASES.append((f"reload {n} samples at i{at}",
                  S((1, 58), (at - 2, 85), (n, 96), (201 - at - n, 85)), 0))
# --- must FAIL: every leak shape ----------------------------------------
CASES.append(("linear leak +0.25MB/iter",
              S((1, 58), (19, 85)) + ramp(180, 85, 0.25), 1))
CASES.append(("slow leak +0.08MB/iter",
              S((1, 58), (19, 85)) + ramp(180, 85, 0.08), 1))
CASES.append(("persistent step +25MB at i110",
              S((1, 58), (109, 85), (90, 110)), 1))
CASES.append(("late step +25MB at i170",
              S((1, 58), (169, 85), (30, 110)), 1))

# --- the loop stopped mid-excursion: the settle samples decide ----------
# Real failure: the suite ended with a lock surface still resident and
# called +11.6 MB of "growth". Driving home to `collapsed` first is what
# makes the verdict about retention rather than about where the loop ended.
SETTLE_CASES = [
    ("ends mid-reload, rests at 67", S((1, 62), (56, 68), (121, 79), (22, 79)),
     [67 * M] * 6, 0),
    ("ends mid-reload, LEAKED to 95", S((1, 62), (56, 68), (121, 79), (22, 79)),
     [95 * M] * 6, 1),
    ("flat loop, settle confirms", S((1, 62), (199, 68)), [68 * M] * 6, 0),
]


# --- r2 #13: expanded baseline vs collapsed tail hid a real leak ---------
# The loop cycles lock/themes/CC, so rss[40:60] sits ~32 MB ABOVE the
# collapsed resting level. Comparing that to a collapsed tail printed
# "growth -32.2 MB" on every run, and a genuine +5 MB leak vanished inside
# the offset. With a collapsed baseline the same series must FAIL.
STATE_CASES = [
    ("expanded base, collapsed tail, FLAT",
     S((1, 62), (199, 95)), [63 * M] * 6, [63 * M] * 6, 0),
    # +5 MB is INSIDE the 8 MB tolerance, so the gate passes it by design.
    # Pinned so nobody "fixes" the bound without reading this.
    ("expanded base, collapsed tail, +5MB within tolerance",
     S((1, 62), (199, 95)), [68 * M] * 6, [63 * M] * 6, 0),
    ("expanded base, collapsed tail, +9MB LEAK",
     S((1, 62), (199, 95)), [72 * M] * 6, [63 * M] * 6, 1),
    ("expanded base, collapsed tail, +12MB LEAK",
     S((1, 62), (199, 95)), [75 * M] * 6, [63 * M] * 6, 1),
]


def main():
    bad = 0
    for name, rss, want in CASES:
        if len(rss) != 200:
            print(f"BAD FIXTURE {name}: {len(rss)} samples, expected 200")
            bad += 1
            continue
        base_rest = statistics.median(rss[40:60])
        fails, g, peak, exc = verdict(rss, base_collapsed=[base_rest] * 6)
        ok = fails == want
        bad += 0 if ok else 1
        tag = "ok " if ok else "BAD"
        print(f"{tag} {name:34s} growth={g:+6.1f} peak={peak:+6.1f} "
              f"excursions={len(exc)} -> {'FAIL' if fails else 'PASS'} "
              f"(want {'FAIL' if want else 'PASS'})")
    total = len(CASES)
    for name, rss, settled, want in SETTLE_CASES:
        fails, g, peak, exc = verdict(rss, settled,
                                      base_collapsed=[statistics.median(rss[40:60])] * 6)
        ok = fails == want
        bad += 0 if ok else 1
        total += 1
        tag = "ok " if ok else "BAD"
        print(f"{tag} {name:34s} growth={g:+6.1f} peak={peak:+6.1f} "
              f"-> {'FAIL' if fails else 'PASS'} "
              f"(want {'FAIL' if want else 'PASS'})")
    for name, rss, settled, base_c, want in STATE_CASES:
        fails, g, peak, exc = verdict(rss, settled, base_collapsed=base_c)
        ok = fails == want
        bad += 0 if ok else 1
        total += 1
        tag = "ok " if ok else "BAD"
        print(f"{tag} {name:34s} growth={g:+6.1f} peak={peak:+6.1f} "
              f"-> {'FAIL' if fails else 'PASS'} "
              f"(want {'FAIL' if want else 'PASS'})")
    # And prove the OLD arithmetic really did hide it: same leak, expanded
    # baseline. If this ever stops reading negative, the fixture is wrong.
    _, g_old, _, _ = verdict(STATE_CASES[2][1], STATE_CASES[2][2])
    ok = g_old < 0
    bad += 0 if ok else 1
    total += 1
    print(f"{'ok ' if ok else 'BAD'} old expanded-baseline arithmetic still hides it "
          f"(growth={g_old:+.1f}, want < 0)")
    print(f"\n{total - bad}/{total} assertions pass")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())