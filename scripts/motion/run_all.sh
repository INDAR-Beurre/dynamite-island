#!/usr/bin/env bash
# run_all.sh — one command, whole baseline, same JSON shape every time.
#
#   scripts/motion/run_all.sh [OUTDIR] [TRIALS]
#
# OUTDIR defaults to /tmp/motion-baseline/<stamp> (never the repo: media is not
# gitignored and a 250 MB frame set must not be committed). TRIALS defaults 5.
#
# Stages
#   00 instrument  motion.py: wf-recorder 60 Hz + bounds   -> PASS/FAIL per metric
#   01 settled     grim, 6 s, no trigger          -> is the GUARD at rest
#   02 transition  grim, 2 s, `music` at 0.8 s    -> control, no PTS
#   03 sustained   grim, 3 s, bars driven at 100Hz-> long-run content rate
#   04 recorded    wf-recorder 2 s x TRIALS, CFR  -> THE baseline, real PTS
#   05 vfr         wf-recorder 2 s, damage-gated  -> cross-check of 04
#
# Every stage leaves frame_*.ppm + timestamps.txt + meta.txt + analysis.json
# behind, so a reviewer can re-derive any number without re-running the panel.
# Exit is non-zero only if a stage fails; a low score is data, not an error.
set -uo pipefail
cd "$(dirname "$0")" || exit 1

STAMP=$(date +%Y%m%d-%H%M%S)
OUT=${1:-/tmp/motion-baseline/$STAMP}
TRIALS=${2:-5}
# Derive the crop from the LIVE pill instead of hardcoding it. A 260-logical-px
# window around a 65-px pill is 75% wallpaper, which inflates the noise floor
# and can mask a real stall (the 20261118-213403 "settled" control showed
# median delta 13.8 and 4 spikes on a pill that cannot move).
# Margin is generous enough to contain the widest expanded panel.
REGION=$("${PYTHON:-python3}" - <<'PY'
import json, subprocess, sys
try:
    d = json.loads(subprocess.run(
        [sys.executable, "../../scripts/measure.py"],
        capture_output=True, text=True, check=True).stdout)
    p = d["pill"]
    if d.get("state") == "not-collapsed":
        raise ValueError("not collapsed")
    s = 1.5
    cx = (p["x"] + p["w"] / 2) / s
    x0 = max(0, int(cx - 200)); x1 = int(cx + 200)
    print(f"{x0},0,{x1},210")
except Exception:
    print("640,0,900,210")   # fall back to the old window, never abort
PY
)
FPS=20
SETTLE=5
PY=${PYTHON:-python3}

say() { printf '\n=== %s ===\n' "$*"; }
run_stage() {  # run_stage <dir> <analyze args...>
  local d=$1; shift
  $PY analyze.py "$d" --json "$@" > "$d/analysis.json" || return 1
  $PY - "$d" <<'EOF'
import json, sys
d = sys.argv[1]
r = json.load(open(f"{d}/analysis.json"))
m, s = r.get("motion") or {}, r.get("score", {})
print(f"  frames {r['frames_captured']}/{r['frames_requested'] or '?'} captured"
      f"  drop {r['drop_rate_pct']}%  {r['achieved_fps']} fps"
      f"  [{r['interval_source']}]")
print(f"  interval_ms median {r['interval_ms']['median']} p95 "
      f"{r['interval_ms']['p95']}  zero-delta {r['zero_delta_count']}"
      f"/{r['frames_compared']}  spikes {r['spike_count']}")
if m:
    print(f"  poses {m['poses']} (unique {m['poses_unique']}) over {m['span_ms']}ms "
          f"= {m.get('pose_rate_hz')} Hz  dup {m['duplicate_rate']}  "
          f"jump {m.get('jump_rate')}  completeness {m.get('completeness')}")
print(f"  SCORE {s.get('smoothness')} ({s.get('kind')})")
EOF
}

[ "$(id -u)" -ne 0 ] && [ -z "${XDG_RUNTIME_DIR:-}" ] && export XDG_RUNTIME_DIR=/run/user/$(id -u)
mkdir -p "$OUT" || exit 1
say "baseline -> $OUT   (trials $TRIALS, loadavg $(cut -d' ' -f1-3 /proc/loadavg))"
printf 'stamp=%s\nregion=%s\ntrials=%s\nloadavg=%s\n' "$STAMP" "$REGION" "$TRIALS" \
  "$(cut -d' ' -f1-3 /proc/loadavg)" > "$OUT/env.txt"
{ git -C .. rev-parse --short HEAD 2>/dev/null || echo unknown; } > "$OUT/commit.txt"
# Which binary answered the commands: a fixer may rebuild/restart mid-baseline.
PID=$(pidof dynamite-rust 2>/dev/null | awk '{print $1}')
{ echo "panel_pid=$PID"
  echo "panel_exe_started=$( [ -n "$PID" ] && ps -o lstart= -p "$PID" | sed 's/^ *//' )"
  echo "panel_exe_mtime=$( [ -e "/proc/$PID/exe" ] && date -r "$(readlink -f /proc/$PID/exe)" '+(%Y-%m-%d %H:%M:%S)' )"
  echo "panel_exe=$( [ -n "$PID" ] && readlink -f /proc/$PID/exe )"; } > "$OUT/panel.txt"
cat "$OUT/panel.txt" | sed 's/^/  /' 
sleep "$SETTLE"

FAILED=()

say "01 settled (grim, 6 s, no trigger)"
./capture.sh "$OUT/01-settled" 6 "$FPS" --region "$REGION" >"$OUT/01-settled.log" 2>&1 \
  && run_stage "$OUT/01-settled" || FAILED+=(01-settled)

say "02 control (grim, 20 Hz: cannot resolve a 160 ms morph — shows why PTS capture is needed)"
./capture.sh "$OUT/02-transition" 2 "$FPS" --region "$REGION" --trigger "music@0.8" >"$OUT/02-transition.log" 2>&1 \
  && run_stage "$OUT/02-transition" || FAILED+=(02-transition)
# grim cannot time a 160 ms morph: stage 02 is a control that shows the drop
# rate and the content rate agree with 04 without needing PTS. Its SCORE is
# not the baseline.

say "03 sustained (grim, 3 s, 27 bars at 100 Hz)"
mkdir -p "$OUT/03-sustained"
$PY drive_bars.py 2.6 >"$OUT/03-sustained/bars_driver.log" 2>&1 &
DRV=$!
./capture.sh "$OUT/03-sustained" 3 "$FPS" --region "$REGION" \
  >"$OUT/03-sustained.log" 2>&1 && run_stage "$OUT/03-sustained" || FAILED+=(03-sustained)
wait $DRV 2>/dev/null
tail -2 "$OUT/03-sustained/bars_driver.log" 2>/dev/null | sed 's/^/  driver: /'

# 00 is the bounds-checking instrument (motion.py). It exits 1 on a violated
# bound and that DOES fail this run — a missed bound is the result.
say "00 instrument (motion.py) — bounded metrics, fails the run on a violation"
MR="$OUT/00-instrument"
mkdir -p "$MR"
if [ ! -f ./motion.py ]; then
  echo "  skipped: motion.py not present in this checkout"
elif $PY ./motion.py --source wf --trials "$TRIALS" \
     --json-out "$MR/report.json" > "$MR/stdout.json" 2> "$MR/table.txt"; then
  echo "  instrument PASSED every bound"
else
  rc=$?
  if [ "$rc" -eq 2 ]; then
    echo "  instrument could not run (rc=2) — see $MR/table.txt (not a failure of this run)"
  else
    echo "  instrument ran, bounds violated:"
    sed -n '/^PASS\|^FAIL\|^UNRESOLVED\|^FAIL  overall/p' "$MR/table.txt" | sed 's/^/    /'
    # A violated bound is a real result, so it must fail the run.
    FAILED+=(00-instrument)
  fi
fi

say "04 recorded CFR 60 Hz x $TRIALS (real PTS — the baseline)"
# This screen is shared: other agents open windows, switch workspaces, drive the
# same panel. A trial that recorded somebody else's change is not evidence, so
# each trial is retried until the capture was quiet before our own trigger.
ATTEMPTS=4
is_clean() { $PY -c 'import json,sys; print(int(json.load(open(sys.argv[1]))["contamination"]["clean"]))' "$1/analysis.json" 2>/dev/null; }
for i in $(seq 1 "$TRIALS"); do
  d="$OUT/04-record-cfr-$i"
  for a in $(seq 1 $ATTEMPTS); do
    echo "--- trial $i/$TRIALS attempt $a/$ATTEMPTS"
    rm -rf "$d"
    if ! ./record.sh "$d" 2 60 --region "$REGION" --trigger "music@0.8" \
         >"$OUT/04-record-cfr-$i.log" 2>&1; then
      FAILED+=("04-cfr-$i"); break
    fi
    run_stage "$d" || { FAILED+=("04-cfr-$i"); break; }
    [ "$(is_clean "$d")" = 1 ] && break
    echo "    contaminated (another agent on the screen) — retrying"
    sleep 0.6
  done
  [ "$(is_clean "$d")" = 1 ] || echo "    WARNING: trial $i kept UNCLEAN after $ATTEMPTS attempts"
done

say "05 recorded VFR (damage-gated, cross-check only)"
d="$OUT/05-record-vfr"
./record.sh "$d" 2 60 --region "$REGION" --trigger "music@0.8" --vfr \
  >"$OUT/05-record-vfr.log" 2>&1 && run_stage "$d" || FAILED+=("05-vfr")

say "SUMMARY"
$PY - "$OUT" <<'EOF'
import glob, json, statistics as st, sys
out = sys.argv[1]
rows, cfr = [], []
for p in sorted(glob.glob(f"{out}/*/analysis.json")):
    r = json.load(open(p)); m = r.get("motion") or {}; s = r.get("score", {})
    name = p.split("/")[-2]
    rows.append((name, r.get("recorder_mode") or "grim", m, s, r))
    if name.startswith("04"):
        cfr.append((m, s, r))
print(f"{'stage':18s} {'clock':10s} {'clean':5s} {'poses':>5s} {'uniq':>4s} {'dups':>4s} "
      f"{'jump':>5s} {'C':>5s} {'D':>5s} {'J':>5s} {'SCORE':>6s}")
for name, mode, m, s, raw in rows:
    clean = "yes" if (raw.get("contamination") or {}).get("clean") else "NO"
    if m:
        print(f"{name:18s} {mode:10s} {clean:5s} {m['poses']:5d} {m['poses_unique']:4d} "
              f"{m['duplicates']:4d} {m['spikes']:5d} "
              f"{str(m.get('completeness')):>5s} {str(m.get('duplicate_rate')):>5s} "
              f"{str(m.get('jump_rate')):>5s} {str(s.get('smoothness')):>6s}")
    else:
        print(f"{name:18s} {mode:10s} {'-':>5s} {'-':>4s} {'-':>4s} {'-':>5s} "
              f"{'-':>5s} {'-':>5s} {'-':>5s} {str(s.get('smoothness')):>6s}")
sc = [s["smoothness"] for _, s, _ in cfr if s.get("smoothness") is not None]
po = [m["poses"] for m, _, _ in cfr if m]
if sc:
    print(f"\nHEADLINE (04, n={len(sc)}): median {st.median(sc)}  "
          f"min {min(sc)}  max {max(sc)}   formula {cfr[0][1]['formula']}")
if po:
    print(f"poses per recorded morph: {po}  median {st.median(po)} "
          f"(target {10} = one pose per 16.7 ms over a 160 ms morph)")
vr = [r for r in rows if r[0].startswith("05")]
if vr and vr[0][2] and po:
    print(f"cross-check 05-vfr: {vr[0][2]['poses']} poses, {vr[0][2]['span_ms']} ms span, "
          f"same morph -> agrees with 04" if abs(vr[0][2]['poses'] - st.median(po)) <= 3
          else f"cross-check 05-vfr: {vr[0][2]['poses']} poses vs 04 median {st.median(po)}: DISAGREE")
print(f"\nmedia: {out}   (keep out of git)")
EOF

if [ ${#FAILED[@]} -gt 0 ]; then
  say "FAILED stages: ${FAILED[*]}"; exit 1
fi
say "done — no stage failed"
