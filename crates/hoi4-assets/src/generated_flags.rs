use crate::TgaImage;
use std::path::{Path, PathBuf};

pub fn generated_historical_flag(tag: &str) -> Option<TgaImage> {
    if let Some(img) = local_png_flag(tag) {
        return Some(img);
    }

    // SPR/SPA are intentionally not procedurally generated: the project
    // carries hand-authored PNGs in b1/国旗, and those must be used.
    None
}

pub fn local_png_flag(tag: &str) -> Option<TgaImage> {
    let upper = tag.to_ascii_uppercase();
    let lower = tag.to_ascii_lowercase();
    let file_names = [format!("{upper}.png"), format!("{lower}.png")];

    for root in flag_roots() {
        for file_name in &file_names {
            let path = root.join(file_name);
            if let Some(img) = load_png_rgba(&path) {
                return Some(img);
            }
        }
    }
    None
}

fn flag_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Ok(cwd) = std::env::current_dir() {
        push_ancestor_flag_roots(&mut roots, &cwd);
    }

    if let Some(manifest_dir) = option_env!("CARGO_MANIFEST_DIR") {
        push_ancestor_flag_roots(&mut roots, Path::new(manifest_dir));
    }

    roots
}

fn push_ancestor_flag_roots(roots: &mut Vec<PathBuf>, start: &Path) {
    for dir in start.ancestors() {
        let candidate = dir.join("国旗");
        if candidate.is_dir() && !roots.iter().any(|p| p == &candidate) {
            roots.push(candidate);
        }
    }
}

fn load_png_rgba(path: &Path) -> Option<TgaImage> {
    let file = std::fs::File::open(path).ok()?;
    let mut decoder = png::Decoder::new(file);
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let bytes = &buf[..info.buffer_size()];

    let pixels = match info.color_type {
        png::ColorType::Rgba => bytes.to_vec(),
        png::ColorType::Rgb => {
            let mut out = Vec::with_capacity((info.width * info.height * 4) as usize);
            for px in bytes.chunks_exact(3) {
                out.extend_from_slice(&[px[0], px[1], px[2], 255]);
            }
            out
        }
        png::ColorType::Grayscale => {
            let mut out = Vec::with_capacity((info.width * info.height * 4) as usize);
            for &v in bytes {
                out.extend_from_slice(&[v, v, v, 255]);
            }
            out
        }
        png::ColorType::GrayscaleAlpha => {
            let mut out = Vec::with_capacity((info.width * info.height * 4) as usize);
            for px in bytes.chunks_exact(2) {
                out.extend_from_slice(&[px[0], px[0], px[0], px[1]]);
            }
            out
        }
        png::ColorType::Indexed => return None,
    };

    Some(TgaImage {
        width: info.width,
        height: info.height,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spanish_flags_prefer_local_pngs() {
        let spr = generated_historical_flag("spr").expect("SPR local PNG should load");
        let spa = generated_historical_flag("spa").expect("SPA local PNG should load");
        assert!(spr.width > 0 && spr.height > 0);
        assert!(spa.width > 0 && spa.height > 0);
        assert_eq!(spr.pixels.len(), (spr.width * spr.height * 4) as usize);
        assert_eq!(spa.pixels.len(), (spa.width * spa.height * 4) as usize);
    }
}
