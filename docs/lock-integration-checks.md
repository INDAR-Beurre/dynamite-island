# Keyboard focus and optional face-authenticated lock

The menu focus fix is deployed. The new real-lock launcher is installed and
prepared, but the existing live lock binding remains on swaylock until the owner
can test the actual camera and password in person. The panel's lockscreen and
Power-menu Lock action remain reversible previews.

## Implementation

- Open menus request exclusive keyboard focus for arrow/Escape navigation;
  collapsed, dictation, and lock-preview states request no keyboard focus.
- `--render-lock-background PATH WIDTH HEIGHT` runs offline and exports the
  existing wallpaper/veil/frosted-clock renderer without a password capsule.
  Size is limited to 64..8192 per side and at most 16 megapixels.
- `deploy/dynamite-lock.py` renders per-output images and launches maintained
  Hyprlock 0.9.7 with native input and a dedicated PAM service. Files and the
  instance guard stay alive through launcher HUP/INT/TERM until authentication
  finishes. Failed minute refreshes are retried at the next minute, not each second.
- `/etc/pam.d/dynamite-lock` is root-owned mode 644 and matches the tracked
  service: sufficient `pam_howdy.so`, then the existing `login` password fallback.
  Existing Howdy configuration/models, greeter, swaylock service and lock binding
  were not changed. No live authentication or camera attempt was made.

## Verification

- Release offline/locked Rust suite: **336 passed, 0 failed, 1 ignored**.
  The existing ignored wall-time benchmark is not a correctness claim.
- `python3 scripts/keyboard_selftest.py`: PASS in a private headless Mango
  session: Left/Right cycling and wrapping; rendered Up/Down selection;
  Escape in all eight menus; collapsed state releases keyboard focus.
- `python3 scripts/lock_selftest.py`: PASS with a private user/mount namespace,
  private compositor and private PAM fixture. Real session-lock acknowledgement;
  wrong fixture password and Escape remain locked; correct fixture password
  unlocks; a successful face-module *fixture* unlocks without a password.
  Launcher SIGTERM retained its assets/lock until authentication completed.
  Preparing an existing directory is refused; oversized export is refused.
- `python3 deploy/dynamite-lock.py --check`: PASS on the live installation.
- `--prepare` rendered an RGBA 2256x1503 image for the reported live output
  dimensions without locking. Temporary files were removed afterwards.
- Only the stale panel process was stopped; its existing supervisor relaunched
  the release build. Running executable and target inode match. The output needed
  a wake request before the layer appeared; Mango was not restarted.
- Both external security-review attempts failed before delivering findings
  (provider throttle/rate limit). There is **no independent security approval**
  for this new integration. The earlier limited council pass concerns the older
  shell snapshot, not this authentication feature.

Actual Howdy recognition and the normal password stack still need an attended
hardware test. Follow `deploy/README.md#dynamite-session-lock` before changing the
shortcut. No lock-at-boot or idle-lock behavior was enabled.
