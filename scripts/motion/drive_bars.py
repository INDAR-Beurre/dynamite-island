#!/usr/bin/env python3
"""drive_bars.py <seconds> — push a continuous dictation-bar animation.

The island only animates dictation bars while something feeds it amplitudes, so
a capture of a "held" animation is otherwise just a still frame. This sends
`dictation_bars: <27 floats>` at ~100 Hz for <seconds>, then `dictation_end`.

Why this exists: the 160 ms morph is shorter than one grim round-trip on a
loaded box, so it cannot be sampled. Sustained motion can.
"""
import math
import socket
import sys
import time

REFRESH_HZ = 100
BARS = 27
WAVE_HZ = 1.0          # bars breathe once a second — visible at 20 fps
PHASE_STEP = 0.35      # per-bar offset, so neighbours are out of phase


def main() -> int:
    secs = float(sys.argv[1]) if len(sys.argv) > 1 else 5.0
    s = socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM)
    s.settimeout(1.0)
    try:
        s.send(b"dictation_start")
    except OSError as e:
        print(f"driver: cannot send: {e}", file=sys.stderr)
        return 3
    t0 = time.time()
    sent = 0
    try:
        while time.time() - t0 < secs:
            t = time.time() - t0
            vals = " ".join(
                f"{0.5 + 0.5 * math.sin(2 * math.pi * WAVE_HZ * t + i * PHASE_STEP):.3f}"
                for i in range(BARS))
            s.send(f"dictation_bars: {vals}".encode())
            sent += 1
            time.sleep(1.0 / REFRESH_HZ)
    except OSError as e:
        print(f"driver: send failed after {sent}: {e}", file=sys.stderr)
        return 4
    finally:
        try:
            s.send(b"dictation_end")
        except OSError:
            pass
        s.close()
    print(f"driver: sent {sent} bar updates over {secs:.1f}s")
    return 0


if __name__ == "__main__":
    sys.exit(main())
