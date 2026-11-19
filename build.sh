#!/bin/sh
# Build both crates. They are NOT a workspace (different smithay-client-toolkit
# pins), so each needs its own invocation. Fail loudly on the first failure.
set -eu
cd "$(dirname "$0")"

command -v cargo >/dev/null 2>&1 || {
    echo "build.sh: no cargo on PATH. Install Rust 1.88+ (rustup or your distro's rust package)." >&2
    exit 1
}

for lib in xkbcommon alsa; do
    pkg-config --exists "$lib" 2>/dev/null || {
        echo "build.sh: missing $lib (pkg-config). Install libxkbcommon / alsa-lib headers." >&2
        exit 1
    }
done

echo "==> panel"
# "$@" belongs to the panel only: the dictation crate is not vendored, so
# `./build.sh --offline` would fail its half even on a healthy machine.
cargo build --release "$@"

echo "==> dictation"
(cd tools/dictation && cargo build --release)

echo
echo "panel:     $PWD/target/release/dynamite-rust"
echo "dictation: $PWD/tools/dictation/target/release/static-dictation"