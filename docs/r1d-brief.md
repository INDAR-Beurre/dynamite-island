# Fixer r1d brief — verified findings only

Every number here was measured by the parent or by a critic and then
re-checked by the parent. Retracted claims are listed at the bottom so you
do not "fix" something that is already correct.

---

## 0. BLOCKER — the pill must GROW INTO the panel, not sit above it

This is the largest structural difference from the reference and it explains
why the expanded states look wrong even though the collapsed pill matches.

**Reference, measured two independent ways:**

`docs/reference/pill-settled.png` pill rows **19..60**.
`docs/reference/panel-power.png` panel rows **20..219**.
**Same top edge.**

60 fps morph frames (`/tmp/ref60/f000168.png` … `f000182.png`), dark bbox:

| frame | y-range | h | w |
|---|---|---|---|
| 168 | 19..60 | 42 | 111 |
| 171 | 19..82 | 64 | 220 |
| 174 | 20..158 | 139 | 590 |
| 177 | 20..183 | 164 | 709 |
| 180 | 20..205 | 186 | 817 |
| 182 | 20..207 | 188 | 825 |

**The top edge is pinned at y=19–20 for all 14 frames while the height grows
42 → 188.** One capsule, anchored at the top, growing downward and outward.

**Ours:** `src/main.rs:449-460` — `panel_top_menu() = STAGE_PAD_TOP + 28 + 2 = 33`
logical, with the comment *"the expanded menus hang BELOW the pill … so the
pill stays visible above the panel with a 4 px gap"*. That is a different
design. The creator's own words: *"The button itself actually opens up into
the menu"*, *"why not have everything pop up out of one single island"*.

**Fix:** the expanded panel's top edge is the pill's top edge (`gap_top`).
There is no separate visible pill above an expanded panel. Keep the stage
slack for shadow tails, but the panel's own y-origin moves to the pill's.

---

## 1. BLOCKER — every expanded panel is 18–28 % too wide

Critic #32 measured live captures; parent re-derived from `IslandState::size()`
(`src/main.rs:200-214`) and they agree. Compare **width as a fraction of screen
width**, never absolute pixels — his screen is 2560 device, ours 2256.

| panel | ours logical | ours device | ours %W | ref %W | ratio | our aspect | ref aspect |
|---|---|---|---|---|---|---|---|
| Power | 400×150 | 600 | 26.596 | 21.016 | **1.266** | 2.667 | 2.677 |
| Themes | 600×366 | 900 | 39.894 | 31.172 | **1.280** | 1.639 | 1.632 |
| Launcher | 504×366 | 756 | 33.511 | 26.406 | **1.269** | 1.377 | 1.382 |
| Wallpaper | 1008×270 | 1512 | 67.021 | 56.680 | **1.182** | 3.733 | **10.147** |

Aspects match within 0.5 % for three of four — so this is a **systematic width
error**, not a scale artefact. The wallpaper panel is also **3.2× too deep**.

**Target logical sizes** (reference width fraction × our screen ÷ 1.5, height
from the reference aspect):

```
Power      316 x 118     (now 400 x 150)
Themes     469 x 287     (now 600 x 366)
Launcher   397 x 287     (now 504 x 366)
Wallpaper  853 x  84     (now 1008 x 270)
```

The wallpaper panel at 84 logical px tall is a **single row of thumbnails**,
not a grid. Check `src/wallpaper_panel.rs:150-151` and its layout before
assuming the current grid survives the change.

**Do not shrink text to fit.** Scale the layout; if content no longer fits,
that is a finding to report, not a reason to keep the old width.

---

## 2. BLOCKER — the morph presents 2–3 poses where the reference shows 14

Full measurement in **`docs/reference-motion-60fps.md`**. Summary:

- Reference expand = **14 distinct poses in 233 ms, one per displayed frame**.
- Width and height share **one** progress value (columns agree within 0.0082).
- Easing MSE over the 14 normalised samples:
  **easeOutQuad 0.00259** · easeOutCubic 0.01201 · easeOutQuart 0.02802 ·
  linear 0.03509 · easeOutQuint 0.04391 · easeInOutCubic 0.04957.
  **easeOutQuad fits 4.6× better than our current easeOutCubic.**
- Settle overshoot **+1.1 %** decaying over ~9 frames (825×188 → 816×186).

**Already landed and verified — do not undo:** the bounded morph damage rect
(`src/main.rs:5337`, `union_rect(content_rect(from), content_rect(to))`) and
`upscale_canvas_2x_region`. Parent proved the region upscale **bit-exact at
847 rect cases across 8 sizes** (`scripts/perf/region_exactness.rs`) and timed
every realistic morph rect at **< 3.1 ms** (`scripts/perf/morph_rect_bench.rs`).

**Still to do:**
1. Switch the morph easing to **easeOutQuad** (`src/morph.rs:123,148,300,419,451`).
   Add the variant; keep `EaseOutCubic` for any caller that needs it. Update
   `progress_inverse` (`src/morph.rs:406-419`) — it must stay exact.
2. Add the **~1 % settle overshoot** the reference has. Not OutBack 1.15 (that
   rendered as a one-frame black slab); a small decaying tail after the main
   growth. If you add overshoot, **the damage rect must cover it** —
   `src/morph.rs:143` warns callers must not assume `<= 1.0`.
3. Verify with `bash scripts/motion/run_all.sh /tmp/motion-r1d 3`. The gate
   wants **≥8 poses** and **pose-hold median ≤24 ms**. Report the actual
   numbers, not "should be smoother".

**Constraint:** the rect-safety proof depends on easing being monotone on
[0,1]. easeOutQuad is. An overshoot tail is not — size the rect for it.

---

## 3. MAJOR — panel fill is near-black, reference is pure black

Critic #32 measured our power panel interior p10/p25/p50 = **3/4/5**
(mean 4.66); reference **0/0/0**. Sites: `src/main.rs:776-777,814,858-862`.
The collapsed pill already measures fill 5.0 against the reference's 0–2 —
same defect, same fix. Make the material genuinely black.

---

## 4. MAJOR — power panel palette and label brightness

Measured by #32, confirmed against the reference frame:

| | ours | reference |
|---|---|---|
| active disc | (124,162,245) | **(121,161,245)** |
| inactive disc | (28,30,34) | **(25,26,31)** |
| label peak luminance | 251 | **~222** |

`src/library.rs:36,48`, `src/power_panel.rs:81,84-86,495`.

**The hue family is correct — do not change it.** Only these small offsets.

---

## 5. MINOR — power disc pitch is non-uniform

Ours: centres 938.5, 1028.5, 1127.5, 1226.5, 1316.5 → pitch **90, 99, 99, 90**,
diameter 62. Reference: pitch ~82 uniform, diameter 54.
Relative to panel width ours averages 0.1575 vs reference 0.1533.
`src/power_panel.rs:91-98`. Make the pitch uniform at **0.153 × panel width**.

---

## 6. MAJOR — the microphone is open from login and can never close

Critic #31, verified live: PipeWire node `alsa_capture.static-dictation` in
state R, `pcm0c` RUNNING, `appl_ptr` advancing ~48k frames/s.

- `tools/dictation/src/main.rs:90` — `capture::start().expect(...)` at launch.
- `tools/dictation/src/capture.rs:57-103` — the stream's owner thread parks
  forever; **no Drop, no stop channel, no JoinHandle**, `AudioIn` is Clone-only
  holding a Receiver. Nothing can release the device.
- `tools/dictation/src/main.rs:108-118` — the drain and 30 ms frame slicing run
  unconditionally; `if recording` is INSIDE the loop. Idle audio is converted,
  resampled and sliced at full rate then thrown away. That is the **96–99
  context-switches/s** (the panel does 1/s).
- **The real cost is battery, not RAM**: with `power_save=1`,
  `power_save_controller=Y` and playback closed,
  `/sys/bus/pci/devices/0000:00:1f.3/power/runtime_status` stays `active`.
  The open capture is the only thing keeping the HDA codec awake.

**Fix shape** (~50–60 lines, #31's estimate): command channel instead of
`park()` forever; a Drop/stop path so the stream is actually released; move
`capture::start()` into `HotkeyEvent::HoldStart`; turn the `Err` branch into a
pill message instead of `.expect()` (under the supervisor's 1 s restart an
expect becomes a crash loop). Skip the drain entirely when not recording.

**Measure before claiming a win:** `build_input_stream + play()` latency on a
free device. If it exceeds ~50 ms, keep a warm-but-idle path and say so.
#31 could not measure it because the device was busy — do not invent a number.

---

## 7. MINOR — the running supervisor is still the old scratch script

The repo script (`tools/dictation/scripts/dictation_supervisor.sh`, umask 077,
chmod 600, 1 MiB rotation) is correct but **is not the one executing**. The
live supervisor is the scratch copy with umask 0022 and no rotation; the log
is 600 today only because it was chmod'ed by hand. Restarting it from
`~/.config/mango/autostart.sh` (already repointed) picks up the repo script.
Do not kill the owner's running dictation mid-session — note it for next login.

---

## 8. MINOR — dictation holds 33 inherited T3/Electron fds

102 open fds, 33 pointing at `v8_context_snapshot.bin`, `app.asar`, deleted
leveldb inodes and 4 chromium shm segments, because the supervisor was spawned
from the T3 app. Not dictation's code, but it keeps dead inodes alive all
session. `tools/dictation/scripts/dictation_supervisor.sh:38` — close inherited
fds before exec.

---

## Do NOT touch

- The frame-callback back-pressure (`src/main.rs:5108-5114`, `:5376`, `:5417-5421`).
  Verified: 400 packets offered → **400 accepted, 0 blocked, CPU 57.7 %**
  (was 64 accepted, 51 blocked, 103.0 %).
- The socket literals `dictation_start`, `dictation_end`, `dictation_bars:`,
  `collapsed`, and the `DBG morph A -> B` stderr line (harness contract).
- The single-instance guard and its **exit 75** (`src/main.rs:6794-6798`).
  Exiting 0 drove the supervisor to its 10-strike give-up cap.
- `collapsed_width=67 bar_height=25 gap_top=13` in the live config. The
  collapsed pill measures **4.344 % of screen width vs the reference's
  4.336 %** — it is correct. Two earlier "fixes" to these values were wrong.
- The font: `assets/fonts/Inter-Medium.ttf` (wght 550, opsz 20, fvar removed)
  is verified correct by three independent methods.
- `scripts/` — the parent owns the instruments.
- PAM / ext-session-lock. The lock screen is a **visual replica**, explicitly
  documented as not a security boundary. Do not enable lock-at-boot.

---

## Retracted — do not "fix" these

1. **"#F2A65A accent"** — hallucination. The reference accent is
   **#79A1F5 periwinkle**; ours is within 3 units/channel. Obeying this would
   have destroyed the pixel match.
2. **"stroke/cap 0.231 vs 0.200 = too heavy"** — artefact. Stem = ratio × cap:
   reference 0.200×15 = **3.00 px**, ours 0.231×13 = **3.00 px**. Identical ink.
3. **"set collapsed_width=74 bar_height=28"** — derived from a detector factor
   instead of the true scale 1.5. Would have made the pill 13.5 % too wide.
4. **"lapvar_norm 26 % too sharp"** — video band-limiting. Same-string
   cap-normalised ink area ratio is **1.045**. Blurring our output to match a
   codec artefact would make the real desktop worse.
5. **"1 Hz grim probe costs 9.75 % of a core"** — stale.
   `src/wallpaper_blur.rs:495` sleeps **60_000 ms**.
6. **"no dictation_start literal in the binary"** — LLVM compiles
   `word == "literal"` to inline immediates. Both words work end-to-end.

---

## Required verification before you report

```
cargo build --release --offline          # must be 0 errors
cargo test  --release --offline          # 308 pass today; keep them passing
cargo clippy --all-targets --offline -- -D warnings   # report the count
python3 scripts/stress/stress.py         # >=1000 asserts, RESULT PASS
bash scripts/motion/run_all.sh /tmp/motion-r1d 3
python3 scripts/measure.py               # collapsed pill, quiet screen
```

Report **actual numbers** for each, and say explicitly what you did not verify.
---

## 9. BLOCKER (visual) — the lock screen material is wrong in three ways

`docs/lockscreen-spec.md` is the authority; it was written from the reference
pixels by a vision-capable reviewer and every number below was re-checked by
the parent. **Read that file before touching `src/lockscreen.rs`.**

**Important:** `/tmp/ref/lock/l60.png` and `l320.png` are the OLD macOS-style
**comparison image** the author shows to explain what he is NOT building.
The final design is `l40`/`l400` (resting) and `l120` (typing). Copies are in
`docs/reference/lock-resting.png` and `docs/reference/lock-typing.png`.

### 9a. The veil is VERTICAL, not radial
`src/lockscreen.rs:960-975` draws a radial elliptical scrim
(`t = dx*dx + dy*dy`, `SCRIM_CENTRE 0.22` → `SCRIM_EDGE 0.42`).
The reference transmits ≈.78 at **both x100 and x2400 at y720** — a radial
vignette cannot do that. Measured recipe, normalised to screen height H:

```
top_ramp    = clamp((0.27778*H - y) / (0.27778*H), 0, 1)
bottom_ramp = clamp((y - 0.75*H) / (0.25*H), 0, 1)
transmission(y) = 0.77635045 - 0.20660489*top_ramp - 0.28306150*bottom_ramp
alpha(y) = 1 - transmission(y)
```

Parent verified this against all nine measured patch alphas:
**mean abs error 0.0037, max 0.0105**. Plateau (y .278–.750 H) alpha **.224**,
top edge **.430**, bottom edge **.507**.

Note the plateau value .224 is essentially our current `SCRIM_CENTRE 0.22` —
the centre alpha was already right. **Only the shape is wrong.** Do not
"adopt the author's 0.12 dim_strength": that dims unfocused *clients* relative
to a bright focused window, compositor-side; ours is an absolute alpha over a
static wallpaper. Not substitutable — see `docs/transcript-dim-session.md`.

### 9b. The backdrop is SHARP, not globally blurred
Best-fit global Gaussian is **radius 0.6 device px** (gradient residual
+0.089 %, RGB RMSE 2.60/255) — a compression-scale effect, not a UI blur.
**Do not apply a 20–24 px global blur.** That is the OLD design.

### 9c. The clock is wallpaper-sampled frosting INSIDE the glyphs
`src/lockscreen.rs:1000-1009` paints an opaque **white base** then a fixed
**cyan specular gradient** (`C_CYAN` at `:199`). Both are wrong. The reference
clock ink has median RGB **[87,235,206]**, is **92.8 % teal-ish**, and **varies
across x** (left third [127,221,194] vs mid/right [87,235,206]) — it is the
wallpaper showing through, tinted. Measured equivalent:

```
B = GaussianBlur(registered wallpaper, radius = 0.016667*H)   // 24 px at 1440
material.r = clamp(1.19396*B.r + 83.78911, 0, 255)
material.g = clamp(1.25239*B.g + 83.09509, 0, 255)
material.b = clamp(1.25567*B.b + 76.69171, 0, 255)
out = backdrop*(1 - glyph_coverage) + material*glyph_coverage
```

RGB RMSE **2.39/255** over 114,924 glyph-interior pixels. A common-channel
approximation `clamp(1.227918*B + 82.943521, 0, 255)` gives RMSE 3.62 — usable
if the per-channel form is awkward, but **keeping the wallpaper-driven hue
matters more than the exact coefficients**. Do not hardcode #57EBCE as a
palette colour; it is this wallpaper's hue, not a brand token.

The wallpaper is **zoomed 1.030× about screen centre** in the locked state
(verified against pole/support edges over a 1.000–1.070 scan). Sample the
**registered, pre-veil** wallpaper for the glyph material, and draw the
separately veiled backdrop through the holes.

**Remove the white base and the cyan specular pass entirely.** A frosted
rectangular card behind the clock would be both wrong and an AI-slop tell —
the reference has no card, no border, no avatar, no date, no bottom field.

### 9d. Clock size
Reference clock ink **868×279** at 2560×1440 (colour mask
`(G>170)&(B>150)&(R<200)&(G-R>40)`; a luminance mask is contaminated by a cream
mast and an orange buoy and returns 2057×340 — do not use it).
Cap/H = **.19375**, ink/W = **.33906**. Ours after the advance fix is 867.9
wide — **−0.01 %**, already correct. `CLOCK_PX_FRAC 0.27` at
`src/lockscreen.rs:188` stays.

**The reference glyphs are much heavier than ours**: vertical stems 76–77 px
on a 279 px cap = stroke/cap **.272–.276**, terminals conspicuously rounded.
The author states the font is **SF Compact Display**. We ship Quicksand-Bold.
Do not claim a font identification from shape correlation — that method was
tried and retracted here. Report the stroke/cap gap as a finding if you cannot
close it with the shipped font.

---

## 10. RESOLVED — do NOT remove the workspace discs

The reference's resting top band contains **only the pill**. Measured across
all 33 stills: in `docs/reference/pill-settled.png` there are **92 dark pixels
outside the island column in the entire frame**, all at the screen edges.

A naive reading says "delete the workspace strip to match". **That is wrong.**
The other dark clusters in the later stills (s49–s113, at x≈507–655,
1827–1956, 2364–2429) are **wallpaper content**, not UI — sampling
`/tmp/ref/s/s65.png` at (580,40) gives RGB (35,22,8), at (1890,40) (38,24,2),
at (2390,40) (47,30,9): dark wood and window frames in a photograph of a room.
Our equivalent positions are all exactly (20,20,36), the compositor's
`root_color`, because no wallpaper is running.

**The owner explicitly asked to keep their workspace indicators and Bluetooth
panel.** They are additions to the reference, not deviations from it. Keep
them. Judge them on their own craft (uniform pitch, one accent, no invisible
shadow), not against a reference frame that never had them.

---

## 11. MAJOR — the clock is not white. It is a pale periwinkle.

Measured on **25 reference stills**, core ink pixels only (top 8 % by
luminance, inside the stadium, away from AA edges):

```
median of per-frame medians   (188, 214, 255)   #BCD6FF
p25 .. p75                    (185..192, 212..215, 253..255)
```

Ours measures **(254, 254, 254)** — pure white. `src/lock_timer.rs:293-296`
writes `v = c * 255` into all three channels equally.

**This is a fixed UI colour, not wallpaper frosting.** Proof: the clock reads
the same in two scenes with opposite white points —

| scene | scene white point | clock core |
|---|---|---|
| blue sky (`s1`, `s25`) | (236, 242, 254) | (188, 214, 255) |
| warm indoor (`s65`, `s85`, `s101`) | (228, 239, 215) | (189, 213, 252) |

A wallpaper-derived tint would follow the scene. It does not. Neither does a
global video grade: white-balancing on the cloud gives (203, 225, 256), still
not neutral, and the two scenes would need opposite corrections.

**The rest of the reference's text is tinted the same way, by role:**

| element | core RGB | B−R |
|---|---|---|
| clock | (188, 214, 255) | 67 |
| "Lock" label | (201, 207, 233) | 32 |
| uptime | (112, 112, 120) | 8 |

So it is one cool-neutral ramp, strongest on the primary text. Do not invent
three unrelated colours; derive the secondary/tertiary from the same hue.

**Fix:** `src/lock_timer.rs:293-296` — scale the coverage by the token instead
of writing `v` to all three channels. Keep the alpha-over structure; the
per-channel `max()` there is a separate known defect (it erases the AA ramp on
any non-black background).

**Caution:** blue channel clips at 255 in the source, so the true token may be
slightly brighter than #BCD6FF. Use #BCD6FF and say it is a lower bound.

---

## 12. Environment — the wallpaper was not running

Every earlier screenshot of ours sat on flat **RGB(20,20,36)** —
`~/.config/mango/config.conf:115` `root_color = 0x141424ff`. `swaybg` is
installed and `autostart.sh:58-60` launches it, but no process was running.
Started it (`swaybg -i ~/.config/mango/wallpaper/wallpaper.png -m fill`); the
top band now shows real wallpaper and the capsule reads correctly against it.

This was a large part of "looks really bad compared to his setup" and it was
**not a code defect**. Any visual score taken before this point was scoring a
black band. Re-take screenshots before judging colour or contrast.
