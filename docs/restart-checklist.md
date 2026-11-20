# Session restart checklist — for when the display is back

The panel survived a compositor crash and the screen is still dead. When anyone
starts work again, run these FIRST:

1. **Restart mango yourself.** The panel (pid from pgrep -f dynamite-rust) is
   alive but the screen is frozen; only a compositor restart brings it back.
   Verify: `timeout 12 grim /tmp/p.png` returns rc 0.
2. **Rebuild and restart the panel** so the running binary matches `HEAD`:
   `cargo build --release --offline`
   kill old, `setsid nohup ./target/release/dynamite-rust </dev/null >/tmp/panel.log 2>&1 & disown`
3. **Check two instances did not stack**: `mmsg get all-layers` must list
   `dynamite-rust` once.
4. Run every gate and record rows (the one that could not run after the fix):
   - `python3 scripts/stress/stress.py` (new `load_leak` already has collapsed baseline; growth was -32.2 from the expanded-baseline artifact, now reads 0)
   - `python3 scripts/halo_check.py` (after restart, result may differ)
   - `python3 scripts/render_check.py`
   - `python3 scripts/motion/motion.py` for the pose-hold trend
   - `python3 scripts/measure.py`; `python3 scripts/scorecard.py --label renice`
5. Visual deltas still open (blocked on live screen): #2 upper-panel fill grey 9 vs ref 0, Power text 35–50% large (`src/power_panel.rs:106-110`); #3 lock capsule 1.098× wide / 0.886× tall, dot dia 1.352× (`src/lockscreen.rs:247-249,263,281-293,1131-1175`); #1 pill ~3.9% narrow (`collapsed_width` 67).

**Round 2 score: median 7 vs gate ≥9 — do NOT dispatch fixers purely by r2 minors; the median account must be re-measured on a live screen first.**
