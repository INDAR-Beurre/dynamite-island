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

- `cargo test --release --offline`: **320 passed / 0 failed**
- `python3 scripts/stress/stress.py`: **tests=1207 asserts=6179 fails=0**,
  socket p50 0.6 / p95 0.7 / p99 14.4 / max 16.0 ms (bound 400)
- `python3 scripts/halo_check.py`: **holes 0, PASS** on all 8 menu states
- `python3 scripts/render_check.py`: **PASS 9/9**
- Pill: aspect 2.61–2.69 (ref 2.643), cap/pill 0.361 (ref 0.357),
  lapvar_norm ~1330–1415 (ref 1277), idle CPU 0.0–1.0 %, Pss ~30–40 MB
- Clock ink (187,213,254) vs reference (188,214,255)
- Stage surface is 1176x512 logical; lockscreen shm ring is released on
  unlock. Pss is flat across lock/unlock cycles.
- Motion detector now reproduces four hand-measured frames exactly.

## Known-open, do not spend your budget rediscovering

1. **Morph timing needs source-aware reading.** wf-recorder relative PTS
   shows 16–17 distinct poses at 16.7 ms hold and ~200–217 ms settle duration,
   no overshoot. Its absolute command latency is **UNRESOLVED** because the
   recorder PTS has no reliable wall-clock anchor. A grim run measures onset
   worst 56 ms but only ~30 fps, so its 41.5 ms pose hold and 248.7 ms settle
   duration are sampling-limited, not proof of 60 Hz dropped frames.
2. **Dictation opens the mic at login** and never closes it (battery, not
   privacy — idle audio is drained and discarded). Do not propose lazy
   capture without first measuring warm/cold open latency; `PRE_ROLL` cannot
   recover audio because the hotkey path clears the backlog. The startup
   timeout and failed-transcription WAV retention are already fixed.
3. **Lockscreen is a visual preview, not a security boundary.** Never add
   PAM, session-lock, or swaylock wiring; the owner cannot recover a lockout.

## Motion instrument traps

- Do not quote the old 341–349 ms onset: it came from a recorder offset that
  could place the trigger before the recording. The harness now marks wf
  absolute latency/settle unresolved instead of fabricating a failure.
- The connected-component detector is pinned by `scripts/motion/selftest.py`
  against four hand-measured frames. If those /tmp files are absent it skips
  the geometry case; do not claim that case ran.
- Measure on an empty tag. The owner's dark app window can merge with a panel.

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
