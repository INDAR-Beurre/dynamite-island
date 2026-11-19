# dynamite-rust — STATUS

All nine island states are implemented and reachable over the control socket:
Collapsed, ControlCenter, Music, Calendar, Themes, Clipboard, Wallpapers,
Dictation, Lockscreen — plus the owner's additions (Bluetooth submenu,
workspace discs) and a tray host. See `README.md` to build and run.

**The lock screen is a visual replica, not a security boundary** — no PAM, no
`ext-session-lock-v1`. Do not enable lock-at-boot. `README.md` explains why.

## Design choices
- Compositor client: smithay-client-toolkit (layer-shell + shm SlotPool + calloop WaylandSource)
- Text: fontdue. UI face Quicksand Bold (static, wght 700), vendored in
  `assets/fonts/` and embedded in the binary — no filesystem font is required.
  Clock face Inter baked at wght 550 / opsz 20.
- Raster: manual SDF capsule + glyph coverage into wl_shm Argb8888 — no tiny-skia, no GPU
- Redraws: frame-callback driven. Socket input sets a wish; the compositor's
  frame event does the paint. A 100 Hz microphone stream cannot overrun it.
- Control: unix datagram $XDG_RUNTIME_DIR/dynamite-rust.sock; second invocation = remote
- Namespace: dynamite-rust, layer overlay, anchor TOP (centred by the compositor)

## Measurement
`scripts/measure.py` (geometry, glyphs, RSS, CPU), `scripts/stress/stress.py`
(1207 tests / ~5900 assertions), `scripts/motion/run_all.sh` (screen recordings
split into stills, pose counts, frame pacing), `scripts/scorecard.py` (trend
ledger). Ground-truth reference frames live in `docs/reference/`.

## Log
- (build pending)
