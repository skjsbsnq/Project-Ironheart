//! Parser for HOI4's `map/terrain.bmp` — an 8-bit palette-indexed BMP where
//! each pixel value is a terrain *index* (0..=31 in vanilla), not an RGB.
//!
//! The `image` crate decodes paletted BMPs into RGB, throwing away the index
//! we actually need. We parse the BMP header by hand to recover the raw index
//! data and the embedded palette (in case downstream code wants the original
//! palette colours).

use std::path::Path;

/// Per-pixel terrain index map.
#[derive(Debug, Clone)]
pub struct TerrainBitmap {
    pub width: u32,
    pub height: u32,
    /// Flat row-major (top-down). Each value is a terrain palette index 0..=255.
    pub pixels: Vec<u8>,
    /// 256-entry RGB palette read from the BMP header (entry i = palette[i]).
    /// HOI4 uses indices 0..=31 only; remaining entries are typically zero.
    pub palette: [[u8; 3]; 256],
}

impl TerrainBitmap {
    #[inline]
    pub fn get(&self, x: u32, y: u32) -> u8 {
        let x = x.min(self.width.saturating_sub(1));
        let y = y.min(self.height.saturating_sub(1));
        self.pixels[(y * self.width + x) as usize]
    }
}

/// Load `terrain.bmp` from disk.
pub fn load_terrain_bmp(path: &Path) -> Result<TerrainBitmap, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    parse_indexed_bmp(&bytes)
}

/// Parse an 8-bit indexed BMP from a byte slice. Supports the common BITMAPINFOHEADER
/// (40-byte) variant and `BI_RGB` compression. Bottom-up rows are flipped to
/// produce a top-down `pixels` array.
pub fn parse_indexed_bmp(bytes: &[u8]) -> Result<TerrainBitmap, String> {
    if bytes.len() < 54 {
        return Err(format!("BMP too small: {} bytes", bytes.len()));
    }
    if &bytes[0..2] != b"BM" {
        return Err("Not a BMP file (missing 'BM' magic)".into());
    }
    let pixel_offset = u32::from_le_bytes(bytes[10..14].try_into().unwrap()) as usize;
    let info_size = u32::from_le_bytes(bytes[14..18].try_into().unwrap()) as usize;
    let width = i32::from_le_bytes(bytes[18..22].try_into().unwrap());
    let height_signed = i32::from_le_bytes(bytes[22..26].try_into().unwrap());
    let bpp = u16::from_le_bytes(bytes[28..30].try_into().unwrap());
    let compression = u32::from_le_bytes(bytes[30..34].try_into().unwrap());

    if width <= 0 {
        return Err(format!("Invalid BMP width: {width}"));
    }
    if bpp != 8 {
        return Err(format!("Expected 8-bit indexed BMP, got {bpp}bpp"));
    }
    if compression != 0 {
        return Err(format!(
            "Only uncompressed (BI_RGB) BMP supported (compression={compression})"
        ));
    }

    let bottom_up = height_signed > 0;
    let height = height_signed.unsigned_abs();
    let width = width as u32;

    // Palette starts immediately after the info header.
    // Entries are 4 bytes each: B, G, R, _.
    let palette_offset = 14 + info_size;
    let palette_entries = (pixel_offset.saturating_sub(palette_offset)) / 4;
    let mut palette = [[0u8; 3]; 256];
    for i in 0..palette_entries.min(256) {
        let off = palette_offset + i * 4;
        if off + 3 > bytes.len() {
            break;
        }
        // BMP palette is stored as BGR(A).
        palette[i] = [bytes[off + 2], bytes[off + 1], bytes[off]];
    }

    // Each row is padded to a 4-byte boundary.
    let row_size = ((width + 3) / 4) * 4;
    let needed = pixel_offset + (row_size as usize) * (height as usize);
    if bytes.len() < needed {
        return Err(format!(
            "BMP truncated: need {needed} bytes for pixels, have {}",
            bytes.len()
        ));
    }

    let mut pixels = vec![0u8; (width * height) as usize];
    for row in 0..height {
        let bmp_row = if bottom_up { height - 1 - row } else { row };
        let src = pixel_offset + (bmp_row as usize) * (row_size as usize);
        let dst = (row * width) as usize;
        pixels[dst..dst + width as usize].copy_from_slice(&bytes[src..src + width as usize]);
    }

    Ok(TerrainBitmap {
        width,
        height,
        pixels,
        palette,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a tiny 4×2 indexed BMP (bottom-up) in memory and parse it.
    fn synth_bmp(bottom_up: bool) -> Vec<u8> {
        let width = 4u32;
        let height = 2u32;
        let row_size = 4u32; // already 4-aligned
        let palette_size = 4 * 256;
        let pixel_data_size = row_size * height;
        let pixel_offset = 14 + 40 + palette_size; // FileHdr + DIB + Palette
        let file_size = pixel_offset + pixel_data_size;

        let mut buf = Vec::new();
        // BITMAPFILEHEADER (14 bytes)
        buf.extend_from_slice(b"BM");
        buf.extend_from_slice(&file_size.to_le_bytes());
        buf.extend_from_slice(&[0u8; 4]); // reserved
        buf.extend_from_slice(&pixel_offset.to_le_bytes());
        // BITMAPINFOHEADER (40 bytes)
        buf.extend_from_slice(&40u32.to_le_bytes()); // size
        buf.extend_from_slice(&(width as i32).to_le_bytes());
        buf.extend_from_slice(
            &(if bottom_up {
                height as i32
            } else {
                -(height as i32)
            })
            .to_le_bytes(),
        );
        buf.extend_from_slice(&1u16.to_le_bytes()); // planes
        buf.extend_from_slice(&8u16.to_le_bytes()); // bpp
        buf.extend_from_slice(&0u32.to_le_bytes()); // compression
        buf.extend_from_slice(&pixel_data_size.to_le_bytes());
        buf.extend_from_slice(&[0u8; 4 * 4]); // x/y ppm + colours used + important
                                              // Palette: 256 BGRA entries. Set entry 0 = (10,20,30), entry 1 = (40,50,60).
        for i in 0..256u32 {
            if i == 0 {
                buf.extend_from_slice(&[30, 20, 10, 0]); // BGR(A) = R=10,G=20,B=30
            } else if i == 1 {
                buf.extend_from_slice(&[60, 50, 40, 0]);
            } else {
                buf.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
        // Pixel data: 2 rows of 4 pixels.
        // Top row should be [0, 1, 0, 1]; bottom row [1, 1, 0, 0]
        // For bottom-up storage we write bottom row first.
        if bottom_up {
            buf.extend_from_slice(&[1, 1, 0, 0]); // bottom row (in image: bottom)
            buf.extend_from_slice(&[0, 1, 0, 1]); // top row
        } else {
            buf.extend_from_slice(&[0, 1, 0, 1]); // top row
            buf.extend_from_slice(&[1, 1, 0, 0]); // bottom row
        }
        buf
    }

    #[test]
    fn parse_bottom_up_bmp() {
        let bmp = synth_bmp(true);
        let t = parse_indexed_bmp(&bmp).unwrap();
        assert_eq!(t.width, 4);
        assert_eq!(t.height, 2);
        // Top-down output: top row (y=0) first
        assert_eq!(&t.pixels[0..4], &[0, 1, 0, 1]);
        assert_eq!(&t.pixels[4..8], &[1, 1, 0, 0]);
        // Palette decoded
        assert_eq!(t.palette[0], [10, 20, 30]);
        assert_eq!(t.palette[1], [40, 50, 60]);
        assert_eq!(t.palette[2], [0, 0, 0]);
    }

    #[test]
    fn parse_top_down_bmp() {
        let bmp = synth_bmp(false);
        let t = parse_indexed_bmp(&bmp).unwrap();
        assert_eq!(&t.pixels[0..4], &[0, 1, 0, 1]);
        assert_eq!(&t.pixels[4..8], &[1, 1, 0, 0]);
    }

    #[test]
    fn rejects_non_bmp() {
        let mut bad = vec![0u8; 60];
        bad[0] = b'X';
        bad[1] = b'X';
        let err = parse_indexed_bmp(&bad).unwrap_err();
        assert!(err.contains("Not a BMP"), "got: {err}");
    }

    #[test]
    fn rejects_24bit_bmp() {
        let mut bmp = synth_bmp(true);
        // patch bpp from 8 to 24
        bmp[28] = 24;
        let err = parse_indexed_bmp(&bmp).unwrap_err();
        assert!(err.contains("8-bit"), "got: {err}");
    }
}
