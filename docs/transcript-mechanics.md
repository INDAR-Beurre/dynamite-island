# Transcript mechanics — three unmined videos

Sources (auto-captions, so technical terms are garbled: "quick shell / quickshot / Quixel" =
Quickshell, "hyper land / hyperline" = Hyprland, "QS-P" = `qs -p`, "cute quick" = `QtQuick`).

1. `Learn Quickshell Positioning in 15 Minutes [MYJ3M9anz38].txt` — **T1**
2. `Hyprland's Hidden Layout, Set Up Properly [x78K7K7PYCw].txt` — **T2**
3. `Why Ricing Quickshell on Hyprland Is Still Hard for You [KLx_ktEhuUU].txt` — **T3**

Tags: **[S]** = he states it (verbatim quote follows). **[I]** = my inference, labelled as mine.

## Headline negative result (read this first)

**All three files contain zero statements about frame callbacks, damage tracking, timers,
redraw cadence, GPU cost, or why a shell feels laggy.** T3 is ~19 KB and is entirely about
learning strategy (don't go down the docs rabbit hole); T2 is about Hyprland's `monocle`
layout. The owner's #1 complaint ("it's really laggy") gets no material from these three
videos. The only performance-adjacent sentence in any of them is T3: **[S]** *"animations
are also super important because they're what separates Quickshell from everything else and
they're what take your setup to the next level."* — a taste claim, not a mechanism.

Do not treat anything below as a fix for the lag. It is geometry, plus two warnings.

## 1. Anchoring a top-centre bar (T1)

**[S]** *"anchors is how you stick a window to the edges of your screen. You guys have top
true, so it's glued to the top, and since I didn't pick a side, it just sits in the middle."*

**[S]** *"Change it to bottom, true, and it jumps to the bottom. It's literally just which
edges do you want it to stick to?"*

So: the centred position is produced by anchoring **top only** and letting the compositor
centre horizontally. He never computes a centre x. That is the opposite of our approach
(`src/main.rs:1216` — *"Live: 752 (screen centre 1504 / 2)"*; `src/main.rs:886` —
`x0 = (1504 - STAGE_W)/2 = 164`). Our 752/1504 pair is a hardcoded function of this one
output; his is output-independent. See §7.

**[S]** *"Then add left true, as well as right true … Of course, you want right true as well
for it to be a full proper bar"* → **[S]** *"Now, this is going to make it stretch across the
whole width, and when you stick a window, the two opposite edges, it has to touch both of
them, so it gets stretched to fill the screen … The implicit width doesn't matter anymore"*

**[I]** Consequence for a 3:2 panel: a left+right-anchored bar is *always* full screen width,
so its interior must be laid out with anchors from the edges, never with a fixed content
width, or the content drifts off-centre as the screen narrows. We are top-only-anchored with
an explicit stage width, so we are immune — but the tension is real: his layout scales with
the screen, ours is absolute. On 2256x1504@1.5 (logical 1504x1002, 3:2) a fixed 1100-wide
panel leaves 202 px either side; on his 16:9 1920x1080 it would leave 410. Nothing breaks,
it just reads wider. **[I]** No proportional sizing anywhere in his numbers, so the aspect
difference does not break his geometry.

## 2. Exclusive zone (T1 + T2) — this one we already get right

**[S]** (T1) *"And you'll notice that your other window is moved down to make room for the
bar. Quickshaw does that for you automatically when you anchor a bar like this, so your apps
never end up hiding behind it."*

**[S]** (T2) *"every tiled window on the workspace gets sized to the full usable area. So
that means your whole screen minus your bar and your gaps."*

**[I]** Read together: the bar's reserved strip is *subtracted from the tiling area*, so the
zone must equal the visible bar, not the surface. Our stage is 1176x636 with only the pill
visible and we reserve `pill_strip_px()` (`src/main.rs:6333`, `:6650`). Had we reserved the
stage, every tiled window would lose 636 px of height — on a 1002 px-tall logical screen
that is 63% of the desktop. T2 is the quote that makes that failure mode concrete. **No
change; keep the strip.**

## 3. Margins and the gap arithmetic (T1)

**[S]** *"For me, the margins are going to be top 18, top 18, as well as left 18, and right
18."*

**[S]** *"You see gaps out is 20 and because there's also shadow, there's like two pixels of
shadow, I've got to account for that and so that's where I got the 18 from."*

**[I]** His 18 is not a design constant — it is `gaps_out 20` minus a 2 px shadow allowance,
i.e. the bar's *visible* top edge lands 2 px below the top of the gap its neighbours get.
That logic is Hyprland-config-derived (`decoration { gaps_out }`) and **we skipped porting
it**: MangoWM has its own gaps and we do not read them, so there is no 20 to subtract from.
Our `gap_top` (default 3, `src/main.rs:236`) is an independent value and stays one.

**[S]** *"If you ever set a margin and nothing moves, check that you've actually anchored
that side."* — then, ~20 s later, he tests it and retracts:

**[S]** *"Okay, now never mind. I thought that it wouldn't work. So doesn't matter. You can
add margin bottom as well and that's just going to push the windows down."*

**Do not cite "margins only work on anchored edges" as a rule.** He asserted it and then
disproved it in the same video. The captions contain both halves; a reader who quotes only
the first sentence gets a wrong rule. (Relevant to us because we have `STAGE_PAD_TOP` and
`gap_top` doing margin work in one dimension only.)

## 4. The shape: transparent window + filled rectangle (T1)

**[S]** *"the window is going to get in the way we don't have that much customization
ability. So what we're going to do is we're going to make a rectangle and we're going to
make the rectangle actually fill the panel window."*

**[S]** *"a rectangle with an anchors dot fill of parent"* … *"anchors dot fill parent
basically means stretch me to fill whatever I'm inside of"*

**[S]** radius: first *"a radius of something more that's more rounded, something like 20"*,
then *"Maybe it will change this to something like 15 matches around theness of the windows."*

**[I]** This is our architecture exactly — one surface, an SDF capsule drawn inside it — and
it implies the corner radius belongs to the drawn capsule, not to the surface or the
compositor. That is already true of `src/main.rs` (manual SDF into wl_shm). No change.
The 20 → 15 wobble is him eyeballing; **neither number is a measurement**, do not adopt
either for the pill.

**[S]** *"if I wanted to completely make the bar disappear, I would just change the hex color
to a transparent"* — **[I]** same trick our transparent stage relies on; it is a known
Quickshell idiom, not a hack of ours.

## 5. Interior numbers (T1) — usable as proportion hints only

All from the tutorial bar, which is a Waybar-shaped status bar, **not** the Dynamite pill:

| item | [S] verbatim | note |
|---|---|---|
| window | *"implicit width … 300"*, *"implicit height, set that to something like 40"* | 7.5:1 bar, thin |
| inner left inset | *"incurs dot left margin is going to be 16"* | 16 px |
| right group | *"an anchors dot right margin of 16"*, *"you have a spacing of eight"* | 16 / 8 |
| dots | *"the rectangle width is going to be 12, height is going to be 12, radius is going to be just height divided by two"* | Ø12, r = h/2 |
| clock weight | *"font dot weight of something like 600"* | 600 |

**[I]** Two structural facts worth keeping because they are ratios, not values: (a) the
corner radius is stated as `height / 2` for round things — pill-ness is derived from height,
never a separate magic number; (b) horizontal padding (16) is ~40% of bar height (40), which
is the same order as our `inner_padding` default 14 against `bar_height` 28 (50%). Both sit
in the same band, so our padding is not out of family.

**Colours in T1 are unusable and must not enter the palette.** The captions mangle every hex:
*"a color will set it to something like 1E1E2E"* survives, but *"C zero C A F five"*,
*"F of nine e six four"*, *"seven seven DC triple F"*, *"nine EC, nine EC, E six, eight, six,
eight"* are not recoverable with confidence. **[I]** They look like the Tokyo Night /
Catppuccin families (≈#1E1E2E, #C0CAF5, #F9E64F, #7DCFFF, #9ECE6A), and these are the
*tutorial's decorative traffic-light dots*, not the island. Do not let a later worker
"recover" one of these and call it measured — that is how the invented-colour incident
happened.

## 6. Warning: anchoring to a non-parent, non-sibling (T1)

**[S]** *"You can only anchor something to its parent or to something sitting right next to
it inside the same parent. If you try to anchor to something buried inside a different item,
it won't work."*

**[S]** *"it cannot anchor to an item that isn't apparent or a sibling"* (caption garble for
"a parent or a sibling") — and the failure mode: *"The thing that you were trying to place
just gives up and sits in the top left corner."*

**[I]** The interesting part is the failure mode, not the rule: no exception, no crash — the
item silently lands at (0,0). We have no QML, so the rule does not transfer; the *class* of
bug does. Any coordinate we derive from another element (disc centres, tray row, `panel_top_menu()`)
must be a shared function, or it will silently pin to the top-left of the stage. That is
already the pattern in `src/main.rs` (`nav_disc_centres()` shared by draw + hit-test +
input region).

## 7. Multi-monitor (T3)

**[S]** *"you have a variance, which basically makes one bar per monitor. And that's like the
majority of what is discussed inside of this particular section."* (caption "variance" =
Quickshell `Variants`.)

**[I]** One panel instance per output is the normal shape; ours is a single surface whose
centre is the literal 752/1504 pair. On a second output, or if the owner ever changes scale,
we would draw an island centred on the wrong screen. This is **not** something the transcripts
prove is broken today — it is the one place where his stated model and our code differ in
kind: his position is *derived from the compositor*, ours is *derived from a constant*.

## 8. Layer choice — the one probable behavioural mismatch (T2)

**[S]** (T2, on fullscreen under `monocle`) *"So you can go through full screen over your bar
whenever you need to."*

**[I]** That sentence only makes sense if his bar sits on the `top` layer, which fullscreen
windows cover. Our header says *"Layer: overlay"* (`src/main.rs:12`) — overlay draws **above**
fullscreen surfaces, so our pill would hang over a fullscreen video where his disappears.
**[I, unverified]** I did not check a frame of his dynamic-island video for this, and
Quickshell's `PanelWindow` default layer is something I am recalling, not quoting. Treat as a
hypothesis to confirm from frames, not a finding. If confirmed, the change is one enum.

Note the cost of switching: an overlay surface is also what lets us sit above the lockscreen
preview. His bar being covered by fullscreen is a Hyprland-layer fact, **not** a deliberate
design decision he explains anywhere in these three files.

## 9. What I skipped, and why

- T2 in its entirety except §2 and §8: it is `monocle` layout (`windowrule`, `hl.dsp.window.cyclenext`,
  `hl.workspace_rule`, `hyperctl activeworkspace`). Hyprland display-server config we cannot
  consume on MangoWM. Numbers there are about *window* geometry, not shell geometry.
- Smart-gaps warning from T2 — **[S]** *"that rule counts visible tile windows and monocle
  only ever shows one. So on a monocle workspace, that rule fires all the time and you'll
  never see gaps"* — kept only as a pattern: a "single visible window ⇒ drop gaps" rule
  misfires under any stacked/monocle model. MangoWM's gap handling is its own; we do not
  implement such a rule.
- T3 apart from §7: pedagogic. It names two doc pages worth reading (*Introduction* = getting
  data via `Process`; *Item size and position* = *"how actually you're supposed to place
  items inside of your panel window"*) and otherwise sells a course.
- Fractional scaling: **nothing in any of the three files**. No mention of scale, DPI, or
  per-output scale. Our 1.5 is unaddressed by this material — the earlier docs' 1504x1002
  derivation stands on its own.

## 10. Net effect on the build

1. Nothing here explains the lag. Look elsewhere (our own `docs/smoothness-baseline.md`
   capture path is the honest instrument; these transcripts are not).
2. Exclusive zone: confirmed correct as a *visible-strip* zone, with T2 supplying the reason.
3. Anchor-top-only centring: correct in principle, wrong in our implementation's *provenance*
   — we hardcode 752 instead of deriving from output width at configure time.
4. Layer: probable mismatch (overlay vs top), unverified, one-line fix if frames confirm.
5. Margin rule from T1 is self-retracted; do not propagate it.
6. Every hex in T1 is garbled except #1E1E2E, and none of them belong to the island.
