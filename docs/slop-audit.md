# AI-slop audit — rust-island-build (CPU-rasterised Wayland island)

Scanner note: the kill-ai-slop CSS/JSX scanner reports 0 findings (0 files
scanned) — expected, no CSS/HTML exists. All findings below are from manual
reading of the draw code. Line numbers refer to the live `src/*.rs`
(non-`.bak`) files.

## Raw counts

- **Distinct colour constants**: ~78 named bindings across 14 files, of
  which **~22 are near-duplicates of 6 core values** (see F1). Roughly 45
  genuinely distinct RGB values appear in the compiled UI fill/glyph paths.
- **Accent-family hues (saturated)**: 5 — #7CA2F5 (124,162,245),
  #93A4F5 (147,164,245), #74A3F6 (116,163,246), #7aa2f7 (122,162,247), and
  the mint legacy #8CCF7E (test-only). #05163C is the highlight tint
  derived from the accent.
- **Type sizes (live draw paths)**: 20 distinct `_PX` constants spanning
  10.5–36.5, resolving to ~11 distinct rendered px values (10.5, 11,
  12, 12.3, 12.5, 13, 13.4, 16, 19, 20, 36.5)
- **Radii**: disciplined — panels 20 (CC_RADIUS, PANEL_RADIUS), rows/pills
  9–12, tiles 4, artwork 6, toggle mini-radius 2.5, calendar ~18. All
  derive from role, not hand-tuned per call. No per-element wild radii.
- **Secondary-text greys in use**: **4 different values** — #9AA0A6
  (154,160,166), #BEC4CA (190,196,202), #C0C4C9 (192,196,201), #C8CCD0
  (200,204,208) — plus pure #FFFFFF in themes/wallpaper panels.

## Findings (most severe first)

```
slop  src/clipboard_panel.rs:118  purple/cyan/blue doc-mock lines inside
      clip preview tiles       -> mock clip content in one grey; the
      #BB9AF7/#7DCFFF/#E0E4FC rainbow reads as "showcase" not data
slop  library.rs:36 + main.rs:927 + controlcenter_panel.rs:362,366 +
      music_panel.rs:288      four hand-typed periwinkles for one accent
      (#7CA2F5 vs #93A4F5 vs #74A3F6 vs #93A4F5) -> one ACCENT const,
      one derived; components drift when the theme changes
slop  launcher_panel.rs:177, clipboard_panel.rs:106,
      controlcenter_panel.rs:374, music_panel.rs:279-282
      four near-identical secondary greys (#9AA0A6/#BEC4CA/#C0C4C9/
      #C8CCD0) -> one TEXT_SECONDARY token shared via library.rs
slop  src/dictation_canvas.rs:126-132 + :400-588  22px-reach glow/halo
      + bloom + breathing idle hum on the waveform -> flat bars, one
      soft 1px ambient falloff max; the halo is decoration, not state
slop  src/main.rs:693-699 + :2499-2507  two-tone rim highlight + shadow
      on every disc (nav AND workspace) at alphas 0.65/0.55 -> drop the
      rims; keep the small shadow only if the panel also casts one
slop  src/lockscreen.rs:199  #AAF9FE cyan clock on dark wallpaper ->
      white/near-white like every other panel text; lone saturated hue
      that is not the accent
slop  src/music_panel.rs:283-287  #B0B0B0 loaded-vs-#2E2E2E track + a
      third grey CTRL_SIDE -> two states: white active, #2E2E2E idle
slop  src/themes_panel.rs:1149-1152  icon chip (16,18,24 disc) +
      per-theme teal dot behind every row icon -> drop the chip /
      dot; muted-tone icons read cleaner than pastel tiles
slop  src/lockscreen.rs:199-204 / main.rs:673-706  duplicate shadow
      systems (SHADOW2_LAMBDA 7.2px + PANEL_SHADOW_BLUR 10px +
      DISC_SHADOW_BLUR 4px, three alphas) -> one shared shadow helper,
      one offset/blur/alpha scale
```

### F1 — accent fragmentation is the deep slop (severity: high)

The island *has* one accent in spirit — periwinkle #7CA2F5 — and
`library.rs:36-41` even declares `ISLAND_ACCENT == DEFAULT_ACCENT`. But the
accent is **re-typed by hand at five call sites with three different byte
values**:

- `library.rs:36` `DEFAULT_ACCENT = (124, 162, 245)` = #7CA2F5
- `main.rs:927` `WS_ACTIVE_FILL = (147, 164, 245)` = #93A4F5
- `main.rs:929` `WS_ACTIVE_GLYPH = (11, 27, 58)` navy — duplicated as
  `controlcenter_panel.rs:363` `C_TOGGLE_ON_GLYPH` and test consts
  `NAVY`/`C_DOC_*` neighbours
- `controlcenter_panel.rs:362` `C_TOGGLE_ON = (147, 164, 245)` = #93A4F5
- `controlcenter_panel.rs:366` `C_SLIDER_FILL = (116, 163, 246)` = #74A3F6
- `music_panel.rs:288` `DOT_ACTIVE = [0x93, 0xA4, 0xF5]` = #93A4F5
- `clipboard_panel.rs:116` `C_DOC_STROKE = (122, 162, 247)` = #7AA2F7

Six accents where the system needs one. Two of the values differ from the
"official" token by 23/23/0 and 8/1/1 — invisible per-component but
collectively the "six slightly different near-blacks" pathology applied to
the hue the whole UI leans on. Why it reads machine-made: an accent that
is *almost* consistent is exactly what a code generator emits when each
panel is written in its own session; a human designer keeps one swatch.
**Fix:** `library.rs` already exports `ACCENT`/`ISLAND_ACCENT`; every site
above should read `crate::library::island_accent()` (the existing RwLock
already supports theme override), with the navy glyph derived once from
it. Delete the local constants. This is also the only finding whose fix
*makes* the theming feature actually work — today changing the theme
changes some accents and not others.

### F2 — four secondary-text greys (severity: high)

`launcher_panel.rs:177` #9AA0A6, `controlcenter_panel.rs:374` #BEC4CA,
`clipboard_panel.rs:106` #C0C4C9, `music_panel.rs:279/280/282`
#C0C4C9/#C8CCD0 — four greys for the same job (body/secondary text on
near-black frost). The comments show they were *individually patched
toward legibility* over iterations (music: "peak 155 — on the 150 floor"),
i.e. hand-tuned per call site, drifting apart. Reads as machine-made
because the eye sees three panels whose "dim text" is a different grey.
**Fix:** one `TEXT_SECONDARY` in `library.rs`, used by all four; if music
needs a slightly brighter date line, derive it with one documented
scale factor, not a new hex.

### F3 — white vs. grey secondary flip-flops (severity: medium)

`themes_panel.rs:264` and `wallpaper_panel.rs:198` set `C_SECONDARY =
#FFFFFF` (iter128 comments even note the panels *disagree*), while
launcher/clipboard/CC keep grey. Same-role constant, opposite values,
per-panel. **Fix:** decide once (the iter128 white decision is well
argued — contrast), apply it through the shared token from F2.

### F4 — dictation waveform: glow, bloom, halo, breathing idle (severity: high)

`dictation_canvas.rs:126-132`: `GLOW_K/GLOW_REACH/BLOOM/GLOW_TEAL/
GLOW_A/GLOW_IDLE` — a 22-px halo cut-off, a 0.32 bloom aura "inside the
mound", and a "dim teal hum… breathing with v" on silence. Plus
`GRAD_L/GRAD_C/GRAD_R` — a three-stop teal→cyan→blue gradient along the
bar span (`wave_color`, line ~163) — the only multi-stop gradient in the
codebase. This is the classic "designed in a thumbnail" tell: at 100% on a
dark desktop it is a luminous blob, while every other surface of this
island is flat and restrained. The comments even cite a "reference" with a
jewel-halo, but references sampled from flashy concept renders are exactly
what the user is fighting. **Fix:** flat teal bars on the near-black
capsule, alpha-ramped by the whisper tables (keep those — they encode
*state*), no halo, no bloom, one hue not a gradient. If a hint of life is
wanted on idle, a single 1.5px alpha falloff beyond the bar edge is the
ceiling.

### F5 — disc rim highlights + triple shadow system (severity: medium)

`main.rs:693-699` (`RIM_LIGHT` 168,176,205 @0.65 top / `RIM_DARK`
8,9,11 @0.55 bottom, `RIM_WIDTH` 1.5) applied in `disc_rim`/`disc_rim_r`
(:2499-2507), used by **both** `draw_nav_disc` (:2914) and
`draw_workspace_disc` (:2584). A lit-top/dark-bottom rim is a skeuomorphic
"3D coin" treatment; combined with the disc drop shadow
(`DISC_SHADOW_OFF` (2,3) blur 4 alpha 0.35), panel drop shadow (blur 10,
alpha 0.40), and the dictation canvas's separate exponential shadow
(`dictation_canvas.rs:153-158`, tint 10,20,35, λ=7.2), the build carries
**three independent shadow formulations and two rim systems** for the
same class of small rounds. The workspace-disc comment at `main.rs:2596`
even records that the blurred halo there was *removed* as
"decoration / glow" — the nav discs kept theirs. **Fix:** one shadow
helper; drop the rims on both disc types (the 0.92 fill factor at
`main.rs:2880/2951` already gives the discs a soft edge against the
frost); if a rim survives anywhere, define it in `library.rs` so both
call sites share it.

### F6 — clip doc-mock rainbow lines (severity: medium)

`clipboard_panel.rs:115-118`: the "document" preview tile draws its mock
lines in **purple #BB9AF7, cyan #7DCFFF, and pale #E0E4FC** — three extra
saturated hues that exist nowhere else, on a decorative mock of content.
This is pixel-art syndrome: filling an illustrative placeholder with
showcase colours. Reads machine-made because real clipped documents don't
have per-line rainbow gradients; it's a "screenshot for the docs" tell.
**Fix:** the existing `C_TILE_GLYPH` #4A4A4A / `C_DOC_LINE3` #E0E4FC
pair already encodes text lines fine; drop #BB9AF7 and #7DCFFF.

### F7 — lone cyan lockscreen clock (severity: medium)

`lockscreen.rs:199` `C_CYAN = (170, 249, 254)` = `#AAF9FE`, which is **pale,
not saturated** — the original audit's adjective was wrong (corrected after
the vision-route measurement).

**The real defect is the MATERIAL, and the fix is not white.** Measured on
`/tmp/ref/lock/l40.png`: the reference clock is **wallpaper-sampled frosting
inside the letterforms** — median ink RGB `[87,235,206]`, 92.8% teal-ish,
0.7% near-neutral, and it **varies across x** as the wallpaper changes. A flat
fill of any single colour is wrong, white included. See
`docs/lockscreen-spec.md` for the blur/zoom/veil recipe.

So this is **not** a "two-hue drift" tell: the hue comes from the wallpaper, so
it is one material honestly sampled, not a second invented accent. Do not
"fix" it by desaturating to white — that would move further from the reference.

### F8 — music panel grey pile (severity: low-medium)

`music_panel.rs:280-287`: TRACK_GREY #C8CCD0 (playing), CTRL_SIDE #B0B0B0
(loaded controls), PROGRESS_FILL #B0B0B0, DIM_GREY #6B6B6B, plus
DATE_GREY/IDLE_TEXT #C0C4C9 — five greys for text/control states in one
panel, with a comment history showing states were added one iteration at
a time ("three-state scheme"). Three states need two values: active
white/near-white, idle dim. **Fix:** collapse to `TEXT_SECONDARY` (F2) +
one `CONTROL_DIM`.

### F9 — theme-panel icon chip + pastel dot (severity: low-medium)

`themes_panel.rs:266` `C_ICON_DISC = (16,18,24)` chip behind every row
icon (`fill_circle` at :1149) plus a per-theme `icon_teal` dot (:1152,
values sampled "(APPROX)"!). Icon-behind-a-chip is the pastel-tile tell —
icons on the frost itself is the restrained move. **Fix:** drop the disc
(and the `(APPROX)` sampled teal); let the six theme dots already in the
row carry the colour information.

### F10 — duplicated highlight, scrollbar, field tokens (severity: low, consistency)

`C_HIGHLIGHT (5,22,60)` is defined identically in
`launcher_panel.rs:178`, `clipboard_panel.rs:107`, `themes_panel.rs:262`;
`C_SCROLLBAR (64,66,70)` in launcher + clipboard; `C_FIELD_BG (26,28,31)` in
launcher + as `C_BELL_BG` in `controlcenter_panel.rs:390`. Nothing is
*wrong* with the values (identical bytes), but per-file retyping is how
F1-style drift starts, and the comments already repeat the same rationale
three times. **Fix:** hoist to `library.rs` next to `ACCENT`.

## What is already restrained and must be protected

These are correct as-is; a later agent "improving" them would make the
UI look more AI-generated, not less. **Do not touch:**

- **The frost material** (`wallpaper_blur.rs`, `frost_fill_px`,
  `frost_fill_px_blur` in `main.rs:769-830`): one flat material, one
  floor (2.5), one alpha (0.92), no per-panel glass layers, no gradients.
  The iter128 re-grade rationale (fill ≈2-3 on pure black) is exactly the
  restraint the taxonomy asks for. No glassmorphism, no odd/even walls
  (`_d`/`_x`/`_y` params are deliberately inert — keep them so).
- **The single-source accent machinery that exists**:
  `library.rs::ACCENT` RwLock + `accent_for_theme` — the token system is
  already built; F1 is finishing it, not redesigning it.
- **Panel radius discipline**: 20 px panels, 36 px search field radius
  10, rows radius 9, tiles radius 4, toggles 2.5 — every radius relates
  to its element's height, no orphan values. Radii constants live beside
  their geometry, not scattered.
- **Type scale:** it *looks* scattered in constants (~20 `_PX` consts) but
  actually resolves to a sane ladder — ~10.5/11 body, 12/13 titles,
  16 chevrons, 19/20 glyphs, 36.5 hero clock — and sizes are set per
  *role* per panel with measured justification (all comments cite
  measured spans). Per-role sizing with measurement history is not slop;
  do not "normalise" it to a global 3-size scale. Do not consolidate
  `CLOCK_PX 36.5` or `TEXT_PX 13.4` — they are measured against real
  fontdue metrics, and the 36.5/13.4 oddities are corrections to round
  numbers, not laziness.
- **No emoji-as-icons anywhere; icons come from a real icon font**
  (Material Symbols Rounded via fontdue, `tray.rs` Adwaita/hicolor PNG
  fallback chain) — correct, keep it; the `\u{E1A7}` private-use bind-rune
  (`launcher_panel.rs:1081`) is a single documented exception.
- **No pulsing/breathing animations, no glow dots, no nested cards, no
  per-item separators** in the seven main panels — the one exception is
  dictation (F4) and the CC header divider (a single 1px structural rule,
  `controlcenter_panel.rs:423`, which is fine).
- **Real-data-only UI:** no invented stat rows, no fake numbers; the
  calendar/clipboard/music panels render live payload. Themes panel
  shows real swatches from theme palettes (`THEMES` array is data, not
  decoration — it is *content* there; keep it).
- **The `0.92` fill factor** on discs (`main.rs:2880,2951,2647`) and the
  alpha-coverage AA discipline (premultiplied SDF, supersampled discs)
  — this is correct rasterisation craft, not styling; protect it.
- **Shadow restraint outside F5's inconsistency**: the panel shadow
  itself (offset 5, blur 10, alpha 0.40) is small and physically
  plausible — the *right* size for a floating bar. The fix is unifying
  systems, not shrinking further.

## Priority order for fixing

1. F1 (accent unification) — highest value, enables real theming
2. F2+F3 (secondary text token) — one change, visible consistency win
3. F4 (dictation glow/gradients) — the largest "designed in a thumbnail" region
4. F6, F7 (rainbow mock lines, lone cyan)
5. F5 (shadow/rim unification — merges, not redesigns)
6. F8, F9, F10
