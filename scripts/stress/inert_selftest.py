#!/usr/bin/env python3
"""Ground truth for _inert_batch: a guilty payload must be NAMED, not blamed
on its whole batch.

The old form added len(batch) to fails and printed "at least one was
executed", so one bad payload indicted 282 innocents and the report never
said which. That is a gate that cannot direct a fix. These fixtures pin the
new behaviour without needing a live panel.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import stress  # noqa: E402


class FakeApp:
    """Records what was sent since log_mark(); `guilty` is the set that reacts."""

    def __init__(self, guilty):
        self.guilty = set(guilty)
        self.since_mark = []
        self.guarded = 0
        self.witnesses = 0

    def drain(self, *a, **k):
        return True

    def log_mark(self):
        self.since_mark = []

    def send_drained(self, payload, **k):
        self.since_mark.append(payload)
        return "ok"

    def guard(self, ctx=""):
        self.guarded += 1


class Out:
    def __init__(self):
        self.tests = 0
        self.asserts = 0
        self.fails = 0


def run(guilty, payloads):
    app = FakeApp(guilty)
    out = Out()
    problems = []
    real = stress._log_reacted

    def fake_reacted(a, budget=0.6):
        a.witnesses += 1
        return any(p in a.guilty for p in a.since_mark)

    stress._log_reacted = fake_reacted
    try:
        stress._inert_batch(app, out, problems, payloads, "fixture")
    finally:
        stress._log_reacted = real
    return app, out, problems


def main():
    checks = []

    def ck(name, cond, detail=""):
        checks.append((name, cond, detail))

    batch = [b"w%03d" % i for i in range(282)]

    # 1. clean batch: one witness, zero failures, every payload counted
    app, out, probs = run(set(), batch)
    ck("clean_batch_no_fails", out.fails == 0, str(probs))
    ck("clean_batch_one_witness", app.witnesses == 1, str(app.witnesses))
    ck("clean_batch_counts_all", out.tests == 282 and out.asserts == 282,
       f"tests={out.tests} asserts={out.asserts}")

    # 2. one guilty payload in the middle: exactly ONE failure, and it is named
    culprit = batch[137]
    app, out, probs = run({culprit}, batch)
    ck("single_culprit_one_fail", out.fails == 1, f"fails={out.fails} {probs}")
    ck("single_culprit_named",
       len(probs) == 1 and repr(culprit[:48]) in probs[0], str(probs))
    ck("single_culprit_cheap", app.witnesses <= 20, str(app.witnesses))

    # 3. first and last positions (bisection edge cases)
    for idx in (0, 281):
        app, out, probs = run({batch[idx]}, batch)
        ck(f"edge_culprit_{idx}",
           out.fails == 1 and repr(batch[idx][:48]) in probs[0], str(probs))

    # 4. two guilty payloads: both named, two failures
    two = {batch[3], batch[200]}
    app, out, probs = run(two, batch)
    ck("two_culprits_both_named",
       out.fails == 2 and all(repr(p[:48]) in " ".join(probs) for p in two),
       str(probs))

    # 5. single-element batch
    app, out, probs = run({b"solo"}, [b"solo"])
    ck("singleton_named", out.fails == 1 and b"solo" in probs[0].encode(),
       str(probs))

    bad = [c for c in checks if not c[1]]
    for name, ok, detail in checks:
        print(f"{'PASS' if ok else 'FAIL'} {name}" + (f"  {detail}" if not ok else ""))
    print(f"\n{len(checks) - len(bad)}/{len(checks)} checks pass")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
