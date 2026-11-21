# Verify and recover the shell session

Start by checking the current session; a past outage is not a reason to restart it.

1. Check `mmsg get all-monitors`, then run `mmsg get all-layers` and
   `timeout 12 grim /tmp/dynamite-check.png`.
   An inactive monitor with zero dimensions is asleep, not proof of a compositor
   crash: wake it with normal input (or `mmsg dispatch wakeup_monitor` for verification)
   before launching the panel. Mango closes new layer surfaces when no output is active.
   If capture succeeds and the `dynamite-rust` layer is present, leave MangoWM running.
   If the compositor does not answer, stop capture attempts and let the owner recover
   the desktop. **Do not restart MangoWM, log out, or run a real locker remotely.**
2. Build the panel with `cargo build --release --offline` and check that the running
   executable matches `target/release/dynamite-rust`. Only restart the panel when it
   is stale. Use the existing supervisor if it is running; do not start a second one.
3. Confirm that `mmsg get all-layers` lists exactly one `dynamite-rust` layer and that
   the Unix datagram socket belongs to that panel process.
4. With no build or other load-heavy job running, verify:
   - `cargo test --release --offline`
   - `python3 scripts/stress/stress.py`
   - `python3 scripts/stress/mutation_gate.py`
   - `python3 scripts/halo_check.py`
   - `python3 scripts/render_check.py`
   - `python3 scripts/motion/selftest.py`
   - `python3 scripts/motion/motion.py --source wf --trials 3 --state controlcenter --burst 1.6`
   - `python3 scripts/measure.py` and `python3 scripts/scorecard.py --label recovery`

The stress suite includes transport-return assertions; its count is not a count
of distinct UI behaviors. Record the separate visual and mutation checks too.
Recorder PTS cannot establish absolute command-to-screen latency; use the grim
capture path for onset and report its sampling limitation.

The optional `upscale_region_perf` wall-time benchmark requires an idle host:
`cargo test --release --offline upscale_region_perf -- --ignored --test-threads=1 --nocapture`.
The regular suite checks region output and unchanged out-of-damage pixels without
using scheduler-dependent timing as a correctness assertion.

The Power-menu Lock button and the `lock` / `lockscreen` CLI commands start
the **real authenticated locker**; do not invoke them during remote recovery.
The explicit `lock-preview` command is only a clickable visual harness. See
the [setup and attended tests](../deploy/README.md#dynamite-session-lock).
Do not change authentication or enable lock-at-boot during recovery. For isolated regressions, use
`python3 scripts/keyboard_selftest.py` and `python3 scripts/lock_selftest.py`.
