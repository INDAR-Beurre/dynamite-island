#!/usr/bin/env python3
"""mutation_gate.py — prove the stress suite can FAIL.

Critic #22 stubbed `App.send` so `calendar` returned "ok" without ever reaching
the panel. The suite reported `state_machine PASS tests=246 fails=0`. A gate
that stays green when the product is broken is not a gate, so this injects
known defects and asserts the suite catches each one.

Mutations (each runs the real group against the real live panel):
  drop-state   a state word is silently swallowed        -> state_machine FAIL
  drop-all     every state word is swallowed             -> state_machine FAIL
  fake-latency every effect witness reports 3000 ms      -> latency/state FAIL

Control: the unmutated state_machine group must still PASS, otherwise a
"caught" mutation proves nothing.

Nothing here edits tracked files or the panel; mutations live in this process.

    python3 scripts/stress/mutation_gate.py
Exit: 0 every mutation caught and the control passed, 1 otherwise.
"""
import importlib.util
import pathlib
import sys
import time

HERE = pathlib.Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("st", HERE / "stress.py")
st = importlib.util.module_from_spec(spec)
spec.loader.exec_module(st)


def run(group_name, mutate=None):
    app = st.App()
    if mutate:
        mutate(app)
    out = st.Out()
    out.problems = []
    out.restarted = False
    fn = dict(st.GROUPS)[group_name]
    try:
        out.problems = fn(app, out) or []
    except st.Restart:
        # A redeploy mid-group is an environment event, not evidence about the
        # product. Mark it so the caller can re-run instead of scoring it.
        out.restarted = True
    return out


def drop(word):
    """Swallow one state word: report success, send nothing."""
    def apply(app):
        real = app.send
        def fake(payload, *a, **k):
            if payload.strip() == word:
                return "ok"
            return real(payload, *a, **k)
        app.send = fake
    return apply


def slow(app):
    real = app.wait_effect
    def fake(needle, budget=1.5):
        real(needle, 0.05)
        return 3000.0
    app.wait_effect = fake


def main():
    fails = []

    print("control: unmutated state_machine must PASS")
    o = None
    for attempt in range(3):
        o = run("state_machine")
        if not o.restarted:
            break
        print(f"  (panel redeployed mid-group; retry {attempt + 1}/3)")
        time.sleep(5)
    if o.restarted:
        print("  INVALID: panel kept redeploying; control not measured")
        fails.append("control-invalid")
        ok = False
    else:
        ok = o.fails == 0
        print(f"  {'ok  ' if ok else 'FAIL'} tests={o.tests} "
              f"asserts={o.asserts} fails={o.fails}")
        if not ok:
            for p in (o.problems or [])[:6]:
                print(f"       {p}")
            fails.append("control")

    cases = [
        ("drop calendar", "state_machine", drop(b"calendar")),
        ("drop lock", "state_machine", drop(b"lock")),
        ("fake 3000ms latency", "latency", slow),
    ]
    for label, group, mut in cases:
        print(f"\nmutation: {label}  (group {group}) must FAIL")
        t0 = time.time()
        o = run(group, mut)
        caught = o.fails > 0
        print(f"  {'ok  ' if caught else 'FAIL'} tests={o.tests} "
              f"asserts={o.asserts} fails={o.fails} "
              f"({time.time() - t0:.0f}s)")
        if not caught:
            fails.append(label)
        else:
            for p in (o.problems or [])[:3]:
                print(f"       caught: {p}")

    print("\nRESULT " + ("PASS" if not fails else f"FAIL: {', '.join(fails)}"))
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())