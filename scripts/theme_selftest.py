#!/usr/bin/env python3
"""Check profile changes in temp dirs; --browser also renders two local fixtures."""
import argparse
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('waterfox_theme', ROOT / 'deploy/waterfox-theme.py')
setup = importlib.util.module_from_spec(spec)
spec.loader.exec_module(setup)


def check_setup(root):
    profile = root / 'profile'
    (profile / 'chrome').mkdir(parents=True)
    (profile / 'prefs.js').touch()
    css = profile / 'chrome/userContent.css'
    prefs = profile / 'user.js'
    original = b'@charset "UTF-8";\n.fixture { border: 0; }\n'
    css.write_bytes(original)
    prefs.write_bytes(b'user_pref("browser.shell.checkDefaultBrowser", false);\n')
    config = root / 'config with spaces'
    setup.configure(profile, config, False)
    setup.configure(profile, config, False)
    assert css.read_bytes().count(b'Dynamite web theme') == 1
    assert css.read_bytes().startswith(b'@charset "UTF-8";\n@import ')
    assert b'%20' in css.read_bytes()
    assert css.with_name('userContent.css.before-dynamite').read_bytes() == original
    setup.configure(profile, config, True)
    assert css.read_bytes() == original
    assert prefs.read_bytes() == b'user_pref("browser.shell.checkDefaultBrowser", false);\n'
    prefs.write_text(f'user_pref("{setup.PREF}", false);\n')
    try:
        setup.configure(profile, config, False)
    except ValueError:
        pass
    else:
        raise AssertionError('must respect an explicitly disabled custom stylesheet')
    assert css.read_bytes() == original
    # A BOM without @charset also leaves the managed import removable.
    prefs.write_bytes(b'')
    bom = b'\xef\xbb\xbf.fixture { border: 0; }\n'
    css.write_bytes(bom)
    setup.configure(profile, config, False)
    setup.configure(profile, config, False)
    assert css.read_bytes().count(b'Dynamite web theme') == 1
    setup.configure(profile, config, True)
    assert css.read_bytes() == bom
    print('PASS: profile setup preserves content, backs up, deduplicates, removes, respects explicit disable')


def check_browser(root):
    from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
    import threading
    from PIL import Image

    class Fixture(BaseHTTPRequestHandler):
        def do_GET(self):
            body = b'''<!doctype html><body style="background:white;color:black"><main style="background:white"><h1>Theme fixture</h1><a href="#">Link</a><button>Button</button><img width="40" height="40" style="position:absolute;left:40px;top:160px" src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='40' height='40'%3E%3Crect width='40' height='40' fill='red'/%3E%3C/svg%3E"></main></body>'''
            self.send_response(200)
            self.send_header('Content-Type', 'text/html')
            self.end_headers()
            self.wfile.write(body)
        def log_message(self, *_):
            pass

    profile = root / 'browser'
    profile.mkdir()
    (profile / 'prefs.js').touch()
    config, runtime = root / 'config', root / 'runtime'
    runtime.mkdir(mode=0o700)
    library = config / 'dynamite-rust/library'
    library.mkdir(parents=True)
    (library / 'themes.json').write_text('''{"themes": {
"green": {"palette": ["#2D353B","#A7C080","#E69875","#DBBC7F","#7FBBB3","#D699B6"],"wallpapers": []},
"pink": {"palette": ["#272D2D","#576365","#80416C","#DD97AF","#483E66","#F7F3F3"],"wallpapers": []}}}''')
    setup.configure(profile, config, False)
    env = dict(os.environ, XDG_CONFIG_HOME=str(config), XDG_RUNTIME_DIR=str(runtime), MOZ_HEADLESS='1')
    with ThreadingHTTPServer(('127.0.0.1', 0), Fixture) as http:
        threading.Thread(target=http.serve_forever, daemon=True).start()
        try:
            for theme, expected in [('green', (45,53,59)), ('pink', (39,45,45))]:
                subprocess.run([str(ROOT / 'target/release/dynamite-rust'), '--apply-theme', theme], env=env, check=True)
                screenshot = root / f'{theme}.png'
                subprocess.run(['waterfox', '-no-remote', '-profile', str(profile), '--headless',
                                '--screenshot', str(screenshot), '--window-size', '640,360',
                                f'http://127.0.0.1:{http.server_port}/'], env=env, capture_output=True, timeout=35, check=True)
                with Image.open(screenshot) as image:
                    image = image.convert('RGB')
                    assert image.getpixel((350,200)) == expected
                    assert image.getpixel((60,180)) == (255,0,0)
        finally:
            http.shutdown()
    print('PASS: browser renders two theme backgrounds, preserves source image colours')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--browser', action='store_true')
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='dynamite-theme-check-') as directory:
        root = Path(directory)
        check_setup(root)
        if args.browser:
            check_browser(root)


if __name__ == '__main__':
    main()
