#!/usr/bin/env python3
"""animate.py <command> [args]

Sends ONE control command to the running panel so a specific animation can be
captured deterministically. This is the same unix datagram socket the panel
binds at startup.

DISCOVERED FROM SOURCE (not guessed) — src/main.rs:
  * socket path      socket_path(), src/main.rs:1855-1861
                       $XDG_RUNTIME_DIR/dynamite-rust.sock  (fallback /tmp)
  * socket type      UnixDatagram, set_nonblocking, src/main.rs:6065-6068
  * handler          src/main.rs:6073-6213, one `word` per datagram, trimmed
  * state morphing   IslandState::from_word (src/main.rs:165-181) ->
                       begin_morph(loop_handle, new_state)  [160 ms, MORPH_DURATION_MS]
                     plus word aliases from the same handler.

COMMANDS THE BINARY ALREADY SUPPORTS (all non-destructive; this script refuses
anything not on this list, so it can never lock the screen, power off, or touch
the wallpaper):
  collapsed | dictation | lock | power | music | calendar | controlcenter
  launcher | themes | clipboard | traymenu          state words -> 160 ms morph
  clock                                            alias for collapsed
  dictation_start / dictation_end                  Enter/leave Dictation state
  dictation_bars: <27 floats 0..1>                 push waveform bar heights
  locktype / lockback / lockwrong                  lockscreen PREVIEW only; these
                                                   are no-ops outside Lockscreen,
                                                   and "lockscreen" itself is
                                                   refused here (it takes over the
                                                   screen). locktype/lockback are
                                                   harmless without it.
  reload                                           re-read config.conf

DELIBERATELY REFUSED by this script: quit (kills the panel — another agent is
using it), lockscreen (full-screen takeover), and the wallpaper/themes mutate
path is only reached via state words, which are previews only.

Usage:
  animate.py music                 # begin 160 ms expand morph
  animate.py collapsed             # begin 160 ms collapse morph
  animate.py bars 0.1 0.9 0.3      # start dictation + push a bar pattern
  animate.py --list
Exit 0 on send, 2 on refusal, 3 if the socket is missing.
"""
import os
import socket
import sys
import time

SOCK = os.path.join(os.environ.get("XDG_RUNTIME_DIR", "/tmp"), "dynamite-rust.sock")

STATE_WORDS = {
    "collapsed", "dictation", "lock", "power", "music", "calendar",
    "controlcenter", "displays", "launcher", "themes", "clipboard", "traymenu", "clock",
}
SAFE_WORDS = {"reload", "locktype", "lockback", "lockwrong",
              "dictation_start", "dictation_end"}
REFUSED = {
    "quit": "would kill the running panel (another agent is using it)",
    "lockscreen": "takes over the whole screen",
}


def send(word: str) -> None:
    s = socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM)
    try:
        s.sendto(word.encode(), SOCK)
    finally:
        s.close()

def list_cmds() -> None:
    print(f"socket: {SOCK}")
    print("state words (160 ms morph): " + " ".join(sorted(STATE_WORDS)))
    print("other safe words:           " + " ".join(sorted(SAFE_WORDS)))
    print("dictation_bars: <27 floats> (via `bars`)")
    print("refused: " + ", ".join(f"{k} ({v})" for k, v in REFUSED.items()))


def main() -> int:
    argv = sys.argv[1:]
    if not argv or argv[0] in ("-h", "--help", "--list"):
        list_cmds()
        return 0

    cmd = argv[0]
    rest = argv[1:]

    if cmd in REFUSED:
        print(f"refused: '{cmd}' {REFUSED[cmd]}", file=sys.stderr)
        return 2

    if not os.path.exists(SOCK):
        print(f"socket not found: {SOCK} (is dynamite-rust running?)", file=sys.stderr)
        return 3

    if cmd == "bars":
        vals = [max(0.0, min(1.0, float(v))) for v in rest][:27]
        vals += [0.0] * (27 - len(vals))
        word = "dictation_bars: " + " ".join(f"{v:.3f}" for v in vals)
    elif cmd in STATE_WORDS or cmd in SAFE_WORDS:
        if rest:
            print(f"'{cmd}' takes no arguments", file=sys.stderr)
            return 2
        word = cmd
    elif cmd.startswith("dictation_bars:"):
        word = cmd  # raw passthrough of an already-formed bar line
    else:
        print(f"unknown command '{cmd}'", file=sys.stderr)
        list_cmds()
        return 2

    try:
        send(word)
    except ConnectionRefusedError:
        # The panel rebinds its socket on restart; a datagram sent into the
        # stale inode is refused. Transient — retry a few times.
        for _ in range(20):
            time.sleep(0.25)
            try:
                send(word)
                break
            except ConnectionRefusedError:
                continue
        else:
            print(f"send failed: {SOCK} refuses datagrams (panel restarting?)",
                  file=sys.stderr)
            return 3
    except OSError as e:
        print(f"send failed: {e}", file=sys.stderr)
        return 3
    print(f"sent -> {SOCK}: {word[:80]}{'...' if len(word) > 80 else ''}")
    return 0

if __name__ == "__main__":
    sys.exit(main())
