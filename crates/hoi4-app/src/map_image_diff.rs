use std::fmt::Write as _;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageDiffMetrics {
    pub width: u32,
    pub height: u32,
    pub pixels: u64,
    pub ssim_luma: f64,
    pub average_color_delta: f64,
    pub luma_delta: f64,
    pub edge_delta: f64,
}

impl ImageDiffMetrics {
    pub fn to_json(self, project: &Path, reference: &Path) -> String {
        let mut out = String::new();
        out.push_str("{\n");
        let _ = writeln!(out, "  \"phase\": \"12\",");
        let _ = writeln!(
            out,
            "  \"project\": \"{}\",",
            json_escape(&project.display().to_string())
        );
        let _ = writeln!(
            out,
            "  \"reference\": \"{}\",",
            json_escape(&reference.display().to_string())
        );
        let _ = writeln!(out, "  \"width\": {},", self.width);
        let _ = writeln!(out, "  \"height\": {},", self.height);
        let _ = writeln!(out, "  \"pixels\": {},", self.pixels);
        let _ = writeln!(out, "  \"ssim_luma\": {:.8},", self.ssim_luma);
        let _ = writeln!(
            out,
            "  \"average_color_delta\": {:.8},",
            self.average_color_delta
        );
        let _ = writeln!(out, "  \"luma_delta\": {:.8},", self.luma_delta);
        let _ = writeln!(out, "  \"edge_delta\": {:.8}", self.edge_delta);
        out.push_str("}\n");
        out
    }

    pub fn summary(self) -> String {
        format!(
            "ssim_luma={:.5} avg_color_delta={:.3} luma_delta={:.3} edge_delta={:.3} size={}x{}",
            self.ssim_luma,
            self.average_color_delta,
            self.luma_delta,
            self.edge_delta,
            self.width,
            self.height
        )
    }
}

pub fn diff_png_files(project: &Path, reference: &Path) -> Result<ImageDiffMetrics, String> {
    let project_image = load_png_rgba8(project)?;
    let reference_image = load_png_rgba8(reference)?;
    if project_image.width != reference_image.width
        || project_image.height != reference_image.height
    {
        return Err(format!(
            "image dimensions differ: {}x{} vs {}x{}",
            project_image.width,
            project_image.height,
            reference_image.width,
            reference_image.height
        ));
    }
    Ok(diff_rgba8(
        project_image.width,
        project_image.height,
        &project_image.rgba,
        &reference_image.rgba,
    ))
}

pub fn write_diff_report(
    project: &Path,
    reference: &Path,
    output_path: &Path,
) -> Result<ImageDiffMetrics, String> {
    let metrics = diff_png_files(project, reference)?;
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    std::fs::write(output_path, metrics.to_json(project, reference)).map_err(|err| {
        format!(
            "failed to write image diff report {}: {}",
            output_path.display(),
            err
        )
    })?;
    Ok(metrics)
}

fn diff_rgba8(width: u32, height: u32, a: &[u8], b: &[u8]) -> ImageDiffMetrics {
    debug_assert_eq!(a.len(), b.len());
    let pixels = width as usize * height as usize;
    let mut luma_a = Vec::with_capacity(pixels);
    let mut luma_b = Vec::with_capacity(pixels);
    let mut color_sum = 0.0;
    let mut luma_delta_sum = 0.0;

    for idx in 0..pixels {
        let p = idx * 4;
        let ar = a[p] as f64;
        let ag = a[p + 1] as f64;
        let ab = a[p + 2] as f64;
        let br = b[p] as f64;
        let bg = b[p + 1] as f64;
        let bb = b[p + 2] as f64;
        let la = luma(ar, ag, ab);
        let lb = luma(br, bg, bb);
        luma_a.push(la);
        luma_b.push(lb);
        let dr = ar - br;
        let dg = ag - bg;
        let db = ab - bb;
        color_sum += (dr * dr + dg * dg + db * db).sqrt();
        luma_delta_sum += (la - lb).abs();
    }

    ImageDiffMetrics {
        width,
        height,
        pixels: pixels as u64,
        ssim_luma: ssim(&luma_a, &luma_b),
        average_color_delta: color_sum / pixels.max(1) as f64,
        luma_delta: luma_delta_sum / pixels.max(1) as f64,
        edge_delta: edge_delta(width as usize, height as usize, &luma_a, &luma_b),
    }
}

fn luma(r: f64, g: f64, b: f64) -> f64 {
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

fn ssim(a: &[f64], b: &[f64]) -> f64 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let n = a.len() as f64;
    let mean_a = a.iter().sum::<f64>() / n;
    let mean_b = b.iter().sum::<f64>() / n;
    let mut var_a = 0.0;
    let mut var_b = 0.0;
    let mut cov = 0.0;
    for (&av, &bv) in a.iter().zip(b.iter()) {
        let da = av - mean_a;
        let db = bv - mean_b;
        var_a += da * da;
        var_b += db * db;
        cov += da * db;
    }
    let denom = (n - 1.0).max(1.0);
    var_a /= denom;
    var_b /= denom;
    cov /= denom;
    let c1 = (0.01_f64 * 255.0).powi(2);
    let c2 = (0.03_f64 * 255.0).powi(2);
    ((2.0 * mean_a * mean_b + c1) * (2.0 * cov + c2))
        / ((mean_a.powi(2) + mean_b.powi(2) + c1) * (var_a + var_b + c2))
}

fn edge_delta(width: usize, height: usize, a: &[f64], b: &[f64]) -> f64 {
    if width < 3 || height < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    let mut count = 0usize;
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let idx = y * width + x;
            let edge_a = (a[idx + 1] - a[idx - 1]).abs() + (a[idx + width] - a[idx - width]).abs();
            let edge_b = (b[idx + 1] - b[idx - 1]).abs() + (b[idx + width] - b[idx - width]).abs();
            sum += (edge_a - edge_b).abs();
            count += 1;
        }
    }
    sum / count.max(1) as f64
}

struct RgbaImage {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

fn load_png_rgba8(path: &Path) -> Result<RgbaImage, String> {
    let file = File::open(path).map_err(|err| format!("{}: {}", path.display(), err))?;
    let decoder = png::Decoder::new(BufReader::new(file));
    let mut reader = decoder
        .read_info()
        .map_err(|err| format!("{}: {}", path.display(), err))?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|err| format!("{}: {}", path.display(), err))?;
    let bytes = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => bytes.to_vec(),
        png::ColorType::Rgb => {
            let mut out = Vec::with_capacity(info.width as usize * info.height as usize * 4);
            for rgb in bytes.chunks_exact(3) {
                out.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
            out
        }
        png::ColorType::Grayscale => {
            let mut out = Vec::with_capacity(info.width as usize * info.height as usize * 4);
            for &v in bytes {
                out.extend_from_slice(&[v, v, v, 255]);
            }
            out
        }
        png::ColorType::GrayscaleAlpha => {
            let mut out = Vec::with_capacity(info.width as usize * info.height as usize * 4);
            for ga in bytes.chunks_exact(2) {
                out.extend_from_slice(&[ga[0], ga[0], ga[0], ga[1]]);
            }
            out
        }
        png::ColorType::Indexed => {
            return Err(format!(
                "{}: indexed PNG diff input is not supported",
                path.display()
            ));
        }
    };
    Ok(RgbaImage {
        width: info.width,
        height: info.height,
        rgba,
    })
}

fn json_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => {
                let _ = write!(out, "\\u{:04x}", ch as u32);
            }
            ch => out.push(ch),
        }
    }
    out
}

pub fn default_output_path() -> PathBuf {
    PathBuf::from("target/map_parity_diff/report.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufWriter;

    #[test]
    fn identical_images_have_perfect_diff_metrics() {
        let rgba = vec![
            10, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 255, 100, 110, 120, 255,
        ];
        let metrics = diff_rgba8(2, 2, &rgba, &rgba);
        assert!((metrics.ssim_luma - 1.0).abs() < 1e-9);
        assert_eq!(metrics.average_color_delta, 0.0);
        assert_eq!(metrics.luma_delta, 0.0);
        assert_eq!(metrics.edge_delta, 0.0);
    }

    #[test]
    fn changed_images_report_nonzero_delta() {
        let mut a = Vec::new();
        let mut b = Vec::new();
        for _ in 0..16 {
            a.extend_from_slice(&[0, 0, 0, 255]);
            b.extend_from_slice(&[20, 40, 80, 255]);
        }
        let metrics = diff_rgba8(4, 4, &a, &b);
        assert!(metrics.average_color_delta > 0.0);
        assert!(metrics.luma_delta > 0.0);
        assert!(metrics.ssim_luma < 1.0);
    }

    #[test]
    fn png_loader_round_trips_rgba() {
        let dir = std::env::temp_dir().join("hoi4_map_image_diff_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.png");
        write_test_png(&path, 2, 1, &[1, 2, 3, 255, 4, 5, 6, 255]);
        let image = load_png_rgba8(&path).unwrap();
        assert_eq!(image.width, 2);
        assert_eq!(image.height, 1);
        assert_eq!(image.rgba, vec![1, 2, 3, 255, 4, 5, 6, 255]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn write_test_png(path: &Path, width: u32, height: u32, rgba: &[u8]) {
        let file = File::create(path).unwrap();
        let writer = BufWriter::new(file);
        let mut encoder = png::Encoder::new(writer, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(rgba).unwrap();
    }
}
