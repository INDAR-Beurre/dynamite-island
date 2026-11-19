#!/bin/sh
# Launch the island panel. No silent substitution: if the binary is missing we
# exit non-zero rather than starting a different UI (the old script fell back
# to `qs -c dynamite`, an unrelated Quickshell config).
set -eu
ISLAND_HOME="${ISLAND_HOME:-$(cd "$(dirname "$0")/.." && pwd)}"
BIN="$ISLAND_HOME/target/release/dynamite-rust"
[ -n "${XDG_RUNTIME_DIR:-}" ] || { echo "dynamite: XDG_RUNTIME_DIR unset" >&2; exit 1; }
[ -x "$BIN" ] || { echo "dynamite: no binary at $BIN — run $ISLAND_HOME/build.sh" >&2; exit 1; }
exec "$BIN"
