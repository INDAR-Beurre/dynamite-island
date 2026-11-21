# Authenticated lock and keyboard integration

## Current behavior

- **Super+Ctrl+L**, the Power-menu **Lock** button, and the public `lock` /
  `lockscreen` CLI commands launch the same authenticated Hyprlock backend.
- **Super+Ctrl+Shift+L** retains the old swaylock password locker as a backup.
  The previous bindings are saved in
  `~/.config/mango/bind.conf.before-dynamite-default-lock`.
- `lock-preview` is explicitly a reversible renderer test. Legacy raw preview
  datagrams remain available for regression tests, but cannot unlock Hyprlock.
- The dedicated PAM service tries the existing Howdy setup, then the existing
  Linux login password. **No account password or separate PIN was configured
  or changed by this work.** Actual recognition and credentials need an attended
  test; no real camera or password attempt was made during development.

## Implementation

- Open menus request exclusive keyboard focus for arrow/Escape navigation;
  collapsed, dictation, and preview states request no keyboard focus.
- The Power action collapses the island and starts a worker that launches and
  reaps the maintained Python locker. It does not run an interpolated shell
  command or display a fake lock. The CLI replaces itself with that launcher.
- `--render-lock-background PATH WIDTH HEIGHT` exports the existing wallpaper,
  veil, and frosted clock offline. Dimensions are limited to 64..8192 per side
  and at most 16 megapixels.
- `deploy/dynamite-lock.py` launches Hyprlock 0.9.7 with native input and a
  dedicated PAM service. Private assets and the instance guard remain alive
  through launcher HUP/INT/TERM until authentication finishes. Failed minute
  refreshes retry at the next minute.
- `/etc/pam.d/dynamite-lock` is root-owned mode 644 and matches the tracked
  service: sufficient `pam_howdy.so`, then the existing `login` password fallback.
  Existing Howdy models/configuration, greeter, and swaylock PAM were preserved.

## Verification

- Release offline/locked Rust suite: **308 passed, 0 failed, 1 ignored**.
  The ignored wall-time benchmark is not a correctness claim. Release build
  succeeded with existing warnings; no warning-free lint claim is made.
- `python3 scripts/keyboard_selftest.py`: PASS in a private headless Mango:
  Left/Right cycling and wrapping; rendered Up/Down selection; Escape in all
  eight menus; collapsed state releases keyboard focus.
- `python3 scripts/lock_selftest.py`: PASS in a private user/mount namespace
  and compositor, using the actual public `lockscreen` and `lock` CLI routes
  and the production launcher checks. Only inside that namespace, the Howdy
  module is replaced with deny/permit fixtures and `login` with a test stack.
  These fixture mounts do not change the live PAM files or installed module.
  The installed-module repair and separate real-module test are recorded below.
  - `lock-preview` remains a reversible renderer test.
  - Hyprlock acknowledges a real session lock.
  - Escape, the preview's `unlock` datagram, and an incorrect fixture password
    cannot dismiss it; the correct fixture password unlocks it.
  - A successful face-module fixture short-circuits a denying password stack.
  - Launcher SIGTERM retains its assets/guard until authentication completes.
  - Existing prepare directories and oversized exports are rejected.
- `python3 deploy/dynamite-lock.py --check`: PASS on the live installation.
- The two shortcut lines were checked for uniqueness, parsed by Mango, reloaded,
  and read back. No key combination or live Lock action was triggered.
- Final visual deployment stopped only stale panel PID 9279 through its
  pidfd; supervisor 1629 started PID 11735. Its executable matches release inode
  `66307:24380724`, and exactly one island overlay is present after waking the
  output. Mango PID 1562 was retained; browser and dictation were not restarted.

Both earlier external security-review attempts failed before delivering
findings. There is **no independent security approval** for this integration;
the earlier limited council result concerns a different shell snapshot.

## Reference proportions and real-module verification — November 21, 2026

- The large clock coverage mask is resampled **5% wider**, without changing
  its height, vertical placement, proportional glyph advances, wallpaper
  material, native password capsule, or the separate small island clock.
  For the controlled `11:28` string at 1440p, visible ink is **863×277**
  (aspect **3.1155**) versus the reference **868×279** (**3.1111**).
  This corrects proportions; it does not claim an identical font. The draw
  path also avoids calculating and rasterizing an unused duplicate layout.
- Howdy Next's official latest-release endpoint reports **v3.4.2**, published
  October 31, 2026. Installed package metadata was already **3.4.2-2**, but its
  13,784-byte PAM module exported no `pam_sm_authenticate` or other PAM hooks.
  This was reproduced by both `ctypes` and the ELF dynamic-symbol table.
- Built the unmodified official v3.4.2 source with prefix `/usr`, sysconfdir
  `/etc`, libexecdir `lib`, and PAM directory `/usr/lib/security`. All **eight**
  focused upstream ABI/export/eligibility/auth-flow/runtime tests passed.
  Only `pam_howdy.so` was replaced, atomically, root-owned mode 755. Existing
  helper programs, enrolled models, Howdy configuration, PAM services, and
  Linux account password were preserved.
- Original module SHA-256:
  `02407c0869e34e5b0def2742409d12927d012b5b0791f2673de4de644fec90c5`.
  Root-owned backup:
  `/usr/lib/security/pam_howdy.so.before-dynamite-repair-20261121-004720`.
  Repaired module SHA-256:
  `725321044a6da370977f718b457a4630c327f9007f5a30ce89dd9640090fb797`.
  A later package reinstall may replace this repair; launcher preflight now
  rejects a module missing its authentication entry point or dependencies.
- `python3 scripts/howdy_module_selftest.py`: PASS for both the uninstalled
  candidate and repaired installed module. It creates a **new root-owned
  mount namespace**, with a private disabled Howdy config, private PAM/login
  fixtures, and both camera helper executables replaced by `false` only inside
  that namespace. It verifies all six PAM hooks, the real module's disabled
  result `PAM_AUTHINFO_UNAVAIL (9)`, wrong-password rejection, correct fixture
  password acceptance, and matching `PAM_USER` / `PAM_SERVICE`.
  This separate namespace is necessary because Howdy deliberately validates
  root ownership of config ancestors; the unprivileged Wayland test's UID
  mapping cannot satisfy that policy. The production policy was not weakened.
- Re-ran the private real-Hyprlock CLI tests and keyboard tests: PASS. No live
  session lock, camera scan, real password attempt, or compositor restart was
  performed. `deploy/dynamite-lock.py --check` passes after the module repair.

Keep the backup shortcut and perform the first actual face/password test while
at the machine. Actual camera recognition remains unverified; no lock-at-boot
or idle-lock behavior was enabled.
