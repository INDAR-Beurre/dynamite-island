# Dictation tool — now part of the package

Previously this lived in `~/.t3/scratch/2026-11-13-can-you-please-like-i-e1185665/`,
a dated scratch directory. It worked only because that temp folder was never
cleaned: clearing scratch would have killed dictation and made the panel's
dictation state unreachable. The owner asked for it to be embedded, so the
source now ships here.

## Build

```sh
cd tools/dictation
cargo build --release --offline
```

Verified offline: **0 errors, 38 warnings, 132 crates**, binary
`target/release/static-dictation` (3.4 MB). All seven direct dependencies were
already in the local registry cache, so no network is needed.

`target/` is excluded by the repo's existing `.gitignore` — source only is
tracked (29 files, 448 KB).

## Why it is NOT a Cargo workspace member

The panel uses `smithay-client-toolkit 0.19`; this crate uses `0.21.1`, plus
`cpal`, `reqwest` + `rustls-tls`. A shared workspace would unify the lockfile
and could force version resolution changes onto a working panel binary. Keeping
separate manifests keeps the two builds independent. This is a deliberate
trade-off, not an oversight.

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

## Panel contract (unchanged)

The four literals in `docs/socket-contract.md` are byte-identical in this copy:

| literal | source |
|---|---|
| `dictation_start` | `src/main.rs:38` |
| `dictation_end` | `src/main.rs:42` |
| `dictation_bars:<v> …` | `src/main.rs:132` |
| `collapsed` | `src/pill/tsar.rs:129` |

All four were sent to the live panel and accepted after the migration.