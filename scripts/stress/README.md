# Stress harness — dynamite-rust panel

Black-box stress tests against the **running** panel. Nothing is rebuilt, no
source file is touched, and the process is never restarted: every test drives
the app's own control socket and observes the app from the outside
(`/proc`, `ss`, `grim`, and the app's stderr).

```
bash scripts/stress/run_all.sh                     # all groups
bash scripts/stress/run_all.sh --group latency     # one group
```

Exit code 0 only if every group passed. Safe to run against the production
island.

## The protocol (recovered from `src/main.rs`, not guessed)

Control socket: `$XDG_RUNTIME_DIR/dynamite-rust.sock` — a **unix datagram**
socket, bound in `src/main.rs` (`UnixDatagram::bind`), non-blocking, payload
read into `[0u8; 512]` and turned into a word with
`String::from_utf8_lossy(&buf[..n]).trim()`.

| kind | words |
| --- | --- |
| state (`IslandState::from_word`) | `collapsed` `dictation` `lock` `power` `music` `calendar` `controlcenter` `launcher` `wallpaper` `themes` `clipboard` `traymenu` |
| lockscreen | `lockscreen` `unlock` `locktype` `lockback` `lockwrong` |
| dictation | `dictation_start` `dictation_end` `dictation_bars:<floats>` |
| misc | `clock` (= `collapsed`) `reload` |
| **destructive** | `quit` — stops the island |

Consequences the tests encode:

* the payload is trimmed and then compared **as a whole string**, so
  `collapsed ` is accepted but `collapsed\x00` and `collapsedmusic` are not;
* matching is **case-sensitive** (`Collapsed` is inert);
* invalid UTF-8 becomes replacement characters via `from_utf8_lossy` and is
  therefore inert — it must never be able to reach a UTF-8 assumption;
* anything over 512 bytes is truncated by the app (kernel `EMSGSIZE` applies
  earlier, around 200 KB);
* unknown words are silently ignored: there is no reply channel at all, so
  every assertion here is an *effect* assertion, never a response assertion.

`quit` is in a `FORBIDDEN` set in `stress.py`; `App.send()` raises if a test
ever tries to send it.

## How effects are observed without a reply channel

A datagram socket gives no acknowledgement, so "did the app accept the word?"
is measured three ways, and each test uses the weakest one that answers its
question:

1. **stderr morph line.** `begin_morph` prints
   `dynamite-rust: DBG morph A -> B (morph_running=…)` when a state change
   actually starts. The harness resolves the log by reading
   `/proc/<pid>/fd/2` — the supervisor writes `~/.cache/dynamite.log`, but a
   hand-started instance may redirect to `/tmp/*.log`, and assuming a path
   silently turns every effect check into a no-op. Log rotation is handled by
   comparing `(st_dev, st_ino, size)` and re-reading from a saved offset.
   A state change with unchanged geometry takes the instant path and logs
   nothing, so a missing line is reported as a note, never counted as a pass.
2. **CPU ticks.** `/proc/<pid>/stat` utime+stime must advance while the app
   draws; a wedged event loop stops consuming CPU even though the process
   still exists.
3. **the screen.** `grim` + `ffmpeg` measure the pill capsule itself.

## Liveness: why "not blank" is not enough

The wallpaper is dark, so a naive "the pill region is not uniformly blank"
check passes even with the panel gone — measured: the wallpaper band is
83–89 % dark pixels, the same as the pill interior. The harness instead
measures the **capsule edge**: it finds the densest dark row in the top
120 px, takes the dark run's bounding box there, then compares dark fraction
inside the box with dark fraction just outside it.

Collapsed pill, scale 1.5 (2256×1504 screen, stage 1504×1002):

| measurement | value |
| --- | --- |
| interior dark fraction | 0.88 |
| exterior dark fraction | 0.20 |
| width | ≈ 111 px = `collapsed_width` 74 × 1.5 |
| capsule top | y ≈ 4 px (`gap_top` 3 × 1.5) |

Assertions: interior ≥ 0.55, exterior ≤ 0.55, top ≤ 14 × scale, width within
0.6–1.6 × `collapsed_width × scale`. That fails when the pill is missing,
squashed, or has lost its background — unlike a not-blank check.

## Groups

| group | what it does | count |
| --- | --- | --- |
| `protocol_fuzz` | every word × 12 malformed-argument variants (empty, space, NUL, `\x00\x00\x01`, `\n`, `\r\n`, non-UTF-8 `\xff\xfe\xfd`, `\x80\x81`, 4 KB ASCII, long UTF-8, 200 KB, 1 MB); every word + NUL/non-UTF-8/long garbage as a batch inertness check; case near-misses; whitespace-padded words (must be accepted); all 132 two-word datagrams; newline injection; 10 `dictation_bars` payloads incl. NaN/±inf/arity 120; 4 × 1000-command rapid fire; socket still a datagram socket | 628 |
| `state_machine` | forward + reverse sweeps, full 12×12 transition matrix, lockscreen/typing/unlock cycles, `locktype` saturation at 32, dictation cycle, 40 seeded random transitions; every drive bounded at 1500 ms | 220 |
| `load_leak` | 200 iterations of lockscreen → unlock → launcher → wallpaper → collapsed, VmRSS sampled every iteration | 200 |
| `latency` | 120 send → accepted round-trips, p50/p95/p99 | 120 |
| `liveness` | process live, socket is a dgram, socket answers, pill on screen, CPU advances while drawing (plus a pill check after *every* group) | 5 |

Counts are the tests that actually executed, accumulated per group; a group
that aborts mid-way still reports the tests it ran, because those sends
really hit the process.

## Passing criteria (all falsifiable)

* **RSS:** the first 20 iterations are warm-up — the first lockscreen entry
  decodes a full-screen RGBA (`load_lock_wallpaper`, 1504×1002×4 ≈ 6 MB) and
  the first glyph draw parses the icon font. Growth is measured from the
  median of that warm-up to the final sample; **bound 8 MB**. The absolute
  curve is printed as numbers, not just a verdict.
* **Latency:** p95 of send → morph-started ≤ **400 ms**. The 160 ms morph
  animation is not included (the timestamp is the log line, not the redraw).
* **State changes:** ≤ **1500 ms** from send to accepted.
* **Liveness:** as above, after every group.

## Failure output

Each failing test prints the exact word and payload length, so a finding can
be reproduced with a single command, e.g.

```
GROUP protocol_fuzz  FAIL  tests=628 fails=1 (37.2s)
      ! music+1048576B arg: send returned too_long, expected ok
```

## Things this harness deliberately does not do

* It does not send `quit`, and it does not restart or rebuild the panel.
* It does not test hover, clicks or keyboard — those need input injection
  into the layer surface, not the control socket.
* It does not treat a missing morph log line as a pass: identical-geometry
  transitions legitimately log nothing, so they are reported as notes.
* It does not verify *what* the panel draws beyond the collapsed pill
  geometry; per-state rendering is checked only as "still responsive".
