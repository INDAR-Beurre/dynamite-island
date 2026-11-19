# Reference motion spec — measured from the creator's recording

**Scope.** Independent reference-only measurement. I did not inspect our render cost. All comparisons below are scale-invariant (fractions, ratios, ms, pose counts); no cross-screen pixel comparison is made.

## 1. Evidence and a dataset correction

| Item | Finding |
|---|---|
| Reference screenshots | `docs/reference/pill-settled.png` 111×42 pill; `docs/reference/panel-power.png` 538×201 panel centred at x=1279.5 (2560×1440 screen). |
| Source video | `/home/alex/Downloads/Reeldrop/The Perfect Dynamic Island Hyprland Setup (Every Choice Explained) [Iv8OVf-4Yxw].mp4`; `ffprobe`: 2560×1440, **60 fps**, 124457 frames. |
| `/tmp/ref/dense/d00001.png` … `d00675.png` | 900×410 crops, not full frames: crop `x=380..2180, y=0..820` then scale ×0.5 (companion `/tmp/ref/an/seq.py`). |
| `/tmp/ref/dense/d0001.png` … `d0180.png` | Separate full 2560×1440 sparse extraction; **not** 60 fps sequential frames. Used only as a cross-check. |
| 5-digit sampling interval | **200 ms (5 fps)**, not 16.7 ms. Verified by matching decoded video crops: `d00063` → t=12.4 s MAD 0.34; `d00389` → t=77.6 s MAD 0.08; `d00016` → t=3.0 s MAD 0.81. `/tmp/ref/an/seq_out.txt` also labels `t=(i-1)/5`. |
| Full-set timing | Full `d0050` matches small `d00064/65` (t≈12.6 s, MAD≈0.99 after downscale); full `d0064` matches small `d00080` (t≈15.8 s, MAD≈1.05). Exact 4-digit interval not established; not needed. |

**Consequence:** every duration below has ±200 ms quantisation. At 5 fps a ~0.3 s morph yields only 1–2 sampled intermediate poses. The source video is 60 fps, but the provided PNGs cannot tell us how many distinct source frames were drawn inside those 200 ms gaps. I do **not** claim a measured 60 fps pose count for him.

## 2. Method

For each 5-digit frame: `a = RGB array`, `dark = a.max(axis=2) <= 24`, inspect only `y=0..220`; bridge horizontal gaps ≤25 px (internal separators split the capsule), then take the bridged span containing crop-centre x=450. Boxes are in **crop-local** pixels; multiply by 2 for reference-screen pixels. Fractions use the reference screen (2560×1440).

Other UI was flagged by outside-dark pixels (`x<100 or x>800` in the small crop, top 220 rows, count >500): **68/675 frames** (`d00297–d00305`, `d00561`, `d00618–d00675`). In the full set the analogous threshold (`x<900 or x>1700`, top 260 rows, >1000) flags **45/180 frames** (`d0013–d0018`, `d0038–d0049`, `d0145–d0159`, `d0169–d0180`). Downscaled numeric inspection of `d00297`, `d00561`, `d00618`, `d00669` shows browser/settings-like windows; those ranges were excluded from pose/duration selection. Full `d0013` appears only in a labelled cross-check and is one of the flagged frames.

## 3. Duration (sampled windows, 5 fps)

“Window” = last settled frame → first settled frame including all sampled morph frames. “First-move→settled” = first frame whose bbox differs → first settled frame. Both are multiplied by the 200 ms PNG interval; true onset/settle can each lie anywhere in the adjacent sample gap.

| Transition | Frames (indices) | Window | First-move→settled | Distinct intermediate sampled poses |
|---|---|---:|---:|---:|
| Power panel open | `d00387` (pill) → `d00388` → `d00389` (settled) → `d00390` stable | **400 ms** | **200 ms** | 1 |
| Power panel close | `d00403` (panel) → `d00404` → `d00405` (pill) → `d00406` stable | **400 ms** | **200 ms** | 1 |
| Wide panel open | `d00210` (pill) → `d00211` → `d00212` → `d00213` (settled) | **600 ms** | **400 ms** | 2 |
| Wide panel close | `d00225` (panel) → `d00226` → `d00227` → `d00228` (pill) | **600 ms** | **400 ms** | 2 |
| Earlier panel close | `d00078` (panel) → `d00079` → `d00080` (pill) | **400 ms** | **200 ms** | 1 |
| Another close | `d00368` (panel) → `d00369` → `d00370` → `d00371` (pill) | **600 ms** | **400 ms** | 2 |
| Early large panel (not used as a clean morph) | `d00014` → `d00015` … `d00024` | 2000 ms | 1800 ms | multiple |

**Range from clean transitions:** sampled windows **400–600 ms**; first-move-to-settled **200–400 ms**. The likely power-panel settle is ~200–400 ms, but a 5 fps extraction cannot resolve it more tightly. The 2000 ms early case is a different/longer sequence (and has a width excursion), so it is reported but excluded from the target range.

Full-frame cross-checks: pill `d0008–d0011` w=110 h=42; first growth `d0012` w=380 h=96; settled power panel `d0050` w=538 h=201 (matches `docs/reference/panel-power.png`); `d0063` w=474 h=177; pill returns `d0064` w=110 h=42. These confirm the same kind of growth/collapse, at the full-set's coarser interval.

## 4. Pose count and easing samples

Boxes below are crop-local (w,h); normalize w and h independently as `(value-start)/(target-start)`.

### Power-panel open — `d00387` → `d00389`

| Frame | Normalised time (sample steps) | (w,h) local | Normalised (w,h) | Reference-screen fraction (w,h) |
|---|---:|---:|---:|---:|
| `d00387` | 0.00 | (56,20) | (0.000, 0.000) | (0.0438, 0.0278) |
| `d00388` | 0.50 | (192,71) | **(0.636, 0.630)** | (0.1500, 0.0986) |
| `d00389` | 1.00 | (270,101) | (1.000, 1.000) | (0.2109, 0.1403) |
| `d00390` | — | (270,101) | (1,1) | stable |

One midpoint cannot distinguish linear from an ease-out curve: linear predicts 0.50, ease-out-quad 0.75, ease-out-cubic 0.875 at t=0.5, while the sample is 0.63 — but its true time could be anywhere in the preceding/following 200 ms gap.

### Power-panel close — `d00403` → `d00405`

Collapse progress is `(start-current)/(start-target)`.

| Frame | Time | (w,h) | Collapse progress |
|---|---:|---:|---:|
| `d00403` | 0.00 | (269,100) | (0.000, 0.000) |
| `d00404` | 0.50 | (212,80) | **(0.263, 0.247)** |
| `d00405` | 1.00 | (52,19) | (1.000, 1.000) |

### Wide-panel open — `d00210` → `d00213` (best curve evidence)

| Frame | Time | (w,h) | Normalised (w,h) |
|---|---:|---:|---:|
| `d00210` | 0.00 | (54,20) | (0.000, 0.000) |
| `d00211` | 0.33 | (360,99) | **(0.495, 0.490)** |
| `d00212` | 0.67 | (682,183) | **(1.015, 1.012)** |
| `d00213` | 1.00 | (673,181) | (1,1) |

Reference curve values for comparison: at t=1/3, linear=0.333, ease-in-out≈0.259, ease-out-quad=0.556, ease-out-cubic=0.704; measured ≈0.49. At t=2/3, linear=0.667, ease-in-out≈0.741, ease-out-quad=0.889, ease-out-cubic=0.963; measured ≈1.01 (slight overshoot). **Classification: ease-out-like, early and fast; linear and ease-in-out are poor fits. Cubic vs quadratic cannot be separated with two samples.** The sampled poses are 0.49 at ~1/3 and target by ~2/3 of the window.

Earlier close `d00078` (537,210) → `d00079` (116,42) → `d00080` (54,19): collapse progress at the midpoint is **0.871/0.880**, also fast-out-like.

## 5. Overshoot

- Power-panel open `d00387→d00388→d00389`: width and height increase monotonically; **no observed overshoot**.
- Wide-panel open: `d00212` (682,183) exceeds the settled `d00213` (673,181) by **+1.5% width, +1.2% height**, then settles.
- Early large sequence: `d00016` width 412 local vs settled `d00024` width 338 local (+21.9%) — a different, 2 s sequence, not the power-panel morph. Do not use it to justify a large overshoot on standard morphs.

**Conclusion on our OutBack → EaseOutCubic change:** correct for the power panel. The only clean measured overshoot is ~1%, not 15%; EaseOutBack 1.15 was both unmatched and the source of the one-frame black slab (`src/main.rs:5048-5070` documents that regression). If any overshoot is reintroduced, keep it at ~1–2%, never 15%.

## 6. Choreography during the morph

- **Clock:** in the small crop, the clock zone `x=437..462,y=17..23` has 39 bright pixels (`min(rgb)>=180`) at `d00386/387`; **0** at `d00388`, `d00389`, `d00390`; returns (39 px) at `d00405`. Full-frame tight zone `x=1254..1306,y=34..47`: 226 px at `d0010/d0011`; 0 at `d0012`, `d0050`, `d0063`; 222 px at `d0064`. So the clock is gone by the first sampled morph frame and returns on settle; no scale-up or sliding clock is observed.
- **Panel content:** already present during growth. Power open `d00388` has 1585 light pixels (`max(rgb)>=60`) inside its 192×51 bbox vs 2094 in the settled `d00389` 270×50 bbox — 76% of settled light with 73% of settled area. Wide open `d00211` has 3245 light pixels vs 13952 at settled `d00213` (23% with 30% of the area). Content is not delayed until after the capsule finishes.
- **Tray / right top-bar element:** an accent-coloured component (within ±45 of `#79A1F5`) with bbox `x=1970..2082, y=0..60`, area 643, is unchanged in full frames `d0010` (pill), `d0012` (first growth), `d0050` (settled panel), `d0064` (pill). At least that element stays put.
- **Workspace discs:** **unmeasured.** Accent-coloured pixels are spread across the top bar and wallpaper; I could not isolate the workspace discs reliably enough to say whether they scale, fade, or move. No finding is made here.

## 7. Frame rate / duplicate frames

Raw PNG hashes: 5-digit set **655 unique / 675**, with only **4 consecutive-identical pairs** (max run 2); full set **180 unique / 180**, 0 consecutive pairs. The sampled recording is not padded with duplicated PNGs. This is a statement about the 5 fps/4 fps PNG sets, **not** a measurement of the source video's 60 fps duplicate rate.

## 8. Reference → ours → gap

| Dimension | Reference (measured) | Ours (given / repo) | Gap |
|---|---|---|---|
| Clean morph sampled window | 400–600 ms, 5 fps quantised (`d00387–389`, `d403–405`, `d210–213`) | 160 ms configured, **317 ms observed settle** | Nominal duration below reference sampled window; observed settle is inside it. Duration alone is not the defect. |
| Distinct poses | 3–4 sampled poses incl. endpoints; 1–2 mid poses per clean morph at 5 fps; source-60fps count **unmeasured** | 2–3 poses total in the morph | **Blocker**: ours presents almost no interpolated poses. Fix the pose pipeline, then tune duration. |
| Easing | Ease-out-like; wide-open progress 0.49 at ~1/3, 1.01 at ~2/3 (`d210–213`); cubic vs quad unresolved | EaseOutCubic (`src/morph.rs:148`, `src/main.rs:5050-5070`) | Reasonable; no evidence for a change. |
| Overshoot | 0% power panel (`d387–389`); +1.2–1.5% wide panel (`d212`→`d213`) | None after OutBack 1.15 was removed | Correct direction. If reinstated, target ~1–2%, not 15%. |
| Clock | Ink gone by first morph sample, back at settle (`d388`, `d405`; full `d0012`, `d0064`) | Outgoing scratch fades via `frame.outgoing_opacity` (`src/main.rs:3578-3584`) | No measurable mismatch at 5 fps; no change recommended. |
| Panel content | Present during growth at ~75%/23% of settled light (`d388`, `d211`) | Incoming scratch composited during morph (`src/main.rs:3698-3700`) | Aligned. |
| Top-bar/tray | Fixed accent component across pill/panel (`d0010`, `d0012`, `d0050`, `d0064`) | Not investigated here | No reference evidence that it should move with the capsule. |
| Workspace discs | Unmeasured | Not investigated here | Honest gap; do not invent a target. |

## 9. Bottom line for the owner

The reference target is **not “160 ms with 2–3 poses.”** Clean reference morphs occupy **200–400 ms first-move-to-settled (400–600 ms sampled windows)** and always show at least one distinct intermediate pose even at 5 fps; at source 60 fps the pose count is likely much higher but was not measurable from the supplied PNGs. Our observed 317 ms settle is already in the reference range — the missing piece is a distinct capsule pose on each displayed frame, with EaseOutCubic and no large overshoot.

Unmeasured / not claimed: exact source-60fps pose count, exact sub-200 ms easing shape for the power panel, workspace-disc behaviour, and any reference render-cost comparison.
