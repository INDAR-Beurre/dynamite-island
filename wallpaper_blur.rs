//! Fake frosted-glass backdrop (iter29, a3 retry).
//!
//! The compositor blur provably does NOT affect this layer surface
//! (iter28 A/B test: phase A vs B pixel-identical, mean|Δ| = 0.00), so
//! uniform translucency just shows SHARP wallpaper bleeding through —
//! the left tip of the pill reads 130–202 over a bright cloud while the
//! interior reads ~96. The fix is faked in-buffer: the island reads the
//! live wallpaper file itself, downsamples it to logical screen size,
//! box-blurs it (radius 12 logical px), and composites the blurred crop
//! UNDER the dark glass fill, so the surface pixel is (near-)opaque and
//! no sharp wallpaper ever leaks through the compositor.
//!
//! Cache: one blurred logical-size RGB buffer (1504x1002x3 ≈ 4.5 MB),
//! keyed by the wallpaper file's (mtime, size). `ensure_loaded()` stats
//! the file (cheap) on every draw and only re-decodes/re-blurs when the
//! key changes, so a wallpaper change is picked up on the next draw.
//! Unreadable/undecodable wallpaper → `None`, and callers fall back to
//! the iter28 black-fill behaviour.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// Logical screen size (mango eDP-1: 1504x1002 @ scale 1.5).
pub const LOGICAL_W: usize = 1504;
pub const LOGICAL_H: usize = 1002;
/// Box-blur radius in logical px (task range ~8–12).
pub const BLUR_RADIUS: usize = 12;

fn wallpaper_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/alex".to_string());
    PathBuf::from(home).join(".config/mango/wallpaper/wallpaper.png")
}

/// Blurred wallpaper in logical screen coords: RGB, LOGICAL_W x LOGICAL_H.
pub struct BlurredWallpaper {
    pub data: Vec<u8>,
}

impl BlurredWallpaper {
    /// Nearest sample at logical screen coords (clamped to the screen).
    /// The buffer is already blurred, so nearest is smooth enough.
    pub fn sample(&self, sx: f32, sy: f32) -> [u8; 3] {
        let x = (sx.floor() as i32).clamp(0, LOGICAL_W as i32 - 1) as usize;
        let y = (sy.floor() as i32).clamp(0, LOGICAL_H as i32 - 1) as usize;
        let i = (y * LOGICAL_W + x) * 3;
        [self.data[i], self.data[i + 1], self.data[i + 2]]
    }
}

struct CacheEntry {
    mtime_secs: u64,
    mtime_nanos: u32,
    len: u64,
    /// None = the load failed for this key (do not retry every frame).
    blurred: Option<Arc<BlurredWallpaper>>,
}

static CACHE: Mutex<Option<CacheEntry>> = Mutex::new(None);

/// Stat the wallpaper; (re)load + (re)blur only when (mtime, size)
/// changed. Cheap after the first load: one stat + one mutex lock.
/// Returns None when the file is unreadable or undecodable.
pub fn ensure_loaded() -> Option<Arc<BlurredWallpaper>> {
    let path = wallpaper_path();
    let md = std::fs::metadata(&path).ok()?;
    let mtime = md.modified().ok()?;
    let (secs, nanos) = match mtime.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => (d.as_secs(), d.subsec_nanos()),
        Err(_) => (0, 0),
    };
    let len = md.len();
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(e) = guard.as_ref() {
        if e.mtime_secs == secs && e.mtime_nanos == nanos && e.len == len {
            return e.blurred.clone();
        }
    }
    let blurred = load_and_blur(&path).map(Arc::new);
    *guard = Some(CacheEntry {
        mtime_secs: secs,
        mtime_nanos: nanos,
        len,
        blurred: blurred.clone(),
    });
    blurred
}

fn load_and_blur(path: &Path) -> Option<BlurredWallpaper> {
    let (rgb, w, h) = decode_png_rgb(path)?;
    let mut logical = downsample_to_logical(&rgb, w, h);
    box_blur_rgb(&mut logical, LOGICAL_W, LOGICAL_H, BLUR_RADIUS);
    Some(BlurredWallpaper { data: logical })
}

/// Decode a PNG to 8-bit RGB (any colour type the `png` crate reports).
/// Returns (rgb, width, height). Interlace is handled by the reader.
fn decode_png_rgb(path: &Path) -> Option<(Vec<u8>, usize, usize)> {
    use png::ColorType;
    let file = File::open(path).ok()?;
    let decoder = png::Decoder::new(file);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width as usize, info.height as usize);
    if w == 0 || h == 0 || info.line_size == 0 {
        return None;
    }
    let sixteen = info.bit_depth == png::BitDepth::Sixteen;
    let palette: Vec<u8> = info.palette.clone().unwrap_or_default();
    // Bytes per pixel in the decoded buffer.
    let bpp = match info.color_type {
        ColorType::Rgb => {
            if sixteen {
                6
            } else {
                3
            }
        }
        ColorType::Rgba => {
            if sixteen {
                8
            } else {
                4
            }
        }
        ColorType::Grayscale => {
            if sixteen {
                2
            } else {
                1
            }
        }
        ColorType::GrayscaleAlpha => {
            if sixteen {
                4
            } else {
                2
            }
        }
        ColorType::Indexed => 1,
    };
    let mut out = vec![0u8; w * h * 3];
    for y in 0..h {
        let row = buf.get(y * info.line_size..(y + 1) * info.line_size)?;
        for x in 0..w {
            let o = (y * w + x) * 3;
            let s = x * bpp;
            match info.color_type {
                ColorType::Rgb => {
                    if sixteen {
                        out[o] = row[s];
                        out[o + 1] = row[s + 2];
                        out[o + 2] = row[s + 4];
                    } else {
                        out[o] = row[s];
                        out[o + 1] = row[s + 1];
                        out[o + 2] = row[s + 2];
                    }
                }
                ColorType::Rgba => {
                    if sixteen {
                        out[o] = row[s];
                        out[o + 1] = row[s + 2];
                        out[o + 2] = row[s + 4];
                    } else {
                        out[o] = row[s];
                        out[o + 1] = row[s + 1];
                        out[o + 2] = row[s + 2];
                    }
                }
                ColorType::Grayscale => {
                    let g = row[s];
                    out[o] = g;
                    out[o + 1] = g;
                    out[o + 2] = g;
                }
                ColorType::GrayscaleAlpha => {
                    let g = row[s];
                    out[o] = g;
                    out[o + 1] = g;
                    out[o + 2] = g;
                }
                ColorType::Indexed => {
                    let idx = row[s] as usize;
                    let p = palette.get(idx * 3..idx * 3 + 3)?;
                    out[o] = p[0];
                    out[o + 1] = p[1];
                    out[o + 2] = p[2];
                }
            }
        }
    }
    Some((out, w, h))
}

/// Area-average downsample of an RGB image to logical screen size.
/// Each logical pixel averages the source rect it covers (weights =
/// overlap area), so a 1.5x factor stays exact and alias-free.
fn downsample_to_logical(rgb: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut out = vec![0u8; LOGICAL_W * LOGICAL_H * 3];
    let sx = w as f32 / LOGICAL_W as f32;
    let sy = h as f32 / LOGICAL_H as f32;
    for ly in 0..LOGICAL_H {
        let y0 = ly as f32 * sy;
        let y1 = (ly + 1) as f32 * sy;
        let sy0 = (y0.floor() as usize).min(h.saturating_sub(1));
        let sy1 = ((y1.ceil() as usize).min(h)).max(sy0 + 1);
        for lx in 0..LOGICAL_W {
            let x0 = lx as f32 * sx;
            let x1 = (lx + 1) as f32 * sx;
            let sx0 = (x0.floor() as usize).min(w.saturating_sub(1));
            let sx1 = ((x1.ceil() as usize).min(w)).max(sx0 + 1);
            let mut acc = [0f32; 3];
            let mut wsum = 0f32;
            for syi in sy0..sy1 {
                let wy = (y1.min(syi as f32 + 1.0) - y0.max(syi as f32)).max(0.0);
                if wy <= 0.0 {
                    continue;
                }
                for sxi in sx0..sx1 {
                    let wx = (x1.min(sxi as f32 + 1.0) - x0.max(sxi as f32)).max(0.0);
                    if wx <= 0.0 {
                        continue;
                    }
                    let wt = wx * wy;
                    let si = (syi * w + sxi) * 3;
                    acc[0] += rgb[si] as f32 * wt;
                    acc[1] += rgb[si + 1] as f32 * wt;
                    acc[2] += rgb[si + 2] as f32 * wt;
                    wsum += wt;
                }
            }
            let o = (ly * LOGICAL_W + lx) * 3;
            if wsum > 0.0 {
                out[o] = (acc[0] / wsum) as u8;
                out[o + 1] = (acc[1] / wsum) as u8;
                out[o + 2] = (acc[2] / wsum) as u8;
            }
        }
    }
    out
}

/// Separable box blur (sliding window, O(n) per pass): one horizontal
/// pass into a temp buffer, then one vertical pass back into `buf`.
/// Edge pixels are clamped (replicated), so a uniform image is unchanged
/// and a step spreads over exactly 2r+1 px.
fn box_blur_rgb(buf: &mut Vec<u8>, w: usize, h: usize, r: usize) {
    if r == 0 || w == 0 || h == 0 {
        return;
    }
    let win = (2 * r + 1) as u32;
    let mut tmp = vec![0u8; buf.len()];
    // Horizontal pass.
    for y in 0..h {
        let mut sum = [0u32; 3];
        for i in 0..2 * r + 1 {
            let xi = (i as i32 - r as i32).clamp(0, w as i32 - 1) as usize;
            let si = (y * w + xi) * 3;
            sum[0] += buf[si] as u32;
            sum[1] += buf[si + 1] as u32;
            sum[2] += buf[si + 2] as u32;
        }
        for x in 0..w {
            let di = (y * w + x) * 3;
            tmp[di] = (sum[0] / win) as u8;
            tmp[di + 1] = (sum[1] / win) as u8;
            tmp[di + 2] = (sum[2] / win) as u8;
            let rem = (x as i32 - r as i32).clamp(0, w as i32 - 1) as usize;
            let add = (x as i32 + r as i32 + 1).clamp(0, w as i32 - 1) as usize;
            let si_r = (y * w + rem) * 3;
            let si_a = (y * w + add) * 3;
            for c in 0..3 {
                sum[c] = sum[c] - buf[si_r + c] as u32 + buf[si_a + c] as u32;
            }
        }
    }
    // Vertical pass (tmp -> buf).
    for x in 0..w {
        let mut sum = [0u32; 3];
        for i in 0..2 * r + 1 {
            let yi = (i as i32 - r as i32).clamp(0, h as i32 - 1) as usize;
            let si = (yi * w + x) * 3;
            sum[0] += tmp[si] as u32;
            sum[1] += tmp[si + 1] as u32;
            sum[2] += tmp[si + 2] as u32;
        }
        for y in 0..h {
            let di = (y * w + x) * 3;
            buf[di] = (sum[0] / win) as u8;
            buf[di + 1] = (sum[1] / win) as u8;
            buf[di + 2] = (sum[2] / win) as u8;
            let rem = (y as i32 - r as i32).clamp(0, h as i32 - 1) as usize;
            let add = (y as i32 + r as i32 + 1).clamp(0, h as i32 - 1) as usize;
            let si_r = (rem * w + x) * 3;
            let si_a = (add * w + x) * 3;
            for c in 0..3 {
                sum[c] = sum[c] - tmp[si_r + c] as u32 + tmp[si_a + c] as u32;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_blur_uniform_unchanged() {
        let mut buf = vec![200u8; 8 * 6 * 3];
        box_blur_rgb(&mut buf, 8, 6, 3);
        assert!(buf.iter().all(|&v| v == 200));
    }

    #[test]
    fn box_blur_step_spreads_over_2r_plus_1() {
        // 9x1 image: 0s then 255s, step between x=3 and x=4, radius 2.
        let mut buf = vec![0u8; 9 * 3];
        for x in 4..9 {
            buf[x * 3] = 255;
            buf[x * 3 + 1] = 255;
            buf[x * 3 + 2] = 255;
        }
        box_blur_rgb(&mut buf, 9, 1, 2);
        // Window is 5 wide: x=1 sees 0/5 white, x=2 sees 1/5, x=3 2/5,
        // x=4 3/5, x=5 4/5, x=6+ 5/5.
        assert_eq!(buf[1 * 3], 0);
        assert_eq!(buf[2 * 3], 51);
        assert_eq!(buf[3 * 3], 102);
        assert_eq!(buf[4 * 3], 153);
        assert_eq!(buf[5 * 3], 204);
        assert_eq!(buf[6 * 3], 255);
        assert_eq!(buf[8 * 3], 255);
    }

    #[test]
    fn box_blur_radius_12_kills_a_narrow_spike() {
        // A 1-px white line on black, radius 12: peak must drop below
        // 255/25 + margin — the smudge mechanism (a sharp bright edge)
        // cannot survive the blur.
        let w = 64usize;
        let mut buf = vec![0u8; w * 3];
        buf[32 * 3] = 255;
        box_blur_rgb(&mut buf, w, 1, 12);
        let peak = buf.chunks_exact(3).map(|p| p[0]).max().unwrap();
        assert!(peak <= 20, "spike peak {peak} after r=12 blur");
    }

    #[test]
    fn downsample_uniform_stays_uniform() {
        let rgb = vec![77u8; 6 * 4 * 3];
        let out = downsample_to_logical(&rgb, 6, 4);
        assert_eq!(out.len(), LOGICAL_W * LOGICAL_H * 3);
        assert!(out.iter().all(|&v| v == 77));
    }

    #[test]
    fn downsample_two_by_two_averages() {
        // 2x2 source, one white pixel: the first logical pixel covers
        // the whole source only if the source is 2x2 mapped onto a
        // 1504x1002 grid — instead verify the averaging kernel directly
        // on a 3008x2004 source (exactly 2x logical): logical (0,0)
        // averages source (0..2, 0..2).
        let mut rgb = vec![0u8; 3008 * 2004 * 3];
        rgb[0] = 200;
        rgb[1] = 100;
        rgb[2] = 50;
        let out = downsample_to_logical(&rgb, 3008, 2004);
        assert_eq!([out[0], out[1], out[2]], [50, 25, 12]);
    }

    #[test]
    fn sample_clamps_to_screen() {
        let bw = BlurredWallpaper {
            data: vec![9u8; LOGICAL_W * LOGICAL_H * 3],
        };
        assert_eq!(bw.sample(-5.0, -5.0), [9, 9, 9]);
        assert_eq!(bw.sample(99999.0, 99999.0), [9, 9, 9]);
        assert_eq!(bw.sample(0.4, 0.4), [9, 9, 9]);
    }

    #[test]
    fn decode_png_rgb_roundtrip() {
        // Encode a tiny 4x3 RGB PNG, decode it back through our path.
        use std::io::Cursor;
        let mut encoded = Vec::new();
        {
            let mut enc = png::Encoder::new(Cursor::new(&mut encoded), 4, 3);
            enc.set_color(png::ColorType::Rgb);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().unwrap();
            let mut rows = Vec::new();
            for y in 0..3u8 {
                for x in 0..4u8 {
                    rows.push(x * 40);
                    rows.push(y * 60);
                    rows.push(200);
                }
            }
            w.write_image_data(&rows).unwrap();
            w.finish().unwrap();
        }
        let path = std::env::temp_dir().join("iter29-png-roundtrip.png");
        std::fs::write(&path, &encoded).unwrap();
        let (rgb, w, h) = decode_png_rgb(&path).expect("decode our own PNG");
        assert_eq!((w, h), (4, 3));
        assert_eq!(&rgb[0..3], &[0, 0, 200]);
        assert_eq!(&rgb[9..12], &[120, 0, 200]);
        let _ = std::fs::remove_file(&path);
    }
}
