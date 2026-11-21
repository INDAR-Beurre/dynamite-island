#!/bin/sh
# Install the boot UI without restarting greetd or changing authentication.
set -eu
if [ "$#" -ne 2 ] || { [ "$1" != --check ] && [ "$1" != --install ]; }; then
    echo "usage: $0 --check|--install USER_HOME" >&2
    exit 2
fi
mode=$1
source_home=$(readlink -f "$2")
repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
for program in python3 sway dbus-run-session dbus-update-activation-environment; do
    command -v "$program" >/dev/null || { echo "missing: $program" >&2; exit 1; }
done
[ -x "$repo/target/release/dynamite-rust" ]
[ -f "$source_home/.config/mango/wallpaper/wallpaper.png" ]
[ -r /usr/lib/libgtk4-layer-shell.so ]
python3 - "$repo" <<'PY'
import ast, sys
from pathlib import Path
p = Path(sys.argv[1])
ast.parse((p / 'deploy/face-greeter.py').read_text())
import gi
gi.require_version('Gtk', '4.0')
gi.require_version('Gtk4LayerShell', '1.0')
PY
if [ "$mode" = --check ]; then
    echo 'Boot greeter files/dependencies ready; no service or authentication changed.'
    exit 0
fi
[ "$(id -u)" -eq 0 ] || { echo 'Run --install with sudo.' >&2; exit 1; }
python3 - <<'PY'
import tomllib
from pathlib import Path
c = tomllib.loads(Path('/etc/greetd/config.toml').read_text())
assert c['default_session'] == {'command': '/usr/local/bin/face-greeter-session', 'user': 'greeter'}, 'unexpected greetd configuration; refusing to replace another greeter'
PY
backup="/var/backups/dynamite-greeter-$(date +%Y%m%d-%H%M%S)"
install -d -m 700 "$backup"
for file in /usr/local/bin/face-greeter /usr/local/bin/face-greeter-session /usr/local/bin/face-greeter-run /etc/greetd/sway-greeter.conf; do
    [ ! -L "$file" ] || { echo "refusing symlink: $file" >&2; exit 1; }
    cp -p "$file" "$backup/$(basename "$file")"
done
cp -p /etc/greetd/config.toml "$backup/config.toml"
assets=/usr/local/share/dynamite-greeter
install -d -m 755 /usr/local/libexec/dynamite-greeter "$assets/.config/mango/wallpaper" "$assets/.config/dynamite-rust"
install -m 755 "$repo/target/release/dynamite-rust" /usr/local/libexec/dynamite-greeter/dynamite-rust
install -m 644 "$source_home/.config/mango/wallpaper/wallpaper.png" "$assets/.config/mango/wallpaper/wallpaper.png"
# Copy only the existing clock preference; never copy user sessions or credentials.
python3 - "$source_home" "$assets" <<'PY'
from pathlib import Path
import sys
src = Path(sys.argv[1]) / '.config/dynamite-rust/config.conf'
clock = '1'
if src.is_file():
    for line in src.read_text().splitlines():
        key, sep, value = line.partition('=')
        if sep and key.strip() == 'clock_24h':
            clock = '1' if value.strip() in ('1', 'true') else '0'
p = Path(sys.argv[2]) / '.config/dynamite-rust/config.conf'
p.write_text(f'clock_24h={clock}\n')
p.chmod(0o644)
PY
install -d -o greeter -g greeter -m 700 /var/cache/dynamite-greeter
install -m 755 "$repo/deploy/face-greeter.py" /usr/local/bin/face-greeter
install -m 755 "$repo/deploy/face-greeter-run" /usr/local/bin/face-greeter-run
install -m 644 "$repo/deploy/sway-greeter.conf" /etc/greetd/sway-greeter.conf
install -m 755 "$repo/deploy/face-greeter-session" /usr/local/bin/face-greeter-session
printf 'Installed boot UI. Backup: %s\n' "$backup"
echo 'greetd/PAM, enrollment and running desktop unchanged. Takes effect at the next login.'
