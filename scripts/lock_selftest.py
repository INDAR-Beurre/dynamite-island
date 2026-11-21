#!/usr/bin/env python3
"""Test real Hyprlock only in a private compositor and private PAM mount.

Deny/permit modules model face failure/success without camera access. The real
Howdy module and its password fallback are checked by howdy_module_selftest.py
in a separate root-owned mount namespace (Howdy validates root-owned ancestors).
Needs user/mount namespaces, Mango, wtype, Hyprlock, and software EGL.
"""
import importlib.util
import json
import os
from pathlib import Path
import pwd
import signal
import socket
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("locker", ROOT / "deploy/dynamite-lock.py")
locker = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(locker)


def wait_for(predicate, timeout=10):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if result := predicate():
            return result
        time.sleep(.05)
    raise AssertionError("private lock test timed out")


def isolated_test():
    parent_mount = os.environ.get("DYNAMITE_PARENT_MOUNT_NAMESPACE")
    uid_map = Path("/proc/self/uid_map").read_text().split()
    if (os.getuid() != 0 or len(uid_map) != 3 or uid_map[0] != "0" or uid_map[1] == "0"
            or uid_map[2] != "1" or not parent_mount or parent_mount == os.readlink("/proc/self/ns/mnt")):
        raise RuntimeError("must run in this test's private user/mount namespace")
    with tempfile.TemporaryDirectory(prefix="dynamite-lock-test-") as temp:
        directory = Path(temp)
        pam_dir = directory / "pam.d"
        pam_dir.mkdir()
        checker = directory / "check-password.py"
        checker.write_text("import json, os, sys\nfrom pathlib import Path\n"
                           f"Path({str(directory / 'auth-attempt')!r}).write_text(json.dumps({{k: os.environ.get(k) for k in ('PAM_SERVICE', 'PAM_USER')}}))\n"
                           "sys.exit(0 if sys.stdin.buffer.read(512).rstrip(b'\\0') == b'headless-test-only' else 1)\n")
        (pam_dir / "dynamite-lock").write_bytes(locker.PAM_SOURCE.read_bytes())
        (pam_dir / "login").write_text(
            f"auth required pam_exec.so expose_authtok /usr/bin/python3 {checker}\n"
            "account required pam_permit.so\n")
        subprocess.run(["mount", "--bind", str(pam_dir), "/etc/pam.d"], check=True)
        # This namespace maps only the desktop user to root; host-root path
        # owners are unmapped. Test real Howdy separately without weakening its
        # ownership policy, and keep this real-Wayland fixture camera-free.
        subprocess.run(["mount", "--bind", "/usr/lib/security/pam_deny.so",
                        "/usr/lib/security/pam_howdy.so"], check=True)
        assert os.path.samefile("/usr/lib/security/pam_deny.so", "/usr/lib/security/pam_howdy.so")
        test_user = pwd.getpwuid(os.getuid()).pw_name
        config = directory / "mango.conf"
        config.write_text("animations = 0\n")
        env = {k: v for k, v in os.environ.items() if k not in (
            "DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET", "MANGO_INSTANCE_SIGNATURE", "SWAYSOCK")}
        env.update(XDG_RUNTIME_DIR=temp, XDG_CONFIG_HOME=temp, HOME=temp,
                   DBUS_SESSION_BUS_ADDRESS="unix:path=/nonexistent-dynamite-test-bus",
                   WLR_BACKENDS="headless", WLR_HEADLESS_OUTPUTS="1", WLR_RENDERER="gles2",
                   WLR_RENDERER_ALLOW_SOFTWARE="1", LIBGL_ALWAYS_SOFTWARE="1")
        processes = []

        def start(args, logname):
            with (directory / logname).open("wb") as log:
                child = subprocess.Popen(args, env=env, stdout=log, stderr=log, start_new_session=True)
            processes.append(child)
            return child

        def run(*args):
            return subprocess.run(args, env=env, check=True, capture_output=True, text=True, timeout=15)

        try:
            mango = start(["mango", "-c", str(config)], "mango.log")
            env["WAYLAND_DISPLAY"] = str(wait_for(lambda: next(
                (p for p in directory.glob("wayland-*") if p.is_socket()), None)))
            env["MANGO_INSTANCE_SIGNATURE"] = str(wait_for(lambda: next(directory.glob("mango-*.sock"), None)))
            start(["wtype", "-s", "60000"], "keyboard.log")
            outputs = json.loads(run("mmsg", "get", "all-monitors").stdout)["monitors"]
            start([str(locker.BINARY)], "panel.log")
            preview_socket = directory / "dynamite-rust.sock"
            wait_for(preview_socket.is_socket)
            panel_log = directory / "panel.log"
            run(str(locker.BINARY), "lock-preview")
            wait_for(lambda: "DBG morph Collapsed -> Lockscreen" in panel_log.read_text(errors="replace"))
            run(str(locker.BINARY), "unlock")
            wait_for(lambda: "DBG morph Lockscreen -> Collapsed" in panel_log.read_text(errors="replace"))
            print("PASS: explicit lock-preview stays a reversible renderer test")
            # Check offline rendering separately; the CLI below runs the full
            # production checks against the namespace-only PAM/module fixtures.
            config_path, images = locker.prepare(directory / "rendered", outputs)
            assert images[0].read_bytes().startswith(b'\x89PNG\r\n\x1a\n')
            try:
                locker.prepare(config_path.parent, outputs)
            except FileExistsError:
                pass
            else:
                raise AssertionError("preparing again would replace existing files")
            failed_export = subprocess.run(
                [str(locker.BINARY), "--render-lock-background", str(images[0]), "8192", "8192"],
                env=env, capture_output=True, timeout=5)
            assert failed_export.returncode == 2 and b"at most 16 megapixels" in failed_export.stderr
            assert images[0].read_bytes().startswith(b'\x89PNG\r\n\x1a\n')
            child = start([str(locker.BINARY), "lockscreen"], "hyprlock.log")
            log = directory / "hyprlock.log"
            wait_for(lambda: "onLockLocked called" in log.read_text(errors="replace") or child.poll() is not None)
            assert child.poll() is None, "Hyprlock failed before receiving input"
            assert "onLockLocked called" in log.read_text(errors="replace"), "no real session-lock acknowledgement"
            assert "Config has errors" not in log.read_text(errors="replace"), "generated lock config is invalid"
            child.send_signal(signal.SIGTERM)
            with socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM) as preview:
                preview.sendto(b"unlock", str(preview_socket))
            run("wtype", "-k", "Escape")
            time.sleep(.4)
            assert child.poll() is None, "launcher shutdown, preview socket or Escape dismissed a real lock"
            assert next(directory.glob("dynamite-lock-*/rendered/background-0.png"), None), "launcher removed lock assets while Hyprlock was active"
            run("wtype", "wrong-test-password", "-k", "Return")
            wait_for(lambda: (directory / "auth-attempt").exists())
            attempt = json.loads((directory / "auth-attempt").read_text())
            assert attempt == {"PAM_SERVICE": "dynamite-lock", "PAM_USER": test_user}, attempt
            time.sleep(1)
            assert child.poll() is None, "incorrect password unlocked the session"
            assert "failed" in log.read_text(errors="replace").lower(), "failure not reported"
            run("wtype", "headless-test-only", "-k", "Return")
            child.wait(timeout=10)
            assert child.returncode == 0, "correct fixture password did not unlock cleanly"
            assert not list(directory.glob("dynamite-lock-*/rendered")), "launcher left temporary assets behind"
            assert mango.poll() is None, "unlock terminated the private compositor"
            print("PASS: real lock and face-failure fallback share the PAM user; Escape/preview unlock/wrong password rejected; correct fixture accepted")
            # Model a successful face module: it must short-circuit the
            # password fallback without a password or real camera access.
            (pam_dir / "login").write_text("auth required pam_deny.so\n")
            subprocess.run(["mount", "--bind", "/usr/lib/security/pam_permit.so",
                            "/usr/lib/security/pam_howdy.so"], check=True)
            assert os.path.samefile("/usr/lib/security/pam_permit.so", "/usr/lib/security/pam_howdy.so")
            face = start([str(locker.BINARY), "lock"], "face.log")
            face.wait(timeout=10)
            assert face.returncode == 0, "successful face fixture did not unlock"
            assert "Unlocked, exiting!" in (directory / "face.log").read_text(errors="replace")
            print("PASS: lock CLI uses the successful face fixture without a password; no real biometric check was attempted")
        except BaseException:
            for name in ("mango.log", "hyprlock.log", "face.log", "keyboard.log"):
                path = directory / name
                if path.exists():
                    print(name, path.read_text(errors="replace")[-6500:])
            raise
        finally:
            # On failure, end the private compositor before stopping a wrapper
            # that intentionally waits for its separate locker process.
            for child in processes[:1] + processes[:0:-1]:
                if child.poll() is None:
                    os.killpg(child.pid, signal.SIGTERM)
                    try:
                        child.wait(timeout=3)
                    except subprocess.TimeoutExpired:
                        os.killpg(child.pid, signal.SIGKILL)
                        child.wait(timeout=3)


if __name__ == "__main__":
    if sys.argv[1:] == ["--namespace"]:
        isolated_test()
    elif not sys.argv[1:]:
        if os.getuid() == 0:
            raise SystemExit("run the private lock test as an unprivileged user")
        env = dict(os.environ, DYNAMITE_PARENT_MOUNT_NAMESPACE=os.readlink("/proc/self/ns/mnt"))
        subprocess.run(["unshare", "-Ur", "-m", "--propagation", "private", sys.executable,
                        str(Path(__file__).resolve()), "--namespace"], env=env, check=True)
    else:
        raise SystemExit("usage: python3 scripts/lock_selftest.py")
