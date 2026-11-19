#!/usr/bin/env bash
# Keep the global hotkey listener alive: restart it if it ever exits.
#
# Runs the in-repo build only. A missing build is a loud error, not a silent
# substitution: the old scratch-directory fallback ran a binary whose source
# was not in the repo, so nobody could audit or reproduce what was running.
set -u
here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG="$HOME/.cache/static-dictation.log"

# Every transcription is printed to stdout, so this log is a verbatim record
# of everything the owner dictates. It was mode 644 — readable by any local
# user — and never rotated. Create it private BEFORE anything is appended,
# and tighten it if an older run already left it world-readable.
umask 077
mkdir -p "$(dirname "$LOG")"
: >>"$LOG"
chmod 600 "$LOG" 2>/dev/null || true
# Keep one previous generation; the file grows without bound otherwise.
if [ "$(stat -c %s "$LOG" 2>/dev/null || echo 0)" -gt 1048576 ]; then
    mv -f "$LOG" "$LOG.1"
    : >>"$LOG"
    chmod 600 "$LOG" 2>/dev/null || true
fi

# r2 #8c: no lock meant a second supervisor started a second binary; the
# newcomer stole the control socket and the old writer kept appending to a
# rotated file. Take an exclusive lock for the whole supervision, not per child.
LOCK="$HOME/.cache/static-dictation.lock"
exec 9>"$LOCK"
if ! flock -n 9; then
    echo "supervisor: another supervisor holds $LOCK; exiting" >&2
    exit 0
fi

BIN="$here/target/release/static-dictation"
if [ ! -x "$BIN" ]; then
    # stderr as well as the log: a failure buried in ~/.cache is invisible
    # in the session that started it.
    msg="supervisor: no dictation binary at $BIN; build it with:
  cd $here && cargo build --release"
    echo "$msg" >&2
    echo "$msg" >>"$LOG"
    exit 1
fi

# Forward our TERM to the child: stopping the supervisor used to leave the
# child dictating with an orphaned hotkey grab (r2 #7d).
child=""
trap '[ -n "$child" ] && kill -TERM "$child" 2>/dev/null; exit 0' TERM INT

rotate() {
    if [ "$(stat -c %s "$LOG" 2>/dev/null || echo 0)" -gt 1048576 ]; then
        mv -f "$LOG" "$LOG.1"
        : >>"$LOG"
        chmod 600 "$LOG" 2>/dev/null || true
    fi
}

while true; do
    # Rotate INSIDE the loop: a stable instance appends forever, and the
    # startup-only check relied on frequent restarts to bound the file.
    rotate
    "$BIN" >>"$LOG" 2>&1 &
    child=$!
    wait "$child"
    child=""
    sleep 1
done