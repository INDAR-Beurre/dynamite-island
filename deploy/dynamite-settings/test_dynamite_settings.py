#!/usr/bin/env python3
"""Non-destructive tests for dynamite_settings.py.

Run:  python3 -m unittest test_dynamite_settings -v
      (python3 -m pytest test_dynamite_settings.py -v works too, if pytest exists)

Sandboxing strategy (no real user config, bind.conf, API key or system file is
ever read or written):
  * Before importing the module under test, HOME / XDG_CONFIG_HOME /
    XDG_RUNTIME_DIR are pointed at a throwaway directory under /tmp, so every
    module-level path constant (CONFIG_PATH, BIND_PATH, DICT_KEY_PATH,
    ISLAND_SOCK, ...) already points inside the sandbox.
  * Each test additionally rebinds the specific module constants it touches to
    its own per-test TemporaryDirectory, so assertions are local and exact.
  * All subprocess side effects (pgrep / pkill / nohup / sudo / mango /
    elogind-inhibit) are faked with recorders; no real process is spawned.
  * API keys used in tests are dummy strings ("gsk_test_dummy_0000").
"""

import atexit
import importlib.util
import os
import shutil
import stat
import subprocess
import sys
import tempfile
import types
import unittest
from unittest import mock

HERE = os.path.dirname(os.path.abspath(__file__))
MODULE_PATH = os.path.join(HERE, "dynamite_settings.py")

# ---------------------------------------------------------------- sandbox
_SANDBOX = tempfile.mkdtemp(prefix="dynamite-settings-test-")
os.makedirs(os.path.join(_SANDBOX, "run"), exist_ok=True)
os.environ["HOME"] = _SANDBOX
os.environ["XDG_CONFIG_HOME"] = os.path.join(_SANDBOX, ".config")
os.environ["XDG_RUNTIME_DIR"] = os.path.join(_SANDBOX, "run")
atexit.register(shutil.rmtree, _SANDBOX, True)

_spec = importlib.util.spec_from_file_location("dynamite_settings", MODULE_PATH)
ds = importlib.util.module_from_spec(_spec)
sys.modules["dynamite_settings"] = ds
_spec.loader.exec_module(ds)

DUMMY_KEY = "gsk_test_dummy_0000"


class RunRecorder:
    """Stands in for subprocess.run / Popen and records argv lists."""

    def __init__(self, returncode=0, stdout=""):
        self.calls = []
        self.returncode = returncode
        self.stdout = stdout

    def __call__(self, cmd, *args, **kwargs):
        self.calls.append([str(c) for c in cmd])
        return types.SimpleNamespace(returncode=self.returncode,
                                     stdout=self.stdout, stderr=self.stdout)


class DsTestCase(unittest.TestCase):
    """Per-test tmp dir + helpers that rebind ds module constants locally."""

    def setUp(self):
        super().setUp()
        self._tmp = tempfile.TemporaryDirectory(prefix="dset-test-")
        self.tmp = self._tmp.name
        self._restores = []

    def tearDown(self):
        for name, old in reversed(self._restores):
            setattr(ds, name, old)
        self._tmp.cleanup()

    def patch(self, name, value):
        old = getattr(ds, name)
        setattr(ds, name, value)
        self._restores.append((name, old))
        return old

    # --- per-group path patchers ---
    def patch_config(self):
        d = os.path.join(self.tmp, "xdg", "dynamite-rust")
        self.patch("CONFIG_DIR", d)
        self.patch("CONFIG_PATH", os.path.join(d, "config.conf"))
        return d

    def patch_bind(self):
        d = os.path.join(self.tmp, "mango")
        os.makedirs(d)
        p = os.path.join(d, "bind.conf")
        self.patch("BIND_PATH", p)
        return p

    def patch_key(self):
        p = os.path.join(self.tmp, "static-dictation", "api_key")
        self.patch("DICT_KEY_PATH", p)
        return p

    def patch_sock(self):
        p = os.path.join(self.tmp, "run", "dynamite-rust.sock")
        self.patch("ISLAND_SOCK", p)
        return p

    def write_bind(self, *lines):
        with open(ds.BIND_PATH, "w", encoding="utf-8") as fh:
            fh.write("\n".join(lines) + "\n")

    def read_bind(self):
        with open(ds.BIND_PATH, encoding="utf-8") as fh:
            return fh.read()

    def fake_mango(self, exit_code=0, stderr=""):
        d = os.path.join(self.tmp, "bin")
        os.makedirs(d, exist_ok=True)
        exe = os.path.join(d, "mango")
        with open(exe, "w", encoding="utf-8") as fh:
            fh.write("#!/bin/sh\n")
            if stderr:
                fh.write('echo "%s" 1>&2\n' % stderr.replace('"', ""))
            fh.write("exit %d\n" % exit_code)
        os.chmod(exe, 0o755)
        os.environ["PATH"] = d + os.pathsep + os.environ["PATH"]


# ============================================================ 1. config
class TestConfig(DsTestCase):
    def setUp(self):
        super().setUp()
        self.patch_config()

    def write_conf(self, *lines):
        os.makedirs(ds.CONFIG_DIR, exist_ok=True)
        with open(ds.CONFIG_PATH, "w", encoding="utf-8") as fh:
            fh.write("\n".join(lines) + "\n")

    def test_load_missing_file_returns_defaults(self):
        values, unknown = ds.load_config()
        self.assertEqual(values, ds.DEFAULTS)
        self.assertEqual(unknown, {})

    def test_load_parses_ints_bools_and_strings(self):
        self.write_conf(
            "# a comment",
            "bar_height=40",
            "notch_mode=1",
            "hover_open=0",
            "theme=auto",
            "custom_flag=42",
            "line without equals",
        )
        values, unknown = ds.load_config()
        self.assertEqual(values["bar_height"], 40)
        self.assertIs(values["notch_mode"], True)
        self.assertIs(values["hover_open"], False)
        self.assertEqual(values["theme"], "auto")
        self.assertEqual(values["wallpaper"], "")  # untouched default
        self.assertEqual(unknown, {"custom_flag": "42"})

    def test_load_does_not_clamp_out_of_range_values(self):
        # Documented behaviour: load_config stores the raw int; clamping to the
        # (min, max) bounds in KEYS happens in the UI sliders, not on load.
        self.write_conf("bar_height=10000")
        values, _ = ds.load_config()
        self.assertEqual(values["bar_height"], 10000)

    def test_load_bad_int_keeps_default(self):
        self.write_conf("bar_height=abc")
        values, _ = ds.load_config()
        self.assertEqual(values["bar_height"], ds.DEFAULTS["bar_height"])

    def test_load_bool_only_literal_one_is_true(self):
        self.write_conf("hover_open=true")
        self.assertEqual(ds.load_config()[0]["hover_open"], False)  # quirk: raw == "1"
        self.write_conf("hover_open=0")
        self.assertEqual(ds.load_config()[0]["hover_open"], False)
        self.write_conf("hover_open=1")
        self.assertEqual(ds.load_config()[0]["hover_open"], True)

    def test_int_keys_have_clamp_bounds_defined(self):
        # UI clamp ranges the sliders enforce (KEYS spec).
        for key, (kind, default, lo, hi, step) in ds.KEYS.items():
            if kind == "int":
                self.assertIsNotNone(lo, key)
                self.assertIsNotNone(hi, key)
                self.assertGreaterEqual(step, 1, key)
                self.assertLessEqual(lo, default, key)
                self.assertLessEqual(default, hi, key)

    def test_save_writes_all_known_keys_in_order(self):
        os.makedirs(ds.CONFIG_DIR, exist_ok=True)
        ds.save_config(dict(ds.DEFAULTS), {})
        with open(ds.CONFIG_PATH, encoding="utf-8") as fh:
            lines = fh.read().splitlines()
        self.assertEqual(lines[0],
                         "# dynamite-rust island configuration (edited by Dynamite Settings)")
        keys_in_file = [l.split("=", 1)[0] for l in lines[1:]]
        self.assertEqual(keys_in_file, ds.KEY_ORDER + ["hover_state"])
        # empty string values are left to the island's own default
        self.assertFalse(any(l.startswith("theme=") for l in lines))
        self.assertFalse(any(l.startswith("wallpaper=") for l in lines))
        self.assertIn("bar_height=28", lines)

    def test_save_creates_config_dir_and_normalizes_bools(self):
        self.assertFalse(os.path.exists(ds.CONFIG_DIR))
        values = dict(ds.DEFAULTS)
        values["notch_mode"] = True
        values["hover_open"] = False
        ds.save_config(values, {})
        self.assertTrue(os.path.isfile(ds.CONFIG_PATH))
        content = self.read_conf()
        self.assertIn("notch_mode=1", content.splitlines())
        self.assertIn("hover_open=0", content.splitlines())

    def test_save_omits_empty_strings_keeps_unknown(self):
        values = dict(ds.DEFAULTS)
        values["wallpaper"] = "wall.jpg"
        values["hover_state"] = ""
        ds.save_config(values, {"legacy": "7"})
        lines = self.read_conf().splitlines()
        self.assertIn("wallpaper=wall.jpg", lines)
        self.assertIn("legacy=7", lines)
        self.assertFalse(any(l.startswith("hover_state=") for l in lines))

    def read_conf(self):
        with open(ds.CONFIG_PATH, encoding="utf-8") as fh:
            return fh.read()

    def test_round_trip_preserves_values_and_unknown(self):
        self.write_conf(
            "bar_height=40", "expanded_height=300", "clock_24h=0",
            "theme=nord", "weird_key=11",
        )
        values, unknown = ds.load_config()
        ds.save_config(values, unknown)
        values2, unknown2 = ds.load_config()
        self.assertEqual(values, values2)
        self.assertEqual(unknown, unknown2, "unknown keys must survive a save")


# ============================================================ 2. API key
class TestApiKey(DsTestCase):
    def setUp(self):
        super().setUp()
        self.key_path = self.patch_key()

    def test_read_missing_returns_empty(self):
        self.assertEqual(ds.read_api_key(), "")

    def test_save_writes_key_with_newline_and_0600(self):
        self.assertFalse(os.path.exists(os.path.dirname(self.key_path)))
        ds.save_api_key(DUMMY_KEY)
        with open(self.key_path, encoding="utf-8") as fh:
            self.assertEqual(fh.read(), DUMMY_KEY + "\n")
        mode = stat.S_IMODE(os.stat(self.key_path).st_mode)
        self.assertEqual(mode, 0o600, "api key file must be 600, got %03o" % mode)

    def test_read_strips_whitespace(self):
        ds.save_api_key(DUMMY_KEY)
        self.assertEqual(ds.read_api_key(), DUMMY_KEY)

    def test_api_key_info_present(self):
        ds.save_api_key(DUMMY_KEY)
        info = ds.api_key_info()
        pattern = r"Key stored \(%d characters, permissions 600\)" % len(DUMMY_KEY)
        self.assertRegex(info, pattern)

    def test_api_key_info_missing(self):
        self.assertEqual(
            ds.api_key_info(),
            "No key stored yet — dictation cannot transcribe until one is added.")

    def test_clear_removes_file(self):
        ds.save_api_key(DUMMY_KEY)
        self.assertTrue(ds.clear_api_key())
        self.assertFalse(os.path.exists(self.key_path))
        self.assertEqual(ds.read_api_key(), "")

    def test_clear_missing_returns_false(self):
        self.assertFalse(ds.clear_api_key())

    def test_save_overwrites_previous_key(self):
        ds.save_api_key("old-dummy")
        ds.save_api_key(DUMMY_KEY)
        self.assertEqual(ds.read_api_key(), DUMMY_KEY)


# ============================================================ 3. keybindings
class TestBindParsers(DsTestCase):
    def setUp(self):
        super().setUp()
        self.patch_bind()

    def test_is_dynamite_line(self):
        self.assertTrue(ds.is_dynamite_line(
            "bind=SUPER,m,spawn,/opt/bin/dynamite-rust,launcher"))
        self.assertFalse(ds.is_dynamite_line("bind=SUPER,m,spawn,foot"))
        self.assertFalse(ds.is_dynamite_line(
            "mousebind=SUPER,m,spawn,/opt/bin/dynamite-rust"))
        self.assertFalse(ds.is_dynamite_line("# mentions dynamite-rust"))
        self.assertFalse(ds.is_dynamite_line(""))

    def test_parse_missing_file_returns_empty(self):
        self.assertEqual(ds.parse_dynamite_lines(), [])
        self.assertEqual(ds.parse_all_shortcuts(), [])

    def test_parse_dynamite_lines_extracts_only_them(self):
        self.write_bind(
            "# user config",
            "bind=SUPER,none,reload",
            "bind=SUPER,m,spawn,/opt/bin/dynamite-rust,launcher",
            "mousebind=btn_left,all,none",
            "bind=SUPER,t,spawn,/opt/bin/dynamite-rust,clipboard",
        )
        rows = ds.parse_dynamite_lines()
        self.assertEqual([r["combo"] for r in rows], ["SUPER+m", "SUPER+t"])
        self.assertEqual(rows[0]["cmd"], "/opt/bin/dynamite-rust,launcher")
        self.assertEqual(rows[1]["cmd"], "/opt/bin/dynamite-rust,clipboard")
        self.assertEqual(rows[0]["raw"],
                         "bind=SUPER,m,spawn,/opt/bin/dynamite-rust,launcher")

    def test_parse_none_mods_gives_bare_key(self):
        self.write_bind("bind=none,m,spawn,/opt/bin/dynamite-rust,music")
        row = ds.parse_dynamite_lines()[0]
        self.assertEqual(row["combo"], "m")
        self.assertEqual(row["cmd"], "/opt/bin/dynamite-rust,music")

    def test_norm_combo(self):
        cases = {
            "SUPER + m": "SUPER,m",
            "m": "none,m",
            "SHIFT+NONE+F2": "SHIFT,F2",
            "super+m": "SUPER,m",
            "none+t": "none,t",
            "SUPER": None,        # modifiers but no key
            "": None,
        }
        for text, expected in cases.items():
            self.assertEqual(ds.norm_combo(text), expected, text)

    def test_build_shortcuts(self):
        lines = ds.build_shortcuts([
            {"combo": "SUPER+m", "cmd": "/opt/bin/dynamite-rust,launcher"},
            {"combo": "SHIFT+n", "cmd": "  /opt/bin/dynamite-rust,music  "},
            {"combo": "SUPER", "cmd": "/opt/bin/dynamite-rust"},      # no key -> skipped
            {"combo": "CTRL+x", "cmd": "   "},                        # empty cmd -> skipped
        ])
        self.assertEqual(lines[0],
                         "# Dynamite shortcuts (managed by Dynamite Settings)")
        self.assertEqual(lines[1],
                         "bind=SUPER,m,spawn,/opt/bin/dynamite-rust,launcher")
        self.assertEqual(lines[2],
                         "bind=SHIFT,n,spawn,/opt/bin/dynamite-rust,music")
        self.assertEqual(len(lines), 3)

    def test_pretty_combo(self):
        # current API: input is '+'-separated mods (new bind.conf format)
        self.assertEqual(ds.pretty_combo("SUPER+m"), "Super + m")
        self.assertEqual(ds.pretty_combo("ALT+SHIFT+F4"), "Alt + Shift + F4")
        self.assertEqual(ds.pretty_combo("none"), "")
        self.assertEqual(ds.pretty_combo("SUPER"), "Super")

    def test_parse_all_shortcuts_unambiguous_rows(self):
        self.write_bind(
            "# comment only",
            "bind=none,btn_left,spawn,foot",
            "mousebind=btn_left,all,none",
            "bind=SUPER,none,reload",
        )
        rows = ds.parse_all_shortcuts()
        self.assertEqual(len(rows), 3, "one row per bind/mouse/gesture line")
        by_keycap = {k: d for k, d in rows}
        self.assertEqual(by_keycap.get("left click"), "Terminal")
        self.assertEqual(by_keycap.get("btn_left + all"), "all none")

    def test_parse_build_round_trip(self):
        self.write_bind("bind=SUPER,m,spawn,/opt/bin/dynamite-rust,launcher")
        rows = ds.parse_dynamite_lines()
        rebuilt = ds.build_shortcuts(rows)
        self.assertIn(
            "bind=SUPER,m,spawn,/opt/bin/dynamite-rust,launcher", rebuilt[1:])
        self.assertEqual(ds.parse_dynamite_lines(), rows,
                         "parse(build(parse(file))) must be stable")


class TestSaveShortcuts(DsTestCase):
    ORIGINAL = (
        "# user config",
        "bind=SUPER,none,reload",
        "bind=SUPER,none,spawn,foot",
        "mousebind=btn_left,all,none",
        "bind=SUPER,m,spawn,/opt/bin/dynamite-rust,launcher",
        "bind=SUPER,t,spawn,/opt/bin/dynamite-rust,clipboard",
        "gesturebind=none,btn_left,swipe-up,spawn,waterfox",
    )

    def setUp(self):
        super().setUp()
        self.patch_bind()
        self.write_bind(*self.ORIGINAL)
        self.rows = [
            {"combo": "SUPER+m", "cmd": "/opt/bin/dynamite-rust,launcher"},
            {"combo": "SHIFT+k", "cmd": "/opt/bin/dynamite-rust,dictation"},
        ]

    def test_save_preserves_every_unrelated_line(self):
        self.fake_mango(exit_code=0)
        result = ds.save_shortcuts(self.rows)
        self.assertTrue(result.startswith("Saved — bind.conf updated"), result)
        lines = self.read_bind().splitlines()
        # every non-dynamite line survives verbatim
        for line in self.ORIGINAL:
            if not ds.is_dynamite_line(line):
                self.assertIn(line, lines, "lost unrelated line: %r" % line)
        # the dynamite block is rebuilt
        self.assertIn("bind=SUPER,m,spawn,/opt/bin/dynamite-rust,launcher", lines)
        self.assertIn("bind=SHIFT,k,spawn,/opt/bin/dynamite-rust,dictation", lines)
        # and the old dynamite lines are gone (except the one rewritten identically)
        self.assertNotIn("bind=SUPER,t,spawn,/opt/bin/dynamite-rust,clipboard", lines)
        self.assertEqual(lines[0], "# user config")
        # permissions set to 644, no leftover temp file
        mode = stat.S_IMODE(os.stat(ds.BIND_PATH).st_mode)
        self.assertEqual(mode, 0o644, "bind.conf must be 644, got %03o" % mode)
        leftovers = [f for f in os.listdir(os.path.dirname(ds.BIND_PATH))
                     if f.endswith(".conf") and f != "bind.conf"]
        self.assertEqual(leftovers, [])

    def test_save_appends_block_when_no_dynamite_lines_exist(self):
        self.write_bind("# fresh file", "bind=SUPER,none,reload")
        self.fake_mango(exit_code=0)
        result = ds.save_shortcuts(self.rows)
        self.assertTrue(result.startswith("Saved"), result)
        lines = self.read_bind().splitlines()
        self.assertEqual(lines[:2], ["# fresh file", "bind=SUPER,none,reload"])
        self.assertEqual(lines[2],
                         "# Dynamite shortcuts (managed by Dynamite Settings)")

    def test_mango_rejection_leaves_file_untouched(self):
        before = self.read_bind()
        self.fake_mango(exit_code=1, stderr="bad combo SUPER+x")
        result = ds.save_shortcuts(self.rows)
        self.assertEqual(
            result,
            "mango rejected the new config (bad combo SUPER+x) — nothing was changed.")
        self.assertEqual(self.read_bind(), before, "rejected save must not write")
        leftovers = [f for f in os.listdir(os.path.dirname(ds.BIND_PATH))
                     if f.endswith(".conf") and f != "bind.conf"]
        self.assertEqual(leftovers, [], "temp validation file must be unlinked")

    def test_no_mango_skips_validation(self):
        bin_dir = os.path.join(self.tmp, "empty-bin")
        os.makedirs(bin_dir)
        old = os.environ["PATH"]
        os.environ["PATH"] = bin_dir  # no mango, no sh needed by which()
        try:
            result = ds.save_shortcuts(self.rows)
        finally:
            os.environ["PATH"] = old
        self.assertEqual(
            result, "Saved — bind.conf updated (mango not found; validation skipped).")
        self.assertIn("bind=SHIFT,k,spawn,/opt/bin/dynamite-rust,dictation",
                      self.read_bind().splitlines())

    def test_missing_bind_conf_reports_not_found(self):
        os.remove(ds.BIND_PATH)
        self.assertEqual(ds.save_shortcuts(self.rows),
                         "bind.conf not found — nothing saved.")


# ============================================================ 4. health / deps
class TestHealth(DsTestCase):
    def test_island_running_reflects_socket_file(self):
        sock = self.patch_sock()
        self.assertFalse(ds.island_running())
        os.makedirs(os.path.dirname(sock), exist_ok=True)
        with open(sock, "w"):
            pass
        self.assertTrue(ds.island_running())

    def test_poke_reload_only_when_running(self):
        sock = self.patch_sock()
        rec = RunRecorder()
        with mock.patch.object(ds.subprocess, "run", rec):
            ds.poke_reload()
            self.assertEqual(rec.calls, [], "no reload attempt when not running")
            os.makedirs(os.path.dirname(sock), exist_ok=True)
            with open(sock, "w"):
                pass
            ds.poke_reload()
            self.assertEqual(rec.calls, [[ds.ISLAND_BIN, "reload"]])

    def test_dictation_running_reflects_pgrep(self):
        for rc, expected in ((0, True), (1, False)):
            rec = RunRecorder(returncode=rc)
            with mock.patch.object(ds.subprocess, "run", rec):
                self.assertEqual(ds.dictation_running(), expected)
            self.assertEqual(rec.calls, [["pgrep", "-f", ds.DICT_PROC]])

    def test_dictation_supervisor_running_reflects_pgrep(self):
        rec = RunRecorder(returncode=0)
        with mock.patch.object(ds.subprocess, "run", rec):
            self.assertTrue(ds.dictation_supervisor_running())
        self.assertEqual(rec.calls, [["pgrep", "-f", "dictation_supervisor.sh"]])

    def test_restart_dictation_binary_pkills_and_restarts(self):
        sup = os.path.join(self.tmp, "dictation_supervisor.sh")
        with open(sup, "w"):
            pass
        self.patch("DICT_SUPERVISOR", sup)
        run, popen = RunRecorder(returncode=1), RunRecorder()
        with mock.patch.object(ds.subprocess, "run", run), \
             mock.patch.object(ds.subprocess, "Popen", popen):
            ds.restart_dictation_binary()
        self.assertEqual(run.calls[0], ["pkill", "-f", ds.DICT_PROC])
        # start_dictation_service then probes both processes (same recorder)
        self.assertEqual(run.calls[1], ["pgrep", "-f", "dictation_supervisor.sh"])
        self.assertEqual(run.calls[2], ["pgrep", "-f", ds.DICT_PROC])
        self.assertEqual(popen.calls, [["nohup", "bash", sup]],
                         "supervisor present -> restart via the supervisor")

    def test_start_service_noop_when_already_running(self):
        popen = RunRecorder()
        with mock.patch.object(ds.subprocess, "run", RunRecorder(returncode=0)), \
             mock.patch.object(ds.subprocess, "Popen", popen):
            ds.start_dictation_service()
        self.assertEqual(popen.calls, [])

    def test_start_service_falls_back_to_binary(self):
        self.patch("DICT_SUPERVISOR", os.path.join(self.tmp, "nope.sh"))
        self.patch("DICT_BIN", os.path.join(self.tmp, "static-dictation"))
        run, popen = RunRecorder(returncode=1), RunRecorder()
        with mock.patch.object(ds.subprocess, "run", run), \
             mock.patch.object(ds.subprocess, "Popen", popen):
            ds.start_dictation_service()
        self.assertEqual(popen.calls, [["nohup", os.path.join(self.tmp,
                                                              "static-dictation")]])

    def test_caffeinate_active(self):
        with mock.patch.object(ds.subprocess, "run",
                               RunRecorder(stdout="handle-lid-switch")):
            self.assertTrue(ds.caffeinate_active())
        with mock.patch.object(ds.subprocess, "run", RunRecorder(stdout="none")):
            self.assertFalse(ds.caffeinate_active())
        with mock.patch.object(ds.subprocess, "run",
                               lambda *a, **k: (_ for _ in ()).throw(FileNotFoundError)):
            self.assertFalse(ds.caffeinate_active(), "missing tool -> inactive")

    def _caffeinate_sandbox(self):
        dropin = os.path.join(self.tmp, "lid.conf")
        disabled = dropin + ".disabled"
        self.patch("LID_DROPIN", dropin)
        self.patch("LID_DROPIN_DISABLED", disabled)
        self.patch("CAFFEINATE_BIN", os.path.join(self.tmp, "caffeinate"))
        return dropin, disabled

    def _state(self, results):
        it = {"i": 0}

        def fake():
            idx = min(it["i"], len(results) - 1)
            it["i"] += 1
            return results[idx]
        return fake

    def test_set_caffeinate_on_spawns_and_restores_dropin(self):
        dropin, disabled = self._caffeinate_sandbox()
        with open(disabled, "w"):
            pass
        run, popen = RunRecorder(), RunRecorder()
        with mock.patch.object(ds.subprocess, "run", run), \
             mock.patch.object(ds.subprocess, "Popen", popen), \
             mock.patch.object(ds, "caffeinate_active", self._state([False, True])), \
             mock.patch.object(ds.time, "sleep", lambda s: None):
            self.assertTrue(ds.set_caffeinate(True))
        self.assertEqual(run.calls, [["sudo", "-n", "mv", disabled, dropin]],
                         "disabled drop-in restored first")
        self.assertEqual(popen.calls, [[ds.CAFFEINATE_BIN]])

    def test_set_caffeinate_off_stops_and_disables_dropin(self):
        dropin, disabled = self._caffeinate_sandbox()
        with open(dropin, "w"):
            pass
        run, popen = RunRecorder(), RunRecorder()
        with mock.patch.object(ds.subprocess, "run", run), \
             mock.patch.object(ds.subprocess, "Popen", popen), \
             mock.patch.object(ds, "caffeinate_active", self._state([False])), \
             mock.patch.object(ds.time, "sleep", lambda s: None):
            self.assertFalse(ds.set_caffeinate(False))
        self.assertEqual(run.calls[0], [ds.CAFFEINATE_BIN, "stop"])
        self.assertEqual(run.calls[1], ["sudo", "-n", "mv", dropin, disabled])
        self.assertEqual(popen.calls, [])


class TestDependencies(DsTestCase):
    # mirrors the tool list rendered by SettingsWindow._deps_card
    TOOLS = ["mmsg", "wpctl", "brightnessctl", "playerctl",
             "swaync-client", "cliphist", "wtype", "mango"]

    def test_dependency_check_mechanism(self):
        for name in self.TOOLS:
            self.assertIsInstance(shutil.which(name), (str, type(None)), name)

    def test_module_still_checks_the_same_tool_list(self):
        # drift guard between the UI card and this test's expectations
        with open(MODULE_PATH, encoding="utf-8") as fh:
            source = fh.read()
        body = source.split("def _deps_card", 1)[1].split("\n    def ", 1)[0]
        for name in self.TOOLS:
            self.assertIn(name, body, name)


# ============================================================ 5. syntax
class TestSyntax(unittest.TestCase):
    def test_module_compiles(self):
        r = subprocess.run([sys.executable, "-m", "py_compile", MODULE_PATH],
                           capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, r.stderr)

    def test_test_file_compiles(self):
        here = os.path.abspath(__file__)
        r = subprocess.run([sys.executable, "-m", "py_compile", here],
                          capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, r.stderr)


if __name__ == "__main__":
    unittest.main(verbosity=2)
