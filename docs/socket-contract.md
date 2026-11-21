# Control-socket contract (do not break)

`$XDG_RUNTIME_DIR/dynamite-rust.sock` — AF_UNIX **SOCK_DGRAM**, one word per
datagram. A separate live binary depends on these exact literals, so a fixer
must treat them as a frozen public API.

## CLI locking versus renderer test states

The public `dynamite-rust lock` and `lockscreen` commands launch the authenticated
Hyprlock/Howdy backend; the Power-menu Lock button uses the same launcher.
`dynamite-rust lock-preview` explicitly opens the visual harness. Legacy raw
`lockscreen` / `unlock` datagrams remain renderer-test states for the existing
regression harness. They neither acquire nor release Hyprlock's session lock.
Never substitute a preview datagram for the real lock command.

## The external producer

`tools/dictation/` builds `static-dictation`, the dictation tool the user asked
to have "embedded nicely into the package". It is a **separate process** that
talks to the panel only through this socket. It now lives in `tools/dictation/`
in this repo; the old `~/.t3/scratch/…` path is gone and is **not** a fallback.

| literal | producer | note |
|---|---|---|
| `dictation_start` | `tools/dictation/src/main.rs:38` `dynamite_send("dictation_start")` | on hotkey press |
| `dictation_end` | `tools/dictation/src/main.rs:42` `dynamite_send("dictation_end")` | on release |
| `dictation_bars:<v> <v> …` | `tools/dictation/src/main.rs:132` | **one datagram per microphone frame**, values formatted `{v:.3}`, space-separated |
| `collapsed` | `tools/dictation/src/pill/tsar.rs:129` | **DEAD CODE — not emitted by the shipped binary.** `src/main.rs:67` sets `pill = None` and `try_spawn()` is `#[cfg(test)]`-only; `grep -ac collapsed target/release/static-dictation` → 0. Kept in this table so nobody revives it by accident; the panel owns its own collapsed state. |

The bar payload is built as
`format!("dictation_bars:{}", sim.bars().iter().map(|v| format!("{v:.3}")).collect::<Vec<_>>().join(" "))`.

## Consequences for any performance work

1. **`dictation_bars:` arrives at microphone frame rate**, not at display rate.
   Redrawing the whole stage per datagram is the lag the user reported. The
   fix must coalesce to the display's frame callback, not drop the word.
2. The word list is **not** extensible unilaterally: renaming or reparsing
   these literals silently breaks dictation, which is a user-visible feature.
3. `DBG morph A -> B` stderr lines are also a contract — the stress harness's
   effect witness (`scripts/stress/stress.py`) reads them to time transitions.
   The per-packet `DBG socket word=` line is *not* a contract and should go.
