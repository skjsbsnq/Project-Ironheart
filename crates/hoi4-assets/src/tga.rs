//! Phase 4.2 (redesign): Targa (`.tga`) 解析器，主要给 `gfx/flags/*.tga` 用。
//!
//! HOI4 国旗格式：
//! - id_length=0, color_map=0
//! - image_type=2 (uncompressed truecolor)
//! - 24 / 32 bpp，**BGRA / BGR** 色序
//! - descriptor bit 5 (0x20) 决定行序：0=bottom-up（默认），1=top-down
//!
//! 我们不支持 RLE / 调色板（vanilla 旗子全是 type=2，DLC mod 也极少用别的）。

use std::path::Path;

use crate::db::AssetDb;
use crate::error::AssetError;

/// 已解码的 TGA。RGBA8，行序 top-down（与 DDS / wgpu 期望一致）。
#[derive(Debug, Clone)]
pub struct TgaImage {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` 字节，RGBA8。
    pub pixels: Vec<u8>,
}

impl TgaImage {
    /// 解析 TGA 字节流。
    pub fn parse(bytes: &[u8]) -> Result<Self, AssetError> {
        if bytes.len() < 18 {
            return Err(AssetError::parse("tga", "header < 18 bytes"));
        }
        let id_length = bytes[0] as usize;
        let color_map_type = bytes[1];
        let image_type = bytes[2];
        let width = u16::from_le_bytes([bytes[12], bytes[13]]) as u32;
        let height = u16::from_le_bytes([bytes[14], bytes[15]]) as u32;
        let bpp = bytes[16];
        let descriptor = bytes[17];
        let top_down = (descriptor & 0x20) != 0;

        if color_map_type != 0 {
            return Err(AssetError::parse("tga", "color-mapped TGA not supported"));
        }
        if image_type != 2 {
            return Err(AssetError::parse(
                "tga",
                format!(
                    "only image_type=2 (uncompressed truecolor) supported, got {}",
                    image_type
                ),
            ));
        }
        if !(bpp == 24 || bpp == 32) {
            return Err(AssetError::parse("tga", format!("unsupported bpp={}", bpp)));
        }
        let pixel_offset = 18 + id_length;
        let bytes_per_pixel = (bpp / 8) as usize;
        let needed = pixel_offset + (width as usize) * (height as usize) * bytes_per_pixel;
        if bytes.len() < needed {
            return Err(AssetError::parse(
                "tga",
                format!("data truncated: need {} bytes, got {}", needed, bytes.len()),
            ));
        }

        let src = &bytes[pixel_offset..needed];
        let mut pixels = vec![0u8; (width as usize) * (height as usize) * 4];

        for y in 0..height as usize {
            // TGA 默认 bottom-up：逻辑第 y 行 = 物理第 (h-1-y) 行
            let src_y = if top_down {
                y
            } else {
                (height as usize) - 1 - y
            };
            for x in 0..width as usize {
                let si = (src_y * width as usize + x) * bytes_per_pixel;
                let di = (y * width as usize + x) * 4;
                if bpp == 32 {
                    // BGRA → RGBA
                    pixels[di] = src[si + 2];
                    pixels[di + 1] = src[si + 1];
                    pixels[di + 2] = src[si];
                    pixels[di + 3] = src[si + 3];
                } else {
                    // BGR → RGBA, alpha=255
                    pixels[di] = src[si + 2];
                    pixels[di + 1] = src[si + 1];
                    pixels[di + 2] = src[si];
                    pixels[di + 3] = 255;
                }
            }
        }

        Ok(Self {
            width,
            height,
            pixels,
        })
    }
}

/// 通过 [`AssetDb`] 加载并解析一个 TGA。
pub fn load_tga(db: &impl AssetDb, relative: impl AsRef<Path>) -> Result<TgaImage, AssetError> {
    let bytes = db.open(relative)?;
    TgaImage::parse(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个最小 TGA：2×2，32bpp，bottom-up（默认），4 个像素。
    /// 物理第 0 行 = 逻辑第 1 行。
    fn make_tga_2x2_bottom_up() -> Vec<u8> {
        let mut v = vec![
            0, // id_length
            0, // color_map_type
            2, // image_type=2
            0, 0, 0, 0, 0, // color map spec (5 bytes, ignored)
            0, 0, // x_origin
            0, 0, // y_origin
            2, 0, // width=2
            2, 0,    // height=2
            32,   // bpp
            0x00, // descriptor: bottom-up
        ];
        // 物理第 0 行（= 逻辑第 1 行）：bgra red, green
        v.extend_from_slice(&[0, 0, 255, 255]); // (0,1) red
        v.extend_from_slice(&[0, 255, 0, 255]); // (1,1) green
                                                // 物理第 1 行（= 逻辑第 0 行）：bgra blue, white
        v.extend_from_slice(&[255, 0, 0, 255]); // (0,0) blue
        v.extend_from_slice(&[255, 255, 255, 255]); // (1,0) white
        v
    }

    #[test]
    fn parse_minimal_32bpp_bottom_up() {
        let bytes = make_tga_2x2_bottom_up();
        let img = TgaImage::parse(&bytes).unwrap();
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 2);
        // 逻辑 (0,0) 对应物理底部（已翻转）= blue → RGBA(0,0,255,255)
        assert_eq!(img.pixels[0..4], [0, 0, 255, 255]);
        // 逻辑 (1,0) = white
        assert_eq!(img.pixels[4..8], [255, 255, 255, 255]);
        // 逻辑 (0,1) = red
        assert_eq!(img.pixels[8..12], [255, 0, 0, 255]);
        // 逻辑 (1,1) = green
        assert_eq!(img.pixels[12..16], [0, 255, 0, 255]);
    }

    #[test]
    fn rejects_non_truecolor() {
        let mut bytes = make_tga_2x2_bottom_up();
        bytes[2] = 1; // colormapped
        assert!(TgaImage::parse(&bytes).is_err());
        bytes[2] = 10; // RLE
        assert!(TgaImage::parse(&bytes).is_err());
    }

    #[test]
    fn rejects_bad_bpp() {
        let mut bytes = make_tga_2x2_bottom_up();
        bytes[16] = 16;
        assert!(TgaImage::parse(&bytes).is_err());
    }

    #[test]
    fn rejects_truncated() {
        let bytes = make_tga_2x2_bottom_up();
        let truncated = &bytes[..bytes.len() - 4];
        assert!(TgaImage::parse(truncated).is_err());
    }
}
