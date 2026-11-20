#!/usr/bin/env python3
"""Exercise the installer only under temporary HOME/XDG directories."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
INSTALL = ROOT / 'deploy/install.sh'


def snapshot(root):
    return {str(p.relative_to(root)): (p.read_bytes(), p.stat().st_mode & 0o777)
            for p in root.rglob('*') if p.is_file()}


def main():
    with tempfile.TemporaryDirectory(prefix='dynamite-install-check-') as directory:
        home = Path(directory)
        config = home / '.config'
        env = dict(os.environ, HOME=str(home), XDG_CONFIG_HOME=str(config))
        cfg = config / 'mango'
        binds = cfg / 'dynamite-bind.conf'
        wrapper = cfg / 'scripts/dynamite_autostart.sh'

        def run(*args, code=0):
            p = subprocess.run([str(INSTALL), *args], env=env, capture_output=True, text=True, timeout=10)
            assert p.returncode == code, (args, p.returncode, p.stdout, p.stderr)

        run('--check')
        run('--help')
        run('--dry-run', code=2)
        run('--check', 'extra', code=2)
        assert not config.exists(), 'reporting and invalid options must not write files'
        run()
        assert f'source = {binds}' in (cfg / 'bind.conf').read_text()
        assert os.access(wrapper, os.X_OK)
        assert wrapper.read_text().startswith('#!/bin/sh\n')
        first = snapshot(home)
        run()
        assert snapshot(home) == first, 'repeat install changed files'

        # Preserve a custom supervisor and user bindings exactly.
        wrapper.write_text('#!/bin/sh\n# custom supervisor\n')
        wrapper.chmod(0o700)
        binds.write_text('# custom bindings\n')
        custom = snapshot(home)
        run()
        assert snapshot(home) == custom, 'customized generated files were overwritten'

        # Migrate only our old invalid line, including the documented ~/ form.
        existing = '# keep this\nbind=SUPER,z,spawn,true\n'
        (cfg / 'bind.conf').write_text(existing + 'source ~/.config/mango/dynamite-bind.conf\n')
        run('--check')
        assert (cfg / 'bind.conf').read_text().endswith('source ~/.config/mango/dynamite-bind.conf\n')
        run()
        assert (cfg / 'bind.conf').read_text() == existing + f'source = {binds}\n'
        with (cfg / 'bind.conf').open('a') as f:
            f.write(f'source {binds}\n')
        run()
        assert (cfg / 'bind.conf').read_text() == existing + f'source = {binds}\n'
        (cfg / 'bind.conf').write_text(existing + 'source=~/.config/mango/dynamite-bind.conf\n')
        before = (cfg / 'bind.conf').read_bytes()
        run()
        assert (cfg / 'bind.conf').read_bytes() == before
        print('PASS: installer dry-run, argument validation, idempotence, preservation and legacy migration')

        if shutil.which('mango'):
            p = subprocess.run(['mango', '-p', '-c', str(cfg / 'bind.conf')], env=env,
                               capture_output=True, text=True, timeout=10)
            diagnostics = p.stdout + p.stderr
            assert p.returncode == 0, diagnostics
            assert 'Invalid line format' not in diagnostics and 'Unknown keyword' not in diagnostics, diagnostics
            print('PASS: installed include and bindings parse in MangoWM')


if __name__ == '__main__':
    main()
