#!/usr/bin/env python3
"""Prove measure.py's pill detector refuses frames that are not the capsule.

Five instrument defects have already been found in this project's harnesses.
A detector that silently scores a dark app window as the pill poisons every
critic that quotes it, so the guard needs its own test. Run: python3 scripts/measure_selftest.py
"""
import sys
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
from measure import pill_bbox  # noqa: E402

H, W = 1504, 2256


def frame(fill, box, v):
    a = np.full((H, W, 3), fill, np.uint8)
    x0, y0, x1, y1 = box
    a[y0:y1, x0:x1] = v
    return a


def main() -> int:
    fails = []

    ref_path = Path(__file__).resolve().parent.parent / "docs" / "reference" / "pill-settled.png"
    if ref_path.exists():
        b = pill_bbox(np.array(Image.open(ref_path).convert("RGB")))
        if not b or b.get("not_collapsed"):
            fails.append(f"reference frame refused: {b}")
        else:
            print(f"reference scores: {b['w']}x{b['h']} aspect {b['aspect']}")

    b = pill_bbox(frame(200, (1080, 20, 1177, 56), 0))
    if not b or b.get("not_collapsed"):
        fails.append(f"synthetic capsule refused: {b}")
    else:
        print(f"synthetic capsule scores: {b['w']}x{b['h']}")

    merged = frame(200, (1060, 20, 1240, 300), 24)
    merged[20:56, 1080:1177] = 0
    for name, img in [
        ("dark window", frame(200, (1060, 60, 1240, 300), 24)),
        ("dark window merged with capsule", merged),
        ("fullscreen dark from y=0", frame(200, (0, 0, W, 400), 0)),
    ]:
        b = pill_bbox(img)
        if not (b and b.get("not_collapsed")):
            fails.append(f"{name} was scored as a pill: {b}")
        else:
            print(f"{name} refused: {b['reason']}")

    for f in fails:
        print("FAIL:", f)
    print("RESULT", "FAIL" if fails else "PASS")
    return 1 if fails else 0


if __name__ == "__main__":
    raise SystemExit(main())