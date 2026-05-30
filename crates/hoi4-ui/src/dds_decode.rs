//! 共享 DDS → RGBA8 解码器 — V5 阶段 B.5。
//!
//! 给 [`crate::nine_slice::NineSlice`] 与 [`crate::icons::IconBank`] 共用一份
//! 解码逻辑，避免两边各写一遍。
//!
//! ## 支持格式
//!
//! - **BGRA8**：vanilla 主流 `gfx/interface/{tiles,goals,ideas}/*.dds` 几乎都是
//!   这种（实测 `tiled_window.dds` / `focus_GER_*.dds` 均 32 bpp BGRA）。
//!   通道交换 BGRA → RGBA。
//! - **BGR555**：部分老领袖肖像使用 16 bpp X1R5G5B5。
//! - **BC1 (DXT1)**：4 bpp，1-bit alpha 或纯不透明。8 字节 / 4×4 块。
//! - **BC3 (DXT5)**：8 bpp，插值 alpha。16 字节 / 4×4 块。
//!
//! 不支持：BC5（法线贴图，不在 UI 路径上）/ BC4 / BC7 / DX10 扩展头里的
//! 现代格式（vanilla HOI4 用不到）。
//!
//! ## 实现来源
//!
//! 纯 Rust 实现，无外部 dep。BC1 / BC3 算法参考 Microsoft DirectXTex 与
//! S3TC 公开规范；与 `image-rs` / `texpresso` 等同。

use hoi4_assets::dds::{DdsFormat, DdsImage};

/// 解码失败原因。
#[derive(Debug)]
pub enum DdsDecodeError {
    /// 不支持的格式（B.5 起步只解 BGRA8 / BC1 / BC3）。
    Unsupported(DdsFormat),
    /// mip 0 数据缺失（DDS 解析时被截断）。
    NoMipZero,
    /// mip 0 大小与 width × height × bpp 不匹配（损坏文件）。
    SizeMismatch { expected: usize, actual: usize },
}

impl std::fmt::Display for DdsDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported(fmt) => {
                write!(
                    f,
                    "DDS format not supported by hoi4-ui::dds_decode: {fmt:?}"
                )
            }
            Self::NoMipZero => write!(f, "DDS image has no mip 0"),
            Self::SizeMismatch { expected, actual } => {
                write!(
                    f,
                    "DDS mip 0 size mismatch: expected {expected} bytes, got {actual}"
                )
            }
        }
    }
}

impl std::error::Error for DdsDecodeError {}

/// 把 `DdsImage` 的 mip 0 解码为 RGBA8 字节流（每像素 4 字节，r/g/b/a 顺序）。
///
/// 输出长度 = `width * height * 4`。
pub fn decode_mip0_to_rgba(dds: &DdsImage) -> Result<Vec<u8>, DdsDecodeError> {
    // 先校验格式：让「不支持」错误优先于「无 mip0」错误（caller 凭 Unsupported
    // 决定 fallback；NoMipZero 是数据损坏，处理路径不同）。
    match dds.format {
        DdsFormat::Bgra8 | DdsFormat::Bgr555 | DdsFormat::Bc1 | DdsFormat::Bc3 => {}
        other => return Err(DdsDecodeError::Unsupported(other)),
    }
    let mip0 = dds.mip_data(0).ok_or(DdsDecodeError::NoMipZero)?;
    match dds.format {
        DdsFormat::Bgra8 => decode_bgra8(mip0, dds.width, dds.height),
        DdsFormat::Bgr555 => decode_bgr555(mip0, dds.width, dds.height),
        DdsFormat::Bc1 => decode_bc1(mip0, dds.width, dds.height),
        DdsFormat::Bc3 => decode_bc3(mip0, dds.width, dds.height),
        other => Err(DdsDecodeError::Unsupported(other)),
    }
}

// ─── BGRA8 ────────────────────────────────────────────────────────────────

fn decode_bgra8(src: &[u8], w: u32, h: u32) -> Result<Vec<u8>, DdsDecodeError> {
    let expected = (w as usize) * (h as usize) * 4;
    if src.len() < expected {
        return Err(DdsDecodeError::SizeMismatch {
            expected,
            actual: src.len(),
        });
    }
    let mut out = Vec::with_capacity(expected);
    for chunk in src[..expected].chunks_exact(4) {
        let b = chunk[0];
        let g = chunk[1];
        let r = chunk[2];
        let a = chunk[3];
        out.extend_from_slice(&[r, g, b, a]);
    }
    Ok(out)
}

fn decode_bgr555(src: &[u8], w: u32, h: u32) -> Result<Vec<u8>, DdsDecodeError> {
    let expected = (w as usize) * (h as usize) * 2;
    if src.len() < expected {
        return Err(DdsDecodeError::SizeMismatch {
            expected,
            actual: src.len(),
        });
    }
    let mut out = Vec::with_capacity((w as usize) * (h as usize) * 4);
    for chunk in src[..expected].chunks_exact(2) {
        let raw = u16::from_le_bytes([chunk[0], chunk[1]]);
        let b = expand_5_to_8((raw & 0x001f) as u8);
        let g = expand_5_to_8(((raw >> 5) & 0x001f) as u8);
        let r = expand_5_to_8(((raw >> 10) & 0x001f) as u8);
        out.extend_from_slice(&[r, g, b, 255]);
    }
    Ok(out)
}

fn expand_5_to_8(v: u8) -> u8 {
    (v << 3) | (v >> 2)
}

// ─── BC1 (DXT1) ───────────────────────────────────────────────────────────

/// 解码 BC1 / DXT1。每 4×4 块 8 字节：c0 (u16) c1 (u16) indices (u32)。
fn decode_bc1(src: &[u8], w: u32, h: u32) -> Result<Vec<u8>, DdsDecodeError> {
    let bw = w.div_ceil(4) as usize;
    let bh = h.div_ceil(4) as usize;
    let expected_blocks = bw * bh;
    let expected_bytes = expected_blocks * 8;
    if src.len() < expected_bytes {
        return Err(DdsDecodeError::SizeMismatch {
            expected: expected_bytes,
            actual: src.len(),
        });
    }
    let mut out = vec![0u8; (w as usize) * (h as usize) * 4];

    for by in 0..bh {
        for bx in 0..bw {
            let off = (by * bw + bx) * 8;
            let block = &src[off..off + 8];
            let c0 = u16::from_le_bytes([block[0], block[1]]);
            let c1 = u16::from_le_bytes([block[2], block[3]]);
            let indices = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
            let palette = bc1_palette(c0, c1);
            write_block_rgba(&palette, indices, &mut out, w, h, bx, by);
        }
    }

    Ok(out)
}

/// BC1 的 4 色调色板：endpoints + 1/3、2/3 插值（不透明分支）或半混合 + 透明
/// （透明分支，c0 ≤ c1 时启用）。
fn bc1_palette(c0: u16, c1: u16) -> [[u8; 4]; 4] {
    let (r0, g0, b0) = unpack_565(c0);
    let (r1, g1, b1) = unpack_565(c1);

    let mut p = [[0u8; 4]; 4];
    p[0] = [r0, g0, b0, 255];
    p[1] = [r1, g1, b1, 255];
    if c0 > c1 {
        // 不透明分支：1/3、2/3 插值。
        p[2] = [
            ((2u16 * r0 as u16 + r1 as u16) / 3) as u8,
            ((2u16 * g0 as u16 + g1 as u16) / 3) as u8,
            ((2u16 * b0 as u16 + b1 as u16) / 3) as u8,
            255,
        ];
        p[3] = [
            ((r0 as u16 + 2u16 * r1 as u16) / 3) as u8,
            ((g0 as u16 + 2u16 * g1 as u16) / 3) as u8,
            ((b0 as u16 + 2u16 * b1 as u16) / 3) as u8,
            255,
        ];
    } else {
        // 透明分支：半混合 + 透明。
        p[2] = [
            ((r0 as u16 + r1 as u16) / 2) as u8,
            ((g0 as u16 + g1 as u16) / 2) as u8,
            ((b0 as u16 + b1 as u16) / 2) as u8,
            255,
        ];
        p[3] = [0, 0, 0, 0];
    }
    p
}

/// 解一个 RGB 565 → (R8, G8, B8)，按 5/6/5-bit 扩展到 8-bit。
fn unpack_565(c: u16) -> (u8, u8, u8) {
    let r = ((c >> 11) & 0x1F) as u8;
    let g = ((c >> 5) & 0x3F) as u8;
    let b = (c & 0x1F) as u8;
    // 5-bit → 8-bit：x << 3 | x >> 2；6-bit：x << 2 | x >> 4。
    let r8 = (r << 3) | (r >> 2);
    let g8 = (g << 2) | (g >> 4);
    let b8 = (b << 3) | (b >> 2);
    (r8, g8, b8)
}

/// 把一个 4×4 块的颜色（按 indices 选 palette[0..3]）写到目的 RGBA buffer。
fn write_block_rgba(
    palette: &[[u8; 4]; 4],
    indices: u32,
    out: &mut [u8],
    w: u32,
    h: u32,
    bx: usize,
    by: usize,
) {
    for py in 0..4 {
        let y = by * 4 + py;
        if y >= h as usize {
            break;
        }
        for px in 0..4 {
            let x = bx * 4 + px;
            if x >= w as usize {
                break;
            }
            let bit_off = (py * 4 + px) * 2;
            let idx = ((indices >> bit_off) & 0x3) as usize;
            let dst = (y * w as usize + x) * 4;
            out[dst..dst + 4].copy_from_slice(&palette[idx]);
        }
    }
}

// ─── BC3 (DXT5) ───────────────────────────────────────────────────────────

/// 解码 BC3 / DXT5。每 4×4 块 16 字节：8 字节 alpha + 8 字节 BC1-style RGB。
fn decode_bc3(src: &[u8], w: u32, h: u32) -> Result<Vec<u8>, DdsDecodeError> {
    let bw = w.div_ceil(4) as usize;
    let bh = h.div_ceil(4) as usize;
    let expected_bytes = bw * bh * 16;
    if src.len() < expected_bytes {
        return Err(DdsDecodeError::SizeMismatch {
            expected: expected_bytes,
            actual: src.len(),
        });
    }
    let mut out = vec![0u8; (w as usize) * (h as usize) * 4];

    for by in 0..bh {
        for bx in 0..bw {
            let off = (by * bw + bx) * 16;
            let alpha_block = &src[off..off + 8];
            let color_block = &src[off + 8..off + 16];

            // alpha 部分。
            let a0 = alpha_block[0];
            let a1 = alpha_block[1];
            let alpha_palette = bc3_alpha_palette(a0, a1);
            // 6 字节 = 48 bit，每像素 3 bit。
            let alpha_bits = u64::from_le_bytes([
                alpha_block[2],
                alpha_block[3],
                alpha_block[4],
                alpha_block[5],
                alpha_block[6],
                alpha_block[7],
                0,
                0,
            ]);

            // 颜色部分（结构同 BC1，但 BC3 的颜色块始终走 1/3 - 2/3 插值，
            // 不复用 BC1 的「c0 ≤ c1 透明分支」——alpha 由独立通道处理）。
            let c0 = u16::from_le_bytes([color_block[0], color_block[1]]);
            let c1 = u16::from_le_bytes([color_block[2], color_block[3]]);
            let indices = u32::from_le_bytes([
                color_block[4],
                color_block[5],
                color_block[6],
                color_block[7],
            ]);
            let color_palette = bc3_color_palette(c0, c1);

            for py in 0..4 {
                let y = by * 4 + py;
                if y >= h as usize {
                    break;
                }
                for px in 0..4 {
                    let x = bx * 4 + px;
                    if x >= w as usize {
                        break;
                    }
                    let pixel_idx = py * 4 + px;
                    let color_idx = ((indices >> (pixel_idx * 2)) & 0x3) as usize;
                    let alpha_idx = ((alpha_bits >> (pixel_idx * 3)) & 0x7) as usize;

                    let rgb = color_palette[color_idx];
                    let a = alpha_palette[alpha_idx];
                    let dst = (y * w as usize + x) * 4;
                    out[dst] = rgb[0];
                    out[dst + 1] = rgb[1];
                    out[dst + 2] = rgb[2];
                    out[dst + 3] = a;
                }
            }
        }
    }

    Ok(out)
}

/// BC3 颜色调色板（始终 4-色 1/3-2/3 插值，与 BC1 不透明分支一致）。
fn bc3_color_palette(c0: u16, c1: u16) -> [[u8; 4]; 4] {
    let (r0, g0, b0) = unpack_565(c0);
    let (r1, g1, b1) = unpack_565(c1);
    [
        [r0, g0, b0, 255],
        [r1, g1, b1, 255],
        [
            ((2u16 * r0 as u16 + r1 as u16) / 3) as u8,
            ((2u16 * g0 as u16 + g1 as u16) / 3) as u8,
            ((2u16 * b0 as u16 + b1 as u16) / 3) as u8,
            255,
        ],
        [
            ((r0 as u16 + 2u16 * r1 as u16) / 3) as u8,
            ((g0 as u16 + 2u16 * g1 as u16) / 3) as u8,
            ((b0 as u16 + 2u16 * b1 as u16) / 3) as u8,
            255,
        ],
    ]
}

/// BC3 alpha 8 元素调色板。`a0 > a1` → 6 中间插值；`a0 ≤ a1` → 4 中间插值 + 0/255。
fn bc3_alpha_palette(a0: u8, a1: u8) -> [u8; 8] {
    let mut p = [0u8; 8];
    p[0] = a0;
    p[1] = a1;
    if a0 > a1 {
        for i in 1..7 {
            p[i + 1] = (((7 - i) as u16 * a0 as u16 + i as u16 * a1 as u16) / 7) as u8;
        }
    } else {
        for i in 1..5 {
            p[i + 1] = (((5 - i) as u16 * a0 as u16 + i as u16 * a1 as u16) / 5) as u8;
        }
        p[6] = 0;
        p[7] = 255;
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 最小 BGRA8：1×1 蓝色像素 (B=255, G=0, R=0, A=255) → RGBA (R=0, G=0, B=255, A=255)。
    #[test]
    fn bgra8_one_pixel_swaps_channels() {
        let dds = DdsImage {
            width: 1,
            height: 1,
            format: DdsFormat::Bgra8,
            mips: vec![hoi4_assets::dds::MipLevel {
                width: 1,
                height: 1,
                offset: 0,
                size: 4,
            }],
            data: vec![255, 0, 0, 255], // BGRA = blue
        };
        let rgba = decode_mip0_to_rgba(&dds).unwrap();
        assert_eq!(rgba, vec![0, 0, 255, 255]);
    }

    #[test]
    fn bgr555_one_pixel_expands_channels() {
        let dds = DdsImage {
            width: 1,
            height: 1,
            format: DdsFormat::Bgr555,
            mips: vec![hoi4_assets::dds::MipLevel {
                width: 1,
                height: 1,
                offset: 0,
                size: 2,
            }],
            data: vec![0x00, 0x7c], // R=31, G=0, B=0
        };
        let rgba = decode_mip0_to_rgba(&dds).unwrap();
        assert_eq!(rgba, vec![255, 0, 0, 255]);
    }

    /// BC1 一个块全 endpoint 0：c0=0xFFFF (white) c1=0x0000 (black) indices=0 → 16 个白像素。
    #[test]
    fn bc1_one_block_all_endpoint0() {
        // c0=0xFFFF (white) c1=0x0000 (black) indices=0x00000000
        let block: [u8; 8] = [0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let dds = DdsImage {
            width: 4,
            height: 4,
            format: DdsFormat::Bc1,
            mips: vec![hoi4_assets::dds::MipLevel {
                width: 4,
                height: 4,
                offset: 0,
                size: 8,
            }],
            data: block.to_vec(),
        };
        let rgba = decode_mip0_to_rgba(&dds).unwrap();
        assert_eq!(rgba.len(), 4 * 4 * 4);
        // 每个像素应为白色（不严格 255，因 5-bit 解到 8-bit 是 0xFF）。
        for i in 0..16 {
            let off = i * 4;
            assert_eq!(rgba[off], 255);
            assert_eq!(rgba[off + 1], 255);
            assert_eq!(rgba[off + 2], 255);
            assert_eq!(rgba[off + 3], 255);
        }
    }

    /// BC3 一个块：alpha 全 0xFF，颜色全白（验证 alpha 与 color 通道独立）。
    #[test]
    fn bc3_one_block_full_alpha_white_color() {
        let block: [u8; 16] = [
            0xFF, 0xFF, // a0=255, a1=255
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // alpha indices = 0 → palette[0] = a0 = 255
            0xFF, 0xFF, 0x00, 0x00, // c0=0xFFFF (white), c1=0x0000 (black)
            0x00, 0x00, 0x00, 0x00, // color indices = 0 → palette[0] = c0 = white
        ];
        let dds = DdsImage {
            width: 4,
            height: 4,
            format: DdsFormat::Bc3,
            mips: vec![hoi4_assets::dds::MipLevel {
                width: 4,
                height: 4,
                offset: 0,
                size: 16,
            }],
            data: block.to_vec(),
        };
        let rgba = decode_mip0_to_rgba(&dds).unwrap();
        for i in 0..16 {
            let off = i * 4;
            assert_eq!(rgba[off], 255);
            assert_eq!(rgba[off + 1], 255);
            assert_eq!(rgba[off + 2], 255);
            assert_eq!(rgba[off + 3], 255);
        }
    }

    /// 不支持的格式返回 Unsupported。
    #[test]
    fn bc5_returns_unsupported() {
        let dds = DdsImage {
            width: 4,
            height: 4,
            format: DdsFormat::Bc5,
            mips: Vec::new(),
            data: Vec::new(),
        };
        match decode_mip0_to_rgba(&dds) {
            Err(DdsDecodeError::Unsupported(DdsFormat::Bc5)) => {}
            other => panic!("expected Unsupported(Bc5), got {other:?}"),
        }
    }
}
