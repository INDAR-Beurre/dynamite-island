# Displays integration checks

Verified on 2026-11-21 in `/home/alex/rust-island-build`.

The desktop pill now includes **Displays** between Control Center and Calendar.
Open it with `target/release/dynamite-rust displays`, or move right from
Control Center. Up/Down adjusts scale by 0.05; Page Up/Down selects an output;
Enter applies; `r` refreshes; Left/Right and Escape use the existing menu navigation.
The pointer controls offer presets, fine adjustment, output selection and Apply.

Apply tests the output change, applies it through `wlr-randr`, and saves only
that output's scale in `$XDG_CONFIG_HOME/mango/monitor.conf`. Existing modes,
positions, other output rules and comments are preserved. A failed save rolls
back the live scale and reports the failure in the panel.

## Verification

- `cargo test --release --offline`: 313 passed, 0 failed, 1 ignored.
- `cargo build --release --offline`: passed.
- `python3 scripts/keyboard_selftest.py`: passed on a private headless Mango.
  Covers real Wayland keyboard delivery across all nine menus, live scale
  changes and persistence, restoration after a failed save, launcher selection
  and keyboard release on collapse. Fractional scale assertions allow Wayland
  fixed-point rounding.
- `python3 scripts/render_check.py displays`: passed on the live laptop at 1.5×;
  measured 659 physical pixels wide for a 438 logical pixel panel. Inspected
  `/tmp/render_displays.png` and `/tmp/dynamite-displays-headless.png`.
- Greeter Sway config validated as the `greeter` user with its own headless
  runtime and a readable copy of the config.
- Python syntax checks and `git diff --check`: passed.

## Installed state

The existing panel supervisor started the rebuilt release. Exactly one
`dynamite-rust` process and overlay layer remained after deployment. The
compositor and greetd were not restarted.

The laptop session's included `~/.config/mango/monitor.conf` already has an
`eDP-1` rule at 1.5× and remains byte-for-byte unchanged. The installed
`/etc/greetd/sway-greeter.conf` now also contains `output eDP-1 scale 1.5`,
matching `deploy/sway-greeter.conf` for future installs. The live output remains
2256×1504 at 1.5×. Removing the obsolete Mango window rules triggered a config
reload; the pre-migration live position of 0,0 was restored without changing
the saved layout.

Removed `~/.local/bin/scaler`,
`~/.local/share/applications/display-scaler.desktop`, and the two
`app_id:display-scaler` rules in `~/.config/mango/rule.conf`. Refreshed the desktop
application database. Backups of those files, the previous panel executable,
and both monitor and greeter configs are in:

`/home/alex/.local/state/dynamite/display-migration-20261121-095853`

No reboot, real login, camera/PAM authentication, or physical external-display
hotplug was performed. Startup scaling is configured and syntax-validated;
its appearance on the next physical boot remains untested.
