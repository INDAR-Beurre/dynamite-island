//! Global hotkey listener: evdev (Super+D = hold to talk) with stdin/socket fallback.

use anyhow::Result;
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum HotkeyEvent {
    /// Hold Super+D: record while held, transcribe on Super release.
    HoldStart,
    HoldEnd,
    /// Control-socket toggle (no keyboard shortcut).
    Toggle,
}

/// key codes (values from linux/input-event-codes.h, NOT X keycodes: X = kernel + 8)
const KEY_D: u16 = 32;
const KEY_LEFTMETA: u16 = 125; // Super
const KEY_RIGHTMETA: u16 = 126; // Super

const EV_KEY: u16 = 0x01;
const EV_SYN: u16 = 0x00;
const DEBOUNCE: Duration = Duration::from_millis(175);
/// an idle gap this long means a held meta key missed its keyup — stop trusting it
const MODIFIER_IDLE_RESET: Duration = Duration::from_secs(2);
/// bounded reopen attempts for transient device-read errors before the watcher exits
const REOPEN_ATTEMPTS: u32 = 5;
const REOPEN_SLEEP: Duration = Duration::from_millis(500);
/// raw errno values meaning the device node is permanently gone (unplug / removed)
const ENODEV: i32 = 19;
const ENOENT: i32 = 2;

// uinput / evdev ioctl request codes (values from linux/uinput.h, linux/input.h)
const UI_SET_EVBIT: libc::c_ulong = 0x4004_5564;
const UI_SET_KEYBIT: libc::c_ulong = 0x4004_5565;
const UI_DEV_SETUP: libc::c_ulong = 0x405c_5503;
const UI_DEV_CREATE: libc::c_ulong = 0x5501;
const UI_DEV_DESTROY: libc::c_ulong = 0x5502;
const EVIOCGNAME_128: libc::c_ulong = 0x8080_4506;
const EVIOCGBIT_KEY_64: libc::c_ulong = 0x8040_4521;
const EVIOCGRAB: libc::c_ulong = 0x4004_4590;

/// device names we must never watch back: our own virtual keyboard and the test injector
const SELF_DEVICE_PREFIXES: [&str; 2] = ["static-dictation-kbd", "burst-inject"];

#[repr(C)]
#[allow(dead_code)] // layout documentation; events are parsed from bytes below
struct InputEvent {
    tv_sec: i64,
    tv_usec: i64,
    type_: u16,
    code: u16,
    value: i32,
}
const EVENT_SIZE: usize = 24; // two i64s + type/code/value
                              // shortcut: assumed fixed 24-byte native-endian input_event; newer time64 kernels use the same sizes
                              // type_, code, value live at offsets 16, 18, 20 regardless of the time encoding.

/// A minimal virtual keyboard on /dev/uinput, used to re-emit events we grabbed away.
/// All ioctls are fire-and-forget best-effort: if any setup step fails we mark the device
/// invalid and Drop skips UI_DEV_DESTROY (kernel cleans up an un-created fd on close anyway).
struct UinputKbd {
    file: std::fs::File,
    ok: bool,
}

#[repr(C)]
struct UinputSetup {
    id: [u16; 4], // bustype, vendor, product, version
    name: [u8; 80],
    ff_effects_max: u32,
}

impl UinputKbd {
    fn new() -> Option<Self> {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open("/dev/uinput")
            .ok()?;
        let mut dev = UinputKbd { file, ok: true };
        // enable every key code 1..=248 so re-emitted events are always accepted
        for code in 1..=248u16 {
            dev.ioctl(UI_SET_KEYBIT, code as libc::c_ulong);
        }
        dev.ioctl(UI_SET_EVBIT, EV_KEY as libc::c_ulong);
        dev.ioctl(UI_SET_EVBIT, EV_SYN as libc::c_ulong);
        let name_bytes = b"static-dictation-kbd\0";
        let mut name = [0u8; 80];
        name[..name_bytes.len()].copy_from_slice(name_bytes);
        let setup = UinputSetup {
            id: [0x03, 0x1235, 0x5679, 1],
            name,
            ff_effects_max: 0,
        };
        dev.ok &= unsafe {
            libc::ioctl(
                dev.file.as_raw_fd(),
                UI_DEV_SETUP,
                &setup as *const UinputSetup,
            )
        } == 0;
        dev.ok &= unsafe { libc::ioctl(dev.file.as_raw_fd(), UI_DEV_CREATE) } == 0;
        if !dev.ok {
            return None;
        }
        Some(dev)
    }

    fn ioctl(&mut self, req: libc::c_ulong, arg: libc::c_ulong) {
        self.ok &= unsafe { libc::ioctl(self.file.as_raw_fd(), req, arg) } == 0;
    }

    /// Emit one raw event struct (as 24 bytes) followed by a SYN separator.
    fn emit(&mut self, event: &[u8; EVENT_SIZE]) {
        let mut syn = [0u8; EVENT_SIZE];
        syn[16..18].copy_from_slice(&EV_SYN.to_ne_bytes());
        if self.file.write_all(event).is_err() || self.file.write_all(&syn).is_err() {
            eprintln!("hk: uinput write failed");
        }
    }
}

impl Drop for UinputKbd {
    fn drop(&mut self) {
        if self.ok {
            unsafe { libc::ioctl(self.file.as_raw_fd(), UI_DEV_DESTROY) };
        }
    }
}

/// What the watcher puts back on the virtual keyboard for one event.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Emit {
    Forward,
    Hide,
    /// Send the buffered Super press first, then this event, so other
    /// Super+key shortcuts still reach apps. Carries the physical Super code.
    FlushSuper(u16),
    /// A chord key hid itself after the Super press was flushed: emit only the
    /// synthetic Super press, not the current event.
    FlushSuperOnly(u16),
}

/// Keyboard state shared by ALL device watchers: a grab-and-re-emit chain
/// (keyd → clones → compositor) can deliver Super and D on different /dev/input
/// nodes, so per-device state would split the chord. Shared also makes the
/// cross-node debounce collapse duplicated physical presses.
struct KeyState {
    /// Physical Super code held, and whether its press was handed to apps.
    /// The chord discards the buffered press; any other key flushes it.
    super_held: bool,
    super_code: Option<u16>,
    super_pending: bool,
    super_emitted: bool,
    super_chord_fired: bool,
    /// Super+D started a push-to-talk hold; the tracked Super release ends it.
    super_hold_active: bool,
    d_swallowed: bool,
    /// Emission decision for the event just passed to decide().
    emit: Emit,
    last_hold: Option<Instant>,
    last_event: Instant, // any tracked stream activity, for the stale-hold safety
}

impl KeyState {
    fn new() -> Self {
        KeyState {
            super_held: false,
            super_code: None,
            super_pending: false,
            super_emitted: false,
            super_chord_fired: false,
            super_hold_active: false,
            d_swallowed: false,
            emit: Emit::Forward,
            last_hold: None,
            last_event: Instant::now(),
        }
    }
}

/// Build a synthetic EV_KEY event. Used to flush a modifier press that was held
/// back while we waited to see whether the next key completed a dictation chord.
fn synthetic_key(code: u16, value: i32) -> [u8; EVENT_SIZE] {
    let mut b = [0u8; EVENT_SIZE];
    b[16..18].copy_from_slice(&EV_KEY.to_ne_bytes());
    b[18..20].copy_from_slice(&code.to_ne_bytes());
    b[20..24].copy_from_slice(&value.to_ne_bytes());
    b
}

/// Tracked keys, for the startup/keypress log line that makes recognition visible.
fn tracked(code: u16) -> bool {
    matches!(code, KEY_D | KEY_LEFTMETA | KEY_RIGHTMETA)
}

fn key_name(code: u16) -> &'static str {
    match code {
        KEY_D => "D",
        KEY_LEFTMETA => "LEFTMETA",
        KEY_RIGHTMETA => "RIGHTMETA",
        _ => "?",
    }
}

/// True for read errors that mean the device node is permanently gone.
fn device_gone(e: &std::io::Error) -> bool {
    matches!(e.raw_os_error(), Some(ENODEV) | Some(ENOENT))
}

/// True if this device is our own virtual keyboard (or the e2e test injector).
fn is_self_device(fd: std::os::fd::RawFd) -> bool {
    let mut name = [0u8; 128];
    let n = unsafe { libc::ioctl(fd, EVIOCGNAME_128, name.as_mut_ptr()) };
    if n <= 0 {
        return false;
    }
    let name = String::from_utf8_lossy(&name[..(n as usize).min(128)]);
    SELF_DEVICE_PREFIXES.iter().any(|p| name.starts_with(p))
}

/// True for a keyboard that can emit D. KEY_A separates keyboards from
/// mice/touchpads, whose motion events we cannot relay.
fn has_d_key(fd: std::os::fd::RawFd) -> bool {
    let mut bits = [0u8; 64];
    let n = unsafe { libc::ioctl(fd, EVIOCGBIT_KEY_64, bits.as_mut_ptr()) };
    if n <= 0 {
        return false;
    }
    let has = |code: u16| bits[(code / 8) as usize] & (1 << (code % 8)) != 0;
    has(KEY_D) && has(30) // 30 = KEY_A
}

impl KeyState {
    fn past(&self, last: &Option<Instant>) -> bool {
        match last {
            Some(t) => t.elapsed() >= DEBOUNCE,
            None => true,
        }
    }

    /// Hide the current key without dropping a modifier press flushed earlier
    /// in decide(); otherwise a Super release would arrive with no matching press.
    fn hide_event(&mut self) {
        self.emit = match self.emit {
            Emit::FlushSuper(super_code) => Emit::FlushSuperOnly(super_code),
            _ => Emit::Hide,
        };
    }
}

/// Map one EV_KEY event to an action; updates Super-hold tracking and debounce.
fn decide(state: &mut KeyState, code: u16, value: u8) -> Option<HotkeyEvent> {
    state.emit = Emit::Forward;
    // Safety: a missed Super keyup (split streams, or a remapper eating the
    // release) would leave the hold stuck ON forever.
    if !state.d_swallowed
        && !state.super_hold_active
        && (state.super_held
            || state.super_pending
            || state.super_emitted
            || state.super_chord_fired)
        && state.last_event.elapsed() >= MODIFIER_IDLE_RESET
    {
        state.super_held = false;
        state.super_code = None;
        state.super_pending = false;
        state.super_emitted = false;
        state.super_chord_fired = false;
        state.super_hold_active = false;
    }
    state.last_event = Instant::now();

    // Hold the Super press back until the next key tells us whether this is
    // Super+D (dictation) or an ordinary Super shortcut that apps should see.
    if value == 1 && !matches!(code, KEY_LEFTMETA | KEY_RIGHTMETA) && state.super_pending {
        state.super_pending = false;
        if code == KEY_D {
            // Discard the buffered press: no synthetic release will be needed.
            state.emit = Emit::Hide;
        } else if let Some(super_code) = state.super_code {
            state.super_emitted = true;
            state.super_chord_fired = true;
            state.emit = Emit::FlushSuper(super_code);
        }
    }

    match value {
        0 => {
            if code == KEY_D && state.d_swallowed {
                state.emit = Emit::Hide;
                state.d_swallowed = false;
            }
            if matches!(code, KEY_LEFTMETA | KEY_RIGHTMETA) {
                let ended = state.super_code == Some(code) && state.super_hold_active;
                if state.super_code == Some(code) {
                    // Only the tracked Super key closes the hold; a second Super
                    // pressed alongside it has no synthetic press to release.
                    state.super_held = false;
                    state.super_code = None;
                    state.super_chord_fired = false;
                    state.super_hold_active = false;
                    state.emit = if state.super_emitted {
                        Emit::Forward
                    } else {
                        Emit::Hide
                    };
                    state.super_emitted = false;
                    state.super_pending = false;
                } else {
                    state.emit = Emit::Hide;
                }
                return if ended {
                    Some(HotkeyEvent::HoldEnd)
                } else {
                    None
                };
            }
            None
        }
        1 => match code {
            KEY_D
                if state.super_held && !state.super_chord_fired && state.past(&state.last_hold) =>
            {
                state.super_chord_fired = true;
                state.super_hold_active = true;
                state.super_pending = false;
                state.super_emitted = false;
                state.last_hold = Some(Instant::now());
                state.d_swallowed = true;
                state.hide_event();
                Some(HotkeyEvent::HoldStart)
            }
            KEY_D if state.super_held || state.d_swallowed => {
                state.d_swallowed = true;
                state.hide_event();
                None
            }
            KEY_LEFTMETA | KEY_RIGHTMETA => {
                if !state.super_held {
                    state.super_held = true;
                    state.super_code = Some(code);
                    state.super_pending = true;
                    state.super_emitted = false;
                    state.super_chord_fired = false;
                }
                state.emit = Emit::Hide;
                None
            }
            _ => None,
        },
        _ => {
            // 2 = repeat: keep the chord keys hidden, and don't leak a repeat
            // of a Super whose press we buffered but never handed to apps.
            if (code == KEY_D && state.d_swallowed)
                || (matches!(code, KEY_LEFTMETA | KEY_RIGHTMETA)
                    && (state.super_code != Some(code) || !state.super_emitted))
            {
                state.emit = Emit::Hide;
            }
            None
        }
    }
}

/// Debounce for the stdin path, mirroring the evdev window: a burst inside
/// DEBOUNCE collapses to one action.
fn stdin_allowed(st: &mut KeyState) -> bool {
    let ok = match st.last_hold {
        Some(t) => t.elapsed() >= DEBOUNCE,
        None => true,
    };
    if ok {
        st.last_hold = Some(Instant::now());
    }
    ok
}

fn watch_file(
    path: &str,
    tx: &crossbeam_channel::Sender<HotkeyEvent>,
    state: &std::sync::Arc<std::sync::Mutex<KeyState>>,
    live: &std::sync::Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
    uinput: &std::sync::Arc<std::sync::Mutex<Option<UinputKbd>>>,
) -> Option<std::thread::JoinHandle<()>> {
    let mut file = match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
    {
        Ok(f) => f,
        Err(_) => File::open(path).ok()?, // read-only watcher on a write-protected node
    };
    if is_self_device(file.as_raw_fd()) {
        return None; // never watch our own virtual keyboard (or the test injector)
    }
    // Grab every keyboard that can emit D, so neither half of the chord ever
    // reaches the focused window; EBUSY (another grabber) falls back to read-only.
    let mut grabbed = false;
    if uinput.lock().unwrap().is_some() && has_d_key(file.as_raw_fd()) {
        let arg: libc::c_int = 1;
        grabbed = unsafe { libc::ioctl(file.as_raw_fd(), EVIOCGRAB, arg) } == 0;
    }
    if grabbed {
        eprintln!("hk: {path} grabbed (Super+D will not type into windows)");
    }
    // shortcut: a watcher that failed to grab is only a listener — re-emitting
    // its events would duplicate keys the compositor already received from that
    // device. Upgrade if a host grabs a keyboard without relaying it onward.
    // the registry is authoritative: never two watchers on one device path
    if !live.lock().unwrap().insert(path.to_string()) {
        return None; // a live watcher already owns this path
    }
    let tx = tx.clone();
    let state = state.clone();
    let live = live.clone();
    let uinput = uinput.clone();
    let path = path.to_string(); // owned so it can move into the 'static watcher thread
    let short = path.rsplit('/').next().unwrap_or(&path).to_string();
    Some(std::thread::spawn(move || {
        let mut buf = [0u8; EVENT_SIZE];
        'watch: loop {
            match file.read_exact(&mut buf) {
                Ok(()) => {
                    // shortcut: kernel input_event is native-endian; no unsafe
                    let u16at = |o: usize| u16::from_ne_bytes([buf[o], buf[o + 1]]);
                    let (type_, code, value) = (u16at(16), u16at(18), u16at(20));
                    if type_ == EV_KEY {
                        // value: 0=release 1=press 2=autorepeat; decide() ignores repeat
                        if tracked(code) && value != 2 {
                            eprintln!(
                                "hk: {short} {} {}",
                                key_name(code),
                                if value == 1 { "press" } else { "release" }
                            );
                        }
                        // never hold the state lock across eprintln/send: a closed
                        // stderr panics eprintln and would poison the shared mutex
                        // Super+D: Super is buffered, then discarded on D — no
                        // synthetic release is needed, and no 'd' reaches apps.
                        let (hit, emit) = {
                            let mut st = state.lock().unwrap();
                            let hit = decide(&mut st, code, value as u8);
                            (hit, st.emit)
                        };
                        if grabbed {
                            let mut guard = uinput.lock().unwrap();
                            if let Some(dev) = guard.as_mut() {
                                match emit {
                                    Emit::Hide => {}
                                    Emit::FlushSuper(super_code)
                                    | Emit::FlushSuperOnly(super_code) => {
                                        dev.emit(&synthetic_key(super_code, 1));
                                        if matches!(emit, Emit::FlushSuper(_)) {
                                            dev.emit(&buf);
                                        }
                                    }
                                    Emit::Forward => dev.emit(&buf),
                                }
                            }
                        }
                        if let Some(h) = hit {
                            eprintln!("hk: {short} -> {h:?}");
                            if tx.send(h).is_err() {
                                break 'watch; // receiver gone
                            }
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // reopened a device with no data yet; keep polling
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(e) => {
                    if device_gone(&e) {
                        // unplugged: exit cleanly; the rescan re-arms if it returns
                        break 'watch;
                    }
                    // transient error: bounded reopen attempts, then give up
                    let mut reopened = false;
                    for _ in 0..REOPEN_ATTEMPTS {
                        std::thread::sleep(REOPEN_SLEEP);
                        match File::open(&path) {
                            Ok(f) => {
                                file = f;
                                reopened = true;
                                break;
                            }
                            Err(re) if device_gone(&re) => break,
                            Err(_) => continue,
                        }
                    }
                    if !reopened {
                        break 'watch;
                    }
                }
            }
        }
        // deregister so the rescan may re-arm this path (single live watcher invariant)
        live.lock().unwrap().remove(&path);
        eprintln!("hk: {short} watcher exited (device gone or channel closed)");
    }))
}

fn stdin_fallback(tx: &crossbeam_channel::Sender<HotkeyEvent>) -> Result<()> {
    eprintln!("hk: no readable /dev/input/event* — stdin fallback: \"hold\"/\"start\" & \"release\"/\"stop\" = hold, \"quit\" = exit");
    let mut line = String::new();
    let mut st = KeyState::new(); // same per-kind debounce windows as the evdev path
    loop {
        line.clear();
        let n = std::io::stdin().read_line(&mut line)?;
        if n == 0 {
            return Ok(()); // EOF
        }
        let t = line.trim_end_matches(['\n', '\r']).trim();
        let ev = match t {
            "hold" | "start" => Some(HotkeyEvent::HoldStart),
            "release" | "stop" => Some(HotkeyEvent::HoldEnd),
            "quit" => return Ok(()),
            _ => None,
        };
        if let Some(ev) = ev {
            let allowed = match ev {
                HotkeyEvent::HoldStart => stdin_allowed(&mut st),
                HotkeyEvent::HoldEnd => true,
                HotkeyEvent::Toggle => true,
            };
            if allowed {
                tx.send(ev)?;
            }
        }
    }
}

/// Blocks the calling thread forever, sending events. evdev if readable, else stdin fallback.
/// Rescans /dev/input periodically so hot-plugged keyboards (and uinput test devices)
/// that appear after startup are picked up. A device path is watched by at most one
/// live thread (the `live` registry); watchers exit on unplug and are re-armed by the
/// rescan if the node returns.
pub fn listen(tx: crossbeam_channel::Sender<HotkeyEvent>) -> Result<()> {
    // e2e escape hatch on hosts where evdev grabs/uinput are blocked
    if std::env::var_os("STATIC_DICTATION_STDIN_HOTKEYS").is_some() {
        return stdin_fallback(&tx);
    }
    let state = std::sync::Arc::new(std::sync::Mutex::new(KeyState::new()));
    let live: std::sync::Arc<std::sync::Mutex<std::collections::HashSet<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));
    // ONE shared virtual keyboard for re-emitting grabbed events; None on hosts
    // without /dev/uinput access, in which case grabbing is disabled everywhere
    let uinput: std::sync::Arc<std::sync::Mutex<Option<UinputKbd>>> =
        std::sync::Arc::new(std::sync::Mutex::new(UinputKbd::new()));
    let mut armed = 0usize;
    let mut first_scan = true;
    loop {
        let mut found = 0usize;
        let mut new_watchers = 0usize;
        if let Ok(entries) = std::fs::read_dir("/dev/input") {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if !name.starts_with("event") {
                    continue;
                }
                found += 1;
                let path = format!("/dev/input/{}", name);
                if live.lock().unwrap().contains(&path) {
                    continue; // live watcher already owns this path
                }
                if watch_file(&path, &tx, &state, &live, &uinput).is_some() {
                    armed += 1;
                    new_watchers += 1;
                    eprintln!("hk: watching {path}");
                }
            }
        }
        if new_watchers > 0 {
            eprintln!("hk: {armed} input nodes armed (Super+D = hold to talk)");
        }
        if first_scan {
            if found == 0 {
                return stdin_fallback(&tx); // nothing readable at all
            }
            // r2 #7d: event nodes existed but none could be opened (lost
            // input-group/ACL, no /dev/uinput). `found` counts directories,
            // not watchers, so this looked healthy with no hotkey at all.
            if armed == 0 {
                eprintln!(
                    "hk: {found} input nodes present but 0 could be opened —                      no hotkey. Check the `input` group and /dev/uinput."
                );
            }
            first_scan = false;
        }
        // shortcut: 2s poll instead of inotify; fine for a tool that idles at 10ms anyway
        std::thread::sleep(Duration::from_secs(2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> KeyState {
        KeyState::new()
    }

    #[test]
    fn only_super_d_is_bound() {
        let mut s = state();
        // Keys from the removed AltGr+L / Alt+Space shortcuts are now inert.
        for code in [38u16, 56, 57, 100, 97, 82, 110] {
            assert_eq!(decide(&mut s, code, 1), None, "key {code} must be inert");
            assert_eq!(s.emit, Emit::Forward);
            assert_eq!(decide(&mut s, code, 0), None, "key {code} must be inert");
            assert_eq!(s.emit, Emit::Forward);
        }
        assert_eq!(decide(&mut s, KEY_D, 1), None);
        assert_eq!(s.emit, Emit::Forward);
        assert_eq!(decide(&mut s, KEY_D, 0), None);
        assert_eq!(s.emit, Emit::Forward);
    }

    #[test]
    fn super_d_holds_until_super_release_with_no_stray_keys() {
        for super_code in [KEY_LEFTMETA, KEY_RIGHTMETA] {
            let mut s = state();
            assert_eq!(decide(&mut s, super_code, 1), None);
            assert_eq!(s.emit, Emit::Hide); // buffered, not typed into apps
            assert!(s.super_pending);
            assert_eq!(decide(&mut s, KEY_D, 1), Some(HotkeyEvent::HoldStart));
            assert_eq!(s.emit, Emit::Hide);
            assert!(s.super_hold_active);
            assert!(!s.super_pending); // buffered Super discarded by the chord
            assert!(!s.super_emitted); // so no synthetic release is needed
            for v in [2, 0, 1, 0] {
                assert_eq!(decide(&mut s, KEY_D, v), None);
                assert_eq!(s.emit, Emit::Hide); // no 'd', no second hold start
            }
            assert_eq!(decide(&mut s, super_code, 0), Some(HotkeyEvent::HoldEnd));
            assert_eq!(s.emit, Emit::Hide);
            assert!(!s.super_hold_active);
            assert!(!s.super_held);
            assert_eq!(decide(&mut s, KEY_D, 1), None);
            assert_eq!(s.emit, Emit::Forward); // plain D still types normally
            assert_eq!(decide(&mut s, KEY_D, 0), None);
            assert_eq!(s.emit, Emit::Forward);
        }
    }

    #[test]
    fn super_d_debounce_collapses_two_starts_in_the_window() {
        let mut s = state();
        assert_eq!(decide(&mut s, KEY_LEFTMETA, 1), None);
        assert_eq!(decide(&mut s, KEY_D, 1), Some(HotkeyEvent::HoldStart));
        assert_eq!(decide(&mut s, KEY_D, 0), None);
        assert_eq!(decide(&mut s, KEY_LEFTMETA, 0), Some(HotkeyEvent::HoldEnd));
        assert_eq!(decide(&mut s, KEY_LEFTMETA, 1), None);
        assert_eq!(decide(&mut s, KEY_D, 1), None); // inside 175 ms window
        assert_eq!(s.emit, Emit::Hide);
        assert_eq!(decide(&mut s, KEY_D, 0), None);
        assert_eq!(decide(&mut s, KEY_LEFTMETA, 0), None); // no start ⇒ no end

        s.last_hold = Some(Instant::now() - DEBOUNCE - Duration::from_millis(5));
        assert_eq!(decide(&mut s, KEY_LEFTMETA, 1), None);
        assert_eq!(decide(&mut s, KEY_D, 1), Some(HotkeyEvent::HoldStart));
        assert_eq!(decide(&mut s, KEY_LEFTMETA, 0), Some(HotkeyEvent::HoldEnd));
    }

    #[test]
    fn two_realistic_chords_one_second_apart_both_start() {
        let mut s = state();
        for _ in 0..2 {
            assert_eq!(decide(&mut s, KEY_LEFTMETA, 1), None);
            assert_eq!(decide(&mut s, KEY_D, 1), Some(HotkeyEvent::HoldStart));
            assert_eq!(decide(&mut s, KEY_D, 0), None);
            assert_eq!(decide(&mut s, KEY_LEFTMETA, 0), Some(HotkeyEvent::HoldEnd));
            std::thread::sleep(Duration::from_millis(750));
        }
    }

    #[test]
    fn other_super_shortcuts_still_reach_apps() {
        const KEY_A: u16 = 30;
        let mut s = state();
        assert_eq!(decide(&mut s, KEY_RIGHTMETA, 1), None);
        assert_eq!(s.emit, Emit::Hide);
        assert_eq!(decide(&mut s, KEY_A, 1), None);
        assert_eq!(s.emit, Emit::FlushSuper(KEY_RIGHTMETA));
        assert!(s.super_emitted);
        assert_eq!(decide(&mut s, KEY_A, 0), None);
        assert_eq!(s.emit, Emit::Forward);
        assert_eq!(decide(&mut s, KEY_RIGHTMETA, 0), None);
        assert_eq!(s.emit, Emit::Forward); // balanced release for flushed press
        assert_eq!(decide(&mut s, KEY_A, 1), None);
        assert_eq!(s.emit, Emit::Forward); // plain A still works
    }

    #[test]
    fn second_super_press_cannot_duplicate_the_synthetic_press() {
        let mut s = state();
        assert_eq!(decide(&mut s, KEY_LEFTMETA, 1), None);
        assert_eq!(decide(&mut s, KEY_RIGHTMETA, 1), None);
        assert_eq!(s.emit, Emit::Hide);
        assert_eq!(decide(&mut s, 30, 1), None); // Super+A
        assert_eq!(s.emit, Emit::FlushSuper(KEY_LEFTMETA));
        assert_eq!(decide(&mut s, KEY_RIGHTMETA, 0), None);
        assert_eq!(s.emit, Emit::Hide); // untracked Super release hidden
        assert_eq!(decide(&mut s, KEY_LEFTMETA, 0), None);
        assert_eq!(s.emit, Emit::Forward); // only the tracked press is released
    }

    #[test]
    fn long_hold_past_idle_reset_still_ends() {
        let mut s = state();
        assert_eq!(decide(&mut s, KEY_LEFTMETA, 1), None);
        assert_eq!(decide(&mut s, KEY_D, 1), Some(HotkeyEvent::HoldStart));
        s.last_event = Instant::now() - Duration::from_secs(10);
        assert_eq!(decide(&mut s, KEY_LEFTMETA, 0), Some(HotkeyEvent::HoldEnd));
        assert_eq!(s.emit, Emit::Hide);
    }

    #[test]
    fn stale_super_cleared_after_idle_gap() {
        let mut s = state();
        assert_eq!(decide(&mut s, KEY_LEFTMETA, 1), None);
        s.last_event = Instant::now() - Duration::from_secs(10);
        assert_eq!(decide(&mut s, KEY_D, 1), None); // stale hold must NOT fire
        assert_eq!(s.emit, Emit::Forward);
        assert!(!s.super_held);
    }

    #[test]
    fn stdin_debounce_collapses_bursts() {
        let mut s = state();
        assert!(stdin_allowed(&mut s));
        assert!(!stdin_allowed(&mut s)); // sub-window repeat swallowed
        std::thread::sleep(DEBOUNCE + Duration::from_millis(30));
        assert!(stdin_allowed(&mut s)); // a distinct line after the window passes
    }

    #[test]
    fn ten_thousand_random_interleaved_events_maintain_strict_uinput_and_hold_invariants() {
        use std::collections::HashMap;
        const KEY_TAB: u16 = 15;
        const KEY_A: u16 = 30;
        let keys = [KEY_LEFTMETA, KEY_RIGHTMETA, KEY_D, KEY_A, KEY_TAB];
        let mut s = state();
        let mut phys_down: HashMap<u16, bool> = keys.iter().map(|&k| (k, false)).collect();
        let mut uinput_down: HashMap<u16, bool> = keys.iter().map(|&k| (k, false)).collect();
        let mut ptt_open = false;
        let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut next_u64 = || -> u64 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        fn apply_uinput(uinput_down: &mut HashMap<u16, bool>, code: u16, val: u8, step: usize) {
            let was = *uinput_down.get(&code).unwrap_or(&false);
            match val {
                1 => {
                    assert!(!was, "step {step}: duplicate keydown on uinput for {code}");
                    uinput_down.insert(code, true);
                }
                2 => {
                    assert!(was, "step {step}: orphan repeat on uinput for {code}");
                }
                0 => {
                    assert!(was, "step {step}: orphan keyup on uinput for {code}");
                    uinput_down.insert(code, false);
                }
                _ => panic!("unexpected value {val}"),
            }
        }

        for step in 0..10_000usize {
            let r = next_u64();
            let draining = (step % 97) > 90;
            let (code, val) = if draining {
                let held = keys.iter().copied().find(|k| phys_down[k]);
                match held {
                    Some(k) => (k, 0u8),
                    None => {
                        assert!(
                            !ptt_open,
                            "step {step}: HoldStart left open after all keys released"
                        );
                        for &k in &keys {
                            assert!(
                                !uinput_down[&k],
                                "step {step}: uinput key {k} stuck down when idle"
                            );
                        }
                        if step % 194 == 0 {
                            s.last_event = Instant::now() - Duration::from_secs(3);
                        }
                        if let Some(lt) = s.last_hold {
                            s.last_hold = Some(lt - (DEBOUNCE + Duration::from_millis(5)));
                        }
                        continue;
                    }
                }
            } else {
                let k = keys[(r as usize) % keys.len()];
                let is_down = phys_down[&k];
                let v = if !is_down {
                    1u8
                } else if ((r >> 8) % 5) == 0 {
                    2u8
                } else {
                    0u8
                };
                (k, v)
            };

            if val == 1 {
                phys_down.insert(code, true);
            } else if val == 0 {
                phys_down.insert(code, false);
            }
            if let Some(lt) = s.last_hold {
                if ((r >> 16) % 3) == 0 {
                    s.last_hold = Some(lt - (DEBOUNCE + Duration::from_millis(5)));
                }
            }

            let ev = decide(&mut s, code, val);
            match ev {
                Some(HotkeyEvent::HoldStart) => {
                    assert!(!ptt_open, "step {step}: duplicate HoldStart");
                    ptt_open = true;
                }
                Some(HotkeyEvent::HoldEnd) => {
                    assert!(ptt_open, "step {step}: HoldEnd without active HoldStart");
                    ptt_open = false;
                }
                Some(HotkeyEvent::Toggle) | None => {}
            }

            match s.emit {
                Emit::Hide => {}
                Emit::Forward => apply_uinput(&mut uinput_down, code, val, step),
                Emit::FlushSuper(super_code) | Emit::FlushSuperOnly(super_code) => {
                    apply_uinput(&mut uinput_down, super_code, 1, step);
                    if matches!(s.emit, Emit::FlushSuper(_)) {
                        apply_uinput(&mut uinput_down, code, val, step);
                    }
                }
            }
        }

        for &k in &keys {
            if phys_down[&k] {
                let ev = decide(&mut s, k, 0);
                match ev {
                    Some(HotkeyEvent::HoldEnd) => {
                        assert!(ptt_open);
                        ptt_open = false;
                    }
                    Some(HotkeyEvent::HoldStart) => panic!("release cannot HoldStart"),
                    Some(HotkeyEvent::Toggle) | None => {}
                }
                if s.emit == Emit::Forward {
                    apply_uinput(&mut uinput_down, k, 0, 10_000);
                }
            }
        }
        assert!(!ptt_open, "HoldStart left open at end of fuzzer");
        for &k in &keys {
            assert!(!uinput_down[&k], "uinput key {k} stuck down at end");
        }
    }
}
