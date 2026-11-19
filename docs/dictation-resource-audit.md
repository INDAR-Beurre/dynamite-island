# Dictation + panel: always-on microphone and idle resource audit

Auditor lens: **always-on microphone + idle resource cost**. Read/run only; this file is the
only thing written. No service restarted, no recording started, no edit under `src/` or
`tools/dictation/src/`.

All measurements below are from this machine, 2026-11-19, between 03:40 and 04:48 local.
The commands are quoted with their raw output so every number can be re-derived.

---

## 0. Process identity at time of audit

```
$ pgrep -af static-dictation | grep -v 'bin/bash'
4035 /home/alex/.t3/scratch/2026-11-13-can-you-please-like-i-e1185665/target/release/static-dictation

$ pgrep -x dynamite-rust
28880

$ ls -l /proc/4035/exe   # (via /proc)
/home/alex/.t3/scratch/2026-11-13-can-you-please-like-i-e1185665/target/release/static-dictation

$ tr '\0' ' ' < /proc/4034/cmdline
bash /home/alex/.t3/scratch/2026-11-13-can-you-please-like-i-e1185665/scripts/dictation_supervisor.sh
```

`pgrep -a static-dictation` fails with `pattern longer than 15 characters`; use `pgrep -af`.

Two facts up front, because they change how the numbers should be read:

1. **The dictation process that is running is the old scratch build, started by the old
   scratch supervisor**, not the in-repo one. The binary happens to be byte-identical to the
   in-repo build; the supervisor is not the in-repo script (see §6).
2. The panel binary was rebuilt and restarted repeatedly by another agent during this audit
   (`target/release/dynamite-rust` mtime moved 04:08:59 → 04:23:57 → 04:34:51 → 04:47:40),
   so panel CPU and Pss moved with it. Both the transient and the settled figures are
   reported separately in §2.

---

## 1. The always-on microphone claim: CONFIRMED

### Code

`tools/dictation/src/main.rs:90` — unconditional, before the event loop:

```rust
let audio = capture::start().expect("no microphone available");
```

Event loop starts at `tools/dictation/src/main.rs:107`.

`tools/dictation/src/capture.rs:57-103` — the stream is built and played on a dedicated
thread, which then parks forever:

```rust
// shortcut: cpal Stream is !Send, so build+play it on the thread that keeps it.
...
                let _ = init_tx.send(Ok(()));
                // Stream stays alive while this thread parks forever.
                loop {
                    std::thread::park();
                }
```

There is no way to get the stream back:

```
$ grep -n 'impl Drop' tools/dictation/src/capture.rs tools/dictation/src/main.rs
(no output)

$ grep -n 'unpark\|ThreadId\|JoinHandle' tools/dictation/src/capture.rs
(no output)

$ grep -n 'rx: Receiver' tools/dictation/src/capture.rs
122:    rx: Receiver<Vec<f32>>,
```

`AudioIn` is `#[derive(Clone)]` and holds only that receiver, so cloning or dropping it
cannot release the device.

### Live evidence

A PipeWire capture node exists and is running:

```
$ timeout 25 pw-cli ls Node | grep -A5 'alsa_capture.static-dictation'
 		client.id = "89"
 		application.name = "PipeWire ALSA [static-dictation]"
 		node.description = "ALSA Capture [static-dictation]"
 		node.name = "alsa_capture.static-dictation"
 		media.class = "Stream/Input/Audio"
 		media.type = "Audio"
 		media.category = "Capture"

$ timeout 25 pw-top -b -n 2 | grep -E 'static-dictation|alsa_input' | tail -2
R   62   1024  48000 151.9us   4.2us  0.01  0.00   59    S32LE 2 48000 alsa_input.pci-0000_00_1f.3.analog-stereo
R   92   1102  44100  38.7us  89.6us  0.00  0.00   54    F32LE 2 44100  + alsa_capture.static-dictation
```

State `R` = running, `ERR 54` = xruns accumulated, and the ALSA device underneath is
genuinely capturing:

```
$ head -3 /proc/asound/card0/pcm0c/sub0/status
state: RUNNING
owner_pid   : 1654
trigger_time: 216555.737654504

$ # appl_ptr sampled 3 s apart
appl_ptr    : 1719342080
appl_ptr    : 1719486464     # +145 384 frames in 3 s (~48 461 frames/s at 4 B/frame)
```

`/proc/asound/card0/pcm0p/sub0/status` is `closed` — so the *only* open PCM on this codec is
the dictation capture.

### Verdict

The mic is open from login whether or not the owner dictates. #21's claim is true; the line
numbers were right and still are.

---

## 2. Idle cost of both processes

### Canonical 30 s sample

```
$ # 2026-11-19T04:44:16+01:00
dictation T0:  Rss 6448 kB | Pss 1589 kB | Pss_Anon 616 | Pss_File 846 | Pss_Shmem 127 | Swap 7928 kB
dictation T30: Rss 6448 kB | Pss 1572 kB | Pss_Anon 616 | Pss_File 829 | Pss_Shmem 127 | Swap 7928 kB
panel     T0:  Rss 34116 kB | Pss 30816 kB | Pss_Anon 27052 | Pss_File 3456 | Pss_Shmem 308 | Swap 0
panel     T30: Rss 34116 kB | Pss 30816 kB | identical

dictation: vol_ctxt 2909 in 30s =  96/s    cpu 27 ticks in 30s = 0.90% of one core
panel:     vol_ctxt   30 in 30s =   1/s    cpu 28 ticks in 30s = 0.93% of one core

threads dictation=28  panel=11
fds     dictation=102 panel=71
```

Repeated samples agree; a later 20 s pair gave `dictation 99/s 0.70%` and `panel 14/s`, and a
15 s panel-only sample gave `4/s 0.77%`. Rates are rates, not single reads.

| | Pss | threads | fds | vol ctxt/s | CPU |
|---|---|---|---|---|---|
| `static-dictation` 4035 | **1 572–1 597 kB** | **28** | **102** | **96–99** | 0.7–0.9 % |
| `dynamite-rust` 28880 (settled) | **30 816 kB** | 10–11 | 70–71 | 1 | 0.6–0.9 % |
| combined | **~32.4 MB** | 38–39 | 172–173 | ~100 | ~1.6 % |

Pss is used throughout, not RSS, because the shmem pages are shared with the compositor
(`Pss_Shmem` 127 kB for dictation, 308 kB for the panel; the panel's RSS also includes
4 chromium shm segments inherited from the T3 app).

### Where the dictation cost actually sits

```
$ for t in /proc/4035/task/*; do echo "$(cat $t/comm) $(cat $t/wchan)"; done | sort | uniq -c | sort -rn
     20 static-dictatio evdev_read
      2 static-dictatio hrtimer_nanosleep
      1 static-dictatio __skb_wait_for_more_packets
      1 static-dictatio futex_do_wait
      1 module-rt         do_epoll_wait
      1 data-loop.0       do_epoll_wait
      1 cpal_alsa_in      poll_schedule_timeout.constprop.0
      1 alsa-pipewire     do_epoll_wait
```

20 threads are one blocking `read()` each on `/dev/input/eventN` — those cost 0 CPU and 0
context switches while idle. The churn is four threads:

```
data-loop.0    (pipewire)  ~47 vol-ctxt/s, 0.40 % CPU
cpal_alsa_in   (alsa)      ~24 vol-ctxt/s, 0.20 % CPU
static-dictatio (main)     ~100 vol-ctxt/s, 0.10 % CPU
alsa-pipewire              ~1.2 vol-ctxt/s
```

So ~28 threads is *not* the problem; ~3 threads waking 170×/s to discard audio is.

Memory profile, worst offenders:

```
$ awk '/^[0-9a-f]+-[0-9a-f]+ /{name=$6} /^Pss:/{p[name]+=$2} END{...}' /proc/4035/smaps | sort -rn | head -6
    612 kB  binary
    472 kB  [anon]
    120 kB  memfd:pipewire-memfd (65664)
    105 kB  libpipewire
     72 kB  [heap]
     54 kB  libasound
```

The process is **7 924 kB swapped out** (`VmSwap`) with `minflt +0 / majflt +0 over 20 s`,
i.e. the idle pages stay in swap and are not thrashing back — good, and the reason its Pss
looks so small.

### fd census (102 open)

```
  33  inherited T3/Electron file   (/v8_context_snapshot.bin, app.asar, t3code-v2 leveldb,
                                     GPUCache, 5 of them deleted inodes)
  20  evdev (/dev/input/eventN)
  19  anon_inode (eventfd/epoll)
  10  socket (all unix; no IP sockets)
   6  memfd pipewire
   4  pipe
   2  log file (~/.cache/static-dictation.log)
   1  /dev/uinput
   1  /dev/dri/renderD128
   1  /dev/null
   4  /dev/shm/.org.chromium.Chromium.* (deleted)
   1  /
```

33 of 102 fds have nothing to do with dictation: the supervisor was spawned from the T3
(Electron) app, so the tool inherited its snapshots, asar and leveldb handles and keeps five
deleted leveldb inodes plus four chromium shm segments alive. That is a launch-hygiene
defect, not a leak in dictation's own code.

### Panel memory profile

```
$ awk '/^[0-9a-f]+-[0-9a-f]+ /{name=$6} /^Pss:/{p[name]+=$2} END{...}' /proc/28880/smaps | sort -rn | head -6
  13628 kB  [heap]
  13284 kB  [anon]
   3472 kB  /home/alex/rust-island-build/target/release/dynamite-rust
    308 kB  /memfd:smithay-client-toolkit
     64 kB  [stack]
     53 kB  /usr/lib/libxkbcommon.so.0.13.2
```

Post-restart transient, measured deliberately because a reviewer sampling at the wrong moment
would report 60–99 MB:

```
04:41:16 pss=55086 kB  04:41:26 pss=61319  04:41:36 pss=60087  04:41:46 pss=60096
04:46:22 pss=33061  04:46:32 33065  04:46:42 33065  04:46:52 33064  04:47:02 33069  04:47:12 33069
```

and CPU 98.9 % → 70.1 % → 48.2 % across three 10 s samples right after a restart, settling at
0.6–0.9 %. The panel is not a 99 %-CPU process; it was mid-rebuild.

---

## 3. Privacy problem or resource problem?

**Primarily a resource problem. Secondarily a mild privacy problem. It is not a leak of
audio content.** The distinction #21 left open is real, so here is what was checked:

- **Is idle audio buffered?** No, and it is not even unread. `tools/dictation/src/main.rs:108-118`:

  ```rust
  while let Ok(chunk) = audio.try_recv() {
      buf.extend_from_slice(&chunk);
  }
  while buf.len() >= FRAME {
      let frame: Vec<f32> = buf.drain(..FRAME).collect();
      if recording {
          ...
  ```

  The drain and the 30 ms frame slicing run unconditionally; `if recording` is *inside* the
  frame loop. Idle audio is pulled out of the bounded (256 chunk) queue, sliced, and thrown
  away. That is why the main thread wakes 100×/s and why the cpal callback and resampler run
  at full rate for nothing. "An open capture node that is not being read" is the wrong
  model here: it *is* read, then discarded.
- **Is it persisted?** No. `~/.cache/static-dictation.log` was 90 205 B and unchanged across
  a 30 s sample and an 8 s sample; `groq::transcribe()` (`tools/dictation/src/groq.rs:6`) is
  only reached from `finalize()` when an utterance is worth transcribing.
- **Is it on the network?** No. All 10 sockets in `/proc/4035/fd` are unix; no IP socket
  exists while idle.
- **Bounded?** Yes, `bounded::<Vec<f32>>(256)` (`capture.rs:52`) with a drop-oldest policy and
  a one-shot `capture: audio queue full, dropping audio` warning (`capture.rs:75-84`).

What *is* a privacy cost: a `media.category = "Capture"` node is visible to any local user
via `pw-cli`/`pactl`, the desktop mic indicator is permanently on (which trains the owner to
ignore it), and `hk.rs:404` does `EVIOCGRAB` on `event2` and `event9` for the lifetime of the
process, so those keyboards are owned by the tool. The grab is not a keylogger — the log gate
is `if tracked(code)` (`hk.rs:465`) and only `KEY_D`/`LEFTMETA`/`RIGHTMETA` lines appear:

```
$ grep -o 'watching /dev/input/event[0-9]*' ~/.cache/static-dictation.log | sort -u | wc -l
22
$ tail ~/.cache/static-dictation.log   # only hk: LEFTMETA press/release and version banners
```

### Resource cost that is easy to miss

The audio codec is held powered **only** by this stream:

```
$ cat /sys/module/snd_hda_intel/parameters/power_save
1
$ cat /sys/module/snd_hda_intel/parameters/power_save_controller
Y
$ cat /sys/bus/pci/devices/0000:00:1f.3/power/runtime_status    # sampled twice, 20 s apart
active
active
$ head -1 /proc/asound/card0/pcm0p/sub0/status
closed
```

`power_save=1` would normally suspend the HDA controller once every PCM is closed. The
playback PCM is closed and the controller is still `active`, so the open capture stream is
what keeps it awake. On a laptop that is the concrete battery cost of §1 — larger than the
1.6 MB of Pss.

---

## 4. The smallest correct fix — and why it is not a one-liner

The obvious shape ("start on press, drop on release") **is not achievable by moving line 90**
with the current types, for three verified reasons:

1. **Nothing can stop the stream.** The stream lives on a parked thread (`capture.rs:99-102`);
   `AudioIn` is `Clone` and holds only a `Receiver`; there is no `impl Drop`, no `JoinHandle`,
   no `ThreadId`, no unpark path (all three greps in §1 return nothing). `drop(audio)` is a
   no-op for the device.
2. **cpal is `!Send`** — already handled correctly, and this constrains the fix's shape: the
   stream must be built on its own thread, so a lazy start means *spawning that thread on
   `HotkeyEvent::HoldStart`* and *signalling it to exit on `HoldEnd`*, not moving a value
   between threads.
3. **`main.rs:90`'s `.expect()` must become a runtime error path.** Today a vanished
   microphone aborts the process; with the supervisor's `while true; "$BIN"; sleep 1`
   (`scripts/dictation_supervisor.sh:37-40`) that becomes a 1 Hz crash loop. A lazy start
   makes "no mic" a thing that can happen at press time, so it needs a message to the pill
   instead of a panic.

Concretely, the smallest correct change:

- `capture.rs`: give the capture thread a command channel instead of `park()` forever; on
  `Stop` drop the stream and exit the thread; make `AudioIn` non-`Clone` or give it a `Drop`
  that sends `Stop`. **~40–60 lines.**
- `main.rs`: move line 90 into `HoldStart`/`Toggle` (or into a `capture::start()` call guarded
  by `Option<AudioIn>`), handle `Err` by sending an error state to the panel. **~10 lines.**
  The existing `audio.drop_backlog()` sites (main.rs:145, 150, 166, 183, 197, 201) and
  `buf.clear()` (167, 202) keep working; on a fresh start the queue is already empty, so the
  `HoldStart` `drop_backlog()` becomes redundant but harmless.
- `main.rs:50 PRE_ROLL` is a *tail trim* (`if voice_frames == 0 && utterance.len() > PRE_ROLL
  { drain(..excess) }`, main.rs:121-122), not a pre-buffer, so a lazy start loses no pre-roll
  behaviour that does not already exist.

**Latency: I could not measure it, and I will not guess.** The hotkey path itself is cheap —
`hk_rx` is polled every 10 ms (`main.rs:215-218`), so press→detect is already ≤ 10 ms. The
open question is how long `cpal::build_input_stream` + `play()` takes on this host. A
standalone cpal 0.15 probe built offline (`/home/alex/.cache/miclat`) printed the device and
format in 6–14 ms and then **hung inside `build_input_stream` for > 60 s, never returning**
(reproduced 4×, killed by `timeout`):

```
$ timeout 20 ./target/release/miclat
dev=Ok("default") (flushed)
cfg 44100 Hz 2 ch F32
exit=124
```

That is confounded, not conclusive: pid 4035 already owns the device, and a second ALSA
capture client cannot open at all —

```
$ timeout 5 arecord -D hw:0,0 -f S16_LE -r 16000 -c 1 /dev/null
arecord: main:850: audio open error: Device or resource busy
```

`-D plughw:0,0` gives the same error; `-D default` and `-D pipewire` hang;
`pw-cat --record --target 92` hangs and writes 0 B. **So the lazy-start latency is an honest
gap: it must be measured on a host where the device is free, i.e. after the fix lands.**
If it turns out to be tens of milliseconds, the fallback is to keep the device open but stop
the *consumer* (drop `AudioIn`, let the bounded queue fill and drop-oldest, zero CPU) — that
keeps the codec awake, so it is strictly worse for battery and should only be the fallback.

---

## 5. Embedding quality

| check | result |
|---|---|
| tracked files, clean tree | `git ls-files tools/dictation \| wc -l` → **30**; `git status --porcelain tools/dictation` → **empty** |
| offline build from clean checkout | **works** (`cargo build --release --offline`, 0 errors, 38 warnings, 132 crates) with a warm `~/.cargo/registry` |
| offline build with cold `CARGO_HOME` | **fails** — `error: no matching package named 'anyhow' found ... location searched: crates.io index`; `cargo metadata --offline` fails on `cesu8 v1.1.0`. There is no vendor dir and no `.cargo/config.toml`, so "offline" means "already cached". EMBEDDING.md's own caveat is accurate. |
| tests | `cargo test --release --offline` → **102 passed, 2 ignored** |
| build entry point | root `build.sh:17-22` builds the panel, then `(cd tools/dictation && cargo build --release "$@")`, prints both binary paths |
| references to old scratch dir | in no source file (`grep -rn 't3/scratch' tools/dictation/src tools/dictation/scripts` → none). Only historical prose: `EMBEDDING.md:3`, `EMBEDDING.md:53`, `docs/socket-contract.md:13` |
| secrets in tree | `grep -rlE 'gsk_[A-Za-z0-9]{20,}' tools/ src/ docs/` → **no hits**; `~/.config/static-dictation/api_key` is mode **600**; key read from `GROQ_API_KEY` or that file (`config.rs:14,29`); no `GROQ_API_KEY` in the running process environ |
| transcript log private | `-rw------- 1 alex users 90205 ... static-dictation.log` — **600 holds**, unchanged size across 30 s |
| rotation | present in `scripts/dictation_supervisor.sh:15-24` (`umask 077`, `: >>"$LOG"`, `chmod 600`, rotate at 1 048 576 B keeping `.1`) |

The transcript log genuinely contains dictated text, so the 600 mode is load-bearing, not
cosmetic:

```
$ grep -vE '^hk:|^static-dictation:|^$' ~/.cache/static-dictation.log | head
Yo!
Yo-ho-ho.
On benchmarks is the
M-A-I...
```

---

## 6. What is wrong with the embedding (three real defects)

### 6a. The running supervisor is the OLD scratch script, with the OLD umask

```
$ md5sum <scratch>/scripts/dictation_supervisor.sh tools/dictation/scripts/dictation_supervisor.sh
b796c135558d55322de46df35b0488fd  (scratch)
26e13b3167ad35c7cdeeeb5539fa5fc6  (repo)

$ grep Umask /proc/4034/status
Umask:	0022
```

The scratch script is 10 lines: no `umask 077`, no `chmod`, **no rotation**. The log is only
600 today because it was chmod'ed by hand earlier (`ls -l` shows mtime Nov 18 22:07, i.e. it
has not been rewritten since). Any restart that *recreates* the file under `umask 0022` yields
644 again:

```
$ umask 0022; : > $T/log;  stat -c %a $T/log     -> 644
$ ( umask 077; : > $T/log2; chmod 600 $T/log2 )  -> 600
```

**So the private-log fix lives in a script that is not executing.** It survives only until the
next supervisor start from the scratch path.

### 6b. Both docs still claim a fallback that does not exist

`tools/dictation/EMBEDDING.md:52-54`:

> resolves its binary relative to its own location, prefers the in-repo build, and **falls back
> to the original scratch binary** if the in-repo one is missing — so a failed rebuild never
> leaves the owner without dictation.

`docs/socket-contract.md:13`: "that path is now only a fallback in the supervisor."

The repo script does the opposite and says so in its own header:

```
26: BIN="$here/target/release/static-dictation"
27: if [ ! -x "$BIN" ]; then
30:     msg="supervisor: no dictation binary at $BIN; build it with:
34:     exit 1
```

A missing binary is a loud failure, deliberately. The docs describe the pre-`e971ce8`
behaviour.

### 6c. The panel build in `target/` has no dictation handler

```
$ strings -a target/release/dynamite-rust | grep -c dictation_start
0
$ grep -c '"dictation_start"' src/main.rs
1
```

`src/main.rs:6756` compares `word == "dictation_start"`, yet the freshly built binary contains
no such literal, while other socket literals (`lockscreen` 3/3, `unlock` 2/2, `quit` 2/3) do
appear. I could not explain this from source alone and I am not going to invent a cause: it
may be string-pool layout, `strip`, or another agent's in-flight edit (`src/main.rs` mtime
04:46, mid-audit). What I can state is the observable consequence: the dictation tool's
`dictation_start`/`dictation_end`/`dictation_bars:` messages (`main.rs:38,42,132`) are only
useful if the panel parses them, and the shipped binary gives no evidence that it does. This
needs one `dynamite_send` + panel-draw round trip to settle, which I did not run (it would
have required posting to the socket the panel owns while another agent was rebuilding it).

Not a defect, recorded so nobody re-reports it: `pgrep -x dynamite-rust` at 04:44:54 showed
`another instance owns /run/user/1000/dynamite-rust.sock -- exiting`, and the autostart
wrapper backed off. That is the panel's own single-instance guard working correctly during a
rebuild, not a dictation problem.

---

## 7. Honest gaps

- **Lazy-start latency unmeasured.** The cpal probe hangs because the device is already held
  by pid 4035. Must be measured on a free device.
- **Idle audio *content* not verified.** I proved audio is drained and discarded and that no
  log/network write happens, but I did not capture a sample to prove the discarded frames
  contain live room audio rather than silence (doing so would mean recording, which is out of
  scope). `pcm0c` advancing `appl_ptr` at ~48 k frames/s is strong indirect evidence.
- **`pactl` and `pw-dump` are unusable here.** `pactl info` / `pactl list source-outputs`
  print `Got SIGINT, exiting.` and hang; `pw-dump` never exits, so its JSON cannot be parsed.
  `pw-cli ls Node` intermittently takes 15–30 s and sometimes times out.
- **Two `wireplumber` processes** (pid 1656 since Nov 16, pid 30572 since Nov 19 04:32) exist.
  Pre-existing session-manager oddity, unrelated to dictation, outside this lens.
- Whether closing the capture stream actually lets `0000:00:1f.3` reach `suspended` is
  **inferred** from `power_save=1` + closed playback PCM + `active` status while capture runs.
  Proving it needs the fix to land, since every attempt to close pid 4035's stream would mean
  killing the owner's tool.

---

## 8. Commands used

```sh
pgrep -af static-dictation ; pgrep -x dynamite-rust
grep -E '^(Rss|Pss|Pss_Anon|Pss_File|Pss_Shmem|Swap):' /proc/$PID/smaps_rollup
grep -E '^(VmRSS|VmSwap|Threads):' /proc/$PID/status
ls /proc/$PID/fd | wc -l
awk '/^voluntary_ctxt/{print $2}' /proc/$PID/status          # sampled 30 s apart
awk '{print $14+$15}' /proc/$PID/stat                        # CPU ticks (100 Hz)
for t in /proc/$PID/task/*; do cat $t/comm; cat $t/wchan; done
timeout 25 pw-cli ls Node | grep -A5 static-dictation
timeout 25 pw-top -b -n 2 | grep -E 'static-dictation|alsa_input'
head -3 /proc/asound/card0/pcm0c/sub0/status
cat /sys/bus/pci/devices/0000:00:1f.3/power/runtime_status
cat /sys/module/snd_hda_intel/parameters/power_save{,_controller}
timeout 5 arecord -D hw:0,0 -f S16_LE -r 16000 -c 1 /dev/null
timeout 20 /home/alex/.cache/miclat/target/release/miclat
cd tools/dictation && cargo build --release --offline && cargo test --release --offline
grep -rlE 'gsk_[A-Za-z0-9]{20,}' tools/ src/ docs/
stat -c '%a %U %s %y' ~/.cache/static-dictation.log
tr '\0' ' ' < /proc/4034/cmdline ; grep Umask /proc/4034/status
```
