# deploy/ — the launcher layer

These files start the panel and bind its hotkeys. They previously lived only in
`~/.config/mango/`, hardcoded `/home/alex` and `/run/user/1000`, so the package
shipped a binary but not the thing that launches it.

## Install

```sh
./build.sh
./deploy/install.sh          # or --check to see what it would do
```

`install.sh` adds Dynamite's include and layer rule while preserving unrelated
`bind.conf` and `rule.conf` entries. It repairs the old `source path` include
syntax to Mango's `source = path`. Existing binding files and customized
autostart wrappers are preserved. `--check` reports without writing; unknown
options fail before any file is created.

What it installs:

| file | contents |
|---|---|
| `~/.config/mango/dynamite-bind.conf` | `SUPER+space` → launcher, `SUPER+v` → clipboard |
| one line in `bind.conf` | sources the above |
| one line in `rule.conf` | the panel layer rule below |
| `~/.config/mango/scripts/dynamite_autostart.sh` | wrapper that sets `ISLAND_HOME` |

Then add `~/.config/mango/scripts/dynamite_autostart.sh` to your autostart and
reload Mango.

## If you would rather do it by hand

`bind.conf` — two spawn lines, using the absolute path to the built binary:

```
bind=SUPER,space,spawn,/path/to/rust-island-build/target/release/dynamite-rust launcher
bind=SUPER,v,spawn,/path/to/rust-island-build/target/release/dynamite-rust clipboard
```

`rule.conf` — append, do not replace:

```
layer_rule=no_blur:1,no_shadow:1,no_animation:1,layer_name:dynamite-rust
```

On setups that use SwayNC with layer blur enabled, add `no_blur:1` to the
existing `swaync-notification-window` rule (or add the example from
[`rule.conf`](rule.conf) if none exists). Reload Mango and dismiss any popup
that was already open; it can retain the old rule until its surface closes.

**The `layer_name` must be `dynamite-rust`** — that is the namespace the binary
sets (`NAMESPACE` in `src/main.rs`). An earlier `layer_name:dynamite` matched
nothing, so the no-blur/no-shadow optimisation silently did not apply.

## Dictation

`tools/dictation/` needs **evdev access to the keyboard and a writable
`/dev/uinput`** — normally the `input` group:

```sh
sudo usermod -aG input "$USER"     # then log out and back in
```

Without the required device permissions, the hotkey listener reports that it
cannot attach. See [`tools/dictation/README.md`](../tools/dictation/README.md)
for private API-key setup. After building both crates, add this command to your
session autostart separately from the panel:

```sh
/path/to/rust-island-build/tools/dictation/scripts/dictation_supervisor.sh
```

Speech is sent to `api.groq.com` for transcription and needs a Groq API key
(`GROQ_API_KEY` or `$XDG_CONFIG_HOME/static-dictation/api_key`, falling back to
`~/.config/static-dictation/api_key`). Keep the key file mode `600`. Load the
`uinput` kernel module if `/dev/uinput` is missing (`sudo modprobe uinput`);
then check device permissions. The installer never changes groups or loads
kernel modules for you.

## Dynamite session lock

The panel's `lockscreen` command and its Power menu open a **preview only**.
For an actual lock, use the separate launcher below. It uses Hyprlock's Wayland
session lock and native password input, with Dynamite's wallpaper and clock.
The clock refreshes once a minute; the launcher only runs while locked.

Requirements: a release build of the panel, MangoWM with `mmsg`, Python 3,
Hyprlock (tested with 0.9.7), and an already configured Howdy Next installation
(tested configuration for 3.4.2). From the checkout root, install the dedicated
service if it doesn't already exist, then check dependencies without locking:

```sh
sudo install -o root -g root -m 644 deploy/dynamite-lock.pam /etc/pam.d/dynamite-lock
python3 deploy/dynamite-lock.py --check
```

This service tries Howdy, then includes the normal `login` PAM stack for
password fallback. It does not change the greeter, swaylock, or account setup.
The service does not enable Howdy's synthetic-keypress workaround.

Run the first real test only while you're at the machine and know your login
password. Run `python3 deploy/dynamite-lock.py --lock` once and test face unlock.
Press Enter with the field empty if Howdy is waiting for the prompt. Lock
again with your face out of view, let Howdy time out, and verify your password
unlocks it. The headless regression test uses fixture authentication; it
cannot verify your actual camera or credentials.

Only after both work, replace the command in your existing MangoWM lock binding
with `python3 /absolute/path/to/rust-island-build/deploy/dynamite-lock.py --lock`.
Do not add a duplicate binding or enable lock-at-boot during testing. To revert,
restore your old binding. `deploy/install.sh` does not touch lock bindings or PAM.