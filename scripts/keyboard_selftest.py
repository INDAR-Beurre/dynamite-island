#!/usr/bin/env python3
"""Real compositor key-delivery regression; uses a private headless Mango only."""
import io
import os
from pathlib import Path
import shutil
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
        monitor_dir = directory / "mango"
        monitor_dir.mkdir()
        monitor_config = monitor_dir / "monitor.conf"
        original_rules = "# fixture layout\nmonitor_rule=name:DP-9,x:1920,y:0,scale:0.7\n"
        monitor_config.write_text(original_rules)
        # Trace the real commands so save failures must prove live rollback.
        command_log = directory / "display-commands"
        bin_dir = directory / "bin"
        bin_dir.mkdir()
        wrapper = bin_dir / "wlr-randr"
        wrapper.write_text('#!/bin/sh\nprintf "%s\\n" "$*" >> "$XDG_RUNTIME_DIR/display-commands"\n'
                           f'exec {shutil.which("wlr-randr")} "$@"\n')
        wrapper.chmod(0o755)
        env = {k: v for k, v in os.environ.items() if k not in (
            "DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET", "MANGO_INSTANCE_SIGNATURE", "SWAYSOCK")}
        env.update(XDG_RUNTIME_DIR=temp, XDG_CONFIG_HOME=temp, HOME=temp,
                   DBUS_SESSION_BUS_ADDRESS="unix:path=/nonexistent-dynamite-keyboard-bus",
                   WLR_BACKENDS="headless", WLR_HEADLESS_OUTPUTS="1", WLR_RENDERER="pixman",
                   PATH=str(bin_dir) + os.pathsep + os.environ.get("PATH", ""))
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
                ("Right", "ControlCenter -> Displays"), ("Left", "Displays -> ControlCenter"),
                ("Left", "ControlCenter -> Launcher"), ("Escape", "Launcher -> Collapsed")]:
                send_key(key)
                wait_for(lambda: expected in log.read_text())
            # Exercise every menu through the same actual Wayland keyboard path.
            for state in ("power", "calendar", "themes", "clipboard", "wallpaper", "controlcenter", "displays", "launcher", "music"):
                run(binary, state)
                time.sleep(.35)
                before = log.read_text().count(" -> Collapsed")
                send_key("Escape")
                wait_for(lambda: log.read_text().count(" -> Collapsed") > before)
            # The menu must change a real output and persist only its scale.
            run(binary, "displays")
            time.sleep(1.2)
            output = run("wlr-randr").stdout.split()[0]

            def scale():
                return float(next(line.split(":", 1)[1] for line in run("wlr-randr").stdout.splitlines()
                                  if line.strip().startswith("Scale:")))

            run("grim", "/tmp/dynamite-displays-headless.png")
            assert scale() == 1.0
            send_key("Up")
            send_key("Return")
            wait_for(lambda: "scale:1.05" in monitor_config.read_text())
            assert abs(scale() - 1.05) < .005, "Apply did not change the real headless output"
            assert monitor_config.read_text().startswith(original_rules), "Apply changed another output's layout"
            time.sleep(1.2)
            send_key("Down")
            send_key("Return")
            wait_for(lambda: f"name:{output},scale:1.00" in monitor_config.read_text())
            assert scale() == 1.0
            time.sleep(1.2)
            # A non-writable save target must restore the original live scale.
            saved = monitor_dir / "saved.conf"
            monitor_config.rename(saved)
            monitor_config.symlink_to(saved)
            before_commands = command_log.read_text().count(f"--output {output} --scale 1.00")
            send_key("Up")
            send_key("Return")
            wait_for(lambda: command_log.read_text().count(f"--output {output} --scale 1.00") > before_commands)
            wait_for(lambda: scale() == 1.0)
            assert monitor_config.is_symlink() and "scale:1.05" not in saved.read_text(), "failed save altered the config"
            send_key("Escape")
            # Up/Down must change actual list selection, not only the key mode.
            run(binary, "clipboard")
            time.sleep(.35)
            send_key("Right")  # wrap Clipboard -> Music
            wait_for(lambda: "Clipboard -> Music" in log.read_text())
            run(binary, "launcher")
            time.sleep(.65)  # Let the 560 ms spring settle before comparing selection pixels.
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
            print("PASS: nine-menu navigation and Escape, real display apply/save/rollback, visible list selection, focus release")
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
