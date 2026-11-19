//! Control socket: one-word commands over a Unix socket, so anything can drive
//! the dictation without the keyboard chord (like the island's control socket).
//!
//! Socket path: $XDG_RUNTIME_DIR/static-dictation.sock (fallback /tmp).
//! Commands (newline-terminated, one per connection):
//!   toggle     — start/stop continuous dictation (no keyboard shortcut)
//!   hold_start — same as pressing Super+D
//!   hold_end   — same as releasing Super
//!   status     — replies "recording=true" or "recording=false"
//!   quit       — clean shutdown (flushes stdout/stderr)
//! Anything else replies "unknown".

use std::io::{BufRead, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::hk::HotkeyEvent;
use crossbeam_channel::Sender;

const SOCKET_IO_TIMEOUT: Duration = Duration::from_millis(500);

pub fn sock_path() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("static-dictation.sock")
}

/// Pure command parse — unit-testable without a socket.
pub fn parse_cmd(line: &str) -> Option<&'static str> {
    match line.trim() {
        "toggle" => Some("toggle"),
        "hold_start" => Some("hold_start"),
        "hold_end" => Some("hold_end"),
        "status" => Some("status"),
        "quit" => Some("quit"),
        _ => None,
    }
}

/// Spawn the listener. `recording` is shared with the main loop; `tx` carries
/// events into it. Failure to bind logs and returns — dictation still works
/// via keyboard hotkeys; the socket is a convenience, not a dependency.
pub fn spawn(tx: Sender<HotkeyEvent>, recording: Arc<AtomicBool>) {
    let path = sock_path();
    // r2 #7d/#8c: this used to unlink unconditionally, so a second launch
    // stole the control channel from the live instance (which kept an
    // unreachable listener) and then hung opening the mic. Probe first:
    // if something answers, a live owner exists — refuse, do not unlink.
    if UnixStream::connect(&path).is_ok() {
        eprintln!(
            "ctrl: {} already has a live owner; this instance will not take it",
            path.display()
        );
        return;
    }
    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            // Nobody answered the probe, so this is a stale file from a
            // crashed run. Safe to replace.
            let _ = std::fs::remove_file(&path);
            match UnixListener::bind(&path) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("ctrl: socket bind failed: {e}");
                    return;
                }
            }
        }
        Err(e) => {
            eprintln!("ctrl: socket bind failed: {e}");
            return;
        }
    };
    thread::spawn(move || {
        for stream in listener.incoming() {
            handle_one(stream, &tx, &recording);
        }
    });
}

fn handle_one(
    stream: std::io::Result<UnixStream>,
    tx: &Sender<HotkeyEvent>,
    recording: &Arc<AtomicBool>,
) {
    let Ok(mut s) = stream else { return };
    // Never let a stalled/half-open peer block subsequent control commands.
    let _ = s.set_read_timeout(Some(SOCKET_IO_TIMEOUT));
    let _ = s.set_write_timeout(Some(SOCKET_IO_TIMEOUT));
    let Some(Ok(line)) = s
        .try_clone()
        .ok()
        .and_then(|r| std::io::BufReader::new(r).lines().next())
    else {
        return;
    };
    match parse_cmd(&line) {
        Some(c @ ("toggle" | "hold_start" | "hold_end" | "status" | "quit")) => {
            let _ = writeln!(s, "ok");
            match c {
                "toggle" => {
                    let _ = tx.send(HotkeyEvent::Toggle);
                }
                "hold_start" => {
                    let _ = tx.send(HotkeyEvent::HoldStart);
                }
                "hold_end" => {
                    let _ = tx.send(HotkeyEvent::HoldEnd);
                }
                "status" => {
                    let r = recording.load(Ordering::SeqCst);
                    let _ = writeln!(s, "recording={r}");
                }
                _ => {
                    let _ = std::io::stdout().flush();
                    let _ = std::io::stderr().flush();
                    // Leave the island collapsed: `quit` during a recording
                    // used to wedge it expanded with frozen bars (r2 #35b).
                    crate::dictation_hide();
                    std::process::exit(0);
                }
            }
        }
        Some(_) | None => {
            let _ = writeln!(s, "unknown");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::unbounded;
    use std::io::Read;

    #[test]
    fn known_commands_parse() {
        assert_eq!(parse_cmd("toggle"), Some("toggle"));
        assert_eq!(parse_cmd("hold_start"), Some("hold_start"));
        assert_eq!(parse_cmd("hold_end"), Some("hold_end"));
        assert_eq!(parse_cmd("status"), Some("status"));
        assert_eq!(parse_cmd("quit"), Some("quit"));
        assert_eq!(parse_cmd(" toggle \n"), Some("toggle"));
    }

    #[test]
    fn unknown_commands_rejected() {
        assert_eq!(parse_cmd(""), None);
        assert_eq!(parse_cmd("togglex"), None);
        assert_eq!(parse_cmd("TOGGLE"), None);
    }

    #[test]
    fn socket_burst_and_stalled_client_recover_cleanly() {
        let dir = std::env::temp_dir().join(format!("sd-ctrl-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let sock = dir.join("ctrl.sock");
        let listener = UnixListener::bind(&sock).unwrap();
        let (tx, rx) = unbounded::<HotkeyEvent>();
        let rec = Arc::new(AtomicBool::new(false));
        let rec_srv = Arc::clone(&rec);

        let srv = thread::spawn(move || {
            for stream in listener.incoming().take(12) {
                handle_one(stream, &tx, &rec_srv);
            }
        });

        // 1. Intentionally stalled client (connects, sends no newline, holds socket open)
        //    must time out inside handle_one (~500ms) instead of wedging the listener.
        let stalled = UnixStream::connect(&sock).unwrap();

        // 2. Next valid commands must all succeed promptly after the stall timeout.
        let mut s = UnixStream::connect(&sock).unwrap();
        s.write_all(b"hold_start\n").unwrap();
        let mut resp = String::new();
        s.read_to_string(&mut resp).unwrap();
        assert_eq!(resp.trim(), "ok");
        drop(stalled);

        for cmd in [
            "status",
            "hold_end",
            "toggle",
            "unknown_cmd",
            "toggle",
            "hold_start",
            "hold_end",
            "status",
            "toggle",
            "status",
        ] {
            let mut c = UnixStream::connect(&sock).unwrap();
            writeln!(c, "{cmd}").unwrap();
            let mut out = String::new();
            c.read_to_string(&mut out).unwrap();
            if cmd == "unknown_cmd" {
                assert_eq!(out.trim(), "unknown");
            } else {
                assert!(out.starts_with("ok"), "cmd {cmd} got {out:?}");
            }
        }
        srv.join().unwrap();
        let events: Vec<HotkeyEvent> = rx.try_iter().collect();
        assert_eq!(
            events,
            vec![
                HotkeyEvent::HoldStart,
                HotkeyEvent::HoldEnd,
                HotkeyEvent::Toggle,
                HotkeyEvent::Toggle,
                HotkeyEvent::HoldStart,
                HotkeyEvent::HoldEnd,
                HotkeyEvent::Toggle,
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
