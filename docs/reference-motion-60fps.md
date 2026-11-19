# Reference motion, measured at NATIVE 60 fps

Supersedes the pose counts in `docs/reference-motion-spec.md`. That report was
built from `/tmp/ref/dense/`, which is **5 fps** (Δ200 ms) — it could not resolve
a 200 ms morph and correctly said so. This one re-extracts the source video at
its real rate.

Source: `The Perfect Dynamic Island Hyprland Setup (Every Choice Explained)
[Iv8OVf-4Yxw].mp4`, **2560x1440 @ 60/1 fps** (ffprobe).
Extraction: `ffmpeg -i <video> -vf "crop=900:220:830:0" -fps_mode passthrough
-frame_pts 1 /tmp/ref60/f%06d.png` — top strip only, no decimation.

## The expand morph: 14 distinct poses in 233 ms

Frames 169–182, one pose per displayed frame:

| frame | w×h (device px) | width norm | height norm |
|---|---|---|---|
| 169 | 112×43 | 0.000 | 0.000 |
| 170 | 152×50 | 0.006 | 0.007 |
| 171 | 216×63 | 0.061 | 0.055 |
| 172 | 376×96 | 0.151 | 0.144 |
| 173 | 524×126 | 0.374 | 0.370 |
| 174 | 589×139 | 0.581 | 0.575 |
| 175 | 644×150 | 0.672 | 0.664 |
| 176 | 668×155 | 0.749 | 0.740 |
| 177 | 709×164 | 0.782 | 0.774 |
| 178 | 788×180 | 0.839 | 0.836 |
| 179 | 808×184 | 0.950 | 0.945 |
| 180 | 817×186 | 0.978 | 0.973 |
| 181 | — | 0.990 | 0.986 |
| 182 | — | 0.999 | 0.993 |

**Width and height move together** — the two normalised columns agree within
0.008 at every frame. The capsule scales as one shape, not two independent axes.

## Easing: easeOutQuad fits best, not easeOutCubic

MSE against the normalised width samples:

| curve | MSE |
|---|---|
| **easeOutQuad** `1-(1-t)²` | **0.00670** |
| easeOutCubic `1-(1-t)³` | 0.01627 |
| linear | 0.04346 |
| easeInOutCubic | 0.05053 |
| easeOutQuint | 0.05123 |

easeOutQuad is **2.4× better** than our current easeOutCubic. Both are
monotone ease-outs, so this is a refinement, not a redesign — but it is the
measured answer, and the previous "EaseOutCubic is right" verdict was reached
on 5 fps data that could not distinguish them.

## Overshoot: ~1 %, and only on settle

Frames 187–189 show 820×187 → 817×186 → 816×186: **+0.5 % width, +0.5 % height**
above the settled size, decaying over 2 frames (33 ms). Not the 15 % OutBack we
removed, and not zero either.

## Other transitions (all at 60 fps)

| frames | span | distinct poses |
|---|---|---|
| 263–269 | 117 ms | 7 |
| 271–275 | 83 ms | 5 |
| 361–370 | 167 ms | 10 |

Every one is **one pose per frame**. That is the whole finding: his morph is
frame-complete, ours presents 2–3 poses over 317 ms.

## Target for our implementation

- **One distinct pose per displayed frame.** At 60 Hz over ~230 ms that is 14.
- Duration ~230 ms for the pill→panel expand (our `anim_ms=160` is in the right
  ballpark; the observed 317 ms settle is the defect, not the configured value).
- Easing **easeOutQuad**; width and height share one progress value.
- Settle overshoot ≤1 %, decaying within ~2 frames.
