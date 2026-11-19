#!/usr/bin/env python3
"""The creator's own taste test, made into a number.

He states the rule twice in the transcripts (docs/transcript-taste.md):
  "contrast is basically deciding where people look first"
  "whatever that you can still see in the blurry version is what your
   desktop is shouting about."

So: blur a capture until fine detail is gone, then ask what survived.
A quiet design leaves only the clock and the active workspace. Anything
else that survives -- a glow, a bloom, a saturated tile -- is spending
contrast budget it was not given.

This does NOT judge beauty. It ranks survivors by how far they stand out
from their surroundings after the blur, which is exactly the quantity he
describes. The reviewer decides whether the ranking is the intended one.

Blur radius scales with image height so a 1440p video still and a 1504px
screen capture are comparable.
"""
import argparse
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter

# He blurs "a screenshot" without giving a radius. 1.5% of height removes
# glyph detail (a 15px cap height at 1440) while keeping panel-scale shapes.
SIGMA_FRAC = 0.015
# A survivor must beat its surroundings by this much to count as shouting.
SHOUT_MIN = 6.0


def survivors(path, sigma_frac=SIGMA_FRAC, crop=None):
    im = Image.open(path).convert("RGB")
    full_h = im.size[1]
    if crop:
        im = im.crop(crop)
    a = np.asarray(im).astype(np.float32)
    h, w = a.shape[:2]
    # Scale from the ORIGINAL height even when cropped, so cropping to the
    # bar strip does not silently shrink the blur to 2px and turn the test
    # into an edge detector. The caller passes the full-frame height.
    sigma = max(2.0, (full_h or h) * sigma_frac)
    # Luminance, not max-channel: a saturated blue tile is loud to the eye
    # even when its green channel is unremarkable.
    lum = a @ np.array([0.2126, 0.7152, 0.0722], dtype=np.float32)
    blur = np.asarray(
        Image.fromarray(lum.astype(np.uint8)).filter(
            ImageFilter.GaussianBlur(sigma))
    ).astype(np.float32)

    # "Surroundings" must be LOCAL, not the frame median. Half of any image
    # is above its own median, so a global baseline labels the wallpaper
    # itself as one giant survivor -- which is what the first version did.
    # A second, much wider blur is the neighbourhood each region sits in;
    # the difference keeps panel-scale objects and cancels broad wallpaper
    # gradients, which appear in both.
    surround = np.asarray(
        Image.fromarray(lum.astype(np.uint8)).filter(
            ImageFilter.GaussianBlur(sigma * 6))
    ).astype(np.float32)
    base = float(np.median(blur))
    dev = blur - surround

    # Label connected bright regions on the blurred image. Simple flood
    # fill on a downsampled grid -- exact pixel boundaries do not matter
    # once the image is blurred by 20px.
    step = max(2, int(sigma / 2))
    grid = dev[::step, ::step]
    # Signed: a black pill on a bright wallpaper shouts just as loudly as a
    # bright tile on a dark one. Rank by magnitude, keep the sign for the report.
    mask = np.abs(grid) > SHOUT_MIN
    seen = np.zeros_like(mask, dtype=bool)
    out = []
    H, W = mask.shape
    for y in range(H):
        for x in range(W):
            if not mask[y, x] or seen[y, x]:
                continue
            stack = [(y, x)]
            seen[y, x] = True
            cells = []
            while stack:
                cy, cx = stack.pop()
                cells.append((cy, cx))
                for dy, dx in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                    ny, nx = cy + dy, cx + dx
                    if 0 <= ny < H and 0 <= nx < W and mask[ny, nx] and not seen[ny, nx]:
                        seen[ny, nx] = True
                        stack.append((ny, nx))
            if len(cells) < 3:
                continue
            ys = [c[0] for c in cells]
            xs = [c[1] for c in cells]
            vals = [float(grid[c[0], c[1]]) for c in cells]
            out.append({
                "bbox": [min(xs) * step, min(ys) * step,
                         (max(xs) + 1) * step, (max(ys) + 1) * step],
                "area_px": len(cells) * step * step,
                "shout": round(max(vals, key=abs), 1),
                "shout_mean": round(sum(vals) / len(vals), 1),
                "sign": "bright" if max(vals, key=abs) > 0 else "dark",
            })
    out.sort(key=lambda r: -abs(r["shout"]))
    return {
        "png": str(path), "w": w, "h": h,
        "sigma_px": round(sigma, 1),
        "baseline_lum": round(base, 1),
        "n_survivors": len(out),
        "survivors": out[:12],
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("png")
    ap.add_argument("--sigma-frac", type=float, default=SIGMA_FRAC)
    ap.add_argument("--crop", default=None,
                    help="x0,y0,x1,y1 -- judge ONE region, not the desktop. "
                         "For panel-vs-panel comparison crop to the bar "
                         "strip: open windows and wallpaper texture "
                         "otherwise dominate the ranking.")
    ap.add_argument("--quiet", action="store_true")
    args = ap.parse_args()
    crop = tuple(int(v) for v in args.crop.split(",")) if args.crop else None
    if crop and len(crop) != 4:
        print("--crop needs x0,y0,x1,y1", file=sys.stderr)
        return 2
    if not Path(args.png).exists():
        print(f"no such file: {args.png}", file=sys.stderr)
        return 2
    r = survivors(args.png, args.sigma_frac, crop)
    if args.quiet:
        print(json.dumps(r))
        return 0
    print(f"{r['png']}  {r['w']}x{r['h']}  blur sigma {r['sigma_px']}px  "
          f"baseline lum {r['baseline_lum']}")
    print(f"{r['n_survivors']} region(s) still shouting after the blur:")
    for i, s in enumerate(r["survivors"], 1):
        x0, y0, x1, y1 = s["bbox"]
        print(f"  {i:2}. {s['sign']:6} shout {s['shout']:7.1f} "
              f"(mean {s['shout_mean']:6.1f})  area {s['area_px']:7d}px  "
              f"bbox x[{x0},{x1}] y[{y0},{y1}]")
    if not r["survivors"]:
        print("  (nothing survives -- either genuinely quiet, or the panel "
              "is not in this frame)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
