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

M = 1024
BOUND = 8.0


def verdict(rss):
    """(fails, growth_mb, peak_mb, excursions) — mirrors group_load."""
    tail_all = rss[20:]
    # The baseline is the SETTLED region [40:60], not the whole tail (a slow
    # leak raises its own median and hides itself: +0.08 MB/iter read as
    # +6.4 MB there) and not [20:40] either (a real run allocated once at i49,
    # so [20:40] sat below the step and called +5.8 MB of nothing "growth").
    # [40:60] is past every allocation observed and still early enough that a
    # slow leak separates from it.
    base = statistics.median(rss[40:60])
    growth = (statistics.median(rss[-20:]) - base) / M
    peak = (max(tail_all) - base) / M
    excursions = []
    run = start = 0
    pk = 0.0
    for i in range(20, len(rss)):
        ex = (rss[i] - base) / M
        if ex > BOUND:
            if run == 0:
                start = i
            run += 1
            pk = max(pk, ex)
        elif run:
            excursions.append((start, run, pk))
            run = pk = 0
    if run:
        excursions.append((start, run, pk))
    return (1 if growth > BOUND else 0), growth, peak, excursions


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


def main():
    bad = 0
    for name, rss, want in CASES:
        if len(rss) != 200:
            print(f"BAD FIXTURE {name}: {len(rss)} samples, expected 200")
            bad += 1
            continue
        fails, g, peak, exc = verdict(rss)
        ok = fails == want
        bad += 0 if ok else 1
        tag = "ok " if ok else "BAD"
        print(f"{tag} {name:34s} growth={g:+6.1f} peak={peak:+6.1f} "
              f"excursions={len(exc)} -> {'FAIL' if fails else 'PASS'} "
              f"(want {'FAIL' if want else 'PASS'})")
    print(f"\n{len(CASES) - bad}/{len(CASES)} assertions pass")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())