#!/usr/bin/env python3
"""Real boot-greeter UI with fake greetd IPC and a private headless Sway.

Never connects to live greetd, PAM, camera, or the desktop compositor.
"""
import argparse
import json
import os
from pathlib import Path
import signal
import socket
import struct
import subprocess
import tempfile
import time

import gi

gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf, GLib

ROOT = Path(__file__).resolve().parents[1]


def wait_for(predicate, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = predicate()
        if result:
            return result
        time.sleep(.05)
    raise AssertionError("private greeter check timed out")


def read_exact(connection, count):
    data = bytearray()
    while len(data) < count:
        block = connection.recv(count - len(data))
        if not block:
            raise EOFError("test greeter connection closed")
        data.extend(block)
    return data


def receive(connection):
    size, = struct.unpack("@I", read_exact(connection, 4))
    assert size < 1024 * 1024
    return json.loads(read_exact(connection, size))


def reply(connection, value):
    data = json.dumps(value).encode()
    # Exercise short reads, including a split length header.
    packet = struct.pack("@I", len(data)) + data
    connection.sendall(packet[:2])
    connection.sendall(packet[2:])


def save_rgb(path, pixels):
    image = GdkPixbuf.Pixbuf.new_from_bytes(GLib.Bytes.new(pixels), GdkPixbuf.Colorspace.RGB,
                                           False, 8, 1280, 720, 1280 * 3)
    image.savev(str(path), "png", [], [])


def png_rgb(data):
    loader = GdkPixbuf.PixbufLoader.new_with_type("png")
    loader.write(data)
    loader.close()
    image = loader.get_pixbuf()
    assert (image.get_width(), image.get_height()) == (1280, 720)
    pixels, stride, channels = image.get_pixels(), image.get_rowstride(), image.get_n_channels()
    return bytes(value for y in range(720) for x in range(1280)
                 for value in pixels[y * stride + x * channels:y * stride + x * channels + 3])


def session(fixture, scenario, screenshot=None, installed=False):
    directory = fixture / scenario
    directory.mkdir(mode=0o700)
    runtime = directory / "runtime"
    runtime.mkdir(mode=0o700)
    assets = directory / "assets"
    wallpaper = assets / ".config/mango/wallpaper/wallpaper.png"
    wallpaper.parent.mkdir(parents=True)
    pixels = b"".join(bytes((30 + y // 12, 90 + y // 9, 140 + y // 15)) * 1280 for y in range(720))
    save_rgb(wallpaper, pixels)
    socket_path = directory / "greetd.sock"
    listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    listener.bind(str(socket_path))
    listener.listen(1)
    listener.settimeout(15)
    config = directory / "sway.conf"
    config.write_text("xwayland disable\noutput * mode 1280x720\noutput * bg #000000 solid_color\n")
    env = {k: v for k, v in os.environ.items() if k not in (
        "DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET", "SWAYSOCK", "MANGO_INSTANCE_SIGNATURE",
        "GREETD_SOCK", "LD_PRELOAD", "EGL_PLATFORM", "WLR_DRM_DEVICES")}
    env.update(HOME=str(directory), XDG_RUNTIME_DIR=str(runtime), XDG_CONFIG_HOME=str(directory),
               XDG_CACHE_HOME=str(directory / "cache"), WLR_BACKENDS="headless", WLR_HEADLESS_OUTPUTS="1",
               WLR_RENDERER="pixman", GSK_RENDERER="gl", LIBGL_ALWAYS_SOFTWARE="1",
               DBUS_SESSION_BUS_ADDRESS="unix:path=/nonexistent-dynamite-greeter-test-bus",
               GREETD_SOCK=str(socket_path), GREETER_USER="fixture-user", GREETER_SESSION="fixture-session",
               GREETER_ASSETS=str(assets), GREETER_RENDERER=("/usr/local/libexec/dynamite-greeter/dynamite-rust"
                   if installed else str(ROOT / "target/release/dynamite-rust")),
               LD_PRELOAD="/usr/lib/libgtk4-layer-shell.so")
    children = []

    def start(args, name):
        with (directory / name).open("wb") as log:
            child = subprocess.Popen(args, env=env, stdout=log, stderr=log, start_new_session=True)
        children.append(child)
        return child

    def key(*args):
        subprocess.run(["wtype", *args], env=env, check=True, capture_output=True, timeout=8)
        time.sleep(.15)

    def capture():
        result = subprocess.run(["grim", "-"], env=env, check=True, capture_output=True, timeout=8)
        return png_rgb(result.stdout)

    connection = None
    try:
        start(["sway", "-c", str(config)], "sway.log")
        env["WAYLAND_DISPLAY"] = str(wait_for(lambda: next(
            (p for p in runtime.glob("wayland-*") if p.is_socket()), None)))
        start(["wtype", "-s", "60000"], "keyboard.log")
        time.sleep(.2)
        app = "/usr/local/bin/face-greeter" if installed else str(ROOT / "deploy/face-greeter.py")
        greeter = start(["python3", app], "greeter.log")
        connection, _ = listener.accept()
        connection.settimeout(12)
        assert receive(connection) == {"type": "create_session", "username": "fixture-user"}
        wait_for(lambda: "shared background ready" in (directory / "greeter.log").read_text())
        background = directory / "cache/dynamite-greeter/background.png"
        time.sleep(.4)  # The idle callback queues a Wayland frame after its log line.
        expected = png_rgb(background.read_bytes())
        shot = capture()
        if screenshot:
            save_rgb(screenshot, shot)
        # Shared Rust image appears unmodified below the native password capsule.
        assert shot[100 * 1280 * 3:] == expected[100 * 1280 * 3:]
        # Empty capsule keeps the lock renderer's 52:30 proportions; hidden entry
        # sizing must not make it permanently as wide as the password widget.
        capsule = [(x, y) for y in range(15, 60) for x in range(570, 710)
                   if max(shot[(y * 1280 + x) * 3:(y * 1280 + x) * 3 + 3]) <= 5]
        assert capsule
        assert 34 <= max(x for x, _ in capsule) - min(x for x, _ in capsule) + 1 <= 38
        if scenario == "face":
            reply(connection, {"type": "auth_message", "auth_message_type": "info", "auth_message": "Face matched fixture user"})
            assert receive(connection) == {"type": "post_auth_message_response", "response": None}
            reply(connection, {"type": "success"})
        else:
            reply(connection, {"type": "auth_message", "auth_message_type": "secret", "auth_message": "Password:"})
            wait_for(lambda: "authentication input requested" in (directory / "greeter.log").read_text())
            key("-k", "Escape")
            assert receive(connection) == {"type": "cancel_session"}
            reply(connection, {"type": "success"})
            assert receive(connection) == {"type": "create_session", "username": "fixture-user"}
            reply(connection, {"type": "auth_message", "auth_message_type": "secret", "auth_message": "Password:"})
            time.sleep(.2)
            key("fixture-wrong-only", "-k", "Return")
            assert receive(connection) == {"type": "post_auth_message_response", "response": "fixture-wrong-only"}
            reply(connection, {"type": "error", "error_type": "auth_error", "description": "fixture rejection"})
            wait_for(lambda: "authentication rejected" in (directory / "greeter.log").read_text())
            assert greeter.poll() is None, "wrong password exited the greeter"
            key("-k", "Return")
            assert receive(connection) == {"type": "cancel_session"}
            reply(connection, {"type": "success"})
            assert receive(connection) == {"type": "create_session", "username": "fixture-user"}
            reply(connection, {"type": "auth_message", "auth_message_type": "secret", "auth_message": "Password:"})
            time.sleep(.2)
            key("fixture-correct-only", "-k", "Return")
            assert receive(connection) == {"type": "post_auth_message_response", "response": "fixture-correct-only"}
            reply(connection, {"type": "success"})
        assert receive(connection) == {"type": "start_session", "cmd": ["fixture-session"], "env": []}
        if scenario == "password":
            reply(connection, {"type": "error", "error_type": "error", "description": "fixture session failure"})
            wait_for(lambda: "session start rejected" in (directory / "greeter.log").read_text())
            assert greeter.poll() is None, "failed desktop start silently exited the greeter"
        else:
            reply(connection, {"type": "success"})
            assert greeter.wait(timeout=8) == 0
        logs = (directory / "greeter.log").read_text()
        assert "fixture-wrong-only" not in logs and "fixture-correct-only" not in logs
        assert "Traceback" not in logs
        print(f"PASS: {scenario} flow, shared-renderer pixels, private greetd IPC, no credential logging")
    except BaseException:
        for name in ("sway.log", "greeter.log"):
            path = directory / name
            if path.exists():
                print(name, path.read_text(errors="replace")[-5000:])
        raise
    finally:
        if connection:
            connection.close()
        listener.close()
        for child in reversed(children):
            if child.poll() is None:
                os.killpg(child.pid, signal.SIGTERM)
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL)
                    child.wait(timeout=5)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--screenshot", type=Path, help="save only the synthetic fixture greeter")
    parser.add_argument("--installed", action="store_true", help="exercise the root-installed UI and renderer")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="dynamite-greeter-") as temp:
        fixture = Path(temp)
        session(fixture, "face", args.screenshot, args.installed)
        session(fixture, "password", installed=args.installed)


if __name__ == "__main__":
    main()
