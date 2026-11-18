# scripts/motion — motion-smoothness capture & analysis

Measures whether the dynamic island's animations actually put smooth new frames
on screen, using only what this machine has. **There is no screen recorder
installed** (no `wf-recorder`, no `wl-screenrec`, no `obs`), so the capture path
is a `grim` screenshot loop.

Read the caveats before quoting any number from this directory. A measurement
that overstates its precision is worse than none.

---

## Files

| file | what it does |
|---|---|
| `capture.sh <outdir> <seconds> [fps] [--region x0,y0,x1,y1] [--trigger "CMD@SEC"] [--format ppm\|png]` | loops `grim`, writes `frame_NNNN.ppm`, `timestamps.txt`, `meta.txt` |
| `analyze.py <outdir> [--region …] [--json]` | drop rate, interval stats, delta histogram, duplicate frames, spike frames, morph-window annotation |
| `animate.py <command> [args]` | sends one control-socket command; `--list` prints everything it will send |
| `run_all.sh [outdir]` | settled 5 s capture + one transition capture + one-screen summary |
| `out/<timestamp>/` | results of each `run_all.sh` run |

## Quick start

```sh
bash scripts/motion/run_all.sh          # the baseline
python3 scripts/motion/animate.py --list
bash scripts/motion/capture.sh /tmp/c 5 20 --region 600,0,900,150
python3 scripts/motion/analyze.py /tmp/c
```

---

## Where the control socket and commands came from

Read out of `src/main.rs`, not guessed:

* **Socket path** — `fn socket_path()`, `src/main.rs:1855-1861`:
  `$XDG_RUNTIME_DIR/dynamite-rust.sock`, falling back to `/tmp`.
  Live path here: `/run/user/1000/dynamite-rust.sock`.
* **Socket type** — `UnixDatagram::bind` + `set_nonblocking(true)`,
  `src/main.rs:6065-6068`. It is a **datagram** socket: one 512-byte-max
  datagram, one trimmed word. `SOCK_STREAM` and `SOCK_SEQPACKET` fail with
  `errno 91 Protocol wrong type for socket`.
* **Handler** — `src/main.rs:6073-6213`.
* **State words** — mapped by `IslandState::from_word` (`src/main.rs:165-181`)
  then `begin_morph(...)`: `collapsed`, `dictation`, `lock`, `power`, `music`,
  `calendar`, `controlcenter`, `launcher`, `themes`, `clipboard`, `traymenu`.
  Each starts the 160 ms morph (`MORPH_DURATION_MS`, `src/morph.rs:85`).
* **Other safe words** — `clock` (alias of collapsed), `reload`,
  `dictation_start`, `dictation_end`, `dictation_bars: <27 floats 0..1>`
  (prefix-matched, `src/main.rs:6073`), and the lockscreen *preview* words
  `locktype` / `lockback` / `lockwrong` (no-ops outside the Lockscreen state).
* **Refused on purpose** by `animate.py`: `quit` (would kill the panel) and
  `lockscreen` (takes over the whole screen). `animate.py` also silently
  refuses anything not on its allow-list, so it cannot lock the screen, power
  off, or change the wallpaper.

The panel redraws a morph on a **16 ms timer** (`Duration::from_millis(16)`,
`src/main.rs:4546,4572`), i.e. it aims at ~62 fps for 160 ms ≈ 10 frames.

---

## Capture caveats — the honest list

1. **`grim` is a screenshot tool, not a recorder.** Each call is a fresh
   round-trip to the compositor. Captures are **not vsync-locked**, and the
   loop's spacing is limited by how long `grim` itself takes. The nominal `fps`
   argument is a **ceiling**, and every interval number below (median/mean/p95)
   describes *how fast grim could go*, **not** how the compositor clocked.
2. **Frame intervals are approximate to a few ms.** Each timestamp is two
   `date +%s%N` subprocesses (~1.5 ms each) around a grim call. Intervals are
   start-to-start, so the grim duration is included but not hidden.
3. **What IS trustworthy: the pixel data.** The number of *distinct* frames and
   the mean-absolute-delta between consecutive frames describe what actually
   reached the screen. A zero-delta pair means the screen did not change — that
   claim does not depend on timestamps at all.
4. **The 160 ms morph is under-sampled.** Best achieved rate is ~20-22 fps with
   a 300x150-logical-px crop on an *unloaded* box, so only ~3-4 frames land
   inside a 160 ms animation. `analyze.py` prints a MORPH WINDOW section and
   warns when fewer than 4 frames land in it. Sustained animations (dictation
   bars) are the reliable subject; the one-shot morph is not.
5. **Host load dominates.** Other agents run `cargo` and `ffmpeg` (a single
   `ffmpeg` transcode hit ~400% CPU, load average 18 on 8 cores). Under that
   load the same crop drops from ~20 fps to ~8 fps. `meta.txt` records
   `loadavg_start`/`loadavg_end`/`nproc`, `analyze.py` prints a `LOAD` line and
   flags contention. **Do not compare fps numbers across runs at different
   load.**
6. **`--region` is in LOGICAL px** (grim's own `-g` space), but the image comes
   back at **device** resolution, 1.5x. `--region 600,0,900,150` yields a
   450x225 PPM. The screen is 1504x1002 logical = 2256x1504 device.
7. **PPO/PPM only, in practice.** PNG costs `grim` ~313 ms encode; PPM decode is
   35 ms. Default format is `ppm`. PPM files are ~2-3 MB each; a 5 s run is
   ~150-190 MB.
8. **The pill is not the only moving thing.** A 300x150 crop still contains
   clock digits changed per minute and any compositor effects. The picket band
   is centred on the island, so the clock flipping is a real delta source. For
   the morph captures the crop also contains parts of the expanded state.

## Captures that failed (tried, documented, rejected)

Checked with ffmpeg 7.1 on this box:

| method | result |
|---|---|
| `ffmpeg -f kmsgrab` | `No usable planes found. Error opening input: Invalid argument` (needs DRM master / root) |
| `ffmpeg -f wayland` | `Unknown input format: 'wayland'` — not compiled in |
| `ffmpeg -f pipewire` | `Unknown input format: 'pipewire'` — not compiled in |
| `ffmpeg -f x11grab` | opens (X screen 1504x1003), but the frame is **100% black** (mean pixel 0.0) — the panel never reaches the X server |

`ffmpeg -devices` lists only: alsa, fbdev, iec61883, jack, kmsgrab, lavfi, oss,
pulse, sndio, v4l2, x11grab, xv. **No working ffmpeg capture path exists without
root.** The `grim` loop is the only option, and that is the source of every
timing caveat above.

---

## How to read the output

```
FRAMES      requested 80  captured 81  drop -1.7%
INTERVAL    median 50.0 ms   mean 49.9 ms   p95 54.4 ms  (grim itself: 31.1 ms median) => 20.0 fps achieved
LOAD        1-min loadavg at start 7.01 on 8 cores
CROP        device px 0,0 -> 450,225  (450x225)
DELTA       median(nonzero) … max … spike thr …
HISTOGRAM   buckets of consecutive-frame mean-abs delta
DUPLICATES  N frame(s) identical to predecessor
SPIKES      N jump(s) > threshold
MORPH WINDOW t=… .. … ms (160 ms morph), N frame(s) landed in it
```

* **`drop -1.7%`** — negative means the loop beat its nominal target. Normal
  here; the fps argument is only used to pace, and pacing is skipped whenever
  grim is already slower than the period.
* **`DUPLICATES` outside a trigger window are not a fault.** A settled pill
  does not redraw, so consecutive captures are identical. Use the settled
  capture in `run_all.sh` as the **control**: its duplicate count is the noise
  floor. Only duplicates **inside** the MORPH WINDOW indicate a stalled redraw.
* **`SPIKES`** — a delta more than 3x the median non-zero delta. That is a jump
  big enough to have skipped an intermediate frame. On a static screen the
  threshold degenerates to 0.5 and no spikes are reported.
* **Thresholds**: `ZERO_EPS = 0.05` (mean-abs 8-bit delta) counts as identical;
  `SPIKE_K = 3.0`.

## Reproducing a baseline fairly

Baseline numbers are only comparable at similar load. `meta.txt` records the
load average — check it before diffing two runs. Prefer the **settled vs
transition duplicate delta** and the **distinct-frame count inside the morph
window** over raw fps.
