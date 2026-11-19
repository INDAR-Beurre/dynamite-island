# Lock screen — pixel-measured builder spec

Reference: `/home/alex/Downloads/Reeldrop/The Perfect Hyprland Lock Screen Setup (Every Choice Explained) [C6aSeHhlxsA].mp4`, 2560 × 1440, 60 fps, duration 1712.934 s. Measurements below are **device pixels**, not Wayland logical pixels. Read `docs/lockscreen-transcript-facts.md` first; the font and interaction requirements there remain authoritative. I directly viewed the supplied PNGs, enlarged crops, and newly extracted transition frames. No source edits, builds, process restarts, or live IPC were performed.

## 1. Critical correction: there are TWO designs in these frames

**Do not implement l60/l320 as the final lock screen, whether resting OR typing.** They show the author's *old, macOS-like comparison image*: a small upper clock reading **15:39**, a date, a profile, `user0`, an **empty** `Enter Password` field, top-right tray icons, and a globally blurred wallpaper. That image is inserted into a video whose actual desktop clock reads 11:28–11:34. A dense sequence shows the old image sliding out to reveal the actual desktop. It is not the final lock screen acquiring a transient profile while somebody types.

The final design has a **large clock centered on the screen**, a **small black lock capsule near the top**, and **no date, tray, avatar, username, bottom password chip, or placeholder**. When typing, password dots replace the padlock **inside the top capsule**. The clock and wallpaper stay in place. This agrees with the transcript's removal of complications; the earlier premise that l60/l320 must be typing frames is not supported.

### Classification of EVERY supplied full-size frame

| Frame | State/design | Visible evidence |
|---|---|---|
| `/tmp/ref/lock/l3.png` | Unlocked desktop | Small top clock 11:28; no central clock; sharp wallpaper. |
| `/tmp/ref/lock/l20.png` | Unlocked desktop | Same collapsed top clock, no central clock. |
| `/tmp/ref/lock/l40.png` | **Final locked, resting** | Central 11:28; top capsule contains padlock (cursor overlaps its lower-left edge); no bottom UI. |
| `/tmp/ref/lock/l60.png` | **Old comparison image; typing not demonstrated** | Upper 15:39/date, avatar/name, empty placeholder, tray, global blur. |
| `/tmp/ref/lock/l90.png` | Unlocked desktop / measurement baseline | Small top 11:29, sharp wallpaper. |
| `/tmp/ref/lock/l120.png` | **Final locked, typing** | Central 11:30; top capsule has two bright dots and a third appearing; no avatar/name/bottom chip. |
| `/tmp/ref/lock/l160.png` | Unlocked desktop with browser window | Gumroad/course browser visible; top clock 11:30. |
| `/tmp/ref/lock/l200.png` | Unlocked desktop / second baseline | Top 11:31, no central clock. |
| `/tmp/ref/lock/l260.png` | Unlocked desktop | Top 11:32, no central clock. |
| `/tmp/ref/lock/l320.png` | Old comparison image again | Same fixed 15:39 and empty placeholder; whole inserted image is about 6 px farther right than l60. |
| `/tmp/ref/lock/l400.png` | **Final locked, resting** | Central 11:34; top padlock, no other lock UI. |

The `*_h.png` files are downsampled copies, not additional states. Full-size files were used for numbers.

Additional independent evidence:
- `/tmp/ref/lock/dense14_091.png` (15.500 s): settled final resting state, top padlock, no profile/name/field.
- `/tmp/ref/lock/dense14_131.png` (16.167 s): final typing, five fully bright dots and a further dot appearing, widened capsule; no bottom UI.
- `/tmp/ref/lock/dense61_201.png` (64.333 s): final resting padlock.
- `/tmp/ref/lock/dense61_241.png` (65.000 s): final typing, six fully bright dots and another appearing, widened capsule.
- `/tmp/ref/lock/dense61_001.png` through approximately `/tmp/ref/lock/dense61_091.png`: old 15:39 image remains unchanged. `/tmp/ref/lock/dense61_101.png` (62.667 s) shows its slide-out and the real desktop being exposed. This is video presentation motion, not an authentication state change.

## 2. Final target: resting and typing layouts

Convention: `(x, y, w, h)` is the visible ink/capsule rectangle, upper-left inclusive, lower-right exclusive. Normalized columns mean `x/W, y/H, w/W, h/H`, W=2560, H=1440. Ink thresholds and compression introduce about ±1–2 px for small shapes and ±2–3 px for large glyph edges. Do not confuse ink bounds with font line boxes.

### Resting

| Element | Raw device rectangle / px | x/W | y/H | w/W | h/H |
|---|---:|---:|---:|---:|---:|
| Central clock, `11:28` in l40 | (843, 581, 872, 279) | .329297 | .403472 | .340625 | .193750 |
| Top black capsule, settled | (1246, 58, 68, 40) | .486719 | .040278 | .026563 | .027778 |
| Padlock ink, cursor-free dense14_091 | (1273, 69, 14, 17) | .497266 | .047917 | .005469 | .011806 |
| Wallpaper / veil | (0, 0, 2560, 1440) | 0 | 0 | 1 | 1 |
| Date, avatar, username, placeholder, separate input chip, tray | **ABSENT** | — | — | — | — |

Clock ink center ≈(1279, 720.5), or (.49961 W, .50035 H): implement **center (W/2,H/2)**, not an upper-screen clock. Capsule center (1280,78) = (.5 W,.054167 H); corner radius ≈20 px = .007813 W = .013889 H. Padlock is centered in the capsule within rasterization uncertainty.

For `11:28`, horizontal ink runs in the clock are: first `1` x843–986, second `1` x1003–1145, colon x1173–1253, `2` approximately x1277–1483, `8` approximately x1485–1714. The `1` advance is about **160 px**, whereas the final two wider digits are about **219 px apart**. **Do not force all digits into 0.60 em tabular cells** based on this frame: proportional-looking spacing is visibly tighter around `11`. Specific tracking/advance settings cannot be separated from the unavailable font metrics.

### Typing

Clock, wallpaper, veil, and shadow are unchanged. Only the top capsule changes contents/width. Keep its center at x1280 and y≈78; it does not become a bottom password field.

| Element / observed example | Raw rectangle / px | x/W | y/H | w/W | h/H |
|---|---:|---:|---:|---:|---:|
| Central clock | Same settled clock layout; actual time changes | .329297* | .403472 | .340625* | .193750 |
| l120 capsule: two full dots + next appearing | (1241,58,78,40) | .484766 | .040278 | .030469 | .027778 |
| dense14_131 capsule: five full dots + next appearing | (1216,58,128,40) | .475000 | .040278 | .050000 | .027778 |
| dense61_241 capsule: six full dots + next appearing | (1210,58,140,40) | .472656 | .040278 | .054688 | .027778 |
| dense61_261 capsule: ten fully bright dots | approximately (1179,58,202,40) | .460547 | .040278 | .078906 | .027778 |
| Dot diameter / ink | ≈8–9 px | — | — | .003125–.003516 | .005556–.006250 |
| Dot center pitch | ≈16.2 px | — | — | .006328 | .011250 |
| Padlock, date, avatar, username, placeholder, bottom input chip | **ABSENT** while dots are shown | — | — | — | — |

*Time-dependent ink width, not a fixed 872-px box for every possible time string. Use the font's correct advances and center the resulting clock.

In dense14_131 the bright dot centers are ≈1244,1260,1276.5,1293,1309 px; the further dim dot is appearing near1324. In dense61_261 the ten bright centers run ≈1206.5–1352.5, pitch≈16.2. Mid-animation capsule width **lags** the incoming dots. These are not settled `width(n)` samples; exact final side padding and per-character resize duration are not identifiable from the supplied rapid-typing sequences. Minimum builder rule: widen smoothly, keep height/center fixed, and do not clip the dots; do not derive pitch by dividing transient capsule width by count.

Author-stated interaction: first typed character reveals this password view; Esc or clearing the password removes it and restores the lock icon; wrong password shakes. Do not add identity UI in either state. Exact shake timing was not measured.

### Typography and color — final target

- **Family: SF Compact Display**, author's explicit choice; same family as collapsed clock. Use that named face rather than treating Quicksand as an exact match. Exact font file/optical variant is unavailable.
- Central clock cap/ink height **279 px = .193750 H = .108984 W**. First/second `1` vertical stems are **76–77 px**, stroke/cap **.272–.276**; other digit strokes are roughly70–86 px depending on contour. Terminals are conspicuously rounded. This is visually a **very heavy/Black-like** treatment; nominal weight≈900 is a starting hypothesis, NOT an identified font-table value.
- `/tmp/ref/s/s17.png`: collapsed black capsule approximately (1225,20,110,40); text ink (1253,33,54,15). Cap height **15 px = .010417 H = .005859 W**; zero side strokes≈2–3 px, stroke/cap≈.13–.20. Zero advances≈13 px. Text appears medium-to-semibold, not the giant clock's extreme weight. Same family is **high confidence from the transcript**, only **medium confidence from pixels**: common oval zeros/two-dot colon, but much heavier rounded large glyphs and low-resolution small text prevent uniquely proving the same face/variant. Do not claim same font *weight*.
- Capsule interior is **#000000**, flat opaque, core luminance0–2 with compression noise; it is a separate material from the clock. Capsule antialiasing is coverage only. No colored capsule border.
- Padlock sampled bright-pixel median in cursor-free crop is approximately **#e1d4cb**; typing-dot bright medians approximately **#ede2c9** / **#ebe0c7**. These tiny compressed glyphs do not contain a reliable flat full-coverage RGB plateau. Treat these as **observed composites**, warm off-white, with nominal opaque ink and AA coverage; their intrinsic color versus opacity cannot be separated. A warm light foreground, not mint, is the visually justified starting point. Pure #ffffff dots are brighter/cooler than the samples.
- Clock broad interior over teal is **#57ebce** (87,235,206), repeatedly identical in l40, l120, l400, and dense frames. BUT it is **not a flat mint fill**: l40 (920,600) is **#ffab88**, (910,650) **#c5a89d**, (950,800) **#6be8cc**, all inside the first `1`, matching the wallpaper's orange pole/purple supports below it. The color variation is tied to wallpaper content, **not** to an arbitrary horizontal cyan-to-white gradient.
- **Translucency belongs to the glyph-shaped clock surface itself.** There is no surrounding frosted rectangle. Sharp wallpaper continues through the holes and between glyphs; inside the first `1` the wallpaper is smoothly blurred and lightened. Implement a wallpaper-sampled material **clipped to the glyph mask**, plus its shadow. A white glyph over a blurred rectangular panel is wrong.
- A plain white source-over alpha is disproved quantitatively: at a teal pixel the unlocked backdrop is about (3,121,103) and clock ink is (87,235,206). Solving against white gives alpha≈.33,.85,.68 per R/G/B, not a common alpha. The recovered frosted material has gain>1; **true compositing alpha cannot be uniquely estimated** without knowing the shader/brightness operation. See the measured recipe below instead of inventing an RGBA value.

## 3. Reproducible backdrop and glyph-material recipes

### Registration is essential

The final locked wallpaper is **zoomed approximately1.030× around screen center** compared with unlocked l90. This was verified against visible pole/support edges and a scale scan1.000–1.070. Without registration a blur fit misleadingly reduces pixel error by smoothing misaligned edges.

Pillow inverse affine for the registered source, W=2560,H=1440:

```python
z = 1.030
registered = unlocked.transform(
    (W, H), Image.Transform.AFFINE,
    (1/z, 0, W/2*(1-1/z), 0, 1/z, H/2*(1-1/z)),
    Image.Resampling.BICUBIC,
)
```

### Full-screen backdrop: essentially sharp, vertically shaded

The **final** lock is not globally frosted like l60. Best gradient-matching global blur is only **PIL GaussianBlur radius0.6 px** after zoom registration, an image-filter/compression-scale effect rather than a visible UI blur. Radius0 is visually reasonable; do not apply a20–24px global blur to the final design.

Veil color: **#000000**, spatial alpha. The observations favor a **vertical** shade, not a radial vignette: at y720, both x100 and x2400 transmit≈.78, while top-center and top-corner patches transmit≈.62. A convenient fitted recipe is:

```text
top_ramp    = clamp((400 - y)/400, 0, 1)
bottom_ramp = clamp((y - 1080)/360, 0, 1)
transmission(y) = .77635045 - .20660489*top_ramp - .28306150*bottom_ramp
alpha(y) = 1 - transmission(y)
backdrop = GaussianBlur(registered, .6) * transmission(y)
```

Thus black alpha≈**.224** on the central plateau (y400–1080; .278–.750 H), **.430** at the top edge, **.507** at the bottom edge. Approximate measured patch alphas: y60 .400, y180 .341, y300 .273, y420 .224, y720 .226, y1080 .219, y1200 .326, y1320 .412, y1380 .449. Compression and dark scenery perturb individual patches. These are a fit, not proof of the author's original gradient stops.

Measurement mask: x50–2499,y90–1399, excluding central glyph region x761–1799,y531–909 and capsule region x1101–1449,y91–169. Luminance is **encoded-RGB Rec.709 luma**, `Y=.2126R+.7152G+.0722B`, not linear-light photometric luminance. Statistic is mean `abs(Y[x+1]-Y[x])` only where both adjacent pixels are valid. Veil coefficients were least-squares fitted using every eighth pixel on each axis; RGB RMSE evaluated over the whole mask.

| Global Gaussian radius, registered source | Predicted mean horizontal gradient | Locked measured gradient | RGB RMSE / 255 |
|---:|---:|---:|---:|
| 0 | 1.61419 | 1.48002 | 2.65249 |
| .5 | 1.52142 | 1.48002 | 2.54339 |
| **.6** | **1.48133** | **1.48002** | **2.59927** |
| .7 | 1.43786 | 1.48002 | 2.72212 |

Radius.6 gradient residual **+0.001318 intensity/px, +0.0891%**, RGB RMSE2.60/255. Least pixel error is radius.5, demonstrating that matching one statistic does not uniquely identify the compositor's filter. Radius1–4 oversmooth the final backdrop.

### Clock frosted surface: Gaussian radius24 px, glyph-clipped

Use the **registered, pre-veil wallpaper** to sample the glyph material; draw the separately veiled backdrop through the holes. Measured effective recipe:

```text
B = GaussianBlur(registered wallpaper, radius=24 device px)
material.r = clamp(1.19396*B.r + 83.78911, 0, 255)
material.g = clamp(1.25239*B.g + 83.09509, 0, 255)
material.b = clamp(1.25567*B.b + 76.69171, 0, 255)
output = backdrop*(1-glyph_coverage) + material*glyph_coverage
```

This is an **empirical equivalent**, not the original shader source or a physically inferred RGBA layer. A simpler common-channel approximation is `clamp(1.227918*B + 82.943521,0,255)`; RGB RMSE3.62 instead of2.39. Keeping wallpaper-driven hue is more important than hardcoding the coefficients as a universal palette. Radius24 = **.009375 W = .016667 H**; uncertainty approximately±2–4 device px for this particular Gaussian equivalent.

Fit region x800–1769,y550–889. Interior mask `R>60,G>145,B>115`, eroded4px with PIL MinFilter(9); 114,924 valid glyph-interior pixels. Per-channel affine fit excludes that channel's output>=248 to avoid fitting saturated pixels; predicted colors are clipped before residuals. Gradient is measured only across adjacent interior-mask pixels, so it measures frosted texture rather than glyph edges.

| Glyph blur radius / px | Mean predicted interior gradient | Observed | RGB RMSE / 255 |
|---:|---:|---:|---:|
| 4 | .21517 | .08264 | 11.3562 |
| 8 | .14225 | .08264 | 8.3394 |
| 12 | .11366 | .08264 | 5.7538 |
| 16 | .09959 | .08264 | 3.8387 |
| 20 | .08903 | .08264 | 2.6305 |
| **24** | **.08255** | **.08264** | **2.3931** |
| 28 | .07907 | .08264 | 3.0854 |
| 32 | .07489 | .08264 | 4.2693 |
| 40 | .06502 | .08264 | 6.7027 |

Radius24 gradient residual≈**-.00009 intensity/px (-.11%)**; RGB RMSE≈**2.39/255**. The common-channel approximation gives gradient.082521 versus.082638, residual-.000117 (-.142%). **This is the substantial blur to reproduce: inside the glyphs, not over the whole screen.**

### Shadow

There is a dark, diffuse glyph-following shadow, without a card. A fit around l40 glyph exteriors favors **black #000000, Gaussian sigma/radius≈16 px, offset(0,+4 px), opacity≈.248, no additional spread**. Normalized blur .006250 W/.011111 H; dy .001563 W/.002778 H. Fit tested radii2,4,6,8,10,12,16,20 and dy0,2,4,6,8,12.

Measurement: crop x780–1799,y520–939; binary glyph silhouette from the color threshold above; exterior band up to30px, exclude first3px to avoid AA; discard backdrop luma<=30. Fit fractional luma darkening. Best residual **.02025** of backdrop luma versus **.05734** with no shadow. Nearby alternatives (radius12,dy4,alpha.306; radius20,dy6,alpha.205) have residual.02166/.02179, so blur/opacity are **not uniquely determined**. Treat16px/+4px/.25 as a builder starting point, not exact recovered CSS. Zero horizontal offset and no extra dilation were sufficient; exact spread is not identifiable.

### What is NOT present

Final resting AND typing: no rectangular card/panel behind the clock, no glyph border/stroke, no card border, no avatar/name/date/weather/tray, no separate bottom input, no arbitrary fixed cyan specular gradient. There **is** a whole-screen vertical black veil and a wallpaper-dependent gradient/texture **inside glyphs**. The padlock capsule is a distinct opaque-black shape.

## 4. Dense lock transition: continuation of the pill, verified

Extraction kept native cadence with:

```sh
ffmpeg -ss 14 -i '... [C6aSeHhlxsA].mp4' -t 3 -fps_mode passthrough /tmp/ref/lock/dense14_%03d.png
ffmpeg -ss 61 -i '... [C6aSeHhlxsA].mp4' -t 5 -fps_mode passthrough /tmp/ref/lock/dense61_%03d.png
```

Frame `dense14_NNN` time=14+(NNN-1)/60 s; `dense61_NNN` time=61+(NNN-1)/60. Adjacent source frames sometimes repeat screen states; 60fps container does not imply60 unique UI poses each second.

| Time / frame | Capsule rectangle / px | Visible event |
|---|---|---|
| 14.867 / dense14_053 | (1225,20,110,40) | Ordinary top clock capsule; unshaded desktop. |
| ~14.967 / dense14_059 | same | Backdrop darkening/zoom starts before giant clock. |
| 15.100 / dense14_067 | same | Small clock still in capsule; central clock not yet visible. |
| 15.133 / dense14_069 | (1231,28,98,40) | Clock text has emerged below capsule; padlock begins appearing; capsule descends/shrinks. Clock ink roughly(1117,222,328,105). |
| 15.200 / dense14_073 | (1244,44,72,40) | Padlock evident; clock lower and larger, roughly(977,406,607,194). |
| 15.233 / dense14_075 | (~1249,52,63,40) | Capsule briefly narrower than final; clock roughly(907,499,746,238). |
| 15.267 / dense14_077 | (~1249,57,62,40) | Clock approaches screen center, roughly(858,563,844,270). |
| 15.333 / dense14_081 | (1246,58,68,40) | Capsule at rest; clock slightly overshoots below final placement (top≈589 versus581). |
| 15.500 / dense14_091 | (1246,58,68,40) | Settled central clock, padlock; no other UI. |

Clock-transition boxes use bright core row/column thresholds rather than exact AA bounds; accept approximately±3px. The main findings are not threshold-sensitive.

Second sequence confirms it: desktop at63.50, shading/zoom by63.567, extracted clock near capsule at63.667, padlock visible by63.733–63.767, large clock descending/growing at63.767–63.933, settled by≈64.1. Thus **the frames DO support the pill changing into a padlock and descending, with the clock emerging from it**. The symbol looks like a **padlock** (shackle + body), not an isolated drawn keyhole.

Important implementation differences from a fade: capsule moves from center(1280,40) to(1280,78), **38px descent = .026389 H**; width110→briefly≈62→68; clock moves down from near the capsule while scaling up, then makes a small spring-like overshoot/settle. The whole effect from first shading to stable clock takes≈.53s here, or≈.40s from initial clock emergence; most travel is complete within≈.20s of emergence. Exact easing and phase trigger timestamp are not recoverable; do not claim a250ms stationary fade reproduces this.

Typing begins later in these sequences; it changes the top capsule only. Unlock shows the reverse clock flight toward the pill, not just a fade at screen center, but a precise unlock timing/curve was not fitted. Expanded-island closure before locking is author-stated; these dense sequences start from a collapsed island and do not measure the expanded-state closure.

## 5. Older comparison design — measurements ONLY to disambiguate l60/l320

**This section is NOT the final builder target.** If the user later explicitly asks for the old macOS-style version, these numbers describe l60. Its resting-versus-typing distinction is not recoverable: it is a static image with an empty placeholder, NOT proven typing. Do not invent separate old resting/typing layouts.

| Old element | Raw rectangle / px | x/W | y/H | w/W | h/H |
|---|---:|---:|---:|---:|---:|
| Date ink `Friday, July 24` | (1159,166,218,31) | .452734 | .115278 | .085156 | .021528 |
| Clock ink `15:39` | (1048,233,449,142) | .409375 | .161806 | .175391 | .098611 |
| Avatar circle incl. ring, approximate | (1226,1113,80,80) | .478906 | .772917 | .031250 | .055556 |
| White person icon | (~1252,1137,30,32) | .489063 | .789583 | .011719 | .022222 |
| Username `user0` ink | (1234,1225,67,18) | .482031 | .850694 | .026172 | .012500 |
| Input capsule, approximate AA boundary | (1134,1275,266,42) | .442969 | .885417 | .103906 | .029167 |
| `Enter Password` ink, incl. lowercase descenders | (1156,1289,138,16) | .451563 | .895139 | .053906 | .011111 |
| Top-right battery/Wi-Fi bright-pixel union | (2445,12,74,19) | .955078 | .008333 | .028906 | .013194 |

Corrections to parent's measurements: old clock ink width is **449px, NOT ~299px**. The y1137–1168 range describes the **person icon**, not the80px avatar circle. Username h18, placeholder total ink h16 including descenders. Date-to-clock ink gap36px (clock top233−date bottom197). Clock ink center≈(1272.5,304), not the final design's center-screen clock.

Date ink center1268 is about12px left of screen center; username/circle are similarly left. l320 shifts the whole old image's UI and tray about6px to the right, so this is at least partly **inserted-image placement**, not a dependable final-layout alignment requirement. Font side bearings also affect the clock's ink center. Do not apply a-12.5px correction to the final clock.

Observed old composite colors (l60; eroded ink where possible):

| Element | Sampled composite hex | Material / alpha estimate |
|---|---|---|
| Clock median ink | **#bdd0cd**; broad plateau mode#bccecd | White-like over local backdrop, effective alpha≈.73–.75; not opaque pure white. |
| Date interior median | **#b5ccc9** | White-like AA/translucent ink; estimated full-coverage alpha≈.70–.75, lower confidence due narrow strokes/chroma noise. |
| Username interior median | **#d7edee** | White-like, effective alpha≈.84–.92; small strokes prevent precise separation from AA. |
| Placeholder interior median | **#b4ccca** (less-covered pixels#9dbeb9) | White-like over chip, effective alpha≈.60–.70. |
| Input chip empty right interior | **#476d65** | Translucent white-like fill, approximately.19–.23 over the local veiled backdrop; no opaque gray plate. |
| Avatar circle fill | **#2c6f67** | White over backdrop, approximately.17–.19. |
| Avatar ring bright segment | **#669b96** | White-like thin AA ring≈1–2px; estimated coverage-weighted alpha≈.35–.45. |
| Person icon | **#fffeff** | Essentially opaque white. |

Old clock vertical stem approximately20–23px /142 = .14–.16, considerably lighter than the final clock. Date caps≈23px/.01597 H (the31px total includes ascenders/descenders); username total ink18px, caps about17px; placeholder caps≈14px/.00972 H, total16px. Exact old font family/weight/tracking cannot be identified and should not be substituted for the author's final SF Compact Display choice.

### Old global blur reproduction, kept separate

Using l90 without final1.03 registration, the old design indeed requires a broad global blur. A UI-excluded mask (x50–2499,y90–1399, exclude x1001–1579,y91–419 and x1101–1479,y1081–1399), same luma/gradient definition, and a vertical-veil fit gives:

| Global Gaussian radius / px | Predicted gradient | Old measured | RGB RMSE /255 |
|---:|---:|---:|---:|
| 12 | .159137 | .109064 | 6.6417 |
| 16 | .121191 | .109064 | 5.1298 |
| **18** | **.108840** | **.109064** | **4.6175** |
| 20 | .099109 | .109064 | 4.2515 |
| 24 | .084623 | .109064 | 3.9148 |
| 28 | .075046 | .109064 | 4.0027 |

Gradient-matched old radius≈**18px**, residual-.000224 intensity/px **(-.206%)**; pixel-error minimum nearer24px, due screenshot alignment/resampling and imperfect filter/veil equivalence. Radius18 coefficients: transmission=.763442−.191313*top_ramp−.299218*bottom_ramp, using the same400/1080/360 ramps. Hence old black veil alpha≈.237 center/.428 top/.536 bottom. This fit is weaker than the final glyph-material fit; no unique original radius is implied.

The parent's full-frame numbers are **reproducible**, but they use **arithmetic RGB-channel mean**, not Rec.709 luma: l60 mean56.13314 versus l90 mean78.66284 (ratio≈.7136), with mean absolute horizontal gradient of that channel-mean image **.310601 versus1.988673** (6.403× reduction). They compare the **old screenshot** with the desktop and include different UI. They are not a recipe for the final screen. Rec.709 full-frame luma means instead are66.52462/93.22829, with gradients.29585/2.11883. In one more restrictive shared UI-excluded mask, old l60/unlocked Rec.709 means were65.10/91.71, ratio.710, and old gradient.113; excluding sharp UI is why that gradient is much lower. Do not conflate old global blur with final glyph-only frosting.

## 6. Current implementation deltas

Read all1135 lines of `/home/alex/rust-island-build/src/lockscreen.rs`. Source is being worked on concurrently; table refers to the observed file contents and line numbers at this measurement pass. Current code contains stale comments about a different reference clip (`00:05`, f0620); those are not measurements of this supplied video. Do not enable old date/avatar UI to fix the final lock.

| Element | Final reference | Current code (file:line) | Delta / priority |
|---|---|---|---|
| Clock material | Wallpaper-sampled, glyph-only blur≈24px, lightened/tinted; plateau#57ebce, orange/purple variation in first1 | `src/lockscreen.rs:196`, `src/lockscreen.rs:199-201`, `src/lockscreen.rs:1000-1009`: opaque white base then fixed cyan gradient | **Largest material error:** implement actual sampled frosted glyph material; remove arbitrary specular pass. |
| Actual cyan value | No universal cyan; teal composite here#57ebce | `src/lockscreen.rs:199`: `(170,249,254)` = **#AAF9FE**, alpha.9 fades by42% width | Verified: **NOT saturated cyan**; it is very pale cyan. Still the wrong material, and the flat-white remaining glyphs are wrong. |
| Font | Authored SF Compact Display; very heavy rounded clock treatment | `src/lockscreen.rs:918`, `src/lockscreen.rs:978`: supplied font documented as Quicksand Bold | Family mismatch; don't claim pixel parity. Nominal reference weight is uncertain. |
| Clock geometry | `11:28` ink872×279, centered | `src/lockscreen.rs:188-194`, `src/lockscreen.rs:411-436`, `src/lockscreen.rs:980-985`: .27H font, .70 cap ratio, four .60em cells plus .21em colon | Layout proxy predicts**1014.768×272.16 @1440**, about**16.4% too wide**, cap proxy2.5% too short. Actual raster ink depends on font. Keep centering, correct face/advances before adjusting nominal pixel size. |
| Digit spacing | `1` advances≈160px; wide-digit centers≈219px apart | `src/lockscreen.rs:672-717`: fixed tabular cells | Excess space around `11`; do not use a fixed-cell approximation as reference evidence. |
| Wallpaper | ≈1.030× centered zoom | `src/lockscreen.rs:941-959`: direct wallpaper copy | **Missing zoom**; produces measurable pole/support misregistration. |
| Global blur | Essentially sharp; equivalent radius.6 only | `src/lockscreen.rs:941-959`: no wallpaper blur | Mostly correct to leave backdrop sharp; **do not add old-style global blur**. Add substantive blur only inside clock. |
| Veil | Vertical central alpha≈.224, top≈.430,bottom≈.507; mid-height side edges remain≈.22 | `src/lockscreen.rs:205-206`, `src/lockscreen.rs:965-974`: radial .22→.42 | Current radial shade makes horizontal side edges at y720≈.17 alpha too dark; misses stronger bottom shading. |
| Shadow | Approx Gaussian16px,+4px,alpha.25; nonunique fit | `src/lockscreen.rs:202-204`, `src/lockscreen.rs:988-998`: box blur≈10.67px,+4px,alpha.35 @1440 | Vertical offset agrees; blur kernel/opacity differ. Upgrade after correcting font/material. |
| Idle capsule | 68×40, top58 | `src/lockscreen.rs:211-213`, `src/lockscreen.rs:873-890`: 76×36,top56 @1440 | Reference is8px narrower,4px taller,2px lower. Pure black is correct. |
| Padlock | Ink14×17, warm off-white; full padlock silhouette | `src/lockscreen.rs:229`, `src/lockscreen.rs:809-855`: nominal21.33px Material Symbols, white | Nominal font size is not ink bounds; measure actual glyph. Warm foreground is closer to video. |
| Password dots | Diameter8–9px,pitch≈16.2px; capsule animated | `src/lockscreen.rs:214`, `src/lockscreen.rs:228`, `src/lockscreen.rs:899-906`: dia12px,pitch16px @1440 | Pitch close; **dots about33–50% too large**. Width evolution inside this draw function is immediate, not a measured smooth per-character ramp. |
| Rest-only complication removal | No date/avatar/name/bottom field | `src/lockscreen.rs:363-366` default date None; `src/lockscreen.rs:1015-1022` optional date below clock | Default absence is **correct for final video**. Optional date's invented lower placement is not l60's upper date; keep it off. Avatar/name absence is correct even while typing. |
| Lock entry | Capsule descends38px with width undershoot; clock flies downward/outward and slightly overshoots; ~.53s including shade | `src/lockscreen.rs:296-318`: 100ms scrim then150ms ease-out, clock scale.92→1, `clock_dy:0.0` | **Missing clock travel/spring**, wrong chronology/duration. Frame evidence contradicts stationary scale/fade. |
| Wrong-password shake | Author says shake; waveform unmeasured | `src/lockscreen.rs:865-877`: fixed positive4*s offset while wrong | One fixed displacement is **not a shake**; exact intended waveform cannot be supplied from these frames. |
| Unlock | Visible reverse flight to top | `src/lockscreen.rs:325-335`: -120*s placeholder travel | Current160px travel @1440 is far smaller than center-to-top≈642px; exact endpoint attachment/timing not fitted. |

## 7. What I could NOT determine — do not present these as measured

1. **Original shader/true glyph opacity.** The pixel-equivalent gain+offset is measured; ordinary alpha blending fails. Brightness/contrast/saturation, texture sampling, true alpha, gamma and source color are non-identifiable from the compressed final pixels. No single RGBA fill is justified.
2. **Exact font file, optical variant, weight number, hinting/AA setting, or letter-spacing value.** SF Compact Display is explicitly authored; cap heights, stem widths, visible rounding and glyph runs are measured. Black≈900 is a visual hypothesis. Tiny pill text cannot authenticate the exact font binary.
3. **A unique original Gaussian/shader radius.** Radius24 is the final glyph gradient-equivalent after registration; global.6 and old18 are separate fits. Other filter kernels/resampling recipes can match these statistics. Old screenshot displacement weakens its fit further.
4. **Unique shadow blur/opacity/spread.** Nearby fits are nearly as good;16px/+4px/.25 is a practical equivalent, not recovered authored settings.
5. **Settled capsule width for each password length, exact per-dot resize duration, caret rendering or character-reveal behavior.** Captured typing is rapid and transitional. Dot pitch/diameter and observed widths are measured; stable padding is not.
6. **Exact easing functions, animation trigger timestamp, wrong-password shake waveform, or expanded-island closure timing.** Dense frames prove movement and small overshoot, not a specific spring constant. Wrong password is transcript evidence, not a measured failure sequence here.
7. **Old comparison image's resting-vs-typing state.** Empty placeholder and unchanged15:39 do not prove typing; do not relabel it as the final typing design.
8. **Intrinsic RGBA of tiny padlock/dots/old text.** Sampled hexes are compressed composites; AA coverage and opacity cannot be uniquely separated. Old alpha ranges assume white-like source ink and local backdrop interpolation.
9. **Current live rendering/security behavior.** No live capture was necessary and concurrent agents could race IPC. No socket commands were sent, no `quit`, no cargo, no panel restart. Code comparison is static and subject to concurrent edits; this visual spec does not certify an actual secure session lock.

### Builder acceptance checks

- Rest: only central clock and top padlock capsule; typing: same clock plus dots **in that capsule**, never an avatar/name/bottom field.
- On this wallpaper, the first `1` must contain softened orange/purple wallpaper texture; the other broad ink regions should look mint rather than pure white. Holes/gaps retain sharp veiled wallpaper.
- Match clock ink center and279px height before tuning colors. Check `11:28` width≈872px, not a fixed1015px tabular layout.
- Match capsule68×40/top58 at rest; dots8–9px and pitch≈16.2px.
- Lock must move the capsule and visibly emit/descend the clock; a standalone fade is not equivalent.

---

## PARENT CORRECTION — clock width, measured ink-vs-ink (2026-11-19 02:45)

Critic #27b reported "clock ~16% too wide, retune advances". **The direction is
right, the number was wrong, and the reason is different.** It compared an
ADVANCE BOX against an INK measurement — the same apples-to-oranges error class
this run has caught five times.

Reference re-measured by COLOUR, not luminance. A luminance threshold is
contaminated: `l40.png`'s wallpaper has a cream mast and an orange buoy inside
the clock's row band, so `lum>110` returned 2057x340 (80% of screen width).
Masking the glyph teal instead — `(G>170)&(B>150)&(R<200)&(G-R>40)` — gives:

| quantity | reference `l40.png` | ours (tabular, px=388.8) |
|---|---|---|
| string | `11:28` | same |
| ink bbox | x[847,1714] y[581,859] | — |
| ink size | **868 x 279** | **963.6 x 280** |
| width % of 2560 | 33.9% | 37.6% |
| centre x | **1280.5** (screen centre 1280) | centred by construction |
| ink aspect | 3.111 | 3.441 |

- **Height is already correct**: 280 vs 279 = +0.4%. Do NOT change `CLOCK_PX_FRAC`.
- **Width is +11.0%**, not +16%. Measured ink-vs-ink through a faithful
  reimplementation of `rasterize_clock`'s tabular cell layout (`src/lockscreen.rs:673-700`).
- Natural (non-tabular) advances give ink **824** = **-5.1%**, i.e. the tabular
  cells are what push it wide.
- **Solved minimal fix**: `DIGIT_ADVANCE_EM 0.60 -> 0.522`, `COLON_ADVANCE_EM
  0.21 -> 0.198` (`src/lockscreen.rs:190,192`) yields ink **867.9 = -0.01% error**.

**Deeper finding the critic missed — the reference is NOT tabular.** Measured
centre-to-centre glyph pitches: **123.0, 138.5, 166.5, 219.5 px**. Tabular spacing
is uniform by definition (ours: 233.3 px for every digit). His `1` is much
narrower than his `8`; that is proportional spacing, consistent with SF Compact
Display. A tabular layout cannot match those pitches at ANY single advance value.

Trade-off, stated so the fixer chooses knowingly: proportional advances match
the reference letterform but let the block width change as digits change
(`11:28` -> `11:29`), which shifts the centred clock. Tabular keeps it still.
**Recommendation: keep tabular, adopt 0.522/0.198.** The width match is exact and
the pill stays put; the pitch difference is a second-order letterform gap that
no available font can close anyway (SF Compact Display is absent from this
machine — `fc-list` finds no SF face — and cannot be shipped).
