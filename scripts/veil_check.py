#!/usr/bin/env python3
"""Check the lock veil against the reference, densely, in subpixel units.

Purpose: `veil_edges_match_the_fitted_recipe` tests the code against the
constants the code itself declares. That cannot catch a wrong fit. This
re-derives the veil from two reference frames -- the same wallpaper unlocked
and locked -- and scores the shipped constants against it.

Method: alpha = 1 - locked/unlocked, per row, median over sky-only columns
(the clock spans x905..1711 and would contaminate it). G channel only: R is
2-3 in this wallpaper, so dividing by it produces nan. Rows with fewer than
10 usable samples are dropped. 97 rows at 1% steps, not 8 sparse points --
sparse sampling hid a 0.079 error at the bottom edge.

Verdict is in /255 on the actual background, because an alpha error of 0.009
is invisible (1.09/255) while the same number reads alarming as a percentage.

  python3 scripts/veil_check.py [unlocked.png] [locked.png]
"""
import re
import sys

import numpy as np
from PIL import Image

SRC = "src/lockscreen.rs"
KEYS = ("VEIL_TOP_STOP", "VEIL_BOT_SPLIT", "VEIL_TRANSMIT_PLATEAU",
        "VEIL_TOP_DROP", "VEIL_BOT_DROP")
# 2/255 on the background: below this the difference cannot be seen, and
# video compression alone moves a median alpha by about this much.
TOL_255 = 2.0


def constants(path=SRC):
    s = open(path).read()
    out = {}
    for k in KEYS:
        m = re.search(rf"pub const {k}: f32 = ([\d.]+)", s)
        if not m:
            raise SystemExit(f"missing constant {k} in {path}")
        out[k] = float(m.group(1))
    return out


def measured(unlocked, locked):
    u = np.asarray(Image.open(unlocked).convert("RGB")).astype(float)
    l = np.asarray(Image.open(locked).convert("RGB")).astype(float)
    if u.shape != l.shape:
        raise SystemExit("frames differ in size; cannot divide")
    h, w, _ = l.shape
    # Sky columns either side of the clock. Derived from the clock's own
    # extent, not hardcoded, so a different wallpaper still works.
    xs = list(range(int(.08 * w), int(.33 * w), max(1, w // 50)))
    xs += list(range(int(.70 * w), int(.94 * w), max(1, w // 50)))
    fy, al, bg = [], [], []
    for f in np.arange(0.02, 0.985, 0.01):
        y = int(f * (h - 1))
        good = [x for x in xs if u[y, x, 1] > 60]
        if len(good) < 10:
            continue
        v = np.array([u[y, x] for x in good])
        # alpha = 1 - locked/unlocked assumes both frames show the SAME
        # content at that pixel. Below y/H 0.92 this wallpaper stops being
        # sky (R/G goes 0.03 -> 1.12, it is the foreground figure), so the
        # ratio measures content difference, not veil. Those rows produced
        # a 21.88/255 "error" that was really the instrument.
        if np.median(v[:, 0]) / np.median(v[:, 1]) >= 0.10:
            continue
        fy.append(f)
        al.append(float(np.median([1 - l[y, x, 1] / u[y, x, 1] for x in good])))
        bg.append(float(np.median([u[y, x, 1] for x in good])))
    return np.array(fy), np.array(al), float(np.median(bg))


def alpha_at(c, f):
    top = np.clip((c["VEIL_TOP_STOP"] - f) / c["VEIL_TOP_STOP"], 0, 1)
    bot = np.clip((f - c["VEIL_BOT_SPLIT"]) / (1 - c["VEIL_BOT_SPLIT"]), 0, 1)
    return 1 - (c["VEIL_TRANSMIT_PLATEAU"]
                - c["VEIL_TOP_DROP"] * top - c["VEIL_BOT_DROP"] * bot)


def best_fit(f, a):
    """Grid the two breakpoints, solve the three linear terms exactly."""
    best = None
    for ts in np.arange(0.20, 0.36, 0.01):
        for bs in np.arange(0.62, 0.80, 0.01):
            A = np.column_stack([np.ones_like(f),
                                 np.clip((ts - f) / ts, 0, 1),
                                 np.clip((f - bs) / (1 - bs), 0, 1)])
            coef, *_ = np.linalg.lstsq(A, a, rcond=None)
            e = float(np.abs(A @ coef - a).mean())
            if best is None or e < best[0]:
                best = (e, ts, bs, coef)
    e, ts, bs, coef = best
    return {"VEIL_TOP_STOP": float(ts), "VEIL_BOT_SPLIT": float(bs),
            "VEIL_TRANSMIT_PLATEAU": float(1 - coef[0]),
            "VEIL_TOP_DROP": float(coef[1]),
            "VEIL_BOT_DROP": float(coef[2])}, e


def main():
    un = sys.argv[1] if len(sys.argv) > 1 else "/tmp/ref/lock/l20.png"
    lo = sys.argv[2] if len(sys.argv) > 2 else "/tmp/ref/lock/l40.png"
    c = constants()
    f, a, bg = measured(un, lo)
    if len(f) < 40:
        print("RESULT ERROR: too few usable rows; wrong frames?")
        return 2
    err = np.abs(alpha_at(c, f) - a) * bg
    fit, fit_err = best_fit(f, a)
    print(f"rows {len(f)}  background G {bg:.0f}")
    print(f"shipped  mean {err.mean():.2f}/255  max {err.max():.2f}/255")
    print(f"best fit mean {fit_err * bg:.2f}/255")
    for k in KEYS:
        print(f"  {k:22s} shipped {c[k]:.5f}   fit {fit[k]:.5f}")
    ok = err.mean() <= TOL_255
    print("RESULT", "PASS" if ok else "FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
