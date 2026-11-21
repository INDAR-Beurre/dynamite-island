<div align="center">

# 🏝️ Dynamite Island

**A lightweight Wayland Dynamic Island written in pure Rust.**  
Single binary · Direct `wlr-layer-shell` · Zero GTK / Qt / QML · ~30–45 MB RAM

[![Rust](https://img.shields.io/badge/Rust-1.88%2B-orange?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Wayland](https://img.shields.io/badge/Wayland-wlr--layer--shell-blue?style=for-the-badge&logo=wayland&logoColor=white)](https://wayland.freedesktop.org/)
[![MangoWM](https://img.shields.io/badge/MangoWM-0.18.3-purple?style=for-the-badge)](https://github.com/DreamMaoMao/mangowc)
[![RAM](https://img.shields.io/badge/RAM-~30--45%20MB-brightgreen?style=for-the-badge)](#)
[![Fonts](https://img.shields.io/badge/Fonts-SIL%20OFL%201.1-lightgrey?style=for-the-badge)](#licence)

</div>

---

## ⚡ One-Line Install

On **Arch / Artix** (with MangoWM):

```sh
curl -fsSL https://raw.githubusercontent.com/INDAR-Beurre/dynamite-island/main/install.sh | bash
```

> **What it does:** Checks build tools, clones into `~/.local/share/dynamite-island`, compiles both release binaries (`dynamite-rust` + `static-dictation`), and runs `./deploy/install.sh` to wire up MangoWM keybinds, layer rules, and autostart.  
> Need the build deps first?
> ```sh
> sudo pacman -S --needed git rust base-devel libxkbcommon alsa-lib pkgconf
> ```

---

## ✨ What You Get

- **🕒 Smart Top Pill** — Shows the live clock at rest; hover left to switch MangoWM workspaces or right for active tray icons.
- **🎛️ Control Center** — Wi-Fi, Bluetooth, Brightness, Volume, latest notification preview, and Power actions.
- **🔔 Notifications & Volume/Brightness OSD** — Mirrors `org.freedesktop.Notifications` over D-Bus without replacing your daemon. Volume and brightness keys expand the pill cleanly.
- **🎵 Music, 📅 Calendar, 🖥️ Displays, 📋 Clipboard, 🖼️ Wallpapers & 🎨 Themes** — Built-in panels with smooth 560 ms spring morphs and frosted-glass blur.
- **🎙️ Hold-to-Speak Dictation** — [`tools/dictation/`](tools/dictation/README.md) records while a key is held, transcribes via Groq Whisper, types into the focused window, and animates a live level bar in the pill.
- **🔒 Session Lock** — Integrates with Hyprlock + Howdy/PAM (`dynamite-rust lockscreen`). `dynamite-rust lock-preview` is a visual preview only.

---

## 📦 Optional Runtime Tools

Missing tools degrade gracefully to an empty row instead of crashing:

| Feature | Required Tools |
|---|---|
| Wi-Fi & Bluetooth | `nmcli`, `iw`, `bluetoothctl`, `rfkill` |
| Volume & Brightness | `wpctl` or `pactl`, `brightnessctl` |
| Music & Notifications | `playerctl`, `swaync-client` |
| Clipboard & Wallpapers | `cliphist`, `wl-copy`, `swaybg`, `grim`, `curl` |
| MangoWM Workspaces & Displays | `mmsg` |
| Dictation Typing | `wtype` *(or `xdotool`)* + `input` group (`uinput`) |

---

## 🛠️ Manual Build & Run

```sh
git clone https://github.com/INDAR-Beurre/dynamite-island.git
cd dynamite-island
./build.sh
./deploy/install.sh          # optional: installs keybinds, layer rules & autostart
./target/release/dynamite-rust
```

Drive any panel via `$XDG_RUNTIME_DIR/dynamite-rust.sock`:

```sh
printf 'controlcenter' | socat - UNIX-SENDTO:"$XDG_RUNTIME_DIR/dynamite-rust.sock"
```

Supported states: `collapsed`, `controlcenter`, `notifications`, `displays`, `music`, `calendar`, `themes`, `clipboard`, `wallpapers`, `lockscreen`, `unlock`, `dictation_start`, `dictation_end` ([full contract](docs/socket-contract.md)).

---

## ⚙️ Configuration

Edit `~/.config/dynamite-rust/config.conf` (`key=value`, `#` comments). Defaults are used if missing:

| Key | Default | Range | Purpose |
|---|---|---|---|
| `collapsed_width` | `74` | `40–600` | Collapsed pill width (logical px) |
| `bar_height` | `28` | `18–80` | Collapsed pill height (logical px) |
| `gap_top` | `3` | `0–200` | Gap above the pill |
| `hover_open` | `true` | `bool` | Expand on hover |
| `hover_state` | `controlcenter` | state | Panel opened on hover |
| `scroll_cycle` | `true` | `bool` | Scroll wheel cycles panels |
| `clock_24h` | `true` | `bool` | 24-hour clock |
| `notch_mode` | `0` | `0 / 1` | Notch-style top geometry |
| `theme` | `—` | name | Island accent palette (`--apply-theme NAME`) |
| `wallpaper` | `—` | filename | Wallpaper file for frost sampling |

> **Tip:** Make sure a wallpaper daemon like `swaybg` is running so the frosted glass has a background to sample.

---

## 🧪 Development

```sh
cargo clippy --all-targets --all-features -- -D warnings
cargo test --release
python3 scripts/stress/stress.py
python3 scripts/notification_selftest.py
```

---

## 📄 Licence

Fonts under `assets/fonts/` are licensed under **SIL Open Font License 1.1** (`OFL-Quicksand.txt`; Inter is likewise OFL).
