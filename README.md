<div align="center">

# 🏝️ Dynamite Island (`dynamite-rust`)

**A fast, lightweight Wayland Dynamic Island written in pure Rust.**  
One binary. Direct `wlr-layer-shell`. Zero GTK, Qt, or QML overhead.

[![Rust](https://img.shields.io/badge/Rust-1.88%2B-orange?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Wayland](https://img.shields.io/badge/Wayland-wlr--layer--shell-blue?style=for-the-badge&logo=wayland&logoColor=white)](https://wayland.freedesktop.org/)
[![Compositor](https://img.shields.io/badge/MangoWM-0.18.3-purple?style=for-the-badge)](https://github.com/DreamMaoMao/mangowc)
[![Memory](https://img.shields.io/badge/RAM-~30--45%20MB-brightgreen?style=for-the-badge)](#)
[![Fonts License](https://img.shields.io/badge/Fonts-SIL%20OFL%201.1-lightgrey?style=for-the-badge)](#licence)

</div>

---

**Dynamite Island** sits as a sleek, frosted-glass pill at the top-centre of your Wayland screen, displaying the live clock at rest and smoothly spring-morphing into rich interactive panels—**Control Center**, **Notifications**, **Music**, **Calendar**, **Displays**, **Clipboard**, **Wallpapers**, **Themes**, **Power**, and **Dictation**—all rendered directly into Wayland shared memory in ~30–45 MB of RAM.

> **Status:** Daily-usable on **MangoWM**. The `lock-preview` command is a visual preview only; real session locking integrates with Hyprlock + PAM/Howdy (see [Lock screen](#lock-screen)).

---

## ✨ Features

| Area | What it does |
|---|---|
| 🕒 **Collapsed Pill & Clusters** | Compact top-centre clock capsule. Hovering reveals the MangoWM workspace strip on the left and active system tray icons on the right. |
| 🎛️ **Control Center** | Live Wi-Fi, Bluetooth, Display Brightness, Sound Volume, latest notification preview, and quick Power access. |
| 🔔 **Notifications & OSD** | Mirrors `org.freedesktop.Notifications` over D-Bus (up to 32 recent messages, interactive inbox with scroll & dismiss). Volume and brightness keys expand the pill briefly instead of cluttering the corner. |
| 🎵 **Music (MPRIS)** | Track metadata, album art, playback progress, and play/pause/next/prev controls via `playerctl`. |
| 🖥️ **Displays** | Per-output scale presets and fine `0.05` stepping; applies immediately and persists to `~/.config/mango/monitor.conf`. |
| 📅 **Calendar, Clipboard, Wallpapers & Themes** | Built-in monthly calendar, `cliphist` clipboard browser, wallpaper switcher (`swaybg`), and island accent theme picker. |
| 🎙️ **Hold-to-Speak Dictation** | Companion crate in [`tools/dictation/`](tools/dictation/README.md): hold a key, speak, release—transcribes via Groq Whisper and types into the focused window while the pill animates a live audio level bar. |
| 🔒 **Session Lock** | Authenticated Hyprlock + Howdy/PAM session lock styled with Dynamite's wallpaper frost and giant clock. |

---

## 📦 Prerequisites

### Build-time (required)

| Dependency | Purpose | Arch / Artix Package |
|---|---|---|
| **Rust 1.88+** (2021 ed.) | Compiles both crates (locked deps require 1.88) | `rust` |
| **C toolchain (`cc`)** | Native deps including dictation TLS | `base-devel` |
| **`libxkbcommon`** | Keymap handling | `libxkbcommon` |
| **`libasound`** | Dictation microphone capture | `alsa-lib` |
| **`pkg-config`** | Locates native libraries above | `pkgconf` |

### Runtime

- **Compositor:** Any compositor implementing `wlr-layer-shell` (**tested on MangoWM 0.18.3**). Workspace discs and the wallpaper picker are Mango-specific (`mmsg`, `~/.config/mango/wallpaper/`); on Hyprland or Sway, the pill and panels work while those two integrations stay empty. `ext_session_lock_v1` is not used.
- **Dictation:** Needs evdev keyboard access and writable `/dev/uinput` (usually `input` group + `sudo modprobe uinput`), plus a Groq API key (`GROQ_API_KEY` or `static-dictation/api_key`). See [`tools/dictation/README.md`](tools/dictation/README.md) and [`deploy/README.md`](deploy/README.md#dictation).

### Optional External Tools

Every external tool is invoked defensively—missing tools degrade gracefully to an empty row rather than crashing:

| Tool | Enables | Without it |
|---|---|---|
| `bluetoothctl`, `rfkill` | Bluetooth row & toggle | Blank Bluetooth row |
| `nmcli`, `iw` | Wi-Fi row, SSID, signal | Blank Wi-Fi row |
| `wpctl`, `pactl` | Volume slider & mute | Volume shows nothing |
| `playerctl` | Music panel (track, art, position) | Empty music panel |
| `brightnessctl` | Brightness slider | Brightness shows nothing |
| `swaync-client` | Notification count / DND sync | No notification badge |
| `cliphist`, `wl-copy` | Clipboard history panel | Empty clipboard |
| `mmsg` | MangoWM workspace & output state | Workspaces don't track |
| `grim` | Wallpaper frost sampling for lock screen | Flat lock background |
| `curl` | Wallpaper download in Themes panel | Download fails quietly |
| `swaybg` | Applying wallpaper selections | Selection not displayed |
| `socat` | Sending manual socket commands | Use CLI args instead |
| `wtype` *(or `xdotool`)* | Dictation auto-typing | Transcription not typed |
| `python3` | Optional locker launcher & selftests | Dev/locker helpers unavailable |

Quick check:

```sh
for t in bluetoothctl rfkill nmcli iw wpctl pactl playerctl brightnessctl \
         swaync-client cliphist wl-copy mmsg grim curl wtype; do
  command -v "$t" >/dev/null || echo "missing: $t"
done
```

---

## 🛠️ Build & Install

The panel (`smithay-client-toolkit 0.19`) and dictation (`0.21.1`) are intentionally kept as **two separate crates** rather than a single Cargo workspace so their lockfiles stay independent.

```sh
./build.sh                   # builds both release binaries
./deploy/install.sh          # installs bindings, layer rules, and autostart (--check to preview)
```

Or build manually:

```sh
cargo build --release                         # -> target/release/dynamite-rust
cd tools/dictation && cargo build --release   # -> tools/dictation/target/release/static-dictation
```

> **Note:** The first build requires network access to fetch locked crates. Launcher scripts resolve binaries relative to the repo, so keep the checkout where you build it. See [`deploy/README.md`](deploy/README.md) for deployment details.

---

## 🚀 Usage & IPC

Run directly:

```sh
./target/release/dynamite-rust
```

It opens a `wlr-layer-shell` surface anchored to the top edge and listens on `$XDG_RUNTIME_DIR/dynamite-rust.sock` (UNIX datagram). Drive it from keybinds or scripts:

```sh
printf 'controlcenter' | socat - UNIX-SENDTO:"$XDG_RUNTIME_DIR/dynamite-rust.sock"
```

Supported state words include `collapsed`, `controlcenter`, `notifications`, `displays`, `music`, `calendar`, `themes`, `clipboard`, `wallpapers`, `lockscreen`, `unlock`, `dictation_start`, and `dictation_end` (see [`docs/socket-contract.md`](docs/socket-contract.md)).

### 🔔 Notifications & Volume/Brightness OSD

- The island mirrors `org.freedesktop.Notifications` on the session bus without claiming the daemon name.
- Incoming volume/brightness notifications using the `x-canonical-private-synchronous` hint expand the pill briefly without stealing keyboard focus or interrupting an open menu.
- SwayNC's Do Not Disturb state suppresses incoming banners.
- Run `python3 scripts/notification_selftest.py` to test the headless D-Bus/compositor notification flow.

### 🖼️ Wallpaper & Frost

The capsule uses a frosted-glass backdrop sampled from your active wallpaper. Make sure a wallpaper daemon (such as `swaybg`) is running behind the compositor:

```sh
pgrep -a swaybg || swaybg -i ~/.config/mango/wallpaper/wallpaper.png -m fill &
```

---

## ⚙️ Configuration

Config lives at `~/.config/dynamite-rust/config.conf` (`key=value`, `#` comments). Missing files use defaults, unknown keys are ignored, and all numeric values are clamped safely.

- **Workspace strip:** Hidden at rest; hover over the clock and move left to switch workspaces.
- **`hover_open`:** Controls whether hovering over the pill opens an expanded menu.

| Key | Default | Range | Description |
|---|---|---|---|
| `collapsed_width` | `74` | `40–600` | Collapsed capsule width (logical px) |
| `bar_height` | `28` | `18–80` | Collapsed capsule height (logical px) |
| `expanded_height` | `150` | `150–448` | Music panel height |
| `gap_top` | `3` | `0–200` | Top margin above the capsule |
| `inner_padding` | `14` | `0–64` | Panel inner padding |
| `anim_ms` | `160` | `≤1000` | Legacy key (transitions use a 560 ms spring) |
| `hover_open` | `true` | `bool` | Expand on pointer hover |
| `hover_state` | `controlcenter` | state name | Target panel opened on hover |
| `scroll_cycle` | `true` | `bool` | Scroll wheel cycles between panels |
| `clock_24h` | `true` | `bool` | 24-hour clock format |
| `notch_mode` | `0` | `0 / 1` | Notch-style top geometry |
| `wallpaper` | `—` | filename | Wallpaper file for frost sampling |
| `theme` | `—` | name | Accent palette for the island UI |

---

## 🎨 Themes

Select a palette in the **Themes** panel or apply from the CLI:

```sh
./target/release/dynamite-rust --apply-theme NAME
```

Themes only style the island's own accents—your GTK apps, websites, and browser chrome are untouched. To clean up legacy browser overrides from older versions:

```sh
python3 deploy/waterfox-theme.py /path/to/waterfox/profile --remove
```

---

## 🔒 Lock Screen

The Power menu's **Lock** button and `dynamite-rust lockscreen` (or `lock`) launch the authenticated Hyprlock session lock with Dynamite's wallpaper and clock. Its PAM configuration tries your existing Howdy setup first, then your Linux login password.

`dynamite-rust lock-preview` is a clickable visual preview for development only and provides no security. See [`deploy/README.md`](deploy/README.md#dynamite-session-lock) for setup.

---

## 🧪 Development & Verification

```sh
cargo clippy --all-targets --all-features -- -D warnings
cargo test --release
python3 scripts/stress/stress.py        # >=1000 assertions, 5 groups
python3 scripts/measure.py              # geometry + sharpness vs reference
python3 scripts/blurtest.py <png>       # contrast-budget check
./scripts/motion/run_all.sh <outdir> 3  # frame-by-frame smoothness
```

`scripts/measure.py` requires the pill to be settled and collapsed. See [`docs/restart-checklist.md`](docs/restart-checklist.md) and `COUNCIL_LOG.md` for measurement methodology and review history.

---

## 📄 Licence

Fonts under `assets/fonts/` are licensed under the **SIL Open Font License 1.1** (see `OFL-Quicksand.txt`; Inter is likewise OFL).
