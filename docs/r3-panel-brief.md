# Round 3: verified build for independent council review

## Goal and safety

The owner wants a close, independently implemented visual replica of the local
creator videos: the centred dynamic island grows into clean menus; keep the
owner's workspaces, Bluetooth and dictation, improve smoothness and RAM, and
avoid invented decoration. The source is Rust on MangoWM 0.18.3, not Hyprland.
The original brief did **not** authorize a new visual identity: a failed child's
rounded-rectangle/left-aligned-clock redesign was removed. Review reference
fidelity, not that abandoned direction.

The lock screen is now explicitly a **visual preview**, not a security lock.
The owner cannot recover a lockout while away. Never invoke swaylock, add PAM or
session-lock, restart MangoWM, signal live dictation, replace wallpaper, change
configuration, or drive/capture the live desktop during this review.

Working directory: `/home/alex/rust-island-build`. Actual reference material is
in `docs/reference/`, extracted from the owner's videos/transcripts in
`/home/alex/Downloads/Reeldrop`. No Sleek reference was available or invented.
Read `docs/critic-guide.md` for quantisation, aspect-ratio, clock-string and codec
traps, but do not execute its live commands or start wallpaper.

## Build and tests

- `cargo build --release --offline`: successful, 64 warnings, not warning-clean.
- `cargo test --release --offline`: 332 passed, zero failures, one ignored
  wall-time benchmark. `/tmp/r3-merged-tests.log`.
- Opt-in idle-host `upscale_region_perf`: region/full ratio 0.247, bound still
  0.35, pass. `/tmp/r3-perf-check.log`. Do not run it under panel contention.
- The normal region test now checks every destination pixel, including unchanged
  sentinel pixels outside damage; an unbounded full-stage write must fail without
  relying on scheduler timing.
- Dictation crate: 103 passed, two ignored. Signal-handler flag test is green;
  actual signal delivery was NOT exercised against the live process. Its clippy
  `-D warnings` remains red on existing dead standalone-pill code/function casts.
- Supervisor `bash -n`, Python syntax, `git diff --check`, leak selftest 19/19 pass.

## Live verification of this merged binary

Panel PID 15556 and `target/release/dynamite-rust` matched executable inode
24380677 at capture. Exactly one overlay layer was present. Startup initially
returned normally because eDP-1 was asleep (zero dimensions, Wayland sent
`layer_surface.closed`); waking only the output resolved it. No compositor
restart or real lock occurred.

- `/tmp/r3-verified-stress.log`: 1207 cases, 6179 assertions, zero failures,
  five groups. About 4000 assertions are transport return checks, NOT distinct
  semantic product tests. Send-to-accepted p50 21.7 / p95 26.2 / max 76.3 ms;
  this is not presentation latency.
- Load/leak: collapsed baseline 39 MB Pss; collapsed tail +0.0 MB; peak +1.3 MB.
  The gate compares like states, not expanded memory against a collapsed tail.
- Harness mutation checks caught dropped calendar (17 failures), dropped lock
  preview (16), and injected 3000 ms latency (121). These mutate the harness,
  not all product paths; do not oversell them.
- `/tmp/r3-halo.log`: zero holes across eight menus.
- `/tmp/r3-render.log`: all nine states rendered. The render detector is not a
  complete content/interaction/height oracle; height containment has Rust tests.
- `/tmp/r3-measure.json`: pill 97x36, centre offset 0, width 4.30% of screen,
  cap/pill 0.361, reported RSS 60.5 MB and CPU 0.5%. Do not call RSS Pss.

## Fresh screenshots and motion (bounded read-only evidence)

Current 2256x1504 captures taken on an empty tag, with original tag restored:
`docs/reviews/r3/{pill,power,controlcenter,themes,lock-resting,lock-typing}.png`.
Compare only your assigned screenshots against the relevant reference files.
Use actual PNG pixels for quantitative checks; read tools may downsample.

`/tmp/r3-motion.json`, `/tmp/r3-motion.log` and extracted frames under
`/tmp/motion-runs/20261120-094721-controlcenter/` are current-build evidence.
Three clean-screened trials: poses [11,14,13], travel 633 px, worst relative
settle duration 277.3 ms, no reversals/overshoot. Reference has ~14 poses over
233 ms. The run is **NOT green**: interval pacing, dropped-frame ratio,
duplicate ratio and soft-edge checks fail; absolute command-to-screen latency
and command-relative settle remain unresolved because recorder PTS has no
trustworthy wall-clock anchor. Do not convert uncertainty to either a made-up
340 ms regression or a pass. Inspect a bounded frame set; do not start capture.

## Round-3 changes to inspect

- Reference-sized Power uptime/Lock caption: 8/9 logical px instead of 11/12.5;
  compact proportional-width test in `src/power_panel.rs`.
- Lock preview capsule 52x30 at 1080, dots 6.7, pitch still 12;
  height-normalised layout test in `src/lockscreen.rs`.
- No arbitrary redesign: centred proportional giant clock, no date at rest,
  original stadium pill retained.
- Dictation signal handler only stores `TERMINATE`; cleanup/audio-save happens
  on the ordinary loop thread. A synchronous transcription can delay handling;
  evaluate this honestly. No signal-context filesystem/env/socket work remains.
- Wallpaper restart discovers PID-only swaybg instances, reads exact procfs argv,
  opens pidfds, validates all instances before signaling only vetted PIDs.
  Foreign/unknown process or unsupported pidfds fails closed. Four argv tests;
  no live wallpaper restart was tested.
- Supervisor rotates only BETWEEN child runs. Earlier active-child inode-race
  allegation was retracted; one long-lived child may still exceed 1 MiB.
- Stress discovery uses exact layer/socket parsing, no process-name or VmRSS
  fallback. Recovery checklist is health-first, not an instruction to restart.

Prior medians: round 1 = 4.5; round 2 = 7. Round 3 is unscored.
Strict JSON final only:
`{"score":0,"summary":"...","findings":[{"severity":"blocker|major|minor","title":"...","detail":"...","where":"file:line or named screenshot area"}],"what_would_make_it_a_10":"..."}`.
9+ means you would ship under the assigned lens. Inspect the actual files,
consider a counterexample before reporting each finding, and distinguish observed
failures from unprofiled candidates and untested live behavior. Make no edits.
