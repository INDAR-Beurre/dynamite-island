#!/usr/bin/env python3
"""Self-test for measure.py's state witness.

This guard has now failed twice in ways that LOOKED like success:
  1. it read only 'DBG enter state=', which fires on POINTER ENTER — a panel
     driven over the socket never prints it, so the guard returned None and
     silently fell back to the pixel heuristics it exists to replace;
  2. 'morph_running=true' is printed BEFORE the animation and never cleared,
     so trusting it refuses every measurement forever.

Both are pinned here. Run after touching panel_state() or settled().
"""
import importlib.util
import pathlib
import re
import sys
import tempfile

HERE = pathlib.Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("measure", HERE / "measure.py")
measure = importlib.util.module_from_spec(spec)
spec.loader.exec_module(measure)

fails = []


def check(name, got, want):
    ok = got == want
    print(f"{'PASS' if ok else 'FAIL'}  {name}")
    if not ok:
        fails.append(f"{name}: got {got!r} want {want!r}")


def state_from(lines):
    """panel_state()'s parsing, exercised without a live panel.

    Kept in sync by calling the real regexes: we monkeypatch pid() and the
    /proc read by pointing panel_state at a temp log via its own code path.
    """
    with tempfile.TemporaryDirectory() as d:
        log = pathlib.Path(d) / "panel.log"
        log.write_text("\n".join(lines) + "\n")
        # panel_state reads /proc/<pid>/fd/2; stub the two OS touchpoints.
        orig_pid, orig_readlink = measure.pid, measure.os.readlink
        measure.pid = lambda: 999999
        measure.os.readlink = lambda _p: str(log)
        try:
            return measure.panel_state()
        finally:
            measure.pid, measure.os.readlink = orig_pid, orig_readlink


# 1. socket-driven panel: ONLY morph lines exist. Must still resolve.
s = state_from([
    "dynamite-rust: starting",
    "dynamite-rust: DBG morph Collapsed -> ControlCenter (morph_running=true)",
])
check("socket-only morph line resolves", s and s["state"], "ControlCenter")
check("  via morph witness", s and s["via"], "morph")

# 2. the latest line wins, not the first
s = state_from([
    "dynamite-rust: DBG morph Collapsed -> Music (morph_running=true)",
    "dynamite-rust: DBG morph Music -> Collapsed (morph_running=true)",
])
check("latest morph wins", s and s["state"], "Collapsed")

# 3. lock transition has a fixed target and no flag
s = state_from(["dynamite-rust: DBG morph Collapsed -> Lockscreen (morph_running=false)"])
check("lock-in resolves to Lockscreen", s and s["state"], "Lockscreen")
check("  morph false", s and s["morph"], False)

# 4. unlock line
s = state_from(["dynamite-rust: DBG morph Lockscreen -> Collapsed (morph_running=true)"])
check("unlock resolves to Collapsed", s and s["state"], "Collapsed")

# 5. pointer-enter witness still honoured when present
s = state_from(["dynamite-rust: DBG enter state=Collapsed morph=false pos=(1.0,2.0)"])
check("pointer-enter still works", s and s["state"], "Collapsed")
check("  via pointer-enter", s and s["via"], "pointer-enter")

# 6. no witness at all -> None (caller must not invent a state)
check("no witness returns None", state_from(["dynamite-rust: starting"]), None)

# 7. a morph line naming a state we do not know must not crash
s = state_from(["dynamite-rust: DBG morph Collapsed -> Whatever (morph_running=true)"])
check("unknown target passes through", s and s["state"], "Whatever")

print()
if fails:
    print(f"RESULT FAIL ({len(fails)})")
    for f in fails:
        print("  " + f)
    sys.exit(1)
print("RESULT PASS")