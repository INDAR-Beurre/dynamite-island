# deploy/ — the launcher layer

These files start the panel and bind its hotkeys. They previously lived only in
`~/.config/mango/`, hardcoded `/home/alex` and `/run/user/1000`, so the package
shipped a binary but not the thing that launches it.

## Install

```sh
./build.sh
./deploy/install.sh          # or --check to see what it would do
```

`install.sh` **appends** to your existing `bind.conf` and `rule.conf` — it
never overwrites them. An earlier version of this page said
`cp deploy/rule.conf ~/.config/mango/rule.conf`, which would have replaced
134 lines of real window rules with a 3-line file. Use the script.

What it installs:

| file | contents |
|---|---|
| `~/.config/mango/dynamite-bind.conf` | `SUPER+space` → launcher, `SUPER+v` → clipboard |
| one line in `bind.conf` | sources the above |
| one line in `rule.conf` | the `layer_rule` below |
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

**The `layer_name` must be `dynamite-rust`** — that is the namespace the binary
sets (`NAMESPACE` in `src/main.rs`). An earlier `layer_name:dynamite` matched
nothing, so the no-blur/no-shadow optimisation silently did not apply.

## Dictation

`tools/dictation/` needs **evdev access to the keyboard and a writable
`/dev/uinput`** — normally the `input` group:

```sh
sudo usermod -aG input "$USER"     # then log out and back in
```

Without it push-to-talk silently does nothing. See `tools/dictation/README.md`.

## The lock screen is not a lock

It is a visual replica. It does not authenticate, does not grab the keyboard,
and does not use `ext-session-lock-v1`. **Do not bind it to a locking chord and
do not run it at boot.**