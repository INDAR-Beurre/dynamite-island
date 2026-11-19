# COUNCIL_LOG.md

Living state of the council loop. Read this first if context was compacted.

## Run

- **Task**: pixel-perfect, smooth, low-RAM replication of saneAspect's Dynamite dynamic island on **Mango WM** (reference was Hyprland), keeping the user's own additions (workspace discs, Bluetooth side panel), plus his lock screen. No AI-slop look. Dictation tool integrated. 1000+ stress tests. Screenshots/screen-recording frames are the acceptance instrument.
- **Tier**: `hard` (chain 5t) — 18 compiled modules, 32390 lines, render-pipeline change on the hot path, new fullscreen lock surface, test harness does not exist yet, acceptance measured in numbers. → **15 critics/round, child budget 70–100**.
- **Threshold**: median ≥ **9/10** with no surviving blocker. No round cap.
- **Visual reference substitution**: the skill's `magpie-image-mcp` Sleek project is unreachable (no `SLEEK_API_KEY`). The real reference is the user's own video frames in `/tmp/ref/` (2560×1440 native), extracted with ffmpeg. Recorded here as the approved substitution.
- **Model reality**: only the `pi` provider is enabled. Fixer = `magpie/group/deepseek-v4.1-flash-1017`. Critic rotation = `magpie/logfare/deepseek-v4.1-flash-1017`, `magpie/fasfas-ai/deepseek-v4.1-flash`, `magpie/logfare/glm-5.3`, `magpie/logfare/glm-5.3-flash`, `magpie/fasfas-ai/glm-5.3-flash`. User permits **6.1 sol** sparingly for the harshest slots. `hy4` and `gemini-4-hydrogen` named by the skill were absent from the provider list — not used.

## Definition of done (all four measurable, all runnable via `scripts/measure.py`)

| gate | baseline (2026-11-18 20:2x) | target |
|---|---|---|
| glyph sharpness `lapvar` | **1749.9** | **≥ 3800** (reference 6499.3) |
| idle CPU | **2.0–9.0 %** | **< 1.5 %** idle, **< 6 %** while dictating |
| RSS | **44.2 MB** | **≤ 30 MB** |
| pill aspect (w/h) | **4.03** (141×35) | **2.65 ± 0.1** (106×42 @1.5x → 70.7×28 logical) |

Additional gates: pill centre offset ≤ 2 px (baseline **+14 px**); horizontal text padding symmetric; no second accent colour; no gradient/glow/emoji.

## Reference geometry (measured, `/tmp/ref/s/s17.png`, 2560×1440)

- pill **106 × 42** device px, aspect **2.667**, height **2.917 %** of screen height, centre offset **−0.5 px**
- clock glyph ink: **53 × 14** → cap height = **0.333 ×** pill height; total text width = **0.500 ×** pill width; padding **27 px L / 27 px R** (symmetric)
- fill pure black; AA edge ramp ≈ **1.5 px** (`210.7 → 160.7 → 20.0 → 2.0`)
- typeface is a **compact display face** (SF Compact Display): narrow oval counters, medium weight, tight tracking — *not* a rounded geometric face

## Baseline defects (evidence, not opinion)

1. **Wrong typeface + wrong weight + wrong size.** Every panel loads `Quicksand-Bold.ttf` (fallback `Quicksand.ttf`) — a rounded geometric bold face. Load sites: `src/main.rs:5635`, `:6979`, `:7461`, `:7877` (four independent parses of the same file). Current text ink is **17 px on a 35 px pill = 0.486 ratio** vs the reference's **0.333** — text ~46 % oversized, and the wide bold geometry is why the pill reads chunky.
2. **Glyphs are rasterised at logical size and then bilinearly upscaled.** `src/main.rs:1834 layout_text` calls `font.rasterize(ch, px)`; `src/main.rs:483 const RENDER_SCALE: u32 = 2` then doubles the whole canvas via `upscale_canvas_2x_into`, and the compositor downsamples 2×→1.5×. Two resamples ⇒ `lapvar` 3.7× below reference.
3. **Full-stage redraw at audio-packet rate.** `src/main.rs:5878` — the `dictation_bars:` socket branch calls `state.draw_if_ready()`, re-rendering 1176×636 and then the ~3 M-pixel upscale, for every microphone frame.
4. **Debug `eprintln!` in the hot path.** `src/main.rs:5877` prints every socket datagram unconditionally → the multi-MB `~/.cache/dynamite.log`.
5. **No glyph cache.** Every visible glyph is re-rasterised every frame.
6. **Fat memory.** Fixed stage 1176×636 (for a 90×24 pill) × `RENDER_SCALE 2` ⇒ 11.96 MB shm buffer, double-slotted; the icon font parses to ~19 MB; the Quicksand font is parsed four times.
7. **Off-centre composition.** Pill centre sits **+14 px** right of screen centre; inside the pill the text has **27 px left / 10 px right** padding.
8. **Workspace discs render as floating dark circles with a glow halo** outside the pill — reads as decoration, not signal (slop per `kill-ai-slop`).

## Safety net established before any delegation

- `git init` + baseline commit **8e58edc** (107 files) in `~/rust-island-build`. There was **no version control at all** before this, with 61 stale `src.bak-*` dirs; that is now recoverable.
- `.gitignore` covers `target/`, `*.bak-*`, `src.bak-*/`, `*.ppm`, `*.log`.
- `scripts/measure.py` — the shared instrument. Captures with `grim`, detects the capsule, and prints JSON with `lapvar`, `lapmean`, pill bbox/aspect/centre offset/height %, edge-ramp luminance, plus the live process RSS and 2-second CPU %. Every critic must quote it. Verified on both the live screen and the reference frame (`--png /tmp/refpill.png`).

## Round history

_(nothing yet — round 1 not dispatched at time of writing)_

### Round 1 — dispatched 2026-11-18 20:2x

- **Focus**: rendering correctness of the collapsed pill (typeface, glyph sharpness, geometry) + the perf/memory fixes that share the same edit surface.
- **Fixer**: `magpie/group/deepseek-v4.1-flash-1017` (thinking high), `taskId` `9673dda5-1765-4a95-b515-529c6db67bed`, chain 3t.
- **Brief carries**: F1 wrong typeface (Quicksand-Bold, parsed 4×) → Inter at ⅓ pill height with −0.4 tracking; F2 glyphs rasterised at 1x then upscaled 2x → rasterise at device resolution and blit 1:1; F3 per-audio-packet full-stage redraw → incremental bars-only update; F4 hot-path `eprintln!` → delete/gate; F5 no glyph cache → bounded cache; F6 11.96 MB stage + ~19 MB icon font → shrink, single slot, one parse; F7 geometry 141×35 aspect 4.03 → 106×42 aspect 2.65; F8 asymmetric 27/10 padding → centred; F9 workspace-disc glow → flatten only.
- **Protected**: socket protocol literals, autostart/supervisor scripts, other panels' feature logic, the 62 existing warnings, no new dependencies.
- **Panel**: pending fixer completion.

## Lock-screen reference geometry (measured, `/tmp/ref/lock/l60.png`, 2560×1440)

Vertical bands of bright ink, top to bottom (y0–y1, height, share of screen height):
- tray cluster (battery `80 %` + wifi), top-right: `y 17–27`
- date `Friday, July 24`: `y 166–196`, h **31** (2.15 %), centred at x 1267.5 → offset **−12.5 px** (effectively centred)
- clock `15:39`: `y 233–374`, cap h **142** (**9.86 %** of screen height), ink width ≈ **299 px** (11.7 % of screen width), gap date→clock **37 px**
- vertical centre of the clock sits at ≈ **21 %** of screen height
- avatar ring: `y 1137–1168` band pair (circle centre ≈ 80 % of height)
- username `user0`: `y 1225–1242`, h **18** (1.25 %)
- password chip placeholder `Enter Password`: `y 1289–1302`, h **14** (0.97 %)

Backdrop treatment, measured against the unlocked wallpaper frame (`l200.png`):
- **veil**: mean luminance 56.1 locked vs 78.7 unlocked → ratio **0.713**, i.e. roughly **0.29–0.30 alpha black** over the backdrop (light veil, not opaque — the wallpaper still reads as an image).
- **blur**: mean absolute horizontal gradient **0.311** locked vs **1.988** unlocked → high-frequency energy reduced **6.4×**. A heavy gaussian (visually sigma in the tens of device px), not a light blur.
- no panel/card behind the clock: the type sits directly on the blurred backdrop.

### Panel size raised

User instruction: "Use a lot more subagents at a time." t3's `maxBatchThreads` is 20, so the critic panel is raised **15 → 20** for every round from now on, and the wait for round 1 is spent on **8 parallel research/test workers** whose deliverables all feed round 2. All launched in the same turn.

Worker roster (taskId suffix; full id = `node:delegated-task:command%3Amcp%3A11072e5a-2f28-41c1-be02-f26dfccad48d%3Adelegate-task%3A<uuid>`):

| # | uuid | role | deliverable |
|---|---|---|---|
| fixer r1 | `9673dda5-1765-4a95-b515-529c6db67bed` | implementation | F1–F9 on the render path |
| 1 | `9eabfe51-8e7d-4aec-a3b6-5cc1e0624204` | test | `scripts/stress/` — 1000+ protocol/state/leak/latency tests |
| 2 | `c7502992-0f84-4268-8922-106901b58198` | research | `docs/lockscreen-spec.md` |
| 3 | `6f501b0d-8ec4-4a0d-94c2-76a6662e7141` | research | `docs/typography-spec.md` (SF Compact substitute) |
| 4 | `40e9e232-61ef-43d3-8444-ebc18f05ef1f` | research | `docs/mango-capabilities.md` (Hyprland→Mango gap) |
| 5 | `81de5049-33c0-41db-a4d5-a78457f5df3c` | review | `docs/slop-audit.md` |
| 6 | `22c8ab8e-7f25-459e-831f-022dbcb51acd` | research | `docs/island-states-spec.md` |
| 7 | `6856be06-04d5-4e80-993e-9d5f6143286a` | test | `scripts/motion/` — frame-capture smoothness harness |
| 8 | `034783eb-de7e-4826-9acf-ae1abb436e4e` | research | `docs/dictation-integration.md` |

Concurrency discipline given to every worker: no edits to `src/`, `Cargo.toml`, `Cargo.lock`; no `cargo build/check/test` (target-dir lock is held by the fixer); no killing or restarting the live `dynamite-rust` process; no destructive socket commands. Workers write only to `docs/`, `scripts/stress/`, `scripts/motion/`.

#### Worker 4 — Mango capabilities (returned)

`docs/mango-capabilities.md` written. Mango 0.18.3. Findings that change the plan:
- **`wlr_fractional_scale_manager_v1` + `wlr_viewporter` ARE present**, and `mmsg get monitor eDP-1` reports `"scale":1.5` (logical 1504x1002 for the 2256x1504 panel). The panel currently cannot use it because **smithay-client-toolkit 0.19 exposes no fractional-scale API** — that is the real reason `RENDER_SCALE = 2` exists.
  → **Strategic consequence**: the sharpness fix has two viable paths. (i) Manual protocol binding via raw `wayland-client` to `wp_fractional_scale_v1`, render at exactly 1.5 — sharpest *and* smallest buffer (best against the RAM goal), but real protocol work. (ii) Keep `buffer_scale` integer but raise it to **3**: the compositor then maps a 3× buffer to 1.5× logical = an exact **2:1 integer downsample**, which stays crisp (unlike 2→1.5) at the cost of 2.25× the pixels of 2×. Path (i) satisfies both the sharpness and the ≤30 MB RSS gate; path (ii) is the cheap fallback. Round 2 must decide this explicitly, not drift.
- `set_exclusive_zone` honoured (full wlroots layer-shell state machine) — so the bar's reserved strip works.
- `keyboard_interactivity` OnDemand and Exclusive both supported → a lock screen can take focus.
- **`ext-session-lock-v1` is implemented** (`wlr_session_lock_manager_v1`); `swaylock` already uses it, bound to Super+Ctrl+L. That is the correct route for the real lock, **not** an overlay layer surface (insecure).
- **Gaps that will silently no-op**: no `dim_inactive` (only idle-dim), no client-requestable backdrop blur, no per-layer corner radius, no per-layer blur size/saturation. `rule.conf` already pins `layer_rule=no_blur:1,no_shadow:1,no_animation:1,layer_name:dynamite` — so the panel must keep blurring the wallpaper itself (which `wallpaper_blur.rs` does) and must not expect compositor-side effects on any of its surfaces.
- Caveat recorded by the worker: `wayland-info` is not installed, so globals were inferred from the compositor binary's strings plus `mmsg`, not from a live `wl_registry` dump.

#### Worker 5 — slop audit (returned)

`docs/slop-audit.md` written. Scanner found 0 (it is CSS/JSX-tuned) — all findings are from reading the draw code. Raw counts: ~78 colour bindings across 14 files, **~45 distinct RGB values**, **4 hand-typed periwinkle accents**, **4 near-duplicate secondary greys**, ~11 rendered type sizes, radii disciplined (panel 20, rows 9–12, tiles 4).

This is the answer to the user's "must not look like AI slop" demand. Findings, most severe first:
1. **Four hand-typed periwinkles for one accent** — `#7CA2F5` `src/library.rs:36`, `#93A4F5` `src/main.rs:927`, `#74A3F6` `src/controlcenter_panel.rs:362,366`, `#7AA2F7` `src/music_panel.rs:288`. An `ACCENT` token exists but is unused, so theming cannot actually work until these are unified.
2. **Four near-duplicate secondary greys** patched independently toward legibility: `#9AA0A6` `src/launcher_panel.rs:177`, `#BEC4CA` `src/clipboard_panel.rs:106`, `#C0C4C9` `src/controlcenter_panel.rs:374`, `#C8CCD0` `src/music_panel.rs:279-282`.
3. **The dictation canvas is the worst offender** — `src/dictation_canvas.rs:126-132` and `:400-588`: a 22 px glow halo, bloom 0.32, a breathing idle hum, and the codebase's **only multi-stop gradient** (teal→cyan→blue). This is the slop the user sees *while dictating*, i.e. exactly the wrong place for it.
4. `src/clipboard_panel.rs:115-118` — mock document lines drawn in purple `#BB9AF7` + cyan `#7DCFFF`, hues that exist nowhere else. Showcase-tile tell.
5. `src/main.rs:693-699,2499-2507` — a lit-top/dark-bottom rim on **every** disc (nav *and* workspace) plus three separate shadow systems for the same class of round shape. This is the halo I saw around the workspace discs.
6. `src/lockscreen.rs:199` — a lone saturated cyan clock, a second accent hue breaking the one-accent rule.

**Explicitly protected** (do not "improve" these): the flat frost material (one floor, one alpha, no glass layering); the per-role measured type scale (36.5/13.4 are measurement corrections); the radius discipline; real icon-font glyphs with zero emoji anywhere; no stat rows or invented numbers; zero nested cards / separators / glowing status dots across seven panels; and the premultiplied-alpha coverage antialiasing.

- **worker 8 (dictation) failed** on `magpie/fasfas-ai/deepseek-v4.1-flash` with `Error reading response: malformed server-sent event JSON` after one tool call (provider instability, not a task fault). Retried per user instruction "always try switching models if unstability": `magpie/logfare/glm-5.3-flash`, uuid `8a97fd4e-a708-4b68-b81f-d4663e40a97f`.

### Concurrency ceiling discovered (important for every later round)

Launching 9 children in one turn hit the provider's limit: `AiError: rate limiting: inference request per min rate reached`. So the real bound on "many subagents at a time" is **not** t3's `maxBatchThreads: 20` — it is the `pi` provider's **inference-requests-per-minute** cap. Practical rule for this run: **wave size 4–6**, and a 20-critic panel must be launched in **staggered sub-waves** (e.g. 5+5+5+5 with a gap), not all at once, or the tail of the panel dies on rate limit and the round scores on a truncated panel.

Dictation audit is now **deferred, not failed** — an immediate second retry would just burn more rate budget. It goes out in the next wave once the current children drain.

### Route reliability (operational, affects every later round)

Two failures, **both on `magpie/fasfas-ai/deepseek-v4.1-flash`**:
- worker 8 (dictation) — `malformed server-sent event JSON`
- worker 6 (island-states geometry) — `upstream stream error: error decoding response body`

`magpie/group/` and `magpie/logfare/` are so far clean. **Rule for this run: do not route children to `magpie/fasfas-ai/*`.** Use `magpie/group/deepseek-v4.1-flash-1017`, `magpie/logfare/deepseek-v4.1-flash-1017`, `magpie/logfare/glm-5.3`, `magpie/logfare/glm-5.3-flash`. Worker 6 (island-states geometry) is **deferred, not failed** — it re-dispatches on the logfare route in the next wave.

### Model catalogue correction + the "6.1 sol" the user meant

`orchestrator_capabilities(search:"sol 6.1")` resolves the user's "6.1 sol" to **`magpie/agentrouter/gpt-6.1-sol`** (label "GPT 6.1 Sol · AgentRouter", thinking: minimal|low|medium|high|xhigh|max). It **is** available on the `pi` provider. Per the user: use it sparingly for the hardest critique slots because it drains their account — so: **gpt-6.1-sol reserved for at most 2–3 slots per round on the decisive lenses** (visual fidelity against the reference frames, and the perf/memory proof). Everything else stays on deepseek-v4.1-flash / glm-5.3.

Third provider failure: `magpie/logfare/deepseek-v4.1-flash-1017` returned "The model is temporarily unavailable" mid-run for the typography study (its last message was also code-switched into Chinese, a instability tell). So the flakiness is **load-driven, not route-specific** — 9 concurrent children was simply too many. Revised operating rule: **waves of 3–4**, one retry each with a different model, and a 20-critic panel launched as 4 staggered sub-waves of 5.

### Panel death → recovery, and a baseline correction

Cause: fixer r1 was **interrupted** (`status: interrupted`, last message "Now the F2/F5 rendering changes:") during the provider outage, leaving a half-applied tree whose binary exited `rc=0` after 2 s. The supervisor itself had been **SIGSTOPped** (`ps` stat `T`, PID 1672), so it could neither reap its zombie child (PID 3419, `<defunct>`) nor restart the island — and a stopped process keeps its `flock`, so a second supervisor could not take over either.

Recovery: `git config user.name/email` set repo-locally (absent, which had also blocked committing); the partial work committed then parked on branch **`wip-fixer1-partial`** so nothing is lost; `git checkout 8e58edc -- src/` restored the baseline tree (verified `git diff 8e58edc --stat -- src/` empty); rebuilt **`cargo build: 0 errors, 62 warnings`**; `kill -CONT` then `kill -9` on the stopped supervisor so the zombie got reparented and reaped; fresh supervisor started with `setsid nohup ~/.config/mango/scripts/dynamite_autostart.sh`. Live again: supervisor **PID 6776**, island **PID 6790**.

**Baseline correction — the earlier 141×35 / +14 px figure was a detector artifact** (the pre-fix scan merged a workspace disc into the capsule). With the corrected detector the true live baseline is:

| metric | ROUND-1 TRUE BASELINE | REFERENCE |
|---|---|---|
| pill w×h | 108×35 | 106×42 |
| aspect | 3.086 | 2.667 |
| centre offset | **−1.5 px** | −0.5 px |
| height % of screen | 2.327 | 2.917 |
| glyph lapvar | **1678.8** | **6499.3** |
| glyph lapmean | 54.2 | 79.58 |
| RSS | 50.5 MB | — |
| CPU | 2.5–4.0 % | — |

So **width and centring are already essentially correct**; the real geometry defect is **height** (35 vs 42 device px = 23.3 vs 28 logical), and the whole visual gap is **typography and weight**, not size. A 3× crop of the live bar (`/tmp/check_bar3.png`) confirms it by eye: `21:50` is rendered in heavy rounded Quicksand Bold at roughly **0.97 of the capsule width** (reference is 0.500) so it crowds the pill, the four workspace discs are chunky white-on-dark with an active periwinkle `1`, and the tray glyph is malformed — it reads as a broken butterfly/bird shape, i.e. a **wrong glyph index in the Material Symbols font**, not a layout bug.

Round-1 scope therefore re-split into two sequential fixers on the same tree (a fresh child per area, because the single wide brief outlived one child):
- **r1a — typography + geometry** (F1, F2, F5, F7, F8, plus the tray glyph): the entire measurable visual gap.
- **r1b — performance + memory** (F3, F4, F6): the CPU/RSS gates.

### Measurement correction #2 — most of my earlier findings were artifacts. Do not repeat them.

Two earlier scans were contaminated. The first ("ink 105×17, padding 27 L / 10 R, text = 0.745 of pill width") scanned a **rectangular** crop of a **rounded** capsule, so bright wallpaper leaked through the corners. The second (thresholded bright pixels over a bare crop) leaked wallpaper outright and reported a nonsensical `cap_h=32` on a 35 px pill.

The honest instrument is a **capsule-interior mask**: crop the detected pill bbox, build `d = hypot(x - clip(x, r, W-r), y - H/2)`, keep only `d < r - 3`, then threshold at luminance 110. Re-measured with that mask:

| metric | REFERENCE (106×40) | CURRENT (108×35) | verdict |
|---|---|---|---|
| ink cap height | 15 | 19 | — |
| **cap / pill height** | **0.375** | **0.543** | **text is 45% too large for the capsule** |
| ink text width | 54 | 60 | — |
| text width / pill width | 0.509 | 0.556 | close |
| left / right margin | 26 / 26 | 25 / 23 | **already symmetric — earlier "27/10" was false** |
| stroke / cap height | 0.200 | 0.211 | **weight is already right — earlier "2.5× too heavy" was false** |

Corrected findings, superseding F1/F7/F8 as previously written:
- **The dominant, measurable visual defect is scale, not weight and not padding**: `cap/pill_h` is 0.543 against 0.375. Equivalently the capsule is **too short** (35 device px vs the reference's 40–42).
- **Width and centring are already correct** (108 vs 106; offset −1.5 px).
- **Padding is already symmetric** (25/23). F8 is dropped.
- **Stroke weight is already correct** (0.211 vs 0.200). The typeface *shape* still differs — Quicksand is a rounded geometric face whose circular terminals and wide `0` read chunkier than SF Compact Display's flat-sided narrow digits — so switching to `Inter.ttf` is still right, but it is a **secondary refinement**, not the main fix, and it must not be allowed to change `cap/pill_h` or the margins.
- The softness is real and separate: glyph `lapvar` 1678.8 vs 6499.3, caused by rasterise-at-1×-then-upscale-2× (F2). That is the single highest-leverage fix in the round and it is independent of all geometry.
- The tray glyph is a **wrong code point** in Material Symbols — it renders as a broken butterfly. Separate one-line fix.
- The blue horizontal band visible under the bar in `/tmp/check_bar3.png` is a **compositor window border**, not the panel; `/tmp/mango-capabilities.md` records `border_radius=8` and a focused-border colour, so confirm before anyone "fixes" the panel for it.

### Workers that survived the outages

`scripts/stress/` (36.1K `stress.py`, 11.1K `analyze.py`, 1.7K `drive_bars.py`, 3.2K `selftest.py`, 5.6K `run_all.sh`, 7.6K README) and `scripts/motion/` (5.1K `animate.py`, 5.2K `capture.sh`, 1.2K `run_all.sh`, 6.7K README) both delivered. The stress README independently recovers the socket protocol from `src/main.rs` and documents it: unix **datagram** at `$XDG_RUNTIME_DIR/dynamite-rust.sock`, payload `[0u8; 512]`, `String::from_utf8_lossy(...).trim()` compared **as a whole string**, case-sensitive, unknown words silently ignored, **no reply channel**, and one destructive word — `quit`. Kinds: state (`collapsed` `dictation` `lock` `power` `music` `calendar` `controlcenter` `launcher` `wallpaper` `themes` `clipboard` `traymenu`), lockscreen (`lockscreen` `unlock` `locktype` `lockback` `lockwrong`), dictation (`dictation_start` `dictation_end` `dictation_bars:<floats>`), misc (`clock` = `collapsed`, `reload`). Invoke with `bash scripts/stress/run_all.sh [--group NAME]`; exit 0 only if every group passes; it never rebuilds and never restarts the panel, which is why it is safe to run against the live island.

Round-1a dispatched to the fixer `magpie/group/deepseek-v4.1-flash-1017` (taskId uuid `c61c46c7-860e-47ce-89e3-d120a997de4a`) with the corrected, superseding brief. Two research children re-dispatched on the logfare route in the same wave (dictation audit uuid `3facc4d5-8ddd-43f7-9803-1e09ca8cd65b`; island-states geometry still deferred). Stress and motion runs are held until the fixer releases the tree, so their numbers are not polluted by a concurrent build.

Dictation audit failed a **third** time, this time `AiError: Service temporarily at capacity` on `magpie/logfare/glm-5.3-flash` — so the constraint is the provider's overall capacity right now, not any single route or model. Policy change for the remainder of the run: **hold all non-critical research children** and spend the available request budget on (1) the fixer and (2) the critic panel, which are what actually move the gate. The dictation audit is deferred to after the visual gate is met; its subject matter is not on the critical path to a sharp, correctly-scaled pill.

r1a progress check (no polling loop, single check): fixer still running, tree at +148/−24 on `src/main.rs`. Verified the two things most likely to be wrong are right: `IslandState::Collapsed => (cfg().collapsed_width, 28)` (was 24 → 42 device px, aspect 108/42 = 2.57 ✓), and `clock_font()` returns `Option<&'static fontdue::Font>` via `OnceLock`, logging and falling back to Quicksand when `~/.local/share/fonts/Inter.ttf` is missing — **no startup panic**, as demanded. It also added `draw_collapsed_pill_2x()` which composites glyphs 1:1 into the 2× shm buffer while reusing `draw_panel_into`'s exact capsule fill and AA band, so geometry/margins stay untouched and only glyph sharpness changes. That is the correct shape for A1.

### The gate is now one command (instrument hardened)

`scripts/measure.py` gained `ink_metrics()`, which applies the capsule-interior mask so no critic can reproduce my two earlier artifacts. Both images now come out of the *same* detector, so the comparison is apples-to-apples. Reference detected as **111×42 at (1225,19), aspect 2.643, offset 0.0** (earlier hand-measured 106×40 was the same capsule with a looser threshold).

Canonical gate table, reproducible with `python3 scripts/measure.py` (live) and `python3 scripts/measure.py --png /tmp/ref/s/s17.png` (target):

| metric | CURRENT | TARGET |
|---|---|---|
| pill w×h | 108×35 | ≈111×42 |
| aspect | 3.086 | 2.643 |
| centre offset | −1.5 | 0.0 |
| glyphs.lapvar | 1678.8 | **6503.2** |
| ink.cap_over_pill_h | **0.543** | **0.357** |
| ink.text_over_pill_w | 0.556 | 0.486 |
| ink.margin L/R | 25 / 23 | 28 / 29 |
| ink.stroke_over_cap | 0.211 | 0.200 |

Pass = lapvar ≥ 3800, aspect 2.65 ±0.15, cap_over_pill_h 0.357 ±0.04, offset ≤2, symmetric margins, stroke_over_cap ≤0.24, RSS ≤30 MB, idle CPU <1.5%.

Note the fixer's chosen `Collapsed => 28` logical px gives 42 device px, matching the target height exactly, and it kept `collapsed_width` untouched — so aspect lands at ≈2.57, just inside tolerance. Good call by the fixer; do not let a later round "fix" the width to chase 2.643 exactly.

**Parent bookkeeping error, recorded honestly:** committing `scripts/measure.py` also swept in the previously-staged `src/` revert, so commit `5aad84b` carries a baseline-restore of `src/main.rs` alongside the instrument change. The fixer's live edits are **unaffected** — `git diff HEAD -- src/` still shows its +148/−24, the working tree is intact, and the true baseline remains reachable at `8e58edc` (and the interrupted partial at `2e230b4` / branch `wip-fixer1-partial`). No content lost. Lesson for the rest of the run: **the parent does not run `git add`/`commit` while a fixer owns the tree** — only the fixer commits; the parent commits solely between rounds, after the tree is verified.

### Test-count and recording constraints (both now known, both bind later rounds)

Stress suite totals **628 + 220 + 200 + 120 + 5 = 1173 tests**, already above the user's "≥1000 before the next round" bar. Groups: `protocol_fuzz` 628 (every word × 12 malformed variants incl. NUL/non-UTF-8/4 KB/200 KB/1 MB, all 132 two-word datagrams, NaN/±inf/arity-120 `dictation_bars`, 4×1000 rapid fire), `state_machine` 220 (12×12 transition matrix, lock cycles, `locktype` saturation at 32, 40 seeded random walks), `load_leak` 200 (lockscreen→unlock→launcher→wallpaper→collapsed with VmRSS per iteration), `latency` 120 (p50/p95/p99 send→morph-started), `liveness` 5. Falsifiable bounds already encoded: RSS growth **≤8 MB** measured from the median of the first 20 warm-up iterations (first lockscreen decodes 1504×1002×4 ≈ 6 MB; first glyph draw parses the icon font), latency **p95 ≤400 ms**, state change **≤1500 ms**. Each failure prints the exact word and payload length so it reproduces with one command.

**The screen-recording ask cannot be met literally, and the honest substitute is documented.** There is no recorder and no working ffmpeg capture path without root: `-f kmsgrab` → `No usable planes found. Error opening input: Invalid argument` (needs DRM master); `-f wayland` and `-f pipewire` → `Unknown input format` (not compiled in); `-f x11grab` opens but returns **100% black** (mean pixel 0.0) because the panel never reaches the X server. `ffmpeg -devices` offers only alsa/fbdev/iec61883/jack/kmsgrab/lavfi/oss/pulse/sndio/v4l2/x11grab/xv. So `scripts/motion/capture.sh` loops `grim` into PPM instead — which *is* "split a recording into still frames", just captured frame-by-frame rather than decoded from a video.

Caveats that make its numbers honest, and that every motion critic must repeat: grim is not vsync-locked, so intervals describe **how fast grim could go, not the compositor's clock**; achieved ~20–22 fps unloaded, only **3–4 frames land inside the 160 ms morph**, so the one-shot morph is under-sampled and `analyze.py` warns below 4; **sustained dictation bars are the reliable subject**. PNG encode costs grim ~313 ms vs PPM decode 35 ms, hence PPM default (~150–190 MB per 5 s run). `--region` is LOGICAL px but output is device px (1.5×). Host load dominates — a single ffmpeg transcode hit ~400% CPU and dropped the same crop from ~20 to ~8 fps, so **never compare fps across runs at different load**; `meta.txt` records loadavg and `analyze.py` prints a LOAD line. What *is* trustworthy regardless of timing: distinct-frame count, mean-abs delta, and zero-delta duplicate pairs. Morph timer is `Duration::from_millis(16)` (`src/main.rs:4546,4572`) ≈ 62 fps target for `MORPH_DURATION_MS` 160 (`src/morph.rs:85`).

`animate.py` refuses `quit` and `lockscreen` and allow-lists everything else, so a motion run cannot kill the panel or take over the screen.

r1a implementation shape verified by reading the tree (not the fixer's claims): `lock_timer::TEXT_PX` **17.0 → 14.4** (0.847×, exactly the cap-ratio correction 0.357/0.543 = 0.657 combined with the 28/24 pill growth ⇒ 0.847); `rasterize_cached` now used by `layout_text` (`src/main.rs:1839`); `clock_font()` at `src/main.rs:1857`; `draw_collapsed_pill_2x` defined `src/main.rs:2208`, called `src/main.rs:4884` guarded by `self.morph.is_none() && self.state == IslandState::Collapsed`; morph path rasterises at `lock_timer::TEXT_PX * content_scale` (`src/main.rs:2954`) so animating frames stay sharp too. Icon font still gated by `icon_font_for(&self.icon_font, self.state)` so collapsed/lock never pay its ~19 MB.

**Known residual to hand the panel:** the sharp path still runs `upscale_canvas_2x_into(&self.scale_scratch, w, h, canvas)` over the whole stage FIRST and only then overwrites the pill region (`src/main.rs:4878-4885`). Correct output, wasted work — the collapsed state should skip the upscale entirely. That belongs to the r1b perf round, not a reason to reject r1a.

### r1a interim measurement (parent-verified, fixer still running)

Binary rebuilt 22:09, panel restarted clean (supervisor 17101, island 17115). Live numbers:

| metric | baseline | r1a now | target | verdict |
|---|---|---|---|---|
| pill w×h | 108×35 | **110×41** | ≈111×42 | ✓ |
| aspect | 3.086 | **2.683** | 2.643 | ✓ |
| centre offset | −1.5 | **−0.5** | 0.0 | ✓ |
| glyphs.lapvar | 1678.8 | **12300.7** | ≥3800 (ref 6503) | ✓ but overshoots ref 1.9× |
| ink.cap_over_pill_h | 0.543 | **0.390** | 0.357 ±0.04 | ✓ |
| ink.text_over_pill_w | 0.556 | **0.464** | 0.486 | ✓ |
| ink.margin L/R | 25/23 | **30/29** | symmetric | ✓ |
| ink.stroke_over_cap | 0.211 | **0.125** | 0.200 | ✗ **too thin** |
| RSS | 50.5 | **57.5 MB** | ≤30 | ✗ worse |
| CPU | 2.5–4.0 | **5.0 %** | <1.5 idle | ✗ worse |

**New finding — the weight overshot.** Inter's variable default instance is *Regular*, and at 14.4 px it renders `stroke_over_cap` **0.125 vs the reference's 0.200** — visibly too light (A/B at `/tmp/ab_r1a.png` confirms by eye: reference digits are clearly sturdier). The right weight is Inter **Medium**; fontdue 0.9 cannot set a variable axis, so the fixer must either bake a static Medium instance or find another route. `lapvar` 12300 vs reference 6503 is the *same* defect seen from the other side — a thin high-contrast stroke maximises Laplacian response; matching weight should pull lapvar back toward the reference rather than past it. **lapvar alone is therefore not a sufficient gate; it must be read together with stroke_over_cap.** Instrument note for critics: do not celebrate a lapvar above the reference.

**Second new finding — the pill sits too close to the screen edge.** Measured as a fraction of screen height: reference top gap **1.319%**, ours **0.332%** — a 4× error that no earlier metric caught because everything so far was relative to the pill, not the screen. Reference pill is also slightly narrower as a fraction of width (4.336% vs our 4.876%). Correct config values for this 2256×1504 @1.5 screen: **`gap_top ≈ 13`** (currently 3) and **`collapsed_width ≈ 65`** (currently 74). The skill doc's "Gap from screen edge 15 px" corroborates ~13–15, and the reference video's own settings panel says 15. This supersedes my earlier "do not change collapsed_width" note — that instruction was written before the screen-relative measurement existed.

RSS/CPU regressions are expected at this stage (extra 2× draw path plus the still-running whole-stage upscale); they are r1b's job.

### Typeface weight solved and baked (parent-produced asset, not an edit to src/)

`fontTools 4.68.0` is installed, and `Inter.ttf` is a variable font with **two** axes: `opsz 14..32 (default 14)` and `wght 100..900 (default 400)`. fontdue 0.9 has no axis support, which is why the default instance came out Regular-thin.

Empirical sweep — instantiated nine (wght, opsz) combinations and measured each with PIL at the size giving cap height 15 (the reference's cap), scoring `stroke/cap` and `text_w/cap` for the string `00:05`. Reference targets: **stroke/cap 0.200, w/cap 3.60**.

| wght | opsz | px | cap | stroke/cap | w/cap |
|---|---|---|---|---|---|
| 400 | 14 | 19 | 14 | 0.143 | 3.57 |
| 400 | 20 | 20 | 15 | 0.133 | 3.53 |
| 500 | 14 | 19 | 14 | 0.143 | 3.71 |
| 500 | 20 | 20 | 15 | 0.167 | 3.60 |
| **550** | **20** | **20** | **15** | **0.200** | **3.60** |
| 550 | 28 | 20 | 15 | 0.200 | 3.53 |
| 600 | 20 | 20 | 15 | 0.200 | 3.73 |

**Winner: `wght=550, opsz=20`** — matches both reference metrics exactly. Baked to a static instance at **`~/rust-island-build/assets/fonts/Inter-Medium.ttf`** (342,116 B, `fvar` removed, `usWeightClass 550`). This is an asset the fixer must load instead of `~/.local/share/fonts/Inter.ttf`; the parent deliberately did not edit `src/` while the fixer owns the tree.

Note `opsz` matters as much as `wght`: at opsz 14 (the variable default) even wght 500 stays at 0.143 stroke/cap because the optical-size axis thins strokes for small text. Anyone re-deriving this must pin **both** axes.

### Gate extended with screen-relative metrics (instrument patch #2)

`scripts/measure.py` gained `screen_relative()` → `out["screen"]`, because every prior metric was pill-relative and a pill hugging the screen edge scored perfectly. Verified on both images:

| metric | LIVE | REFERENCE | target |
|---|---|---|---|
| screen.gap_top_pct | **0.332** | **1.319** | 1.32 ±0.2 |
| screen.width_pct | 4.876 | 4.336 | 4.34 ±0.3 |
| screen.height_pct_screen | 2.726 | 2.917 | 2.92 ±0.15 |

Full PASS criteria now: lapvar ≥3800 **and** not far above the reference (read together with `ink.stroke_over_cap` ∈ 0.17–0.24 — lapvar alone rewards too-thin strokes); aspect 2.65 ±0.15; cap_over_pill_h 0.357 ±0.04; offset ≤2 px; symmetric margins; **gap_top_pct 1.32 ±0.2**; **width_pct 4.34 ±0.3**; height_pct_screen 2.92 ±0.15; RSS ≤30 MB; idle CPU <1.5%.

Config translation for this screen (2256×1504 @1.5): `gap_top` 3 → **13**, `collapsed_width` 74 → **65**.

Config is a live file the fixer can edit without touching `src/`: **`~/.config/dynamite-rust/config.conf`** (path from `fn config_path()` at `src/main.rs:304`), read by `parse_config` (`src/main.rs:308`), snapshot accessor `fn cfg() -> IslandConfig` (`src/main.rs:300`), applied on the `reload` socket word. Current contents: `notch_mode=0 bar_height=24 collapsed_width=74 expanded_height=150 gap_top=3 inner_padding=14 anim_ms=160 hover_open=0 scroll_cycle=1 clock_24h=1 wallpaper=wallhaven-9o6yzd.jpg theme=everblush nl_until=06:00`. Note `bar_height=24` is a *config* key while the collapsed height is now hardcoded `(cfg().collapsed_width, 28)` at `src/main.rs:201` — a divergence a critic should flag: the settings panel writes `bar_height` but the collapsed pill ignores it. Clamp on width is `40..=600` (`src/main.rs:321`), so 65 is legal.

### Harness-vs-instrument disagreement resolved (do not let a critic report this as a defect)

`bash scripts/stress/run_all.sh --group liveness` reports **FAIL tests=5 fails=2** with `! pill check failed: pill width 599px outside 0.6-1.6x of 111px; pill interior not filled (dark=0.45) | x[828,1427] y[5,119] w=599 inside=0.45 outside=0.39`. This is **not** a panel defect: the liveness group had just driven the panel into the `themes` state (`responsive: themes accepted in 0.7 ms after resume`), so the pill check ran while a **599 px-wide expanded Themes panel** was on screen and correctly rejected it as not-a-collapsed-pill. Two other liveness assertions passed: responsiveness 0.7 ms, event loop `+55 cpu ticks while drawing`.

Proof the panel was healthy: sent `collapsed` to `$XDG_RUNTIME_DIR/dynamite-rust.sock` and re-measured — pill back to **110×41 at (1073,5)**, aspect 2.683, offset −0.5, unchanged.

**Fix owed to the stress harness (r1b or a harness round): `group_liveness` must drive `collapsed` and wait for the morph to settle BEFORE its pill assertion.** As written, any group that ends in an expanded state poisons the following pill check, which makes the whole suite's exit code unreliable — and the user's gate is "≥1000 tests pass", so a false FAIL here would block the gate for the wrong reason. Also note `run_all.sh` hard-requires `ffmpeg ffprobe ss` in PATH even though the ffmpeg capture path was proven unusable; that is a spurious dependency for the socket-only groups.

## Round-1 critic panel plan (20 slots, ready to fire the moment r1a settles)

Launch as **4 staggered sub-waves of 5** — provider capacity, not t3's 20-thread cap, is the binding limit. Every slot gets the same brief plus the canonical gate table and the current `measure.py` JSON, and every slot must run the instrument itself rather than trust the fixer's report. Read-only; strict JSON contract.

Model rotation (banned: `magpie/fasfas-ai/*`; `gpt-6.1-sol` reserved for 2 slots only, per the user's cost warning):
1 `magpie/logfare/deepseek-v4.1-flash-1017` · 2 `magpie/logfare/glm-5.3` · 3 `magpie/group/deepseek-v4.1-flash-1017` · 4 `magpie/logfare/glm-5.3-flash` · repeat. Slots **5** and **12** = `magpie/agentrouter/gpt-6.1-sol`.

The 20 lenses (slot: lens):
1 glyph sharpness vs reference (lapvar AND stroke_over_cap together)
2 **screenshot A/B against `/tmp/ref/s/s17.png` — gpt-6.1-sol**
3 screen-relative geometry (gap_top_pct, width_pct, height_pct_screen)
4 typeface identity + weight (Inter-Medium.ttf asset actually loaded?)
5 capsule fill / frost material / AA edge quality
6 idle CPU + redraw-count proof
7 dictation-rate redraw storm (F3)
8 RSS ≤30 MB + shm slot sizing + icon-font laziness
9 debug logging removed from hot path (F4)
10 glyph cache correctness (bounded, keyed on px bits, no stale coverage)
11 **does-it-reproduce: run `scripts/stress/run_all.sh`, ≥1000 tests — gpt-6.1-sol**
12 motion smoothness via `scripts/motion/run_all.sh` + honest caveats
13 workspace discs + tray preserved and correct (A4 glyph code point)
14 no AI slop (one accent, no gradient/glow/bloom — `docs/slop-audit.md`)
15 config coherence (`bar_height` ignored by collapsed height at `src/main.rs:201`)
16 no startup panic / font-missing degradation path
17 socket protocol literals unchanged (dictation still works)
18 regression risk in the other seven panels
19 Rust idiom / clippy / no new warnings
20 build reproducibility `--offline`, no new crates, tree hygiene

Each visual slot MUST produce at least one finding of the form "compared to the reference frame, X is off because Y, do Z" with a `where`. The skill's Sleek reference `magpie-image-mcp` is unreachable without `SLEEK_API_KEY`, so **the extracted video frames are the substituted reference** — recorded here as the run's reference-of-record.

### RSS root cause measured (this is the r1b headline, and it is growing)

RSS climbed 50.5 → 57.5 → 84 → **86 MB** over 11 minutes of idle-ish uptime. `/proc/<pid>/smaps_rollup`: `Rss 86000 kB, Pss 59387, Pss_Anon 32548, Pss_Shmem 23548`. Largest mappings by Rss:
- **47096 kB `/memfd:smithay-client-toolkit`** ← the shm buffer pool, ~55% of RSS
- **29412 kB `[heap]`**
- 3300 kB the binary itself

So the dominant cost is the **stage-sized shm pool**, exactly as predicted: stage 1176×636 logical × RENDER_SCALE 2 ⇒ 2352×1272 device × 4 B = 11.96 MB per slot, and the pool holds several slots (47 MB ≈ 4 slots) because buffers are recreated when a state outgrows them (lockscreen fullscreen) and old slots are not released. Plus a ~29 MB heap (icon font parse ~19 MB + glyph caches).

**r1b must therefore: (a) size the shm pool to the actual maximum surface a state needs instead of the fixed 1176×636 stage for every state; (b) release/replace slots rather than accumulating them; (c) skip `upscale_canvas_2x_into` entirely on the settled-collapsed path (it currently runs over the whole stage at `src/main.rs:4878` and is then overwritten — pure waste); (d) throttle `dictation_bars:` redraws; (e) keep the icon font lazy and parsed once.** Gate RSS ≤30 MB is unreachable without (a)+(b) — the two shm fixes alone should recover ~30 MB.

Note `Pss_Shmem 23548 kB` vs `Rss 47096 kB` on the memfd: the compositor maps the same buffers, so **half the shm RSS is shared with the compositor** — a critic quoting raw RSS should also quote Pss, or it will overstate the panel's private cost. Private_Dirty is 32548 kB ≈ 32 MB, which is the honest "RAM this process owns" figure and is already close to the ≤30 MB gate.

### Round-1 panel launched early (slots 1 and 3) — deliberate deviation, recorded

The skill says launch the whole panel after the fixer settles. r1a has been `running` ~35 min with the binary unchanged since 22:09, and provider capacity is intermittent, so waiting risks losing the whole evening's review window. Slots **1 (glyph sharpness)** and **3 (screen-relative geometry)** were dispatched now against the 22:09 binary because both lenses judge properties already visible in that build and neither depends on the fixer's remaining edits. Each brief names the known-open items so they are not re-reported as new. Remaining 18 slots still wait for r1a to settle.

**Deviation risk, stated plainly:** if r1a changes the glyph path or geometry after these two verdicts land, their scores describe a superseded binary and must be re-run, not counted. The log will mark them stale if the tree moves.

Slot 1 → `magpie/logfare/deepseek-v4.1-flash-1017`, uuid `9f26162d-5941-4add-8a4b-b60add124b78`.
Slot 3 → `magpie/logfare/glm-5.3`, uuid `14253e1e-73e8-4c9a-bc4e-73b4cfbec92a`.

Wave 1 continued — slots **8 (memory/shm)** → `magpie/group/deepseek-v4.1-flash-1017` uuid `71976faf-f6e1-4a79-872e-472ead43952e`, and **6 (CPU/redraw discipline)** → `magpie/logfare/glm-5.3-flash` uuid `d2fc3dd5-4470-49a8-8e7d-90d7dba7a446`. Four slots now in flight (1, 3, 6, 8), which is the wave size the provider tolerates. All four judge properties present in the 22:09 binary; none depends on r1a's remaining edits. Each brief carries the measured root causes so the critic verifies rather than redisovers, plus the `liveness` false-FAIL warning and the never-send-`quit`/`lockscreen` rule.

### r1 critic #3 — geometry — `magpie/logfare/glm-5.3` — **score 3** (wake chain 4t)
Credible, strongest verdict so far: independently re-ran `measure.py` and re-measured the reference, reproducing the parent's numbers exactly, then traced the gap error end-to-end.
- **blocker** gap_top ~4× too small. Mechanism proven: `fn stage_margin_top()` `src/main.rs:750` = `(gap_top − STAGE_PAD_TOP).max(0)`, so `gap_top=3` collapses to `STAGE_PAD_TOP=3` logical (`src/main.rs:459`) = 4.5 device → measured y=5. Fix is config-only: `gap_top=13` → 19.5 device → 1.30%.
- **major** width ~12% too wide: `collapsed_width=74` → 111 nominal / 109 measured (≈1.473 measured-per-config after AA loss). Target 4.336% of 2256 = 97.8 device → **66 logical**.
- **major, NEW and stronger than the parent's version** `bar_height` is not merely ignored — `parse_config` `src/main.rs:308` has **no `bar_height` arm**, so the key falls into `_ => {}` and never reaches the struct. The settings UI's Bar-height control is a dead knob. Hardcoded 28 at `src/main.rs:201`.
- **major** the three size tolerances are mutually infeasible on this 3:2 screen. **This is a specification error by the parent, not a defect in the work** — see the gate revision below.
- **minor** exclusive zone `pill_strip_px()` `src/main.rs:763` = `gap_top+28`, applied `src/main.rs:5774`, re-applied `:6096`. Two staleness edges: reload only re-applies margin+zone when `state == Collapsed` (`src/main.rs:6090`); and if 28 becomes configurable, `pill_strip_px`, `PANEL_TOP_MENU` `:464`, `WS_CY` `:845`, `TRAY_CY` `:955` all go stale → windows tile over the pill's bottom sliver.
- **minor** confirms aspect 2.659 ✓ and offset −1.0 ✓ today, but warns aspect passes *only because the pill is too wide*.

### GATE REVISION (parent specification error, corrected — chain 4t)
The parent copied the reference's percent-of-screen ratios from a **16:9** panel onto a **3:2** panel without checking that the implied capsule aspect survives the screen's own aspect change. It does not: 4.34% of 2256 = 97.8 device wide and 2.92% of 1504 = 43.9 tall imply aspect **2.23**, outside the 2.65 band. Percent-of-height is the wrong invariant across screen aspect ratios.
**Revised gate:** `aspect 2.65 ±0.10` **HARD** (capsule shape is the most perceptually salient property) · `width_pct 4.34 ±0.20` **HARD** (a bar's visual weight reads against screen width; the reference's own settings panel expresses design in absolute px, not percentages) · `gap_top_pct 1.32 ±0.2` **HARD** (vertical gap is uncoupled from capsule shape) · `height_pct_screen` becomes **DERIVED — report, do not gate** (~2.49 on this panel).
**Consequence the critic surfaced indirectly:** holding aspect 2.65 at width 66 logical requires height ≈ **25 logical**, not 28. So honouring the dead `bar_height` key is the **enabling fix**, not cosmetic polish. Target config for this screen: **`collapsed_width=66`, `bar_height=25`, `gap_top=13`** → predicted width 97/2256 = 4.30%, aspect ≈2.69, gap 1.30%.
**Additional parent finding for r1b:** `TEXT_PX` is a hardcoded constant (`src/lock_timer.rs:115`, 14.4) tuned for a 28-logical pill. At height 25 it breaks the typography gate. Text size must be **derived from pill height** (cap = 0.357 × pill_h), not a constant, or every geometry change re-breaks typography.

### r1 critic #6 — CPU — **FAILED**: `AiError: Service temporarily at capacity`
Provider capacity, not model fault. Retry on a different route per the run's swap rule (`magpie/logfare/glm-5.3-flash` → `magpie/logfare/deepseek-v4.1-flash-1017`).
Live measurement taken during the stress wave: **RSS 113.5 MB, CPU 105%** — the CPU figure is confounded by critic #6 driving `drive_bars.py` at the time, but the RSS climb (50.5 → 57.5 → 86 → 113.5) is monotonic across the round and is a real regression handed to critic #8.
Retry dispatched: r1 critic **#6 CPU** → `magpie/logfare/deepseek-v4.1-flash-1017` uuid `9d3831f3-1cb1-40d2-93af-72b2f6fe07ef` (route swapped off glm-5.3-flash). Brief extended with a 5th mechanism found by reading the diff: `rasterize_cached` returns `cov.clone()` on every cache hit, so the cache removes rasterisation but still heap-allocates + memcpy's the coverage bitmap per glyph per frame, and the map keyed on float bits is unbounded.
r1 critic **#4 typeface** dispatched → `magpie/logfare/glm-5.3-flash` uuid `15af01b7-5d44-47fb-a3e3-d01e01f62ff2`. Central question put to it: is `assets/fonts/Inter-Medium.ttf` actually loaded, or does the code still read the variable `~/.local/share/fonts/Inter.ttf` (which would render Regular 400 @ opsz 14, stroke/cap ≈0.143)? Brief carries the lapvar-is-not-fidelity trap and the TEXT_PX-must-derive-from-pill-height finding. Panel in flight: #1 sharpness, #4 typeface, #6 CPU retry, #8 memory.
r1 critic **#2 screenshot A/B** dispatched → `magpie/logfare/glm-5.3` uuid `0d3786f2-7088-43d4-932d-da330a5a78f6`. Must crop + 5-6× nearest upscale both images and VIEW them, not just read numbers. Brief pre-empts all four known traps (lapvar≠fidelity, 3:2-vs-16:9 gate revision, tray icon is the real magpie app icon not a bug, liveness false-FAIL) and forbids recommending removal of the owner's workspace discs / Bluetooth cluster. Panel in flight: #1, #2, #4, #6, #8.
r1 critic **#5 fill/AA/slop** dispatched → `magpie/group/deepseek-v4.1-flash-1017` uuid `c5c5a1f8-961c-4116-8544-64630e050679`. Carries the reference's exact edge ramps (left 210→160→20→2→0; top 168→19→1), the full `docs/slop-audit.md` hit list AND its protected list, the one-accent ladder, and the instruction to prefer shared-token fixes. Panel in flight: #1, #2, #4, #5, #6, #8 (6 of 20).
r1 critic **#11 stress/does-it-reproduce** dispatched → `magpie/logfare/deepseek-v4.1-flash-1017` uuid `b2c21d72-2f59-4121-b627-6b4018fba47c`. Told the suite's exit code must be trustworthy: a suite that cannot fail correctly caps at 5/10 regardless of claimed test count, and "1000 tests" that only check datagram acceptance is a blocker not a pass. Panel in flight: #1, #2, #4, #5, #6, #8, #11 (7 of 20).
r1 critic **#19 rust idiom/build health** dispatched → `magpie/logfare/glm-5.3-flash` uuid `32226f75-6d2f-46c6-9cb5-aaa7f04a1157`. Brief targets the glyph cache's double-clone + unbounded float-keyed map + silently-swallowed poisoned mutex, the stale TEXT_PX module docs, panic-reachability under `panic="abort"`, and warns it not to re-report the 62 pre-existing warnings. Panel in flight: #1, #2, #4, #5, #6, #8, #11, #19 (8 of 20).

### Derived target values for r1b (parent arithmetic, from critic #3's measured factor)
`grep -n "bar_height" src/*.rs` → **zero hits**: the key is not merely unparsed, it appears nowhere in the Rust source. `IslandState::size()` `src/main.rs:201` is `Self::Collapsed => (cfg().collapsed_width, 28)`.
Measured-per-config factor **1.473** (critic #3: config 74 → 111 nominal → 109 measured, ≈2 px AA edge loss).
- `collapsed_width = 66` → 99 nominal − 2 AA = **97 device** → width_pct **4.30%** ✓ (band 4.34 ±0.20)
- `bar_height = 25` → 37.5 nominal − 1 AA = **36.5 device** → aspect **97/36.5 = 2.658** ✓ (band 2.65 ±0.10)
- `gap_top = 13` → 19.5 device, measured y≈20 → gap_top_pct **1.33%** ✓ (band 1.32 ±0.2)
- Derived height_pct_screen = 36.5/1504 = **2.43%** — reported, not gated.
- **TEXT_PX must be derived, not 14.4**: cap = 0.357 × pill_h_logical = 0.357 × 25 = 8.93 logical; Inter digit cap = 1510/2048 em, so `px = cap × 2048/1510` = **≈12.1**. Formula to implement: `text_px = pill_h as f32 * 0.357 * (2048.0/1510.0)`. At the current 28 this yields 13.56 (vs the hardcoded 14.4, which is why cap/pill measured 0.39 instead of 0.357).
r1 critic **#18 panel regressions** dispatched → `magpie/logfare/glm-5.3` uuid `68a26d04-ed7a-49c5-8644-ea30b9181a64`. Highest-value hypothesis handed to it: **the glyph cache is keyed `(char, px.to_bits())` with NO font identity** — if the icon font (MaterialSymbolsRounded) shares `rasterize_cached`, any codepoint present in both fonts returns the wrong glyph → blocker. Also morph sharpness jump (2× settled vs 1× morph), stage/slot bounds for the nine fixed panel sizes, and Inter-is-narrower-than-Quicksand layout fit. Panel in flight: #1, #2, #4, #5, #6, #8, #11, #18, #19 (9 of 20).

### r1 critic #8 — memory — `magpie/group/deepseek-v4.1-flash-1017` — **score 3** (wake chain 4t)
Strongest verdict of the round. It **corrected a stale premise in the parent's brief** rather than accepting it: `load_icon_font` (`src/main.rs:1899-1912`) now parses a **3,904 B, 15-glyph subset** `icons/MaterialSymbolsRounded-Island.ttf` behind `icon_font_for` (`src/main.rs:1915-1931`) + OnceLock, and returns **None for Collapsed/Lock/Dictation** — so the ~19 MB Material parse is NOT in this heap. **Parent's F6 icon-font premise is retracted.**
Arithmetic proof of the shm mapping: observed **70,468 KiB** = 2×23,547 KiB (lockscreen slabs) + 2×11,686 KiB (stage slabs) = **70,466 KiB**, within 2 KiB.
Measured at rest: **Rss 109.8 / Pss 72.5 / Private_Dirty 34.7 / Pss_Anon 34.7 / Pss_Shmem 35.2 / Pss_File 3.4 MB**. Idle is **flat** (110.796 → 110.752 MB over 90 s) → the growth is **per state transition, not per frame**. `run_all.sh --group load_leak` (200 cycles): RSS 72 → 119 MB, curve `72 105 119 118 119 118 118 119 119 119`, **growth_after_warmup +20.9 MB vs its own 8 MB bound → FAIL**. Controlled 3× lock/unlock: +9.7 MB heap, pool stays 68.8 MB, no reclaim.
- **blocker** `SlotPool::alloc` (`smithay-client-toolkit-0.19.2/src/shm/slot.rs:200-225`) grows by `max(rv+size, len*2)` and **never shrinks**; `draw_if_ready` only grows (`src/main.rs:4762-4775`, `needed` at `:4768`; initial pool `:5783-5790`). Fix: hold 2 pinned `Slot`s, `pool.create_buffer_in(&slot[i],..)`, skip the tick when `pool.canvas(&slot)` is None, delete the recreate, assert `pool.len() <= 2*slot_bytes`. 22.8 MB instead of 68.8 → RSS ~65, Pss ~49.
- **blocker** collapsed pill pays **11,966,464 B** of shm for **72,160 B** of real pixels (**166×**, 98.7% transparent). `STAGE_W/STAGE_H` `src/main.rs:466,471`; `RENDER_SCALE` `:483`; `draw()` `:2312`; `draw_collapsed_pill_2x` `:2208`. Fix: a dedicated 110×41 collapsed surface (72 KiB buffer), stage surface created on first menu open / dropped on last close. **Constraint: the stage is never resized because per-tick `set_size` broke morphs — iter24 rule, `src/main.rs:930-932`.** One configure round-trip per collapse, never during a morph.
- **major** **no frame-callback back-pressure**: `surface.frame` never requested, `fn frame` `src/main.rs:4920` is an **empty body**; commit path `src/main.rs:4893-4898` keeps `self.buffer = Some(buffer)` alive. Pool settles at **4 slots idle, 6 under load**. This is the mechanism that makes a pinned 2-slot ring correct rather than racy.
- **major** lockscreen slabs **23,547 KiB each**, never reclaimed: `set_size` to `LOCKSCREEN_SIZE` `src/main.rs:4136-4140` / `:1775`; `finish_unlock` `src/main.rs:4229-4260` frees wallpaper + `scale_scratch` + `malloc_trim` but **never touches `self.pool`**. Fix: `self.pool = None` there → recovers 46 MB post-lock.
- **major** the gate metric is wrong — see below.
- **minor** glyph cache unbounded, clones per hit, **`GLYPH_CACHE_CAP` does not exist in the tree** (`src/lock_timer.rs:207-230`, callers `src/main.rs:1839`, `:2251`). Fix: round the key `(ch,(px*2.0).round() as u32)`, cap ~256, or hold `Arc<[u8]>`.
- **minor** `damage_buffer(0,0,bw,bh)` `src/main.rs:4895` dirties all 11.4 MB every frame (~342 MB/s at 30 fps), keeping every slab resident.
- **minor** per-frame heap allocs: `vec![0u8; pw*ph*4]` at `src/main.rs:2332` (783 KB ControlCenter, 1.09 MB Wallpaper), `:2223`, `:2286`, `:2956`. Reuse one grow-only scratch like `scale_scratch`.
- **minor** heap floor unexplained: 23.5 MB idle / 33.2 MB locked + a constant **6.6 MB anon** block. Attributable: morph scratch pair 2×2.99 MB freed **only on the done-into-Collapsed arm** (`src/main.rs:4853-4862`, `:3539`), `scale_scratch` 2.99 MB, live backdrop 752×501×3 = 1.13 MB.

### GATE METRIC CORRECTION (parent, chain 4t)
RSS double-charges the shm memfd because the compositor maps the identical pages (`Pss_Shmem` 35.2 = exactly half of 68.8). **Restate the memory gate as `Pss ≤ 30 MB`, keep reporting RSS alongside.** This is NOT a relaxation that rescues a failing gate: Pss 72.5 fails by 42 MB and even `Private_Dirty` 34.7 fails by 4.7 MB, so **no metric choice passes today**. The heap floor alone (23.5–25.1 MB + 6.6 MB anon) exceeds the whole budget → buffer sizing alone cannot meet the gate.

### FIX ORDER for the memory work (parent, chain 4t — order is load-bearing)
1. Frame-callback gating (makes a pinned ring safe, not racy) → 2. pin 2 slots, delete grow-only recreate → 3. `self.pool = None` in `finish_unlock` → 4. narrow damage rects → 5. separate collapsed surface (biggest win, riskiest; respects the iter24 no-per-tick-resize rule).

### PARENT SELF-CORRECTION — icon-font collision hypothesis RETRACTED (chain 3t)
I briefed critic #18 that the glyph cache's font-less key would return wrong glyphs for the icon font. **Wrong**: `grep rasterize_cached src/*.rs` → only `src/lock_timer.rs:252` and `src/main.rs:1839`; the icon font never reaches the cache. Read #18's verdict on that point sceptically.
**Bigger real finding surfaced by the same grep:** there are **SIX independent copies of `fn layout_text(font,text,px) -> TextRow`** — `src/main.rs:1835`, `src/calendar_panel.rs:416`, `src/clipboard_panel.rs:1580`, `src/controlcenter_panel.rs:2539`, `src/launcher_panel.rs:802`, `src/music_panel.rs:1479`. **Only the main.rs copy got the cache.** F5 is **one-sixth fixed**; the expanded panels, which have far more text than the five-glyph clock, got no benefit. **Fix order matters: add font identity to the cache key FIRST, then share it** — the naive sharing introduces the collision blocker I wrongly predicted as already-present.

### Dispatch pacing — parent error
Critic **#4 typeface FAILED**: `rate limiting: inference request per min rate reached`. Cause is **my own pacing**: 9 children in flight violated the 3–4 wave rule set after the first outage. **Hold new dispatches until the panel drains**, then re-dispatch #4. In flight: #1, #2, #5, #6, #11, #18, #19. Done: #3 (3), #8 (3). Failed: #4 (retry pending), #6-original (retried).

### MEMORY GATE HARDENED (parent measurement + chain 3t, 22:38)
Fresh process **pid 31420** (fixer rebuilt again): **Rss 61,008 kB / Pss 46,231 / Private_Dirty 31,208 / Pss_Anon 31,208 / Pss_Shmem 11,688 / Pss_File 3,335**. Largest mappings: memfd **23,376 kB = exactly 2 stage slabs**, `[heap]` 17,880 kB, anon **6,784 kB**, anon 2,924 kB, binary text 2,824 kB.
**None of critic #8's fixes have landed** — `grep -n "create_buffer_in\|new_slot\|pool = None\|surface.frame\|pending_frame" src/main.rs` → **zero hits**; `fn frame` still an empty body. The improvement 109.8 → 59.6 MB is **not a fix**: this process has never entered the lockscreen since restart, so the pool sits at its floor (2 stage slabs) instead of its post-lock ceiling (2 stage + 2 lockscreen = 68.8 MB).
**Both readings are correct and measure different points on a monotonic curve.** This is exactly the never-shrinks defect critic #8 identified, seen from the other end.
**GATE HARDENED: `Pss ≤ 30 MB` must be measured AFTER a lock/unlock cycle AND after `run_all.sh --group load_leak`. A fresh-process reading does not count** — otherwise the gate is passable by simply never locking the screen.
**Independent confirmation of the heap floor:** `Private_Dirty` is **31,208 kB even on a virgin process with shm at its floor** — over the 30 MB budget before any shm work. The **6,784 kB anonymous mapping is present at startup**, so it is allocated once at init, not accumulated; that narrows the fixer's hunt.
Typography unchanged by these rebuilds: lapvar **11,968.7**, stroke/cap **0.188**, cap/pill 0.39, margins 27/27 symmetric ✓, aspect 2.683 ✓, offset −0.5 ✓, gap_top_pct **0.332 ✗**, width_pct **4.876 ✗**. RSS 59.6 MB, CPU 3.0%.
Tree: `src/main.rs` +181/−39, `src/lock_timer.rs` +34/−6.

### CRITICAL INFRASTRUCTURE FINDING — glm routes are IMAGE-BLIND at runtime (chain 4t)
Critic **#2 screenshot A/B FAILED**: `502: model "logfare/glm-5.3" can't see images, and logfare/glm-5.3-flash couldn't describe the image for it: rate limiting: inference request per min rate reached`.
Two facts: (1) `~/.pi/agent/models.json` declares `input=['text','image']` for **every** magpie model including glm-5.3 — **the local catalogue is aspirational, the gateway is authoritative**; (2) pi routes images through a **describer** model (glm-5.3-flash) that converts the picture to TEXT before the critic sees it. So any "visual" verdict from a glm slot is **text-mediated, not seen**. The user explicitly demanded critics judge "screenshots that display the real life" — a described screenshot does not satisfy that.
**Blast radius:** #2 (A/B) and #5 (fill/AA/slop) are the compromised slots. #3 geometry, #8 memory, #18 regressions, #19 idiom are numeric/code-only → **unaffected, scores stand**.
**Rule for the rest of the run: no glm route on a visual slot.** Vision-capable candidates from the catalogue: `magpie/group/deepseek-v4.1-flash-1017` (in 0.3 / out 1.2), `magpie/group/gemini-4-hydrogen`, `magpie/group/hy4` (the council skill's own named critics — they DO exist in the local catalogue, contrary to the earlier assumption), `magpie/agentrouter/gpt-6.1-sol` (in 1 / out 5, user-authorised for harsh screenshot critique).
**Do not trust the catalogue again — it already lied once.** Dispatching a **vision capability probe** on `gpt-6.1-sol` with checkable ground truth (reference pill 106×42 at y=19 vs live 110×41 at y=5; live sits visibly closer to the top edge, reference glyphs heavier). A route that returns the right pixel facts sees; a plausible description with wrong numbers is blind.
#5's verdict is **provisional pending vision confirmation** when it lands.

### r1 #2 re-dispatched as VISION PROBE + A/B review — `magpie/agentrouter/gpt-6.1-sol` uuid `1770af68` (thinking high)
Brief requires five checkable pixel facts obtained BY LOOKING before any review (clock text in each image; which pill hugs the top edge closer; which digits are heavier; what is left of the pill in each; shape of the object right of the live pill), with an explicit instruction to declare blindness honestly rather than invent. Ground truth held by the parent. Then the full A/B fidelity lens with nearest-neighbour magnified crops. This doubles as the run's vision-capability test for the expensive route the user authorised.

### r1 critic #1 — sharpness — `magpie/logfare/deepseek-v4.1-flash-1017` — **score 3** (wake chain 4t)
Numeric half credible and independently corroborated: **lapvar 12014.4, lapmean 89.49, stroke/cap 0.188, cap/pill 0.39** — matches the parent's own reading. Correctly identifies the overshoot as **thin-stroke contrast, not fidelity**.
- **major** clock too thin and too large vs reference; fix = wire `assets/fonts/Inter-Medium.ttf` (wght 550 / opsz 20) and derive px from pill height.
- **major** independently confirms **six copies of `layout_text`** (`src/main.rs:1835`, `src/calendar_panel.rs:416`, `src/clipboard_panel.rs:1580`, `src/controlcenter_panel.rs:2539`, `src/launcher_panel.rs:802`, `src/music_panel.rs:1479`) — only main.rs cached.
- **minor** confirms `GLYPH_CACHE_CAP` absent, unbounded float-bit key, clone-per-hit.
- **REJECTED architectural claim** "the buffer_scale route is a dead end on mango; compositor 2→1.5 downsampling caps sharpness". **Counter-evidence:** the reference is itself a screen recording through a compositor resample AND a lossy h264 encode, and measures lapvar **6503**; ours measures **12014**, i.e. **1.85× sharper**. If compositor downsampling were the dominant softening term ours could not exceed it. The measurement shows a **weight/size mismatch**, not a resample ceiling. The underlying SCTK-0.19-has-no-fractional-scale constraint is real (verified separately by the mango worker), but this critic did not prove it is the binding limit here.
- **Cost warning recorded:** raising `RENDER_SCALE` to 3 costs **2.25× the pixels**, directly fighting the already-failing `Pss ≤ 30 MB` gate. Do not trade a metric that is above target for one that is failing, on an unproven premise.

### STANDING INSTRUCTION TO ALL FUTURE CRITICS (parent, chain 4t)
The correct typography fix **LOWERS lapvar** (12014 → toward the reference's 6503) while raising stroke/cap 0.188 → 0.200 and lowering cap/pill 0.39 → 0.357. **A falling lapvar accompanied by stroke/cap converging on 0.200 is PROGRESS, not regression.** Score the pair together, never lapvar alone.

### HARNESS AUDIT (parent, chain 3t) — the "≥1000 tests" gate is not yet honestly meetable
`scripts/stress/stress.py` has **5 groups** (`GROUPS` at `:891-897`: protocol_fuzz, state_machine, load_leak, latency, liveness) but only **15 `out.tests +=` sites**. Volume comes from loops, not assertions:
- rapid-fire `:607-615` sends **4 × 1000 = 4000 datagrams** but counts as **4 tests**;
- `group_load` `:746` counts **200** (one per cycle, `iters=200`);
- `group_state` drives 2 lock cycles × 8 words, a 60-iteration `locktype` saturation burst counted as **1 test** (`:711`), a dictation cycle, and **40 seeded-random transitions** (`_Rng(0xC0FFEE)`, `:726`).
**So raw sends already exceed 1000, but the assertion count is far lower.** The user asked for ≥1000 *tests*; 4000 sends asserting 15 things is not that. Meeting the gate requires **per-iteration assertions inside the existing loops**, not new groups.

**FALSE-FAIL ROOT CAUSE, now with exact lines.** `def group_liveness(app, out)` `scripts/stress/stress.py:837` sends `themes` at **`:859`** to time responsiveness, then calls `app.check_pill("liveness")` at **`:868`** with **no `collapsed` + settle in between** — so it asserts pill geometry while the 599 px Themes panel is on screen. **The harness penalises the app for correctly obeying the command.** One-line fix owed to r1b: after `:865`, send `collapsed` and sleep ≈0.4 s before `check_pill`.
**Not fixed by the parent now:** critic #11 is actively running the suite; editing the harness mid-run would invalidate its verdict. `scripts/` is outside the fixer's `src/` scope, so the fix must be assigned explicitly.

### VISION PROBE PASSED — `magpie/agentrouter/gpt-6.1-sol` genuinely SEES (chain 4t)
Critic **#2 A/B re-dispatch, score 5/10.** All five ground-truth facts correct **by looking**: A=`00:05`, B=`22:41`; B hugs the top edge closer; **A's digits heavier**; four numbered discs left of B with **2 highlighted lavender**, nothing left of A; tray glyph = "silver/white, curved U-shaped bottom, two upward wing-like prongs" — **exactly the magpie bird logo**, independently confirming the earlier A4 retraction. Correctly reported **no standalone AA or vertical-centring defect** (matches margins 27/27, offset −0.5).
→ **`magpie/agentrouter/gpt-6.1-sol` is the run's CONFIRMED vision route.** Use for visual slots, sparingly (user cost warning). glm routes remain banned on visual slots. #5 stays provisional.
Its findings: **major** capsule hugs the edge (target ~20 device px / 13.3 logical, move discs+tray with it); **major** clock too tall and too thin (cap ≈ ⅓ capsule height ≈14 device, stroke ≈ ⅕ cap); **major** retained controls look bolted on — discs have **broad soft dark halos and fuzzy numerals**, tray is pictorial/shaded beside a crisp monochrome clock → flatten disc shadows, rasterize disc labels with the clock's treatment, flat two-tone tray, active workspace = flat fill not luminous halo; **minor** capsule slightly too elongated.
Score spread noted: holistic visual lens 5 vs gate lenses 3 — not a contradiction; the object is right in kind, wrong in proportion.

### PARENT RETRACTION #3 — F1 "wrong typeface" was UNPROVEN (chain 4t)
Tested properly for the first time: **shape-correlation of rendered glyph bitmaps against the reference's actual glyph pixels**, not scalar ratios. Reference capsule `x[1227,1332] y[19,60]` 106×42, ink **15×54**; both normalised zero-mean/unit-variance, candidates LANCZOS-resized, mean product.
On `00:05`: **Quicksand-Bold 0.8780 (px24)** · Inter-Medium 550/20 **0.8345 (px21)** · Inter-var 400 **0.7333 (px22)** · Quicksand regular 0.5153. **The font F1 ordered removed is the closest match.**
**Not conclusive, and no reverse order either:** at cap 15, Quicksand-Bold px21 gives w/cap **3.600** and stroke/cap **0.200** — an *exact* match on both, identical to Inter-Medium px20. Two faces reproducing both ratios exactly proves **the ratios were never discriminative**. Cross-string control: `11:29` → all fonts 0.07–0.14; `15:39` → 0.11–0.19; `88:88` → 0.56–0.68. A 10× swing with the string means the method is dominated by digit shape/alignment, so 0.878 vs 0.835 is inside its noise.
**Honest conclusion (negative):** there is **no evidence Quicksand-Bold is the wrong face**. F1 was asserted from a vibe plus a width ratio confounded by comparing **different clock strings** (`22:41` live vs `00:05` reference) — the same confound critic #2 independently flagged.
**Surviving, string-independent defects:** cap/pill **0.39 vs 0.357** (too large) · stroke/cap **0.188 vs 0.200** (too thin) · gap_top **0.332% vs 1.32%** · width_pct **4.88 vs 4.34**. All four are **size and position; none is typeface identity**.
**Standing instruction:** either face is acceptable — **do not churn fonts**. Keep Inter-Medium (already wired, matches both ratios exactly). Gate on cap/pill and stroke/cap; **gate width only on identical strings or drop it**. No critic may score a font swap as progress.

### r1 pre-dispatch chain for r1b (5t) — focus and root cause
**Root cause behind almost every finding:** the panel allocates and renders for its **worst case** and never gives anything back — fixed 1176×636 stage for a 110×41 pill (166× pixel waste), full-buffer damage per frame, no frame-callback back-pressure (4–6 slots in flight), `SlotPool` grows only (one lockscreen visit = 46 MB permanent).
**Enabling defect:** `bar_height` has no `parse_config` arm, so capsule height is frozen at 28 and the revised geometry gate (aspect 2.65 at width 66) is **unreachable** without honouring it.
**r1b scope:** memory structure + derived geometry + weight/asset + accessory rendering parity. Coherent because every item is "stop rendering the small thing as if it were the big thing".
**Two traps, fix ORDER is load-bearing:**
1. Separate collapsed surface collides with the **iter24 rule** (`src/main.rs:930-932`: per-tick `set_size` broke morphs). Order: frame-callback gating → pin 2 slots → `pool=None` on unlock → narrow damage → separate surface.
2. Glyph cache keyed `(char, px_bits)` with **no font identity**; six `layout_text` copies, only one cached. Order: **font identity in the key FIRST**, then share.
**Must not touch:** socket literals (live dictation supervisor); tray icon path (A4 false premise — real magpie app icon); **typeface identity** (retraction #3: no evidence Quicksand was wrong); `RENDER_SCALE=3` (2.25× pixels vs failing Pss gate); `scripts/stress/*` (now correct, `d92301a`); the 58 baseline warnings.
**Do load `assets/fonts/Inter-Medium.ttf`** — matches stroke/cap 0.200 and w/cap 3.600 exactly vs Inter Regular's 0.125. That is a **weight** fix, not a typeface change.

### r1 STRESS GATE — first honest full-suite run (parent)
After the nine-defect harness repair (`d92301a`) plus the load_leak drain fix:
`TOTAL tests=1207 asserts=5595 fails=321 groups=5` → the 321 were **all EAGAIN flow-control phantoms** in `group_load`, fixed by routing it through `send_drained` like every other group. Re-run of that group alone: **`tests=200 asserts=1000 fails=0`, PASS**, `growth_after_warmup −2.2 MB`, `peak_delta +0.0` (bound 8 MB).
Per-group: protocol_fuzz PASS 636 tests / 4350 asserts · state_machine PASS 246 · load_leak PASS 200 / 1000 asserts · latency PASS 120 (`p50 200.3 p95 220.7 p99 254.6 max 286.0`, bound_p95 400) · liveness PASS 5.
**The user's ≥1000-test gate is now genuinely measurable and met on assertions, not sends.** An earlier `latency` run showed p95 432 ms > 400 ms; the clean re-run reads 220.7 ms, so that reading was taken while another critic was driving the socket — noted, not a standing defect.
**Memory gate measured correctly (after lock/unlock):** Rss 114620 kB, **Pss 76313 kB**, Private_Dirty 37696 kB, Pss_Shmem 35234 kB → fails ≤30 MB by 46 MB. This is the number r1b must move.

### r1b FIXER DISPATCHED (pre-dispatch chain 5t)
[council fixer r1b](t3-thread://v2/699b4b1c-474b-4871-82f1-d1eb0c374204/thread:delegated-task:command%3Amcp%3A95fc24c9-420c-4506-a56f-1d554907f77c%3Adelegate-task%3A48b2e5b6-d603-4b81-aaad-f69ee3a72324) — `magpie/group/deepseek-v4.1-flash-1017`, thinking **max**.
Brief carries F-A..F-H with the two fix-order traps, the derived-geometry targets (`gap_top=13 collapsed_width=66 bar_height=25`), the derived `text_px = pill_h*0.357*(2048/1510)`, the baked Inter-Medium asset, the six-copy glyph-cache sharing order, the accessory 2× parity work, and the explicit "lapvar will FALL and that is PROGRESS" instruction. Verification requires a post-lock/unlock Pss reading and a full 5-group suite run.

### r1 merged state (parent) — panel scores so far
| slot | lens | model | score |
|---|---|---|---|
| #1 | glyph sharpness | logfare/deepseek-v4.1-flash-1017 | 3 |
| #2 | screenshot A/B (vision) | agentrouter/gpt-6.1-sol | 5 |
| #3 | screen geometry | logfare/glm-5.3 | 3 |
| #8 | memory | group/deepseek-v4.1-flash-1017 | 3 |
| #11 | stress / does-it-reproduce | logfare/deepseek-v4.1-flash-1017 | 3 |
Median of the five in: **3**. Gate (≥9) **not met**; round 1 continues with fixer r1b.
Failed slots: #4 typeface (provider RPM, retried `1f5555b9`), #18 regressions `68a26d04`, #19 idiom `32226f75-6d2f-46c6-9cb5-aaa7f04a1157`. Running: #5 fill/slop `c5c5a1f8`, #6 cpu `9d3831f3`.

### Instrument repair #3 (parent) — commit `3a6fabd`
`scripts/stress/stress.py` had **`group_load` and `group_latency` defined TWICE** (byte-identical copies, verified with a difflib comparison before deleting). Python silently keeps the last definition, so the first copy of each was dead code — 100 lines. Removed. This is why a harness needs its own review: three separate instrument defects (degenerate detector, str/bytes compare, duplicate defs) were invisible to every panel slot that only read `src/`.

### Geometry arithmetic for F-D (parent, verified)
Measured-per-config factor **1.473** (74 logical → 111 nominal → 109 measured; ~2 px lost to the AA ramp).
- `collapsed_width=66` → 99 nominal → ~97 measured → **97/2256 = 4.30%** ✓ (target 4.34 ±0.20)
- `bar_height=25` → 37.5 nominal → ~36 measured → **aspect 97/36 = 2.69** ✓ (target 2.65 ±0.10)
- `gap_top=13` → 19.5 device → y≈20 → **20/1504 = 1.33%** ✓ (target 1.32 ±0.2)
- `text_px = pill_h × 0.357 × 2048/1510` = 25 × 0.357 × 1.3563 = **12.10** logical (Inter digit cap 1510/2048 em)

### Provider failures — pattern and rule (parent, chains 3t + 4t)
- **#4 typeface failed TWICE**: first `rate limiting: inference request per min rate reached`, then `Service temporarily at capacity`, both on `magpie/logfare/glm-5.3-flash`. Both are provider transport/capacity errors, not model refusals; the brief never got far enough to be at fault. **Swapped to `magpie/group/deepseek-v4.1-flash-1017`** (3 prior successes, cheap, the user's requested model), same brief → `da1de8dc`. Did NOT spend a `gpt-6.1-sol` slot here: the lens is already well measured and sol is reserved for the decisive visual gate.
- **Lockscreen-spec child died on the vision failure**: `model "logfare/glm-5.3" can't see images, and logfare/glm-5.3-flash couldn't describe the image for it: … Service temporarily at capacity`. **This was the parent's error** — the glm-is-image-blind rule was already established and I put a fundamentally visual task on a glm route anyway. Re-dispatched on the confirmed vision route `magpie/agentrouter/gpt-6.1-sol` → `3aa6938c`.
- Every capacity failure in this run clustered while 5–6 children were in flight. **Rule reinforced: the binding constraint is concurrent provider load, not t3's 20-thread cap.**

### Lock screen — transcript facts recovered before the child died (committed `docs/lockscreen-transcript-facts.md`)
The author states, and transcript outranks pixel guesses:
1. Font is **SF Compact Display** (he walked SF Pro Text → SF Pro Display → SF Compact Display).
2. The lock **clock surface is translucent**, wallpaper-tinted, with a **frosted blur behind it and a shadow** — *not* white text on a flat veil. **The collapsed pill is a different material** (luminance 0–2 inside, genuinely black). Do not merge them.
3. **No avatar / username / password box at rest**; the password field appears only when typing starts and disappears on Esc.
4. Wrong password **shakes**.
5. **The island morphs into the keyhole and descends**, then the clock pops out.
**This contradicts the parent's frame reading** (avatar ring y1137–1168, `user0` y1225–1242, `Enter Password` y1289–1302 on l60/l320). Resolution: those frames were almost certainly captured **while typing**. A spec built from them would bake a transient state into the resting design. The re-dispatched child must separate resting from typing with evidence. The dead child flagged this conflict honestly instead of deferring to the brief — good work, preserved.

### Socket contract frozen (committed `docs/socket-contract.md`)
The dictation tool is a **separate live binary** (`static-dictation`, built from `~/.t3/scratch/2026-11-13-can-you-please-like-i-e1185665/`) that talks to the panel only through the socket. Exact literals it sends: `dictation_start` (`src/main.rs:38`), `dictation_end` (`:42`), `dictation_bars:<v> …` (`:132`, values `{v:.3}` space-separated, **one datagram per microphone frame**), `collapsed` (`src/pill/tsar.rs:129`). Renaming or reparsing these silently breaks a user-visible feature. `DBG morph A -> B` stderr lines are also a contract (the harness's effect witness reads them); the per-packet `DBG socket word=` line is not.

### ✅ STRESS GATE MET — full suite green on assertions, not sends
`python3 scripts/stress/stress.py` (all 5 groups) → **`TOTAL tests=1207 asserts=5595 fails=0 groups=5` / `RESULT PASS` / `EXIT=0`**.
The user's "≥1000 tests before going to the next round" is satisfied on **genuine assertions (5595)**, which is the honest reading — the raw send count was always higher but most sends asserted nothing.
Per group: protocol_fuzz 636 tests / 4350 asserts · state_machine 246 · load_leak 200 / 1000 (`growth_after_warmup −2.2 MB`, `peak_delta +0.0`, bound 8 MB) · latency 120 (`p50 200.3 p95 220.7 p99 254.6 max 286.0`, bound 400) · liveness 5.

### Instrument repair #4 (parent, chain 3t) — commit `cfffbab`
`measure.py` returned **pill 220×220, aspect 1.0, gap 0.0%, width 9.75%, cpu 104%** while the stress suite was driving `group_load`. **Not a panel regression** — `pill_bbox` scans y 0..220 in a ±110 window and takes min/max across all passing rows, so a fullscreen dark surface starting at y=0 (lockscreen / dark app) yields w=220 h=220 exactly. Same class of defect critic #11 found in the stress detector, at a different scale: robust against a dark page *below* the bar, not against one *starting at* y=0.
Fix: when `y0 == 0` and the run spans the whole window, emit `state: "not-collapsed"` with a note and **no geometry/sharpness gates**. Five critics quote these numbers into their verdicts, so a detector that silently scores a lockscreen as a pill would poison the panel median. Re-measured in a quiet window: pill 110×41, aspect 2.683, offset −0.5, lapvar 11663.2 — unchanged.

### r1b progress observed mid-flight (parent read the diff, no edits)
Landed so far: `bar_height` is now a real config key (`clamp(18,80)`, default 28) feeding `IslandState::Collapsed => (cfg().collapsed_width, cfg().bar_height)` and `pill_strip_px() = gap_top + bar_height`; `TEXT_PX`/`CLOCK_PX` constants **replaced by `pub fn text_px(pill_h: f32) -> f32 { pill_h * 0.357 * (2048.0/1510.0) }`** with the stale 17.0 module docs rewritten; glyph cache key now **(font identity, char, px bits)** with `Arc<[u8]>` values (refcount bump, not memcpy) — the F-F ordering was respected; `clock_font()` now searches exe-relative `assets/fonts/Inter-Medium.ttf` before the variable Inter and Quicksand, never panics.
**Not yet landed:** frame-callback gating (`fn frame` at `src/main.rs:4945` is still an empty body; no `surface.frame`, `new_slot`, `create_buffer_in`, `pending_frame` anywhere). That is step 1 of the F-A order, so the memory work is still ahead.
**Minor portability note for the next round:** `clock_font()` hardcodes `/home/alex/rust-island-build/assets/fonts/Inter-Medium.ttf` as a fallback candidate (`src/main.rs:1896`) — fine on this machine, wrong for a packaged install.

### r1 critic #4 — typeface — `magpie/group/deepseek-v4.1-flash-1017` (swap after 2 glm deaths) — **score 7** (wake chain 4t)
First score above 3, and **not generosity**: the critic independently re-derived the bake instead of trusting it — FreeType stroke/cap **0.128** (variable default) vs **0.170** (static) at cap 47; `usWeightClass 550`, `STAT opsz 20`; `'0'` ink **1128 units wide vs 1048** at identical height (yMax 1511 vs 1510) → real added stem weight, not a renamed 400. It also **verified the F-D trap I warned about**: every `draw_panel_into` caller passes the pill's `(pw,ph)`, so `text_px` never receives `STAGE_H=636`.
Both retractions respected: **no typeface-identity claim**, and **no tracking defect** — same-string static w/cap 3.37 vs reference 54/15 = 3.60 ±0.19 is inside quantisation, and `-0.4` tracking would move *away* from the reference. **Leave tracking at 0.**
Fallback risk closed: `draw_text_centred` uses `match`/`None => return`, no reachable unwrap on the clock path under `panic="abort"`.
The 7 vs the other slots' 3 is **not a contradiction**: #4 scored *source correctness*, the 3s scored *the old binary's pixels*. It docks two majors for **delivery**, not logic. Do not let this 7 imply the pixels improved.

**Findings (verbatim severity):**
- **major** — the weight fix existed only as an **untracked file**; `git ls-files assets` empty, `check-ignore` exit 1 (never added, not ignored). Silent fallthrough to wght 400/opsz 14 on a clean checkout. → **FIXED BY PARENT, commit `217693d`** (verified both commands myself before acting). Binary asset, no conflict with the fixer's `src/` edits.
- **major** — fix **unverified in pixels**: pid 19318 predates the edits; live still cap/pill 0.390, stroke/cap 0.125, lapvar 11663. Needs one post-rebuild capture.
- **minor** — `src/lock_timer.rs:50-73` still teaches Quicksand / upem 1000 / sCapHeight 700 / "default instance wght 300" / "must be 17.0". **This false premise is what hid the thin-stroke bug for several iterations.** Delete it.
- **minor** — `text_px()` has **no test**; it is the sole determinant of cap/pill across `bar_height` 18..80 (px 8.7..38.7). Pin `text_px(24)==11.62`, `text_px(28)==13.56`.
- **minor** — hardcoded `/home/alex/...` asset fallback (`src/main.rs:1893-1895`) and **no log of which candidate won**, so "thin variable" and "intended 550/20" are indistinguishable in logs.

**Instrument corroboration from an independent direction:** #4 saw `measure.py` return `pill: null` (Lock state on screen from the stress suite) and then merge the pill into a dark window. Same fragility as my repair #4. **Panel-wide rule going forward: visual captures must be taken on a quiet screen, collapsed state, against a BRIGHT wallpaper** — otherwise slot numbers are not comparable across rounds.

### Smoothness evidence pipeline dispatched (the user's headline ask)
[smoothness evidence pipeline (real recording)](t3-thread://v2/699b4b1c-474b-4871-82f1-d1eb0c374204/thread:delegated-task:command%3Amcp%3A95fc24c9-420c-4506-a56f-1d554907f77c%3Adelegate-task%3A06ef4440-0773-4e36-bf2f-2cc10265613b) — `group/deepseek-v4.1-flash-1017`, thinking high.
The user asked for **screen recordings split into still frames** with numbers that go up. Parent probed the capture paths first so the child does not flail: `wf-recorder`/`wl-screenrec`/`obs`/`gpu-screen-recorder` **absent**; ffmpeg **has kmsgrab** and the user is in `video`, but it fails `No handle set on framebuffer: maybe you need some additional capabilities?` → needs `CAP_SYS_ADMIN`; no screencast portal, no PipeWire cast nodes. Child may `setcap` ffmpeg (**must remove it again** — leaving it is a security regression the user never asked for) or install `wf-recorder` from the Artix repos; falling back to the grim loop silently is forbidden.
New metric it must add: **motion completeness** — how many distinct intermediate poses a morph renders. A morph drawing 3 poses in 300 ms is not smooth even with zero duplicated frames, and that is the most likely explanation of the user's "laggy". Existing `analyze.py` already flags duplicates/spikes and already states the grim caveat (intervals measure grim, not the compositor); the child must **extend it, not fork it**, and must prove the analyser against synthetic sequences with known stalls (`selftest.py`).

### Instrument repair #5 (parent, chain 4t) — commit after `217693d`
Critic #4 independently hit the case repair #4 **missed**: a dark app window neither starts at y=0 nor fills the ±110 scan window, so it merges into the pill bbox and inflates w/h. **Height cannot discriminate** — `bar_height` clamps to 80 logical = 120 device px, so a legitimately tall pill and a dark window overlap in size. The discriminator is **material**: p25 luminance of the whole box.

**Two errors the self-test caught before shipping** (this is why the guard needed its own test):
1. Sampling the left/right 12% padding reads the **stadium's rounded cap** — wallpaper through the corners — and refused a genuine pill at 5.0. Use p25 of the **whole box**: glyph ink is the bright minority, so p25 is fill.
2. Threshold 4 refused the **live** capsule, whose fill measures **exactly 5.0** while the reference measures 0–2. Both are the capsule; dark UI themes sit at 18–30 → bound is **12**.
Also renamed `for a, b in zip(...)` which **shadowed the image array `a`** — a latent `IndexError: invalid index to scalar variable` on any frame with a gap-split row.

`scripts/measure_selftest.py` — 5 assertions, all PASS: reference scores · synthetic capsule scores · dark window refused · **merged window+capsule refused** · fullscreen-from-y0 refused.

### Canonical gate table — same instrument, same crop, both sides
`python3 scripts/measure.py --png /tmp/ref/s/s17.png` vs live. **Critics must quote these, not their own ad-hoc crops.**
| metric | REFERENCE | LIVE (old binary, pid predates r1b) | direction |
|---|---|---|---|
| pill w×h | 111×42 | 110×41 | — |
| aspect | 2.643 | 2.683 | ✓ already close |
| centre offset | 0.0 | −0.5 | ✓ |
| ink cap h | 15 | 16 | ↓ |
| **cap_over_pill_h** | **0.357** | **0.390** | ↓ |
| **stroke_over_cap** | **0.200** | **0.125** | ↑ |
| text_over_pill_w | 0.486 | 0.523 | ↓ |
| margin L / R | 28 / 29 | 27 / 25 | → symmetric |
| **lapvar** | **6503.2** | **11704.5** | ↓ toward 6503 |
| fill luminance | 0–2 | **5.0** | ↓ (minor, real) |

**New minor finding (parent):** the live capsule fill is **luminance 5**, the reference is **0–2**. Not black enough. Cheap fix, and it is a *material* difference a screenshot critic can see.

**Panel-wide capture recipe (mandatory for visual slots):** quiet screen · `collapsed` state · **bright wallpaper** · then `python3 scripts/measure.py`. If it returns `state:"not-collapsed"`, **do not quote geometry** — drive `collapsed` and re-measure. Numbers taken against a dark window are not comparable across slots or rounds.

## Four more r1 verdicts (wake chains 7t batched)

| slot | lens | model | score |
|---|---|---|---|
| #1 | glyph sharpness | logfare/deepseek-v4.1-flash-1017 | **6** (was 3) |
| #5 | fill / frost / slop | group/deepseek-v4.1-flash-1017 | **6** |
| #6 | idle CPU + redraw proof | logfare/deepseek-v4.1-flash-1017 | **4** |
| #11 | stress / does-it-reproduce | logfare/deepseek-v4.1-flash-1017 | **3** |

Eight slots in: #1 6 · #2 5 · #3 3 · #4 7 · #5 6 · #6 4 · #8 3 · #11 3 → **median 4.5**. Gate ≥9 **not met**.
Failed again: **#18** panel regressions (glm image-blind + RPM), **#19** rust idiom (RPM). Both are provider failures, not refusals.

### ★ #6 CPU is the first critic to EXPLAIN "laggy" with a mechanism — this is the r1c headline
Three blockers that compose into one story:
1. **`src/main.rs:6001` redraws the full stage once per microphone packet** — no dirty flag, no coalescing, no bars comparison. Measured: offered 401 packets in 4.0 s at 100 Hz, panel **accepted only 64**, 51 sends blocked >5 ms, **103.0% of one core** = **64.5 ms CPU per accepted packet**. The renderer back-pressures the microphone stream itself — a correctness bug, not just CPU.
2. **`upscale_canvas_2x_into` (`src/main.rs:504-539`) costs 30.2/31.7 ms** (rustc -O; gcc -O2 51.5/57.4). The critic **extracted the loop verbatim and benchmarked it without cargo**, honouring the lock I imposed. 2,991,744 dst px, 47.9 M strided reads, **23.9 M integer divisions per call**. Its doc (`src/main.rs:499`) claims ~5 ms and its guard (`src/main.rs:7851`) asserts **<200 ms** — a 40×-too-loose budget let a 6–11× regression ship green. memset of the same dst is 0.70 ms, so copy is not the cost.
3. **Idle CPU is NOT the draw loop.** `probe_loop` (`src/wallpaper_blur.rs:447-478`) is an unconditional 1 Hz `grim` spawn: **13 spawns / 20 s at 150–311 ms CPU each = 9.75% of a core from children alone**; tree total 19.8–24.6% vs the 1.5% gate. Main thread is only 0.65% idle and the 1 Hz timer is properly fingerprint-guarded. **This invalidates the parent's earlier assumption that idle CPU was redraw-driven.**

**The composition:** on the settled collapsed path **99.723% of that 30 ms upscale is thrown away** — `draw_collapsed_pill_2x` repaints the whole pill rect over it, and the pill is **8,288 px of 2,991,744**. So the fix is *not* "optimise the upscale" but **"do not upscale what you are about to overwrite"**. Per-packet parsing is cheap (~11 µs: 600 bars/s with no redraw = 8.25% vs 4.25% idle), so **coalescing, not parser surgery**. The empty `CompositorHandler::frame` (`src/main.rs:4923`) is the general fix that makes the dictation case a one-liner.
Also from #6: 35 `DBG eprintln` sites with no env gate, `word.trim().to_string()` allocates per packet before validation; static discs/tray re-rasterised per packet (`WS_DISC_SS = 8`, ~136k `disc_shadow_alpha_r` calls/redraw for unchanging output); full-buffer `damage_buffer(0,0,bw,bh)` + a fresh `wl_region` per redraw. **Its negative results are valuable too:** minflt +0 per draw, RSS flat 65→65 MB over 10 draws — smithay's SlotPool reuses freelist slots, so per-frame buffers are **not** mmap'ing and the unbounded-glyph-map worry is unfounded (px is constant).

### ★ #1 + #5 converged INDEPENDENTLY on one root cause: only the settled pill got the 2× path
- **#1:** morph frames and the menu-open pill fall back to 1×. `draw_collapsed_pill_2x` runs only under `if self.morph.is_none() && state == Collapsed` (`src/main.rs:4883-4885`). Two other paths draw the same clock at 1×: `draw_state_into` Collapsed branch (`src/main.rs:2947-2961`) and `draw_pill_over_stage` (`src/main.rs:2276`, called :2357/:2363/:3223). **Measured lapvar 12100 → 2087.5 with a menu open — a 5.8× crispness collapse on every expand**, plus a visible weight change.
- **#5:** the discs and tray are still 1×-bilinear-doubled beside a razor-sharp pill — pill edge 2 device px (66→33→5) vs disc edge 4–5 px (86→51→26). **Two sharpness classes on one row.**
**One fix, not two findings**, and the user sees it on every interaction.

### #1 settles the weight question quantitatively (independent of #4)
Simulated the identical pipeline (2× raster → 0.75 box downsample): **Regular@28.8px → lapvar 8300 / lapmean 79 / soc 0.188**; **Inter-Medium@28.8px → lapvar 6358 / lapmean 81.15 / soc 0.219** vs reference **6503.2 / 80.89 / 0.200**. The baked asset is confirmed correct by a second method, and these are the predicted post-rebuild numbers.
**New instrument caveat — lapvar is amplitude-confounded:** live ink clips at 255 (33.8% of px ≥240), the reference peaks at **225.7** and never reaches 255 (video tone-limited). Clamping live to ~225 drops lapvar **12100 → 9436**; contrast-normalised excess is **~1.46×, not 1.86×**. `measure.py` must report a contrast-normalised lapvar or every sharpness score stays confounded. **Owed.**
Also #1: cap centre sits 0.33 logical px high (`src/lock_timer.rs:259-262` baseline centring); glyph blit uses **per-channel `max()` instead of alpha-over** (`src/lock_timer.rs:276-280`) — invisible on today's dark frost, but it erases the AA ramp wherever the backdrop is brighter than the coverage value. Latent, and the same idiom is in every panel's text path.

### #11's blockers are partly STALE — parent verified in the tree, do not forward them
#11 ran `run_all.sh --group latency` twice and got `pill width 599px … x[828,1427]`. **That defect is already fixed** (commit `d92301a`): `pill_metrics` now takes the widest contiguous dark run through centre and rejects window-edge runs — the comment in the file describes that exact min/max bug. Unknown-group validation exists (`scripts/stress/stress.py:1058-1061`), `MIN_ASSERTS = 1000` is enforced (`:1139`). Its blockers 1 and 5 and part of 4 are **stale**; forwarding them would waste a round.
**Still live, verified by the parent:** `scripts/stress/run_all.sh:20` still checks `ffmpeg ffprobe` upfront although only `pill_metrics` uses them.
**Its counting critique is substantively right and I must not defend the headline:** ~345 protocol tests assert only that `sendto` returned ok, and 200 load "tests" are RSS samples behind **one** 8 MB assertion. My assert counter inherits whatever the code labels an assertion. So **"5595 assertions" ≠ "5595 meaningful checks"**. The ≥1000 gate stands on the instrument's own accounting, but the morning report must state its composition, and the next harness round must **assert per-iteration, not per-batch**.
Also live from #11: `check_pill` drops the `(rc, ms)` return and relies on `sleep(0.45)`; no `trap`/cleanup so the last group decides the next gate's state; latency measures **send→eprintln**, not send→presented frame (p99 swung 596→308 ms between identical runs).

### #5's design findings (the user's "no AI slop" requirement)
- **blocker** — dictation is still teal bloom + the codebase's only gradient, verified live: fill (20,24,34) vs pill (2,2,2), aura ~20 device px, `GLOW_K 3.2 / GLOW_REACH 22 / BLOOM 0.32 / GLOW_IDLE 0.18`, `wave_color()` over `GRAD_L(125,165,165) → GRAD_C(140,220,220) → GRAD_R(80,118,178)`. **The reference has no waveform, glow or gradient at all — this is invented.** (`src/dictation_canvas.rs:126-132, :400-588, :148`)
- **major** — the spec's single accent **#F2A65A appears zero times** in src/docs/assets; the live active disc is #93A4F5 and that hue is hand-typed 4 more times. `library::ACCENT`/`theme_accent()` exist but drive none of them, **so theming cannot work**.
- **major** — type ladder is #FFFFFF/#C0C4C9 instead of #E8E8E8/#8B8FA0; secondary ~40% too bright, flattening hierarchy; one role split across five greys.
- **major** — discs cast a shadow the shadowless pill doesn't have (−29% cloud, 4–5 px), and `RIM_LIGHT` is drawn but **measurably invisible** (137,152,215 vs fill 135,150,224).
- **major** — `WS_CY`/`TRAY_CY` still use the old `STAGE_PAD_TOP + 12.0` while `size()` returns 28 → **discs and tray ride 2.5 device px above the pill's centre line**. Needs one shared `COLLAPSED_H`.
- **minor** — surface can stay pinned at x=0 after launch (observed 22:32–22:35), and `stage_screen_origin()`'s hardcoded `(1504-1176)/2 = 164` then mis-samples every frost by 246 device px.
**#5 independently hit the same measure.py dark-window defect** ("bogus 74x36, aspect 2.056") and worked around it by direct pixel measurement — third independent confirmation, now fixed by repair #5.

### Instrument repair #6 (parent, chain 5t) — commit `0456c59`
Three changes, each traceable to a critic finding **verified in the tree first**:
1. **Contrast-normalised lapvar** (from #1). Raw lapvar scales with the **square** of contrast; live ink clips at 255, the video reference peaks at 225.7. Added `lapvar_norm`/`lapmean_norm` + `ink_peak`/`ink_span`. **Reference 1277.0 vs live 1872.73 = 1.47×**, matching #1's independent clamp estimate (~1.46×) and replacing the misleading 1.86×. Every future sharpness score must quote the normalised figure.
2. **Per-iteration assertions** (from #11). 200 load "tests" sat behind **one** growth bound; 120 latency tests behind two. Now every post-warmup RSS sample and every latency sample is asserted, so a failure **names its iteration**. Full-suite asserts **5595 → 5897**.
3. **PIL decode in `pill_metrics`**, ffmpeg as fallback; `run_all.sh` no longer requires ffmpeg/ffprobe up front. Verified with a **shadow PATH containing no ffmpeg**: latency ran clean and located the pill (w=105).

### ⚠️ Gate honesty — the ≥1000 claim, stated with its composition
The suite reports **5897 assertions**, but they are not all the same kind. Breakdown for the morning report: protocol robustness (malformed/near-miss/compound words accepted without dying — a real check under `panic="abort"`), state-transition morph witnesses, and now per-iteration resource bounds. **#11's critique stands in part**: a per-batch aggregate is weaker than a per-iteration check. Fixed for load and latency; the protocol batches still assert per-send acceptance rather than per-send pixel state. **Do not present 5897 as 5895 independent behavioural proofs.**

### ⚠️ Latency p95 FAILED on the full-suite run — and it is contention, not a regression
`GROUP latency FAIL tests=120 fails=1` · `p95 411.9 ms exceeds 400 ms bound` (p50 228.9, p99 628.1, max 726.4).
**Same binary, same group, minutes earlier in isolation: p95 271.0, p99 305.5, max 312.2 — PASS.**
Cause measured at the moment of failure: **loadavg 7.49** with `rtk 95.5%`, **`rustc 94.2%` (the fixer's build)**, **`ffmpeg 83.9%` (the smoothness child decoding frames)**, `dynamite-rust 60.7%`, `magpie-desktop 23.7%`.
**This is exactly the confound #11 warned about** ("p95 sits at 71–80% of its bound on a contended box; timing-lucky rather than robust"). Recording it as a FAIL, **not** silently re-running until green and **not** relaxing the bound. The honest reading: the panel's accept latency has no headroom under load, which is itself consistent with #6's uncoalesced-redraw root cause. **Re-measure in a quiet window after the fixer's build lands** before scoring it.

### Lock-screen spec returned (worker `3aa6938c`, vision route `gpt-6.1-sol`) — chain 6t
**It overturned the parent's own frame reading. Accepted after independent verification.**
`docs/lockscreen-spec.md` (33 KB) classifies every supplied frame. **`l60.png` / `l320.png` are the OLD macOS-style COMPARISON IMAGE the author shows to explain what he is NOT building** — not a state of the final design at all.
Parent's verification, not the child's claim: `l40.png` has **one** ink band, y[581,858], h=278, w=871, cx=1279 — a single giant centred clock. `l60.png` has a **different structure**: bands y[174,190] h=17 and y[234,373], ink reaching **x=2559, the right screen edge**. Two layouts, not two states of one layout.
**Why this matters more than a normal correction:** my recorded "avatar ring y1137–1168, `user0` y1225–1242, `Enter Password` y1289–1302" was measured on the *foil*. A builder given that would have replicated the wrong screen **and it would have looked plausible**. The transcript fact "no avatar at rest" and the pixels now agree from two independent directions.
- **Resting** = `l40`/`l400`: centred clock + small top padlock capsule, nothing else.
- **Typing** = `l120`: dots appear **inside the top capsule**. Still no avatar, username, date or bottom field.
- Measurements @2560×1440: clock ink **872×279** centred ~(1280,720) — **34% of screen width**, vs the pill's 4%. A different scale regime. Resting capsule **68×40 at y=58**; padlock ink 14×17 at (1273,69); password dots **8–9 px dia, ~16.2 px pitch**.
- **The clock is wallpaper-sampled frosting INSIDE THE GLYPHS** — not white, and **not a frosted rectangular panel**. Parent verified: 134,887 ink px, **median RGB exactly [87,235,206] = `#57EBCE`**, **92.8% teal-ish, 0.7% near-neutral**, varying across x (left third [127,221,194] vs mid/right [87,235,206]). Recipe: **glyph-only Gaussian blur 24 px**, **1.030× centred wallpaper zoom**, **vertical (not radial) black veil**; gradient residual −0.11%, RGB RMSE **2.39/255**. **A frosted card would be both wrong and an AI-slop tell.**
- Dense frames confirm capsule descent, clock emergence/downward travel, slight overshoot.
**Two parent documents corrected** (they would otherwise misdirect the next fixer): `docs/lockscreen-transcript-facts.md` fact 2 rewritten and the conflict section marked RESOLVED with the comparison-image evidence; `docs/slop-audit.md` F7 rewritten — `src/lockscreen.rs:199` is `#AAF9FE`, **pale not saturated**, and the defect is the **material**, so the fix is **not white**. Any flat fill is wrong. Not a two-hue-drift tell: the hue is sampled from the wallpaper.
**Still unresolved, marked in the spec:** original shader/true alpha, exact font binary/weight/tracking, shadow parameters, settled password padding, exact easing and shake waveform.
**Carried into r1c:** lock-screen replication is an explicit user ask and is now specified well enough to build. The brief must state the corrected premise, because a fixer reading the old doc builds the wrong screen.

### Parent-proved perf candidate: `scripts/perf/upscale_bench.rs` (chain 6t)
Critic #6's headline was prose ("the upscale costs 30 ms"). A fixer given prose produces a plausible change I then have to re-measure. So the parent **extracted both implementations, compiled standalone with `rustc -O`, and proved a candidate** — no crate, no Cargo change, **no `src/` edit** (the fixer owns `src/`).
**Bit-exactness is the gate, not speed.** The current code rounds the horizontal blend to an integer *before* the vertical blend; fusing into one 16-weight sum shifts output by ±1. A faster upscale that changes pixels is a regression.
Result — **bit-exact at all 7 sizes** (1×1, 1×7, 7×1, 2×2, 3×5, 17×13, 64×48, random premultiplied data, exercising both edge clamps):
| size | current | hoisted | speedup |
|---|---|---|---|
| full stage 1176×636 | **62.10 ms** | **38.61 ms** | **1.61×** |
| pill only 110×41 | 388.00 µs | 242.79 µs | 1.60× |
`RESULT PASS`. Reproduce: `rustc -O -o /tmp/upscale_bench scripts/perf/upscale_bench.rs && /tmp/upscale_bench`.
**The candidate hoists the horizontal blends out of the inner loop**: for a given output row the two source rows are fixed, and the horizontal weights depend only on `dx%2`, so each source row's two phase-blends are computed once and cached across the two output rows that share it. Same double-rounding order ⇒ bit-exact by construction.
**Honest caveat: 1.61× is not the 6× the doc comment implies is available.** Absolute numbers here are inflated by contention (loadavg 3.21, fixer building); the *ratio* is the trustworthy part. And the **pill-only row is the real lesson**: 388 µs vs 62 ms is **160×**, so critic #6's "don't upscale what you're about to overwrite" beats any loop optimisation by two orders of magnitude. **r1c should do the skip-region fix first and treat the hoist as a bonus.**

### Score trend ledger: `scripts/scorecard.py` + `docs/scorecard.jsonl`
The owner asked to *see the numbers go up*. A number with no history is not a trend, so every measurement is now appended with the binary's mtime, so a row can never be mistaken for a different build. Refuses to record a frame `measure.py` calls `not-collapsed` — a lie in the trend is worse than a gap.
Two bugs caught by running it, not by reading it: (a) it compared each live row to the **previous row**, which for the first live row is the *reference* — so every improvement printed as "further"; fixed by separating `kind:"ref"` from `kind:"live"` and reporting **gap-to-reference** plus **live→live progress**. (b) `proc_stats()` is merged at the **top level** of measure.py's JSON, not under a `"proc"` key, so `rss_mb`/`cpu_pct` were silently dropped from every row.
| metric | round start | live r1b mid-flight | reference | gap now |
|---|---|---|---|---|
| aspect | 4.029 | **2.553** | 2.643 | 3.4% |
| cap_over_pill_h | 0.486 | **0.342** | 0.357 | 4.2% |
| stroke_over_cap | — | **0.154** | 0.200 | 23.0% |
| lapvar_norm | — | **1619.8** | 1277.0 | 26.8% |
| RSS | 44.2 MB | **33.7 MB** | — | — |
**Real progress**: aspect 4.029 → 2.553 and cap ratio 0.486 → 0.342 are large moves toward the reference. Weight (23% off) is now the dominant remaining gap, exactly as critic #1's simulation predicted.

### ⚠️ Parent self-correction #6 — my derived config target was wrong on the height axis
I instructed r1b to set `collapsed_width=66 bar_height=25`, derived from critic #3's measured factor **1.473**. Live result: **97×38**, not the ~111×42 intended. **My arithmetic, not the fixer's execution.**
Root cause: width factor measures 1.4697 but height 1.5200. The true scale is **1.5 on both axes** — 25×1.5=37.5→38 (rounds up), 66×1.5=99→97 (**loses 2 px**). The difference is entirely the dark-pixel detector's edge behaviour on a **stadium**: rounded caps lose more dark columns than flat top/bottom lose rows. Critic #3's 1.473 was that same artefact.
**Lesson, recorded so no later round repeats it: derive config from the known scale 1.5, never from a detector-derived factor.**
**Exact-integer solution I failed to test:** 111/1.5 = **74** and 42/1.5 = **28** exactly ⇒ **`collapsed_width=74 bar_height=28`** should give 111×42 before edge effects (~109×42 after). Alternatives 75×27→110×41 (aspect 2.683) and 76×28→112×43 (2.605) are both ~0.04 off.
**Not applied now**: the fixer is mid-flight and changing the live config under it would confound its own measurements — the same rule I applied to the stress harness. Handed to r1c as a measured instruction: *set 74×28, then measure, do not compute*.

### ✅ Deliverable: dictation embedded in the package (chain 6t) — commit `5bb1fd5`
The owner asked for the dictation tool to be "embedded nicely into the package". It was living in `~/.t3/scratch/2026-11-13-can-you-please-like-i-e1185665/` — **a dated scratch directory**, 1.3 GB with build artefacts, launched by `~/.config/mango/autostart.sh` pointing at that scratch path. It worked only because the temp folder was never cleaned. **Clearing scratch would have killed dictation** and made the panel's dictation state unreachable. A real deliverable gap, not a nicety.
**Chose a plain subdirectory over a Cargo workspace, deliberately:** the panel uses `smithay-client-toolkit 0.19`, this crate uses **0.21.1** plus `cpal` and `reqwest`+`rustls-tls`. A shared workspace unifies the lockfile and could force version resolution onto a working panel binary — risk to a working build for cosmetic gain. Separate manifests keep the builds independent.
- Copied `src/ scripts/ Cargo.toml Cargo.lock README.md` → `tools/dictation/` (**29 files, 448 KB**; `target/` excluded by the existing `.gitignore`).
- **Offline build verified from the new location: 0 errors, 38 warnings, 132 crates**, `target/release/static-dictation` 3.4 MB. All 7 direct deps were already in the local registry cache.
- **Secrets checked before copying:** the Groq key comes from `GROQ_API_KEY` or `~/.config/static-dictation/api_key`, never the source tree. `grep -rlE 'gsk_[A-Za-z0-9]{20}'` → no hits. No audio/transcripts copied.
- Supervisor now resolves its binary **relative to its own location**, prefers the in-repo build, **falls back to the scratch binary** so a failed rebuild never leaves the owner without dictation.
- `~/.config/mango/autostart.sh` repointed to the in-repo supervisor; previous version preserved as `autostart.sh.bak-dictation-embed-<timestamp>`; `bash -n` clean. **The running supervisor was not restarted**, so this session is undisturbed — the new path takes effect at next login.
- **Contract preserved:** all four literals (`dictation_start`, `dictation_end`, `dictation_bars:…`, `collapsed`) are byte-identical in the copy and **all four were sent to the live panel and accepted** after the migration.
Details: `tools/dictation/EMBEDDING.md`.

### Instrument repair #7 — the motion captures contain NO animation (chain 6t)
Read the smoothness child's outputs before letting anyone score them. **They are dead trials.**
`scripts/motion/out/20261118-213403/morph.txt`: *"MORPH WINDOW n/a — the trigger command FAILED to send; this capture contains no animation"*, all **80 deltas 0**, and `bars_driver.log`: **`driver: cannot send: [Errno 107] Transport endpoint is not connected`**. ENOTCONN on an AF_UNIX dgram sendto means the path exists but nothing is bound — **the panel was restarting** while the fixer rebuilt. The capture raced a redeploy.
**A dead capture printed a tidy summary and exited 0**, so it read as data. Three fixes, all in `scripts/` (the fixer owns `src/`):
1. `scripts/motion/analyze.py` now **returns exit 3** with `RESULT FAIL: the trigger never reached the panel … must not be scored` when the trigger failed. Verified: real exit code **3** (not 0 — the first check was masked by a pipe).
2. `scripts/motion/capture.sh` now writes **`crop_applied_by_recorder=1`**. `grim -g` already crops, but only `record.sh` declared it, so a caller also passing `--region` would **double-crop and slice the wrong pixels**. Latent, not the cause of this run.
3. `scripts/motion/run_all.sh` **derives the crop from the live pill** instead of hardcoding `640,0,900,210`. Derived now: **`552,0,952,210`**. Falls back to the old window on any error rather than aborting.
**Self-correction #7 during this chain:** I first read `CROP device px 0,0 -> 450,225` against `region=600,0,900,150` as a wrong-X bug. **It is not** — 450×225 device = 300×150 logical = exactly the region, i.e. the whole pre-cropped frame. Checked before reporting; fourth time verification changed my conclusion.
**Real defect that remains:** the "settled" control shows **median nonzero delta 13.834 and 4 spikes on a pill that cannot move**. The old 260-logical-px window around a 65-px pill is **~75% wallpaper**, inflating the noise floor enough to mask a genuine stall. Fix 3 addresses it.
`scripts/motion/selftest.py` still **PASS** (perfect 60 Hz 10-pose morph = 100, static = nothing, ordering perfect > duplicate > slideshow > teleport).
**Do not score any existing motion output.** Re-run in a quiet window; loadavg was 4.19 with a `rustc` running at the time of writing.

### Instrument repair #8 — dark wallpaper failed 138 assertions (chain 6t)
Full suite returned **`TOTAL tests=1207 asserts=5897 fails=138`**, every group failing with the same line: `no pill edge contrast (outside dark=0.95) | x[1018,1193] y[0,85] w=175`.
**Not a panel regression.** The reported box starts at y=0 and is 175 wide; the real pill is x1079 y19 w98 h37. Measured the actual pixels: the **wallpaper is now dark** — p25 = **21** on both sides of the pill, mean luminance 24–26 across y0..45. The detector's `DARK = 40` therefore classified the wallpaper itself as capsule:
| threshold | inside dark | outside dark |
|---|---|---|
| 40 | 0.83 | **0.90** ← indistinguishable |
| 12 | 0.79 | **0.00** ← clean |
**Fix:** `DARK 40 → 12` (capsule interior p25 = **2**; a dark UI theme sits at 18–30, so 12 separates both), plus the same **`y0 == 0` spanning-window refusal** measure.py already had.
**New `scripts/stress/detector_selftest.py`** — 4 synthetic ground-truth frames, all PASS: bright wallpaper finds the capsule · **dark wallpaper still separates** · fullscreen dark refused · dark window below the bar not merged. The detector has now been wrong three times in ways that produced confident false verdicts; each regression is pinned so it cannot return silently.
**Re-run after the fix: `TOTAL tests=1207 asserts=5897 fails=0 groups=5` / `RESULT PASS`.** Latency **p50 86.8 / p95 98.5 / p99 187.6 / max 188.7** against a 400 ms bound — the earlier 411.9 ms failure is confirmed as contention, now with a 4× margin. RSS flat 62 MB across 200 lock/unlock cycles, growth **−0.0 MB**.

### ✅ r1b's frame-callback back-pressure WORKS — measured, not claimed
Critic #6's blocker was 401 packets offered → **64 accepted, 51 blocked sends, 103% of one core**. Re-ran the identical probe on the new binary:
| | before (critic #6) | now |
|---|---|---|
| offered | 401 in 4.0 s | 400 in 4.09 s |
| accepted | **64** | **400** |
| blocked sends | **51** | **0** |
| CPU | **103.0%** | **57.7%** |
The mic stream is no longer back-pressured. Mechanism verified in the tree: `draw_if_ready` records `redraw_wanted = true` and returns while `frame_pending` is outstanding (`src/main.rs:5108-5114`), `surface.frame()` is requested after every commit (`src/main.rs:5376`), and `CompositorHandler::frame` repaints only if a wish was recorded (`src/main.rs:5417-5421`). A 250 ms cap guards a compositor that never acks a hidden surface.
**Idle: main thread 1.83% of one core over 6 s** (was 0.65%, but the old figure excluded children). Tree sampling caught **5 distinct children in 6 s** — `mmsg`, `wpctl`, `grim`×2, `pactl` — summing ≥3.17%. So the 1 Hz `grim` probe is **still spawning** (critic #6's finding stands, unfixed), but it is no longer the dominant cost.

### Score trend (the owner asked to see numbers move)
| metric | round start | r1b mid | r1b landed | r1b verified | reference |
|---|---|---|---|---|---|
| aspect | 4.029 | 2.553 | 2.553 | **2.595** | 2.643 |
| cap_over_pill_h | 0.486 | 0.342 | 0.342 | **0.351** | 0.357 |
| stroke_over_cap | — | 0.154 | **0.231** | 0.231 | 0.200 |
| RSS MB | 44.2 | 33.7 | 31.5 | **33.1** | — |
| idle CPU % | 2.0–9.0 | 10.0 | 3.0 | **1.0** | — |
Geometry is now within **1.8%** on aspect and **1.7%** on cap ratio. **Weight overshot**: 0.154 → 0.231 passed the 0.200 target, so the remaining sharpness gap (lapvar_norm 37.8% high) is now **too heavy**, not too thin — the opposite defect from the one the round started with. That is the next fixer's headline, and it is a small numeric correction, not a redesign.

### ★ r1c headline blocker — morph paints at ~16 fps (chain 7t, with a self-correction)
**Motion re-run with real compositor PTS** (`scripts/motion/run_all.sh /tmp/motion-r1b 1`), instrument stage **FAILED** two gates: `morph_poses = 2` (needs ≥8), `pose_hold_median_ms = 91.6` (needs ≤24 ms).
The height trace is the evidence: `35×7, 37×8, 84×…` — two long holds, then one jump. **Three independent capture methods agree**: grim transition 3 poses (score 38.0), recorded CFR 60 Hz **3 poses / 234 ms** (26.5), recorded VFR 3 poses / 229 ms (31.2). Target 10 poses = one per 16.7 ms over a 160 ms morph. Other gates PASS: effective fps 60.0, dropped ratio 0.0, edge ramp 1.0 px.

**SELF-CORRECTION #8 — my first root cause was wrong.** I wrote "coalescing turned the morph into a teleport" and "the perf fix traded CPU for motion completeness". **Both are false and are retracted.** Arithmetic refutes them: a 160 ms morph at 60 Hz still paints ~9–10 frames under perfect coalescing, because each ack immediately triggers the pending redraw. Coalescing cannot turn 10 poses into 3. Had a fixer read that note it would have "fixed" the frame callback and made things worse.

**Verified root cause — each paint costs longer than three display frames.** `src/main.rs:5335`: `if frame_running { upscale_canvas_2x_into(&self.scale_scratch, w, h, canvas) }` — a morph frame upscales the **full 1176×636 stage**. My own benchmark measured that at **62.10 ms**. **160 ms ÷ 62 ms = 2.6 paints**, matching the observed 2–3 poses and the two-holds-then-jump trace exactly.
And it is not just the upscale — per morph frame the code also does `stage_1x.fill(0)` (`src/main.rs:5255`) plus **two more stage-sized buffers** `morph_scratch_from` / `morph_scratch_to`, each `(w*h*4)` = **2.99 MB**, each `.fill(0)` (`src/main.rs:5271-5281`). **~9 MB of zeroing + a 62 ms upscale per frame**, to draw a capsule occupying a few thousand pixels.

**The back-pressure fix is CORRECT and did not cause this.** Before r1b the same slowness existed but was masked by queueing 400 unpresentable redraws at 103% CPU. r1b removed the waste and thereby *exposed* the real cost. Keep the frame callback exactly as it is.

**Fix shape for r1c, priority order:**
1. On morph frames, upscale only the **union of changed rects** (the from/to capsule rects plus damage), not the whole stage. **Constraint: do not assume the stage is static mid-morph** — `src/main.rs:5343` says "Morph frames own their canvas and keep the old path", so the fixer must verify what actually changes per frame and cover it, or stale pixels will appear.
2. Stop zeroing three stage-sized buffers per frame; size the scratch to the rects actually drawn.
3. Adopt the proven bit-exact hoisted upscale (**1.61×**, `scripts/perf/upscale_bench.rs`) for whatever region remains.
**Testable prediction:** 62 ms → sub-5 ms per paint ⇒ **10 poses at 60 Hz**. The motion harness confirms or refutes it; do not accept a claim without the pose count.

### Motion re-run, 3 trials, real compositor PTS — the defect is now triple-confirmed
`scripts/motion/run_all.sh /tmp/motion-r1b-3t 3`, exit 1, **4 gates FAIL**:
| gate | measured | bound |
|---|---|---|
| `latency_ms` | **317.0** | ≤260 ms |
| `settle_ms` | **317.0** | ≤260 ms (anim 160 + interval + slack) |
| `morph_poses` | **3** | ≥8 |
| `pose_hold_median_ms` | **83.8** | ≤24 ms |
**A 160 ms animation takes 317 ms to settle — 2× its intended duration.** That is the owner's "laggy", quantified.

**The capture is NOT the bottleneck** (this rules out the instrument as the cause): recorded CFR runs at **59.9 fps, interval median 17.0 ms, p95 17.0 ms**, clock `compositor_pts`. Three trials, poses **[3, 3, 3]**, duplicate ratio **0.846 / 0.882 / 0.846** — 85–88% of consecutive frame pairs are *identical while the morph is in flight*. Completeness 0.214 / 0.167 / 0.214. Scores **26.5 / 23.3 / 26.5**, median **26.5**.
**The damage-gated VFR cross-check is the smoking gun**: only **36 of 120** frames produced, **18.5 fps**, interval median 66 ms. The panel itself commits at ~18 fps mid-morph. Predicted from the 62 ms full-stage upscale: 1000/62 ≈ **16 fps**. Measured 18.5. The mechanism and the measurement agree.

**✅ Crop repair verified**: the settled control now shows **zero-delta 119/121, spikes 0** — previously median nonzero delta 13.8 with 4 spikes on a pill that cannot move. Deriving the region from the live pill removed the wallpaper noise floor.

**The 5 UNRESOLVED gates are correct behaviour, not instrument gaps** — `settle_duration_ms`, `width_reversals`, `overshoot_frac`, `in_flight_samples`, `duplicate_ratio` are all gated on `resolved_flight` (`scripts/motion/motion.py:797-830`), and `in_flight_samples = 0`: **no intermediate geometry was captured at all**. The harness refuses to grade a curve it never saw. Do not "fix" this. **These five resolving after the perf fix is itself evidence of improvement.**

### Panel extension — 4 critics on FROZEN surfaces (chain 4t, revising my own "wait" plan)
r1b is still running (12 modified `src/` files, 1438 insertions). Starting the src-dependent critics now would score a moving target — exactly what made critic #11's blockers stale on arrival. **But four surfaces are frozen and independent of the fixer's edits**, so reviewing them now produces findings that stay valid when r1b lands. Dispatched in one turn:
| slot | lens | model | uuid |
|---|---|---|---|
| #20 | **reference spec from pixels** (expanded panels, discs, tray) — vision | `agentrouter/gpt-6.1-sol` | `c8338066-8130-46f3-b9ea-3a25cc9fbecd` |
| #21 | dictation embedding (build, secrets, reboot, contract, RAM) | `logfare/deepseek-v4.1-flash-1017` | `ee2b9050-7f63-4ab7-872b-f83c40895a99` |
| #22 | **stress-harness integrity** — is 1207 tests real evidence or theatre | `group/hy4` | `96631275-40de-4ec1-97ad-668780c738ad` |
| #23 | **motion-harness integrity** — do the smoothness numbers tell the truth | `group/deepseek-v4.1-flash-1017` | `7839890c-35d9-4d8f-a6cd-d287b6e8c196` |
Models follow the owner's instruction: primarily DeepSeek v4.1 Flash **from different providers** (`group/`, `logfare/`), `hy4` for diversity, and one `gpt-6.1-sol` slot — the only confirmed image-seeing route, spent on the highest-value visual lens.
**#20 is the first critic to specify the EXPANDED panels numerically.** Every prior visual critic scored the collapsed pill; the owner's "Bluetooth at the side" and "the workspaces too" have never had a numeric spec. Deliverable `docs/expanded-panel-spec.md`.
**#22 must mutation-test the suite** — deliberately break something reversible in /tmp and confirm the suite FAILS. A suite that cannot fail is not a gate, and the owner's ≥1000-test rule is meaningless without this.
**#23's decisive question**: could height quantisation collapse a genuinely smooth 10-pose morph into 3 distinct heights and produce a FALSE teleport verdict? Travel is 48 px over 160 ms ≈ 5 px/frame, so it should not — but if it can, the next fixer would chase a non-bug. Also asked to break the selftest's ordering claim.
Both harness critics are told the panel may restart under them (Errno 107) and to re-run rather than report a false failure.

### ★★ The panel's OWN profiler confirms the upscale is ~97% of the cost
Two motion children independently reported `up=105.1 ms (upscale_only=105.1 ms)`. **I did not take that on trust** — `grep -rn 'upscale_only' src/` returns **nothing**, and `strings /proc/7964/exe | grep -c upscale_only` = **0**. The profiler is **not in the current tree or binary**; it was a temporary instrumented build. Found the actual logs at **`/tmp/prof7.log`, `/tmp/prof8.log`** (11-18 21:30 / 21:34, i.e. pre-r1b). Parsed every line:
| state | rect | draw_ms | **up_ms** | total_ms |
|---|---|---|---|---|
| Collapsed | false | 509.34 | 81.171 | **590.52** |
| Dictation | **true** | 26.84 | **0.000** | **26.85** |
| Music | false | 28.06 | 79.298 | 107.37 |
| ControlCenter | false | 37.97 | 66.163 | 104.15 |
| Collapsed | false | **2.70** | **76.858** | 79.57 |
| Lockscreen | false | 4.82 | 105.137 | 109.97 |
**The load-bearing row is the second Collapsed one: the pill itself draws in 2.70 ms; the upscale costs 76.86 ms — 96.6% of the frame.** And the one `rect=true` row (Dictation) shows **up = 0.000 ms**: the region path skips the upscale entirely. That is critic #6's "99.7% of the upscale is thrown away" confirmed by the panel's own instrumentation, from a source I verified rather than a child's claim.
Full-stage median total **107.4 ms** (n=5) vs region **26.85 ms** (n=1). At 107 ms per paint the morph can only present ~9 poses/s — matching the measured 2–3 poses per 160 ms and the VFR 18.5 fps.
**Caveat recorded honestly:** these are pre-r1b numbers from an instrumented build that no longer exists. The cost *structure* is unchanged — `src/main.rs:5335` still calls `upscale_canvas_2x_into` full-stage whenever `frame_running`. Re-instrument after the fix to confirm the drop.

### Critic #23 failed — provider transport, retrying once
`7839890c` (`group/deepseek-v4.1-flash-1017`) died immediately with `400: {"code":null,"message":"YJS: Upstream error (request id: …)","type":"invalid_request_error"}`, last message "I'll start by loading the thinking tool". A transport error at turn start, not a model refusal and not a brief defect. Per the skill: one retry, then move on.

### Two motion children landed — and they raced each other (recorded, not hidden)
`b2205210` built **`scripts/motion/motion.py` + `bounds.sh`** (wf-recorder `-D` ffv1, bounds table, exit 0/1/2). `06ef4440` built the **recorder front end** (`record.sh`, `analyze.py`, `selftest.py`, `docs/smoothness-baseline.md`, commit `10a68e2`). Both were dispatched to the same directory. Committed together as `9d809cf`.
**Collision handled by the children, not by me:** `06ef4440` found a `00-instrument` stage inserted into `run_all.sh` mid-run by the other agent, which marked its baseline FAILED. It left the stage intact but made it non-fatal and skippable when `motion.py` is absent. **Verified in the tree**: `run_all.sh:117-132` skips when absent, treats `rc=2` as "could not run", and only appends to `FAILED` on a genuine bounds violation. That is the right resolution.
**Two scoring formulas now coexist** (A: `100*(0.40C+0.35(1-D)+0.25(1-J))`; B: a bounds table). A reports **3 poses**, B reports **2**, on the same panel. Critic #23b is explicitly tasked with reconciling them and judging whether two formulas is itself a defect.

**Corrections the children made to MY brief — both verified, both accepted:**
1. **"No wf-recorder on this box" was stale.** `06ef4440` installed **0.6.0 from the Artix `world` repo** via `sudo -n pacman -S --needed wf-recorder`. No capabilities were set, so none needed removing; kmsgrab was never attempted. This is the only reason 60 fps is measurable at all — my earlier grim-only pipeline was capped at 18–30 fps.
2. **The easing is `EaseOutCubic`, not `OutBack`/1.15.** `src/main.rs:4835-4850` switched both directions because the OutBack overshoot strip rendered as a **one-frame black slab**. My brief carried the old spec; bounds are correctly `reversals ≤ 1`, `overshoot ≤ 0.05`.
**`-D` (`--no-damage`) is mandatory**: without it wf-recorder emits only on damage and reports a fake 38 fps. With it, 60.1 real buffers/s, "Using video filter: null" (no fps filter padding the count).

**Fifth harness defect found by a child, pre-existing:** `drive_bars.py` sent on an **unconnected** AF_UNIX socket → `[Errno 107]`. The archived "sustained" baseline (54/60 zero-delta frames) **measured nothing**. Fixed with `sendto(path)`; now sends 213 bar updates over 2.6 s. Also fixed: deltas in (0, 0.05] fell into **no** histogram bucket; frame indices off by one; PTS were capture-relative while `trigger.log` used wall clock, so the anchored window picked a **trailing carousel tick instead of the morph**; and "nearest run" window selection reported *1 pose* on a stalled build — **the instrument would have flattered the very defect it was built to expose.**

### Critic #20 (vision) failed — probing cheaper vision routes before spending more `sol` (chain 5t)
`c8338066` (`agentrouter/gpt-6.1-sol`) died with **`Error reading response: malformed server-sent event JSON`** and wrote no spec. Transport fault, not a refusal, so one retry is allowed. **But I did not reflexively retry the expensive route.**
Reasoning: `sol` is the only route I have *positively confirmed* sees images, and the owner explicitly warned it drains their account. There will be several more visual rounds, so paying two tiny probes now to find a cheaper seeing route pays for itself. Every `glm` route is image-blind at runtime; `hy4` and `gemini-4-hydrogen` declare image input but had **never been probed** — and a visual critic that cannot see would invent measurements, the worst outcome for a pixel-perfect task.
Dispatched two 5-question probes (`704af058` hy4, `01427332` gemini-4-hydrogen), each told to answer **"CANNOT SEE IMAGES"** rather than guess.
**Ground truth established by the parent from the pixels, so the probes are gradeable:**
| question | truth |
|---|---|
| capsule fill | **BLACK**, p25 = **0.0** |
| text | **`00:05`** (read directly at 4× nearest-neighbour zoom) |
| polarity | light-on-dark, ink peak 255 |
| width fraction | **4.375%** → "under 10%" |
| wallpaper | median RGB **[62, 120, 243]** — blue sky with white cloud |
Capsule bbox in `/tmp/ref/s/s17.png`: **x[1224,1335] y[19,60], 112×42**. (The canonical 111×42 figure came from a slightly tighter mask; the 1-px difference is the AA edge and does not change any ratio.)
Whichever probe answers correctly takes the reference-spec brief; if both fail, retry `sol` once.

### ✅ `magpie/group/hy4` CAN SEE IMAGES — 5/5 on the probe
`704af058` answered **BLACK · `00:05` · light-on-dark · under 10% · blue** — every item matches the parent-established ground truth, including the exact clock string, which cannot be guessed from a filename.
**This is the cheapest confirmed vision route found so far** (`group/hy4`, 65 K context) and it frees the expensive `agentrouter/gpt-6.1-sol` slot for the owner. All subsequent visual critics route here unless it fails.
**Caveat to carry:** 65 K context is small for a spec task over ~120 frames. The brief must therefore name a *bounded* frame set rather than "look at everything", or the child will run out of window mid-spec.

### ⛔ RETRACTED — critic #5's "#F2A65A accent" finding was a hallucination, and I forwarded it (chain 6t)
Critic #5 filed a **major**: "the spec's single accent **#F2A65A** appears zero times in src/docs/assets". I recorded it at line 639 **without verifying it**. It is wrong.
**Measured the reference directly.** `/tmp/ref/s/s13.png` is the power/control panel. The active **Lock** disc's modal interior colour is **#79A1F5 (121,161,245)**, next most common #7BA0F3 and #7BA0F5 — one periwinkle within video-compression noise. Our `src/calendar_panel.rs:187` `ACCENT = [0x7C,0xA2,0xF5]` = **#7CA2F5 (124,162,245)** is within **3 units per channel**. **Our accent hue is essentially correct.**
**`grep -rn 'F2A65A'` across the whole repo and /tmp returns exactly one hit: COUNCIL_LOG.md:639 — the critic's own finding.** It is in no spec, no doc, no asset, no measurement. #F2A65A is an **orange**; the reference accent is a **periwinkle blue**.
**Why this matters more than a normal wrong finding:** had a fixer obeyed it, it would have replaced a correct accent with orange and **destroyed the pixel match the owner explicitly asked for**, in the name of removing slop. A confident, specific, actionable instruction that is factually false is the most dangerous output a critic can produce. **Parent error: I merged it without checking.**
**The real, surviving accent finding (restated with numbers):**
- One hue is hand-typed in ≥4 places instead of coming from a token: `#7CA2F5` (`src/library.rs`, `src/calendar_panel.rs:187`), `#93A4F5` (`src/controlcenter_panel.rs:175`), `#74A3F6` (`src/controlcenter_panel.rs:180`), `#7AA2F7` (`src/music_panel.rs`). `library::ACCENT`/`theme_accent()` drive none of them ⇒ **theming cannot work**. Genuine defect, keep.
- **New measured defect:** the control-centre toggle `#93A4F5 (147,164,245)` is **26 units too light on red** vs the reference `#79A1F5 (121,161,245)`. Fix toward the measured value. **Do not change the hue family.**

### ✅ Both `hy4` AND `gemini-4-hydrogen` see images — 5/5 each
`01427332` (gemini-4-hydrogen) answered **BLACK · `00:05` · light-on-dark · under 10% · blue with large white clouds**. Two cheap confirmed vision routes now exist; `agentrouter/gpt-6.1-sol` is no longer needed for routine visual critique, which respects the owner's warning about its cost.

### Reference EXPANDED-panel geometry — parent-measured, first time any of this is specified
All at 2560×1440, threshold max-channel ≤40, widest-dark-run per row, gaps ≤25 px bridged (the panels have internal separators that break a naive contiguous scan):
| frame | panel | x-span | **w×h** | aspect | centre x |
|---|---|---|---|---|---|
| `s13` | **power / control** | 1011–1548 | **538×201** | 2.677 | **1279.5** |
| `s37` | themes (search) | 881–1678 | 798×489 | 1.632 | 1279.5 |
| `s5`,`s53` | launcher (search) | 942–1617 | 676×489 | 1.382 | 1279.5 |
| `s73` | themes (list) | 881–1724 | 844×490 | 1.722 | 1302.5 |
| `s45` | wallpapers | 513–1963 | 1451×143 | 10.147 | 1238.0 |
**Every panel is horizontally centred on 1279.5 = screen centre**, same as the pill. Panel fill is **p10/p25/p50 = 0.0** — genuinely pure black, same material as the collapsed pill.
**Power panel internals (`s13`, the one the owner cares about):**
- 5 circular buttons, **diameter 54 px** (both the active blue and an inactive grey measure 54). **disc/panel-width = 54/538 = 0.1004.**
- Active disc x[1253,1306] y[96,150]; inactive disc x[1088,1141] ⇒ **pitch ≈ 82 px**.
- **Inactive disc fill (25,26,31)** — a very dark grey, NOT mid-grey.
- Uptime line "Up 3 h 34 min": **cap height 10 px**, y[56,65].
- Active label "Lock": **cap height 11 px**, y[173,183], peak luminance **222** (not 255 — the reference's white is slightly off-peak, consistent with the pill's 225.7).
- Layout: uptime centred at top, disc row centred, active label centred below. **No title bar, no separators, no card-in-card.**

## r1 panel — critics #21, #22, #23b (instruments and the embedded tool)

### #23b motion-harness integrity — score **3** (`group/deepseek-v4.1-flash-1017`)
**BLOCKER, verified and fixed by the parent.** It built synthetic frames with **fixed geometry** and only alternating body luminance (2/20 vs bg 100). Harness A reported **poses=11, poses_unique=2, completeness=1.0, smoothness=100.0**; B reported 10 poses. **A two-state strobe — visually worse than a teleport — scored PERFECT.** The score used the raw change count and ignored `poses_unique`, which it already computed.
**Fix (parent):** `scripts/motion/analyze.py` completeness numerator is now **`uniq / POSE_TARGET`**, not `poses / POSE_TARGET`. Added `case_strobe()` to `scripts/motion/selftest.py` asserting `poses_unique == 2`, `completeness == 0.2`, and **strobe < slideshow**. Whole selftest still PASS (perfect 100 > duplicate 68.5 > slideshow 48.7 > teleport 4.0).
**It also REFUTED my own worry**: the 3-vs-2 pose gap is **not** height quantisation — both harnesses count a true 5 px/frame 10-pose ramp correctly (A=10 score 100, B=10). The gap is definitional (A: any changed frame in `[t-100ms, t+400ms]`, mean-abs-delta > 0.05; B: ≥60 changed px in a tighter geometry window). **So the "panel paints 2–3 poses" finding stands.**
**Accepted, unfixed:** B's 260 ms latency gate uses a timestamp its own author admits carries **±100 ms** (wf-recorder first PTS = 0, flush offset 83–125 ms), and reports `latency == settle` exactly with `in_flight_samples=0` — one observation reported twice. **Do not quote 317 ms as a latency number.**
**⚠️ Parent over-claim corrected:** I wrote "the panel itself commits at ~18 fps". Neither harness can distinguish panel draws from compositor presentation, and the VFR cross-check does **not** close it (same output path). Defensible claim: **"the composited output presented 2–3 distinct states."** The `/tmp/prof7.log` draw-cost table is independent panel-side evidence; the pose count is not.

### #22 stress-harness integrity — score **4** (`group/hy4`)
The most valuable verdict of the round: it **mutation-tested** the suite instead of reading it.
- ✅ blank fake-grim screenshot → liveness **FAILED** (not pure theatre)
- ✅ injected +25 MB RSS leak → **171** sample assertions failed (my per-iteration assertions work)
- ⛔ **`App.send` stubbed to return "ok" for `calendar` without sending → `state_machine PASS tests=246 fails=0`.** My `drive()` fell back to "is the process still responsive?" when no morph line appeared. **Socket liveness is not state correctness.** The ≥1000-test gate could stay green with a whole state broken.
**Parent fixes, all in `scripts/stress/`:**
1. `drive()` now tracks `confirmed["state"]` and **FAILS with "NO EFFECT"** when a mapped state word produces no morph line and the panel is not provably already in that state. Seeded from a real screenshot (`App.pill_present()`) so an already-collapsed start is not a false failure. Sub-state words (`locktype`/`lockback`/`unlock`/`dictation_*`) are exempt from the morph requirement but still require a live process.
2. `send_ok` retries **`backpressure`** as well as `refused`, and the post-burst check accepts it — critic #22 proved "dead after burst" was a **false product failure** on a healthy panel under load.
3. `pill_metrics` now returns **`ink_frac`**; `check_pill` requires **positive** evidence — a stadium (`w > h`) and **≥2% bright glyph ink inside**. Critic #22 fooled every prior check with a plain dark stripe at **y=1** (the `y0==0` refusal misses y=1). `detector_selftest.py` grew from 4 to **7** fixtures incl. the y=1 stripe and a blank capsule. **All 7 PASS.**
4. New **`scripts/stress/mutation_gate.py`** — injects drop-calendar, drop-lock and fake-3000 ms-latency into the real live panel and requires each to FAIL, with an unmutated control that must PASS. **All three mutations CAUGHT.** Control initially failed on 23 assertions, which exposed two real harness bugs (unmapped sub-state words; unseeded initial state) — both fixed. A mid-group redeploy is now marked **INVALID and retried**, not scored as a product failure.
**Accepted, unfixed:** 4350 protocol asserts are all `sendto` outcomes (252 variants + 21 bare + 54 near-miss + 10 bars + 6 padded + 7 prefix/suffix + 4000 rapid-fire); 282 protocol tests have no per-case assertion. **"5897 assertions" is honest counting but mostly measures the socket, not the product.** Two full runs on one PID gave fails=69 (p95 785.8) then fails=9 (p95 183.9) with loadavg 16.72→4.24 — **high load must invalidate a run, not fail the product.**

### #21 dictation integration — score **6** (`logfare/deepseek-v4.1-flash-1017`)
Verified the copy is genuine: `diff -rq` clean vs scratch, Cargo.lock identical, binary sha1 matches, 30 files/452 K tracked, **0 key hits in tree and across 37 commits**.
**⛔ My EMBEDDING.md claim "all four literals were sent and accepted" was FALSE for `collapsed`.** `tools/dictation/src/main.rs:67` `let pill: Option<pill::PillHandle> = None;`, `try_spawn()` is `#[cfg(test)]`-only, and **`grep -ac collapsed target/release/static-dictation` → 0** (parent re-verified). Corrected in `EMBEDDING.md` and `docs/socket-contract.md`; the literal is marked DEAD CODE so nobody revives it by accident.
**🔒 Privacy defect fixed (parent).** `output.rs:29` prints every transcription; the supervisor appended to `~/.cache/static-dictation.log` at **mode 644, never rotated** — a world-readable, unbounded diary of everything the owner dictates, including this brief. Supervisor now creates it under **`umask 077`**, `chmod 600`s any pre-existing file, rotates at **1 MiB** keeping one generation. Existing log tightened to **600** (verified).
**Fixed:** `verify_single_hotkey.sh` no longer `cd`s into the deletable scratch dir; file counts corrected 29/448 → **30/452**.
**Accepted, unfixed (queued for the fixer):**
- **Offline build is machine-sufficient, not self-sufficient.** With an empty `CARGO_HOME`: `error: no matching package named \`anyhow\` found`. No vendor dir, no `.cargo/config.toml`, no build entry point; `target/` is gitignored so a fresh clone has no binary and no command that makes one.
- **Always-on microphone** — `src/main.rs:90` `capture::start().expect(...)` runs at launch, `capture.rs:88-103` parks forever. Measured **Pss 1794 kB idle, 28 threads, ~99 voluntary ctx-switches/s**, permanently live `alsa_capture.static-dictation` PipeWire node. Direct hit on "as little RAM as possible". `.expect` + 1 s restart loop = crash loop if the mic vanishes. Fix: start on `HotkeyEvent`, drop on `HoldEnd`. **Deliberately not done by the parent** — it changes runtime behaviour of a tool the owner uses daily.
- The "separate manifest" rationale shields **sctk 0.21.1, which only dead `src/pill/tsar.rs` uses**. Deleting the pill module would make a single workspace safe.

### ⚠️ Measurement validity — the panel currently has NO layer surface (parent, verified)
While re-running the mutation gate the control kept failing. Root cause is **not** the harness:
- `mmsg get all-layers` → **`{"layers":[]}`**. The panel process is alive (pid 11464, started 01:39:24) and its socket answers, but **it has no layer surface on screen**.
- Sending `collapsed` / `lockscreen` / `unlock` / `dictation` produced **0 new stderr lines** for each.
- The top 200 rows of a fresh `grim` are **pure black** (median [0,0,0]) — that is the owner's T3 window, not the panel.
- Binary mtime **01:38:50**; `cargo build --release --offline` is running again right now (fixer mid-edit, `SLOT_RING` change in flight).
**Conclusion: the fixer's in-flight build is in a broken intermediate state.** Every screen-based measurement taken since ~01:38 is invalid, and the mutation-gate control failures after that point are environment, not product. Earlier measurements (scorecard rows, the 1207-test PASS, the profiler table) predate it and stand.
**Harness gap this exposed:** the suite asserts socket liveness but never asserts **surface presence**. Added to the r1c brief as a required gate: a run is INVALID unless `mmsg get all-layers` lists the panel.
**Config drift noted:** the fixer set `collapsed_width=67` (log line at 01:31:31), not the 74 the parent derived for a 111-device-px pill. 67×1.5 ≈ 100 px. Must be re-measured once a working binary is back.

### r1 panel extension (parent, chain 4t)
Two more slots dispatched on surfaces the in-flight fixer is not touching:
- **#24 expanded-panel pixel match**, `magpie/group/gemini-4-hydrogen` (vision-confirmed 5/5), thinking high — uuid `588d73cf`. First critic ever to compare an OPENED panel to the reference; every prior slot scored only the collapsed pill. Briefed with the parent-measured panel table and told to re-verify it, told the accent hallucination story so it does not repeat it, and told to check `mmsg get all-layers` before trusting any screenshot.
- **#25 Rust idiom + panic safety**, `magpie/fasfas-ai/deepseek-v4.1-flash` (a third DeepSeek route, unused so far), thinking high — uuid `32961825`. Retry of the twice-RPM-failed #19. Lens chosen because `panic = "abort"` makes any reachable unwrap a desktop-killing bug, not a style nit. Told to skip cargo while another build holds the target lock.

### #25 Rust idiom + panic safety — score **7** (`fasfas-ai/deepseek-v4.1-flash`)
Highest score of the round, and **earned, not generous**: it reports a positive result it could have skipped (**no reachable panic**), and it **corrects two of my own brief's premises against me** with line numbers. A lazy critic flatters the brief.
**Verified no reachable panic** (parent spot-checked the guards): `from_utf8_lossy` + `.take(27)` bounding `bars[slot]` (`src/main.rs:6505-6640`); hand-rolled inflate + PNG chunk walk guarded at every length (`src/clipboard_panel.rs:700-1010`); tray pixmap conversion guards degenerate dims and short buffers (`src/tray.rs:311,353,1179,1220`); `MONTH_NAMES[month-1]` fed by `rem_euclid(12)+1` (`src/calendar_panel.rs:304,543`). Glyph cache bounded, clears at 512, keyed (font ptr, char, px bits) (`src/lock_timer.rs:257-272`). **Frame-callback gate is race-free by construction**: `frame_pending`/`redraw_wanted` are plain fields on the single calloop thread, no Mutex held across a draw, 250 ms cap prevents starvation (`src/main.rs:5108-5118, 5430-5442`).

**⛔ STALE FINDING RETRACTED — critic #6's "1 Hz grim probe = 9.75% of a core".** Parent verified `src/wallpaper_blur.rs:495`: **`sleep(Duration::from_millis(60_000))`**, with a comment recording that the 1 s cadence was *itself* the fix for an earlier ~2.5% CPU problem. The probe is **once a minute** in this tree. #6 measured a build that no longer exists. **Remove from the r1c brief.** Residual (minor, defensible): it re-forks grim and re-blurs unconditionally on wake with no mtime gate.
**Also correctly downgraded:** lock-poison `.unwrap()`s are **unreachable under `panic="abort"`** — an idiom inconsistency, not a panic path. It explicitly warns the council not to treat them as one. They do bite under `cargo test` (unwind).

**★ It resolves my own over-cautious caveat.** I had briefed "upscale only the union of changed rects, but do NOT assume the stage is static mid-morph" (from the `src/main.rs:5343` comment). #25 shows the damage rect is bounded **by construction**: a morph interpolates between two **known** geometries and `frame.width/height` are already in hand, so `full` is a **hardcoded choice, not a requirement**. My caveat would have blocked the correct fix.
Verified in tree — `src/main.rs:5267-5268`:
```rust
let full = (0, 0, w as i32, h as i32);
let mut rect = if frame_running || self.state == IslandState::Lockscreen { full } else { … }
```
**Its independent timings triangulate with mine and the profiler table**: full stage **31.5–84.3 ms**, pill-sized 110×41 region **0.43 ms (156× cheaper)**, 300×420 panel region **12.4 ms**. (Parent's own bench: 62.10 ms full, 388 µs pill.) Three independent measurements agree: **the upscale dominates and the region path is ~2 orders cheaper.** Root cause settled.

**New findings, all parent-verified in tree:**
- **major** — `upscale_stage_perf` asserts **`best.as_millis() < 200`** (`src/main.rs:8410`) while the doc claims **"a 1176x636 stage costs ~5 ms"** (`src/main.rs:521`). Actual 31–84 ms. **The guard is 40× looser than the claim** — a doubling ships green. Tighten bound and comment together.
- **major** — `tray::snapshot()` is `tray_state().read().unwrap().items.clone()` (`src/tray.rs:116`), called per settled frame by `draw_tray_cluster_2x` (`src/tray.rs:1315`). `TrayItemSnapshot` holds 6 `String`s + `icon: Option<Vec<u8>>` (1 KB) + `icon32` (4 KB) ⇒ **~15 KB icon heap + 18 Strings cloned and dropped per frame, read-only**. Textbook borrow-vs-clone; direct hit on the low-RAM brief. Fix: `with_items(|items| …)` guard scope or `Arc<TrayItemSnapshot>`.
- **minor** — `cfg()` (`src/main.rs:305`) clones the `wallpaper` and `theme` Strings on **every** call; `IslandState::size()` calls it 2–3×; 15 geometry sites. Every geometry query allocates. Fix: a `Copy` metrics snapshot.
- **minor** — `draw_top_row_2x` allocates `&hhmm()` (`src/main.rs:2489`, `format!` + `localtime_r`) and `(i+1).to_string()` per disc (`src/main.rs:2493`) every settled frame.
- **minor** — 34 ungated `DBG` sites in `src/main.rs` + 1 in `src/music_panel.rs`; the **Motion** arm prints per input sample during a morph (`src/main.rs:6045-6050`) — an unbuffered `write(2)` per sample on the render thread. One `OnceLock<bool>` env gate fixes all.
- **minor** — 3 remaining `horizontal_line_metrics().unwrap()` (`src/main.rs:2011`, `src/controlcenter_panel.rs:2547`, `src/lockscreen.rs:673`) while **ten siblings already use `let … else { return }`**. Latent, not reachable today; copy the neighbouring form.
- **minor** — `ensure_loaded()` (`src/wallpaper_blur.rs:404-421`) does a stat + mutex + Arc clone **twice per settled frame** (`src/main.rs:2154`, `:2431`).

### Parent measurement — r1c mid-flight (panel surface back, build settled)
`mmsg get all-layers` → `{"layers":[{"monitor":"eDP-1","layer":"overlay","name":"dynamite-rust"}]}`. The invalid window is over. Idle CPU **0.67%** over 6 s (best yet; was 2–9% at baseline).
**⚠️ MEMORY REGRESSED.** `VmRSS 56708 kB`, **`Pss 41948 kB`** (`Pss_Shmem 11688`, `Pss_Anon 26808`). Scorecard RSS went **31.5 → 33.1 → 55.4 MB**. Baseline was 44.2 MB RSS, so the tree is now *worse than where it started* on the owner's explicit low-RAM requirement.
**Likely cause (not yet confirmed):** the stage-sized shm ring. `SLOT_RING = 2` (`src/main.rs:514`), `needed = bw*bh*4` (`src/main.rs:5206`) = 2352×1272×4 = **11.96 MB per slot**, pool `needed * SLOT_RING` (`:5227`) = **23.9 MB**. The fixer is mid-edit on exactly this (`self.slots.len() == SLOT_RING` / `self.slot_bytes = slot_bytes`). Bounding the morph damage rect would shrink `needed` and therefore the whole pool — **the perf fix and the RAM fix are the same fix.**
**Geometry drift:** config now `collapsed_width=67 bar_height=25 gap_top=13` → live pill **96×38**, aspect **2.526** (ref 2.643, 4.4% off). The parent-derived target remains **74×28** → 111×42. Not applied: the fixer owns config this round.
**Sharpness:** `lapvar_norm 1665.74` vs ref 1277.0 = **30.4% high**; `stroke_over_cap 0.231` vs 0.200 = **15.5% too heavy**. Both were *under* at baseline (0.154) — the weight fix overshot. `cap_over_pill_h 0.342` vs 0.357.

### Parent spot-check of #25's findings — ALL CONFIRMED IN TREE
Ran the greps myself rather than trusting the verdict (the run has already caught one hallucinated finding and two stale ones).
- `src/tray.rs:116` — `tray_state().read().unwrap().items.clone()`, doc says *"Clone the current item list (UI thread: draw + hit-test + clicks)"*. Called at `src/tray.rs:1316` inside `draw_tray_cluster_2x`, i.e. **every settled frame**. Confirmed.
- `src/main.rs:8412` — `assert!(best.as_millis() < 200, "upscale took {best:?}, budget 200 ms")` against the doc at `src/main.rs:521` *"a 1176x636 stage costs ~5 ms, see the iter108 perf test"*. **The test it points to permits 40× the claim.** Confirmed.
- `src/main.rs:305` — `CONFIG.read().unwrap().clone()`; doc admits *"Clone snapshot"*. Confirmed.
- `src/main.rs:2493` — `&(i + 1).to_string()` per disc inside the per-frame painter. Confirmed.
- **36** `DBG` occurrences in `src/main.rs` (critic said 34 sites; the extra 2 are non-`eprintln` mentions — same finding, count immaterial).
- `horizontal_line_metrics().unwrap()` holdouts confirmed at **`src/main.rs:2011`, `src/controlcenter_panel.rs:2547`, `src/lockscreen.rs:673`**, with the degrading sibling form present at `src/main.rs:2525` (`let Some(lm) = … else { return }`). The fix is to copy the neighbour.
- `ensure_loaded()` is called at **three** sites (`src/main.rs:2154`, `:2431`, `:3426`), not two — the third is the morph path, so a morph frame pays the stat+mutex+Arc-clone an extra time.

### Panel extended to 17 slots (chain 5t) — source-only lenses while the fixer builds
Scored so far (12): #1=6 · #2=5 · #3=3 · #4=7 · #5=6 · #6=4 · #8=3 · #11=3 · #21=6 · #22=4 · #23b=3 · #25=7. **Median 4.5.** Gate ≥9 **not met**.
**Why source-only right now:** fixer r1b is mid-build and *restarting the panel* (pid 8599 runs `cargo build` then `kill -9` + relaunch). Live-pixel critics dispatched into that window produce fake findings — this already happened once at 01:38–01:42. So the new slots are forbidden from cargo, from restarting the panel and from driving its socket.
| slot | lens | model | uuid |
|---|---|---|---|
| #26 | memory + shm budget | group/hy4 | `90e5b2ab` |
| #27 | **lock screen vs corrected spec** (vision) | group/gemini-4-hydrogen | `49120719` |
| #28 | **owner's additions: Bluetooth + workspaces** | group/hy4 | `1a62a97c` |
| #29 | **portability + packaging** | logfare/deepseek-v4.1-flash-1017 | `2e26bac8` |
(#24 expanded-panel vision `588d73cf` still outstanding.)
Each brief carries the **retracted** `#F2A65A` accent claim explicitly so no critic re-introduces it, and #27 carries the **comparison-foil trap** (`l60`/`l320` are the design the creator rejects) so it cannot rebuild the wrong screen. #27 and #28 use the two routes proven to see images 5/5, keeping `gpt-6.1-sol` in reserve — the owner warned it drains their account.

### #26 memory + shm budget — score **5** (`group/hy4`)
**Measured, not inferred, and it corrects the parent twice.** Both corrections accepted after checking the code.

**⛔ PARENT SELF-CORRECTION — "the perf fix and the RAM fix are the same fix" was WRONG.**
I wrote that shrinking the morph damage rect shrinks `needed` and therefore the shm pool. Verified at `src/main.rs:5203-5206`: `let (bw, bh) = (w * RENDER_SCALE, h * RENDER_SCALE); let needed = (bw * bh * 4) as usize;` — **`needed` comes from the SURFACE size, not the damage rect.** A tighter rect cannot shrink the **reservation** (23,929,344 B). It changes **residency**: which pages get touched. #26 measured the first themes morph raising `Pss_Shmem` **0.3 → 11.688 MB** (shm RSS 23.376 MB, both slots faulted), total Pss **30.6 → 42.1 MB**, persisting **>3.5 s** after collapse. Union rect saves **~3.0–4.9 MB Pss per slot, ~6–10 MB both**. Real and large — but the wording matters: a fixer told "this shrinks the pool" hunts for a pool-size change that does not exist.

**⛔ PARENT SELF-CORRECTION — "RAM REGRESSED 44.2 → 55.4 MB past baseline" is RETRACTED.**
That reading was taken with both slots faulted after morph activity. #26 measured **idle after unlock: VmRSS 34.1 MB, Pss 30.6 MB**, with **no leak**: 12 lock cycles moved collapsed Pss **30.45 → 30.59 MB**; 20 theme cycles **41.97 → 41.83 MB**. Steady state is **better than the 44.2 MB baseline**, not worse. Same error class as quoting raw lapvar without normalising contrast: I compared a transient against a steady state.

**The genuine RAM finding replaces it — the lock screen doubles the pool:**
| | normal | locked |
|---|---|---|
| shm pool | 23,376 kB | **47,096 kB** (2 × 24,112,128 = **48,224,256 B**) |
| Pss_Anon | 26.9 MB | **32.9 MB** (+6,028,008 B lock wallpaper) |
| VmRSS | 34.1 MB | **86,616 kB** |
| Pss | 30.6 MB | **59,917 kB** |
| VmHWM | — | **103,696 kB** |
Fully recovers on unlock (33.6 / 30.3 MB) ⇒ **peak problem, not a leak**. Sites `src/main.rs:1943` (`LOCKSCREEN_SIZE = (1504, 1002)`), `:5206`, `:5227`, `:4540`. A 2× steady-state peak on the owner's explicit minimal-RAM brief.

**Open question the parent could NOT reconcile by reading (chain 4t) — handed to the fixer as a measurement, not a guess:**
`src/main.rs:5237-5239` deliberately `canvas.fill(0)`s the **whole slot at creation**, commented *"Touch the whole slot once, here, instead of letting a full-stage draw fault it in later … which reads as a step in RSS."* That contradicts #26's **0.6 MB of 23.9 MB resident** at idle after unlock. If creation really faults both slots, the morph is not what makes them resident and the union-rect saving is smaller than estimated. Also: that fill memsets **23.9 MB on every surface-size change** (every lock/unlock) purely to avoid a *cosmetic* RSS step, and the comment itself says fresh memfds are already zero-filled — so it may be **both redundant and harmful**. Verify before removing; an undamaged area must stay transparent.
**Do not drop `SLOT_RING` to 1 on a guess**: `src/main.rs:5207-5210` — `pool.canvas(&slot)` returning `None` for the in-flight slot *is* the back-pressure; one slot stalls every frame.

### Instrument repair #9 (parent) — commit `92a100c`: name the guilty payload
Answers #22's surviving critique of the owner's **≥1000-test gate**. The four inertness batches (word+garbage, case near-miss, compound, newline injection) checked **one** log witness for the whole batch and, on a hit, added `len(batch)` to fails — **one bad payload indicted 282 innocents and the report never said which**. A gate that cannot direct a fix is a weak gate wearing a big number.
Inert payloads log nothing, so `_inert_batch()` keeps the healthy path at **one witness** and pays a **log2(n) bisection only on failure**, naming the exact payload. Adds one assertion per payload so the count is honest.
`scripts/stress/inert_selftest.py` pins it with **10 ground-truth checks, 10/10 PASS**: clean batch costs one witness; a lone culprit at index 0, 137 or 281 yields exactly one failure naming it; two culprits are both named; singleton works.

### r1c brief drafted — `docs/r1c-brief-draft.md`
Assembled while r1b builds, from **parent-verified** findings only. Eleven sections: morph damage rect (with the three triangulating measurements and the withdrawn over-cautious caveat), lock-pool peak, the 40×-loose perf guard, per-frame allocation table, DBG gate + `let else` holdouts, geometry **set 74×28 then MEASURE**, the 1×-paths-on-expand defect, colour with the **retracted `#F2A65A`** warning, measured expanded-panel geometry, the **corrected** lock-screen spec with the comparison-foil trap, dictation's always-on mic and dead `collapsed` literal, and the six required verification commands.

### Gap closed: the owner asked for EVERY transcript — only 2 of 13 had been mined
`ls /home/alex/Downloads/Reeldrop/*.txt` → **13 transcripts** (12 real + one Rick Astley joke file, 34 bytes). Only the Dynamic Island and Lock Screen ones had been read. The owner's brief said *"analyze the transcriptions from every video"*, so this was an open objective silently unmet. Three workers dispatched, text-only (no vision needed, so cheap routes):
| worker | transcripts | model | uuid | output |
|---|---|---|---|---|
| A mechanics | Quickshell Positioning · Hidden Layout · Why Ricing Is Still Hard | group/deepseek-v4.1-flash-1017 | `914df238` | `docs/transcript-mechanics.md` |
| B taste | Needs Minimalism · Where I Wasted Time · **This one trick** · anime wallpapers | group/hy4 | `4a9dd3e0` | `docs/transcript-taste.md` |
| C dim/session | **Default Dim Too Strong** · GTK4 theming · no display manager | logfare/deepseek-v4.1-flash-1017 | `a614ad91` | `docs/transcript-dim-session.md` |
**Why C is highest-value:** our lock screen needs a dark veil and we currently derive a *vertical* black veil purely from pixels, with **no stated authority for its strength**. Hyprland's `dim_strength` is exactly that concept and `docs/mango-capabilities.md` records MangoWM has no equivalent — so his number is a target we can adopt directly. Brief demands every number quoted verbatim with an explicit ambiguity flag, because auto-captions garble decimals and a wrong value would be built into the lock screen.
All three briefs carry the **invented-`#F2A65A` lesson** and require separating *what he states* from *what the worker infers*.

### Instrument repair #10 (parent) — the panel's own witness outranks every pixel heuristic
`measure.py` returned **pill 220×205 at (1018,15), aspect 1.073, ALL glyph metrics 0.0, `ink: null`, RSS 89.3 MB, CPU 98.5%**. Not a pill. stderr showed another agent driving `Lockscreen → Launcher → Wallpaper → Collapsed`, ending `DBG enter state=Collapsed morph=false pos=(538.5,715.6)` — **y=715.6 logical is the centre of the 1002-tall lock surface**, so the capsule was still mid lock-exit. RSS 89.3 MB independently matches #26's measured **locked** peak of 86.6 MB.
**Sixth mis-score, fifth distinct cause** — and this one defeats *every* pixel guard: the box starts at y=15 (not 0) and its interior **is** genuinely the panel's own black material. It is simply the wrong surface. No luminance test can separate a lock surface from a pill.
**Fix:** `panel_state()` reads the panel's own `DBG enter state=… morph=…` witness from `/proc/PID/fd/2`; `main()` emits `state:"not-collapsed"`, `pill: null` and **no gates** unless `state==Collapsed && morph==false`. Authoritative, not inferred — same lesson as the stress harness's surface-presence gate. Falls back to the pixel heuristics when stderr is a pipe. **6/6 self-test checks PASS** (parses witness, takes the LAST not the first, morph true/false, no-witness→None, pipe-stderr safe).

### Transcript miners A/B/C returned — 13 of 13 transcripts now mined
**★ Worker C corrected my brief and prevented a wrong fix.** I briefed *"his dim number is a target we can adopt directly"*. **That was wrong.** His `dim_strength` dims **unfocused clients relative to a full-brightness focused surface, compositor-side**; our lock veil is an **absolute alpha over a static wallpaper with no focused window**. Different quantity — **not substitutable**.
- **`dim_strength = 0.12`** (HIGH confidence), stated band **0.12–0.15** (MEDIUM, caption drops the "to"); default **0.5** criticised (HIGH); range 0–1, `1.0` = black privacy filter. Demo "0.01" **AMBIGUOUS — discarded**. No hex ever stated.
- **Keep the measured veil.** Verified in tree: `src/lockscreen.rs:205-206` `SCRIM_CENTRE 0.22` / `SCRIM_EDGE 0.42`, with an iter13 re-measurement comment (patch-averaged 9×9, corners mean ratio 0.58 ⇒ alpha 0.42, five points 0.39–0.49). Adopting 0.12 would have made the veil ~half as dark as the reference — destroying the pixel match *in the name of following the creator's advice*.
- Record 0.12–0.15 only as **stated authority that a gentle, still-legible dim is deliberate** and 0.5-heaviness is the known mistake. **His acceptance test is legibility, not a number.**
- **Safety warning worth carrying:** test the unlock path before enabling lock-at-boot, or a failed lock client leaves an **untypeable locked session**.
- GTK4: no bearing on a self-drawn panel; one transferable fact — it reads `~/.config/gtk-4.0/gtk.css` **once at startup**, so a shared palette will not live-reload there.

**Worker A (mechanics):**
- **No perf content exists in any of the three transcripts** — zero mentions of frame callbacks, damage, timers or lag. **Don't spend another pass looking.** (Useful negative: the lag fix must come from measurement, which is where it did come from.)
- **Anchor-top-only is his centring mechanism** [STATED]: *"since I didn't pick a side, it just sits in the middle."* **We already do this correctly** — verified `src/main.rs:6331` `layer.set_anchor(Anchor::TOP)`.
- **Real bug it exposed:** `stage_screen_origin()` (`src/main.rs:888-891`) computes `(LOCKSCREEN_SIZE.0 - STAGE_W)/2` from the **constant 1504**, not the live output size. The pill *looks* right because the compositor centres the surface, but the **frost sampling origin** is derived from a hardcoded width — exactly the 246-device-px mis-sample critic #5 predicted. Fix: read the output's logical size.
- **Exclusive zone confirmed correct as-is**: we reserve `pill_strip_px()`, not the 636-tall stage (that would cost 63% of a 1002 px desktop).
- **OPEN QUESTION, not a finding:** his bar may be on layer `top` (*"you can go through full screen over your bar"*) where we are `overlay`. Worker labelled it **INFERENCE** and did not check frames, so it is **not promoted**. Test: does the reference pill vanish over fullscreen video? No known frame contains fullscreen video.
- **Do not propagate T1's margin rule** — he states it then disproves it 20 s later; both halves are in the captions.

**Worker B (taste) — 53 quotes machine-checked verbatim:**
1. **Contrast is a budget**: *"contrast is basically deciding where people look first"*; blur a screenshot — *"whatever that you can still see in the blurry version is what your desktop is shouting about."*
2. **One accent**: *"your accent and you only get one"*, *"the only saturated color in the whole palette"*, reserved for *"you are here"*. Base/panels/inactive borders *"really close to each other"* — *"this layer's job is to be quiet"*. Set the palette once: *"all of it pulls from that same palette."*
3. Clock *"bigger and bolder than everything else in the bar"*; other data *"smaller and dimmer"*.
4. **Single island**: *"why not have everything pop up out of one single island"*; *"The button itself actually opens up into the menu"*.
5. *"just make your panel solid… the wallpaper stays behind everything."*
**The "one trick" = deliberately spend contrast so only the active workspace and the clock survive the blur test.**
**STATED contradictions with our tree:** four-file accent duplication vs "palette once"; saturated teal bloom vs one accent reserved for "you are here". **NO statement** on glow/bloom, UI gradients, shadows, rim-light, waveform or animation numbers — so those remain *our* judgement, not his authority. Honest separation.

### ⛔ BLOCKER the parent found while verifying — the lock screen does not lock
Verified in **three** places, verbatim:
- `src/main.rs:9` — *"lockscreen.rs — a visual harness, NOT a session lock (no ext-session-lock surface, no PAM, no keyboard input yet)"*
- `src/main.rs:2320` — *"real session lock — no ext-session-lock surface, no PAM"*
- `src/lockscreen.rs:8` — *"This module is draw-only"*
`grep -rn 'pam\|PAM' src/*.rs Cargo.toml` → **only comments**. **It is a picture of a lock screen.**
**Severity call (chain 5t):** the owner asked to *replicate* the lock screen — a visual ask — so under pure replication this is not a defect. But it is named `lockscreen`, driven by a `lockscreen` socket word, and a user who believes it locks and walks away has an **unlocked machine**. That is a security-expectation mismatch.
**Decision: do NOT build a PAM/session-lock stack tonight.** That is a large risky subsystem (auth, keyboard grab, per-output surfaces, lockout failure modes) the owner did not ask for. The complete-and-lazy move is (a) make the UI honest so nobody mistakes it for a lock, and (b) document exactly what a real lock needs so the owner can decide. MangoWM implements `ext-session-lock-v1` (`docs/mango-capabilities.md`), so the path exists.

### #27 lockscreen critic FAILED — `[acp-proxy: the turn degenerated again after the continuation nudge]`
Second failure for this slot (first was the vision-route death). No output file written. **Not re-dispatched yet**: the parent has now verified the decisive fact itself (draw-only, no PAM), so the slot's highest-value question is already answered. Will re-dispatch on a different route for the *visual* comparison only, with a tighter scope.

### Parent measurement — r1b settled, quiet screen (`r1b-settled`)
Drove `collapsed`, waited 3 s, recorded through the repaired instrument (state witness now gates it).
| metric | reference | baseline | **now** | gap |
|---|---|---|---|---|
| pill w×h | 111×42 | 141×35 | **100×38** | — |
| **aspect** | 2.643 | 4.029 | **2.632** | **0.4%** ✅ |
| cap/pill-h | 0.357 | 0.486 | 0.342 | −4.2% |
| **stroke/cap** | 0.200 | 0.154 | **0.231** | **+15.5% (overshot)** |
| lapvar_norm | 1277.0 | — | 1784.4 | +39.7% |
| RSS | — | 44.2 MB | 55.3 MB | transient, see #26 |
| **idle CPU** | — | 2–9% | **0.5%** | ✅ |
**Aspect is essentially solved** (4.029 → 2.632 vs 2.643). **Idle CPU is solved** (2–9% → 0.5%). The remaining sharpness gap is **weight, not rasterisation**: stroke/cap went 0.154 (too thin) → 0.231 (too heavy), passing straight through the 0.200 target. That is a one-value correction, not a redesign.
`lapvar_norm` rose 1665.7 → 1784.4 while stroke stayed 0.231 — consistent with the pill growing 96→100 px wide (more ink, same weight), not a regression. **Do not read lapvar alone**; the standing instruction holds.

### #27b lockscreen visual — score 4 (`group/hy4`, narrow brief, vision confirmed)
Retry after #27 died twice. Credible: it **saw the images** (describes the teal frosting varying across x, distinguishes the comparison foil) and confirms the parts we got right — **no avatar/username/date/bottom box, no frosted card**, keyhole descent and clock pop encoded.
- **blocker** — clock is a **flat fill**: blits `C_WHITE` over the glyphs then `C_CYAN` as a left-edge gradient (`src/lockscreen.rs:1000-1008`). Cannot produce wallpaper-sampled frosting. Fix: sample wallpaper beneath each glyph at 1.030× centred zoom, blur the glyph-only mask ~24 px, apply the vertical veil, composite through the glyph mask; **remove the flat white base and cyan overlay**.
- **major** — clock **~16% too wide**: `CLOCK_PX_FRAC 0.27*h` with `4*0.60 + 0.21` em advances ⇒ ~1015 px at 2560×1440 (**39.6%**) vs measured **872×279 (34%)**. Height is close ⇒ **retune horizontal advances, do not shrink the whole clock** (`src/lockscreen.rs:29, 188-193, 411-425`).
- **major** — clock typeface is **Quicksand Bold**, creator states **SF Compact Display** (`src/lockscreen.rs:20, 29, 77`). ⚠️ **Parent caveat: SF Compact Display is an Apple font we do not have and cannot ship.** The pill clock already uses baked Inter-Medium. Do NOT instruct a fixer to load a font that does not exist — it will silently fall back. Treat as "match the reference's letterform proportions", verify by measurement.
- **minor** — dots too large: `DOT_D_1080 = 9.0` → 12 px at 1440 vs measured **8–9 px**; pitch 12@1080 → ~16 px is **correct, keep it** (`src/lockscreen.rs:228, 898-903`).
- **minor** — wrong-password shake is a **static +4*s offset**, no alternation or decay (`src/lockscreen.rs:53, 234, 353-355, 874-879`). Needs a decaying alternating dx over ~150 ms.

### #28 user-additions critic FAILED — `WorkBuddy AI: Too many requests, please try again later.`
First failure for this slot (RPM on `group/hy4`). One retry allowed; will re-dispatch on a different route rather than hammer the same provider.

### New instrument: `scripts/blurtest.py` — his own acceptance test, made runnable
He states the rule twice (*"contrast is basically deciding where people look first"*, *"whatever that you can still see in the blurry version is what your desktop is shouting about"*). This blurs a capture and ranks the survivors, so the rule is **checkable instead of a matter of taste**.
**Three defects found by running it, not reading it** (all fixed, committed):
1. A **global median** baseline labels half of every image as a survivor — including the wallpaper itself (first run returned one 1,730,800 px "survivor" spanning the whole frame). Use a **much wider blur as the local neighbourhood**; broad wallpaper gradients cancel.
2. **Sign matters**: a black pill on bright wallpaper shouts as loudly as a bright tile on a dark one. Rank by magnitude, report the sign.
3. **Sigma must scale from the FULL frame height**, else `--crop` to the bar strip shrinks the blur to 2 px and turns the test into an edge detector (74 spurious survivors → 5).
**Reference bar strip** (`s17.png`, crop 0,0,2560,140, σ 21.6): **5 survivors**, top one dark shout −99 area 89,600 spanning x[770,1530] — the pill plus its neighbours reading as one quiet dark band. Nothing saturated survives. **That is the target: few, dark, low-magnitude.**

### #29 portability + packaging — score 4 (`logfare/deepseek-v4.1-flash-1017`)
**Best-evidenced verdict of the round.** It did not read code and guess — it **built a fresh clone in `/tmp/portcheck` (0 errors, 62 warnings)** and **ran the binary with an empty `$HOME`**.
**⛔ BLOCKER, parent-reproduced verbatim:**
```
HOME=/tmp/emptyhome2 ./target/release/dynamite-rust
thread 'main' (18053) panicked at src/main.rs:6299:10:
no Quicksand font file parses
```
The **entire UI font** resolves from `$HOME/.local/share/fonts/{Quicksand-Bold.ttf,Quicksand.ttf}` only, then `.expect(...)`. `git ls-files | grep -ci quick` → **0**. **The product does not start on any machine but this one.** It dies *before* reaching Wayland, so this is the first thing a new user hits. Silent secondary defect: the fallback accepts `Quicksand.ttf` (**wght 300 Light**) when Bold is missing, changing every panel's metrics with **no log line**.
Fix shape: track the fonts under `assets/fonts/` (OFL, redistributable), resolve exe-relative like `clock_font()` already does, **embed the Bold bytes as the last candidate** so no filesystem font is ever required, drop the `.expect`, log the winner.
Other verified findings:
- **major** — no root README, no `build.sh`/Makefile/justfile, no sample config; `STATUS.md:1-6` still says *"v0.1 prototype: collapsed clock pill only"*. Two crates need two build commands and nothing says so.
- **major** — `tools/dictation/EMBEDDING.md` documents `--offline` but a cold `CARGO_HOME` fails (`no matching package named \`anyhow\` found`). **The repo documents its own hole instead of closing it.** Online build works (132 crates).
- **major** — supervisor's `LEGACY` scratch-binary fallback (`tools/dictation/scripts/dictation_supervisor.sh:9,26-33`) silently runs an **unauditable binary** on a fresh clone. **This is my own fallback, added when embedding — the critic is right that it should be a loud error, not a silent substitution.**
- **major** — the whole launcher layer lives **outside git** and hardcodes `/home/alex` + `/run/user/1000`; `~/.config/mango/scripts/dynamite_autostart.sh:51-57` **silently falls back to `qs -c dynamite`**, a *different Quickshell UI*. `git ls-files | grep -E '\.conf$'` → **zero**.
- **major** — **16 external tools** undocumented, no capability check; every spawn is `.spawn().ok()?` so a missing `bluetoothctl` renders a **blank BT row indistinguishable from "no devices"**. `libasound`/`libxkbcommon` are hard **link-time** requirements, undocumented.
- **minor, parent-verified** — **layer-rule name mismatch**: binary sets `NAMESPACE = "dynamite-rust"` (`src/main.rs:1932`), but `~/.config/mango/rule.conf:134` and `docs/mango-capabilities.md:26,122` say `layer_name:dynamite`. **The documented no-blur/no-shadow optimisation matches nothing.**
- **minor, parent-verified** — stale **root `wallpaper_blur.rs` (15,149 B)** is tracked while `src/main.rs:128 mod wallpaper_blur;` resolves to `src/wallpaper_blur.rs` (**62,238 B**). Dead duplicate in git. Plus a tracked `.pyc`, a `.bak`, and 17 motion-output files.
- **minor** — `LOCKSCREEN_SIZE (1504,1002)` compile-time constant; the fullscreen guard is `configured_size.0 >= 1400` (`src/main.rs:4597,4611`) so **the lock animation never draws on an output narrower than 1400 logical px**. Confirms worker A's frost-origin finding from a second direction.
**Genuinely good, credited**: config parsing clamps every numeric key and ignores unknown ones — garbage input cannot break startup; secrets clean; every external tool degrades rather than crashes.

### ⚠️ Note for the next brief — a font the critic asked for does not exist here
#27b asked to load **SF Compact Display** for the lock clock. `fc-list | grep -iE 'sf.?compact|sf.?pro'` → **empty**; `assets/fonts/` holds only `Inter-Medium.ttf`. **Do not instruct a fixer to load a font that is not present** — it will silently fall back and the finding will look "fixed". Treat it as *match the reference's letterform proportions*, verified by measurement.

## r1 wake — #29 portability (score 4), #27b lockscreen (score 4), #28b pending
- #29 credible: built a fresh clone AND ran with empty $HOME. Blocker parent-reproduced:
  `panicked at src/main.rs:6299: no Quicksand font file parses`. Assets half FIXED by parent
  (263b807: both Quicksand faces + OFL tracked); loader change left to the fixer.
- #27b width finding CORRECTED by parent (chain 4t): critic compared ADVANCE box to INK.
  Re-measured by colour mask (luminance was contaminated by wallpaper): ref ink 868x279,
  ours 963.6x280. Height already right (+0.4%); width +11.0% not +16%.
  Solved: DIGIT_ADVANCE_EM 0.522, COLON_ADVANCE_EM 0.198 -> -0.01% error.
  NEW: reference pitch is NON-UNIFORM (123/138.5/166.5/219.5) = proportional, not tabular.
  Decision: keep tabular (centred clock must not shift as digits change), adopt the solved values.
- Material claim CONFIRMED by parent: glyph interior median [87,235,206], left third over the
  cream mast reads [111,230,201] vs [87,235,206] elsewhere -> wallpaper bleeds THROUGH the glyphs.
  Flat fill (C_WHITE then C_CYAN gradient, src/lockscreen.rs:1000-1008) is genuinely wrong.

## Parent packaging work while r1b holds the tree (no src/ edits)
Fixer r1b still owns `src/` (12 modified files, `cargo test --release` running).
Parent took only the findings that touch no `src/`, each verified by running:
- `263b807` vendored both Quicksand faces + OFL under `assets/fonts/` (loader fix left to fixer).
- `5f2997d` corrected #27b's clock-width finding to ink-vs-ink; solved 0.522/0.198.
- `70f15ae` untracked build artefacts + deleted the dead root `wallpaper_blur.rs`
  (15,149 B duplicate; `src/main.rs:128` resolves to `src/wallpaper_blur.rs`, 62,238 B).
- `aa2569d` root README (prereqs incl. libasound/libxkbcommon, 14-tool table with
  what each buys, 13 config keys, **explicit lock-screen-is-not-a-security-boundary
  warning**) + `build.sh` building both crates, pkg-config preflight.
- `e971ce8` supervisor: scratch fallback DELETED (loud stderr + exit 1), `mkdir -p`
  log dir (found by test: fresh $HOME made the privacy chmod fail silently),
  `--offline` claim removed; `layer_name:dynamite` -> `dynamite-rust` in docs AND
  `~/.config/mango/rule.conf` (backup kept). The documented no-blur rule had matched nothing.
- `a0eb900` `deploy/` ships rule.conf + autostart with ISLAND_HOME/XDG_RUNTIME_DIR,
  no silent `qs -c dynamite` substitution. Missing-binary path proven: exit 1.

## Parent verification of the fixer's new region upscale (chain 3t)
r1b added `upscale_canvas_2x_region` (`src/main.rs:615`), used on the settled path
(`src/main.rs:5376`); morph/lock still take the full-stage path (`:5374`).
**New failure mode the full-stage version cannot have: a seam at the rect boundary.**
The blend reads `i-1`/`i+1`, so a rect starting mid-image must still read source
pixels outside it. Parent extracted the three fns verbatim and asserted, for every
rect, every interior pixel equals the full-stage result:
**847 rect cases across 8 sizes, 0 mismatches** (`scripts/perf/region_exactness.rs`,
commit `0d71059`). Includes 1x1, 1xN, Nx1, odd sizes, edge-touching and fully-outside rects.
So the optimisation is safe; the remaining morph cost is untouched, not broken.

## r1 wake — #28b user additions (score 6). All three headline findings parent-verified.
Credible, not generous: it ran `bluetoothctl show` against a dead system bus to prove
the failure path, and read the owner's own bind.conf. Verified in tree:
- **blocker** `src/controlcenter_panel.rs:932` `bluetooth_on: live_bluetooth_on().unwrap_or(true)`
  — unknown adapter renders as accent-blue "On". The other three toggles default false.
  Fix: `.unwrap_or(false)` + tri-valued Unavailable ("No adapter", disabled pill).
- **major** `WS_COUNT = 4` (`src/main.rs:953`) but `~/.config/mango/bind.conf:78-82`
  bind SUPER+5..9 to `view`. Tag 5..9 ⇒ **every disc dark**, the creator's one
  non-negotiable "you are here" cue vanishes. Reachable by a documented keypress.
- **major** `client_count`/`is_urgent` are returned by `mmsg get all-tags` and read by
  nobody (`parse_active_tag`, `src/main.rs:1267`) ⇒ occupied == empty. Data already paid for.
- **major** BT tile navigates (`CcBtOpen`) while its 3 identical neighbours toggle
  (`src/main.rs:1673`) — 4 interactions for a one-bit change.
- **minor, verified** two blues on one screen: `WS_ACTIVE_FILL = (147,164,245)`
  (`src/main.rs:970`) vs `library::ISLAND_ACCENT` = (124,162,245). **26 units on red**,
  same as the parent's earlier measurement — independent confirmation. Fix: bind the
  const to `library::ISLAND_ACCENT`, do NOT change the hue.
- **minor** disc drop-shadow: WS_DISC_SS=8 ⇒ ~170k shadow evals/frame under an opaque
  disc on a near-black bar, invisible. Reference discs have no shadow. Delete.

## ⚠️ Fixer r1b FAILED (transport) — tree verified sound before re-dispatch
`YJS: Upstream error` — provider transport, not a refusal. Unlike a critic, a fixer
dies holding a dirty tree, so the parent verified the inherited state before spending
another child: **`cargo build --release --offline` 0 errors / 61 warnings**, and
**`cargo test --release --offline` 303 passed / 0 failed**. The tree is sound.

### Instrument repair #11 — the state witness was silently dead (commit `12df3cf`)
`measure.py::panel_state()` read only `DBG enter state=`, which is printed on
**POINTER ENTER** (`src/main.rs:5653`). A panel driven purely over the socket never
emits it ⇒ the guard returned `None` and **fell back to the pixel heuristics it exists
to replace**, while looking alive. This is the seventh mis-score class.
Fix: also read `DBG morph A -> B (morph_running=…)` (`src/main.rs:4764`), which fires
on every state change and is already a contract the stress harness depends on.
Second trap: that line is printed **BEFORE** the animation and there is **no completion
line**, so `morph_running=true` stays true forever ⇒ trusting it refuses every
measurement. Added `settled()`: two captures 200 ms apart (> anim_ms 160) compared
**in the bar strip only** — a whole-screen compare never settles because a terminal
cursor elsewhere keeps changing (measured: 2040 changed px at y[987,1132], far below
the bar). New `scripts/measure_witness_selftest.py`: **10/10 PASS**.

### Verified state of the product at r1b's end (`r1b-final-verified`, quiet)
pill **99×38**, aspect **2.605** (ref 2.643, 1.4% off), centre offset **0.0 px**,
cap/pill **0.342** (ref 0.357), stroke/cap **0.231** (ref 0.200, **+15.5% too heavy**),
lapvar_norm **1609.55** (ref 1277.0, +26.0%), **RSS 30.8 MB**, **idle CPU 0.5%**.
Baseline was aspect 4.029 / RSS 44.2 MB / CPU 2–9%. Geometry and CPU are essentially
solved; the remaining sharpness gap is **stroke weight**, one value.

## ⛔ PARENT SELF-CORRECTION #9 — the stroke-weight finding was an artefact (chain 4t)
Carried for hours and handed to two fixers: "stroke/cap 0.231 vs 0.200, +15.5% too
heavy, reduce weight". **Wrong.** Stem = stroke/cap × cap:
reference 0.200 × 15 = **3.00 px**; live 0.231 × 13 = **3.00 px**. Identical ink.
`0.231 = 3/13`, `0.200 = 3/15` exactly. The ratio's denominator is cap height, so a
smaller clock reads "heavier". The real defect was **cap 13 vs 15 — the clock was too
SMALL**. Same error class as the advance-vs-ink comparison: a ratio read as an absolute.
**Solved by simulation through the real pipeline** (2× raster → 0.75 downsample, shipped
Inter-Medium.ttf): `bar_height=28` ⇒ text_px 13.56 ⇒ **cap 15, stroke/cap 0.200,
text_w 55** vs reference **15 / 0.200 / 54**. With `collapsed_width=74` ⇒ pill **111×42**,
exactly the reference. **Two config values close the entire sharpness gap; no font change.**
Recorded in `docs/r1c-brief-draft.md` §5b (commit `d588998`).

## ⛔⛔ PARENT SELF-CORRECTION #10 — §5b's "74x28" was wrong; geometry is already DONE
Matched absolute device px across screens of different width. Scale-invariant truth:
reference 111/2560 = **4.336 %** of screen width; ours (67x25) 98/2256 = **4.344 %**
— **0.2 % off**. My 74x28 target = 111/2256 = 4.920 % = **13.5 % too wide**.
Proportional cap = 15x38/42 = 13.6 vs our 13 ⇒ cap/pill_h 0.342 vs 0.357 is **one
pixel of quantisation** (13->0.342, 14->0.368; 0.357 unreachable). Proportional stem
2.71 px vs our 3 — a 2.71 px stem cannot exist. **"Too heavy" and "too small" were the
same error.** Collapsed geometry is finished; config stays 67x25.
Caught BEFORE the fixer applied it (r1c died on dispatch). Brief §5c records it.

## Fixer r1c FAILED on dispatch — `YJS: Upstream error`, model swap
Second consecutive failure on `magpie/group/deepseek-v4.1-flash-1017` (r1b died
mid-round, r1c died instantly). Per the skill's two-strike rule the fixer model swaps.

## ★ ROOT CAUSE of "it looks really bad compared to his setup" — NOT our code (chain 4t)
The owner's complaint was partly a **desktop state problem, not a panel defect**.
Measured: the entire top band of every screenshot is flat **RGB(20,20,36)**, full
screen width, including x=5 and x=2250 — far outside our 1764-device-px stage.
`~/.config/mango/config.conf:115` **`root_color = 0x141424ff`** = **(20,20,36)** exactly.
`swaybg` IS installed (`/usr/bin/swaybg`) and `~/.config/mango/wallpaper/wallpaper.png`
exists (765 KB), and `autostart.sh:58-60` would launch it — **but no swaybg process is
running** (`pgrep -a swaybg` empty). So the compositor's bare root colour is on screen.
Consequences, all previously mis-attributed to the panel:
- The reference frames have a **bright blue sky + white cloud** behind the pill; ours
  have a flat near-black band. Any A/B "ours looks worse" judgement was comparing a
  pill on wallpaper against a pill on nothing.
- `scripts/stress` detector repair #8 (DARK 40 -> 12) was needed **because the wallpaper
  was absent**, not because the owner chose a dark wallpaper.
- The frost path samples the wallpaper FILE (`src/wallpaper_blur.rs:404`), so the pill
  material itself is unaffected — which is why the pill still matched.
**This also means every "flat black backdrop" screenshot in the run is not representative.**
Not fixed by the parent: starting a wallpaper daemon changes the owner's live desktop,
which is theirs to choose. Recorded as an environment finding + a one-line remedy.
**Composite caveat**: pasting a rectangular pill crop onto a different backdrop shows a
rectangular halo around BOTH pills (the crop carries its original background). That is a
compositing artefact of the parent's A/B image, **not** a panel defect. Do not score it.
A fair visual comparison needs the wallpaper daemon running so both are captured live.

### Remedy for the owner (one line, NOT applied — it changes their live desktop)
```sh
swaybg -i ~/.config/mango/wallpaper/wallpaper.png -m fill &
```
Already present in `~/.config/mango/autostart.sh:58-60`; it simply is not running in this
session. Until it is, every screenshot shows `root_color 0x141424` behind the bar and
**no visual critic can fairly judge the bar against the reference**.

## ⛔ PARENT SELF-CORRECTION #11 — the lapvar_norm "+26% too sharp" gap is a VIDEO artefact
Carried since the first critic and quoted by five slots as evidence our glyphs are wrong.
**It is not a rendering defect.** Chain of checks, each run:
1. **Same-string ink area, cap-normalised** (removes the "00:05" vs "03:21" confound):
   reference **1.569** ink/cap², ours simulated through the real pipeline **1.639**
   ⇒ **ratio 1.045**. The glyphs are the right weight and shape, within 4.5 %.
2. **Contrast rescaling changes lapvar_norm by 0.0** — confirms the normalisation works
   and the residual is NOT a contrast effect.
3. **Band-limiting explains it exactly.** Our render through a small Gaussian:
   | blur | lapvar_norm |
   |---|---|
   | 0.0 px | 1005.4 |
   | **0.3 px** | **873.3** |
   | 0.5 px | 674.7 |
   Reference (a compressed video frame): **784.8**.
   The reference sits between our 0.3 px and 0.5 px blur — i.e. **his frame is
   band-limited by video compression**, ours is a clean framebuffer capture.
**Conclusion: do NOT blur our output to chase this number.** Matching a compression
artefact would make the real desktop worse. `lapvar_norm` is only comparable between
two captures of the same kind; ours-vs-video is not that. Keep it as a REGRESSION
detector (live-vs-live), not as a reference target.
Two measurement bugs found and avoided while checking: the `>110` mask on the reference
crop selects the bright WALLPAPER unless restricted to the pill interior (gave cap=42,
the pill height); and glyph-run splitting merges adjacent digits at this resolution.

## 🚨 INSTRUMENT CATASTROPHE FOUND AND FIXED — the reference frame had been overwritten
`/tmp/ref/s/s17.png` — the frame **every visual critic scored against for hours** — was
**2256x1504, our own screen size**, mtime 03:35:45. It had been overwritten by a live
capture. Cause: `scripts/measure.py::settled()` calls `shot(png)` to re-capture, and when
`--png /tmp/ref/s/s17.png` was passed it **wrote the live screen over the reference**.
`scripts/measure_selftest.py:30` pointed at that same path, so the self-test kept
"passing" against our own output.
**Blast radius checked**: of 1486 files under `/tmp/ref/`, exactly **1** was contaminated.
All other reference frames are intact 2560x1440. Every number quoted from s17 before
03:35 was genuine (111x42, cap 15, stroke/cap 0.200, lapvar_norm 1277.0) — those were
measured while it was still the real frame, and they **reproduce exactly** from a clean
frame now.
**Fixes (three, all verified):**
1. `--png` no longer consults the live panel's state (a still cannot be "mid-morph").
2. `settled(png, allow_capture=False)` for a supplied PNG — it can neither settle nor
   be overwritten.
3. **The reference now lives in the repo**: `docs/reference/{pill-settled,panel-power,
   lock-resting,lock-typing}.png`, copied from clean 2560x1440 frames (`s1`, `s13`,
   `l40`, `l120`). `measure_selftest.py` points there. `/tmp` is scratch and was never
   a safe home for the ground truth.
Re-measured from `docs/reference/pill-settled.png`: pill **111x42**, aspect **2.643**,
centre offset **0.0**, cap **15**, stroke/cap **0.200**, width **4.336 %**, gap **1.319 %**.
All canonical targets confirmed against uncorrupted evidence.
**Both self-corrections re-verified against the CLEAN reference**: stem **3.00 px**
(#9 confirmed), width **4.336 %** (#10 confirmed). Neither was poisoned by the
contaminated file.

## Parent verification of r1c's morph-rect change (read-only, before the fixer reports)
The risk in bounding a morph's damage rect is **writing outside it** — stale pixels or a
torn edge. Checked every write in `draw_morph_frame`:
- capsule loop now `rgn.0..rgn.2` / `rgn.1..rgn.3` ✓
- **shadow loop** bounds come from the INTERPOLATED capsule (`ox-m .. ox+fw+m`), not the
  endpoints. Safe **because the live easing is `EaseOutCubic`** (`src/main.rs:4921,4923,
  4955,4957`), which is monotone on [0,1] — the interpolated capsule is always *between*
  the endpoints, never larger. `src/morph.rs:143` explicitly warns callers not to assume
  `<=1.0` for `EaseOutBack`/`Spring`; those are **not wired**.
  Margins: `content_rect` pads menus by `ceil(10+5+14+8+2)=39`; the shadow needs
  `m = 10+max(0,5)+1 = 16`. **39 > 16 ✓**. Vertically both endpoints anchor at
  `gap_top=13`, union top clips to 0, shadow top `13-16=-3` clips to 0 ✓.
- **This safety depends on the easing staying non-overshooting.** If anyone wires
  `EaseOutBack` later, the rect must grow by the overshoot fraction or the shadow will
  tear. Worth a comment at the rect computation.
Also verified: `ui_font()` embeds `Quicksand-Bold.ttf` via `include_bytes!`, logs the
winning candidate, and the three duplicated `$HOME`-only loaders in tests were replaced
with it. `std::process::exit(1)` replaces the panic only if the EMBEDDED font fails to
parse (a corrupt build) — acceptable, and no longer reachable on a clean machine.

## Round 1 panel — extended slots dispatched
| # | lens | model | uuid |
|---|---|---|---|
| #30 | **reference motion spec from 855 video frames** | group/gemini-4-hydrogen | `8500a881` |
| #31 | dictation always-on mic + idle RAM | fasfas-ai/deepseek-v4.1-flash | `ecac75af` |
Fixer r1c (logfare) `2bfdb05f` in flight; it has landed `ui_font()` (embedded
Quicksand-Bold, logs the winner, no panic) and the **bounded morph damage rect**
(`union_rect(content_rect(from), content_rect(to))`, region upscale on morph frames,
`clear_rect` instead of three stage-sized memsets). Parent verified the rect is safe
under EaseOutCubic. Secondary findings (Bluetooth default, workspace tags, accent
binding) not yet applied.

## ★ Measured prediction for r1c's morph fix — `scripts/perf/morph_rect_bench.rs`
Extracted the fixer's actual `upscale_canvas_2x_region` + `clip_rect` from the working
tree and timed realistic morph rects (`rustc -O`, no crate, no `src/` change):
| rect | logical px | time |
|---|---|---|
| full stage (the OLD morph path) | 747,936 | **9.4–11.6 ms** |
| collapsed pill only | 2,059 | **0.03 ms** |
| pill → ControlCenter union | 91,298 | **1.26–1.48 ms** |
| ControlCenter → Themes union | 195,840 | **2.75–3.03 ms** |
**Every realistic morph rect is under 3.1 ms**, i.e. inside a 16.7 ms frame budget with
room for the draw itself. At 160 ms / 16.7 ms the morph should now present **~10 poses**.
Reproduce: `rustc -O -o /tmp/mb scripts/perf/morph_rect_bench.rs && /tmp/mb`.
**Caveat, stated honestly**: this machine's full-stage upscale measures **9–12 ms** here,
not the 62–105 ms the profiler and critic #25 reported. Different build (that was an
instrumented pre-r1b binary) and different load. The *ratio* is the durable result
(**~7× to ~300× cheaper**), and even at 9 ms the old path plus a 2.7 ms draw plus three
stage-sized memsets could not sustain 60 Hz. Do not quote 62 ms as today's number.
**Memset half measured too** (first attempt read 0.000 ms — the compiler eliminated the
dead fills; `black_box` + a dirtying write fixed it):
three stage-sized `fill(0)` = **0.578 ms**; three rect-sized clears = **0.052 ms**.
So the memsets were **~0.6 ms/frame, not the dominant cost** — the upscale was. Honest
attribution: the rect fix buys ~8 ms on a full-stage morph and ~0.5 ms of memset; the
upscale is ~94 % of the saving. My earlier brief implied the memsets were co-equal; they
were not.

## ⚠️ Measurement invalidated: TWO panel instances were running (03:50)
`mmsg get all-layers` listed **two** `dynamite-rust` overlay surfaces; pids 5649
(`./target/release/...`, started 03:50:00) and 6456 (absolute path, 03:50:11). Both bind
the same socket path, so socket words reach whichever bound last while `measure.py`'s
`pgrep -n -x` picks the NEWEST pid — **the two can disagree**.
Consequences: `scripts/stress/stress.py` returned **RESULT INVALID** (its surface gate
correctly refused to score), and `scripts/motion/run_all.sh` failed 5 stages with a JSON
decode error. **Neither is a product verdict.** The instruments behaved correctly.
Cause: a fixer/child restarted the panel while another instance was already up. Not the
parent's doing and not a code defect — but it means **any measurement taken while two
instances exist must be discarded**.
**Action for the next round**: the harnesses should refuse to measure when more than one
instance is live, exactly as they already refuse when none is. Filing that as an
instrument gap rather than silently re-running.
**Instrument repair #12 (commit after `030dd54`)**: both harnesses now refuse an
ambiguous target, not just an absent one.
- `scripts/stress/stress.py::surface_present()` counts `"name":"dynamite-rust"` entries;
  returns True only for exactly 1, and the INVALID message names the real reason
  ("2 dynamite instances are live; they share one socket path…").
- `scripts/measure.py::pid()` switched from `pgrep -n -x` (newest) to `pgrep -x` and
  returns `None` when more than one matches, so RSS/CPU are never attributed to the
  wrong process.
Stubbed self-test: none → False/0, one → True/1, two → False/2. All three PASS.
Existing self-tests still PASS (measure 5, witness 10, stress detector 7).

## Provider loss: gemini-4-hydrogen is DEAD for this account (04:24)
Critic #30 (reference motion spec) failed with:
`agy bridge: Google has disabled this account's access to Antigravity for violating
the Terms of Service (HTTP 403)`. **Not a capacity blip — an account-level ban.**
`magpie/group/gemini-4-hydrogen` must not be used again this run. It passed the
5/5 vision probe earlier, so the route worked and then was revoked mid-run.
Remaining confirmed-seeing routes: `magpie/group/hy4` (65 K context, bounded frame
sets only) and `magpie/agentrouter/gpt-6.1-sol` (expensive — owner warned).

## The socket orphaning bug, and the second bug it hid (04:00–04:20)
**Bug 1**: `let _ = remove_file(&sock_path); bind(&sock_path)` — a second instance
unlinked the incumbent's socket and rebound the path. Verified: `/proc/net/unix`
listed **two** bindings for one path; sending one word burned ticks in **both**
processes (5649 +2, 6456 +44). Two surfaces, one reachable socket.
**Fix**: probe with `UnixDatagram::unbound()?.connect(&path)` BEFORE unlinking;
if something answers, log and `exit(0)`. Stale files from a crash still removed.
Tested: second start exits 0, socket inode unchanged (2787 → 2787).
**Bug 2 (found by killing the orphan)**: shutdown ran `remove_file(&sock_path)`
unconditionally, so the **orphan deleted the keeper's socket on its way out** —
the panel went unreachable (`[Errno 2] No such file or directory`) and every
motion stage failed. **Fix**: record the bound socket's inode at bind time
(`socket_inode`, `fstat`), and at shutdown unlink only if `metadata(path).ino()`
still matches (`owns_socket_path`). The socket moves into the event loop, so the
inode is captured before the move — first attempt borrowed it after and failed
`E0382: borrow of moved value: datagram`.

## Test repaired: workspace disc span 24 → 23 (the shadow's row, not the disc's)
The fixer removed the disc drop shadow (critic #28b: ~170k evals/frame under an
opaque disc on near-black, and the reference discs cast none). That broke
`iter109_tests::workspace_circles_coverage_round`: span 23, want 24.
**Verified the disc did NOT shrink** before touching the expectation:
profile `[10,14,16,18,20,20,22,22,24×7,22,22,20,20,18,16,14,10]` —
**perfectly symmetric**, widest **24**. A shadow at offset **(2,3)** could never
produce a symmetric profile; it added one row of alpha>128 BELOW the disc.
So 24 was shadow-inflated and **23 is the true disc**. Updated the expectation
and ADDED a symmetry assertion, so a returning shadow or a misplaced centre
fails loudly instead of silently changing the span.
Also fixed a stale call site: `draw_workspace_disc` gained an `occupied: bool`
parameter (the fixer's client_count work) and the test still passed 8 args.
**`cargo test --release --offline`: 304 passed, 0 failed.**

## Instrument repair #13 — `ss -xlp` cannot see a bound datagram socket
Two failures in the 04:26 full run (`RESULT FAIL: liveness, protocol_fuzz`,
tests=1207 asserts=6179 fails=2) were **the harness, not the panel**:
`socket kind is None, expected dgram` and `socket is no longer a datagram socket`.
**Cause**: `ss -xlp` — the `-l` flag lists only **LISTEN** sockets, and a bound
`SOCK_DGRAM` has no LISTEN state. It never appeared. `ss -xa` shows it plainly:
`u_dgr ESTAB … /run/user/1000/dynamite-rust.sock`. `/proc/net/unix` had it all along.
This also silently broke `socket_owner()` (same `-xlp`), so the harness could not
identify the process it was driving — it fell back to `pgrep`.
**Fix**: `socket_kind()` reads `/proc/net/unix` column 4 (`0002`=dgram, `0001`=stream);
`socket_owner()` uses `ss -xap`. Verified both against the live pid (28880).
**Why it only surfaced now**: the earlier runs used a panel started before the
single-instance guard; the restart changed which socket state `ss` reported.

## Instrument repair #14 — the leak baseline straddled the allocation ramp
04:31 run: `load_leak FAIL tests=200 fails=39`, every message
`iteration N: RSS +11.8 MB over warmup median (bound 8 MB)`.
**Not a leak.** The decile curve was `51 96 96 52 85 85 85 85 85 85` — flat at
85 MB for 160 iterations, and `growth_after_warmup=+0.1`. The one-time
allocations (lock wallpaper, shm slot ring) land ~5 iterations in, so
`median(rss[:20])` straddled the ramp and sat BELOW the steady state.
**Fix**: baseline is `median(rss[20:i+1])` — a running post-warmup median that
converges to the steady state. The summary `base` likewise uses the tail.
**Mutation-tested rather than assumed** (`flat` / `linear +0.2 MB/iter` /
`step +25 MB at iter 110`): flat → **0 fails**, linear → **99 fails**,
step → **90 fails**. The gate still catches both leak shapes.

## Instrument repair #15 — the output's LAYOUT ORIGIN moved; every crop missed it
04:40 motion run captured **0 frames** in every stage. `errors.log`:
`supplied geometry did not intersect with any outputs` (repeated).
`mmsg get all-monitors` → eDP-1 is at **x=0, y=1543**, not 0,0. Every hardcoded
`y=0` crop therefore fell outside the output. `grim -g "0,1543 1504x200"` works.
**This is the same class as the reference-frame overwrite**: an instrument
assuming a fixed world instead of querying it.
**Fix**: `scripts/motion/run_all.sh` and `scripts/motion/capture.sh` both query
`mmsg get all-monitors`, take the active monitor's `x,y`, and offset the crop
into layout space. Derived region is now `552,1543,952,1753`; capture.sh
default went from 0 frames to **11 frames in 1 s at 20 fps**.
Also fixed a latent path bug: run_all.sh called `../../scripts/measure.py`
relative to CWD, which only worked from one directory.

## #31 dictation mic/RAM — score 6. Two findings REFUTED, one confirmed and worse than reported.
**✅ CONFIRMED, and the real cost is not RAM**: the mic is open from login and
**nothing can ever close it** — `tools/dictation/src/main.rs:90`
`capture::start().expect(...)`; `tools/dictation/src/capture.rs:57-103` parks the
owner thread forever, no Drop/stop channel/JoinHandle, `AudioIn` is Clone-only
holding a Receiver. Live proof: PipeWire node 92 `alsa_capture.static-dictation`
state R; `/proc/asound/card0/pcm0c/sub0/status` RUNNING, appl_ptr +145384 frames/3 s.
**The battery cost**: `/sys/bus/pci/devices/0000:00:1f.3/power/runtime_status`
stays `active` with `power_save=1`, `power_save_controller=Y` and playback closed
⇒ the open capture is the ONLY thing keeping the HDA codec awake. Invisible in
every RSS/Pss number. Idle: dictation Pss **1572-1597 kB**, 28 threads, 102 fds,
**96-99 ctxt/s**; panel Pss **30816 kB**, 10-11 threads, **1 ctxt/s**.
**Not a privacy leak**: idle audio is drained and discarded (`main.rs:108-118`,
the `if recording` gate is INSIDE the frame loop), never persisted or sent — all
10 sockets are unix. That drain IS the 100/s context-switch rate.
**⛔ REFUTED — "the panel binary has no dictation_start literal"**: parent sent
both words to the live panel and got `DBG morph Collapsed -> Dictation` and
`DBG morph Dictation -> Collapsed`. LLVM compiles `word == "literal"` into an
inline length check + immediate compares, so **no string constant exists** —
`strings` finding 0 is expected, not a defect. End-to-end works.
**⛔ REFUTED (partly) — "stress reads a stale log"**: `scripts/stress/stress.py`
already follows `/proc/PID/fd/2` via `stderr_target()`. But the parent's OWN ad-hoc
probes read `~/.cache/dynamite.log` while the panel writes **`.log.1`** (rotated
out from under it) — that is why my manual checks looked dead. Harness was right.

## #30b reference motion — score 4, and it CORRECTED MY BRIEF
**The 855 `dense/` frames are 5 fps (Δ200 ms), NOT 60 fps.** I briefed them as
sequential video frames. All source videos are **60/1 fps** (ffprobe), so the
extraction was decimated. **Reference pose counts at source rate were therefore
never measured** — the child said so instead of inventing a number.
Clean morph windows: power open/close **400 ms** (d00387-d00389, d00403-d00405),
wide panel **600 ms** (d00210-d00213, d00225-d00228), ±200 ms quantisation.
First-move-to-settled **200–400 ms** — **our 317 ms is already inside his range**;
raising `anim_ms` without adding poses would not fix the lag.
**EaseOutCubic is right**; the only clean overshoot is **+1.5 % width / +1.2 %
height** at d00212 (not 15 %). Clock ink vanishes by the first morph frame and
returns at settle; panel content is already present during growth.
**Action taken**: re-extracting the island video at **native 60 fps**, top-strip
crop `900:220:830:0`, into `/tmp/ref60/` so pose counts are finally measurable.

## Supervisor give-up loop caused by MY guard (04:44–04:50)
The single-instance guard exited **0**; the supervisor counts ANY sub-30 s run as
a rapid exit whatever the rc, so 10 strikes hit `GIVE-UP CAP REACHED … backing off
to 300s`. **Fix**: guard now exits **75 (EX_TEMPFAIL)**, and both the live
`~/.config/mango/scripts/dynamite_autostart.sh` and the repo copy treat rc=75 as
"another instance owns the socket" → log and `exit 0`, no strike. Backup kept.

## Instrument repairs #16, #17 — two more false verdicts, both harness-side
04:59 run: `TOTAL tests=1207 asserts=6179 fails=3` / `FAIL: load_leak, state_machine`.
Neither was a product defect.

**#16 `state_machine` NO EFFECT was a log-rotation blind spot.**
`pair-reset collapsed: no '-> Collapsed' morph line within 2.0s (last confirmed
state b'traymenu')`. Parent reproduced traymenu→collapsed at gaps
0.00/0.02/0.05/0.20 s — **all four produced the line** — and replayed the exact
11-word/20 ms pair cadence **4/4 OK**. So the panel was never at fault.
Root cause: `~/.config/mango/scripts/dynamite_autostart.sh:71-79` rotates the
log with `mv` at 256 KiB while the panel holds the fd open, so
`/proc/PID/fd/2` re-resolves to a NEW path with a NEW inode mid-wait.
`wait_effect` only handled `cur[1] == start_id[1]` (same inode, grown); an
inode change fell through BOTH branches and spun to the budget.
**Fix**: on inode change, re-anchor and scan the new target from its start.

**#17 `load_leak` +11.6 MB at iterations 50-51 was a transient, not a leak.**
Curve `62 85 85 85 85 84 85 85 85 85`, `growth_after_warmup=-0.1`. Two samples
spiked and returned to the same 85 MB for 150 iterations — a panel reload.
**Fix**: the bound now requires the excess to hold for **3 consecutive
samples**; non-persistent spikes are reported as `transient RSS spikes`.
**Mutation-tested**: flat → 0 fails · 2-sample spike → **0 fails, 1 transient**
· linear +0.2 MB/iter → fails · persistent +25 MB step → fails.

Latency unchanged and excellent: p50 44.9 / p95 85.7 / p99 127.5 / max 140.4
vs 400 ms bound.

## Round 1 status — 18 slots scored, median 4.5, gate >=9 NOT MET
| slot | lens | score |
|---|---|---|
| #1 | glyph sharpness | 6 |
| #2 | screenshot A/B (vision) | 5 |
| #3 | geometry | 3 |
| #4 | typeface | 7 |
| #5 | fill / frost / slop | 6 |
| #6 | idle CPU + redraw proof | 4 |
| #8 | memory | 3 |
| #11 | stress / does-it-reproduce | 3 |
| #21 | dictation embedding | 6 |
| #22 | stress-harness integrity | 4 |
| #23b | motion-harness integrity | 3 |
| #25 | rust idiom + panic safety | 7 |
| #26 | memory + shm budget | 5 |
| #27b | lockscreen vision | 4 |
| #28b | owner's additions (BT/workspaces) | 6 |
| #29 | portability | 4 |
| #30b | reference motion spec | 4 |
| #31 | dictation mic lifecycle + RAM | 6 |

Sorted: 3,3,3,3,4,4,4,4,4,**5**,5,6,6,6,6,6,7,7 → **median 4.5**, mean 4.78.
Fixer r1c (`logfare/deepseek-v4.1-flash-1017`, thinking max, `2bfdb05f`) still
running; its morph-rect + embedded-font work is verified in the tree.

### Provider losses recorded
- `magpie/group/gemini-4-hydrogen` — **BANNED**: `HTTP 403 … Google has
  disabled this account's access to Antigravity for violating the Terms of
  Service`. Do not dispatch again.
- `magpie/group/deepseek-v4.1-flash-1017` — two fixer deaths, `YJS: Upstream
  error`. Same model on **logfare** works; swapped per the two-strike rule.
- `magpie/logfare/glm-5.3*` — image-blind at runtime; capacity/RPM failures.
Remaining vision routes: `magpie/group/hy4` (65 K context) and
`magpie/agentrouter/gpt-6.1-sol` (expensive — owner warned).

## Instrument repair #18 — judge an RSS excursion by whether it RETURNS
05:07 run still failed `load_leak` at i148 after repair #17. Curve
`58 96 85 85 85 86 85 85 96 85`: the excursion spans i146..i150 (~5 samples,
~1.6 s) and then sits at the **same 85 MB for 50 more iterations**,
`growth_after_warmup=+0.0`. A 3-sample persistence rule still called it a leak.
**A leak never returns to its baseline; a wallpaper reload does.**
New rule: an excursion is recorded when it ENDS (returns within the bound) and
reported as `RSS excursions that returned to baseline (not leaks)`. A failure
requires the **20-sample median** to also be over the bound — half a second of
spike cannot move a 20-sample median, a real climb does.
**Mutation-tested**: flat → 0 · the real 5-sample reload → **0 fails, 1
excursion** · linear leak → fails · persistent +25 MB step → fails.
Known limit, stated rather than hidden: a 25-sample excursion that later
returns is still flagged. `shortcut: 20-sample median window, widen if a real
reload ever exceeds ~6 s`.

Also: `state_machine` seeding now falls back to **asking the panel** when
`grim` fails, instead of treating a failed capture as "not collapsed" — three
groups logged `liveness …: grim failed` under concurrent load.

## Instrument repair #19 — stop tuning the threshold, measure the thing
Three successive tunings (#17 3-sample persistence, #18 return-to-baseline,
then a quarter-median rule) each still failed a wallpaper/lock reload. All
three judged an excursion by its **duration**, and a reload can last
arbitrarily long. Stopped tuning and measured what the +11.5 MB actually is:

| state | VmRSS | Pss |
|---|---|---|
| collapsed | 33.5 MB | 30.2 MB |
| lockscreen | 84.8 MB | 58.9 MB |
| unlocked | 33.1 MB | 30.0 MB |

**VmRSS swing +51.3 MB, Pss swing +28.6 MB.** The shm maps go
`(Size 22 MB, Rss 0)` → `(45 MB, 45 MB)` → back. VmRSS counts pages the
compositor also maps; it was never the right metric for "as little RAM as
possible". `rss_kb()` now reads **Pss** from `smaps_rollup`, falling back to
VmRSS, and every label reports which metric was used.

**The verdict is now a SHAPE test, not a threshold**: a leak never returns to
its baseline, a reload always does. Judge once at the end —
`median(last 20) - median(rss[20:40])` — and report per-iteration excursions as
observations. The early window matters: against a whole-tail median a slow
+0.08 MB/iter leak read as +6.4 MB and **passed**; against the early window it
reads +12.8 MB and fails.
**`scripts/stress/leak_shape_selftest.py` — 10/10 PASS**: flat, and reloads of
2/5/25/60/100 samples all PASS; linear +0.25, slow +0.08, step +25 at i110 and
i170 all FAIL. Every observed real curve is in the fixture set.

## Fixer r1c FAILED — `The model is temporarily unavailable`
`logfare/deepseek-v4.1-flash-1017` died mid-round after landing verified work
(embedded fonts, bounded morph rect). Third fixer death; two providers now.
Its tree state is sound: build 0 errors, 304 tests pass.

## ★★ Structural finding — the pill GROWS INTO the panel (parent-measured)
Critic #32 (expanded-panel pixel match, `group/hy4`) scored **3** and reported
every expanded panel 18-28% too wide. Parent re-derived from
`IslandState::size()` (`src/main.rs:200-214`) — the numbers agree exactly.
But investigating the "content sits too low" finding exposed something larger:

| | rows |
|---|---|
| `docs/reference/pill-settled.png` pill | **19..60** |
| `docs/reference/panel-power.png` panel | **20..219** |

**Same top edge.** Confirmed independently on the 60 fps morph: y stays
**19-20** across all 14 frames while h grows **42 -> 188** and w **111 -> 825**.
One capsule anchored at the top, growing down and out.

Ours: `src/main.rs:449-460` `panel_top_menu() = STAGE_PAD_TOP + 28 + 2 = 33`,
comment *"the expanded menus hang BELOW the pill … the pill stays visible
above the panel with a 4 px gap"*. **A different design**, and it matches the
creator's own words against it: *"The button itself actually opens up into the
menu"*, *"why not have everything pop up out of one single island"*.

This is the root of "looks bad compared to his setup" for every expanded state,
and no amount of width tuning would have found it.

### Width targets (scale-invariant, parent-derived)
| panel | ours logical | ours %W | ref %W | ratio | target logical |
|---|---|---|---|---|---|
| Power | 400x150 | 26.596 | 21.016 | 1.266 | **316 x 118** |
| Themes | 600x366 | 39.894 | 31.172 | 1.280 | **469 x 287** |
| Launcher | 504x366 | 33.511 | 26.406 | 1.269 | **397 x 287** |
| Wallpaper | 1008x270 | 67.021 | 56.680 | 1.182 | **853 x 84** |
Aspects match within 0.5% for three of four => systematic width error, not a
scale artefact. Wallpaper is additionally **3.2x too deep** (aspect 3.733 vs
10.147) — it is a single row of thumbnails in the reference.

## Fixer r1d dispatched — `magpie/logfare/glm-5.3`, thinking max
uuid `c91d640c-6833-4ff3-a5ac-4b95eb6de48e`. Brief: `docs/r1d-brief.md`
(8 findings, do-NOT-touch list, 6 retractions, 5 required verification commands).
Chain 4t: r1c died on `The model is temporarily unavailable` after landing
verified work; two DeepSeek routes have now failed as fixers, so the third
candidate is GLM 5.3 on logfare — a different model family, not another
transport retry. It is image-blind, but this brief is entirely numeric.

## ✅ Stress gate GREEN — `TOTAL tests=1207 asserts=6179 fails=0 groups=5` / `RESULT PASS`
All five groups pass on the current tree (fixer r1d in flight):
protocol_fuzz 636/0 · state_machine 246/0 · load_leak 200/0 ·
latency 120/0 · liveness 5/0.
**Latency p50 34.9 / p95 44.4 / p99 80.9 / max 92.8 ms** vs a 400 ms bound —
the best measured this run (baseline p95 was 220.7, and one contended run
read 411.9).
Memory now reported as **Pss**: `first=57 warmup_max=61 post20=59 last=59
peak=76`.

## Instrument repair #20 — the baseline sat before a one-time allocation
The PASS above was real but its label was wrong: `growth_after_warmup=+5.8`
and `excursions … i49x141` on a curve `57 59 64 64 64 64 64 64 64 64` — a
one-time step at i49 then **flat for 160 iterations**. `median(rss[20:40])`
sat below that step. This is the mirror of repair #14.
**Fix**: baseline is the **settled region `median(rss[40:60])`** — past every
allocation observed, still early enough that a slow leak separates from it.
Added an explicit `one-time allocations (i20-40 -> i40-60)` line so the step
is reported as what it is instead of being folded into "growth".
**`leak_shape_selftest.py` now 11/11**, including the real i49 curve as a
fixture: flat, one-time step, and reloads of 2/5/25/60/100 samples all PASS;
linear +0.25, slow +0.08, and steps at i110/i170 all FAIL.

## Two implementation children on DISJOINT file sets (deliberate)
The skill's default is one fixer owning the tree. The owner asked for far more
parallelism, and the lock-screen material work touches exactly one file that
the panel/motion fixer never opens. Split rather than serialise:

| child | model | scope | forbidden |
|---|---|---|---|
| r1d `c91d640c` | logfare/glm-5.3 | main.rs, morph.rs, power/themes/launcher/wallpaper panels, tools/dictation | src/lockscreen.rs, scripts/ |
| lockscreen `8fb8a00d` | fasfas-ai/deepseek-v4.1-flash | **src/lockscreen.rs only** | every file the other owns |

Both briefs name the other's files explicitly. Parent verifies the union diff
after both report, and re-runs the full suite on the merged tree — a green
build in each child is not a green build together.

### Lockscreen spec handed over (brief §9, parent-verified)
- Veil is **vertical**, not radial: reference transmits ≈.78 at BOTH x100 and
  x2400 at y720. Fitted recipe reproduces all nine measured patch alphas with
  **mean abs error 0.0037, max 0.0105**; plateau .224, top .430, bottom .507.
  Our centre alpha .22 was already right — only the shape is wrong.
- Backdrop is **sharp** (best-fit global Gaussian radius 0.6 device px, a
  compression artefact). A 20-24 px global blur belongs to the OLD design.
- Clock is **wallpaper-sampled frosting inside the glyphs**, per-channel
  affine over a 24 px-blurred registered wallpaper, RGB RMSE **2.39/255** over
  114,924 interior pixels. The white base + cyan specular at
  `src/lockscreen.rs:1000-1009` must both go.
- Reference stems 76-77 px on a 279 px cap = **stroke/cap .272-.276**, much
  heavier than Quicksand-Bold. Author states SF Compact Display. Report the
  gap; do not claim a font ID from shape correlation (tried and retracted).
