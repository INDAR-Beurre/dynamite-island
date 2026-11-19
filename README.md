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
| Rust (edition 2021) | both crates | `rust` |
| `libxkbcommon` + headers | keymap handling | `libxkbcommon` |
| `libasound` + headers | dictation microphone capture | `alsa-lib` |
| `pkg-config` | locating the two above | `pkgconf` |

**Runtime:** a compositor implementing `wlr-layer-shell` (tested: **MangoWM
0.18.3**; Hyprland and sway should work). `ext-session-lock-v1` is *not* used yet.

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

Or by hand:

```sh
cargo build --release                    # -> target/release/dynamite-rust
cd tools/dictation && cargo build --release   # -> tools/dictation/target/release/static-dictation
```

Binaries land in those two paths; the launcher scripts resolve them relative to
the repo, so keep the checkout where you build it.

## Run

```sh
./target/release/dynamite-rust
```

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
| `anim_ms` | 160 | ≤1000 | morph duration |
| `hover_open` | true | bool | expand on pointer hover |
| `hover_state` | controlcenter | state name | which panel hover opens |
| `scroll_cycle` | true | bool | scroll wheel cycles workspaces |
| `clock_24h` | true | bool | 24-hour clock |
| `notch_mode` | 0 | 0/1 | notch-style geometry |
| `wallpaper` | — | filename | wallpaper for frost sampling |
| `theme` | — | name | palette |

## Lock screen

**The lock screen is a visual replica, not a security boundary.** It draws the
giant clock and lock pill, but there is **no `ext-session-lock-v1` surface, no
PAM, and no keyboard input** (`src/main.rs:9`, `src/lockscreen.rs:8`). It cannot
authenticate anyone and it does not block input to other windows.

- Do **not** bind it to a screen-lock hotkey expecting it to secure the session.
- Do **not** enable lock-at-boot: a lock client that cannot authenticate would
  leave you with an untypeable locked session.
- For a real lock, use your compositor's own locker or `swaylock`.

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