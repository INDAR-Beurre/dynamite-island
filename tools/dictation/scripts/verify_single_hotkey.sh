#!/usr/bin/env bash
# Live proof: Super+D is the only keyboard shortcut, and it is push-to-talk.
set -u
# Resolve relative to this script so the proof survives the scratch dir being
# deleted — that deletion is the whole point of embedding the tool.
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
chmod +x scripts/inject_super_d.py
ORIG=$(pactl get-default-source)
pactl load-module module-null-sink sink_name=superd-sink >/dev/null 2>&1
pactl set-default-source superd-sink.monitor
restore() {
    pactl set-default-source "$ORIG" >/dev/null 2>&1
    pactl unload-module module-null-sink >/dev/null 2>&1
}
trap restore EXIT
rm -f /tmp/superd.log /tmp/superd.stdout /tmp/superd-reemit.txt /tmp/superd-watch.log
python3 scripts/watch_reemit.py 26 /tmp/superd-reemit.txt >/tmp/superd-watch.log 2>&1 &
WATCH=$!
cp target/release/static-dictation /tmp/static-dictation-superd
env -u WAYLAND_DISPLAY -u DISPLAY timeout 26 /tmp/static-dictation-superd \
    >/tmp/superd.stdout 2>/tmp/superd.log &
BIN=$!
sleep 1
python3 scripts/inject_super_d.py chord >/tmp/superd-inject.log 2>&1
sleep 1
python3 scripts/inject_super_d.py chord >>/tmp/superd-inject.log 2>&1
sleep 1
python3 scripts/inject_super_d.py old >/tmp/superd-old.log 2>&1
sleep 1
wait "$WATCH" || true
wait "$BIN" || true
echo "=== stdout ==="; cat /tmp/superd.stdout
echo "=== hotkey lines ==="; grep -E "armed|->|hold (start|end)|toggle event|hk:" /tmp/superd.log || true
echo "=== re-emit EV_KEY (code,value) ==="; cat /tmp/superd-reemit.txt || true
echo "=== regression assertions ==="
python3 - <<'PY'
log = open('/tmp/superd.log').read()
lines = open('/tmp/superd-reemit.txt').read().split()
check = {
    'two Super+D holds start': log.count('-> HoldStart') == 2,
    'two Super+D holds end': log.count('-> HoldEnd') == 2,
    'no keyboard toggle emitted': '-> Toggle' not in log,
    'old AltGr+L now just types': '100,1' in lines and '38,1' in lines,
    'no stray D from the chord': lines.count('32,1') == 0,
    'no synthetic Meta keys': '125,1' not in lines and '126,1' not in lines,
}
for name, ok in check.items():
    print(('PASS' if ok else 'FAIL'), name)
raise SystemExit(0 if all(check.values()) else 1)
PY
