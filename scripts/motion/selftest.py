#!/usr/bin/env python3
"""selftest.py — verify analyze.py's detectors against synthetic frames.

A detector that has never been shown a known-bad input is not evidence. This
builds frames with KNOWN duplicates and KNOWN jumps, runs the real analyzer on
them, and asserts it reports exactly those. Cheap: 7 tiny frames, no screen.

Run: python3 scripts/motion/selftest.py
Exit: 0 pass, 1 fail.
"""
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image

HERE = Path(__file__).resolve().parent


def build(d: Path):
    """7 frames. Ground truth:
         idx 0,1,2 -> identical      (2 duplicates)
         idx 3     -> small change
         idx 4     -> identical to 3 (1 duplicate)
         idx 5     -> big jump
         idx 6     -> identical to 5 (1 duplicate)
       so 4 duplicates, and exactly 1 spike (the step into idx 5).
    """
    base = np.zeros((40, 40, 3), np.uint8)
    small = base.copy()
    small[0:10, :, :] = 90          # a modest, obviously-not-a-jump change
    big = base.copy()
    big[:, :, :] = 220              # a huge change = a skipped frame
    frames = [base, base, base, small, small, big, big]

    for i, f in enumerate(frames):
        Image.fromarray(f).save(d / f"frame_{i:04d}.ppm")

    t0 = 1_000_000_000
    with open(d / "timestamps.txt", "w") as fh:
        for i in range(len(frames)):
            s = t0 + i * 20_000_000
            fh.write(f"{i} {s} {s + 8_000_000} 0\n")
    (d / "meta.txt").write_text(
        "requested_seconds=0.14\nrequested_fps=50\nlogical_width=40\n"
        "logical_height=40\nregion=40,40\n"
    )
    return 4, 1


def main() -> int:
    d = Path(tempfile.mkdtemp(prefix="motion-selftest-"))
    try:
        want_dup, want_spike = build(d)
        r = subprocess.run([sys.executable, str(HERE / "analyze.py"), str(d), "--json"],
                           capture_output=True, text=True)
        if r.returncode != 0:
            print("FAIL: analyze.py exited", r.returncode, r.stderr)
            return 1
        import json
        res = json.loads(r.stdout)
        got_dup = res["zero_delta_count"]
        got_spike = res["spike_count"]
        ok = True
        for label, got, want in (("duplicates", got_dup, want_dup),
                                 ("spikes", got_spike, want_spike)):
            mark = "ok" if got == want else "FAIL"
            if got != want:
                ok = False
            print(f"  {label:12s} want {want}  got {got}  [{mark}]")
        # interval math: 6 gaps of 20 ms
        med = res["interval_ms"]["median"]
        if med != 20.0:
            print(f"  interval    want 20.0  got {med}  [FAIL]")
            ok = False
        else:
            print(f"  interval    want 20.0  got {med}  [ok]")
        if not ok:
            print("\nFAIL: analyze.py did not report the injected defects.")
            print("frame deltas were:", [round(x, 1) for x in
                                        sorted({s["delta"] for s in res["spike_frames"]})])
        else:
            print("\nPASS: duplicates, spikes and intervals all detected correctly.")
        return 0 if ok else 1
    finally:
        shutil.rmtree(d, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
