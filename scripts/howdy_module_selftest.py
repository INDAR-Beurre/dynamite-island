#!/usr/bin/env python3
"""Exercise the real Howdy PAM module in private, root-owned mount fixtures.

Requires passwordless sudo for unshare/mount. Never uses the live Howdy config,
models, camera, PAM stack, or account password. No login/lock client is started.
"""
import argparse
import ctypes as c
import json
import os
from pathlib import Path
import pwd
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
MODULE = Path('/usr/lib/security/pam_howdy.so')
HOOKS = ('authenticate', 'acct_mgmt', 'chauthtok', 'close_session', 'open_session', 'setcred')


def isolated_test(args):
    if os.geteuid() != 0 or not args.parent_mount or args.parent_mount == os.readlink('/proc/self/ns/mnt'):
        raise RuntimeError('refusing PAM mounts outside a new root-owned mount namespace')
    subprocess.run(['mount', '--make-rprivate', '/'], check=True)
    with tempfile.TemporaryDirectory(prefix='dynamite-howdy-module-') as temp:
        directory = Path(temp)
        pam_dir, howdy_dir = directory / 'pam.d', directory / 'howdy'
        pam_dir.mkdir()
        howdy_dir.mkdir(mode=0o755)
        (howdy_dir / 'models').mkdir(mode=0o700)
        (howdy_dir / 'config.ini').write_text('[core]\ndisabled = true\n')
        (howdy_dir / 'config.ini').chmod(0o644)
        checker = directory / 'password-fixture.py'
        record = directory / 'attempt.json'
        checker.write_text(
            'import json, os, sys\nfrom pathlib import Path\n'
            f'Path({str(record)!r}).write_text(json.dumps({{k: os.environ.get(k) for k in ("PAM_SERVICE", "PAM_USER")}}))\n'
            'sys.exit(0 if sys.stdin.buffer.read(512).rstrip(b"\\0") == b"fixture-only" else 1)\n')
        (pam_dir / 'dynamite-lock').write_bytes((ROOT / 'deploy/dynamite-lock.pam').read_bytes())
        (pam_dir / 'login').write_text(
            f'auth required pam_exec.so expose_authtok /usr/bin/python3 {checker}\n'
            'account required pam_permit.so\n')
        for source, target in ((pam_dir, '/etc/pam.d'), (howdy_dir, '/etc/howdy'),
                               ('/usr/bin/false', '/usr/lib/howdy/howdy-auth-helper'),
                               ('/usr/bin/false', '/usr/lib/howdy/howdy-compare')):
            subprocess.run(['mount', '--bind', str(source), target], check=True)
            assert os.path.samefile(source, target)
        if args.module:
            subprocess.run(['mount', '--bind', str(Path(args.module).resolve(strict=True)), str(MODULE)], check=True)
        module = c.CDLL(str(MODULE))
        for hook in HOOKS:
            getattr(module, 'pam_sm_' + hook)

        class Message(c.Structure):
            _fields_ = [('style', c.c_int), ('message', c.c_char_p)]
        class Response(c.Structure):
            _fields_ = [('text', c.c_void_p), ('code', c.c_int)]
        callback_type = c.CFUNCTYPE(c.c_int, c.c_int, c.POINTER(c.POINTER(Message)),
                                   c.POINTER(c.POINTER(Response)), c.c_void_p)
        class Conversation(c.Structure):
            _fields_ = [('callback', callback_type), ('data', c.c_void_p)]
        libc, pam = c.CDLL(None), c.CDLL('libpam.so.0')
        libc.calloc.argtypes, libc.calloc.restype = [c.c_size_t, c.c_size_t], c.c_void_p
        libc.strdup.argtypes, libc.strdup.restype = [c.c_char_p], c.c_void_p
        pam.pam_start.argtypes = [c.c_char_p, c.c_char_p, c.POINTER(Conversation), c.POINTER(c.c_void_p)]
        pam.pam_end.argtypes = [c.c_void_p, c.c_int]
        pam.pam_authenticate.argtypes = [c.c_void_p, c.c_int]
        module.pam_sm_authenticate.argtypes = [c.c_void_p, c.c_int, c.c_int, c.c_void_p]

        def authenticate(password=None, direct=False):
            prompts = []
            def converse(count, messages, responses, _):
                if password is None:
                    prompts.extend(messages[i].contents.style for i in range(count))
                    return 19  # PAM_CONV_ERR; disabled Howdy must not prompt.
                memory = libc.calloc(count, c.sizeof(Response))
                if not memory:
                    return 5  # PAM_BUF_ERR.
                result = c.cast(memory, c.POINTER(Response))
                for i in range(count):
                    if messages[i].contents.style in (1, 2):
                        result[i].text = libc.strdup(password)
                responses[0] = result
                return 0
            callback = callback_type(converse)
            conversation = Conversation(callback, None)
            handle = c.c_void_p()
            assert pam.pam_start(b'dynamite-lock', args.test_user.encode(), c.byref(conversation), c.byref(handle)) == 0
            try:
                status = (module.pam_sm_authenticate(handle, 0, 0, None) if direct
                          else pam.pam_authenticate(handle, 0))
                assert not prompts, 'disabled Howdy unexpectedly requested credentials'
                return status
            finally:
                pam.pam_end(handle, 0)

        assert authenticate(direct=True) == 9, 'disabled real module must return PAM_AUTHINFO_UNAVAIL (9)'
        print('PASS: all six PAM entry points exist; real Howdy honors disabled private config', flush=True)
        assert authenticate(b'wrong-fixture') != 0, 'incorrect fixture password accepted'
        assert authenticate(b'fixture-only') == 0, 'real-module fallback rejected correct fixture password'
        assert json.loads(record.read_text()) == {'PAM_SERVICE': 'dynamite-lock', 'PAM_USER': args.test_user}
        print('PASS: production PAM service falls through real Howdy to the same user/password fixture; camera helpers blocked', flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--module', type=Path, help='test an uninstalled candidate instead of the installed module')
    parser.add_argument('--namespace', action='store_true', help=argparse.SUPPRESS)
    parser.add_argument('--parent-mount', help=argparse.SUPPRESS)
    parser.add_argument('--test-user', help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.namespace:
        isolated_test(args)
    else:
        if os.geteuid() == 0:
            parser.error('start as the desktop user; this command creates its own private mount namespace')
        command = ['sudo', '-n', 'unshare', '--mount', '--propagation', 'private', sys.executable,
                   str(Path(__file__).resolve()), '--namespace', '--parent-mount', os.readlink('/proc/self/ns/mnt'),
                   '--test-user', pwd.getpwuid(os.getuid()).pw_name]
        if args.module:
            command += ['--module', str(args.module.resolve(strict=True))]
        subprocess.run(command, check=True, timeout=30)
