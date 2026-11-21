# Music redesign and island-only themes

Implementation follows the Music redesign request and the later instruction to
stop applying island palettes to applications and websites.

The 612 × 150 logical Music panel now gives track title, artist, artwork and
transport controls priority. It uses one aligned transport row, real playback
position/duration, an explicit idle state and the configured clock format.
Artwork and icon coordinates scale with the render canvas. Settled Music draws
its content once at native 2× resolution; morph endpoints retain their content.
Transport commands use direct arguments and the selected MPRIS player, run off
the UI thread with a timeout, and do nothing when idle. Artwork caching remains
bounded to the current track.

Theme selection now updates only island configuration and accents. The GTK and
website CSS generators were removed. The removal-only migration script deletes
exact managed imports and its marked browser preference, and neutralizes old
Dynamite-generated files. It preserves other CSS, file modes and browser data.
Already-open applications may retain loaded CSS until reopened; no application
was restarted to force this.

Validation on the release build:

- Rust suite: **327 passed, 0 failed, 1 ignored**. The ignored test is the existing
  optional wall-clock upscale benchmark. Obsolete Music geometry/dot tests and
  the removed GTK generator test were replaced or removed with those behaviors.
- Music regressions: scaled artwork/control coverage at 1× and 2×, circular hit
  targets, text truncation, retained metadata/parser/decoder cases.
- `python3 scripts/keyboard_selftest.py`: real key delivery in private headless
  Mango passed, including arrows, list selection, Escape and focus release.
- `python3 scripts/theme_selftest.py`: fake-profile cleanup, BOM/charset and owner
  rule preservation, file modes, symlink refusal, idempotence and two actual CLI
  palette changes writing only island configuration passed.
- A disposable private Mango fixture used a fake `playerctl` executable and
  known artwork. Its captured panel showed the artwork in its tile. Actual
  Wayland pointer clicks sent `previous`, `play-pause` and `next` to the fixture's
  selected player. Play/pause changed the rendered glyph; clicking while idle
  sent no command. No real player or live desktop input was used by this check.
- Release build and `git diff --check` passed. The build still reports prior
  warnings; this is not a strict-lint-clean claim.

Deployment replaced only the stale panel under its existing supervisor. The new
process's executable matches the release binary. Mango, the browser, GTK apps,
dictation and the real locker were not restarted. The existing managed GTK and
Waterfox imports were removed and checked absent. No new council score or live
authentication claim is made for this follow-up.
