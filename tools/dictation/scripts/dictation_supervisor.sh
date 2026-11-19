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

# Every transcription is printed to stdout, so this log is a verbatim record
# of everything the owner dictates. It was mode 644 — readable by any local
# user — and never rotated. Create it private BEFORE anything is appended,
# and tighten it if an older run already left it world-readable.
umask 077
: >>"$LOG"
chmod 600 "$LOG" 2>/dev/null || true
# Keep one previous generation; the file grows without bound otherwise.
if [ "$(stat -c %s "$LOG" 2>/dev/null || echo 0)" -gt 1048576 ]; then
    mv -f "$LOG" "$LOG.1"
    : >>"$LOG"
    chmod 600 "$LOG" 2>/dev/null || true
fi

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