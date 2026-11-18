# Control-socket contract (do not break)

`$XDG_RUNTIME_DIR/dynamite-rust.sock` — AF_UNIX **SOCK_DGRAM**, one word per
datagram. A separate live binary depends on these exact literals, so a fixer
must treat them as a frozen public API.

## The external producer

`~/.t3/scratch/2026-11-13-can-you-please-like-i-e1185665/` builds
`static-dictation`, the dictation tool the user asked to have "embedded
nicely into the package". It is a **separate process** that talks to the panel
only through this socket.

| literal | producer | note |
|---|---|---|
| `dictation_start` | `src/main.rs:38` `dynamite_send("dictation_start")` | on hotkey press |
| `dictation_end` | `src/main.rs:42` `dynamite_send("dictation_end")` | on release |
| `dictation_bars:<v> <v> …` | `src/main.rs:132` | **one datagram per microphone frame**, values formatted `{v:.3}`, space-separated |
| `collapsed` | `src/pill/tsar.rs:129` `dgram.send_to(b"collapsed", sock)` | after an utterance finalises |

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
