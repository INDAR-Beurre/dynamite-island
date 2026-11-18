# Lock screen — what the author SAYS (transcript, outranks pixel guesses)

Source: `The Perfect Hyprland Lock Screen Setup (Every Choice Explained)
[C6aSeHhlxsA].mp4` + its `.txt` transcript, in `~/Downloads/Reeldrop/`.
Recovered by a research child before its run died on a provider error.
These are the author's own stated choices, so they outrank any pixel estimate.

1. **Font is SF Compact Display.** He walked the path explicitly:
   SF Pro Text → SF Pro Display → **SF Compact Display**. This is the same
   display face the collapsed pill uses.
2. **The lock clock is wallpaper-sampled frosting INSIDE THE GLYPHS** — not
   white text, and **not a frosted rectangular panel behind the text**.
   The author's words ("translucent, tinted by the wallpaper, frosted blur,
   shadow") describe the letterforms acting as a window onto the blurred
   wallpaper. **Verified in pixels** on `/tmp/ref/lock/l40.png`, clock band
   y[581,858] x[844,1714]: 134,887 ink pixels, **median RGB exactly
   [87,235,206] = `#57EBCE`**, 92.8% teal-ish, only **0.7% near-neutral** —
   so it is neither white nor a flat colour. It **varies across x** (left
   third mean [127,221,194] vs mid/right [87,235,206]), consistent with
   sampling a wallpaper that changes horizontally.
   Recipe measured by the vision child: **glyph-only Gaussian blur 24 px**,
   **1.030× centred wallpaper zoom**, **vertical** (not radial) black veil;
   gradient residual −0.11%, RGB RMSE **2.39/255**.
   **A frosted card behind the clock would be both wrong and an AI-slop tell.**
   **The collapsed pill is a different material** (luminance 0–2 inside,
   genuinely black). Do not merge them.
3. **No avatar, username or password box in the resting state.** The password
   field appears **only once typing starts**, and disappears on **Esc**.
4. **Wrong password shakes.**
5. **The island itself morphs into the lock/keyhole and descends**, then the
   clock pops out. The lock screen is a continuation of the pill's motion,
   not a separate screen that fades in.

## Conflict this resolves — RESOLVED, and the resolution is stronger than expected

The parent's first reading of `/tmp/ref/lock/l60.png` and `l320.png` recorded
an avatar ring at y 1137–1168, username `user0` at y 1225–1242 and an
`Enter Password` placeholder at y 1289–1302.

**Those two frames are the OLD macOS-style COMPARISON IMAGE the author shows
to explain what he is NOT building.** They are not a state of the final design
at all. Independently verified by the parent:
- `l40.png` (final, resting) has **one** ink band: y[581,858], h=278, w=871,
  centre x=1279 — a single giant centred clock, nothing else.
- `l60.png` has a **different structure entirely**: bands at y[174,190] h=17
  and y[234,373], with ink reaching **x=2559, the right screen edge**.

Two different layouts, not two states of one layout. Had this gone to a
builder unchallenged, we would have faithfully replicated the wrong screen —
and it would have looked plausible, which is the worst kind of error.

**Authoritative frame classification and all measurements:
`docs/lockscreen-spec.md`.** Resting = `l40`/`l400`; typing = `l120` (dots
appear **inside the top capsule**, still no avatar/username/date/bottom field).
