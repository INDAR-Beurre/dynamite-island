#!/usr/bin/env python3
"""Enable or detach Dynamite website colours in a chosen browser profile."""

import argparse
import os
from pathlib import Path
import re
import stat
import tempfile

PREF = 'toolkit.legacyUserProfileCustomizations.stylesheets'
PREF_LINE = f'user_pref("{PREF}", true); // Dynamite web theme'
IMPORT_MARKER = ' /* Dynamite web theme */'


def read_regular(path):
    if path.is_symlink() or (path.exists() and not path.is_file()):
        raise ValueError(f'Not a regular file: {path}')
    return path.read_bytes() if path.exists() else b''


def replace(path, content):
    if path.exists() and path.read_bytes() == content:
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as tmp:
        staged = Path(tmp.name)
        try:
            if path.exists():
                os.fchmod(tmp.fileno(), stat.S_IMODE(path.stat().st_mode))
            tmp.write(content)
            tmp.close()
            staged.replace(path)
        finally:
            staged.unlink(missing_ok=True)


def configure(profile, config, remove):
    if not profile.is_dir() or not (profile / 'prefs.js').is_file():
        raise ValueError(f'Not an initialized browser profile: {profile}')
    css = profile / 'chrome/userContent.css'
    prefs = profile / 'user.js'
    old_css, old_prefs = read_regular(css), read_regular(prefs)
    # Preserve the BOM and put imports after an optional @charset rule.
    prefix = re.match(rb'(?:\xef\xbb\xbf)?(?:@charset "[^"\r\n]+";[\r\n]*)?', old_css).group()
    other_css = prefix + b''.join(line for line in old_css[len(prefix):].splitlines(keepends=True)
                                  if not (line.startswith(b'@import url(') and line.rstrip().endswith(IMPORT_MARKER.encode())))
    if remove:
        if old_css != other_css:
            replace(css, other_css)
        if PREF_LINE.encode() in old_prefs.splitlines():
            replace(prefs, b''.join(line for line in old_prefs.splitlines(keepends=True)
                                    if line.rstrip() != PREF_LINE.encode()))
        return

    existing_pref = re.findall(rb'''^\s*user_pref\(\s*["']''' + re.escape(PREF.encode())
                               + rb'''["']\s*,\s*([^)]*)\)''', old_prefs, re.M)
    if any(value.strip() != b'true' for value in existing_pref):
        raise ValueError(f'{prefs} explicitly disables custom stylesheets; leave it unchanged')
    source = (config / 'dynamite-rust/theme/browser.css').resolve().as_uri()
    for path, original in [(css, old_css), (prefs, old_prefs)]:
        backup = path.with_name(path.name + '.before-dynamite')
        if path.exists() and not backup.exists():
            backup.parent.mkdir(parents=True, exist_ok=True)
            with backup.open('xb') as saved:
                os.fchmod(saved.fileno(), stat.S_IMODE(path.stat().st_mode))
                saved.write(original)
    separator = b'\n' if b'@charset' in prefix and not prefix.endswith((b'\r', b'\n')) else b''
    imported = f'@import url("{source}");{IMPORT_MARKER}\n'.encode()
    replace(css, prefix + separator + imported + other_css[len(prefix):])
    if not existing_pref:
        replace(prefs, old_prefs + (b'\n' if old_prefs and not old_prefs.endswith(b'\n') else b'')
                + PREF_LINE.encode() + b'\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('profile', type=Path, help='Profile directory shown in about:support')
    parser.add_argument('--remove', action='store_true', help='Remove only the Dynamite import and startup preference')
    args = parser.parse_args()
    config = Path(os.environ.get('XDG_CONFIG_HOME') or Path.home() / '.config')
    try:
        configure(args.profile.expanduser(), config, args.remove)
    except (OSError, ValueError) as error:
        parser.exit(1, f'{error}\n')
    print('Restart the browser to pick up the change. No browser process was stopped.')


if __name__ == '__main__':
    main()
