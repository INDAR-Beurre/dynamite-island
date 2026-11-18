// Standalone proof for critic #6's headline perf finding. No crate, no Cargo
// change, no src/ edit: build with
//   rustc -O -o /tmp/upscale_bench scripts/perf/upscale_bench.rs && /tmp/upscale_bench
//
// Claim under test: `upscale_canvas_2x_into` costs ~30 ms for a 1176x636 stage
// against a documented ~5 ms, and the cost is structural (each source pixel's
// horizontal blend is recomputed for all four destination pixels it feeds).
//
// A candidate is only acceptable if it is BIT-EXACT against the current
// implementation: the existing code rounds the horizontal blend to an integer
// BEFORE the vertical blend, and fusing the two into one 16-weight sum changes
// output by +/-1. Speed that changes pixels is a regression, not a fix.

/// Current implementation, copied verbatim from src/main.rs:523-556.
fn current(src: &[u8], w: u32, h: u32, dst: &mut [u8]) {
    let (w, h) = (w as usize, h as usize);
    if w == 0 || h == 0 {
        return;
    }
    let (bw, bh) = (2 * w, 2 * h);
    for dy in 0..bh {
        let j = dy / 2;
        let (y0, y1, wy0, wy1) = if dy % 2 == 0 {
            (j.saturating_sub(1), j, 1u32, 3u32)
        } else {
            (j, (j + 1).min(h - 1), 3u32, 1u32)
        };
        for dx in 0..bw {
            let i = dx / 2;
            let (x0, x1, wx0, wx1) = if dx % 2 == 0 {
                (i.saturating_sub(1), i, 1u32, 3u32)
            } else {
                (i, (i + 1).min(w - 1), 3u32, 1u32)
            };
            let di = (dy * bw + dx) * 4;
            for c in 0..4 {
                let s00 = src[(y0 * w + x0) * 4 + c] as u32;
                let s01 = src[(y0 * w + x1) * 4 + c] as u32;
                let s10 = src[(y1 * w + x0) * 4 + c] as u32;
                let s11 = src[(y1 * w + x1) * 4 + c] as u32;
                let top = (wx0 * s00 + wx1 * s01 + 2) / 4;
                let bot = (wx0 * s10 + wx1 * s11 + 2) / 4;
                dst[di + c] = ((wy0 * top + wy1 * bot + 2) / 4) as u8;
            }
        }
    }
}

/// Candidate: hoist the horizontal blends out of the inner loop.
///
/// For output row dy the two source rows (y0,y1) are fixed, and for output
/// column dx the horizontal weights depend only on dx%2. So compute, once per
/// source row, the two horizontal blends every destination column needs:
///   hl[i] = (1*p[i-1] + 3*p[i]   + 2) >> 2   (even dx)
///   hr[i] = (3*p[i]   + 1*p[i+1] + 2) >> 2   (odd dx)
/// then each destination pixel is one vertical blend of two cached values.
/// Same double-rounding order as `current`, so bit-exact by construction.
fn hoisted(src: &[u8], w: u32, h: u32, dst: &mut [u8], scratch: &mut [u32]) {
    let (w, h) = (w as usize, h as usize);
    if w == 0 || h == 0 {
        return;
    }
    let bw = 2 * w;
    // scratch holds 2 rows x w pixels x 2 phases x 4 channels
    debug_assert!(scratch.len() >= 2 * w * 2 * 4);
    let row_stride = w * 2 * 4;

    // Blend one source row into its slot: [phase][i][c]
    fn blend_row(src: &[u8], w: usize, y: usize, out: &mut [u32]) {
        let base = y * w * 4;
        for i in 0..w {
            let il = i.saturating_sub(1);
            let ir = (i + 1).min(w - 1);
            for c in 0..4 {
                let p = src[base + i * 4 + c] as u32;
                let l = src[base + il * 4 + c] as u32;
                let r = src[base + ir * 4 + c] as u32;
                out[(i * 2) * 4 + c] = (l + 3 * p + 2) >> 2;
                out[(i * 2 + 1) * 4 + c] = (3 * p + r + 2) >> 2;
            }
        }
    }

    let mut cached = [usize::MAX; 2];
    for dy in 0..(2 * h) {
        let j = dy / 2;
        let (y0, y1, wy0, wy1) = if dy % 2 == 0 {
            (j.saturating_sub(1), j, 1u32, 3u32)
        } else {
            (j, (j + 1).min(h - 1), 3u32, 1u32)
        };
        // Rows repeat across consecutive dy, so only reblend when they change.
        if cached[0] != y0 {
            let (a, b) = scratch.split_at_mut(row_stride);
            let _ = b;
            blend_row(src, w, y0, &mut a[..row_stride]);
            cached[0] = y0;
        }
        if cached[1] != y1 {
            blend_row(src, w, y1, &mut scratch[row_stride..2 * row_stride]);
            cached[1] = y1;
        }
        let (r0, r1) = scratch.split_at(row_stride);
        let di_row = dy * bw * 4;
        for i in 0..w {
            for phase in 0..2 {
                let si = (i * 2 + phase) * 4;
                let di = di_row + (i * 2 + phase) * 4;
                for c in 0..4 {
                    let top = r0[si + c];
                    let bot = r1[si + c];
                    dst[di + c] = ((wy0 * top + wy1 * bot + 2) >> 2) as u8;
                }
            }
        }
    }
}

fn lcg(state: &mut u64) -> u8 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
    (*state >> 33) as u8
}

fn bench<F: FnMut()>(mut f: F, reps: usize) -> std::time::Duration {
    let mut best = std::time::Duration::MAX;
    for _ in 0..reps {
        let t = std::time::Instant::now();
        f();
        best = best.min(t.elapsed());
    }
    best
}

fn main() {
    let mut fails = 0;

    // 1. Bit-exactness on random premultiplied canvases, including the
    //    degenerate sizes where the edge clamps bite hardest.
    for &(w, h) in &[(1u32, 1u32), (1, 7), (7, 1), (2, 2), (3, 5), (17, 13), (64, 48)] {
        let mut st = 0x1234_5678u64;
        let mut src = vec![0u8; (w * h * 4) as usize];
        for v in src.iter_mut() {
            *v = lcg(&mut st);
        }
        let n = (2 * w * 2 * h * 4) as usize;
        let mut a = vec![0u8; n];
        let mut b = vec![0u8; n];
        current(&src, w, h, &mut a);
        let mut scratch = vec![0u32; 2 * (w as usize) * 2 * 4];
        hoisted(&src, w, h, &mut b, &mut scratch);
        let diffs = a.iter().zip(&b).filter(|(x, y)| x != y).count();
        if diffs == 0 {
            println!("exact  {w:>4}x{h:<4} OK");
        } else {
            fails += 1;
            let first = a.iter().zip(&b).position(|(x, y)| x != y).unwrap();
            println!("exact  {w:>4}x{h:<4} FAIL {diffs} bytes differ, first at {first}: {} vs {}", a[first], b[first]);
        }
    }

    // 2. Timing at the real stage size, and at the pill size the stage is
    //    mostly overwritten by (critic #6: 8,288 of 2,991,744 dst px survive).
    for &(w, h, label) in &[
        (1176u32, 636u32, "full stage 1176x636"),
        (110, 41, "pill only 110x41"),
    ] {
        let mut st = 99u64;
        let mut src = vec![0u8; (w * h * 4) as usize];
        for v in src.iter_mut() {
            *v = lcg(&mut st);
        }
        let n = (2 * w * 2 * h * 4) as usize;
        let mut dst = vec![0u8; n];
        let mut scratch = vec![0u32; 2 * (w as usize) * 2 * 4];
        let tc = bench(|| current(&src, w, h, &mut dst), 5);
        let th = bench(|| hoisted(&src, w, h, &mut dst, &mut scratch), 5);
        println!(
            "time   {label:<22} current {:>8.2?}  hoisted {:>8.2?}  speedup {:.2}x",
            tc, th, tc.as_secs_f64() / th.as_secs_f64()
        );
    }

    if fails > 0 {
        println!("RESULT FAIL: {fails} size(s) not bit-exact");
        std::process::exit(1);
    }
    println!("RESULT PASS: candidate is bit-exact at every size tested");
}