#!/usr/bin/env python3
"""Refresh region_fns_extracted.rs from src/main.rs (brace-matched, verbatim)."""
import pathlib, re
repo = pathlib.Path(__file__).resolve().parent.parent.parent
t = (repo / "src" / "main.rs").read_text()
out = []
for name in ("upscale_canvas_2x_into", "upscale_canvas_2x_region", "clip_rect"):
    m = re.search(r"(?:pub\(crate\) )?fn " + name + r"\b", t)
    if not m:
        raise SystemExit(f"missing {name} in src/main.rs")
    i = t.index("{", m.start()); d = 0; j = i
    while True:
        if t[j] == "{": d += 1
        elif t[j] == "}":
            d -= 1
            if d == 0: break
        j += 1
    out.append(t[m.start():j + 1])
dst = pathlib.Path(__file__).parent / "region_fns_extracted.rs"
dst.write_text("\n\n".join(out) + "\n")
print(f"extracted {len(out)} fns -> {dst}")
