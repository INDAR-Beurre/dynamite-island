#!/usr/bin/env bash
# bounds.sh — the exit-code contract for the motion instrument.
#
#   scripts/motion/bounds.sh [OUTDIR] [TRIALS]
#
# Exit 0 only if every resolved metric passed its bound. Non-zero otherwise, so
# CI or a fixer loop can gate on it. This file is owned by the motion instrument
# and does not depend on the rest of run_all.sh, which several agents share.
set -uo pipefail
cd "$(dirname "$0")" || exit 1
OUT=${1:-/tmp/motion-bounds/$(date +%Y%m%d-%H%M%S)}
TRIALS=${2:-5}
export XDG_RUNTIME_DIR=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}
mkdir -p "$OUT" || exit 1
python3 ./motion.py --source wf --trials "$TRIALS" \
  --json-out "$OUT/report.json" > "$OUT/stdout.json" 2> "$OUT/table.txt"
rc=$?
sed -n '/^PASS\|^FAIL\|^UNRESOLVED\|^NOTE\|^FAIL  overall\|^PASS  overall/p' \
  "$OUT/table.txt"
case $rc in
  0) echo "motion: every bound passed            ($OUT)" ;;
  1) echo "motion: BOUND VIOLATED — see $OUT/table.txt" ;;
  *) echo "motion: could not run (rc=$rc) — see $OUT/table.txt" ;;
esac
exit $rc
