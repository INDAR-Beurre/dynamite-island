# Smoothness baseline — how it is measured, what it says today

Everything here is re-derivable. One command re-captures and re-scores:

```bash
cd /home/alex/rust-island-build/scripts/motion
./run_all.sh /tmp/motion-baseline/roundN 5     # N = the new round number
```

Media goes to `/tmp` only (a 2 s 60 Hz recording is ~45 MB of PPM frames and the
repo's `.gitignore` does not cover media).

## The capture path, and why it is this one

`scripts/motion/record.sh` records the panel with **wf-recorder 0.6.0** (installed
from the Artix `world` repo — `pacman -Ss wf-recorder`). No capabilities were set,
so nothing had to be removed; `kmsgrab` was not needed and was not attempted.

Why not the alternatives:

| path | status |
|---|---|
| grim loop (`capture.sh`) | ~20 fps ceiling, **no vsync**, timestamps are "when grim could get a frame" — cannot truthfully report frame pacing. Kept as a control. |
| `obs`, `wl-screenrec`, `gpu-screen-recorder` | not installed, not in the repos. |
| ffmpeg `kmsgrab` | needs `CAP_SYS_ADMIN`; rejected in favour of a repo package that needs none. |
| xdg-desktop-portal screencast | no screencast portal present on this box, no `pw-cli ls Node` screen-cast node. |

`record.sh <outdir> <seconds> [fps] [--region x0,y0,x1,y1] [--trigger "CMD@SEC"] [--vfr]`

* container `-c ffv1` in `.mkv` — lossless bgr0, **no fps filter is inserted**
  ("Using video filter: null"), so nothing between the compositor and the frames
  invents or drops a timestamp.
* default (`cfr`) adds `-D -r 60`: sample every display slot, once. A duplicate
  frame then means *the panel had nothing new for that slot*.
* `--vfr` omits `-D -r`: real damage-driven PTS. Verified `-D` alone gets 60.1
  real buffers/s (175 frames / 2.897 s, 16/17 ms jitter), so CFR is not padded.
* must be stopped with `kill -INT` (SIGTERM leaves `moov atom not found`) —
  `record.sh` does that.
* frames: `ffmpeg -fps_mode passthrough -pix_fmt rgb24 frame_%04d.ppm`
  (`-vsync` no longer exists in ffmpeg 9), PTS from
  `ffprobe -show_entries frame=pts_time`.

Region: **640,0,900,210 logical** (device 390×315 at scale 1.5) — the pill and the
panel that expands under it. Behind it is the T3 window, which streams text, so
every run is checked for changes *before* the trigger; a trial with any is
retried (`run_all.sh`, `ATTEMPTS=4`) rather than reported.

## The numbers today (round2, 5 clean CFR trials, binary mtime 2026-11-19 00:19:03)

Trigger: `animate.py music` at t = 0.8 s, from `collapsed`. 2 s capture, 60 Hz.

| trial | frames | drop | fps | interval med/p95 | poses | dups | jumps | span | C | D | J | **score** |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 114/120 | 5.0 % | 59.9 | 17.0 / 17.0 ms | 2 | 9 | 1 | 183 ms | 0.182 | 0.900 | 1.00 | **10.8** |
| 2 | 114/120 | 5.0 % | 59.9 | 17.0 / 17.0 ms | 2 | 8 | 1 | 167 ms | 0.200 | 0.889 | 1.00 | **11.9** |
| 3 | 115/120 | 4.2 % | 59.9 | 17.0 / 17.0 ms | 3 | 16 | 1 | 316 ms | 0.158 | 0.889 | 0.50 | **22.7** |
| 4 | 115/120 | 4.2 % | 59.9 | 17.0 / 17.0 ms | 3 | 14 | 1 | 283 ms | 0.177 | 0.875 | 0.50 | **24.0** |
| 5 | 115/120 | 4.2 % | 59.9 | 17.0 / 17.0 ms | 3 | 12 | 1 | 250 ms | 0.200 | 0.857 | 0.50 | **25.5** |

**Headline: median 22.7 / 100, min 10.8, max 25.5. Poses per morph: 2–3 (median 3), target 10.**

The immediately preceding run of the same command (round1) gave median 24.0,
poses [2,3,3,3,3]. So **the run-to-run noise floor is about ±1.3 points on the
median**; a fixer must move the median further than that to claim an
improvement, and the honest primary number to watch is `poses` (3 → 10), not the
score's decimal.

Cross-check, independent capture mode (`--vfr`, damage-gated PTS, 33/120 frames
= 17.2 fps): 3 poses over 264 ms, score 31.7 — same pose count in both records,
so the 2–3 is a property of the panel, not of the sampling.

Controls in the same run:
* `01-settled` (grim, 6 s, nothing triggered): 120/121 frames zero-delta →
  `motion: null`, score `unmeasurable`. A static screen scores *nothing*, not 100.
* `02-control` (grim, 20 Hz, same trigger): caught no motion at all — that is the
  grim instrument's blindness to a 160 ms morph, and the reason this file exists.
* `03-sustained` (grim, 3 s, 27 dictation bars driven at 100 Hz): 43 poses over
  2.22 s (19.4 Hz) in round0, i.e. `dictation_bars` repaints track a 20 fps
  sampler continuously — the smoke path is fine, the *morph* is what stalls.
  (Round1 read 6 poses here because the panel was restarted mid-run; treat this
  stage as qualitative.)

### What that means
In ~250 ms of wall time the panel put 2–3 distinct shapes on screen: ~12 poses per
second where the design asks for 60 (one pose per 16.7 ms of a 160 ms
`MORPH_DURATION_MS`). 85–90 % of the display slots inside the transition carried
no new content. Against `src/morph.rs` (160 ms, ease-out cubic) this is the
"still laggy" the user reports: the animation code exists, the pixels do not
arrive. **`poses` is the number that must go up** (3 → 10 → 60), and it is the
number the score weights most (0.40).

## The score

```
score = 100 * ( 0.40*C  +  0.35*(1 - D)  +  0.25*(1 - J) )

C  completeness = min( 1, poses/10, pose_rate_hz/60 )     poses were shown, and
                                                          not just shown slowly
D  duplicate rate = duplicates / (frames_in_window - 1)   slots that carried
                                                          nothing new
J  jump rate     = spikes / (poses - 1)                    steps > 1.8x the
                                                          window's median step
```

Each term is measurable from the frames alone, and each is capped so it cannot be
gamed: capturing less, moving slower, or rendering fewer poses all lose points
(`selftest.py` asserts perfect > duplicate > slideshow > teleport). `poses < 2`
pins `D = J = 1` (a one-frame teleport must not collect 35 free "no duplicates"
points and score 39). A window with no motion at all is `unmeasurable`, not a
score.

Round N+1 is comparable to round N only if it used the same region, the same
trigger, the same recorder mode and ≥5 trials — `run_all.sh` fixes all of that and
prints the median.

## What this instrument cannot prove

1. **It measures what was presented, not what was rendered.** PTS says a frame
   reached the compositor's output; it cannot say whether the panel laid out 3
   poses or 60. If the panel draws 60 poses and the compositor presents 3, the
   score is still 3 — and to the user that *is* the smoothness they see. Blame
   cannot be assigned between panel and compositor from these numbers alone.
2. **CFR mode imposes the clock.** With `-D -r 60` the intervals are a constant
   16.7 ms grid, so `interval_ms` in CFR mode is an artifact and pacing has to be
   read from the duplicate and jump rates instead. In CFR mode wf-recorder also
   forces the compositor to wake 60×/s, which can only make the panel look *better*
   than it does for a user staring at an idle screen. `--vfr` avoids both but gets
   only ~2–3 samples of a 160 ms morph.
3. **It cannot see inside a transition.** 10 poses is a target, not a proof:
   a 4-pose transition that skips the visually important middle still scores
   partially (that is what `J` is for) but the score does not know *which* pose
   was swallowed.
4. **The trigger anchor is approximate.** `trigger_ms` is wall clock minus the
   first PTS; wf-recorder's start-up puts ~0.1 s of slack there. The window is
   ±100 ms/+400 ms wide to absorb it, which also means a change 300 ms after the
   command is still counted as part of the transition.
5. **The screen is shared.** Other agents open windows and drive the same panel;
   the T3 backdrop streams text continuously. `contamination.clean` (nothing
   moved before the trigger) is the only guard — it cannot detect a change that
   happens to look like a morph.
6. **The binary under test moves.** Every run records `panel.txt` (pid, start
   time, exe mtime). A run against a binary rebuilt mid-baseline measures a
   mixture; compare rounds by that file, not by the date.
7. **`03-sustained` is qualitative.** The panel's state is not reset between
   stages, so its pose count depends on what the previous stage left behind.

## Instrument self-test

`python3 scripts/motion/selftest.py` feeds the real analyser synthetic sequences
with known answers (perfect ramp → 100; injected duplicates → 68.5; injected jump
→ 97.2; 3-pose slideshow → 48.7; one-frame teleport → 4.0; static → unmeasurable;
a pre-cropped recorder frame must not be cropped twice) and asserts ordering.
A detector that has never been shown a known stall cannot certify smoothness.
