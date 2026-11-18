# Lock screen — what the author SAYS (transcript, outranks pixel guesses)

Source: `The Perfect Hyprland Lock Screen Setup (Every Choice Explained)
[C6aSeHhlxsA].mp4` + its `.txt` transcript, in `~/Downloads/Reeldrop/`.
Recovered by a research child before its run died on a provider error.
These are the author's own stated choices, so they outrank any pixel estimate.

1. **Font is SF Compact Display.** He walked the path explicitly:
   SF Pro Text → SF Pro Display → **SF Compact Display**. This is the same
   display face the collapsed pill uses.
2. **The lock clock surface is deliberately TRANSLUCENT**, tinted by the
   wallpaper, with a **frosted-glass blur behind it and a shadow** — *not*
   pure white text on a flat veil.
   **Do not confuse this with the collapsed pill**, which measures luminance
   0–2 inside its capsule in `/tmp/ref/s/s17.png` — the pill IS pure black.
   Two different materials; applying one to the other is a defect.
3. **No avatar, username or password box in the resting state.** The password
   field appears **only once typing starts**, and disappears on **Esc**.
4. **Wrong password shakes.**
5. **The island itself morphs into the lock/keyhole and descends**, then the
   clock pops out. The lock screen is a continuation of the pill's motion,
   not a separate screen that fades in.

## Conflict this resolves

The parent's first reading of `/tmp/ref/lock/l60.png` and `l320.png` recorded
an avatar ring at y 1137–1168, username `user0` at y 1225–1242 and an
`Enter Password` placeholder at y 1289–1302. Per (3) those frames were almost
certainly captured **while typing**, not at rest. A spec built from them would
bake a transient state into the resting design. Any lock-screen builder must
separate **resting** from **typing** states before implementing either.
