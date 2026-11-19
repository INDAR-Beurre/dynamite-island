# r1c fixer brief — DRAFT (parent, assembling while r1b builds)

Not yet dispatched. Findings below are the ones already **verified in tree by the
parent**, not critic claims. Remaining panel verdicts (#24, #26, #27, #28, #29)
get appended before dispatch.

---

## 1. The headline: one fix buys smoothness AND RAM

`src/main.rs:5267-5268`

```rust
let full = (0, 0, w as i32, h as i32);
let mut rect = if frame_running || self.state == IslandState::Lockscreen { full } else { … }
```

Every intermediate morph frame damages and upscales the **whole 1176×636 stage**.

Three independent measurements agree:

| source | full stage | pill-sized region |
|---|---|---|
| parent `scripts/perf/upscale_bench.rs` (rustc -O) | 62.10 ms | 388 µs |
| profiler table `/tmp/prof7.log` (pre-r1b build) | up = 76.86 ms on a **2.70 ms** draw | up = **0.000** on the one `rect=true` row |
| critic #25, same machine | 31.5–84.3 ms | **0.43 ms** (156× cheaper) |

A 160 ms morph at 62 ms/frame paints **2.6 frames**. Measured: **2–3 distinct
poses**, `pose_hold_median 83.8 ms`, and the animation takes **317 ms** to settle.
That is the owner's "really laggy", with numbers.

**The rect is bounded by construction.** A morph interpolates between two
*known* geometries and `frame.width`/`frame.height` are already in hand, so
`full` is a hardcoded choice, not a requirement. Use `union_rect(from, to)` plus
an AA pad.

> An earlier parent caveat said "do not assume the stage is static mid-morph"
> (from the comment at `src/main.rs:5343`). **That caveat was over-cautious and
> is withdrawn** — it would have blocked the correct fix. The from/to rects are
> known; union them.

**Why this is also a RAM fix — RESIDENCY, not reservation.**
`needed = bw*bh*4` (`src/main.rs:5206`) is 2352×1272×4 = **11,964,672 B per slot**,
and the pool is `needed * SLOT_RING` with `SLOT_RING = 2` (`src/main.rs:514`,
`:5227`) = **23,929,344 B**.

> ⚠️ **Parent self-correction.** An earlier draft of this brief said "shrinking
> the damaged rect shrinks `needed` and therefore the whole pool". **That is
> wrong.** `needed` comes from `bw*bh` — the *surface* size — not from the damage
> rect. A tighter rect **cannot** shrink the reservation. Do not go looking for
> a pool-size change; there isn't one here.

What it does change is **which pages get touched**. Critic #26 measured the first
themes morph raising `Pss_Shmem` **0.3 → 11.688 MB** (shm RSS 23.376 MB, both
slots faulted), total Pss **30.6 → 42.1 MB**, persisting **>3.5 s** after collapse.
A union rect touches only the pill's pages: **~3.0–4.9 MB Pss saved per slot,
~6–10 MB for both**, on menu morphs.

### Open question — resolve with a measurement, do not guess

`src/main.rs:5237-5239` deliberately does `canvas.fill(0)` on the **whole slot at
creation**, commented *"Touch the whole slot once, here, instead of letting a
full-stage draw fault it in later … which reads as a step in RSS."*
That contradicts #26's observation of only **0.6 MB of the 23.9 MB reservation
resident** at idle after unlock. Both facts are recorded; the parent could not
reconcile them by reading alone.

Two things follow, and you should measure rather than assume:
- If creation really faults both slots, the morph is not what makes them
  resident, and the union-rect saving is smaller than #26 estimates.
- The creation-time fill memsets **23.9 MB on every surface-size change** — i.e.
  every lock/unlock cycle — purely to avoid a *cosmetic* step in an RSS graph.
  The comment itself notes fresh memfds are already zero-filled, so the fill may
  be **both redundant and harmful** under a minimal-RAM brief. Verify before
  removing: an undamaged area must stay transparent.

### Fix order (do them in this order, measure after each)

1. **Bound the morph damage rect.** Biggest win, ~160×. Do this first.
2. **Stop zeroing three stage-sized buffers per frame** — `stage_1x.fill(0)`
   (`src/main.rs:5298`), `morph_scratch_from.fill(0)` and
   `morph_scratch_to.fill(0)` (`:5305-5314`), ~9 MB of memset per tick. Clear
   only the rect you will paint.
3. **Adopt the proven hoisted upscale** — `scripts/perf/upscale_bench.rs`
   contains a candidate that is **bit-exact at all 7 tested sizes** and 1.61×
   faster. Bit-exactness is the gate, not speed: the current code rounds the
   horizontal blend to an integer *before* the vertical blend; fusing into one
   16-weight sum shifts output ±1. Reproduce with
   `rustc -O -o /tmp/upscale_bench scripts/perf/upscale_bench.rs && /tmp/upscale_bench`.

**Keep the frame-callback back-pressure exactly as it is.** It works: 400 offered
packets → **400 accepted, 0 blocked, CPU 57.7%** (was 64 accepted, 51 blocked,
103.0%). Idle CPU is now **0.67%**. Do not "simplify" it away.

### Testable prediction — this is the acceptance gate

62 ms → sub-5 ms per morph frame ⇒ **≥8 distinct poses** over a 160 ms morph at
60 Hz, `pose_hold_median ≤ 24 ms`, settle ≤ 260 ms.
Run `./scripts/motion/run_all.sh /tmp/motion-r1c 3`. Five further gates are
currently UNRESOLVED because `in_flight_samples=0`; **they should resolve once
the morph actually paints** — that is itself evidence.

---

## 1b. The real RAM headline: the lock screen doubles the pool

> ⚠️ **Parent self-correction #2.** The log currently says *"RAM REGRESSED
> 44.2 → 55.4 MB past baseline"*. **Retract that.** It was measured with both shm
> slots faulted after morph activity — a transient, not steady state. Critic #26
> measured **idle after unlock: VmRSS 34.1 MB, Pss 30.6 MB**, and found **no
> leak**: 12 lock cycles moved collapsed Pss **30.45 → 30.59 MB**; 20 theme
> cycles **41.97 → 41.83 MB**. Steady state is **better** than the 44.2 MB
> baseline. Same error class as quoting raw lapvar without normalising contrast.

The genuine peak is the **lock screen**:

| | normal | locked |
|---|---|---|
| shm pool | 23,376 kB | **47,096 kB** (2 × 24,112,128 B = **48,224,256 B**) |
| Pss_Anon | 26.9 MB | **32.9 MB** (+6,028,008 B lock wallpaper) |
| VmRSS | 34.1 MB | **86,616 kB** |
| Pss | 30.6 MB | **59,917 kB** |
| VmHWM | — | **103,696 kB** |

It **fully recovers** on unlock (33.6 MB RSS / 30.3 MB Pss), so this is a peak
problem, not a leak — but a **2× steady-state peak** on a machine where the owner
asked for minimal RAM. Sites: `src/main.rs:1943` (`LOCKSCREEN_SIZE = (1504, 1002)`),
`:5206`, `:5227`, `:4540`.

Fix shape: reuse the stage buffers for the lock surface instead of allocating a
second 48 MB pool, or render the lock at the stage size and scale. **Do not
reduce `SLOT_RING` to 1 without proving the compositor releases the buffer** —
the comment at `src/main.rs:5207-5210` says `pool.canvas(&slot)` returning `None`
for the in-flight slot *is* the back-pressure, and with one slot that stalls
every frame.

## 2. Tighten the guard that let this ship

- `src/main.rs:521` doc claims *"a 1176x636 stage costs ~5 ms, see the iter108 perf test"*.
- `src/main.rs:8412` that test asserts `best.as_millis() < 200`.

**The guard is 40× looser than the claim**, so a doubling ships green. Fix the
comment and the bound **together**, to the measured post-fix number.

---

## 3. Per-frame allocation (owner asked for minimal RAM)

| where | what | fix |
|---|---|---|
| `src/tray.rs:116` | `snapshot()` = `read().unwrap().items.clone()`, called per settled frame at `src/tray.rs:1316`. `TrayItemSnapshot` = 6 Strings + 1 KB icon + 4 KB icon32 ⇒ **~15 KB + 18 Strings cloned and dropped per frame, read-only** | `with_items(\|items\| …)` guard scope, or `Arc<TrayItemSnapshot>` |
| `src/main.rs:305` | `cfg()` clones the `wallpaper` + `theme` Strings on every call; `size()` calls it 2–3×; 15 geometry sites | a `Copy` metrics snapshot for the arithmetic |
| `src/main.rs:2489`, `:2493` | `&hhmm()` (`format!` + `localtime_r`) and `(i+1).to_string()` per disc, every settled frame | stack buffer + `write!`, or reuse the existing minute cache |
| `src/wallpaper_blur.rs:404-421` | `ensure_loaded()` stat + mutex + Arc clone at **three** sites (`src/main.rs:2154`, `:2431`, `:3426`) — the third is the morph path | cache the mtime check behind a short interval |

---

## 4. Cheap correctness/idiom

- **36 ungated `DBG` sites** in `src/main.rs` + `src/music_panel.rs:1170-1174`.
  The Motion arm prints **per input sample** during a morph
  (`src/main.rs:6045-6050`) — an unbuffered `write(2)` per sample on the render
  thread. One `static DBG: OnceLock<bool>` from an env var gates them all.
  **Keep the `DBG morph A -> B` lines** — the stress harness and the motion
  harness both time transitions from them. That is a contract
  (`docs/socket-contract.md`).
- Three `horizontal_line_metrics().unwrap()` holdouts —
  `src/main.rs:2011`, `src/controlcenter_panel.rs:2547`, `src/lockscreen.rs:673` —
  while ten siblings already use the degrading form. Copy the neighbour at
  `src/main.rs:2525`: `let Some(lm) = font.horizontal_line_metrics(px) else { return };`.
- Lock-poison `.unwrap()`s are **unreachable under `panic="abort"`**. They are an
  idiom inconsistency, **not a panic path** — do not treat them as one. They do
  bite under `cargo test` (unwind). Unify on the recovering form if cheap.

---

## 5. Geometry — set, then MEASURE. Do not compute.

Config `~/.config/dynamite-rust/config.conf` is currently
`collapsed_width=67 bar_height=25 gap_top=13` → live pill **96×38**.

The screen scale is **1.5 on both axes**. Reference pill is **111×42 device px**.
111/1.5 = **74**, 42/1.5 = **28**.

> Set **`collapsed_width=74 bar_height=28`**, then run
> `python3 scripts/measure.py` and report the numbers.
> An earlier parent target (`66×25`) was derived from a *detector-derived*
> factor 1.473 and was wrong — it lost 2 px to the stadium's rounded caps.
> **Never derive config from a detector-derived factor; use the known 1.5.**

Reference targets to converge on: aspect **2.643**, `cap_over_pill_h` **0.357**,
`stroke_over_cap` **0.200**, `text_over_pill_w` **0.486**, margins **28/29**,
fill luminance **0–2**.

**Current sharpness has OVERSHOT**: `stroke_over_cap` **0.231** vs 0.200 (15.5%
too heavy), `lapvar_norm` **1665.74** vs 1277.0 (30.4% high). At baseline it was
0.154 — *under*. The weight fix went past the target. Ease it back.

> **Quote `lapvar_norm`, never raw `lapvar`.** Raw lapvar scales with the square
> of contrast; our ink clips at 255 while the video reference peaks 225.7. The
> normalised pair is **1277.0 (ref) vs 1872.73 (live)** = 1.47×, not the
> misleading 1.86× the raw numbers suggest.

---

## 6. Only the settled pill got the 2× path

`draw_collapsed_pill_2x` runs only under
`if self.morph.is_none() && state == Collapsed` (`src/main.rs:4883-4885`).
Two 1× paths draw the same clock: the `Collapsed` branch of `draw_state_into`
(`src/main.rs:2947-2961`) and `draw_pill_over_stage` (`src/main.rs:2276`, called
at `:2357`, `:2363`, `:3223`). Measured: **lapvar 12100 → 2087.5 with a menu
open** — a 5.8× crispness collapse on every expand. The discs and tray are still
1×-bilinear-doubled (pill edge 2 device px vs disc edge 4–5 px).
**One fix, not two findings**: route every state through the same 2× path.

---

## 7. Colour — one token, and DO NOT change the hue

⛔ **A previous critic's "the accent should be orange `#F2A65A`" finding is
RETRACTED — it was a hallucination.** The parent measured the reference active
disc: **`#79A1F5` (121,161,245), periwinkle blue**. `#F2A65A` appears nowhere in
the repo or the reference except that critic's own text. Obeying it would have
destroyed the pixel match the owner asked for.

The real defects:

- One hue hand-typed in ≥4 places while `library::ACCENT` / `theme_accent()`
  drive none of them, so **theming cannot work**:
  `#7CA2F5` (`src/library.rs:36`, `src/calendar_panel.rs:187`),
  `#93A4F5` (`src/main.rs:927`, `src/controlcenter_panel.rs:362`, `src/music_panel.rs:288`),
  `#74A3F6` (`src/controlcenter_panel.rs:366`),
  `#7AA2F7` (`src/clipboard_panel.rs:116`).
- **`#93A4F5` (147,164,245) is 26 units too light on red** vs the measured
  `#79A1F5`. Converge on the measured value. **Keep the hue family.**
- `src/calendar_panel.rs:187` `#7CA2F5` is within **3 units/channel** of the
  reference and is already correct — leave it.
- Discs cast a shadow the shadowless pill does not have; `RIM_LIGHT` is
  measurably invisible.
- `WS_CY`/`TRAY_CY` derive from `STAGE_PAD_TOP + 12.0` while the collapsed height
  comes from config, so the discs/tray ride a few device px off the pill's
  centreline. Share one `COLLAPSED_H`.

**The dictation waveform is invented.** `src/dictation_canvas.rs:126-132,
:400-588` draws a teal bloom (`GLOW_K 3.2`, `GLOW_REACH 22`, `BLOOM 0.32`,
`GLOW_IDLE 0.18`) and the crate's only multi-stop gradient
(`GRAD_L(125,165,165) → GRAD_C(140,220,220) → GRAD_R(80,118,178)`), with a live
fill of (20,24,34) against the pill's (2,2,2) and a ~20 px aura.
**The reference has no waveform, no glow and no gradient at all.** This is the
largest remaining AI-slop surface. Keep the *function* (the owner uses dictation);
remove the invented decoration and match the pill's material.

---

## 8. Expanded panels — measured reference, first time specified

At 2560×1440, all centred on x=**1279.5** = exact screen centre, same as the pill.
Panel fill p10/p25/p50 = **0.0**, pure black, same material as the pill.

| panel | w×h | aspect |
|---|---|---|
| power/control | **538×201** | 2.677 |
| themes (search) | 798×489 | 1.632 |
| launcher | 676×489 | 1.382 |
| wallpapers | 1451×143 | 10.147 |

Power panel internals: **5 discs, 54 px diameter** (active *and* inactive),
**disc/panel-width = 0.1004**, **pitch ≈ 82 px**, inactive fill **(25,26,31)**
(very dark grey, not mid grey), uptime cap **10 px**, active label cap **11 px**
at peak luminance **222** (not 255). Layout: uptime top-centre, disc row centre,
active label below. **No title bar, no separators, no card-in-card, no border.**

---

## 9. Lock screen — read the CORRECTED spec first

Read `docs/lockscreen-spec.md` and `docs/lockscreen-transcript-facts.md`
**before writing any lock-screen code**.

⚠️ **`/tmp/ref/lock/l60.png` and `l320.png` are the OLD macOS-style comparison
image the creator shows to explain what he is NOT building.** They are not
states of the final design. An earlier parent reading took them for the real
thing and nearly specified an avatar + username + bottom password box that
**does not exist**.

- resting = `l40.png`, `l400.png` — one giant centred clock, nothing else
- typing = `l120.png` — password dots **inside the top capsule**
- `l3/l20/l90/l160/l200/l260` = the unlocked desktop

Measured: clock ink **868×279** centred **x=1280.5** (screen centre 1280) =
**33.9% of screen width**; resting capsule **68×40 at y=58**; dots **8–9 px dia,
~16.2 px pitch**.

⚠️ **Measure the clock by COLOUR, never by luminance.** `l40.png`'s wallpaper has
a cream mast and an orange buoy inside the clock's row band, so `lum>110` returns
**2057×340 (80% of width)** — pure contamination. Use the glyph-teal mask
`(G>170)&(B>150)&(R<200)&(G-R>40)`, restricted to rows 560–880.

### 9-pre. Clock width — the corrected, solved finding

A critic reported "clock ~16% too wide". It compared an **advance box** to an
**ink** measurement. Re-measured ink-vs-ink through a faithful reimplementation
of `rasterize_clock`'s tabular layout (`src/lockscreen.rs:673-700`):

| | reference | ours (px=388.8) |
|---|---|---|
| ink size | **868×279** | **963.6×280** |
| width % of 2560 | 33.9% | 37.6% |

- **Height is already correct** (+0.4%). **Do NOT change `CLOCK_PX_FRAC`.**
- Width is **+11.0%**, not +16%.
- **Solved minimal fix**: `DIGIT_ADVANCE_EM 0.60 → 0.522` and
  `COLON_ADVANCE_EM 0.21 → 0.198` (`src/lockscreen.rs:190,192`) → ink **867.9**,
  error **−0.01%**. Two constants, no layout rewrite.
- Natural (non-tabular) advances give 824 = −5.1%, so the tabular cells are what
  push it wide.

**Deeper, and a deliberate trade-off**: the reference's centre-to-centre glyph
pitches are **123.0, 138.5, 166.5, 219.5 px** — *non-uniform*, i.e. proportional
spacing (his `1` is far narrower than his `8`). Tabular is uniform by definition
(ours: 233.3 px every digit), so **no single advance value can match those
pitches**. Keep tabular anyway: proportional would let the centred block shift
as digits change (`11:28`→`11:29`). Adopt 0.522/0.198, accept the second-order
letterform gap. **SF Compact Display is absent from this machine** (`fc-list`
finds no SF face) and cannot be shipped — do not write a loader for it; a silent
fallback would make the finding look fixed while changing nothing.

**The clock is wallpaper-sampled frosting INSIDE the glyphs** — median ink RGB
**[87,235,206]**, 92.8% teal-ish, and it *varies across x* because each glyph
samples the wallpaper beneath it. Recipe: **glyph-only Gaussian blur ~24 px**,
**1.030× centred wallpaper zoom**, **vertical (not radial) black veil**.

> `src/lockscreen.rs:199` `C_CYAN = (170,249,254)` = `#AAF9FE` is **pale, not
> saturated** — the earlier "lone saturated cyan" slop finding was wrong and is
> corrected. The defect is the **material**, and the fix is **not white**.
> **Any flat fill is wrong. A frosted rectangular card would be wrong AND an
> AI-slop tell.**

Creator's spoken facts (these outrank pixel guesses): font **SF Compact
Display**; wrong password **shakes**; the password field appears only when
typing starts and disappears on Esc; **the island morphs into the keyhole and
descends**, then the clock pops out.

### 9a. ⛔ It does not lock — and that is a decision, not an oversight

Verified in three places: `src/main.rs:9` *"a visual harness, NOT a session lock
(no ext-session-lock surface, no PAM, no keyboard input yet)"*;
`src/main.rs:2320` same; `src/lockscreen.rs:8` *"This module is draw-only"*.
`grep -rn 'pam\|PAM' src/*.rs Cargo.toml` → **only comments**.

**Do NOT build a PAM/session-lock stack this round.** The owner asked for a
*replica*, not an auth system; that subsystem carries lockout failure modes and
is out of scope for tonight. Instead:

1. **Make it honest.** Nothing in the UI may imply the session is secured. A
   user who believes it locks and walks away has an unlocked machine.
2. **Document the gap** in the repo README: what it is (a visual preview), what
   a real lock needs (`ext-session-lock-v1`, which MangoWM implements per
   `docs/mango-capabilities.md`, plus PAM and a keyboard grab), and that it is
   not implemented.
3. **Never enable lock-at-boot.** The creator warns that a failed lock client
   leaves an **untypeable locked session**. Test the unlock path first.

### 9b. The veil: keep the measured values, do NOT adopt his dim number

His `dim_strength = 0.12` (band 0.12–0.15; he criticises the 0.5 default) dims
**unfocused clients relative to a full-brightness focused surface**,
compositor-side. Our veil is an **absolute alpha over a static wallpaper with no
focused window** — a different quantity, **not substitutable**.

**Keep `SCRIM_CENTRE 0.22` / `SCRIM_EDGE 0.42`** (`src/lockscreen.rs:205-206`),
which carry an iter13 patch-averaged re-measurement (corners mean ratio 0.58 ⇒
alpha 0.42, five points 0.39–0.49). Adopting 0.12 would halve the veil and
*destroy* the pixel match in the name of following his advice.

What his number *does* authorize: a **gentle, still-legible** dim is deliberate,
and 0.5-heaviness is the known mistake. **His acceptance test is legibility, not
a number** — dimmed content must still read.

> Note the spec says **vertical** veil while the constant names say
> centre/corners (radial). Reconcile against `docs/lockscreen-spec.md` and the
> frames; do not assume the comment is right just because it is old.

---

## 9c. Two verified defects found while mining the transcripts

**Frost origin is computed from a hardcoded screen width.**
`stage_screen_origin()` (`src/main.rs:888-891`) returns
`(LOCKSCREEN_SIZE.0 as f32 - STAGE_W as f32) / 2.0` — from the **constant 1504**,
not the live output size. The pill *looks* centred because the compositor centres
the surface (`src/main.rs:6331` `layer.set_anchor(Anchor::TOP)` is **correct** and
matches his stated mechanism: *"since I didn't pick a side, it just sits in the
middle"*). But every **frost/wallpaper sample** taken through this origin is
offset on any other output or scale — the 246-device-px mis-sample critic #5
predicted. **Fix: read the output's logical size.**

**Exclusive zone is already right — do not "fix" it.** We reserve
`pill_strip_px()` (`src/main.rs:6333`), not the 636-tall stage. Reserving the
stage would eat 63% of a 1002 px desktop. His rule confirms it: *"your whole
screen minus your bar and your gaps."*

**Open question, NOT a finding:** his bar may be on layer `top` (*"you can go
through full screen over your bar"*) where we are `overlay`. The worker labelled
this **inference** and did not check frames. Do not change the layer on an
inference — it would alter fullscreen behaviour on a guess.

---

## 9d. His stated taste rules — use these to decide the decoration questions

From `docs/transcript-taste.md` (53 quotes machine-checked verbatim):

- **Contrast is a budget.** *"contrast is basically deciding where people look
  first"*; blur a screenshot — *"whatever that you can still see in the blurry
  version is what your desktop is shouting about."*
  **This is a test you can actually run**: blur a capture of our panel and check
  that only the clock and the active workspace survive. If the dictation bloom
  survives, it is stealing the budget.
- **One accent.** *"your accent and you only get one"*, *"the only saturated
  color in the whole palette"*, reserved for *"you are here"*. Base/panels/
  inactive borders *"really close to each other"* — *"this layer's job is to be
  quiet"*. Set it once: *"all of it pulls from that same palette."*
  → **Directly condemns** the four-file accent duplication and the saturated
  teal bloom.
- Clock *"bigger and bolder than everything else in the bar"*; other data
  *"smaller and dimmer"*.
- **Single island**: *"why not have everything pop up out of one single island"*;
  *"The button itself actually opens up into the menu"*.
- *"just make your panel solid… the wallpaper stays behind everything."*

**He says NOTHING about glow, bloom, UI gradients, shadows, rim-light, waveforms
or animation numbers.** Those remain *our* judgement — do not cite him as
authority for removing them. The measured reference (no waveform, no glow, no
gradient) is the authority there.

---

## 10. Dictation — already embedded, two things left

`tools/dictation/` is in the package (commit `5bb1fd5`), deliberately a plain
subdirectory **not** a Cargo workspace: the panel uses smithay-client-toolkit
0.19, dictation 0.21.1 + cpal + reqwest/rustls. A shared workspace would unify
the lockfile onto a working binary. **Do not merge them.**

- **Always-on mic**: `tools/dictation/src/main.rs:90` `capture::start().expect(...)`
  and `tools/dictation/src/capture.rs:88-103` park forever ⇒ **Pss 1794 kB idle,
  28 threads, ~99 ctx-switches/s** with a live `alsa_capture` node while nobody
  is dictating. Start on `HotkeyEvent`, drop on `HoldEnd`.
- **Dead code**: `tools/dictation/src/main.rs:67` `let pill: Option<pill::PillHandle> = None;`
  and `try_spawn()` is `#[cfg(test)]`-only, so the binary **never sends
  `collapsed`** (`grep -ac collapsed …/static-dictation` → 0). Either wire it or
  delete it; do not leave a contract literal that is never sent.

The socket contract is frozen in `docs/socket-contract.md` —
`dictation_start`, `dictation_end`, `dictation_bars:<v> …`, `collapsed`.
`dictation_bars` arrives at **microphone** rate, not display rate: coalesce to
the frame callback, never drop the word.

---

## 11. Verification required before you report

1. `cargo build --release --offline` — 0 errors.
2. `python3 scripts/stress/stress.py` — **all 5 groups, ≥1000 tests**. A run is
   **INVALID** (not failed, not passed) if `mmsg get all-layers` lists no
   dynamite surface, or if the machine is under heavy load — the latency group
   already produced a false FAIL at loadavg 7.49. Re-run in a quiet window;
   **do not relax a bound to make it pass.**
3. `./scripts/motion/run_all.sh /tmp/motion-r1c 3` — report `morph_poses`,
   `pose_hold_median_ms`, `settle_ms`, and whether the five UNRESOLVED gates
   resolved.
4. `python3 scripts/measure.py` on a **quiet screen, `collapsed`, bright
   wallpaper** — if it says `state:"not-collapsed"`, do not quote its geometry.
5. `python3 scripts/scorecard.py` — append the row so the trend is visible.
6. Pss **after a lock/unlock cycle**, not on a virgin process.

Report what you changed per finding, what you left open and why, and the exact
commands you ran with their results.
---

## 0. ⛔ START HERE — the product does not start on another machine

**This outranks every visual finding below.** Reproduced by the parent:

```
HOME=/tmp/emptyhome2 ./target/release/dynamite-rust
thread 'main' panicked at src/main.rs:6299:10:
no Quicksand font file parses
```

The **entire UI font** resolves from `$HOME/.local/share/fonts/{Quicksand-Bold.ttf,
Quicksand.ttf}` only, then `.expect(...)`. It dies **before reaching Wayland**.
`git ls-files | grep -ci quick` was **0**.

**The parent has already vendored the fonts** — `assets/fonts/Quicksand-Bold.ttf`
(78,528 B, static, usWeightClass 700) and `assets/fonts/Quicksand.ttf` (124,824 B,
variable, wght 300), with `assets/fonts/OFL-Quicksand.txt` (SIL OFL 1.1,
redistributable). Commit `263b807`. **The loader change is yours:**

1. Resolve **exe-relative first**, exactly like `clock_font()` already does
   (`src/main.rs:2029-2063`) — `../../assets/fonts`, `assets/fonts`, then `$HOME`.
2. **`include_bytes!` the Bold as the final candidate** so no filesystem font is
   ever required. 78 KB in a 3 MB binary is nothing.
3. **Drop the `.expect`.** Under `panic="abort"` this kills the process.
4. **Log which candidate won**, once. Today "repo Bold", "`$HOME` Light (wght
   300!)" and "fell back" are indistinguishable — and the Light fallback silently
   changes every panel's text metrics.
5. Delete the absolute `/home/alex/...` candidate at `src/main.rs:2042`.

Verify with the exact command above: it must reach Wayland, not panic.

## 0b. The packaging gap — ✅ MOSTLY CLOSED BY THE PARENT, do not redo

The parent took every part of #29 that touches no `src/`. **These are DONE and
committed; verify them, do not reimplement:**

- ✅ `aa2569d` root **README.md** (prereqs incl. `libasound`/`libxkbcommon` as hard
  link-time deps, a 14-tool table saying what each buys and what breaks without
  it, all 13 config keys, and an explicit **lock-screen-is-not-a-security-boundary**
  section) + **`build.sh`** building both crates with a pkg-config preflight.
- ✅ `e971ce8` supervisor `LEGACY` scratch fallback **deleted** — missing build is
  now loud stderr + exit 1 (proven). Also fixed a bug the test exposed: no
  `mkdir -p` for the log dir meant a fresh `$HOME` made the privacy `chmod 600`
  fail silently. `--offline` removed from the documented commands.
- ✅ `e971ce8` `layer_name:dynamite` → `dynamite-rust` in
  `docs/mango-capabilities.md` **and** `~/.config/mango/rule.conf` (backup kept).
- ✅ `a0eb900` **`deploy/`** ships `rule.conf` + `dynamite_autostart.sh` deriving
  paths from `ISLAND_HOME`/`XDG_RUNTIME_DIR`, no `qs -c dynamite` substitution.
- ✅ `70f15ae` git hygiene: untracked the `.pyc`, the `.bak`, 17 `motion/out/`
  files, and deleted the dead root `wallpaper_blur.rs`; `.gitignore` extended.

**Still yours (needs `src/`):**

- **The startup panic in §0** — the assets are vendored (`263b807`) but the
  loader still only reads `$HOME`. This is the blocker.
- **`STATUS.md:1-6`** still claims *"v0.1 prototype: collapsed clock pill only"*.
  Correct it to match reality.
- **14 external tools, no capability check.** Every spawn is `.spawn().ok()?`, so
  a missing `bluetoothctl` renders a **blank BT row indistinguishable from "no
  devices"**. Add a one-shot startup line naming what is missing — `which` in a
  loop, no new dependency. The README already lists them; keep the two in sync.
- **Cold offline build** still impossible for `tools/dictation` (no vendor dir).
  Optional: `cargo vendor` the direct deps the way `png` is already vendored.
  The docs no longer claim otherwise, so this is a nice-to-have, not a lie.
- **Output geometry is a compile-time constant.** `LOCKSCREEN_SIZE (1504,1002)`
  and the guard `configured_size.0 >= 1400` (`src/main.rs:4597,4611`) mean **the
  lock animation never draws on an output narrower than 1400 logical px**. Read
  `OutputState::info().logical_size` once. This is the same root cause as the
  frost-origin bug in §9c — **fix it once, in one place.**
