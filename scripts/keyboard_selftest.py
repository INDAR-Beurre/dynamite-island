#!/usr/bin/env python3
"""Real compositor key-delivery regression; uses a private headless Mango only."""
import io
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]


def wait_for(predicate, timeout=8):
    until = time.monotonic() + timeout
    while time.monotonic() < until:
        result = predicate()
        if result:
            return result
        time.sleep(.05)
    raise AssertionError("headless key-delivery check timed out")


def main():
    with tempfile.TemporaryDirectory(prefix="dynamite-keyboard-") as temp:
        directory = Path(temp)
        config = directory / "mango.conf"
        config.write_text("animations = 0\n")
        env = {k: v for k, v in os.environ.items() if k not in (
            "DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET", "MANGO_INSTANCE_SIGNATURE", "SWAYSOCK")}
        env.update(XDG_RUNTIME_DIR=temp, XDG_CONFIG_HOME=temp, HOME=temp,
                   DBUS_SESSION_BUS_ADDRESS="unix:path=/nonexistent-dynamite-keyboard-bus",
                   WLR_BACKENDS="headless", WLR_HEADLESS_OUTPUTS="1", WLR_RENDERER="pixman")
        processes = []

        def start(args, name):
            with (directory / name).open("wb") as log:
                child = subprocess.Popen(args, env=env, stdout=log, stderr=log, start_new_session=True)
            processes.append(child)
            return child

        def run(*args):
            return subprocess.run(args, env=env, check=True, capture_output=True, text=True, timeout=8)

        def send_key(key):
            run("wtype", "-k", key)
            time.sleep(.35)

        def selection_pixels():
            shot = subprocess.run(["grim", "-"], env=env, check=True,
                                  capture_output=True, timeout=8)
            with Image.open(io.BytesIO(shot.stdout)) as image:
                return image.crop((400, 90, 880, 360)).tobytes()

        try:
            start(["mango", "-c", str(config)], "mango.log")
            env["WAYLAND_DISPLAY"] = str(wait_for(lambda: next(
                (p for p in directory.glob("wayland-*") if p.is_socket()), None)))
            env["MANGO_INSTANCE_SIGNATURE"] = str(wait_for(lambda: next(directory.glob("mango-*.sock"), None)))
            # A headless compositor has no physical keyboard. Keep one virtual
            # keyboard alive before mapping so the initial focus can be sent.
            start(["wtype", "-s", "60000"], "keyboard.log")
            time.sleep(.2)
            binary = str(ROOT / "target/release/dynamite-rust")
            start([binary], "panel.log")
            wait_for(lambda: "dynamite-rust" in run("mmsg", "get", "all-layers").stdout)
            log = directory / "panel.log"
            run(binary, "music")
            time.sleep(.4)
            for key, expected in [
                ("Right", "Music -> Launcher"), ("Right", "Launcher -> ControlCenter"),
                ("Left", "ControlCenter -> Launcher"), ("Escape", "Launcher -> Collapsed")]:
                send_key(key)
                wait_for(lambda: expected in log.read_text())
            # Exercise every menu through the same actual Wayland keyboard path.
            for state in ("power", "calendar", "themes", "clipboard", "wallpaper", "controlcenter", "launcher", "music"):
                run(binary, state)
                time.sleep(.35)
                before = log.read_text().count(" -> Collapsed")
                send_key("Escape")
                wait_for(lambda: log.read_text().count(" -> Collapsed") > before)
            # Up/Down must change actual list selection, not only the key mode.
            run(binary, "clipboard")
            time.sleep(.35)
            send_key("Right")  # wrap Clipboard -> Music
            wait_for(lambda: "Clipboard -> Music" in log.read_text())
            run(binary, "launcher")
            time.sleep(.35)
            before = log.read_text().count("DBG morph")
            selected_first = selection_pixels()
            send_key("Down")
            selected_second = selection_pixels()
            assert selected_first != selected_second, "Down did not move the rendered launcher selection"
            send_key("Up")
            assert selection_pixels() == selected_first, "Up did not restore the first launcher selection"
            assert log.read_text().count("DBG morph") == before, "Up/Down should stay inside the list view"
            send_key("Escape")
            before = log.read_text()
            send_key("Right")
            send_key("Left")
            assert log.read_text().count("DBG morph") == before.count("DBG morph"), "collapsed panel retained the keyboard"
            print("PASS: Left/Right cycle, visible Up/Down selection, Escape in all eight menus, collapse releases focus")
        except BaseException:
            for name in ("mango.log", "panel.log", "keyboard.log"):
                path = directory / name
                if path.exists():
                    print(name, path.read_text(errors="replace")[-3500:])
            raise
        finally:
            for child in reversed(processes):
                if child.poll() is None:
                    os.killpg(child.pid, signal.SIGTERM)
                    try:
                        child.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        os.killpg(child.pid, signal.SIGKILL)
                        child.wait(timeout=5)


if __name__ == "__main__":
    main()
