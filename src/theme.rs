//! Shared, contrast-checked colours for the island.
use std::io::Write;
use std::path::{Path, PathBuf};

pub type Rgb = (u8, u8, u8);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub background: Rgb,
    pub surface: Rgb,
    pub foreground: Rgb,
    pub accent: Rgb,
    pub selection_text: Rgb,
}

fn luminance(rgb: Rgb) -> f64 {
    let linear = |v: u8| {
        let s = f64::from(v) / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(rgb.0) + 0.7152 * linear(rgb.1) + 0.0722 * linear(rgb.2)
}

fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

fn blend(a: Rgb, b: Rgb, amount: u8) -> Rgb {
    let ch = |a: u8, b: u8| {
        ((u16::from(a) * (255 - u16::from(amount)) + u16::from(b) * u16::from(amount) + 127) / 255)
            as u8
    };
    (ch(a.0, b.0), ch(a.1, b.1), ch(a.2, b.2))
}

pub fn ink_on(background: Rgb, preferred: Rgb) -> Rgb {
    if contrast(preferred, background) >= 4.5 {
        preferred
    } else if contrast((0, 0, 0), background) >= contrast((255, 255, 255), background) {
        (0, 0, 0)
    } else {
        (255, 255, 255)
    }
}

fn readable(mut ink: Rgb, background: Rgb) -> Rgb {
    // Small steps retain the source hue even in palettes sampled from dark photos.
    while contrast(ink, background) < 4.5 && ink != (255, 255, 255) {
        ink = blend(ink, (255, 255, 255), 24);
    }
    ink
}

impl Palette {
    pub fn from_swatches(swatches: &[Rgb; 6]) -> Self {
        let mut background = swatches[0];
        while luminance(background) > 0.06 {
            background = blend(background, (0, 0, 0), 24);
        }
        let foreground = readable(
            *swatches
                .iter()
                .max_by(|a, b| luminance(**a).total_cmp(&luminance(**b)))
                .unwrap_or(&swatches[5]),
            background,
        );
        let surface = blend(background, foreground, 14);
        let foreground = readable(foreground, surface);
        let accent = readable(swatches[2], surface);
        let selection_text = ink_on(accent, background);
        Self {
            background,
            surface,
            foreground,
            accent,
            selection_text,
        }
    }

}

pub fn config_home() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        })
}

pub fn atomic_write(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("no parent directory"))?;
    std::fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(
        ".dynamite-{}-{}.tmp",
        std::process::id(),
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    let permissions = match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() => Some(meta.permissions()),
        Ok(_) => return Err(std::io::Error::other("expected a regular file")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)?;
    let result = (|| {
        if let Some(permissions) = permissions {
            f.set_permissions(permissions)?;
        }
        f.write_all(data)?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn even_dark_photo_palettes_have_readable_text_links_and_selection() {
        for swatches in [
            [(0, 0, 0); 6],
            [(255, 255, 255); 6],
            [
                (2, 10, 18),
                (4, 22, 40),
                (13, 70, 128),
                (6, 5, 19),
                (32, 27, 102),
                (116, 115, 112),
            ],
        ] {
            let p = Palette::from_swatches(&swatches);
            assert!(contrast(p.foreground, p.surface) >= 4.5);
            assert!(contrast(p.accent, p.surface) >= 4.5);
            assert!(contrast(p.selection_text, p.accent) >= 4.5);
            assert!(contrast(ink_on(p.accent, (11, 27, 58)), p.accent) >= 4.5);
        }
    }


}
