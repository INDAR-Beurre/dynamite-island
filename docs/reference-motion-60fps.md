# Reference motion, measured at native 60 fps

Critic #30b scored the morph against `/tmp/ref/dense/` and correctly reported
that those PNGs are **5 fps** (Δ200 ms), so pose counts there are meaningless.
The parent re-extracted from the source video at its true rate:

```
ffmpeg -i "The Perfect Dynamic Island Hyprland Setup (Every Choice Explained) [Iv8OVf-4Yxw].mp4" \
       -vf "crop=900:220:830:0" -fps_mode passthrough -frame_pts 1 /tmp/ref60/f%06d.png
```

(`-vsync` is rejected by this ffmpeg build; `-fps_mode passthrough` keeps every
source frame. All source videos are 60/1 fps.)

## The expand, frame by frame

Dark-capsule bbox per frame, `max(channel) <= 24`, top band only:

| frame | w | h | | frame | w | h |
|---|---|---|---|---|---|---|
| 165–168 | 111 | 42 | | 176 | 668 | 155 |
| 169 | 116 | 43 | | 177 | 709 | 164 |
| 170 | 156 | 50 | | 178 | 788 | 180 |
| 171 | 220 | 64 | | 179 | 808 | 184 |
| 172 | 380 | 96 | | 180 | 817 | 186 |
| 173 | 525 | 126 | | 181 | 823 | 187 |
| 174 | 590 | 139 | | 182 | **825** | **188** |
| 175 | 645 | 150 | | 183–191 | 825→816 | 188→186 |

**14 distinct poses in 233 ms — one per displayed frame.** Ours presents 2–3.
That single number is the owner's "really laggy" complaint.

Other transitions, same method: frames 263–269 = 7 poses/117 ms;
271–275 = 5/83 ms; 361–370 = 10/167 ms. **Every one is one pose per frame.**

## Width and height share one progress value

Normalised to the settled 825×188 from the 111×42 pill, the two columns agree
within **0.0082 at every frame** (mean 0.0043). They are not independently
animated — one `t` drives both.

## Easing: easeOutQuad, not easeOutCubic

MSE against the 14 normalised samples, both axes:

| curve | MSE_w | MSE_h | mean |
|---|---|---|---|
| **easeOutQuad** | 0.00257 | 0.00261 | **0.00259** |
| easeOutCubic (ours today) | 0.01158 | 0.01243 | 0.01201 |
| easeOutQuart | 0.02734 | 0.02869 | 0.02802 |
| linear | 0.03582 | 0.03436 | 0.03509 |
| easeOutQuint | 0.04307 | 0.04476 | 0.04391 |
| easeInOutCubic | 0.05045 | 0.04870 | 0.04957 |

**easeOutQuad fits 4.6× better than our current easeOutCubic.** The earlier
"EaseOutCubic is defensible" verdict came from 5 fps samples that could not
separate the two curves.

## Overshoot: ~1%, decaying over ~150 ms

825×188 at frame 182 → 816×186 at 189, then flat. That is **+1.1% width,
+1.1% height** above the resting size, settling over 9 frames. Not the 15%
OutBack that rendered as a one-frame black slab, and not zero either.

## Target for our morph

| | reference | ours today |
|---|---|---|
| poses per morph | **14 (one per frame)** | 2–3 |
| growth duration | 233 ms | 160 ms configured, 317 ms observed |
| easing | **easeOutQuad** | easeOutCubic |
| w/h progress | shared | shared ✓ |
| settle overshoot | ~1.1% over 150 ms | none |
