#!/usr/bin/env python3
"""The leak verdict, in ONE place.

r2 #13: stress.py and leak_shape_selftest.py each carried their own copy of
this arithmetic, so the selftest could pass while the real gate was wrong.
Both now import this function.

r2 #13 also found the real defect: the baseline was sampled INSIDE the loop
while panels were expanded (lock/themes surfaces resident) and the tail was
sampled after driving the panel back to `collapsed`. Those are two different
resting states, so the gate printed "growth -32.2 MB" on every run and any
leak smaller than that offset was invisible. `base_collapsed` fixes it: the
baseline must be measured in the SAME state as the tail.
"""
import statistics

M = 1024
BOUND = 8.0


def verdict(rss, settled=None, base_collapsed=None):
    """(fails, growth_mb, peak_mb, excursions).

    rss            per-iteration samples, taken while states cycle.
    settled        samples taken AFTER driving the panel back to collapsed.
                   Without them the verdict measures wherever the loop
                   stopped: a real run ended with a lock surface up and
                   reported +11.6 MB on a panel that rests at 67 MB.
    base_collapsed samples taken at rest (collapsed) partway through the run,
                   past the one-time allocations. REQUIRED for a truthful
                   growth number; without it the comparison is expanded-vs-
                   collapsed and understates growth by the state offset.
    """
    tail_all = rss[20:]
    # [40:60] is past every one-time allocation observed (one landed at i49)
    # and still early enough that a slow +0.08 MB/iter leak separates from it.
    base_expanded = statistics.median(rss[40:60])
    base = statistics.median(base_collapsed) if base_collapsed else base_expanded
    tail = settled if settled is not None else rss[-6:]
    growth = (statistics.median(tail) - base) / M
    peak = (max(tail_all) - base_expanded) / M
    excursions = []
    run = start = 0
    pk = 0.0
    for i in range(20, len(rss)):
        ex = (rss[i] - base_expanded) / M
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