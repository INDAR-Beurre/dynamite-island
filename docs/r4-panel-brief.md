# Round 4 review brief

The owner wants an independently implemented, nearly pixel-matched Rust shell
inspired by the supplied creator videos on MangoWM, keeping their workspace,
tray, Bluetooth and dictation additions. Their latest request is for selected
themes to actually affect desktop apps and websites. Do not remove those
additions to make the bare creator reference easier to match.

This round reviews the merged implementation, not the old round-3 head.
`docs/reviews/r4/evidence.json` records commands and measured limits. Read
`docs/critic-guide.md` for measurement traps, then your specific files. Where
that older guide conflicts with this brief, this brief is current.

## What changed

- Launcher embedded icons use device coordinates at native render scale.
- CC refresh no longer busy-loops through subprocesses while a panel is open.
  An isolated six-second sample fell from 52.0% to 2.83% of one core (self plus
  child time). This is a sample, not a distribution or a new external claim.
- Generic panel fill is opaque black; Power icon size, ink and label placement
  were adjusted against the video. Workspace/tray additions remain visible.
- Threaded region upscale hoists column weights without changing integer
  rounding. The normal test asserts exact output inside damage and unchanged
  sentinel pixels outside it. The optional wall-time benchmark stays ignored.
- Lock preview exits on a left click without a keyboard grab, PAM or real
  session lock. It drops the bounded lock material cache when leaving.
- Morph remains EaseOutQuad, 233 ms. Its zero-amplitude 150 ms repaint tail
  was removed. No gate threshold was relaxed to get a pass.
- `src/theme.rs` derives contrast-checked palette roles from existing swatches,
  writes GTK 3/4 CSS and website CSS, preserves existing GTK rules through an
  import/backup, and chooses readable foreground ink for theme accents.
  `src/library.rs::apply_theme` is shared by CLI and shell actions; selected
  name persists and the live shell updates. Wallpaper/toolbar themes are
  separate. No extra daemon or dependency was added.
- `deploy/waterfox-theme.py` sets up a managed `userContent.css` import and
  `user.js` preference. It is removable/idempotent, backs up originals,
  preserves BOM/@charset and respects explicit disable. It never touches
  prefs.js, browser sessions, toolbar themes, history, cookies or secrets.
- Website recoloring covers common HTTP(S) DOM backgrounds/text/controls.
  Images/video retain source colours; canvas, complex site rendering and
  internal pages are not promised. Existing GTK apps and the browser must
  restart to read the new styles. The owner will choose when to do that.
- Installer uses Mango's `source = path`, migrates its exact old invalid
  include, rejects unknown flags before writes, preserves customized wrappers
  and generated bindings, and has a fake-HOME parser/idempotence test.

## Artifacts and current limits

`docs/reviews/r4/` has cropped current collapsed, power, controlcenter,
launcher and themes screenshots. They were taken on verified empty tag 9;
previous tag 1 and collapsed state were restored. Crops are native device
pixels from a 2256x1504 output, offset x=328. `lock-preview.png` is downsampled
to 1128x752. Use source rendering for exact lock glyph metrics, not that image.
The current selected theme is Everforest, so an orange shell accent is
intentional. The default reference accent remains available with no theme.
Reference stills are in `docs/reference/`, not the overwritten `/tmp/ref/`.
Do not count theme-accent changes as fidelity failures when the owner selects
that palette. The fixed small clock ink and glyph material remain distinct.

Verified reports:

- 336 Rust tests pass / 1 explicitly ignored wall-time benchmark; build passes
  with existing warnings. Strict warning-clean status is not claimed.
- Stress 1207 cases / 6179 assertions / 0 failures across five groups; about
  4000 assertions are transport checks. Pss collapsed baseline 32 MB,
  settled growth 0.0 MB; acceptance p95 0.7 ms is not presentation latency.
- Mutation canary passes; missing calendar/lock transitions and fake 3000 ms
  acceptance latency are caught. This is bounded coverage, not exhaustive.
- Halo 8/8 and render 9/9 pass. These are liveness/alpha guards, not full
  visual equivalence proofs.
- Five wf-recorder trials show 15/17/16/16/15 poses, 16.7 ms hold, 216.8 ms
  relative settle, duplicate ratio .087 <= .10, no reversals/overshoot,
  0 dropped frames. All resolved gates pass. Absolute onset/settle relative
  to command time remain unresolved because recorder PTS has no wall clock.
- Browser fixture renders two exact backgrounds while preserving a red image.
  GTK3/4 CSS parsed without errors in separate processes.
- Installer selftest passes fake-HOME isolation and Mango config parser.

## Review protocol and boundaries

Read, run and inspect only. Make no source or documentation edits. Think
through counterexamples before asserting a finding. Open the relevant whole
functions, connected callers and tests; do not score the author's prose.
Each finding needs a precise `where`, mechanism and actionable fix. A 9+
means you would ship under the stated scope and limitations.

No live desktop/socket commands, screenshots, input injection, real locks,
Mango/browser restart, live dictation signals, microphone opening, personal
profile/config writes or private browsing data. No subagents. Read supplied
screenshots and use isolated temporary fixtures. Only the test-integrity
critic runs the full Rust suite; other critics may run focused tests, but
avoid concurrent builds/heavy benchmarking. Process lifetime findings must
not be tested on the owner's running processes.

Final reply must be only this JSON, no markdown fences or plan:
{"score":0,"summary":"what you checked and verdict","findings":[{"severity":"blocker|major|minor","title":"...","detail":"...","where":"file:line or named area"}],"what_would_make_it_a_10":"..."}
