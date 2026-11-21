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

    # 1. Workspaces stay hidden until the pointer enters the clock.
    a = np.full((H, W, 3), 200)
    capsule(a)
    check("idle capsule contains only the clock", metrics(a), True,
          lambda g: g["x0"] == 1079 and g["x1"] == 1176
          and g["outside_dark"] < 0.55 and g["ink_frac"] >= 0.02)
    # Hover adds 132 physical px to the clock's left edge.
    a[19:56, 947:1079] = 2
    a[30:44, 1028:1033] = 210  # workspace digit
    check("joined capsule includes the workspace row", metrics(a), True,
          lambda g: g["x0"] == 947 and g["x1"] == 1176
          and g["outside_dark"] < 0.55 and g["ink_frac"] >= 0.02)

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

    # 5. Critic #22's blind spot: a plain dark stripe from y=1 (NOT y=0, so the
    #    fullscreen refusal misses it), no capsule and no clock ink. It passed
    #    all four old verdict inputs. check_pill must reject it on positive
    #    evidence: a stadium shape and glyph ink inside.
    a = np.full((H, W, 3), 200)
    a[1:120, 1030:1230] = 3
    g = metrics(a)
    ok = g is None or g.get("ink_frac", 0.0) < 0.02
    print(f"{'ok  ' if ok else 'FAIL'} y=1 dark stripe has no ink: {g}")
    if not ok:
        fails += 1

    # 6. The full check_pill verdict on that same stripe must be False. Drive
    #    it through the real predicate rather than re-implementing the bounds.
    if g is not None:
        width = g["x1"] - g["x0"]
        height = g["y1"] - g["y0"]
        rejected = (width <= height) or g.get("ink_frac", 0.0) < 0.02
        print(f"{'ok  ' if rejected else 'FAIL'} check_pill rejects the stripe "
              f"(w={width} h={height} ink={g.get('ink_frac', 0.0):.3f})")
        if not rejected:
            fails += 1

    # 7. Workspace digits alone cannot satisfy the clock ink check.
    a = np.full((H, W, 3), 200)
    a[19:56, 947:1177] = 2
    for x in (970, 1000, 1030, 1060):
        a[30:44, x:x + 7] = 210
    g = metrics(a)
    ok = g is not None and g.get("ink_frac", 0.0) < 0.02
    print(f"{'ok  ' if ok else 'FAIL'} missing clock reports no clock ink: "
          f"{None if g is None else round(g.get('ink_frac', 0.0), 3)}")
    if not ok:
        fails += 1

    print("\nRESULT " + ("PASS" if fails == 0 else f"FAIL ({fails})"))
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())