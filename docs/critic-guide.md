# Critic guide — read this before scoring

You are reviewing a Rust replica of a YouTube creator's Wayland "dynamic
island" shell, running on MangoWM. The owner wants a near pixel-perfect
replica, smoother motion, no AI slop, low RAM, and the lock screen replicated.

## Measure, do not impression

Every score must rest on a number you produced yourself. The instruments are
in `scripts/` and they are maintained — use them rather than eyeballing.

```
python3 scripts/measure.py                  # live collapsed pill, one JSON
python3 scripts/measure.py --png <file>     # a still, no live-state consult
python3 scripts/fontcheck.py <ttf> <px> <tracking_em>
python3 scripts/stress/stress.py            # >=1000 asserts, RESULT PASS
bash scripts/motion/run_all.sh <outdir> 3   # poses, fps, latency, settle
python3 scripts/scorecard.py                # trend across recorded rows
```

## Ground truth lives in the repo

`docs/reference/pill-settled.png`, `panel-power.png`, `lock-resting.png`,
`lock-typing.png`, `lock-glyph-frosting.png`. All 2560×1440.

**Do not use `/tmp/ref/s/s17.png`** — it was overwritten with a live capture
during an earlier round. `/tmp` is not a safe home for ground truth.

## Scale-invariant comparison only

His screen is **2560 device px**, ours is **2256 @ scale 1.5**. A 111 px pill
there is a 97–98 px pill here and both are ~4.34 % of screen width. Comparing
absolute pixels across the two has produced three wrong findings already.

| quantity | reference | note |
|---|---|---|
| pill width / screen width | **4.336 %** | the primary size gate |
| pill aspect | **2.643** | |
| centre offset | **0.0 px** | |
| cap / pill height | **0.357** | |
| text width / pill width | **0.486** | |
| subpixel stem / cap | **0.1846** | four frames, two wallpapers |
| text width / cap | **3.600** | |
| pill fill luminance | **0–2** | genuinely black |
| clock colour | **(188,214,255)** `#BCD6FF` | fixed, not wallpaper-derived |

## Six retracted findings — do not resurrect them

1. **"#F2A65A accent"** — hallucinated. The reference accent is periwinkle
   **#79A1F5**; ours is within 3 units/channel. It appears nowhere in any
   reference frame.
2. **"stroke/cap 0.231 vs 0.200 = too heavy"** — modal *integer* stem width is
   quantised. Use subpixel stems (`scripts/fontcheck.py`).
3. **"set collapsed_width=74 bar_height=28"** — derived from a detector factor
   instead of the true scale 1.5; would make the pill 13.5 % too wide.
4. **"lapvar_norm 26 % too sharp"** — video band-limiting. Our render blurred
   0.3–0.5 px reproduces the reference's figure. Do not blur the real desktop
   to match a codec artefact. `lapvar_norm` is a live-vs-live regression
   detector, not a reference target.
5. **"1 Hz grim probe costs 9.75 % of a core"** — stale;
   `src/wallpaper_blur.rs:495` sleeps 60 000 ms.
6. **"no dictation_start literal in the binary"** — LLVM compiles
   `word == "literal"` to inline immediates. Both words work end-to-end.

## Traps that have already burned a reviewer

- **Digit identity swings ink coverage 17 %** (`00:05` 1.346, `05:36` 1.312,
  `05:31` 1.150). Always compare the same string, or average several.
- **The wallpaper may not be running.** If the top band is flat
  RGB(20,20,36) that is the compositor's `root_color`, not the design. Start
  `swaybg -i ~/.config/mango/wallpaper/wallpaper.png -m fill` before judging
  colour or contrast.
- **Two panel instances** can bind the same socket path. `pgrep -x
  dynamite-rust` returning two pids invalidates every measurement.
  `scripts/measure.py` returns no pid in that case; the stress suite reports
  `RESULT INVALID`.
- **A dark app window merges into the pill bbox.** `measure.py` refuses it by
  material (interior p25 > 12) and by the y=0 spanning-window rule. If it says
  `state: "not-collapsed"`, do not quote geometry.
- **Load contamination.** A `cargo build` or an ffmpeg capture running
  alongside will inflate latency and CPU. `docs/scorecard.jsonl` rows carry
  `loadavg`; rows with load > 4 are marked contended and excluded.
- **The reference's expanded panel and its pill share a top edge** (pill rows
  19..60, panel rows 20..219). If you see a separate pill floating above an
  expanded panel, that is our bug, not his design.

## Two different clock materials

- **Island pill clock**: fixed `#BCD6FF`, proven invariant across a blue-sky
  scene (white point 236,242,254) and a warm indoor scene (228,239,215).
- **Lock screen clock**: wallpaper-sampled frosting. Affine over a blurred
  registered wallpaper fits at RGB RMSE 5.28 vs 15.10 for a flat colour.
  `docs/reference/lock-glyph-frosting.png` shows the wallpaper's red bird
  bleeding through the "1" glyph.

Do not unify them.

## Scoring

9+ means "I would ship this". Vague praise is a 6. Every finding needs a
`where` (file:line or a named area). Reply with **only** the JSON object:

```json
{"score":0,"summary":"...","findings":[{"severity":"blocker|major|minor","title":"...","detail":"...","where":"..."}],"what_would_make_it_a_10":"..."}
```