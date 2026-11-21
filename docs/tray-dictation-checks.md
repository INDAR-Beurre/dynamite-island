# Minimal tray and dictation

This supersedes the earlier decorated tray/recording design following the
request to make both minimal. Other menus, the dictation producer, and OS/browser
appearance are unchanged by this correction.

## Design

- Tray items are unboxed, neutral 12-logical-pixel icons. Alpha-bound fitting
  keeps differently padded app images consistent. Normalized luminance retains
  internal details rather than turning every icon into a solid silhouette.
  Premultiplied source-over preserves antialiased and partly transparent edges.
- There is no tray background, accent badge, or attention dot. NeedsAttention
  uses brighter neutral ink. Existing app activation, context-menu and scroll
  paths and 20-pixel click targets remain.
- Dictation is a **74×24** logical black capsule containing **seven** quiet,
  neutral level bars derived from the producer's 27 real bands. It has no
  microphone badge, label, elapsed timer, gradient, or synthetic idle movement.
- The settled capsule paints once at native resolution. Attack/release
  smoothing, original socket literals, and the 1.5-second lost-end lease remain.
  Workspaces and the tray return when recording ends.

## Verification

- Release locked/offline Rust suite: **306 passed, 0 failed, 1 ignored**.
  The ignored test remains the optional wall-time upscale benchmark.
- Release build succeeds with existing warnings; no warning-clean claim.
- Native icon tests check neutral ink, partial alpha and internal detail at
  1×/2×. Recording tests check real levels, reset, malformed samples, transparent
  corners, bounds and premultiplication at 1×/2×/3×.
- `scripts/tray_recording_selftest.py --click-tool /tmp/dynamite-music-vp` passes
  using its own D-Bus daemon, compositor, HOME/runtime and two StatusNotifier
  fixtures. Actual Wayland virtual-pointer input delivers SNI Activate. Synthetic
  samples change the small waveform; normal end and missing-end recovery pass.
  The surrounding recording pixels match the wallpaper captured before the
  panel starts, rather than the collapsed state that legitimately contains
  workspaces/tray. No hardware microphone is opened.
- The cropped examples are actual isolated renderer output at
  `/tmp/dynamite-tray-final.png` and `/tmp/dynamite-recording-final.png`.
- Only panel PID 23406 was replaced through its existing supervisor. Current
  PID 16373 matches the release inode, one overlay is present, and Mango PID
  1562 is unchanged. A monitor wake request restored the output after rollout;
  no compositor restart was performed.

A live 1,207-case/6,179-check regression returned one order-dependent compound
payload witness that bisection could not reproduce. Its state-machine, memory,
latency and liveness groups passed (collapsed Pss baseline 32 MB, retained growth
0.0 MB, transient peak delta 1.9 MB, send-to-accepted p95 0.7 ms). These include
about 4,000 transport checks, not 6,179 distinct semantic scenarios, and the
latency is not presentation latency. This live run is not described as green.

The follow-up in a private Mango session with its own runtime, HOME and D-Bus
**passed all five groups: 1,207 cases, 6,179 mixed checks, 0 failures**. The private
session used a 1280×720 output and a 3-pixel top gap, so its absolute memory and
geometry are not comparisons with the live 1.5-scale output. Its collapsed Pss
baseline was 25 MB, retained growth 0.0 MB, peak delta 1.2 MB; send-to-accepted
p95 was 0.7 ms. The earlier compound witness did not reproduce in isolation.
All fixture processes were reaped and temporary runtime files removed.

Logs for this correction: `/tmp/dynamite-minimal-tests.log`,
`/tmp/dynamite-minimal-private.log`, `/tmp/dynamite-minimal-stress.log`, and
`/tmp/dynamite-minimal-private-regression.log`. These are local transient logs;
the figures above are the preserved record, not a promise the paths persist.
