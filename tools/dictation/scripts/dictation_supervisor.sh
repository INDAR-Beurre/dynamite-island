#!/usr/bin/env bash
# Keep the global hotkey listener alive: restart it if it ever exits.
#
# Prefers the in-repo build so the tool ships with the package; falls back to
# the original scratch build so a failed rebuild never leaves the owner
# without dictation.
set -u
here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LEGACY="$HOME/.t3/scratch/2026-11-13-can-you-please-like-i-e1185665/target/release/static-dictation"
LOG="$HOME/.cache/static-dictation.log"

BIN="$here/target/release/static-dictation"
if [ ! -x "$BIN" ]; then
    BIN="$LEGACY"
    echo "supervisor: in-repo binary missing, using legacy $LEGACY" >>"$LOG"
fi
if [ ! -x "$BIN" ]; then
    echo "supervisor: no dictation binary found; build it with:
  cd $here && cargo build --release --offline" >>"$LOG"
    exit 1
fi

while true; do
    "$BIN" >>"$LOG" 2>&1
    sleep 1
done