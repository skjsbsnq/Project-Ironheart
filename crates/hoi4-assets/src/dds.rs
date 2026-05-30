//! Phase 2.3：DDS 纹理解析器（升级版）。
//!
//! 相比 `hoi4-render::dds`（仅 mip0 + BC1/BC3/BGRA8），本模块：
//! - 支持完整 mip 链提取
//! - 支持 BC5 (ATI2 / 3Dc — 法线贴图)
//! - 支持 DX10 扩展头（DXGI_FORMAT）
//! - 标记 sRGB vs linear（BC1/BC3 = sRGB color；BC5 = linear normal）
//! - 提供 `DdsImage` 作为 `parse_or_get<DdsImage>` 的缓存类型

use crate::error::AssetError;

/// DDS 像素格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DdsFormat {
    /// DXT1 — 4 bpp，1-bit alpha 或无 alpha。sRGB color data。
    Bc1,
    /// DXT5 — 8 bpp，插值 alpha。sRGB color data。
    Bc3,
    /// ATI2 / 3Dc — 双通道 8 bpp。Linear（法线贴图）。
    Bc5,
    /// 未压缩 32-bit BGRA。
    Bgra8,
    /// 未压缩 16-bit X1R5G5B5 / BGR555。
    Bgr555,
    /// 未识别格式（fourcc 或 bit count）。
    Unknown(u32),
}

impl DdsFormat {
    /// 该格式是否应被视为 sRGB 色彩空间。
    pub fn is_srgb(&self) -> bool {
        matches!(self, Self::Bc1 | Self::Bc3 | Self::Bgra8)
    }

    /// 每个 4×4 block 的字节数（压缩格式）；未压缩返回 None。
    pub fn block_bytes(&self) -> Option<u32> {
        match self {
            Self::Bc1 => Some(8),
            Self::Bc3 | Self::Bc5 => Some(16),
            _ => None,
        }
    }

    /// 每像素字节数（未压缩格式）；压缩格式返回 None。
    pub fn bytes_per_pixel(&self) -> Option<u32> {
        match self {
            Self::Bgra8 => Some(4),
            Self::Bgr555 => Some(2),
            _ => None,
        }
    }
}

/// 单个 mip level 的数据切片描述。
#[derive(Debug, Clone)]
pub struct MipLevel {
    pub width: u32,
    pub height: u32,
    /// 在 `DdsImage::data` 中的字节偏移。
    pub offset: usize,
    /// 本 mip 的字节长度。
    pub size: usize,
}

/// 解析后的 DDS 图像。可通过 `AssetDb::parse_or_get::<DdsImage>` 缓存。
#[derive(Debug, Clone)]
pub struct DdsImage {
    pub width: u32,
    pub height: u32,
    pub format: DdsFormat,
    pub mips: Vec<MipLevel>,
    /// 全部 mip 数据连续存储。
    pub data: Vec<u8>,
}

impl DdsImage {
    /// 从原始字节解析 DDS。供 `AssetDb::parse_or_get` 使用。
    pub fn parse(bytes: &[u8]) -> Result<Self, AssetError> {
        parse_dds_image(bytes)
    }

    /// mip 数量。
    pub fn mip_count(&self) -> u32 {
        self.mips.len() as u32
    }

    /// 取某 mip level 的数据切片。
    pub fn mip_data(&self, level: u32) -> Option<&[u8]> {
        let m = self.mips.get(level as usize)?;
        Some(&self.data[m.offset..m.offset + m.size])
    }
}

/// Per-mip copy parameters shared by map passes when uploading DDS textures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DdsUploadMip {
    pub level: u32,
    pub width: u32,
    pub height: u32,
    pub copy_width: u32,
    pub copy_height: u32,
    pub bytes_per_row: u32,
    pub offset: usize,
    pub size: usize,
}

/// Upload plan for a parsed DDS. The copy extent is block-aligned for BC
/// formats so non-power-of-two vanilla mip chains can be uploaded consistently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DdsUploadPlan {
    pub format: DdsFormat,
    pub source_mip_count: u32,
    pub upload_mip_count: u32,
    pub block_width: u32,
    pub block_height: u32,
    pub bytes_per_block: u32,
    pub mips: Vec<DdsUploadMip>,
}

impl DdsUploadPlan {
    pub fn mip_status(&self) -> &'static str {
        if self.upload_mip_count == 0 {
            "Empty"
        } else {
            "Complete"
        }
    }
}

/// Builds the direct GPU-upload plan for a DDS. `None` means the parsed DDS
/// format needs CPU conversion before upload and must not silently enter the
/// generic 1x1 fallback path without being reported by audit.
pub fn dds_upload_plan(dds: &DdsImage) -> Option<DdsUploadPlan> {
    let (block_width, block_height, bytes_per_block) = dds_upload_layout(dds.format)?;
    let mut mips = Vec::with_capacity(dds.mips.len());
    for (level, mip) in dds.mips.iter().enumerate() {
        let blocks_wide = div_ceil(mip.width, block_width);
        let blocks_tall = div_ceil(mip.height, block_height);
        mips.push(DdsUploadMip {
            level: level as u32,
            width: mip.width,
            height: mip.height,
            copy_width: blocks_wide * block_width,
            copy_height: blocks_tall * block_height,
            bytes_per_row: blocks_wide * bytes_per_block,
            offset: mip.offset,
            size: mip.size,
        });
    }

    Some(DdsUploadPlan {
        format: dds.format,
        source_mip_count: dds.mip_count(),
        upload_mip_count: mips.len() as u32,
        block_width,
        block_height,
        bytes_per_block,
        mips,
    })
}

/// Returns the block geometry used by Queue::write_texture for directly
/// supported DDS formats. BGR555 is intentionally excluded because it requires
/// CPU expansion before it can be uploaded to a 32-bit wgpu format.
pub fn dds_upload_layout(format: DdsFormat) -> Option<(u32, u32, u32)> {
    match format {
        DdsFormat::Bc1 => Some((4, 4, 8)),
        DdsFormat::Bc3 | DdsFormat::Bc5 => Some((4, 4, 16)),
        DdsFormat::Bgra8 => Some((1, 1, 4)),
        DdsFormat::Bgr555 | DdsFormat::Unknown(_) => None,
    }
}

fn div_ceil(value: u32, divisor: u32) -> u32 {
    (value + divisor - 1) / divisor
}

// ─── 常量 ───────────────────────────────────────────────────────────────────

const DDS_MAGIC: u32 = 0x20534444; // "DDS "
const DDPF_FOURCC: u32 = 0x4;
const FOURCC_DXT1: u32 = 0x31545844;
const FOURCC_DXT5: u32 = 0x35545844;
const FOURCC_ATI2: u32 = 0x32495441; // "ATI2"
const FOURCC_BC5U: u32 = 0x55354342; // "BC5U"
const FOURCC_DX10: u32 = 0x30315844; // "DX10"

// DXGI format codes we care about
const DXGI_FORMAT_BC1_UNORM: u32 = 71;
const DXGI_FORMAT_BC1_UNORM_SRGB: u32 = 72;
const DXGI_FORMAT_BC3_UNORM: u32 = 77;
const DXGI_FORMAT_BC3_UNORM_SRGB: u32 = 78;
const DXGI_FORMAT_BC5_UNORM: u32 = 83;

// ─── 解析 ───────────────────────────────────────────────────────────────────

fn parse_dds_image(bytes: &[u8]) -> Result<DdsImage, AssetError> {
    if bytes.len() < 128 {
        return Err(AssetError::parse("", "DDS too small (< 128 bytes)"));
    }
    let magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    if magic != DDS_MAGIC {
        return Err(AssetError::parse(
            "",
            format!("Bad DDS magic: {:#X}", magic),
        ));
    }

    let header = &bytes[4..128];
    let height = u32::from_le_bytes(header[8..12].try_into().unwrap());
    let width = u32::from_le_bytes(header[12..16].try_into().unwrap());
    let mip_count = u32::from_le_bytes(header[24..28].try_into().unwrap()).max(1);

    // Pixel format at header offset 72
    let pf = &header[72..104];
    let pf_flags = u32::from_le_bytes(pf[4..8].try_into().unwrap());

    let (format, data_start) = if pf_flags & DDPF_FOURCC != 0 {
        let cc = u32::from_le_bytes(pf[8..12].try_into().unwrap());
        if cc == FOURCC_DX10 {
            // DX10 extended header (20 bytes after main header)
            if bytes.len() < 148 {
                return Err(AssetError::parse("", "DDS DX10 header truncated"));
            }
            let dx10 = &bytes[128..148];
            let dxgi_format = u32::from_le_bytes(dx10[0..4].try_into().unwrap());
            let fmt = match dxgi_format {
                DXGI_FORMAT_BC1_UNORM | DXGI_FORMAT_BC1_UNORM_SRGB => DdsFormat::Bc1,
                DXGI_FORMAT_BC3_UNORM | DXGI_FORMAT_BC3_UNORM_SRGB => DdsFormat::Bc3,
                DXGI_FORMAT_BC5_UNORM => DdsFormat::Bc5,
                other => DdsFormat::Unknown(other),
            };
            (fmt, 148usize)
        } else {
            let fmt = match cc {
                FOURCC_DXT1 => DdsFormat::Bc1,
                FOURCC_DXT5 => DdsFormat::Bc3,
                FOURCC_ATI2 | FOURCC_BC5U => DdsFormat::Bc5,
                other => DdsFormat::Unknown(other),
            };
            (fmt, 128usize)
        }
    } else {
        let rgb_bit_count = u32::from_le_bytes(pf[12..16].try_into().unwrap());
        let r_mask = u32::from_le_bytes(pf[16..20].try_into().unwrap());
        let g_mask = u32::from_le_bytes(pf[20..24].try_into().unwrap());
        let b_mask = u32::from_le_bytes(pf[24..28].try_into().unwrap());
        let a_mask = u32::from_le_bytes(pf[28..32].try_into().unwrap());
        match (rgb_bit_count, r_mask, g_mask, b_mask, a_mask) {
            (32, _, _, _, _) => (DdsFormat::Bgra8, 128usize),
            (16, 0x0000_7c00, 0x0000_03e0, 0x0000_001f, 0) => (DdsFormat::Bgr555, 128usize),
            _ => (DdsFormat::Unknown(rgb_bit_count), 128usize),
        }
    };

    // Build mip chain
    let mut mips = Vec::with_capacity(mip_count as usize);
    let mut offset = 0usize;
    let mut w = width;
    let mut h = height;

    for _ in 0..mip_count {
        let size = mip_size(w, h, format);
        mips.push(MipLevel {
            width: w,
            height: h,
            offset,
            size,
        });
        offset += size;
        w = (w / 2).max(1);
        h = (h / 2).max(1);
    }

    let total_data = offset;
    if bytes.len() < data_start + total_data {
        // Fallback: at least mip0
        let mip0_sz = mips.first().map(|m| m.size).unwrap_or(0);
        if bytes.len() < data_start + mip0_sz {
            return Err(AssetError::parse(
                "",
                format!(
                    "DDS truncated: need {} bytes, have {}",
                    data_start + mip0_sz,
                    bytes.len()
                ),
            ));
        }
        // Truncate mip chain to what's available
        let available = bytes.len() - data_start;
        let mut kept = 0;
        let mut acc = 0usize;
        for m in &mips {
            if acc + m.size > available {
                break;
            }
            acc += m.size;
            kept += 1;
        }
        mips.truncate(kept);
        let data = bytes[data_start..data_start + acc].to_vec();
        return Ok(DdsImage {
            width,
            height,
            format,
            mips,
            data,
        });
    }

    let data = bytes[data_start..data_start + total_data].to_vec();
    Ok(DdsImage {
        width,
        height,
        format,
        mips,
        data,
    })
}

/// 计算单个 mip level 的字节大小。
pub fn mip_size(width: u32, height: u32, format: DdsFormat) -> usize {
    match format {
        DdsFormat::Bc1 => {
            let bw = (width + 3) / 4;
            let bh = (height + 3) / 4;
            (bw * bh * 8) as usize
        }
        DdsFormat::Bc3 | DdsFormat::Bc5 => {
            let bw = (width + 3) / 4;
            let bh = (height + 3) / 4;
            (bw * bh * 16) as usize
        }
        DdsFormat::Bgra8 | DdsFormat::Unknown(_) => (width * height * 4) as usize,
        DdsFormat::Bgr555 => (width * height * 2) as usize,
    }
}

// ─── Frame slicing ──────────────────────────────────────────────────────────

/// 帧切片 UV 矩形（atlas 横向均分）。
/// `u_min..u_max` 是归一化 [0,1] 范围。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameUv {
    pub u_min: f32,
    pub u_max: f32,
    pub v_min: f32,
    pub v_max: f32,
}

/// 按 `no_of_frames` 横向均分纹理，返回每帧的 UV 矩形。
/// frame 0 在最左，frame N-1 在最右。
pub fn compute_frame_uvs(no_of_frames: u32) -> Vec<FrameUv> {
    if no_of_frames == 0 {
        return vec![];
    }
    let step = 1.0 / no_of_frames as f32;
    (0..no_of_frames)
        .map(|i| FrameUv {
            u_min: i as f32 * step,
            u_max: (i + 1) as f32 * step,
            v_min: 0.0,
            v_max: 1.0,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── synth helpers ──────────────────────────────────────────────────────

    const DDPF_FOURCC_FLAG: u32 = 0x4;

    fn synth_dds(width: u32, height: u32, fourcc_str: &[u8; 4], mip_count: u32) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&DDS_MAGIC.to_le_bytes());
        let mut header = vec![0u8; 124];
        header[0..4].copy_from_slice(&124u32.to_le_bytes());
        header[4..8].copy_from_slice(&0x1007u32.to_le_bytes());
        header[8..12].copy_from_slice(&height.to_le_bytes());
        header[12..16].copy_from_slice(&width.to_le_bytes());
        header[24..28].copy_from_slice(&mip_count.to_le_bytes());
        let pf_offset = 72;
        header[pf_offset..pf_offset + 4].copy_from_slice(&32u32.to_le_bytes());
        header[pf_offset + 4..pf_offset + 8].copy_from_slice(&DDPF_FOURCC_FLAG.to_le_bytes());
        header[pf_offset + 8..pf_offset + 12].copy_from_slice(fourcc_str);
        buf.extend_from_slice(&header);

        // Compute total data for all mips
        let format = match fourcc_str {
            b"DXT1" => DdsFormat::Bc1,
            b"DXT5" => DdsFormat::Bc3,
            b"ATI2" => DdsFormat::Bc5,
            _ => DdsFormat::Bc3,
        };
        let mut total = 0usize;
        let mut w = width;
        let mut h = height;
        for _ in 0..mip_count {
            total += mip_size(w, h, format);
            w = (w / 2).max(1);
            h = (h / 2).max(1);
        }
        buf.resize(128 + total, 0xAB);
        buf
    }

    fn synth_bgra(width: u32, height: u32) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&DDS_MAGIC.to_le_bytes());
        let mut header = vec![0u8; 124];
        header[0..4].copy_from_slice(&124u32.to_le_bytes());
        header[4..8].copy_from_slice(&0x1007u32.to_le_bytes());
        header[8..12].copy_from_slice(&height.to_le_bytes());
        header[12..16].copy_from_slice(&width.to_le_bytes());
        header[24..28].copy_from_slice(&1u32.to_le_bytes());
        let pf_offset = 72;
        header[pf_offset..pf_offset + 4].copy_from_slice(&32u32.to_le_bytes());
        // No FOURCC flag, rgb_bit_count = 32
        header[pf_offset + 4..pf_offset + 8].copy_from_slice(&0u32.to_le_bytes());
        header[pf_offset + 12..pf_offset + 16].copy_from_slice(&32u32.to_le_bytes());
        buf.extend_from_slice(&header);
        buf.resize(128 + (width * height * 4) as usize, 0xCD);
        buf
    }

    // ─── tests ──────────────────────────────────────────────────────────────

    #[test]
    fn parse_bc1_single_mip() {
        let dds = synth_dds(256, 128, b"DXT1", 1);
        let img = DdsImage::parse(&dds).unwrap();
        assert_eq!(img.width, 256);
        assert_eq!(img.height, 128);
        assert_eq!(img.format, DdsFormat::Bc1);
        assert_eq!(img.mip_count(), 1);
        assert_eq!(
            img.mip_data(0).unwrap().len(),
            mip_size(256, 128, DdsFormat::Bc1)
        );
    }

    #[test]
    fn parse_bc3_with_mip_chain() {
        let dds = synth_dds(64, 64, b"DXT5", 4);
        let img = DdsImage::parse(&dds).unwrap();
        assert_eq!(img.format, DdsFormat::Bc3);
        assert_eq!(img.mip_count(), 4);
        assert_eq!(img.mips[0].width, 64);
        assert_eq!(img.mips[1].width, 32);
        assert_eq!(img.mips[2].width, 16);
        assert_eq!(img.mips[3].width, 8);
    }

    #[test]
    fn parse_bc5_ati2() {
        let dds = synth_dds(128, 128, b"ATI2", 1);
        let img = DdsImage::parse(&dds).unwrap();
        assert_eq!(img.format, DdsFormat::Bc5);
        assert!(!img.format.is_srgb());
    }

    #[test]
    fn parse_bgra8() {
        let dds = synth_bgra(16, 16);
        let img = DdsImage::parse(&dds).unwrap();
        assert_eq!(img.format, DdsFormat::Bgra8);
        assert_eq!(img.mip_data(0).unwrap().len(), 16 * 16 * 4);
    }

    #[test]
    fn upload_plan_rounds_bc_copy_extent() {
        let dds = synth_dds(22, 8, b"DXT5", 1);
        let img = DdsImage::parse(&dds).unwrap();
        let plan = dds_upload_plan(&img).unwrap();
        assert_eq!(plan.upload_mip_count, 1);
        assert_eq!(plan.block_width, 4);
        assert_eq!(plan.bytes_per_block, 16);
        assert_eq!(plan.mips[0].width, 22);
        assert_eq!(plan.mips[0].copy_width, 24);
        assert_eq!(plan.mips[0].copy_height, 8);
        assert_eq!(plan.mips[0].bytes_per_row, 6 * 16);
    }

    #[test]
    fn upload_plan_supports_bgra8_rows() {
        let dds = synth_bgra(16, 8);
        let img = DdsImage::parse(&dds).unwrap();
        let plan = dds_upload_plan(&img).unwrap();
        assert_eq!(plan.block_width, 1);
        assert_eq!(plan.mips[0].copy_width, 16);
        assert_eq!(plan.mips[0].copy_height, 8);
        assert_eq!(plan.mips[0].bytes_per_row, 16 * 4);
    }

    #[test]
    fn srgb_marking() {
        assert!(DdsFormat::Bc1.is_srgb());
        assert!(DdsFormat::Bc3.is_srgb());
        assert!(!DdsFormat::Bc5.is_srgb());
    }

    #[test]
    fn frame_uvs_single() {
        let uvs = compute_frame_uvs(1);
        assert_eq!(uvs.len(), 1);
        assert!((uvs[0].u_min - 0.0).abs() < 1e-6);
        assert!((uvs[0].u_max - 1.0).abs() < 1e-6);
    }

    #[test]
    fn frame_uvs_three() {
        let uvs = compute_frame_uvs(3);
        assert_eq!(uvs.len(), 3);
        assert!((uvs[0].u_max - 1.0 / 3.0).abs() < 1e-5);
        assert!((uvs[1].u_min - 1.0 / 3.0).abs() < 1e-5);
        assert!((uvs[2].u_max - 1.0).abs() < 1e-5);
    }

    #[test]
    fn truncated_mip_chain_graceful() {
        let mut dds = synth_dds(64, 64, b"DXT5", 4);
        // Truncate so only mip0 + mip1 fit
        let m0 = mip_size(64, 64, DdsFormat::Bc3);
        let m1 = mip_size(32, 32, DdsFormat::Bc3);
        dds.truncate(128 + m0 + m1 + 1); // +1 so mip2 doesn't fully fit
        let img = DdsImage::parse(&dds).unwrap();
        assert_eq!(img.mip_count(), 2); // gracefully truncated
    }

    #[test]
    fn rejects_bad_magic() {
        let mut dds = synth_dds(4, 4, b"DXT1", 1);
        dds[0] = 0;
        assert!(DdsImage::parse(&dds).is_err());
    }

    #[test]
    fn rejects_too_small() {
        assert!(DdsImage::parse(&[0u8; 64]).is_err());
    }
}
