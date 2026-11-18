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
