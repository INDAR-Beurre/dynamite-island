#!/usr/bin/env python3
"""Dynamite Settings — the control panel for the Dynamite island.

Plain-English sections:
  Overview        island / dictation / window-manager status, one-click panel
                  previews, dependency check
  Dictation & key Groq API key (paste / show / save / clear) + service control
  Keyboard        browse all shortcuts, edit the Dynamite ones safely
  Island          look & behaviour (size, motion, clock, theme, wallpaper)
  About

Python 3 + GTK4 (PyGObject), no build step.
"""

import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile
import time
import warnings

warnings.filterwarnings("ignore", message=".*is deprecated", category=DeprecationWarning)

import gi

gi.require_version("Gtk", "4.0")
from gi.repository import GLib, Gtk  # noqa: E402

APP_VERSION = "2.0"
HOME = os.path.expanduser("~")
XDG_CONFIG = GLib.get_user_config_dir()

ISLAND_BIN = os.path.join(HOME, "rust-island-build", "target", "release", "dynamite-rust")
ISLAND_SOCK = os.path.join(
    os.environ.get("XDG_RUNTIME_DIR", "/run/user/1000"), "dynamite-rust.sock"
)
CONFIG_DIR = os.path.join(XDG_CONFIG, "dynamite-rust")
CONFIG_PATH = os.path.join(CONFIG_DIR, "config.conf")
LIB_IMAGES = os.path.join(CONFIG_DIR, "library", "images")
LIB_THEMES = os.path.join(CONFIG_DIR, "library", "themes.json")

BIND_PATH = os.path.join(HOME, ".config", "mango", "bind.conf")
DICT_KEY_PATH = os.path.join(
    os.environ.get("XDG_CONFIG_HOME", os.path.join(HOME, ".config")),
    "static-dictation", "api_key",
)
DICT_BIN = os.path.join(HOME, "rust-island-build", "tools", "dictation",
                        "target", "release", "static-dictation")
DICT_SUPERVISOR = os.path.join(HOME, "rust-island-build", "tools", "dictation",
                               "scripts", "dictation_supervisor.sh")
DICT_PROC = "tools/dictation/target/release/static-dictation"

CAFFEINATE_BIN = os.path.join(HOME, ".local", "bin", "caffeinate")
LID_DROPIN = "/etc/elogind/logind.conf.d/90-lid-ignore.conf"
LID_DROPIN_DISABLED = LID_DROPIN + ".disabled"

# ---------------------------------------------------------------- config
# key -> (kind, default, min, max, step)
KEYS = {
    "notch_mode": ("bool", 0, None, None, None),
    "bar_height": ("int", 28, 18, 80, 1),
    "collapsed_width": ("int", 76, 40, 600, 1),
    "expanded_height": ("int", 150, 60, 800, 1),
    "gap_top": ("int", 3, 0, 200, 1),
    "inner_padding": ("int", 14, 0, 64, 1),
    "anim_ms": ("int", 160, 0, 1000, 10),
    "hover_open": ("bool", 1, None, None, None),
    "scroll_cycle": ("bool", 1, None, None, None),
    "clock_24h": ("bool", 1, None, None, None),
}
KEY_ORDER = list(KEYS)
# String keys the island understands (empty value = "use the island's own default").
STR_DEFAULTS = {"hover_state": "controlcenter", "theme": "", "wallpaper": ""}
HOVER_STATES = [
    ("controlcenter", "Control center"), ("launcher", "Launcher"),
    ("music", "Music"), ("calendar", "Calendar"), ("displays", "Displays"),
    ("wallpaper", "Wallpapers"), ("themes", "Themes"), ("power", "Power"),
    ("clipboard", "Clipboard"), ("notifications", "Notifications"),
    ("dictation", "Dictation"), ("collapsed", "Nothing (stay closed)"),
]
DEFAULTS = {k: spec[1] for k, spec in KEYS.items()}
DEFAULTS.update(STR_DEFAULTS)


def load_config():
    """dict of all known values (defaults filled) + unknown keys kept verbatim."""
    values = dict(DEFAULTS)
    unknown = {}
    try:
        with open(CONFIG_PATH, encoding="utf-8") as fh:
            for line in fh:
                line = line.strip()
                if not line or line.startswith("#") or "=" not in line:
                    continue
                key, _, raw = line.partition("=")
                key, raw = key.strip(), raw.strip()
                if key in KEYS:
                    kind = KEYS[key][0]
                    try:
                        values[key] = int(raw) if kind == "int" else (raw == "1")
                    except ValueError:
                        pass
                elif key in STR_DEFAULTS:
                    values[key] = raw
                else:
                    unknown[key] = raw
    except FileNotFoundError:
        pass
    return values, unknown


def save_config(values, unknown):
    os.makedirs(CONFIG_DIR, exist_ok=True)
    lines = ["# dynamite-rust island configuration (edited by Dynamite Settings)"]
    for key in KEY_ORDER:
        kind = KEYS[key][0]
        val = values[key]
        lines.append(f"{key}={int(val) if kind == 'int' else (1 if val else 0)}")
    for key in STR_DEFAULTS:
        if values.get(key):  # empty = leave it to the island's own default
            lines.append(f"{key}={values[key]}")
    for key, raw in unknown.items():
        lines.append(f"{key}={raw}")
    with open(CONFIG_PATH, "w", encoding="utf-8") as fh:
        fh.write("\n".join(lines) + "\n")


def poke_reload():
    """Ask the running island to reload its config. Silent if not running."""
    if os.path.exists(ISLAND_SOCK):
        subprocess.run([ISLAND_BIN, "reload"],
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                       timeout=3, check=False)


def island_running():
    return os.path.exists(ISLAND_SOCK)


# ---------------------------------------------------------------- caffeinate
def caffeinate_active():
    try:
        out = subprocess.run(["elogind-inhibit", "--list"],
                             capture_output=True, text=True, timeout=5,
                             check=False).stdout
    except Exception:
        return False
    return "handle-lid-switch" in out


def _wait_caffeinate_state(want, tries=10):
    for _ in range(tries):
        if caffeinate_active() == want:
            return True
        time.sleep(0.3)
    return caffeinate_active() == want


def set_caffeinate(on):
    try:
        if on:
            if not os.path.exists(LID_DROPIN) and os.path.exists(LID_DROPIN_DISABLED):
                subprocess.run(["sudo", "-n", "mv", LID_DROPIN_DISABLED, LID_DROPIN],
                               capture_output=True, timeout=10, check=False)
            if not caffeinate_active():
                subprocess.Popen([CAFFEINATE_BIN],
                                 stdin=subprocess.DEVNULL,
                                 stdout=subprocess.DEVNULL,
                                 stderr=subprocess.DEVNULL,
                                 start_new_session=True)
                _wait_caffeinate_state(True)
        else:
            subprocess.run([CAFFEINATE_BIN, "stop"],
                           capture_output=True, timeout=15, check=False)
            _wait_caffeinate_state(False)
            if os.path.exists(LID_DROPIN):
                subprocess.run(["sudo", "-n", "mv", LID_DROPIN, LID_DROPIN_DISABLED],
                               capture_output=True, timeout=10, check=False)
    except Exception:
        pass
    return caffeinate_active()


# ---------------------------------------------------------------- dictation
def dictation_running():
    return subprocess.run(["pgrep", "-f", DICT_PROC],
                          capture_output=True).returncode == 0


def dictation_supervisor_running():
    return subprocess.run(["pgrep", "-f", "dictation_supervisor.sh"],
                          capture_output=True).returncode == 0


def restart_dictation_binary():
    """Kill the binary; the supervisor brings it back in ~1 s."""
    subprocess.run(["pkill", "-f", DICT_PROC], capture_output=True, check=False)
    start_dictation_service()


def start_dictation_service():
    if dictation_supervisor_running() or dictation_running():
        return
    cmd = ["nohup", "bash", DICT_SUPERVISOR] if os.path.exists(DICT_SUPERVISOR) \
        else ["nohup", DICT_BIN]
    subprocess.Popen(cmd, stdin=subprocess.DEVNULL,
                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                     start_new_session=True)


def read_api_key():
    try:
        with open(DICT_KEY_PATH, encoding="utf-8") as fh:
            return fh.read().strip()
    except FileNotFoundError:
        return ""


def api_key_info():
    try:
        mode = stat.S_IMODE(os.stat(DICT_KEY_PATH).st_mode)
        return f"Key stored ({len(read_api_key())} characters, permissions {mode:03o})"
    except FileNotFoundError:
        return "No key stored yet — dictation cannot transcribe until one is added."


def save_api_key(key):
    os.makedirs(os.path.dirname(DICT_KEY_PATH), exist_ok=True)
    with open(DICT_KEY_PATH, "w", encoding="utf-8") as fh:
        fh.write(key + "\n")
    os.chmod(DICT_KEY_PATH, stat.S_IRUSR | stat.S_IWUSR)  # 600


def clear_api_key():
    try:
        os.remove(DICT_KEY_PATH)
        return True
    except FileNotFoundError:
        return False


# ---------------------------------------------------------------- keybindings
MODS = {"SUPER", "CTRL", "ALT", "SHIFT"}
ACTIONS = [
    ("launcher", "Quick launcher"), ("clipboard", "Clipboard history"),
    ("music", "Music"), ("controlcenter", "Control center"),
    ("calendar", "Calendar"), ("displays", "Displays"),
    ("wallpaper", "Wallpapers"), ("themes", "Themes"),
    ("power", "Power"), ("notifications", "Notifications"),
    ("dictation", "Dictation"), ("lockscreen", "Lock screen"),
    ("collapsed", "Close the island"), ("reload", "Reload island settings"),
]
ACTION_CMD = {w: f"{ISLAND_BIN} {w}" for w, _ in ACTIONS}
FRIENDLY = {
    "launcher": "Quick launcher", "clipboard": "Clipboard history",
    "lockscreen": "Lock screen", "music": "Music", "themes": "Themes",
    "wallpaper": "Wallpapers", "controlcenter": "Control center",
    "calendar": "Calendar", "power": "Power", "notifications": "Notifications",
    "displays": "Displays", "foot": "Terminal", "waterfox": "Web browser",
    "keyguard": "Password manager (Bitwarden)", "wlogout": "Log out / power menu",
    "swaylock": "Lock screen (swaylock)", "swaync-client": "Hide/show notifications",
    "nautilus": "File browser", "yazi": "File browser (yazi)",
    "nano": "Text editor", "btop": "System monitor", "screenshot.sh": "Screenshot",
    "volume.sh": "Volume", "brightness.sh": "Brightness", "magpie-gui": "Magpie chat",
}
KEY_PRETTY = {"space": "Space", "btn_left": "left click",
              "btn_right": "right click", "btn_middle": "middle click"}


def is_dynamite_line(line):
    return line.startswith("bind=") and "dynamite-rust" in line


def parse_dynamite_lines():
    """[{'combo': 'SUPER+M', 'cmd': '/path spawn args minus the verb', 'raw'}...].

    A dynamite bind line is  bind=<mods>,<key>,spawn,<command>.
    """
    rows = []
    try:
        with open(BIND_PATH, encoding="utf-8") as fh:
            for line in fh.read().splitlines():
                if not is_dynamite_line(line):
                    continue
                fields = line[len("bind="):].split(",")  # mods, key, spawn, cmd...
                mods = fields[0] if len(fields) > 0 else "none"
                key = fields[1] if len(fields) > 1 else ""
                combo = f"{mods}+{key}" if mods != "none" else key
                cmd = ",".join(fields[3:])
                rows.append({"combo": combo, "cmd": cmd, "raw": line})
    except FileNotFoundError:
        pass
    return rows


def parse_all_shortcuts():
    """[(keycap_label, description)] for every bind / mouse / gesture line."""
    rows = []
    try:
        with open(BIND_PATH, encoding="utf-8") as fh:
            lines = fh.read().splitlines()
    except FileNotFoundError:
        return rows
    for line in lines:
        for prefix in ("bind=", "mousebind=", "gesturebind="):
            if not line.startswith(prefix):
                continue
            fields = line[len(prefix):].split(",")
            combo = fields[0] if fields else "none"
            rest = fields[1:]
            if combo == "none" and rest:
                keycap = KEY_PRETTY.get(rest[0], rest[0])
                rest = rest[1:]
            else:
                mods = pretty_combo(combo)
                keycap = f"{mods} + {KEY_PRETTY.get(rest[0], rest[0])}" if rest else mods
            desc = " ".join(rest)
            spawn = desc.split(" ")
            if spawn and spawn[0] == "spawn" and len(spawn) > 1:
                target = os.path.basename(spawn[1])
                if target == "dynamite-rust":
                    word = spawn[2].strip() if len(spawn) > 2 else ""
                    desc = FRIENDLY.get(word, desc)
                else:
                    desc = FRIENDLY.get(target, " ".join(spawn[1:]))
            rows.append((keycap, desc))
    return rows


def pretty_combo(combo):
    name = {"SUPER": "Super", "CTRL": "Ctrl", "ALT": "Alt", "SHIFT": "Shift"}
    return " + ".join(name.get(p, p) for p in combo.split("+") if p.upper() != "NONE")


def norm_combo(text):
    """'SUPER + M' -> 'SUPER,M' ; 'M' -> 'none,M' ; None when unparseable."""
    parts = [p.strip() for p in text.split("+") if p.strip()]
    mods, keys = [], []
    for p in parts:
        up = p.upper()
        if up in MODS:
            mods.append(up)
        elif up == "NONE":
            pass
        else:
            keys.append(p)
    if not keys:
        return None
    return ("+".join(mods) if mods else "none") + "," + keys[0]


MANAGED_HEADER = "# Dynamite shortcuts (managed by Dynamite Settings)"


def is_managed_line(line):
    """A line the managed block owns: a dynamite bind or its header."""
    return is_dynamite_line(line) or line.startswith(MANAGED_HEADER)
def build_shortcuts(rows):
    out = [MANAGED_HEADER]
    for row in rows:
        line = norm_combo(row["combo"])
        if line is None or not row["cmd"].strip():
            continue
        out.append(f"bind={line},spawn,{row['cmd'].strip()}")
    return out


def save_shortcuts(rows):
    """Rewrite bind.conf, preserving every unrelated line; validate with mango -p."""
    try:
        with open(BIND_PATH, encoding="utf-8") as fh:
            lines = fh.read().splitlines()
    except FileNotFoundError:
        return "bind.conf not found — nothing saved."
    anchor = next((i for i, l in enumerate(lines) if is_managed_line(l)), len(lines))
    others = [l for l in lines if not is_managed_line(l)]
    new = others[:anchor] + build_shortcuts(rows) + others[anchor:]
    content = "\n".join(new) + "\n"

    mango = shutil.which("mango")
    tmp = tempfile.NamedTemporaryFile(
        mode="w", suffix=".conf", dir=os.path.dirname(BIND_PATH),
        delete=False, encoding="utf-8")
    tmp.write(content)
    tmp.close()
    if mango:
        r = subprocess.run([mango, "-c", tmp.name, "-p"],
                           capture_output=True, text=True, timeout=10, check=False)
        if r.returncode != 0:
            os.unlink(tmp.name)
            detail = (r.stderr or r.stdout).strip().splitlines()
            return (f"mango rejected the new config ({detail[0] if detail else 'unknown'}) "
                    "— nothing was changed.")
    shutil.copyfile(tmp.name, BIND_PATH)
    os.chmod(BIND_PATH, stat.S_IRUSR | stat.S_IWUSR | stat.S_IRGRP | stat.S_IROTH)  # 644
    os.unlink(tmp.name)
    return ("Saved — bind.conf updated"
            + ("" if mango else " (mango not found; validation skipped)."))


CSS = """
window { background: #0A0F14; }
.sidebar { background: #0D1219; }
.sidebar row { border-radius: 8px; padding: 7px 10px; margin: 1px 8px; }
.sidebar row:selected { background: #1B2432; }
.sidebar row label { color: #C9D1D9; font-size: 13px; }
.sidebar row:selected label { color: #FFFFFF; }
.icon-glyph { font-size: 16px; min-width: 24px; }
.search { background: #161C26; color: #E6EAF0; border-radius: 8px;
          border: none; margin: 10px 10px 6px 10px; padding: 6px 8px; }
.header-card { background: #11161F; border-radius: 12px; padding: 14px 16px; }
.header-icon { font-size: 26px; }
.header-title { font-size: 16px; font-weight: bold; color: #FFFFFF; }
.header-sub { font-size: 12px; color: #9AA0A6; }
.card { background: #11161F; border-radius: 12px; padding: 12px 16px; }
.card-title { font-size: 13px; font-weight: bold; color: #E6EAF0; }
.row-label { font-size: 13px; color: #E6EAF0; }
.row-value { font-size: 12px; color: #9AA0A6; }
.hint { font-size: 11px; color: #6E7681; }
.dot { font-size: 10px; }
.dot.ok { color: #3FB950; }
.dot.bad { color: #F85149; }
.keycap { background: #1B2432; border-radius: 6px; padding: 2px 8px;
          font-family: monospace; font-size: 12px; color: #C9D1D9; }
scale { margin: 2px 0 6px 0; }
scale trough { background: #232B38; border-radius: 999px; min-height: 6px; }
scale highlight { background: #8FA6F5; border-radius: 999px; }
scale slider { background: #FFFFFF; border-radius: 999px;
               min-width: 14px; min-height: 14px; margin: -4px;
               box-shadow: 0 1px 3px rgba(0,0,0,0.5); }
switch { background: #2A3342; border-radius: 999px; border: none; }
switch:checked { background: #3B82F6; }
switch slider { background: #FFFFFF; border-radius: 999px; }
.page { padding: 14px 18px; }
button.solid { background: #3B82F6; color: #fff; border: none;
               border-radius: 8px; padding: 6px 14px; }
button.solid:hover { background: #5590FF; }
button.plain { background: #1B2432; color: #E6EAF0; border: none;
               border-radius: 8px; padding: 6px 14px; }
button.plain:hover { background: #232E40; }
button.danger { background: #3A1D20; color: #F85149; border: none;
                border-radius: 8px; padding: 6px 14px; }
button.danger:hover { background: #4D262B; }
.status-flash { color: #3FB950; }
.status-error { color: #F85149; }
"""


class SettingsWindow(Gtk.ApplicationWindow):
    def __init__(self, app):
        super().__init__(application=app, title="Dynamite Settings")
        self.set_default_size(780, 560)
        self.values, self.unknown = load_config()
        self._building = True
        self._timers = {}
        self._island_sliders = []
        self._island_switches = []
        self._island_combo_refs = []
        self._dyn_widgets = []

        provider = Gtk.CssProvider()
        provider.load_from_data(CSS.encode())
        Gtk.StyleContext.add_provider_for_display(
            self.get_display(), provider,
            Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)

        root = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        self.set_child(root)

        # ---- sidebar ----
        side = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        side.add_css_class("sidebar")
        side.set_size_request(200, -1)
        root.append(side)
        self.search = Gtk.SearchEntry(placeholder_text="Search Settings")
        self.search.add_css_class("search")
        self.search.connect("search-changed", self._on_search)
        side.append(self.search)
        self.sidebar = Gtk.ListBox()
        self.sidebar.add_css_class("sidebar")
        self.sidebar.set_selection_mode(Gtk.SelectionMode.SINGLE)
        self.sidebar.connect("row-selected", self._on_row_selected)
        side.append(self.sidebar)

        # ---- pages ----
        self.stack = Gtk.Stack()
        self.stack.set_hexpand(True)
        self.stack.set_transition_type(Gtk.StackTransitionType.CROSSFADE)
        self.stack.set_transition_duration(120)
        root.append(self.stack)

        pages = [
            ("home", "🏠", "Overview",
             "Island status, quick previews, and a health check."),
            ("dictation", "🎤", "Dictation & API key",
             "Voice dictation and the Groq key it uses."),
            ("keys", "⌨", "Keyboard",
             "Every shortcut, and the Dynamite ones you can change."),
            ("island", "✦", "Island", "How the island looks and behaves."),
            ("about", "ℹ", "About",
             "This app and the island it controls."),
        ]
        builders = {
            "home": self._build_home, "dictation": self._build_dictation,
            "keys": self._build_keys, "island": self._build_island,
            "about": self._build_about,
        }
        for name, glyph, title, subtitle in pages:
            page = self._make_page(name, glyph, title, subtitle)
            builders[name](page)
            row = self._sidebar_row(glyph, title)
            row.page_name = name
            row.search_text = title.lower()
            self.sidebar.append(row)
        self.sidebar.select_row(self.sidebar.get_row_at_index(0))
        self._building = False

    def _make_page(self, name, glyph, title, subtitle):
        page = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        page.add_css_class("page")
        card = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        card.add_css_class("header-card")
        icon = Gtk.Label(label=glyph)
        icon.add_css_class("header-icon")
        icon.set_halign(Gtk.Align.CENTER)
        card.append(icon)
        card.append(Gtk.Label(label=title, xalign=0))
        sub = Gtk.Label(label=subtitle, xalign=0)
        sub.add_css_class("header-sub")
        card.append(sub)
        page.append(card)
        scroller = Gtk.ScrolledWindow()
        scroller.set_child(page)
        scroller.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        self.stack.add_named(scroller, name)
        return page

    def _sidebar_row(self, glyph, label):
        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        g = Gtk.Label(label=glyph)
        g.add_css_class("icon-glyph")
        box.append(g)
        box.append(Gtk.Label(label=label, xalign=0))
        row = Gtk.ListBoxRow()
        row.set_child(box)
        row.search_text = label.lower()
        return row

    def _card(self, title):
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        box.add_css_class("card")
        lab = Gtk.Label(label=title, xalign=0)
        lab.add_css_class("card-title")
        box.append(lab)
        return box

    def _dot(self, ok):
        lab = Gtk.Label(label="●")
        lab.add_css_class("dot")
        lab.add_css_class("ok" if ok else "bad")
        return lab

    def _status_label(self):
        lab = Gtk.Label(label="", xalign=0, wrap=True)
        lab.add_css_class("hint")
        return lab

    def flash(self, label, text, ok=True):
        old = self._timers.pop(id(label), 0)
        if old:
            GLib.source_remove(old)
        label.set_label(text)
        label.remove_css_class("status-flash")
        label.remove_css_class("status-error")
        label.add_css_class("status-flash" if ok else "status-error")
        self._timers[id(label)] = GLib.timeout_add(3000, self._flash_clear, label)

    def _flash_clear(self, label):
        sid = self._timers.pop(id(label), 0)
        if sid:
            GLib.source_remove(sid)
        label.remove_css_class("status-flash")
        label.remove_css_class("status-error")
        return GLib.SOURCE_REMOVE

    # ---------- Overview ----------
    def _build_home(self, page):
        outer = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        outer.append(self._status_card())
        outer.append(self._quick_actions_card())
        outer.append(self._deps_card())
        page.append(outer)

    def _status_card(self):
        card = self._card("Status")
        self._status_rows = {}
        for key, label in (("island", "Island (Dynamite)"),
                           ("wm", "Window manager (MangoWM)"),
                           ("dictation", "Voice dictation")):
            row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
            dot = self._dot(True)
            name = Gtk.Label(label=label, xalign=0)
            name.add_css_class("row-label")
            name.set_hexpand(True)
            txt = Gtk.Label(label="…", xalign=0)
            txt.add_css_class("row-value")
            row.append(dot)
            row.append(name)
            row.append(txt)
            card.append(row)
            self._status_rows[key] = (dot, txt)
        self._refresh_status()
        btn = Gtk.Button(label="Refresh status")
        btn.add_css_class("plain")
        btn.set_halign(Gtk.Align.START)
        btn.connect("clicked", lambda *_: self._refresh_status())
        card.append(btn)
        return card

    def _refresh_status(self):
        island = island_running()
        wm = subprocess.run(["pgrep", "-x", "mango"],
                            capture_output=True).returncode == 0
        dicting = dictation_running()
        states = {
            "island": (island, "Running — panels ready" if island
                       else "Not running — its panels can't be opened"),
            "wm": (wm, "Running" if wm else "Not running"),
            "dictation": (dicting, "Running — hold Super + D to talk" if dicting
                          else "Stopped — start it from the Dictation & API key page"),
        }
        for key, (dot, txt) in self._status_rows.items():
            ok, text = states[key]
            dot.remove_css_class("ok")
            dot.remove_css_class("bad")
            dot.add_css_class("ok" if ok else "bad")
            txt.set_label(text)

    def _quick_actions_card(self):
        card = self._card("Quick previews — open a panel on the island")
        grid = Gtk.Grid(column_spacing=8, row_spacing=8)
        panels = [("launcher", "Launcher"), ("clipboard", "Clipboard"),
                  ("music", "Music"), ("themes", "Themes"),
                  ("wallpaper", "Wallpapers"), ("controlcenter", "Control center"),
                  ("calendar", "Calendar"), ("power", "Power"),
                  ("notifications", "Notifications"), ("displays", "Displays")]
        for i, (word, label) in enumerate(panels):
            b = Gtk.Button(label=label)
            b.add_css_class("plain")
            b.connect("clicked", lambda _b, w=word, l=label: self._show_panel(w, l))
            grid.attach(b, i % 2, i // 2, 1, 1)
        card.append(grid)
        self._home_status = self._status_label()
        card.append(self._home_status)
        return card

    def _show_panel(self, word, label):
        if not island_running():
            self.flash(self._home_status,
                       "The island is not running, so there is nowhere to open it.",
                       ok=False)
            return
        try:
            subprocess.run([ISLAND_BIN, word],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                           timeout=3, check=False)
            self.flash(self._home_status, f"{label} panel opened on the island.")
        except Exception:
            self.flash(self._home_status, f"Could not open {label}.", ok=False)

    def _deps_card(self):
        card = self._card("Dependency check")
        tools = [
            ("mmsg", "App-to-app messages the island uses"),
            ("wpctl", "Volume control"),
            ("brightnessctl", "Screen brightness"),
            ("playerctl", "Music player controls"),
            ("swaync-client", "Notifications"),
            ("cliphist", "Clipboard history"),
            ("wtype", "Types text into apps for you"),
            ("mango", "The window manager itself"),
        ]
        for name, desc in tools:
            ok = shutil.which(name) is not None
            row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
            row.append(self._dot(ok))
            row.append(Gtk.Label(label=name, xalign=0))
            d = Gtk.Label(label=desc if ok else f"{desc} — not found on this system",
                          xalign=0)
            d.add_css_class("row-value")
            d.set_hexpand(True)
            row.append(d)
            card.append(row)
        return card

    # ---------- Dictation & API key ----------
    def _build_dictation(self, page):
        svc = self._card("Dictation service")
        row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        self._dict_dot = self._dot(True)
        self._dict_state = Gtk.Label(label="…", xalign=0)
        self._dict_state.add_css_class("row-label")
        self._dict_state.set_hexpand(True)
        row.append(self._dict_dot)
        row.append(self._dict_state)
        svc.append(row)
        hint = Gtk.Label(
            label="Hold Super + D and speak; the island shows what you said, and "
                  "release to finish.", xalign=0, wrap=True)
        hint.add_css_class("hint")
        svc.append(hint)
        self._dict_btn = Gtk.Button(label="Restart dictation")
        self._dict_btn.add_css_class("solid")
        self._dict_btn.set_halign(Gtk.Align.START)
        self._dict_btn.connect("clicked", self._dictation_action)
        svc.append(self._dict_btn)
        page.append(svc)

        key = self._card("Groq API key")
        entry_row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        self._key_entry = Gtk.Entry(visibility=False)
        self._key_entry.set_hexpand(True)
        self._key_entry.set_placeholder_text("Paste your Groq API key here")
        self._key_entry.connect("changed", lambda *_: self._update_key_status())
        eye = Gtk.Button(label="Show")
        eye.add_css_class("plain")
        eye.set_valign(Gtk.Align.CENTER)
        eye.connect("clicked", self._toggle_key_visibility)
        entry_row.append(self._key_entry)
        entry_row.append(eye)
        key.append(entry_row)
        self._key_status = self._status_label()
        key.append(self._key_status)
        link = Gtk.LinkButton(uri="https://console.groq.com",
                              label="Get a free key at console.groq.com "
                                    "(sign in → API Keys tab)")
        link.set_halign(Gtk.Align.START)
        key.append(link)
        btns = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        save_b = Gtk.Button(label="Save key")
        save_b.add_css_class("solid")
        save_b.connect("clicked", self._save_key)
        clear_b = Gtk.Button(label="Clear key")
        clear_b.add_css_class("danger")
        clear_b.connect("clicked", self._clear_key)
        btns.append(save_b)
        btns.append(clear_b)
        key.append(btns)
        note = Gtk.Label(
            label=f"Stored on this machine only (~/.config/static-dictation/api_key, "
                  "permissions 600). It is never logged or printed.",
            xalign=0, wrap=True)
        note.add_css_class("hint")
        key.append(note)
        page.append(key)
        page.append(Gtk.Box(vexpand=True))

        self._key_entry.set_text(read_api_key())
        self._refresh_dictation_ui()

    def _toggle_key_visibility(self, btn):
        vis = self._key_entry.get_visibility()
        self._key_entry.set_visibility(not vis)
        btn.set_label("Hide" if not vis else "Show")

    def _refresh_dictation_ui(self):
        running = dictation_running()
        self._dict_dot.remove_css_class("ok")
        self._dict_dot.remove_css_class("bad")
        self._dict_dot.add_css_class("ok" if running else "bad")
        if running:
            self._dict_state.set_label("Running — hold Super + D to talk")
            self._dict_btn.set_label("Restart dictation")
        else:
            self._dict_state.set_label(
                "Stopped" + (" (the service will restart it shortly)"
                             if dictation_supervisor_running() else ""))
            self._dict_btn.set_label("Start dictation")
        self._update_key_status()

    def _update_key_status(self):
        text = self._key_entry.get_text().strip()
        if text and text != read_api_key():
            self.flash(self._key_status,
                       f"New key ready to save ({len(text)} characters — not stored "
                       "until you press Save key).")
        else:
            self._key_status.set_label(api_key_info())

    def _dictation_action(self, _btn):
        was_running = dictation_running()
        if was_running:
            restart_dictation_binary()
            self.flash(self._key_status,
                       "Restarting — the service brings it back in about a second.")
        else:
            start_dictation_service()
            self.flash(self._key_status, "Starting dictation service…")
        self._refresh_dictation_dot_only()
        GLib.timeout_add(1500, self._dictation_status_timer)

    def _dictation_status_timer(self):
        self._refresh_dictation_ui()
        return GLib.SOURCE_REMOVE

    def _refresh_dictation_dot_only(self):
        running = dictation_running()
        self._dict_dot.remove_css_class("ok")
        self._dict_dot.remove_css_class("bad")
        self._dict_dot.add_css_class("ok" if running else "bad")
        self._dict_btn.set_label("Restart dictation" if running else "Start dictation")

    def _save_key(self, _btn):
        key = self._key_entry.get_text().strip()
        if not key:
            self.flash(self._key_status, "Paste a key first (or use Clear key).",
                       ok=False)
            return
        save_api_key(key)
        restart_dictation_binary()  # supervisor re-reads the key on start
        self._refresh_dictation_ui()
        self.flash(self._key_status, "Key saved and dictation restarted with it.")

    def _clear_key(self, _btn):
        clear_api_key()
        self._key_entry.set_text("")
        restart_dictation_binary()
        self._refresh_dictation_ui()
        self.flash(self._key_status,
                   "Key removed — dictation will not transcribe until you add one.")

    # ---------- Keyboard ----------
    def _build_keys(self, page):
        card = self._card("Dynamite shortcuts — the ones you change most")
        hint = Gtk.Label(
            label="Super = the Windows key. Change the key, add a shortcut, or "
                  "remove one — every unrelated line in bind.conf is preserved, "
                  "and mango validates the file before anything is written.",
            xalign=0, wrap=True)
        hint.add_css_class("hint")
        card.append(hint)
        self._dyn_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        card.append(self._dyn_box)
        add_b = Gtk.Button(label="+ Add shortcut")
        add_b.add_css_class("plain")
        add_b.set_halign(Gtk.Align.START)
        add_b.connect("clicked", lambda *_: self._dyn_row("SUPER+C",
                                                           ACTION_CMD["launcher"]))
        card.append(add_b)
        page.append(card)

        exp = Gtk.Expander(label="Show all shortcuts (read-only)")
        inner = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        inner.add_css_class("card")
        for keycap, desc in parse_all_shortcuts():
            row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
            cap = Gtk.Label(label=keycap, xalign=0)
            cap.add_css_class("keycap")
            cap.set_valign(Gtk.Align.CENTER)
            d = Gtk.Label(label=desc, xalign=0)
            d.add_css_class("row-value")
            row.append(cap)
            row.append(d)
            inner.append(row)
        exp.set_child(inner)
        page.append(exp)

        btns = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        save_b = Gtk.Button(label="Save shortcuts")
        save_b.add_css_class("solid")
        save_b.connect("clicked", self._save_keys_ui)
        reset_b = Gtk.Button(label="Revert changes")
        reset_b.add_css_class("plain")
        reset_b.connect("clicked", lambda *_: self._rebuild_dyn_rows(""))
        btns.append(save_b)
        btns.append(reset_b)
        page.append(btns)
        self._keys_status = self._status_label()
        page.append(self._keys_status)
        page.append(Gtk.Box(vexpand=True))
        self._rebuild_dyn_rows("")

    def _dyn_row(self, combo_text, cmd):
        row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        entry = Gtk.Entry()
        entry.set_text(combo_text)
        entry.set_size_request(130, -1)
        entry.set_tooltip_text("Key combo, e.g.  SUPER + M   (Super/Alt/Ctrl/Shift + key)")
        combo_box = Gtk.ComboBoxText()
        combo_box.append_text("Custom command…")
        known = next((i + 1 for i, (w, _) in enumerate(ACTIONS)
                      if ACTION_CMD[w] == cmd), -1)
        for _, label in ACTIONS:
            combo_box.append_text(label)
        combo_box.set_active(known if known >= 0 else 0)
        cmd_entry = Gtk.Entry()
        cmd_entry.set_text(cmd if known < 0 else "")
        cmd_entry.set_hexpand(True)
        cmd_entry.set_visible(known < 0)
        cmd_entry.set_placeholder_text("Command to run, e.g. foot")
        combo_box.connect("changed",
                          lambda cb: cmd_entry.set_visible(cb.get_active() == 0))
        rem = Gtk.Button(label="✕")
        rem.add_css_class("plain")
        rem.set_tooltip_text("Remove this shortcut")
        row.append(entry)
        row.append(combo_box)
        row.append(cmd_entry)
        row.append(rem)
        self._dyn_box.append(row)
        ref = (entry, combo_box, cmd_entry, row)
        self._dyn_widgets.append(ref)
        rem.connect("clicked", lambda *_b, r=ref: self._drop_dyn_row(r))

    def _drop_dyn_row(self, ref):
        self._dyn_box.remove(ref[3])
        if ref in self._dyn_widgets:
            self._dyn_widgets.remove(ref)

    def _rebuild_dyn_rows(self, status=""):
        for ref in list(self._dyn_widgets):
            self._drop_dyn_row(ref)
        for row in parse_dynamite_lines():
            self._dyn_row(row["combo"], row["cmd"])
        self._keys_status.set_label(status)

    def _collect_dyn_rows(self):
        rows = []
        for entry, combo_box, cmd_entry, _row in self._dyn_widgets:
            active = combo_box.get_active()
            if active > 0:
                word = ACTIONS[active - 1][0]
                rows.append({"combo": entry.get_text(), "cmd": ACTION_CMD[word],
                             "raw": ""})
            else:
                cmd = cmd_entry.get_text().strip()
                if cmd:
                    rows.append({"combo": entry.get_text(), "cmd": cmd, "raw": ""})
        return rows

    def _save_keys_ui(self, _btn):
        result = save_shortcuts(self._collect_dyn_rows())
        if result.startswith("Saved"):
            self._rebuild_dyn_rows()
            self.flash(self._keys_status,
                      result + " — reload MangoWM (Super + R) to make it live.")
        else:
            self.flash(self._keys_status, result, ok=False)

    # ---------- Island ----------
    def _build_island(self, page):
        page.append(self._island_controls_card())
        page.append(self._island_theme_card())
        page.append(self._caffeinate_row())
        footer = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        reset_b = Gtk.Button(label="Reset to defaults")
        reset_b.add_css_class("plain")
        reset_b.connect("clicked", self._reset_island)
        apply_b = Gtk.Button(label="Apply")
        apply_b.add_css_class("solid")
        apply_b.connect("clicked", self._apply_island)
        self._island_status = self._status_label()
        self._island_status.set_hexpand(True)
        footer.append(reset_b)
        footer.append(apply_b)
        footer.append(self._island_status)
        page.append(footer)
        page.append(Gtk.Box(vexpand=True))

    def _island_controls_card(self):
        card = self._card("Size and behaviour")
        controls = [
            ("slider", "collapsed_width", "Pill width",
             "How wide the island is when closed"),
            ("slider", "bar_height", "Pill height", "Thickness of the pill"),
            ("slider", "gap_top", "Gap from the top",
             "Space between the screen edge and the pill"),
            ("slider", "expanded_height", "Panel height",
             "Taller or shorter open panels"),
            ("slider", "inner_padding", "Inner padding",
             "Breathing room inside the panels"),
            ("slider", "anim_ms", "Animation speed",
             "How fast panels open (0 = instant)"),
            ("switch", "hover_open", "Open panels on hover",
             "Just touching the pill opens the panel — no click needed"),
            ("switch", "scroll_cycle", "Mouse wheel cycles panels",
             "Scroll over the island to step through its panels"),
            ("switch", "clock_24h", "24-hour clock",
             "Off shows 12-hour time with AM/PM"),
            ("switch", "notch_mode", "Notch mode",
             "Square-notch look (no visual change yet)"),
        ]
        for ctrl in controls:
            card.append(self._make_control(ctrl))
        card.append(self._combo_control("Panel opened on hover",
                                        [("", "Default (control center)")]
                                        + list(HOVER_STATES),
                                        "hover_state"))
        row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=4)
        lab = Gtk.Label(label="Clock preview:", xalign=0)
        lab.add_css_class("row-label")
        self._island_clock_preview = Gtk.Label(xalign=0)
        self._island_clock_preview.add_css_class("row-value")
        row.append(lab)
        row.append(self._island_clock_preview)
        card.append(row)
        self._refresh_clock_preview()
        return card

    def _refresh_clock_preview(self):
        if not getattr(self, "_island_clock_preview", None):
            return
        now = GLib.DateTime.new_now_local()
        fmt = "%H:%M" if self.values.get("clock_24h", 1) else "%I:%M %p"
        self._island_clock_preview.set_label(now.format(fmt))

    def _island_theme_card(self):
        card = self._card("Look — theme and wallpaper")
        card.append(self._combo_control("Island theme", self._theme_options(),
                                        "theme"))
        card.append(self._combo_control("Wallpaper", self._wallpaper_options(),
                                        "wallpaper"))
        hint = Gtk.Label(label="Applied when you press Apply — the island reloads "
                               "and re-skins itself.", xalign=0, wrap=True)
        hint.add_css_class("hint")
        card.append(hint)
        return card

    def _theme_options(self):
        opts = [("", "Default")]
        try:
            with open(LIB_THEMES, encoding="utf-8") as fh:
                opts += [(n, n) for n in json.load(fh)["themes"]]
        except Exception:
            pass
        cur = self.values.get("theme", "")
        if cur and not any(n == cur for n, _ in opts):
            opts.append((cur, f"{cur} (current)"))
        return opts

    def _wallpaper_options(self):
        opts = [("", "Default")]
        try:
            opts += [(f, os.path.splitext(f)[0])
                     for f in sorted(os.listdir(LIB_IMAGES))]
        except FileNotFoundError:
            pass
        cur = self.values.get("wallpaper", "")
        if cur and not any(n == cur for n, _ in opts):
            opts.append((cur, f"{cur} (current)"))
        return opts

    def _combo_control(self, label, options, key):
        head = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        lab = Gtk.Label(label=label, xalign=0)
        lab.add_css_class("row-label")
        lab.set_hexpand(True)
        head.append(lab)
        combo = Gtk.ComboBoxText()
        current = self.values.get(key, "")
        active = 0
        for i, (value, text) in enumerate(options):
            combo.append_text(text)
            if value == current:
                active = i
        combo.set_active(active)
        combo.set_hexpand(True)
        combo.connect("changed", self._on_combo, key)
        head.append(combo)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        box.append(head)
        self._island_combo_refs.append((key, combo))
        return box

    def _on_combo(self, combo, key):
        active = combo.get_active()
        model = combo.get_model()
        self.values[key] = model[active]["id"] if 0 <= active < model.n_rows() else ""
        self._mark_island_dirty(True)

    def _make_control(self, ctrl):
        kind, key, label, hint = ctrl
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        top = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        lab = Gtk.Label(label=label, xalign=0)
        lab.add_css_class("row-label")
        lab.set_hexpand(True)
        top.append(lab)
        box.append(top)
        if kind == "slider":
            _, _, lo, hi, step = KEYS[key]
            val = Gtk.Label(label=str(int(self.values[key])), xalign=0)
            val.add_css_class("row-value")
            top.append(val)
            adj = Gtk.Adjustment(value=float(self.values[key]), lower=lo,
                                 upper=hi, step_increment=step,
                                 page_increment=step * 10)
            scale = Gtk.Scale(orientation=Gtk.Orientation.HORIZONTAL,
                              adjustment=adj)
            scale.set_draw_value(False)
            scale.set_hexpand(True)
            scale.connect("value-changed", self._on_slider, key, val)
            box.append(scale)
            self._island_sliders.append((key, scale, val))
        else:
            sw = Gtk.Switch()
            sw.set_active(bool(self.values[key]))
            sw.set_valign(Gtk.Align.CENTER)
            sw.connect("notify::active", self._on_switch, key)
            top.append(sw)
            self._island_switches.append((key, sw))
            if hint:
                h = Gtk.Label(label=hint, xalign=0, wrap=True)
                h.add_css_class("hint")
                box.append(h)
        return box

    def _on_slider(self, scale, key, val_label):
        v = int(round(scale.get_value()))
        val_label.set_label(str(v))
        self.values[key] = v
        self._mark_island_dirty(True)

    def _on_switch(self, switch, _pspec, key):
        self.values[key] = bool(switch.get_active())
        if key == "clock_24h":
            self._refresh_clock_preview()
        self._mark_island_dirty(True)

    def _mark_island_dirty(self, dirty):
        pass  # reserved for a "unsaved changes" indicator

    def _caffeinate_row(self):
        card = self._card("Laptop lid")
        top = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        lab = Gtk.Label(label="Keep running when the lid closes", xalign=0)
        lab.add_css_class("row-label")
        lab.set_hexpand(True)
        top.append(lab)
        sw = Gtk.Switch()
        sw.set_active(caffeinate_active())
        sw.set_valign(Gtk.Align.CENTER)
        top.append(sw)
        card.append(top)
        self._caffeinate_status = Gtk.Label(xalign=0)
        self._caffeinate_status.add_css_class("hint")
        card.append(self._caffeinate_status)
        self._caffeinate_switch = sw
        self._caffeinate_syncing = False
        sw.connect("notify::active", self._on_caffeinate)
        self._sync_caffeinate_ui(caffeinate_active())
        return card

    def _sync_caffeinate_ui(self, active):
        self._caffeinate_syncing = True
        self._caffeinate_switch.set_active(active)
        self._caffeinate_syncing = False
        self._caffeinate_status.set_label(
            "Lid close is ignored — the machine keeps running"
            if active else "Lid close suspends the machine (default)")

    def _on_caffeinate(self, switch, _pspec):
        if self._caffeinate_syncing:
            return
        real = set_caffeinate(bool(switch.get_active()))
        self._sync_caffeinate_ui(real)

    def _apply_island(self, _btn):
        save_config(self.values, self.unknown)
        theme = self.values.get("theme", "")
        if theme:
            subprocess.run([ISLAND_BIN, "--apply-theme", theme],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                           timeout=5, check=False)
        poke_reload()
        self._mark_island_dirty(False)
        self.flash(
            self._island_status,
            "Saved — the island reloaded with the new settings"
            if island_running() else
            "Saved — the island will pick them up next time it starts")

    def _reset_island(self, _btn):
        for k, d in DEFAULTS.items():
            self.values[k] = d
        self._sync_island_widgets()
        self._apply_island(None)
        self.flash(self._island_status, "Reset to defaults and applied.")

    def _sync_island_widgets(self):
        for key, scale, val in self._island_sliders:
            scale.set_value(float(self.values[key]))
            val.set_label(str(int(self.values[key])))
        for key, sw in self._island_switches:
            sw.set_active(bool(self.values[key]))
        for key, combo in self._island_combo_refs:
            model = combo.get_model()
            for i in range(model.n_rows()):
                if model[i]["id"] == self.values.get(key, ""):
                    combo.set_active(i)
                    break
        self._refresh_clock_preview()

    # ---------- About ----------
    def _build_about(self, page):
        card = self._card(f"Dynamite Settings v{APP_VERSION}")
        lines = [
            "GTK4, Python, no install. Controls the Rust island (dynamite-rust) "
            "on your MangoWM desktop: launcher, clipboard, music, themes, "
            "wallpapers and lockscreen.",
            f"Island config: {CONFIG_PATH}",
            f"Keyboard config: {BIND_PATH}",
            f"Groq key: ~/.config/static-dictation/api_key",
        ]
        for text in lines:
            lab = Gtk.Label(label=text, xalign=0, wrap=True)
            lab.add_css_class("row-value")
            card.append(lab)
        running = island_running()
        state = Gtk.Label(label="Island: running" if running
                          else "Island: not running", xalign=0)
        state.add_css_class("row-label")
        card.append(state)
        self._about_status = self._status_label()
        card.append(self._about_status)
        btn = Gtk.Button(label="Reload island now")
        btn.add_css_class("plain")
        btn.set_halign(Gtk.Align.START)
        btn.connect("clicked", lambda *_: self._reload_about())
        card.append(btn)
        page.append(card)
        page.append(Gtk.Box(vexpand=True))

    def _reload_about(self):
        running = island_running()
        if running:
            poke_reload()
        self.flash(self._about_status,
                   "Island reloaded." if running
                   else "Island is not running — nothing to reload.",
                   ok=running)

    # ---------- navigation ----------
    def _on_row_selected(self, _box, row):
        if row is None:
            return
        self.stack.set_visible_child_name(row.page_name)
        if row.page_name == "home":
            self._refresh_status()
        elif row.page_name == "dictation":
            self._refresh_dictation_ui()

    def _on_search(self, entry):
        text = entry.get_text().lower()
        child = self.sidebar.get_first_child()
        while child is not None:
            child.set_visible(not text or text in child.search_text)
            child = child.get_next_sibling()


def main():
    app = Gtk.Application(application_id="dev.dynamite.settings")
    app.connect("activate", lambda a: SettingsWindow(a).present())
    return app.run(sys.argv)


if __name__ == "__main__":
    sys.exit(main())
