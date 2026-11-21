#!/usr/bin/env python3
"""Boot login using the shared Dynamite background and greetd's existing PAM stack."""
import datetime
import json
import os
from pathlib import Path
import shlex
import socket
import struct
import subprocess
import sys
import threading

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Gdk", "4.0")
from gi.repository import Gdk, GLib, Gtk

try:
    gi.require_version("Gtk4LayerShell", "1.0")
    from gi.repository import Gtk4LayerShell as LayerShell
except (ImportError, ValueError):
    LayerShell = None

USER = os.environ.get("GREETER_USER", "alex")
SESSION_CMD = shlex.split(os.environ.get("GREETER_SESSION", "mango"))
ASSETS = Path(os.environ.get("GREETER_ASSETS", "/usr/local/share/dynamite-greeter"))
RENDERER = Path(os.environ.get("GREETER_RENDERER", "/usr/local/libexec/dynamite-greeter/dynamite-rust"))
CACHE = Path(os.environ.get("XDG_CACHE_HOME", str(Path.home() / ".cache"))) / "dynamite-greeter"


def log(message):
    # Never log PAM responses or password input.
    print("face-greeter:", message, flush=True)


class Greetd:
    def __init__(self, path):
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.connect(path)
        self.lock = threading.Lock()

    def _read(self, size):
        data = bytearray()
        while len(data) < size:
            chunk = self.sock.recv(size - len(data))
            if not chunk:
                raise EOFError("greetd closed the connection")
            data.extend(chunk)
        return data

    def exchange(self, payload):
        with self.lock:
            data = json.dumps(payload).encode()
            self.sock.sendall(struct.pack("@I", len(data)) + data)
            size, = struct.unpack("@I", self._read(4))
            if size > 1024 * 1024:
                raise ValueError("greetd response exceeds 1 MiB")
            return json.loads(self._read(size))


class Greeter(Gtk.Application):
    def __init__(self):
        super().__init__(application_id="org.local.facegreeter")
        self.client = None
        self.busy = False
        self.authenticated = False
        self.retry = False
        self.rendering = False
        self.minute = None

    def do_activate(self):
        if self.get_active_window():
            self.get_active_window().present()
            return
        CACHE.mkdir(parents=True, exist_ok=True, mode=0o700)
        monitor = Gdk.Display.get_default().get_monitors().get_item(0)
        rect = monitor.get_geometry()
        scale = monitor.get_scale_factor()
        self.dimensions = rect.width * scale, rect.height * scale
        unit = rect.height / 1080
        provider = Gtk.CssProvider()
        provider.load_from_data(f"""
window {{ background: black; }}
.capsule {{ background: black; border-radius: 999px; padding: 0; }}
.lock {{ color: white; font-family: 'Material Symbols Rounded'; font-size: {16 * unit}px; }}
entry {{ background: transparent; background-image: none; border: none; box-shadow: none;
  border-radius: 999px; color: white; caret-color: white; padding: 0 5px; min-width: 0; min-height: 0;
  font-size: {14 * unit}px; }}
.status {{ color: #e8e8e8; font: 12px sans-serif; text-shadow: 0 1px 3px black; }}
""".encode())
        Gtk.StyleContext.add_provider_for_display(Gdk.Display.get_default(), provider,
                                                   Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
        win = Gtk.ApplicationWindow(application=self)
        win.set_title("Login")
        win.set_decorated(False)
        win.set_default_size(rect.width, rect.height)
        overlay = Gtk.Overlay()
        win.set_child(overlay)
        self.picture = Gtk.Picture.new_for_filename(str(ASSETS / ".config/mango/wallpaper/wallpaper.png"))
        self.picture.set_content_fit(Gtk.ContentFit.COVER)
        self.picture.set_can_shrink(True)
        self.picture.set_hexpand(True)
        self.picture.set_vexpand(True)
        overlay.set_child(self.picture)

        column = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        column.set_halign(Gtk.Align.CENTER)
        column.set_valign(Gtk.Align.START)
        column.set_margin_top(round(42 * unit))
        overlay.add_overlay(column)
        self.stack = Gtk.Stack()
        self.stack.set_hhomogeneous(False)
        self.stack.set_vhomogeneous(False)
        self.stack.add_css_class("capsule")
        self.stack.set_halign(Gtk.Align.CENTER)
        self.stack.set_size_request(round(52 * unit), round(30 * unit))
        self.symbol = Gtk.Image.new_from_icon_name("system-lock-screen-symbolic")
        self.symbol.set_pixel_size(max(1, round(16 * unit)))
        self.symbol.add_css_class("lock")
        self.stack.add_named(self.symbol, "lock")
        self.entry = Gtk.Entry()
        self.entry.set_visibility(False)
        self.entry.set_width_chars(3)
        self.entry.set_max_width_chars(12)
        self.entry.set_input_purpose(Gtk.InputPurpose.PASSWORD)
        self.entry.connect("activate", self._on_submit)
        self.stack.add_named(self.entry, "entry")
        self.stack.set_visible_child_name("lock")
        column.append(self.stack)
        self.status = Gtk.Label()
        self.status.add_css_class("status")
        self.status.set_wrap(True)
        self.status.set_max_width_chars(64)
        column.append(self.status)

        if LayerShell is not None:
            LayerShell.init_for_window(win)
            LayerShell.set_monitor(win, monitor)
            LayerShell.set_layer(win, LayerShell.Layer.OVERLAY)
            for edge in (LayerShell.Edge.TOP, LayerShell.Edge.BOTTOM, LayerShell.Edge.LEFT, LayerShell.Edge.RIGHT):
                LayerShell.set_anchor(win, edge, True)
            LayerShell.set_exclusive_zone(win, -1)
            LayerShell.set_keyboard_mode(win, LayerShell.KeyboardMode.EXCLUSIVE)
        else:
            win.fullscreen_on_monitor(monitor)
        controller = Gtk.EventControllerKey()
        controller.connect("key-pressed", self._on_key)
        win.add_controller(controller)
        win.present()
        self._tick_clock()
        GLib.timeout_add_seconds(1, self._tick_clock)
        GLib.idle_add(self._connect)
        log("UI ready")

    def _tick_clock(self):
        minute = datetime.datetime.now().strftime("%Y-%m-%d %H:%M")
        if not self.rendering and self.minute != minute:
            self.minute = minute  # Failed renders retry next minute, not every tick.
            self.rendering = True
            threading.Thread(target=self._render, daemon=True).start()
        return True

    def _render(self):
        image = CACHE / "background.png"
        env = dict(os.environ, HOME=str(ASSETS), XDG_CONFIG_HOME=str(ASSETS / ".config"))
        try:
            subprocess.run([str(RENDERER), "--render-lock-background", str(image),
                            *(str(v) for v in self.dimensions)], env=env, check=True,
                           stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, timeout=45)
            GLib.idle_add(self._rendered, str(image))
        except (OSError, subprocess.SubprocessError) as error:
            log(f"background unavailable: {type(error).__name__}")
            GLib.idle_add(self._rendered, None)

    def _rendered(self, image):
        self.rendering = False
        if image:
            self.picture.set_filename(image)
            log("shared background ready")
        return False

    def _connect(self):
        try:
            self.client = Greetd(os.environ["GREETD_SOCK"])
        except (OSError, KeyError) as error:
            self.status.set_text("Login service unavailable. Switch to another console to sign in.")
            log(f"greetd connection failed: {type(error).__name__}")
            return False
        self._request()
        return False

    def _request(self):
        if self.busy or self.authenticated or self.client is None:
            return
        self.busy = True
        self.entry.set_text("")
        self.stack.set_visible_child_name("lock")
        self.status.set_text("")
        if self.retry:
            self._send({"type": "cancel_session"}, self._cancelled)
        else:
            self._send({"type": "create_session", "username": USER})

    def _cancelled(self, response):
        if response.get("type") != "success":
            return self._on_ipc_error("could not reset authentication")
        self.retry = False
        self._send({"type": "create_session", "username": USER})
        return False

    def _send(self, payload, callback=None):
        def worker():
            try:
                response = self.client.exchange(payload)
            except (OSError, EOFError, ValueError) as error:
                GLib.idle_add(self._on_ipc_error, type(error).__name__)
                return
            GLib.idle_add(callback or self._on_response, response)
        threading.Thread(target=worker, daemon=True).start()

    def _on_ipc_error(self, message):
        self.busy = False
        self.client = None
        self.status.set_text("Login service connection lost. Switch to another console to sign in.")
        log(f"greetd IPC error: {message}")
        return False

    def _on_response(self, response):
        kind = response.get("type")
        if kind == "success":
            self.authenticated = True
            self.entry.set_text("")
            self.stack.set_visible_child_name("lock")
            self.status.set_text("")
            log("authentication accepted")
            self._send({"type": "start_session", "cmd": SESSION_CMD, "env": []}, self._session_started)
        elif kind == "error":
            self.busy = False
            self.retry = True
            self.entry.set_text("")
            self.stack.set_visible_child_name("lock")
            self.status.set_text("Not recognised. Press Enter to try again.")
            log("authentication rejected")
        elif kind == "auth_message":
            mode = response.get("auth_message_type")
            message = response.get("auth_message", "").strip()
            if mode in ("secret", "visible"):
                self.busy = False
                self.entry.set_text("")
                self.entry.set_visibility(mode == "visible")
                self.stack.set_visible_child_name("entry")
                self.entry.grab_focus()
                self.status.set_text(message or "Password")
                log("authentication input requested")
            else:
                self.status.set_text(message)
                self._send({"type": "post_auth_message_response", "response": None})
        else:
            self._on_ipc_error("unexpected response")
        return False

    def _session_started(self, response):
        if response.get("type") == "success":
            log("session started")
            self.quit()
        else:
            self.authenticated = False
            self.busy = False
            self.retry = True
            self.status.set_text("Could not start the desktop. Press Enter to retry.")
            log("session start rejected")
        return False

    def _on_submit(self, _entry):
        if self.busy or self.stack.get_visible_child_name() != "entry":
            return
        response = self.entry.get_text()
        self.entry.set_text("")
        self.stack.set_visible_child_name("lock")
        self.busy = True
        self.status.set_text("")
        self._send({"type": "post_auth_message_response", "response": response})

    def _on_key(self, _controller, keyval, _code, _state):
        if keyval in (Gdk.KEY_Return, Gdk.KEY_KP_Enter) and self.stack.get_visible_child_name() != "entry":
            self._request()
            return True
        if keyval == Gdk.KEY_Escape:
            # Escape never starts a session or races an outstanding PAM request.
            if not self.busy:
                self.retry = True
                self._request()
            return True
        return False


if __name__ == "__main__":
    sys.exit(Greeter().run([sys.argv[0]]))
