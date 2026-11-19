# Transcript mining — dim, GTK4, display manager

Read-only research pass. Sources are the auto-captions in `~/Downloads/Reeldrop/`.
Captions garble decimals and proper nouns, so every number below carries the
**verbatim caption string** next to my interpretation. `[stated]` = his words.
`[inferred]` = mine, and never to be quoted as his.

---

## 1. DIM — `Hyprland's Default Dim Setting Is Way Too Strong [3YvwZmcsvjM].txt`

### 1.1 THE NUMBERS (read this first)

| What | Caption words (verbatim) | Value | Confidence |
|---|---|---|---|
| Range of `dim_strength` | "This goes from zero which is no dimming at all to one which is completely black" | `0.0`–`1.0` | **HIGH** — definitional, matches Hyprland |
| Hyprland default (the thing he criticises) | "the default is point five" | **`0.5`** | **HIGH** — stated twice-over in sense, matches known upstream default |
| His recommended value | "you want to do is just set these strengths for yourself something point one two looks pretty good" | **`0.12`** | **HIGH** |
| A band he likes | "Point one two point one five looks great" | `0.12`–`0.15` | **MEDIUM** — caption has no "to"; the band reading is mine |
| Live-test value, unusable | "let's go with 0.01 that'll basically make it almost the same" | `0.01` | **AMBIGUOUS — do not use.** 0.01 cannot be "almost the same" as 0.12; he mistyped live or the caption dropped a digit (`0.1`?). True value unrecoverable. |
| Attitude to 1.0 | "you can also turn this all the way up to one temporarily it's going to make every other window black" | `1.0` = full black | HIGH |

**Headline: his number is `0.12`, with a stated preference band of `0.12`–`0.15`,
against a default of `0.5` that he calls too strong.** No ambiguity on `0.12`
and `0.5`; the only garble is the throwaway `0.01` demo.

### 1.2 WHY he says the default is wrong `[stated]`

> "the default is point five / So the second you turn on dim and active every
> other window gets dimmed by half and your terminal your browser your notes /
> All of it gets so dark that you can barely read it people see that think the
> feature looks bad and turn it off again"

> "Basically here you can barely read anything that is in focus"

The failure he describes is **loss of legibility**, not loss of contrast: at
`0.5` the text in the unfocused windows stops being readable, so the feature
reads as broken and people disable it. His fix is explicitly "one number
changed" — "Takes about literally like five seconds to fix".

### 1.3 What dim is FOR, and what it must not touch `[stated]`

> "hyperland has an option called dim inactive and it darkens every window that
> you're not focused on / Your eyes go straight to whatever's brightest on the
> screen. So dimming everything else makes the window / You're working and jump
> right out."

> "Now your other windows sync back just a little and you can still read them
> perfectly fine / And your focused window still stands up anywhere around"

Two hard constraints fall out of that, both stated:

1. **The focused surface is never dimmed.** Dim is applied to the *unfocused*
   clients only. The contrast is between surfaces, produced by the compositor
   per-window.
2. **Dim must leave the dimmed content readable** ("you can still read them
   perfectly fine"). That legibility bar is what makes `0.5` wrong and `0.12`
   right.

He also names a secondary use `[stated]`:

> "you can also use this as a useful privacy feature ... if you want every other
> window apart from only the window to shoot focused on to be visible / You can
> use this actually acts as a weight privacy filter, which turns off as soon as
> you hover onto a different window"

i.e. `dim_strength = 1.0` is a *hover-to-reveal* privacy mode. Note the
mechanism matters: it is relative and interactive, not a static overlay.

### 1.4 How to apply this to OUR lock screen — the one place we must not overreach

`docs/mango-capabilities.md:23,62-63,99,113` establish MangoWM has **no**
`dim_inactive` and no focus-dim engine (its `dim_*` strings are the dimland
idle-dimmer). So the video's feature cannot be adopted as config, and his number
cannot be adopted as an alpha by direct substitution, because the two things are
not the same operation:

- his `dim_strength` = **relative** dim of unfocused clients, compositor-side,
  with a full-brightness reference surface on screen;
- our lock veil = **absolute** constant black alpha over a static wallpaper,
  with no focused window to act as reference.

`[inferred]` Therefore the correct use of `0.12`–`0.15` is as a **taste
anchor and sanity bound**, not a replacement constant:

- our fitted veil is `SCRIM_CENTRE = 0.22` → `SCRIM_EDGE = 0.42`
  (`src/lockscreen.rs:205-206`, radial, drawn at `src/lockscreen.rs:959`),
  which sits *above* his band at the centre;
- but that `0.22`/`0.42` came from least-squares fitting the actual reference
  lock-screen pixels (`docs/lockscreen-spec.md:107-119`, vertical fit
  `centre≈.224 / top≈.430 / bottom≈.507`), which outranks his stylistic
  preference for a *different* effect;
- so: **keep the measured veil.** Cite `0.12`–`0.15` only as corroboration that
  the reference's centre alpha is in the "gentle, still-readable" regime he
  advocates — our `0.22` centre is the same order, and the heavy end of our
  gradient is at the *bottom edge*, not on the content.

`[inferred]` One further consequence worth flagging to the integrator: our
`COUNCIL_LOG.md:72` measurement put the lock veil at **≈0.29–0.30 alpha**
(mean-luminance ratio 0.713) on a different frame/wallpaper, i.e. three
independent approaches land in `0.22`–`0.30` for the centre. His `0.12` is
lighter than all of them. Do **not** "correct" the veil down to `0.12` on the
strength of this video — that would be substituting a preference for a
measurement, exactly the class of error that produced the invented colour
last time.

**No colour is stated.** "completely black" at `1.0` implies black, but he
never writes a hex or names `#000000`. Do not attribute a colour value to him.

---

## 2. GTK4 theming — `I Fixed Hyprland’s Most Annoying Theming Problem (GTK 4) [728E92q-oRY].txt`

**Verdict in one line: no bearing on a self-drawn Rust panel.** It is a story
about patching `gtk4` itself. Nothing here changes any pixel we draw, and there
are **no hex values, no palette, no typography, no spacing** anywhere in the
transcript.

The actual problem `[stated]`:

> "gtk4 reads your colors from config gtk4.0 gtk.css only once when an app
> starts and ever again. So your normal bar and terminal change instantly. And
> not a list just sits there in the old colors."

His fix `[stated]`: replace Arch's `gtk4` with his patched build that watches
that file and reloads it on change —

> "This package is basically arches normal gtk4 plus a small patch ... makes it
> watch that file and reload it the moment that it changes"

> "it's the same recipe, same settings, it just replaces gtk4 and everything
> else treats it like the real thing."

Practical, caption-garbled identifiers, **not for us to act on**:

- package name appears as "s why you gtk4 life CSS" → almost certainly
  `gtk4-live-css` **[inferred from caption]**;
- a failed first attempt (toggling `prefer-dark` ↔ `prefer-light`) flashed
  white — "there will always be the brightest flash bang";
- themes he cycles to demo are caption-mangled: "Tokyo Knight 2", "Ariadny",
  "Gruffbox"/"Gruffbox Material", "Horizon", "Nightfox", "Nord", "Jose Pine"
  (= Rosé Pine), "Rehan". Treat as noise; no values recoverable.

The **only** transferable fact `[stated]`: GTK4 will not live-reload a shared
palette file. `[inferred]` If we ever hand our palette to GTK apps (a
`gtk.css` for coherence with the owner's apps), a theme change needs those apps
restarted or his patched package. Our own panel repaints itself and is
unaffected. Nothing else here is actionable.

---

## 3. Display manager — `You don’t NEED a display manager for Hyprland [KndDY14ddE4].txt`

No numbers at all in this video. Everything actionable is **ordering**, which is
exactly our startup race, so it is worth recording even without figures.

### 3.1 How he starts the session `[stated]`

> "without a display manager your computer boots to a plain text login / Which
> is called a TTY you type your username and password there and then we want
> hyperline to start on its home"

He puts the launch in the login shell profile — `~/.bash_profile` or
`~/.zprofile` — gated on the TTY:

> "if the TTY is equal to dev TV one one, then you're just going to exact star
> hyperline" (i.e. `if [ "$(tty)" = /dev/tty1 ]; then exec start-hyprland; fi`)

> "this only start hyperlined when you log in on the first TTY that means that
> if you ever need a plane terminal / You just hit control F2 log in there and
> you get a little shell with no hyperline"

Why `exec` `[stated]`:

> "exact means that when hyperline deposes you get logged out completely so
> you're never left with logged in shell just sitting there"

### 3.2 The ordering rule — the part that matters to us `[stated]`

> "you want / Make this actually be bottom most thing that runs in the file
> because once exact ones nothing after in that file ever months" (i.e. "once
> `exec` runs, nothing after in that file ever runs")

> "the reason why anything you know works / This actually works with the stuff
> being on top is because these are just like exporting stuff. These are just
> like / Exporting environment variables, which doesn't really matter all that
> much if you have actual / Other stuff in your z profile, then you're
> definitely going to want this at the bottom so that it still works and
> functions as intended"

`[inferred]` Two rules from this, and they are the useful yield of the video:

1. Anything that must run *before* the compositor goes **above** the exec line
   and must be env exports only (no background daemons, no `sleep` chains);
2. everything that must run *inside* the session must be started **by the
   compositor**, not by the profile — because `exec` is terminal for the file.

He never gives a delay, a `sleep`, a retry count, or a "wait for socket" trick.
**There is no number here to adopt.** He sidesteps the race structurally: the
compositor is started first and owns everything after it.

### 3.3 Session locking / lock-as-login `[stated]`

> "some hyperline s dash if you're using hyper lock dash dash walk that's CMD /
> hyper lock / Hyper lock with a K and that's going to start up hyper lock for
> you now locked / CMD is a hyper line launch option / It's right there in
> hyper line dash dash help it locks your session before anything on your
> desktop shows up and then it starts your lock screen / So your lock screen
> becomes your login screen"

⚠️ The flag name is **caption-garbled** ("dash dash walk", "now locked", "CMD").
It is Hyprland's own `--lock-cmd` launch option `[inferred]`; **MangoWM has no
equivalent** (see `docs/mango-capabilities.md:83` — Mango exposes
`ext-session-lock-v1`, and `swaylock` is already bound in `bind.conf`). Do not
carry the flag across.

The failure mode he warns about is directly relevant to a panel-drawn lock UI
`[stated]`:

> "before you reboot / You just want to run hyper lock once from your current
> session and make sure that it unlocks if your lock screen can't start at boot
> hyper line keeps the session locked anyway / So you get a lock screen with
> nothing on it to type into"

**Test the lock screen's unlock path before enabling lock-at-boot**, because a
lock client that fails to render still leaves the session locked with no way to
type. Recovery is "control all f2 / Login on the second TTY and fix your lock
screen config from there."

He also rejects display managers partly on *palette coherence* grounds
`[stated]`, which echoes what we care about:

> "your whole rice lives in your home folder, right? / So in your dot files ...
> and that's config lives in at see SDDM dot conf dot deep / So it's completely
> outside your dot files ... every time you change your color scheme your login
> screen is stuck on the old one"

i.e. **the lock/login surface should be versioned with the rice**, in dotfiles,
so a theme change reaches it. Our lock screen is in-repo and self-drawn, so we
already satisfy this; it is a rationale, not a to-do.

### 3.4 Autologin caveat, if we ever add one `[stated]`

> "if you want zero typing at boot you can login automatically ... this basically
> means that anyone who turns on your laptop is straight to your desktop"

> "with auto login / If you exit hyperline it logs you straight back in and it
> starts it again / So when you need a plane terminal you have to just use
> control alt f2"

---

## 4. What this changes in our build

1. **Lock veil: no change.** Keep measured `SCRIM_CENTRE 0.22` →
   `SCRIM_EDGE 0.42` (`src/lockscreen.rs:205-206`). His `0.12`–`0.15` is for a
   different mechanism (relative dim of unfocused clients); adopt it as a
   documented sanity bound, not a value. Recording it gives the veil the
   "stated authority" it previously lacked — the authority is *"a gentle dim in
   this band is deliberate, `0.5`-style heaviness is the known mistake"*.
2. **Legibility is the acceptance test** he supplies: at our centre alpha the
   veiled wallpaper must still read. Our `0.22` passes; `0.42` applies only at
   the edges. Worth keeping that framing in the spec.
3. **Never dim the focused/active surface** — if we ever dim anything *inside*
   the panel (e.g. an unfocused sub-panel), `0.12` is his number and the
   focused item must stay at full opacity.
4. **Autostart ordering** (`~/.config/mango/autostart.sh`): our panel is started
   by `nohup ~/.config/mango/scripts/dynamite_autostart.sh &`, which already
   waits for a usable session. The video's rule reinforces *why*: start the
   panel from the compositor-owned path, gate on the compositor actually
   existing, and keep every pre-session export above the exec boundary. The
   existing `MANGO_INSTANCE_SIGNATURE` discovery block is the natural gate.
   `[inferred]` — he gives no number; bounded retry + socket gate is our design.
5. **GTK4: nothing.** Only note for the record: a `gtk.css` shared palette will
   not live-reload in GTK4 apps without restarting them (`[stated]` once above).

## 5. Confidence summary

- `dim_strength` default **0.5** — HIGH, verbatim, two independent clues.
- recommended **0.12**, band **0.12–0.15** — HIGH for `0.12`; MEDIUM for the
  band (the "to" is absent from the caption).
- `0.01` demo value — **AMBIGUOUS, discard.**
- dim applies to **unfocused clients only**, focused stays bright — HIGH.
- `dim_strength` is **0–1**, `1.0` = black privacy filter — HIGH.
- No colour, no hex, anywhere — HIGH (absence).
- DM video: **no numbers**; ordering rules only — HIGH.
- GTK4 video: **no actionable values** — HIGH.
