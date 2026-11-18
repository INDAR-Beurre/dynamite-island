#!/usr/bin/env bash
# Stress harness for the live dynamite-rust panel.
#
# Safe to run against the production process: it only writes to the control
# socket, never sends `quit`, never restarts or rebuilds anything.
#
#   bash scripts/stress/run_all.sh              # everything (~2-3 min)
#   bash scripts/stress/run_all.sh --group latency
#
# Exit code is 0 only when every group passed.
set -u -o pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"

# The panel is a Wayland client: grim and the socket both need these.
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
export WAYLAND_DISPLAY="${WAYLAND_DISPLAY:-wayland-0}"

for tool in python3 grim ffmpeg ffprobe ss; do
    command -v "$tool" >/dev/null 2>&1 || {
        echo "FATAL: $tool not found in PATH" >&2
        exit 2
    }
done

if ! pgrep -x dynamite-rust >/dev/null 2>&1; then
    echo "FATAL: no dynamite-rust process is running" >&2
    echo "       (start it with ~/.config/mango/scripts/dynamite_autostart.sh)" >&2
    exit 2
fi

cd "$repo" || exit 2
python3 "$here/stress.py" "$@"
rc=$?

echo
if [ "$rc" -eq 0 ]; then
    echo "==> stress harness: PASS"
else
    echo "==> stress harness: FAIL (exit $rc)"
fi
exit "$rc"
