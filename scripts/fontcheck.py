#!/usr/bin/env python3
"""Subpixel stem/cap and width/cap for a clock render, vs the reference.

Why this exists: the modal INTEGER stem width is quantised -- 2px vs 3px is
one pixel, and it hid a real 15% weight deficit for several rounds. Integrating
coverage across each stem gives a continuous number that separates weights.

Reference targets, measured on four frames across two wallpapers:
    stem/cap 0.1846   w/cap 3.600
"""
import sys, numpy as np
from PIL import Image, ImageDraw, ImageFont

REF_STEM_CAP = 0.1846
REF_W_CAP = 3.600
STRINGS = ("00:05", "05:36", "12:48", "23:59")


def render(font, text, logical_px, tracking_em=0.0, scale=1.5, ss=2):
    """Raster at 2x device px then downsample, as the real pipeline does."""
    dev = logical_px * scale
    f = ImageFont.truetype(font, int(round(dev * ss)))
    im = Image.new("L", (int(dev * ss * 9), int(dev * ss * 3)), 0)
    d = ImageDraw.Draw(im)
    pen = dev * ss * 0.5
    track = tracking_em * logical_px * scale * ss
    for ch in text:
        d.text((pen, dev * ss * 0.5), ch, font=f, fill=255)
        pen += f.getlength(ch) + track
    a = np.asarray(im)
    ys, xs = np.where(a > 0)
    if not len(xs):
        return None
    im = im.crop((xs.min(), ys.min(), xs.max() + 1, ys.max() + 1))
    return im.resize((max(1, round(im.width / ss)), max(1, round(im.height / ss))),
                     Image.LANCZOS)


def metrics(im):
    a = np.asarray(im).astype(float)
    ink = a > 60
    if ink.sum() < 20:
        return None
    ys, xs = np.where(ink)
    cap = ys.max() - ys.min() + 1
    tw = xs.max() - xs.min() + 1
    span = a.max() - a.min()
    if span < 1:
        return None
    norm = (a - a.min()) / span
    widths = []
    for row in norm[ys.min() + 2:ys.max() - 1]:
        on = row > 0.25
        s = None
        for i, v in enumerate(on):
            if v and s is None:
                s = i
            if not v and s is not None:
                widths.append(float(row[max(0, s - 1):i + 1].sum()))
                s = None
    widths = [w for w in widths if 1.0 < w < 6.0]
    if not widths:
        return None
    return float(np.median(widths)) / cap, tw / cap, cap


def main():
    font = sys.argv[1] if len(sys.argv) > 1 else "assets/fonts/Inter-Medium.ttf"
    px = float(sys.argv[2]) if len(sys.argv) > 2 else 25 * 0.4842
    track = float(sys.argv[3]) if len(sys.argv) > 3 else 0.0
    sc, wc, caps = [], [], []
    for s in STRINGS:
        im = render(font, s, px, track)
        m = metrics(im) if im else None
        if m:
            sc.append(m[0]); wc.append(m[1]); caps.append(m[2])
    if not sc:
        print("no measurable render"); return 1
    a, b = float(np.mean(sc)), float(np.mean(wc))
    print(f"font {font}  px {px:.2f}  tracking {track:.4f}em  cap {caps[0]}")
    print(f"  stem/cap {a:.4f}  ref {REF_STEM_CAP}  {100*(a/REF_STEM_CAP-1):+.1f}%")
    print(f"  w/cap    {b:.4f}  ref {REF_W_CAP}  {100*(b/REF_W_CAP-1):+.1f}%")
    ok = abs(a / REF_STEM_CAP - 1) <= 0.05 and abs(b / REF_W_CAP - 1) <= 0.05
    print("RESULT", "PASS" if ok else "FAIL", "(both within 5%)")
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
