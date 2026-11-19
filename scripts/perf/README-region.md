# region_exactness — bit-exactness gate for the damage-rect upscale

`upscale_canvas_2x_region` (`src/main.rs:615`) upscales only the damage rect.
That is the perf fix, and it introduces a failure mode the full-stage version
cannot have: **a seam at the rect boundary**. The blend reads neighbours
(`i-1`, `i+1`), so a rect that starts mid-image must still read the source
pixels *outside* the rect, or its first row/column differs from the full-stage
result and you get a visible edge on every settled frame.

This test extracts the three functions verbatim from `src/main.rs` and asserts
that for every rect tested, every pixel inside it equals what
`upscale_canvas_2x_into` writes there.

Result: **847 rect cases across 8 sizes, 0 mismatches.** Includes 1×1, 1×N, N×1,
odd sizes, rects touching each edge, and rects entirely outside the image
(clipped to nothing).

Reproduce (no cargo, no crate, does not touch the build):

```sh
python3 scripts/perf/extract_region_fns.py     # refresh from src/main.rs
rustc -O -o /tmp/region_check scripts/perf/region_exactness.rs
/tmp/region_check
```

Sizes ≥9 px use a representative coordinate set rather than all rects — the
exhaustive sweep at 64×48 allocates ~2 M full buffers and exceeds a minute.
Small sizes (≤8) are exhaustive.
