# dynamite-rust

A minimal Wayland "dynamic island": a small black capsule at top-centre showing
the clock, which expands into panels (control centre, music, calendar, themes,
clipboard, wallpapers) and a giant-clock lock screen. Written in Rust against
`wlr-layer-shell`. No GTK, no Qt, no QML — one binary, direct Wayland.

Plus `tools/dictation/`: hold a key, speak, release; the transcription is typed
into the focused window and the island shows a live level bar.

> **Status:** the panel is usable daily on MangoWM. The lock screen is a
> **visual replica only** — see [Lock screen](#lock-screen) before relying on it.

## Prerequisites

**Build-time** (hard requirements — the build fails without them):

| Need | Why | Arch/Artix package |
|---|---|---|
| Rust **1.88+** (edition 2021) | both crates — locked deps need 1.88 | `rust` |
| C compiler and linker (`cc`) | native dependencies including dictation TLS | `base-devel` |
| `libxkbcommon` + headers | keymap handling | `libxkbcommon` |
| `libasound` + headers | dictation microphone capture | `alsa-lib` |
| `pkg-config` | locating the two above | `pkgconf` |

**Runtime:** a compositor implementing `wlr-layer-shell`. **Tested on MangoWM
0.18.3 only.** The workspace discs and the wallpaper picker are Mango-specific
(`mmsg`, `~/.config/mango/wallpaper/`), so on Hyprland or sway expect the pill
and panels to work and those two integrations to be empty. The
`ext_session_lock_v1` protocol is not used.

**Dictation additionally needs** evdev access to the keyboard and a writable
`/dev/uinput` — normally the `input` group:

```sh
sudo usermod -aG input "$USER"      # then log out and back in
```

Load `uinput` if `/dev/uinput` is missing (`sudo modprobe uinput`), then check
its permissions. Dictation sends speech to `api.groq.com` and requires a Groq
API key in `GROQ_API_KEY` or a private `static-dictation/api_key` file under your
config directory. See [`tools/dictation/README.md`](tools/dictation/README.md)
for setup and [`deploy/README.md`](deploy/README.md#dictation) for autostart.

**Optional external tools.** Every one is invoked defensively — a missing tool
degrades to an empty row rather than crashing, which also means **a missing tool
looks identical to "no data"**. Install what you want to see:

| Tool | Buys you | Without it |
|---|---|---|
| `bluetoothctl`, `rfkill` | Bluetooth row + toggle | blank Bluetooth row |
| `nmcli`, `iw` | Wi-Fi row, SSID, signal | blank Wi-Fi row |
| `wpctl`, `pactl` | volume slider + mute | volume shows nothing |
| `playerctl` | music panel (track, art, position) | empty music panel |
| `brightnessctl` | brightness slider | brightness shows nothing |
| `swaync-client` | notification count / DND | no notification badge |
| `cliphist`, `wl-copy` | clipboard history panel | empty clipboard |
| `mmsg` | MangoWM workspace + output state | workspaces don't track |
| `grim` | wallpaper frost sampling for the lock screen | flat lock background |
| `curl` | wallpaper download in the themes panel | download fails quietly |
| `swaybg` | display wallpaper selections | saved selection is not displayed |
| `socat` | manual socket command examples below | use the panel's CLI state argument |
| Python 3 | one-time browser profile theme setup | generated website CSS is not imported |
| `wtype` *(or `xdotool` on X11)* | dictation types into the focused window | transcription goes nowhere |

Check what you have:

```sh
for t in bluetoothctl rfkill nmcli iw wpctl pactl playerctl brightnessctl \
         swaync-client cliphist wl-copy mmsg grim curl wtype; do
  command -v "$t" >/dev/null || echo "missing: $t"
done
```

## Build

Two separate crates, deliberately **not** a Cargo workspace: the panel pins
`smithay-client-toolkit 0.19` and dictation pins `0.21.1`, so a shared lockfile
would force one of them onto a version it was not built against.

```sh
./build.sh          # builds both, fails loudly if either fails
```

The first build needs network access to fetch locked Rust dependencies. Only
part of the dependency tree is vendored; `--offline` works after Cargo's cache
is populated. This is not a fully vendored offline distribution.

Or by hand:

```sh
cargo build --release                    # -> target/release/dynamite-rust
cd tools/dictation && cargo build --release   # -> tools/dictation/target/release/static-dictation
```

Binaries land in those two paths; the launcher scripts resolve them relative to
the repo, so keep the checkout where you build it.

## Install

```sh
./build.sh
./deploy/install.sh          # --check to preview
```

Installs the key bindings (`SUPER+space` launcher, `SUPER+v` clipboard), the
layer rule, and an autostart wrapper. Unrelated `bind.conf` / `rule.conf` entries
and customized wrappers are preserved; obsolete Dynamite includes are repaired.
See [`deploy/README.md`](deploy/README.md).

## Run

```sh
./target/release/dynamite-rust
```

If the panel re-attaches after a compositor crash while the screen is still
blank, see `docs/restart-checklist.md` — bring the compositor up BEFORE
judging any visual gate.

It creates its control socket at `$XDG_RUNTIME_DIR/dynamite-rust.sock` and
anchors a layer surface to the top edge, centred horizontally by the compositor.

Drive it by sending a word to that socket:

```sh
printf 'controlcenter' | socat - UNIX-SENDTO:"$XDG_RUNTIME_DIR/dynamite-rust.sock"
```

Words include `collapsed`, `controlcenter`, `music`, `calendar`, `themes`,
`clipboard`, `wallpapers`, `lockscreen`, `unlock`, `dictation_start`,
`dictation_end`. See `docs/socket-contract.md` for the full contract — those
literals are an API shared with the dictation tool, not internal strings.

### The bar needs a wallpaper behind it

The capsule is a near-black shape. On a bare compositor root it is almost
invisible, and the frost material has nothing to sample. If the bar looks flat
or wrong, check that a wallpaper daemon is actually running:

```sh
pgrep -a swaybg || swaybg -i ~/.config/mango/wallpaper/wallpaper.png -m fill &
```

Without one you see MangoWM's `root_color` (`config.conf`) behind the bar, not
your wallpaper. The panel reads the wallpaper *file* for its frost, so the
capsule still renders — which makes this easy to miss.

## Configuration

`~/.config/dynamite-rust/config.conf`, `key=value`, `#` comments. A missing file
means defaults; unknown keys are ignored; every number is clamped, so a malformed
value cannot break startup.

| Key | Default | Range | Meaning |
|---|---|---|---|
| `collapsed_width` | 74 | 40–600 | capsule width, logical px |
| `bar_height` | 28 | 18–80 | capsule height, logical px |
| `expanded_height` | 150 | 60–800 | panel height |
| `gap_top` | 3 | 0–200 | gap above the capsule |
| `inner_padding` | 14 | 0–64 | panel inset |
| `anim_ms` | 160 | ≤1000 | legacy key; morph currently uses a fixed 233 ms duration |
| `hover_open` | true | bool | expand on pointer hover |
| `hover_state` | controlcenter | state name | which panel hover opens |
| `scroll_cycle` | true | bool | scroll wheel cycles panels |
| `clock_24h` | true | bool | 24-hour clock |
| `notch_mode` | 0 | 0/1 | notch-style geometry |
| `wallpaper` | — | filename | wallpaper for frost sampling |
| `theme` | — | name | selected palette for shell accents, GTK and websites |

## Themes

Click a row in **Themes**, or run `target/release/dynamite-rust --apply-theme NAME`.
The shell accent changes immediately. Restart open GTK 3/4 apps to pick up the
new styles. Apps that do not use GTK or that paint their own interface may not
follow the palette. Selecting a theme does not change the wallpaper.

To enable the same colours in your browser, find **Profile Directory** in
`about:support`, then run:

```sh
python3 deploy/waterfox-theme.py /path/to/waterfox/profile
```

Restart the browser after enabling it and after selecting a new palette. The
stylesheet applies to HTTP and HTTPS pages, not the browser toolbar or internal
pages. It changes common backgrounds, text, links and controls. Images and
videos keep their original colours, and some sites draw parts of their own UI
that the stylesheet cannot cover. There is no extension or background service.
To detach it, run the same command with `--remove` and restart the browser.

GTK imports are added to `gtk-3.0/gtk.css` and `gtk-4.0/gtk.css` under your config
directory. To remove the desktop overrides, delete the Dynamite `@import` line
from those files and restart the affected apps. Any pre-existing files changed by
setup are backed up alongside the originals with a `.before-dynamite` suffix.

## Lock screen

The panel's lock preview is **not a security boundary**. It has no keyboard
grab or authentication; click it to return to the capsule.

For a real lock, the optional Dynamite launcher gives Hyprlock the same
wallpaper and clock. Hyprlock owns the session lock and password field; the
dedicated PAM service tries your existing Howdy setup first, then the normal
password stack. See the [setup and attended test](deploy/README.md#dynamite-session-lock).
The panel installer does not enable the new locker or replace your shortcut.

## Development

```sh
cargo clippy --all-targets --all-features -- -D warnings
cargo test --release
python3 scripts/stress/stress.py        # >=1000 assertions, 5 groups
python3 scripts/measure.py              # geometry + sharpness vs the reference
python3 scripts/blurtest.py <png>       # contrast-budget check
./scripts/motion/run_all.sh <outdir> 3  # frame-by-frame smoothness
```

`scripts/measure.py` refuses to report geometry unless the panel is settled and
collapsed — a capture taken mid-morph or over a dark window measures the wrong
surface. Each instrument has a self-test; run it after touching a detector.

`COUNCIL_LOG.md` records the review loop that produced this state, including
findings that were **retracted** after measurement contradicted them.

## Licence

Fonts under `assets/fonts/` are SIL Open Font License 1.1 (see
`OFL-Quicksand.txt`); Inter is likewise OFL.