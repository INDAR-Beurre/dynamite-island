# Bundled fonts

All three families are under the SIL Open Font License 1.1. Licence texts are
alongside: `OFL-Inter.txt`, `OFL-Quicksand.txt`.

| file | what it is | used for |
|---|---|---|
| `Inter-Clock.ttf` | Inter instanced at **wght 675, opsz 24**, `fvar`/`STAT` removed | the collapsed pill's clock |
| `Inter-Medium.ttf` | Inter instanced at wght 550, opsz 20 | earlier clock bake, kept for comparison |
| `Quicksand-Bold.ttf` | Quicksand Bold | panel headings and labels |
| `Quicksand.ttf` | Quicksand Regular | body text |

## Why the clock has its own bake

The reference's clock stems measure **stem/cap 0.1846** (subpixel, four frames,
two wallpapers). The wght-550 bake gives 0.1576 — **14.6 % too light**. Modal
*integer* stem width hides this: both round to 3 px at one size and 2 px at
another, so the defect survived several review rounds.

wght 675 / opsz 24 gives 0.1883 (+2.0 %). Its glyphs are still 4.3 % narrow,
which is a separate axis: apply tracking of **+0.0275 × text_px** and width
lands at −0.3 %.

Reproduce the bake:

```python
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont
ft = TTFont("~/.local/share/fonts/Inter.ttf")   # the variable original
instantiateVariableFont(ft, {"wght": 675, "opsz": 24}, inplace=True)
del ft["fvar"]; del ft["STAT"]
ft.save("assets/fonts/Inter-Clock.ttf")
```

`fontTools.varLib.instancer` is required — PIL's `set_variation_by_axes`
silently no-ops on this file and renders every weight identically.

Gate it with:

```
python3 scripts/fontcheck.py assets/fonts/Inter-Clock.ttf 12.105 0.0275
```
