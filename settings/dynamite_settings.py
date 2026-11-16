#!/usr/bin/env python3
"""Dynamite Settings — companion settings app for the dynamite-rust island.

Reads/writes ~/.config/dynamite-rust/config.conf (key=value) and pokes the
island's socket remote with the "reload" argument after every save.
Python 3 + GTK4 (PyGObject), no build step, on-demand only.
"""

import os
import subprocess
import sys
import time

import gi

gi.require_version("Gtk", "4.0")
from gi.repository import GLib, Gtk  # noqa: E402

APP_VERSION = "1.0"
ISLAND_BIN = os.path.expanduser("~/rust-island-build/target/release/dynamite-rust")
ISLAND_SOCK = os.path.join(
    os.environ.get("XDG_RUNTIME_DIR", "/run/user/1000"), "dynamite-rust.sock"
)

# key -> (kind, default, min, max, step)   kind: "int" slider or "bool" switch
KEYS = {
    "notch_mode": ("bool", 0, None, None, None),
    "bar_height": ("int", 24, 16, 48, 1),
    "collapsed_width": ("int", 76, 40, 300, 1),
    "expanded_height": ("int", 150, 60, 400, 1),
    "gap_top": ("int", 3, 0, 60, 1),
    "inner_padding": ("int", 14, 0, 40, 1),
    # iter139 step 11 (#8): default was 400, which re-poisoned a reset
    # config; the island's live default is 160 (Q1 still open — 160 is
    # the plan's recommendation, matching live, not a decision for 200).
    "anim_ms": ("int", 160, 0, 1000, 10),
    "hover_open": ("bool", 1, None, None, None),
    "scroll_cycle": ("bool", 1, None, None, None),
    "clock_24h": ("bool", 1, None, None, None),
}
KEY_ORDER = [
    "notch_mode",
    "bar_height",
    "collapsed_width",
    "expanded_height",
    "gap_top",
    "inner_padding",
    "anim_ms",
    "hover_open",
    "scroll_cycle",
    "clock_24h",
]

CONFIG_PATH = os.path.join(
    GLib.get_user_config_dir(), "dynamite-rust", "config.conf"
)

CAFFEINATE_BIN = os.path.expanduser("~/.local/bin/caffeinate")
LID_DROPIN = "/etc/elogind/logind.conf.d/90-lid-ignore.conf"
LID_DROPIN_DISABLED = LID_DROPIN + ".disabled"


def caffeinate_active():
    """True if a handle-lid-switch inhibitor is currently held.

    Same check as `caffeinate status`: look for handle-lid-switch in
    elogind-inhibit --list.
    """
    try:
        out = subprocess.run(
            ["elogind-inhibit", "--list"],
            capture_output=True, text=True, timeout=5, check=False,
        ).stdout
    except Exception:
        return False
    return "handle-lid-switch" in out


def _wait_caffeinate_state(want, tries=10):
    """Poll until the inhibitor state matches `want` (or give up)."""
    for _ in range(tries):
        if caffeinate_active() == want:
            return True
        time.sleep(0.3)
    return caffeinate_active() == want


def set_caffeinate(on):
    """Drive caffeinate to the requested state; return the REAL state.

    ON: restore the elogind drop-in if a previous OFF renamed it to
    .disabled, then start the inhibitor detached. OFF: stop the
    inhibitor (what restores suspend-on-close in this session), then
    rename the drop-in to .disabled so a reboot does not silently
    re-ignore the lid.
    """
    try:
        if on:
            if not os.path.exists(LID_DROPIN) and os.path.exists(LID_DROPIN_DISABLED):
                subprocess.run(
                    ["sudo", "-n", "mv", LID_DROPIN_DISABLED, LID_DROPIN],
                    capture_output=True, timeout=10, check=False,
                )
            if not caffeinate_active():
                subprocess.Popen(
                    ["setsid", "nohup", CAFFEINATE_BIN],
                    stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL, start_new_session=True,
                )
                _wait_caffeinate_state(True)
        else:
            subprocess.run(
                [CAFFEINATE_BIN, "stop"],
                capture_output=True, timeout=15, check=False,
            )
            _wait_caffeinate_state(False)
            if os.path.exists(LID_DROPIN):
                subprocess.run(
                    ["sudo", "-n", "mv", LID_DROPIN, LID_DROPIN_DISABLED],
                    capture_output=True, timeout=10, check=False,
                )
    except Exception:
        pass
    return caffeinate_active()

CSS = """
window { background: #0A0F14; }
.sidebar { background: #0D1219; }
.sidebar row { border-radius: 8px; padding: 7px 10px; margin: 1px 8px; }
.sidebar row:selected { background: #1B2432; }
.sidebar row label { color: #C9D1D9; font-size: 13px; }
.sidebar row:selected label { color: #FFFFFF; }
.icon-glyph { font-family: "Material Symbols Rounded"; font-size: 17px;
              color: #8FA6F5; }
.search { background: #161C26; color: #E6EAF0; border-radius: 8px;
          border: none; margin: 10px 10px 6px 10px; padding: 6px 8px; }
.header-card { background: #11161F; border-radius: 12px; padding: 22px 16px; }
.header-icon { font-family: "Material Symbols Rounded"; font-size: 30px;
               color: #8FA6F5; background: #1D2735; border-radius: 999px;
               padding: 12px 14px; }
.header-title { font-size: 17px; font-weight: bold; color: #FFFFFF; }
.header-sub { font-size: 12px; color: #9AA0A6; }
.row-label { font-size: 13px; color: #E6EAF0; }
.row-value { font-size: 12px; color: #9AA0A6; }
.hint { font-size: 11px; color: #6E7681; }
scale { margin: 2px 0 6px 0; }
scale trough { background: #232B38; border-radius: 999px; min-height: 6px; }
scale highlight { background: #8FA6F5; border-radius: 999px; }
scale slider { background: #FFFFFF; border-radius: 999px;
               min-width: 14px; min-height: 14px; margin: -4px;
               box-shadow: 0 1px 3px rgba(0,0,0,0.5); }
switch { background: #2A3342; border-radius: 999px; border: none; }
switch:checked { background: #3B82F6; }
switch slider { background: #FFFFFF; border-radius: 999px; }
.page { padding: 18px 22px; }
button.reload { background: #1B2432; color: #E6EAF0; border-radius: 8px;
                border: none; padding: 7px 14px; }
button.reload:hover { background: #232E40; }
"""


def load_config():
    """Return dict of values (defaults filled in) + any unknown keys kept."""
    values = {k: spec[1] for k, spec in KEYS.items()}
    unknown = {}
    try:
        with open(CONFIG_PATH, "r", encoding="utf-8") as fh:
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
                else:
                    unknown[key] = raw
    except FileNotFoundError:
        pass
    return values, unknown


def save_config(values, unknown):
    os.makedirs(os.path.dirname(CONFIG_PATH), exist_ok=True)
    lines = ["# dynamite-rust island configuration (edited by Dynamite Settings)"]
    for key in KEY_ORDER:
        kind = KEYS[key][0]
        val = values[key]
        lines.append(f"{key}={int(val) if kind == 'int' else (1 if val else 0)}")
    for key, raw in unknown.items():
        lines.append(f"{key}={raw}")
    with open(CONFIG_PATH, "w", encoding="utf-8") as fh:
        fh.write("\n".join(lines) + "\n")


def poke_reload():
    """Ask the running island to reload its config. Silent if not running."""
    if not os.path.exists(ISLAND_SOCK):
        return
    try:
        subprocess.run(
            [ISLAND_BIN, "reload"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=3,
            check=False,
        )
    except Exception:
        pass


class SettingsWindow(Gtk.ApplicationWindow):
    def __init__(self, app):
        super().__init__(application=app, title="Dynamite Settings")
        self.set_default_size(700, 540)
        self.values, self.unknown = load_config()
        self._save_source = 0
        self._building = True

        provider = Gtk.CssProvider()
        provider.load_from_data(CSS.encode())
        Gtk.StyleContext.add_provider_for_display(
            self.get_display(), provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
        )

        root = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        self.set_child(root)

        # ---- sidebar ----
        sidebar_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        sidebar_box.add_css_class("sidebar")
        sidebar_box.set_size_request(205, -1)
        root.append(sidebar_box)

        self.search = Gtk.SearchEntry(placeholder_text="Search Settings")
        self.search.add_css_class("search")
        self.search.connect("search-changed", self._on_search)
        sidebar_box.append(self.search)

        self.sidebar = Gtk.ListBox()
        self.sidebar.add_css_class("sidebar")
        self.sidebar.set_selection_mode(Gtk.SelectionMode.SINGLE)
        self.sidebar.connect("row-selected", self._on_row_selected)
        sidebar_box.append(self.sidebar)

        # ---- pages ----
        self.stack = Gtk.Stack()
        self.stack.set_hexpand(True)
        self.stack.set_transition_type(Gtk.StackTransitionType.CROSSFADE)
        self.stack.set_transition_duration(120)
        root.append(self.stack)

        self._pages = {}
        self._add_page(
            "bar",
            "dashboard",
            "Bar & Island",
            "Shape and size of the island and the notch.",
            [
                ("switch", "notch_mode", "Notch mode",
                 "Off is today's rendering; on has no visual change yet."),
                ("slider", "bar_height", "Bar height", "px"),
                ("switch", "hover_open", "Open menu on hover",
                 "Hovering the pill opens the menu directly."),
                ("slider", "collapsed_width", "Collapsed width", "px"),
                ("slider", "expanded_height", "Expanded height", "px"),
                ("slider", "gap_top", "Gap from screen edge", "px"),
                ("slider", "inner_padding", "Inner padding", "px"),
            ],
            extra=self._caffeinate_row,
        )
        self._add_page(
            "motion",
            "animation",
            "Motion",
            "How the island moves between its menus.",
            [
                ("slider", "anim_ms", "Animation duration", "ms"),
                ("switch", "scroll_cycle", "Scroll cycles menus",
                 "Mouse wheel over the island steps through the menus."),
            ],
        )
        self._add_page(
            "clock",
            "schedule",
            "Clock & Date",
            "How the collapsed pill shows the time.",
            [
                ("switch", "clock_24h", "24-hour clock",
                 "Off shows 12-hour time with AM/PM."),
            ],
            extra=self._clock_preview,
        )
        self._add_page(
            "about",
            "info",
            "About",
            "This settings app and the island it configures.",
            [],
            extra=self._about_body,
        )

        for name in ("bar", "motion", "clock", "about"):
            row = self._sidebar_row(self._pages[name][0], self._pages[name][1])
            row.page_name = name
            self.sidebar.append(row)
        self.sidebar.select_row(self.sidebar.get_row_at_index(0))
        self._building = False

    # ---------- construction helpers ----------
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

    def _add_page(self, name, glyph, title, subtitle, controls, extra=None):
        page = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=14)
        page.add_css_class("page")

        card = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=5)
        card.add_css_class("header-card")
        card.set_halign(Gtk.Align.FILL)
        icon = Gtk.Label(label=glyph)
        icon.add_css_class("header-icon")
        icon.set_halign(Gtk.Align.CENTER)
        card.append(icon)
        t = Gtk.Label(label=title)
        t.add_css_class("header-title")
        card.append(t)
        s = Gtk.Label(label=subtitle)
        s.add_css_class("header-sub")
        card.append(s)
        page.append(card)

        for ctrl in controls:
            page.append(self._make_control(ctrl))
        if extra is not None:
            page.append(extra())
        page.append(Gtk.Box(vexpand=True))  # spacer
        hint = Gtk.Label(label="Changes are saved and applied automatically.",
                         xalign=0)
        hint.add_css_class("hint")
        page.append(hint)

        scroller = Gtk.ScrolledWindow()
        scroller.set_child(page)
        scroller.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        self.stack.add_named(scroller, name)
        self._pages[name] = (glyph, title)

    def _make_control(self, ctrl):
        kind, key, label = ctrl[0], ctrl[1], ctrl[2]
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        top = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        lab = Gtk.Label(label=label, xalign=0, hexpand=True)
        lab.add_css_class("row-label")
        top.append(lab)
        box.append(top)

        if kind == "slider":
            unit = ctrl[3]
            _, _, lo, hi, step = KEYS[key]
            val = Gtk.Label(label=f"{int(self.values[key])} {unit}")
            val.add_css_class("row-value")
            top.append(val)
            adj = Gtk.Adjustment(value=float(self.values[key]), lower=lo,
                                 upper=hi, step_increment=step,
                                 page_increment=step * 10)
            scale = Gtk.Scale(orientation=Gtk.Orientation.HORIZONTAL,
                              adjustment=adj)
            scale.set_draw_value(False)
            scale.set_hexpand(True)
            scale.connect("value-changed", self._on_slider, key, val, unit)
            box.append(scale)
        else:  # switch
            sw = Gtk.Switch()
            sw.set_active(bool(self.values[key]))
            sw.set_valign(Gtk.Align.CENTER)
            sw.connect("notify::active", self._on_switch, key)
            top.append(sw)
            if len(ctrl) > 3:
                hint = Gtk.Label(label=ctrl[3], xalign=0)
                hint.add_css_class("hint")
                box.append(hint)
        return box

    def _caffeinate_row(self):
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        top = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        lab = Gtk.Label(
            label="Caffeinate — keep running with the lid closed",
            xalign=0, hexpand=True,
        )
        lab.add_css_class("row-label")
        top.append(lab)
        self._caffeinate_syncing = False
        sw = Gtk.Switch()
        sw.set_valign(Gtk.Align.CENTER)
        top.append(sw)
        self.caffeinate_switch = sw
        box.append(top)
        self.caffeinate_status = Gtk.Label(xalign=0)
        self.caffeinate_status.add_css_class("hint")
        box.append(self.caffeinate_status)
        self._sync_caffeinate_ui(caffeinate_active())
        sw.connect("notify::active", self._on_caffeinate_switch)
        return box

    def _sync_caffeinate_ui(self, active):
        self._caffeinate_syncing = True
        self.caffeinate_switch.set_active(active)
        self._caffeinate_syncing = False
        self.caffeinate_status.set_label(
            "Lid close is ignored — machine keeps running"
            if active
            else "Lid close suspends (default)"
        )

    def _on_caffeinate_switch(self, switch, _pspec):
        if self._caffeinate_syncing:
            return
        real = set_caffeinate(bool(switch.get_active()))
        self._sync_caffeinate_ui(real)

    def _clock_preview(self):
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        lab = Gtk.Label(label="Preview", xalign=0)
        lab.add_css_class("row-label")
        box.append(lab)
        self.preview = Gtk.Label(xalign=0)
        self.preview.add_css_class("row-value")
        box.append(self.preview)
        self._refresh_preview()
        return box

    def _about_body(self):
        running = os.path.exists(ISLAND_SOCK)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        for text in (
            f"Dynamite Settings v{APP_VERSION}",
            "Island: Rust replica (dynamite-rust) for mangoWM",
            f"Status: {'running' if running else 'not running'}",
            f"Config: {CONFIG_PATH}",
        ):
            lab = Gtk.Label(label=text, xalign=0)
            lab.add_css_class("row-label" if text.startswith("Dynamite") else "row-value")
            lab.set_wrap(True)
            box.append(lab)
        btn = Gtk.Button(label="Reload island now")
        btn.add_css_class("reload")
        btn.set_halign(Gtk.Align.START)
        btn.connect("clicked", lambda *_: poke_reload())
        box.append(btn)
        return box

    # ---------- behaviour ----------
    def _refresh_preview(self):
        if not hasattr(self, "preview"):
            return
        now = GLib.DateTime.new_now_local()
        fmt = "%H:%M" if self.values.get("clock_24h", 1) else "%I:%M %p"
        self.preview.set_label(f"Pill clock: {now.format(fmt)}")

    def _on_row_selected(self, _box, row):
        if row is not None:
            self.stack.set_visible_child_name(row.page_name)

    def _on_search(self, entry):
        text = entry.get_text().lower()
        child = self.sidebar.get_first_child()
        while child is not None:
            child.set_visible(not text or text in child.search_text)
            child = child.get_next_sibling()

    def _on_slider(self, scale, key, val_label, unit):
        v = int(round(scale.get_value()))
        val_label.set_label(f"{v} {unit}")
        self.values[key] = v
        self._schedule_save()

    def _on_switch(self, switch, _pspec, key):
        self.values[key] = bool(switch.get_active())
        if key == "clock_24h":
            self._refresh_preview()
        self._schedule_save()

    def _schedule_save(self):
        if self._building:
            return
        if self._save_source:
            GLib.source_remove(self._save_source)
        self._save_source = GLib.timeout_add(350, self._save_now)

    def _save_now(self):
        self._save_source = 0
        save_config(self.values, self.unknown)
        poke_reload()
        return GLib.SOURCE_REMOVE


def main():
    app = Gtk.Application(application_id="dev.dynamite.settings")
    app.connect("activate", lambda a: SettingsWindow(a).present())
    return app.run(sys.argv)


if __name__ == "__main__":
    sys.exit(main())
