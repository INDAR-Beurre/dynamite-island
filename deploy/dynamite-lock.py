#!/usr/bin/env python3
"""Dynamite background with Hyprlock's real session lock and PAM input.

--check and --prepare never lock or authenticate. --lock is deliberately explicit.
"""
import argparse
import fcntl
import json
import os
from pathlib import Path
import shlex
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / "target/release/dynamite-rust"
PAM = Path("/etc/pam.d/dynamite-lock")
PAM_SOURCE = ROOT / "deploy/dynamite-lock.pam"


def check():
    if not BINARY.is_file() or not os.access(BINARY, os.X_OK):
        raise RuntimeError("build the panel first: cargo build --release --locked")
    if not shutil.which("hyprlock") or not shutil.which("mmsg"):
        raise RuntimeError("hyprlock and MangoWM's mmsg are required")
    if PAM.is_symlink() or not PAM.is_file():
        raise RuntimeError(f"install the dedicated PAM service: sudo install -o root -g root -m 644 {PAM_SOURCE} {PAM}")
    mode = PAM.stat()
    if mode.st_uid != 0 or mode.st_mode & (stat.S_IWGRP | stat.S_IWOTH):
        raise RuntimeError("the dedicated PAM service must be root-owned and not writable by other users")
    if PAM.read_bytes() != PAM_SOURCE.read_bytes():
        raise RuntimeError("the dedicated PAM service differs from deploy/dynamite-lock.pam; review it before locking")
    if not Path("/usr/lib/security/pam_howdy.so").is_file():
        raise RuntimeError("Howdy's PAM module is missing; install/configure Howdy first")


def monitors():
    result = subprocess.run(["mmsg", "get", "all-monitors"], check=True,
                            capture_output=True, text=True, timeout=5)
    outputs = json.loads(result.stdout)["monitors"]
    outputs = [m for m in outputs if m.get("active") and m["width"] > 0 and m["height"] > 0]
    if not outputs:
        raise RuntimeError("no awake output; wake the monitor before locking")
    return outputs


def config_value(value):
    value = str(value)
    if any(c in value for c in "\n\r#$;{}"):
        raise ValueError("unsupported character in lock configuration path or output name")
    return value


def configuration(outputs, images):
    sections = ["""general {
    hide_cursor = true
    ignore_empty_input = false
    immediate_render = true
    disable_screencopy = true
    fractional_scaling = 2
}
auth {
    pam {
        enabled = true
        module = dynamite-lock
    }
    fingerprint {
        enabled = false
    }
}
animations {
    enabled = false
}
"""]
    for output, image in zip(outputs, images):
        image = config_value(image)
        sections.append(f"""background {{
    monitor = {config_value(output['name'])}
    path = {image}
    reload_time = 1
    reload_cmd = printf '%s' {shlex.quote(image)}
    blur_passes = 0
    noise = 0
    brightness = 1
    contrast = 1
    vibrancy = 0
    crossfade_time = 0
}}
""")
    # Empty monitor matches every output, including a newly connected one.
    h = outputs[0]["height"]
    scale = h / 1080
    sections.append(f"""input-field {{
    monitor =
    size = {52 * scale:.2f}, {30 * scale:.2f}
    position = 0, {-42 * scale:.2f}
    halign = center
    valign = top
    outline_thickness = 0
    rounding = -1
    inner_color = rgba(000000ff)
    outer_color = rgba(000000ff)
    font_color = rgba(ffffffff)
    font_family = Sans
    dots_size = 0.2233
    dots_spacing = 0.79
    dots_rounding = -1
    fade_on_empty = false
    placeholder_text = <span font_family="Material Symbols Rounded" size="{max(1, round(16 * scale))}px" foreground="white">&#xe897;</span>
    swap_font_color = false
    check_color = rgba(000000ff)
    check_text =
    fail_color = rgba(000000ff)
    fail_text = Try again
}}
""")
    return "\n".join(sections)


def render(outputs, images):
    for output, image in zip(outputs, images):
        width = round(output["width"] * output["scale"])
        height = round(output["height"] * output["scale"])
        subprocess.run([str(BINARY), "--render-lock-background", str(image),
                        str(width), str(height)], check=True, timeout=45)


def prepare(directory, outputs):
    images = [directory / f"background-{i}.png" for i in range(len(outputs))]
    settings = configuration(outputs, images)
    directory.mkdir(parents=True, mode=0o700)
    render(outputs, images)
    config = directory / "hyprlock.conf"
    config.write_text(settings)
    return config, images


def run_lock(outputs):
    interrupted = False

    def defer_shutdown(_signum, _frame):
        nonlocal interrupted
        interrupted = True

    for signum in (signal.SIGHUP, signal.SIGINT, signal.SIGTERM):
        signal.signal(signum, defer_shutdown)

    runtime = Path(os.environ["XDG_RUNTIME_DIR"])
    with (runtime / "dynamite-lock.lock").open("a") as guard:
        try:
            fcntl.flock(guard, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise RuntimeError("Dynamite's locker is already running") from None
        with tempfile.TemporaryDirectory(prefix="dynamite-lock-", dir=runtime) as temp:
            config, images = prepare(Path(temp) / "rendered", outputs)
            if interrupted:
                return 1
            child = subprocess.Popen(["hyprlock", "--config", str(config), "--grace", "0"], start_new_session=True)
            # Never forward shutdown to a session-lock client. Keep its files
            # and the instance guard until Hyprlock completes authentication.
            minute = time.strftime("%Y-%m-%d %H:%M")
            try:
                while not interrupted:
                    try:
                        return child.wait(timeout=1)
                    except subprocess.TimeoutExpired:
                        pass
                    now = time.strftime("%Y-%m-%d %H:%M")
                    if now != minute and not interrupted:
                        minute = now
                        try:
                            render(outputs, images)
                        except (OSError, subprocess.SubprocessError) as error:
                            print(f"dynamite-lock: background refresh failed: {error}", file=sys.stderr)
            finally:
                child.wait()
            return child.returncode


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true", help="check dependencies only; no authentication")
    mode.add_argument("--prepare", type=Path, metavar="NEW_DIRECTORY", help="render/configure only; never lock")
    mode.add_argument("--lock", action="store_true", help="lock this session; requires face or password authentication")
    args = parser.parse_args()
    check()
    if args.check:
        print("Dynamite lock dependencies ready. Face and password still require an attended test.")
        return 0
    outputs = monitors()
    if args.prepare:
        config, _ = prepare(args.prepare.resolve(), outputs)
        print(config)
        return 0
    return run_lock(outputs)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"dynamite-lock: {error}", file=sys.stderr)
        sys.exit(1)
