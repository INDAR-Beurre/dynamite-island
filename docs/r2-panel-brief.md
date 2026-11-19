# r2 critic brief — shared preamble

Read `docs/critic-guide.md` first. It lists the instruments, the ground-truth
frames, the gate table, and **six retractions** — findings earlier critics
got wrong. Do not re-report those.

## What to judge

The work is a Rust Wayland layer-shell panel (`dynamite-rust`) replicating a
creator's Hyprland "dynamic island" from his videos. Owner's brief, verbatim
intent: pixel-perfect replication, much smoother and more polished, keep his
own additions (Bluetooth panel, workspaces), replicate the lock screen, as
little RAM as possible, and **it must not look AI-generated**.

Screen 2256x1504, scale 1.5, 60 Hz, MangoWM. Reference frames are 2560x1440
video stills — **compare as fractions of screen width, never absolute pixels**.

## State at r2 open

- `cargo test --release --offline`: **317 passed / 0 failed**
- `python3 scripts/stress/stress.py`: **tests=1207 asserts=6179 fails=0**,
  latency p50 29.0 / p95 57.0 / p99 64.7 / max 76.5 ms (bound 400)
- `python3 scripts/halo_check.py`: **holes 0, PASS** on all 8 menu states
- Pill: aspect 2.694 (ref 2.643), cap/pill 0.361 (ref 0.357),
  lapvar_norm ~1395 (ref 1277), idle CPU 0.0–1.0 %, Pss ~30–40 MB
- Clock ink (187,213,254) vs reference (188,214,255)

## Known-open, do not spend your budget rediscovering

1. **Settled expanded panels are 1x-upscaled.** `redraw_panel_content_2x`
   (`src/main.rs:3021`) is gated behind `if false` at `src/main.rs:5973` and
   only handles Power. Power "Lock" lapvar 1609 vs reference 10222.
2. **Morph presents 3–5 poses**; reference presents 14 in 233 ms, one per
   displayed frame, easeOutQuad, +1.1 % settle overshoot.
3. **Dictation opens the mic at login** and never closes it (battery, not
   privacy — idle audio is drained and discarded). `capture::start()` also
   has **no timeout**: `init_rx.recv()` waits forever, so a wedged audio
   server hangs startup silently.

A fixer is working on 1 and 2 right now. Judge what is IN THE TREE, and say
plainly if a finding may already be in flight.

## Traps that produced false findings before

- **Digit identity swings ink metrics ~17 %.** Control the clock string.
- **Integer-pixel ratios lie across render sizes.** At cap 13 one pixel is
  7.7 %. Use subpixel coverage.
- **The wallpaper may not be running.** `root_color 0x141424ff` shows as a
  flat (20,20,36) band. Check `pgrep swaybg` before scoring colour.
- **Two panel instances invalidate everything.** `pgrep -x dynamite-rust`
  must be 1.
- **A dark app window merges into the pill bbox.** `measure.py` refuses
  these; if it says `not-collapsed`, do not quote geometry.
- **Load contamination.** Check `loadavg`; a `cargo build` in flight doubles
  latency. The scorecard marks such rows `*BUSY*` and excludes them.
- **The reference is a compressed video.** Its lapvar is band-limited; ours
  being higher is not a defect. Never propose blurring our output.

## Two different clock materials — do not unify them

- Island pill clock: fixed **#BCD6FF**, invariant across two scenes with
  opposite white points.
- Lock screen clock: **wallpaper-sampled frosting inside the glyphs**, not a
  flat fill and not a frosted card. A card would be both wrong and slop.

## Output contract

Reply with ONLY this JSON object, nothing else — no prose, no fences:

{"score":0,"summary":"...","findings":[{"severity":"blocker|major|minor","title":"...","detail":"...","where":"file:line or area"}],"what_would_make_it_a_10":"..."}

9+ means "I would ship this". Vague praise is a 6. Every finding needs a
`where`. Verify by running, not by reading the fixer's claims.
