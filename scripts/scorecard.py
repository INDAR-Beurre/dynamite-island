#!/usr/bin/env python3
"""Append a measurement to docs/scorecard.jsonl and print the trend.

The owner asked to see the screenshot numbers go up. A number with no history
is not a trend, so every measurement worth quoting gets recorded here with the
binary's mtime, so a row can never be mistaken for a different build.

    python3 scripts/scorecard.py --label r1b            # measure live + record
    python3 scripts/scorecard.py --png /tmp/x.png --label ref
    python3 scripts/scorecard.py --show                 # table only
"""
import argparse
import json
import os
import pathlib
import subprocess
import sys
import time

REPO = pathlib.Path(__file__).resolve().parent.parent
LEDGER = REPO / "docs" / "scorecard.jsonl"
BIN = REPO / "target" / "release" / "dynamite-rust"

# Lower is better for these; higher is better for the rest.
LOWER = {"lapvar_norm", "lapmean_norm", "rss_mb", "cpu_pct"}
# Direction is meaningless for geometry: the target is a value, not a limit.
TARGETS = {"aspect": 2.643, "cap_over_pill_h": 0.357, "stroke_over_cap": 0.200,
           "text_over_pill_w": 0.486, "lapvar_norm": 1277.0}


def flatten(m):
    """Pull the comparable scalars out of measure.py's nested JSON.

    proc_stats() is merged at the TOP level by measure.py, not under a
    "proc" key, so rss_mb/cpu_pct must be read from the root or every row
    silently loses its resource columns.
    """
    row = {}
    pill = m.get("pill") or {}
    for k in ("w", "h", "aspect", "center_offset_px", "width_pct"):
        if k in pill:
            row[k] = pill[k]
    for section in ("ink", "glyphs", "screen"):
        for k, v in (m.get(section) or {}).items():
            if isinstance(v, (int, float)):
                row[k] = v
    for k in ("rss_mb", "cpu_pct"):
        if isinstance(m.get(k), (int, float)):
            row[k] = m[k]
    return row


def show():
    if not LEDGER.exists():
        print("no measurements recorded yet")
        return 0
    rows = [json.loads(l) for l in LEDGER.read_text().splitlines() if l.strip()]
    keys = ["w", "h", "aspect", "cap_over_pill_h", "stroke_over_cap",
            "lapvar_norm", "rss_mb", "cpu_pct"]
    keys = [k for k in keys if any(k in r["m"] for r in rows)]
    ref = next((r["m"] for r in rows if r.get("kind") == "ref"), None)
    print(f"{'label':<16}" + "".join(f"{k[:13]:>14}" for k in keys))
    for r in rows:
        flag = " *BUSY*" if r.get("contended") else ""
        print(f"{r['label'][:15]:<16}" +
              "".join(f"{r['m'].get(k, '-'):>14}" for k in keys) + flag)

    # Distance to the reference is the score; the trend is between LIVE rows.
    # Comparing a live row against the reference row and calling it "worse"
    # would invert the meaning of every improvement.
    if ref:
        # Contended rows are excluded from the verdict: their numbers belong
        # to whatever else was running, not to this build.
        live = [r for r in rows if r.get("kind") != "ref" and not r.get("contended")]
        if live:
            last = live[-1]["m"]
            print("\ngap to reference (0 = pixel-identical):")
            for k in keys:
                if k in TARGETS and k in last and isinstance(last[k], (int, float)):
                    t = TARGETS[k]
                    gap = abs(last[k] - t)
                    rel = gap / abs(t) * 100 if t else 0
                    print(f"  {k:<18} {last[k]:>10}  target {t:<9} "
                          f"off by {gap:>8.3g} ({rel:.1f}%)")
        if len(live) >= 2:
            a, b = live[-2], live[-1]
            print(f"\nprogress {a['label']} -> {b['label']}:")
            for k in keys:
                if k in TARGETS and k in a["m"] and k in b["m"]:
                    t = TARGETS[k]
                    was, now = abs(a["m"][k] - t), abs(b["m"][k] - t)
                    verdict = ("CLOSER" if now < was - 1e-9 else
                               "further" if now > was + 1e-9 else "same")
                    print(f"  {k:<18} {a['m'][k]:>10} -> {b['m'][k]:<10} {verdict}")
    return 0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--label", default=None)
    ap.add_argument("--png", default=None)
    ap.add_argument("--show", action="store_true")
    ap.add_argument("--ref", action="store_true",
                    help="this row is the reference, not a build")
    args = ap.parse_args()
    if args.show or not args.label:
        return show()

    cmd = [sys.executable, str(REPO / "scripts" / "measure.py")]
    if args.png:
        cmd += ["--png", args.png]
    p = subprocess.run(cmd, capture_output=True, text=True)
    if p.returncode != 0:
        print(p.stdout + p.stderr)
        return p.returncode
    m = json.loads(p.stdout)
    if m.get("state") == "not-collapsed":
        print("REFUSED: measure.py says the frame is not a collapsed pill.\n"
              "Recording it would put a lie in the trend. Re-capture on a "
              "quiet screen with the panel collapsed.")
        return 2
    # A busy machine inflates CPU and latency. Recording that as a build's
    # result would put a lie in the trend (a cargo test next door read 91%
    # on a panel that idles at 0.5%). Mark the row instead of trusting it.
    load = os.getloadavg()[0]
    busy = load > 4.0
    row = {"label": args.label, "t": time.strftime("%Y-%m-%d %H:%M:%S"),
           "loadavg": round(load, 2), "contended": busy,
           "kind": "ref" if args.ref else "live",
           "bin_mtime": (time.strftime("%H:%M", time.localtime(BIN.stat().st_mtime))
                         if BIN.exists() and not args.ref else None),
           "png": args.png, "m": flatten(m)}
    LEDGER.parent.mkdir(parents=True, exist_ok=True)
    with LEDGER.open("a") as f:
        f.write(json.dumps(row) + "\n")
    print(f"recorded {args.label}")
    if busy:
        print(f"WARNING: loadavg {load:.2f} — this machine is busy, so cpu_pct and\n"
              "         any latency figure in this row are NOT the build's steady\n"
              "         state. Re-measure in a quiet window before quoting them.")
    return show()


if __name__ == "__main__":
    sys.exit(main())