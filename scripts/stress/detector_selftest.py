#!/usr/bin/env python3
"""Ground-truth tests for the stress harness's capsule detector.

The detector has now been wrong three times in ways that produced confident
false verdicts (a dark page below the bar, a fullscreen surface from y=0, and
a dark WALLPAPER that scored 0.90 "outside dark" and failed 138 assertions
while the panel was fine). Each regression gets a synthetic frame here so it
cannot come back silently.

    python3 scripts/stress/detector_selftest.py
"""
import importlib.util
import pathlib
import sys
import tempfile

import numpy as np
from PIL import Image

HERE = pathlib.Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("st", HERE / "stress.py")
st = importlib.util.module_from_spec(spec)
spec.loader.exec_module(st)


class _Fake:
    pass


def metrics(a):
    with tempfile.NamedTemporaryFile(suffix=".png", delete=False) as f:
        Image.fromarray(a.astype(np.uint8)).save(f.name)
        return st.App.pill_metrics(_Fake(), f.name)


def capsule(a, x0=1079, y0=19, w=98, h=37, fill=2):
    """Draw a stadium-ish black capsule with bright clock ink inside."""
    a[y0:y0 + h, x0:x0 + w] = fill
    a[y0 + 10:y0 + 26, x0 + 20:x0 + 70] = 245
    return a


def main():
    W, H = 2256, 1504
    fails = 0

    def check(name, got, want_ok, extra=None):
        nonlocal fails
        ok = (got is not None) == want_ok
        if ok and extra is not None and got is not None:
            ok = extra(got)
        print(f"{'ok  ' if ok else 'FAIL'} {name}: {got}")
        if not ok:
            fails += 1

    # 1. Bright wallpaper: the easy case must still work.
    a = np.full((H, W, 3), 200)
    capsule(a)
    check("bright wallpaper finds capsule", metrics(a), True,
          lambda g: abs(g["x0"] - 1079) <= 4 and g["outside_dark"] < 0.55)

    # 2. DARK wallpaper (p25=21). This is the regression that failed 138
    #    assertions: at DARK=40 the outside scored 0.90 and every liveness
    #    check failed while the panel was fine.
    a = np.full((H, W, 3), 21)
    capsule(a)
    check("dark wallpaper still separates", metrics(a), True,
          lambda g: g["outside_dark"] < 0.55 and g["inside_dark"] > 0.55)

    # 3. Fullscreen dark surface starting at y=0 must be REFUSED, not
    #    reported as a giant capsule.
    a = np.full((H, W, 3), 3)
    check("fullscreen dark refused", metrics(a), False)

    # 4. A dark app window BELOW the bar must not be merged into the box.
    a = np.full((H, W, 3), 200)
    capsule(a)
    a[300:, :] = 5
    check("dark window below not merged", metrics(a), True,
          lambda g: g["y1"] < 100)

    print("\nRESULT " + ("PASS" if fails == 0 else f"FAIL ({fails})"))
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())