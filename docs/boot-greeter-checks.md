# Boot greeter integration — 2026-11-21

The reboot screen is a separate greetd login UI. Previously,
`/etc/greetd/config.toml` launched `/usr/local/bin/face-greeter-session`,
which ran a custom GTK screen inside Sway. Changing the session's Hyprlock
appearance did not change that boot UI.

## Changes

- `deploy/face-greeter.py` now displays the **same Rust-generated background**
  as the session lock: wallpaper registration, vertical veil, frosted clock,
  and the wider clock mask. The native top capsule contains the lock icon or
  password input. There is no duplicate date, avatar, animated ring, or old
  login layout.
- greetd still authenticates **alex** through its existing PAM service and
  Howdy enrollment, then starts Mango only after successful authentication.
  Password fallback stays in that service. No PAM stack, password, enrollment,
  greeter account, autologin setting, or active desktop service was changed.
- `deploy/face-greeter-session` restricts **the greeter's Sway only** to the
  Intel PCI GPU, resolving its stable by-path symlink before assigning
  `WLR_DRM_DEVICES` (colons otherwise act as list separators). It also selects
  the Intel render node and GLES2; GTK uses GL rather than Vulkan.
- DisplayLink/EVDI stays installed and available to the user's desktop.
  The Intel-driven internal screen hosts boot login.
- The greeter has a private writable home/cache under
  `/var/cache/dynamite-greeter`, fixing the old `//.cache` permission errors.
  Its session and UI logs are under `.cache/` there.
- The boot UI and renderer are root-installed under `/usr/local`; greetd does
  not need to read or execute files from the private user home. Wallpaper and
  the clock-format preference are root-owned snapshots of the current setup.
  Re-run the installer after changing those assets or rebuilding the renderer.

## Cause of the driver warning

The laptop uses Intel Iris Xe (`i915`). At this boot Intel was `/dev/dri/card1`,
while DisplayLink's EVDI devices were `card0` and `card2`. The old Sway startup
probed those virtual devices, failed several EGL/Vulkan initializations, and
printed `!!! Proprietary DisplayLink drivers are in use !!!`.
The greeter account's home was `/`, also causing shader-cache errors.
This was not evidence of a proprietary Intel driver.

## Installation and rollback

From the checkout root:

```sh
sh deploy/install-greeter.sh --check /home/alex
sudo sh deploy/install-greeter.sh --install /home/alex
```

The installer requires this machine's existing greetd configuration, saves
replaced launchers/config under a timestamped `/var/backups/dynamite-greeter-*`
directory, and never restarts greetd or the compositor.

Installed backup for this change:
`/var/backups/dynamite-greeter-20261121-090035`.
It contains `face-greeter`, `face-greeter-run`, `face-greeter-session`,
`sway-greeter.conf`, and the unchanged `config.toml` for reference.
Restore the first three to `/usr/local/bin/` and `sway-greeter.conf` to
`/etc/greetd/` to revert. Do not restart greetd from an unattended session.

## Verification

`python3 scripts/greeter_selftest.py --screenshot /tmp/greeter-fixture.png`
creates a private headless Sway, fake greetd socket and synthetic wallpaper.
It never calls real PAM or opens a camera. It verifies:

- Shared renderer pixels match the displayed image exactly below the input.
- The hidden entry does not widen the resting 52:30 capsule.
- Fragmented greetd messages are read correctly.
- Face-success notification starts only the requested fixture session.
- Escape cancels an input attempt; a rejected password does not start a session.
- A subsequent accepted fixture password reaches `start_session`.
- A failed desktop start keeps the greeter visible with a retry message.
- Fixture password strings never appear in greeter logs.

The same test with `--installed` passed **as UID 973 (`greeter`)**, using the
root-installed UI and renderer, from a temporary readable copy of the test.
This also verified that the protected user-home source tree is unnecessary.
Installed files were byte-compared with their source/build inputs.

Separately, a private **headless** Sway was launched as the greeter account
with `WLR_RENDERER=gles2` and the Intel render node. Its log reported:

```text
GL vendor: Intel
GL renderer: Mesa Intel(R) Iris(R) Xe Graphics (TGL GT2)
Running compositor on wayland display 'wayland-1'
```

No EVDI, proprietary-driver or software-rendering warning appeared. This
checks hardware rendering without taking the live display/DRM master.

Python compilation, shell syntax and `git diff --check` passed. greetd PID
1277 and Mango PID 1578 stayed running. There was **no reboot, logout, live
camera scan, password attempt, session lock, or compositor restart** during
verification. The next actual boot/physical login remains untested.
