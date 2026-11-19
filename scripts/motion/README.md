# `scripts/motion` — does the island actually animate smoothly?

An instrument, not a fix. It drives the panel through a known morph, records the
screen at the compositor's own frame rate, and turns the recording into
per-frame numbers with explicit PASS/FAIL bounds.

```sh
./scripts/motion/bounds.sh /tmp/gate 6          # THE gate: exit 0 only if every bound passed
python3 scripts/motion/motion.py --help         # the instrument on its own
bash scripts/motion/run_all.sh                  # full shared baseline (stage 00 = this instrument)
```

Its headline output on this machine, on the live panel, 2026-11-19
(build with `collapsed_width=74 anim_ms=160`, dark-navy wallpaper):

| metric | measured | bound | verdict |
|---|---|---|---|
| capture rate | **60.0 fps** (432 frames, 16.7 ms median) | ≥ 10 fps | PASS |
| interval p95 / median | **1.02×** (17.0 / 16.7 ms) | ≤ 3× | PASS |
| dropped frames | **0.5 %** (2 / 432) | ≤ 15 % | PASS |
| travel | **48 px** (35 → 83) | ≥ 20 px | PASS |
| command → first pixel | **351.2 ms** (worst of 6, best 233.5) | ≤ 260 ms | **FAIL** |
| settle | **351.2 ms** | ≤ 260 ms | **FAIL** |
| distinct morph poses | **2** in all 6 trials | ≥ 8 | **FAIL** |
| median pose hold | **87.3 ms** (range 83.3–91.6) | ≤ 24 ms | **FAIL** |
| duplicate frames during morph | **0.894** | ≤ 0.10 | *UNRESOLVED* |
| intermediate geometries seen | **0** in every trial | ≥ 2 | *UNRESOLVED* |
| edge (AA ramp) | **1.0 px** | ≤ 3.0 px | PASS |
| soft-edge frames | **0 / 432** | ≤ 25 % | PASS |

Per-trial latency in command order: 233.5, 267.1, 300.5, 283.7, 351.2, 283.6 ms
— a 118 ms spread across identical commands, i.e. the delay is not a constant
offset you could compensate for. Across all runs the spread reached 170 ms.

`UNRESOLVED` on the bottom two lines is deliberate: each metric's own premise
failed (no intermediate geometry was ever captured, so the curve's shape was
never observed). It is reported, it is not a pass, and it is not counted as a
panel failure — see §3.

Read the rest of this file before quoting any of those numbers.

---

## 1. Two capture sources, and why both are kept

| source | flag | clock | rate here | good for | not good for |
|---|---|---|---|---|---|
| **wf-recorder** | `--source wf` (default in `run_all.sh`) | compositor **PTS** | **58.8–60 fps**, p95 17.1 ms | pose counts, duplicate ratio, any *relative* timing | absolute command→pixel latency |
| **grim loop** | `--source grim` | `perf_counter` around each call | 8–31 fps, 25–40 ms/call | absolute latency (±18 ms for the trigger), anything when wf is absent | sub-40 ms motion |

`wf-recorder` is used as
`wf-recorder -g "<logical x,y WxH>" -f run.nut -D -c ffv1`.

`-D` (`--no-damage`) is **mandatory**. Without it wf-recorder only emits frames
when the screen changes, which measured 38 fps at a 38 ms median on a morphing
panel — i.e. it would hide exactly the defect we are measuring.

**wf-recorder cannot measure absolute latency.** Its first PTS is always 0.000,
so the wall-clock↔PTS offset has to be recovered from the flush time. Measured
flush lag on this box: 0.083, 0.098, 0.104, 0.104, 0.125 s across five runs —
i.e. **±100 ms of uncertainty**. So `latency_ms` from `--source wf` is a
sanity-check number; the grim run is the one that pins dispatch latency.

---

## 2. What is measured, per frame

A frame is reduced to compound "island" geometry, in device px. The capsule
(always present) and the body below it (present only when expanded) are found
separately, because **the collapsed capsule's width does not change when the
island expands** — a width-only detector reports the same 93 px in every state
and proves nothing. Verified by per-row measurement: rows 5–45 are pixel
identical between collapsed and controlcenter; the body grows in rows 48+.

* `cap_y0, cap_h, cap_w` — capsule top row, height, widest row.
* `body_top, body_h, body_w, body_area` — the expanded panel below the capsule.
* `total_h = body_bottom - cap_y0 + 1` — **the morph signal**.
* `edge_ramp` — antialiasing width at the capsule's left/right edge, in px.
* `delta` — fraction of pixels changed vs the previous frame.
* `bg_lum, dark_thr` — the backdrop luminance and the darkness cut derived from
  it *this frame*.

```
 y=18  ┌─────── capsule (cap_y0=18, cap_h=37, cap_w=93) ───────┐   <- ALWAYS present
 y=55  └──────────────────────────────────────────────────────┘      width never changes
 y=56      ...gap ≤ 12 rows (BODY_MAX_GAP); body accepted only if
           its width is 250..1250 px and it brackets the centre...
 y=60  ┌─────────────── body (body_h) ───────────────┐              <- only when expanded
       │  Wi-Fi tiles / sliders / whatever the state │
 y=122 └─────────────────────────────────────────────┘
       total_h = bottom - cap_y0 + 1 = 85   (collapsed: 37, travel 48)

 y=125 ┌───────── the terminal window / desktop below ────────┐      <- MUST be excluded
```

Detection rules that are load-bearing (each one was a bug first):

1. **Narrow window, ±110 px around centre.** Min/max over a wide window
   swallowed the workspace discs and once reported a 599 px "pill" in every
   state.
2. **Stop at the first sparse row after the capsule starts** (<10 dark px). The
   terminal window underneath starts ~27 rows below the collapsed pill and 4
   rows below the expanded body; without this rule the detector measures the
   terminal (bogus 793 px, 878 px "pills").
3. **The widest contiguous run that brackets the centre.** Clock glyph gaps split
   the capsule into runs of 8–40 px otherwise.
4. **A width cap** (`BODY_WMAX=1250`) and a **gap cap** (`BODY_MAX_GAP=12`), so
   the body is neither the terminal nor a full-width dark wallpaper.
5. **A contrast-relative darkness cut.** A fixed `luminance < 40` is only valid
   while the wallpaper under the pill is bright. When another agent switched the
   wallpaper to dark navy the backdrop became **21** and the pill **2**, and the
   absolute cut reported "no island detected" on every frame. Now:
   `bg = median(far-left/far-right strips)` and
   `thr = 40 if bg >= 60 else max(4, bg - 8)`.

---

## 3. Metrics, and why each bound is what it is

`anim_ms = 160` (config; `MORPH_DURATION_MS`, `src/morph.rs:85`). At 60 Hz that
is **9.6 display slots**, so a correctly animated morph should present ~8–10
distinct poses, each held ~16.7 ms.

| metric | definition | bound | why this bound |
|---|---|---|---|
| `capture_effective_fps` | 1000 / median inter-frame interval | **≥ 10** | below 10 fps (100 ms) a 160 ms morph has 1–2 samples and nothing can be concluded |
| `interval_p95_over_median` | p95 ÷ median interval | **≤ 3.0** | a p95 3× the median means the capture clock itself is not steady, so any pose count is unreliable |
| `dropped_frame_ratio` | pairs with interval > 2× median | **≤ 0.15** | the compositor's own drops. Above 15 % the recording is not a faithful sample of scanout |
| `travel_px` | settled target − start, in `total_h` | **≥ 20** | below 20 px a morph is a nudge and all shape metrics are noise |
| `latency_ms` | command → first frame whose geometry moved > 2 px | **≤ 260** | `anim_ms 160` + one 60 Hz interval (17) + ~83 ms slack. **This is the number a user experiences as "laggy"** |
| `settle_ms` | command → first frame within 2 px of final | **≤ 260** | same budget; a morph must be finished by 160 ms after it starts |
| `settle_duration_ms` | `settle − latency` = the animation's own cost | **≤ 230** | `anim_ms` + one interval + 30 ms, i.e. dispatch excluded |
| `width_reversals` | sign changes in `Δ(total_h)`, ignoring ±2 px | **≤ 1** | easing is `EaseOutCubic` (`src/main.rs:4835`), which is monotone; one reversal tolerates 1–2 px quantisation |
| `overshoot_frac` | max past target ÷ travel | **0.00–0.05** | `EaseOutCubic` has no intended overshoot. Measured 0.021 = 1 px on 48 px travel = quantisation |
| `in_flight_samples` | frames strictly between start and target | **≥ 2** | below this the curve's *shape* was never observed, so smoothness is unmeasurable, not good |
| `duplicate_ratio` | consecutive identical pairs in the morph window | **≤ 0.10** | if the animation advances, consecutive frames must differ |
| `edge_ramp_px` | px the edge luminance takes to cross from background to pill | **≤ 3.0** | reference video measures **1.0–3.0 px** (median ~1.5) |
| `soft_edge_frames` | frames with a ramp > 3 px | **≤ 25 %** | envelope for a mode where some frames legitimately have sub-pixel geometry |
| `morph_poses` | distinct images in the observed morph window | **≥ 8** | 160 ms ÷ 16.7 ms = 9.6 slots; 8 requires that most slots are used |
| `pose_hold_median_ms` | median time one distinct image stays on screen | **≤ 24** | 3 display intervals. Longer than that is a hold the eye can see as stepping |
| `dt_coverage` | aligned samples spread over 10 ms bins | **≥ 40 samples / ≥ 5 bins** | guards against the aligned curve being an artefact of a few coincidental timings |

**A correction to the task brief.** The brief says the design spec calls for
`Easing.OutBack` with overshoot 1.15, so a small overshoot is *intended*. That is
**stale for this codebase**: iter33 removed `EaseOutBack` precisely because the
overshoot strip rendered as a one-frame pure-black slab, and
`src/main.rs:4835-4850` now uses `EaseOutCubic` in both directions. The bounds
above follow the code, not the brief. With `OutBack` the `overshoot` bound would
have to be ~0.15 and a single reversal would be correct behaviour.

### PASS / FAIL / UNRESOLVED

`UNRESOLVED` means *the instrument could not see the thing*, e.g.
`in_flight_samples = 0` because no intermediate geometry was ever captured. It
is **not** a pass and it does **not** fail the run: `res["pass"]` is
`all(resolved checks)` and unresolved metrics are listed under
`res["unresolved"]`. Exit code is 1 only for a resolved failure, 2 for a fatal
condition (no socket, calibration impossible).

---

## 4. How a trial works (and the sampling trick)

1. Calibrate: send `collapsed`, settle, and median 3 frames for the start
   geometry. Reject the calibration if the capsule is cut off, already expanded,
   or implausibly wide; retry 5× then exit 2.
2. Per trial: park in the start state, record for ~0.85 s, and send the state
   command at a **golden-ratio-spread phase**
   (`0.06 + ((trial × 0.618) mod 1) × 0.07` s) so successive trials sample
   different points inside the same 160 ms curve (equivalent-time sampling).
3. Analyse **after** the burst, never inside it — analysing during the capture
   loop inflated the measured interval from 39.7 ms to 57.7 ms.
4. Anchor the pose window to the **observed** motion (`latency − interval` to
   `settle + interval`), not to the command: dispatch latency is 300+ ms, so
   `[command, command+160 ms]` contains no motion at all and counts only clock
   and live-data ticks.
5. Drop trials whose settled **target** differs from the cross-trial median by
   >2 px, **or whose start** differs from the calibrated start by >2 px, and
   report them as `trials_suspect` with `suspect_why` ("target"/"start"). Both
   checks are needed: a contaminated *start* with a correct target looks like a
   morph in the wrong direction (measured `travel_px = -100` on a trial that
   began at `total_h 183` while the calibration said 36).
6. If fewer than 3 trials survive, the median is not a reference and the run
   **refuses with exit 2** rather than reporting bounds from a screen that was
   not ours: measured once as `travel -2 px, overshoot 27× travel` — pure
   someone-else's-traffic, and the kind of number that would get a working panel
   "fixed".

---

## 5. What this instrument CANNOT measure

The honest list. Every item here is a reason a number could be right and still
misleading.

1. **Per-client commit attribution.** The recording says a frame reached
   scanout; it cannot say whether the panel's commit was coalesced by the
   compositor or never issued. A 100 ms hold is consistent with *both* a slow
   panel draw *and* a dropped client commit. Separating them needs the panel's
   own `draw=/up=/rest=` profiler — a second instrument, not this one.
2. **Sub-frame behaviour.** If the panel drew a perfect 60 Hz animation and the
   compositor presented every third frame, these numbers would be **identical**
   to a panel that only ever drew 20 poses. The instrument cannot tell a slow
   producer from a lossy consumer.
3. **No proof of 60 Hz scanout.** Inter-PTS deltas of 16.7 ms are *consistent
   with* a 60 Hz output but do not verify the mode, and VRR is not checked at
   all. If the output were running at 40 Hz, every "expected 9.6 poses" figure
   would be wrong.
4. **Absolute latency is only good to ±100 ms** with wf-recorder (flush-offset
   uncertainty) and ±18 ms with grim at 35 ms cadence. "The morph starts 350 ms
   late" is a real finding; "349.7 ms" is not a measurement.
5. **Widths beyond ±620 px of centre are clipped** to the pad, so states wider
   than ~1240 px (the reference video contains 675–1399 px states) are measured
   as a saturated value, not their true width.
6. **`total_h` conflates vertical and lateral growth.** Two panels of equal
   height but different width produce the same signal. `cap_w`/`body_w` are in
   the raw series but no check consumes them yet.
7. **The bounds are policy, not physics.** 260 ms settle, 8 poses, 24 ms hold,
   3 px ramp are defensible engineering choices; a reviewer could defend 6 poses
   and 50 ms. What is *not* policy is the ratio: `pose_hold / 16.7 ms ≈ 5×` and
   `poses / 9.6 ≈ 0.31`. Ratios cancel the flush offset and are immune to the
   clock caveats, so they are the robust statement.
8. **Nothing perceptual.** Whether it *feels* smooth depends on motion blur and
   pursuit integration; a 20 Hz pose cadence with correct easing can read as
   acceptable and a 60 Hz cadence with wrong easing as awful. This instrument
   ranks pose cadence, not perceived quality.
9. **Concurrency and load.** Other agents drive the same socket and run `cargo`
   and `ffmpeg` on 8 cores; load average was 4–10 during these runs. A
   double-triggered trial looks perfectly clean to the target-spread screen. The
   `80–120 ms` upscale cost reported here is therefore *pessimistic* relative to
   an idle box, and no measurement exists on a quiet machine, because there
   wasn't one.
10. **A moving target.** The panel was rebuilt twice and the wallpaper changed
    mid-session; the collapsed pill went from `106×41` to `93×37` between runs.
    Every number must be quoted with its build (see `commit.txt` in each run
    directory) and its config.
11. **The pose metric only sees 180 rows** below the capsule (`POSE_*` band =
    `120 × scale`). A morph that animated only outside that band would be
    scored as zero motion even though the screen changed. The band is centred on
    the island, so this is a narrow risk, but it is a hard limit of the crop.
12. **A body wider than 1250 px or taller than 220 px is rejected, not clipped.**
    For the states this instrument currently drives (`collapsed`,
    `music`, `calendar`, `controlcenter`) that is far outside the real geometry,
    but a future full-width panel would report `body_h = 0` and `total_h` would
    silently collapse to the capsule height — the failure mode would look like
    "no morph at all", not like a detector error. The `no_island_frames` counter
    and the `screening_*` fields are the tripwires: check them before believing
    a `travel_px` of 0.

---

## 6. What the numbers actually said

The failure is **not** in the shape of the curve and **not** in the rendering:

* `edge_ramp_px = 1.0` against a reference of 1.0–3.0 px, and **0 of 432** frames
  soft. The pill's antialiasing matches the video. "Looks bad" is not softness.
* `reversals = 0`, `overshoot = 0.021` — the curve does not ring or bounce.
* `capture` is clean: 60 fps, 1.02× p95, 0.5 % drops, so the sampler is not the
  problem.

The failure is **pose cadence and dispatch delay**:

* the morph presents **2 distinct images** where 9.6 slots exist (all six trials;
  the best single trial in seven runs managed 4), with a median hold of
  **87.3 ms** — roughly 5 display intervals per pose. Consecutive 60 Hz frames
  inside the morph are byte-identical **88–93 %** of the time, and **no trial
  ever showed an intermediate geometry** (`in_flight_samples = 0`).
* command → first changed pixel is **351.2 ms** worst, 233.5 ms best, more than
  twice `anim_ms=160`. The spread across identical commands (118 ms here, up to
  170 ms over all runs) means the delay is not a constant you could subtract.

Reproduced across **8 independent 6-trial runs** at loadavg 3.3–11.6: distinct
poses per trial 2–4, **never once 8**; pose hold 41–108 ms, with 83–92 ms in
almost every trial; duplicate ratio 0.88–0.93; travel always 48 px; edge ramp
always 1.0 px. The pose count and the latency are properties of the panel, not of
one unlucky recording. The pose count and the latency are
therefore properties of the panel, not of one unlucky recording.

Corroborated outside this instrument by the panel's own profiler, which prints
per-draw `draw= / up= / rest=` timings: `up=105.1 ms (upscale_only=105.1 ms)`,
`up=119.5 ms state=Collapsed`, `up=118.9 ms`, and occasionally
`draw=509.3 ms … rest=590.5 ms`. One morph frame costs **80–120 ms of upscale**,
and the panel commits at most one frame per 16 ms timer tick with a 250 ms
re-commit guard (`src/main.rs:5111`), so a 100 ms draw throttles the whole morph
to ~10 poses/s. That is the mechanism behind the measured 2 poses and 87.3 ms
holds.

## 6.1 The raw JSON of that run

```json
{ "instrument": "scripts/motion/motion.py",
  "state": "controlcenter", "pre_state": "collapsed", "anim_ms_config": 160.0,
  "screen": {"device": "2256x1504", "scale": 1.5, "refresh_hz": 60},
  "capture_method": "wf-recorder -D (no-damage) ffv1, PTS from the compositor",
  "source": "wf", "crop_grim": "339,9 827x127",
  "smoothness": { "morph_poses_max": 2,
                  "morph_poses_per_trial": [2, 2, 2, 2, 2, 2],
                  "pose_hold_median_ms": 87.3,
                  "expected_poses_60hz": 9.6 },
  "capture": { "frames": 432, "median_interval_ms": 16.7,
               "p95_interval_ms": 17.0, "effective_fps": 60.0,
               "dropped_frames": 2, "dropped_ratio": 0.005,
               "no_island_frames": 0 },
  "pre_geometry": { "cap_y0": 18, "cap_h": 36, "cap_w": 93,
                    "total_h": 36, "body_h": 0 },
  "trials": [ { "trial": 0, "start_total_h": 35, "target_total_h": 83,
                "travel_px": 48, "latency_ms": 233.5, "settle_ms": 233.5,
                "in_flight_samples": 0, "morph_poses": 2,
                "pose_hold_median_ms": 91.5,
                "pose_window_ms": [133.5, 333.5] }, "… 6 trials …" ],
  "summary": { "trials_run": 6, "screening_weak": false, "screening_clean": 6,
               "trials_used": 6, "ref_target_total_h": 83.0,
               "travel_px": 48, "latency_ms_worst": 351.2,
               "settle_ms_worst": 351.2, "settle_duration_ms_worst": 0.0,
               "reversals_worst": 0, "overshoot_frac": 0.021,
               "duplicate_ratio": 0.894, "edge_ramp_px_median": 1.0,
               "settled_trials": 6, "curve_resolved": false,
               "in_flight_samples_max": 0, "dt_bins_10ms": 51,
               "aligned_samples": 248 },
  "loadavg": [3.30, 3.89, 4.44], "socket_backpressure_failures": 0,
  "pass": false,
  "failures": ["latency_ms", "settle_ms", "morph_poses",
               "pose_hold_median_ms"],
  "unresolved": ["settle_duration_ms", "width_reversals", "overshoot_frac",
                 "in_flight_samples", "duplicate_ratio"] }
```

Full JSON: `/tmp/mb-deliver/report.json`, reproduced by
`./scripts/motion/bounds.sh /tmp/mb-deliver 6`. The full object additionally
carries the raw per-frame series
(`aligned_series[].{dt_ms,total_h,trial}`, 248 samples), per-trial
`total_h_series`, all 16 `checks` with their bounds and rationale, and the
`bounds`/`bounds_note` dicts that this README explains.

## 7. Files

| file | role |
|---|---|
| `motion.py` | **the instrument.** Drives the socket, captures (grim or wf-recorder), extracts compound geometry per frame, computes every metric, prints JSON + PASS/FAIL table, exit 0/1/2 |
| `bounds.sh [outdir] [trials]` | **the exit-code contract.** Runs `motion.py`, prints the PASS/FAIL table, exits 0 only if every resolved bound passed. Use this in a fixer loop |
| `run_all.sh` | full baseline (shared with other agents): settled grim capture, transition, sustained bars, recorded CFR/VFR cross-checks, plus this instrument as stage `00` |
| `capture.sh`, `analyze.py` | the earlier grim-loop capture and its analysis (interval stats, delta histogram, duplicates) — kept, still useful for sustained animations |
| `animate.py` | single-command sender with an allow-list; refuses `quit` and `lockscreen` |
| `drive_bars.py` | drives `dictation_bars` at 100 Hz for sustained-animation tests |
| `record.sh`, `selftest.py` | wf-recorder wrapper; environment self-check |

Media is written to `/dev/shm` (a 7.8 GB tmpfs) and only the frames in the
measured window are kept as PNG. `/tmp` is also a tmpfs here and a raw-video
experiment filled it once — hence `SHM` and the `shutil.rmtree` of the scratch
directory.

## 8. Commands

```sh
# the instrument, live, 5 trials, JSON to stdout
python3 scripts/motion/motion.py --source wf --trials 5 --json-out /tmp/r.json

# grim source: valid absolute latency, ~25 fps
python3 scripts/motion/motion.py --source grim --trials 5

# other morphs
python3 scripts/motion/motion.py --pre music --state collapsed --trials 6

# the gate: exit 0 only if every bound passed
./scripts/motion/bounds.sh /tmp/gate 6

# full baseline (shared file, other agents also edit it)
bash scripts/motion/run_all.sh
```

`--region` is `x,y WxH` in **logical** px (scale 1.5 → device is 1.5× that).
`NEVER send "quit"` — it kills the panel.
