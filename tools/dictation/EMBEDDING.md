# Dictation tool — now part of the package

Previously this lived in `~/.t3/scratch/2026-11-13-can-you-please-like-i-e1185665/`,
a dated scratch directory. It worked only because that temp folder was never
cleaned: clearing scratch would have killed dictation and made the panel's
dictation state unreachable. The owner asked for it to be embedded, so the
source now ships here.

## Build

```sh
cd tools/dictation
cargo build --release
```

Verified: **0 errors, 38 warnings, 132 crates**, binary
`target/release/static-dictation` (3.4 MB).

`--offline` is deliberately **not** in the documented command. It succeeded
here only because `~/.cargo/registry` was already warm; with a cold
`CARGO_HOME` it fails (`error: no matching package named \`anyhow\` found`).
The plain command works cold, given a network. Use `../../build.sh` to build
the panel and this crate together.

`target/` is excluded by the repo's existing `.gitignore` — source only is
tracked (**30 files, 452 KB**).

**Reproducibility limit (critic #21, verified; partly closed).** A cold
`CARGO_HOME` still cannot build **offline** — there is no vendor dir and no
`.cargo/config.toml` for this crate. The documented command no longer claims
otherwise, and `build.sh` at the repo root now builds both crates and fails
loudly. A fresh clone has no binary (`target/` is ignored); the supervisor now
**exits non-zero with a build hint on stderr** instead of silently running a
binary from a dated scratch directory. Open work: vendor the direct deps if
fully offline builds are wanted.

## Why it is NOT a Cargo workspace member

The panel uses `smithay-client-toolkit 0.19`; this crate uses `0.21.1`, plus
`cpal`, `reqwest` + `rustls-tls`. A shared workspace would unify the lockfile
and could force version resolution changes onto a working panel binary. Keeping
separate manifests keeps the two builds independent. This is a deliberate
trade-off, not an oversight.

**Caveat found by critic #21:** sctk 0.21.1 is used *only* by
`src/pill/tsar.rs`, which is dead code in the shipped binary (see below). The
lockfile hazard is real, but it currently shields an unreachable dependency.
Deleting the pill module would make a single workspace safe.

## Launch

`scripts/dictation_supervisor.sh` resolves its binary **relative to its own
location**, prefers the in-repo build, and falls back to the original scratch
binary if the in-repo one is missing — so a failed rebuild never leaves the
owner without dictation.

`~/.config/mango/autostart.sh` now invokes the in-repo supervisor. The previous
version is preserved as `autostart.sh.bak-dictation-embed-<timestamp>`.
The running supervisor was **not** restarted, so the current session is
undisturbed; the new path takes effect at next login.

## Secrets

The Groq key is read from `GROQ_API_KEY` or
`~/.config/static-dictation/api_key` — **never from the source tree**. Verified
no key material was copied in (`grep -rlE 'gsk_[A-Za-z0-9]{20}'` → no hits).

## Panel contract

Three of the four literals in `docs/socket-contract.md` are byte-identical in
this copy **and reachable in the shipped binary**:

| literal | source | in binary? |
|---|---|---|
| `dictation_start` | `src/main.rs:38` | yes |
| `dictation_end` | `src/main.rs:42` | yes |
| `dictation_bars:<v> …` | `src/main.rs:132` | yes |
| `collapsed` | `src/pill/tsar.rs:129` | **NO — dead code** |

**Correction (critic #21).** An earlier version of this file claimed all four
were sent to the live panel and accepted. That was wrong for `collapsed`:
`src/main.rs:67` sets `let pill: Option<pill::PillHandle> = None;` and
`PillHandle::try_spawn()` is called only from `#[cfg(test)]`, so
`grep -ac collapsed target/release/static-dictation` → **0**. The three
reachable literals were exercised live; `collapsed` was sent by the test
harness, not by this tool. No runtime regression — the panel owns its own
collapsed state — but the contract doc lists a producer that does not produce.

## Privacy

Every transcription is printed to stdout and the supervisor appends it to
`~/.cache/static-dictation.log`. That file was **mode 644 and never rotated** —
a world-readable, unbounded record of everything dictated. The supervisor now
creates it under `umask 077`, `chmod 600`s any pre-existing file, and rotates
at 1 MiB keeping one generation. The existing log was tightened to 600.

## Known open issue — always-on microphone

`src/main.rs:90` calls `capture::start()` unconditionally at launch and
`capture.rs:88-103` parks forever, so the cpal stream never stops. Measured on
the live process: **Pss 1794 kB idle, 28 threads, ~99 voluntary context
switches/s**, and a permanently live `alsa_capture.static-dictation` PipeWire
node. The mic is hot whether or not the owner ever dictates. `.expect(…)` plus
the supervisor's 1 s restart loop also turns a vanished microphone into a
crash loop. Fix shape: start the stream on `HotkeyEvent`, drop it on
`HoldEnd`. **Not yet done** — it changes runtime behaviour of a tool the owner
uses daily, so it is queued for a reviewed round rather than slipped in.