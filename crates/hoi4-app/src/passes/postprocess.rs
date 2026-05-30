//! Phase 3.12.2 — 后处理链。
//!
//! 把 3.11.13 翻译的 6 个 wgsl（bloom / downsample / downsample_luminance /
//! lut_blender / restorescene / saturation_slider）串成完整链，接到 3.12.1
//! 建好的 [`super::HdrTarget`] 上。
//!
//! ## 链结构（自上而下）
//!
//! ```text
//!  HDR (W×H, RGBA16Float)
//!   ├─► bloom_bright (W/2×H/2, bright-pass + 9-tap blur)
//!   │      └─► downsample×3 (1/4 → 1/8 → 1/16, RGBA16Float, 13-tap Karis)
//!   │
//!   ├─► lum_log_reduction (W/16, R16Float, 9-tap log-luminance)
//!   │      └─► lum_avg_reduction×N (R16Float, 几何缩小到 1×1)
//!   │
//!   └─► restorescene (HDR + bloom_lvl4 + lum_1x1 → swap-chain LDR)
//!          │  ACES tonemap + 自动曝光 + bloom 叠 + vignette
//!          ▼
//!         (saturation_slider / lut 暂跳过；可在设置面板再接)
//! ```
//!
//! ## 与 sRGB 交换链的契合
//!
//! `restorescene_live.wgsl`（本模块内嵌的修改版）**不**在 fragment 末尾做手工
//! `pow(x, 1/2.2)` —— 我们写到 `Bgra8UnormSrgb`，硬件自动 sRGB 编码即可。**保留** wgsl
//! 原版翻译 `crates/hoi4-render/src/translations/restorescene.wgsl` 不动作为参考。
//!
//! ## 自动曝光
//!
//! 用一条 GPU-only 的多级 reduction：HDR → 1×1 R16Float `lum_tex`。restorescene
//! 直接 `textureSample(lum_tex, ..., (0.5, 0.5))` 取出 `avg_log_lum`，无 CPU 回读。
//!
//! ## 简化版 / 完整版切换
//!
//! `PostProcessMode::Off` → 只做简化 blit（[`super::SimpleBlitPass`] 取代）。
//! `PostProcessMode::Full` → 完整链。F4 / 设置面板可切换。

use crate::passes::HDR_FORMAT;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostProcessDebugView {
    Final,
    HdrRaw,
    TonemapOnly,
    BloomOnly,
}

impl PostProcessDebugView {
    pub const ALL: [Self; 4] = [
        Self::Final,
        Self::HdrRaw,
        Self::TonemapOnly,
        Self::BloomOnly,
    ];

    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|view| *view == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Final => "final",
            Self::HdrRaw => "hdr_raw",
            Self::TonemapOnly => "tonemap_only",
            Self::BloomOnly => "bloom_only",
        }
    }

    const fn as_shader_value(self) -> f32 {
        match self {
            Self::Final => 0.0,
            Self::HdrRaw => 1.0,
            Self::TonemapOnly => 2.0,
            Self::BloomOnly => 3.0,
        }
    }
}

impl Default for PostProcessDebugView {
    fn default() -> Self {
        Self::Final
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PostProcessCalibration {
    pub bloom_bright_threshold: f32,
    pub bloom_prefilter_strength: f32,
    pub middle_grey: f32,
    pub exposure_min: f32,
    pub exposure_max: f32,
    pub aces_input_scale: f32,
    pub final_bloom_strength: f32,
    pub saturation: f32,
    pub vignette_strength: f32,
    pub bloom_debug_gain: f32,
}

impl PostProcessCalibration {
    pub const fn phase9_high() -> Self {
        Self {
            bloom_bright_threshold: 1.05,
            bloom_prefilter_strength: 0.6,
            middle_grey: 0.18,
            exposure_min: 0.6,
            exposure_max: 1.3,
            aces_input_scale: 0.6,
            final_bloom_strength: 0.05,
            saturation: 1.0,
            vignette_strength: 0.08,
            bloom_debug_gain: 4.0,
        }
    }

    pub fn summary(self) -> String {
        format!(
            "exposure=[{:.2},{:.2}] middle_grey={:.2} aces_scale={:.2} bloom={:.2}/{:.2} sat={:.2} vignette={:.2}",
            self.exposure_min,
            self.exposure_max,
            self.middle_grey,
            self.aces_input_scale,
            self.bloom_bright_threshold,
            self.final_bloom_strength,
            self.saturation,
            self.vignette_strength
        )
    }
}

impl Default for PostProcessCalibration {
    fn default() -> Self {
        Self::phase9_high()
    }
}

/// 后处理执行模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostProcessMode {
    /// 简化模式：跳过完整后处理链，由 [`super::SimpleBlitPass`] 接管。
    Off,
    /// 完整后处理链。
    Full,
}

impl Default for PostProcessMode {
    fn default() -> Self {
        // Phase 3.12.2 默认开启 Full；用户可通过设置面板（未来）回退到 Off。
        Self::Full
    }
}

// ─── 内嵌 / 改写的 shader 源 ──────────────────────────────────────────────────

/// 全屏三角形 + 9-tap bright-pass。源自 `translations/bloom.wgsl`，原样使用。
const BLOOM_BRIGHT_WGSL: &str = include_str!("../../../hoi4-render/src/translations/bloom.wgsl");

/// 13-tap Karis-下采样。源自 `translations/downsample.wgsl`。
const DOWNSAMPLE_WGSL: &str = include_str!("../../../hoi4-render/src/translations/downsample.wgsl");

/// 9-tap log-luminance 第一级。源自 `translations/downsample_luminance.wgsl`。
const LUM_LOG_WGSL: &str =
    include_str!("../../../hoi4-render/src/translations/downsample_luminance.wgsl");

/// 9-tap 普通平均（第 2..N 级 luminance reduction 用，本模块内嵌 — 翻译目录的
/// log 版本只适合第 1 级；后续级输入已经是标量 log-luminance 了）。
const LUM_AVG_WGSL: &str = r#"
@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;

struct Params {
    inv_src_size: vec2<f32>,
    _pad: vec2<f32>,
};
@group(0) @binding(2) var<uniform> p: Params;

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VsOut {
    var out: VsOut;
    let uv = vec2<f32>(f32((vid << 1u) & 2u), f32(vid & 2u));
    out.clip_pos = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(uv.x, 1.0 - uv.y);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let q = p.inv_src_size;
    var sum = 0.0;
    let offsets = array<vec2<f32>, 9>(
        vec2<f32>(-1.0, -1.0), vec2<f32>( 0.0, -1.0), vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  0.0), vec2<f32>( 0.0,  0.0), vec2<f32>( 1.0,  0.0),
        vec2<f32>(-1.0,  1.0), vec2<f32>( 0.0,  1.0), vec2<f32>( 1.0,  1.0),
    );
    for (var i = 0; i < 9; i = i + 1) {
        sum = sum + textureSample(src_tex, src_sampler, in.uv + offsets[i] * q).r;
    }
    return vec4<f32>(sum / 9.0, 0.0, 0.0, 1.0);
}
"#;

/// 修改版 restorescene：避免双 sRGB（输出到 sRGB 交换链时跳过 pow），并通过
/// 1×1 `lum_tex` 提供 `avg_log_lum` 而非 uniform。其余 ACES + bloom + vignette
/// 与翻译目录原版逐行等价。
const RESTORESCENE_LIVE_WGSL: &str = r#"
@group(0) @binding(0) var hdr_tex: texture_2d<f32>;
@group(0) @binding(1) var hdr_sampler: sampler;
@group(0) @binding(2) var bloom_tex: texture_2d<f32>;
@group(0) @binding(3) var bloom_sampler: sampler;
@group(0) @binding(4) var lum_tex: texture_2d<f32>;
@group(0) @binding(5) var lum_sampler: sampler;

struct RestoreParams {
    middle_grey: f32,
    bloom_strength: f32,
    vignette_strength: f32,
    /// 0 = 输出非 sRGB（应用手工 gamma），1 = 输出 sRGB（跳过手工 gamma，硬件代劳）
    srgb_target: f32,
    exposure_min: f32,
    exposure_max: f32,
    aces_input_scale: f32,
    saturation: f32,
    debug_view: f32,
    bloom_debug_gain: f32,
    _pad0: vec2<f32>,
    center_uv: vec2<f32>,
    _pad: vec2<f32>,
};
@group(0) @binding(6) var<uniform> rp: RestoreParams;

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VsOut {
    var out: VsOut;
    let uv = vec2<f32>(f32((vid << 1u) & 2u), f32(vid & 2u));
    out.clip_pos = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(uv.x, 1.0 - uv.y);
    return out;
}

fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let scene = textureSample(hdr_tex, hdr_sampler, in.uv).rgb;
    let bloom = textureSample(bloom_tex, bloom_sampler, in.uv).rgb;
    let combined = scene + bloom * rp.bloom_strength;

    // 自动曝光：从 1×1 lum_tex 取场景平均 log-luminance，但 clamp 到 [-1.5, 1.5]
    // 防止极暗场景（如菜单 dark blue）把 exposure 推到 10× 以上。
    let raw_log_lum = textureSample(lum_tex, lum_sampler, vec2<f32>(0.5, 0.5)).r;
    let avg_log_lum = clamp(raw_log_lum, -1.5, 1.5);
    let avg_lum = max(exp(avg_log_lum), 1e-3);
    // exposure clamp 收到 [0.6, 1.3] —— 防止暗场景被自动曝光拉亮 2 倍把
    // 深 navy 水面变回亮 cyan,同时仍允许轻度场景适应
    let exposure = clamp(rp.middle_grey / avg_lum, rp.exposure_min, rp.exposure_max);
    let exposed = combined * exposure;

    var ldr = aces_tonemap(exposed * rp.aces_input_scale);

    // Desaturate 2% — 保留参考图政治色的鲜艳度,只压一点 ACES 的过饱和
    let lum = dot(ldr, vec3<f32>(0.2125, 0.7154, 0.0721));
    ldr = mix(vec3<f32>(lum), ldr, rp.saturation);

    if (rp.debug_view > 0.5 && rp.debug_view < 1.5) {
        ldr = clamp(scene, vec3<f32>(0.0), vec3<f32>(1.0));
    } else if (rp.debug_view > 1.5 && rp.debug_view < 2.5) {
        ldr = aces_tonemap(scene * exposure * rp.aces_input_scale);
    } else if (rp.debug_view > 2.5 && rp.debug_view < 3.5) {
        ldr = clamp(bloom * rp.bloom_debug_gain, vec3<f32>(0.0), vec3<f32>(1.0));
    }

    // 手工 gamma：仅在非 sRGB 目标时启用（sRGB 目标走硬件 sRGB 编码）
    if (rp.srgb_target < 0.5) {
        ldr = pow(ldr, vec3<f32>(1.0 / 2.2));
    }

    // vignette
    let d = distance(in.uv, rp.center_uv);
    let vig = 1.0 - smoothstep(0.4, 0.85, d) * rp.vignette_strength;
    ldr = ldr * vig;

    return vec4<f32>(ldr, 1.0);
}
"#;

// ─── Uniform 结构 ────────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct BloomParams {
    bright_threshold: f32,
    bloom_strength: f32,
    inv_size_x: f32,
    inv_size_y: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct DownParams {
    inv_src_size: [f32; 2],
    _pad: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct LumParams {
    inv_src_size: [f32; 2],
    _pad: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct RestoreParams {
    middle_grey: f32,
    bloom_strength: f32,
    vignette_strength: f32,
    srgb_target: f32,
    exposure_min: f32,
    exposure_max: f32,
    aces_input_scale: f32,
    saturation: f32,
    debug_view: f32,
    bloom_debug_gain: f32,
    _pad0: [f32; 2],
    center_uv: [f32; 2],
    _pad: [f32; 2],
}

// ─── 单个 RT + bind group 的小辅助 ─────────────────────────────────────────────

struct PingTarget {
    #[allow(dead_code)]
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

fn make_target(
    device: &wgpu::Device,
    label: &str,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> PingTarget {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    PingTarget {
        texture,
        view,
        width: width.max(1),
        height: height.max(1),
    }
}

// ─── 共享 BGL：全屏 sample + uniform ──────────────────────────────────────────

fn make_simple_bgl(device: &wgpu::Device, label: &str) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    })
}

fn make_simple_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    src_view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    uniform: &wgpu::Buffer,
    label: &str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(src_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: uniform.as_entire_binding(),
            },
        ],
    })
}

fn make_simple_pipeline(
    device: &wgpu::Device,
    label: &str,
    wgsl: &str,
    layout: &wgpu::BindGroupLayout,
    target_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(wgsl.into()),
    });
    let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&pl),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview: None,
        cache: None,
    })
}

// ─── 真正的 PostProcessChain ─────────────────────────────────────────────────

const BLOOM_LEVELS: usize = 4;
const LUM_LEVELS: usize = 4;

pub struct PostProcessChain {
    /// 完整 / Off 切换。
    pub mode: PostProcessMode,
    pub calibration: PostProcessCalibration,
    pub debug_view: PostProcessDebugView,

    /// 共享：全屏 sample + uniform 用 BGL（bloom / downsample / lum 都用这个 layout）。
    simple_bgl: wgpu::BindGroupLayout,

    sampler: wgpu::Sampler,

    // bloom chain
    bloom_pipeline: wgpu::RenderPipeline,
    bloom_uniform: wgpu::Buffer,
    bloom_targets: Vec<PingTarget>, // 长度 = BLOOM_LEVELS（lvl0 = bright，lvl1..3 = downsample）
    bloom_bind_groups: Vec<wgpu::BindGroup>, // lvl0 输入 hdr，lvl1.. 输入 上一级 view

    downsample_pipeline: wgpu::RenderPipeline,
    downsample_uniforms: Vec<wgpu::Buffer>, // 一个 per downsample step

    // luminance reduction chain
    lum_log_pipeline: wgpu::RenderPipeline,
    lum_avg_pipeline: wgpu::RenderPipeline,
    lum_uniforms: Vec<wgpu::Buffer>,
    lum_targets: Vec<PingTarget>,
    lum_bind_groups: Vec<wgpu::BindGroup>,

    // restorescene final
    restore_bgl: wgpu::BindGroupLayout,
    restore_pipeline: wgpu::RenderPipeline,
    restore_uniform: wgpu::Buffer,
    restore_bind_group: wgpu::BindGroup,

    /// HDR full-res 尺寸（用来计算各级链尺寸）。
    hdr_w: u32,
    hdr_h: u32,
    /// swap-chain 输出格式 (`Bgra8UnormSrgb` 等)。
    swap_format: wgpu::TextureFormat,
    /// `1.0` 当 `swap_format` 是 sRGB 时（restorescene 跳过手工 gamma）。
    srgb_target_flag: f32,
}

impl PostProcessChain {
    pub fn new(
        device: &wgpu::Device,
        hdr_view: &wgpu::TextureView,
        hdr_w: u32,
        hdr_h: u32,
        swap_format: wgpu::TextureFormat,
    ) -> Self {
        let _ = HDR_FORMAT; // sanity reference

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("postprocess_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let simple_bgl = make_simple_bgl(device, "postprocess_simple_bgl");

        // ── bloom ───────────────────────────────────────────────────────
        let bloom_pipeline = make_simple_pipeline(
            device,
            "bloom_bright",
            BLOOM_BRIGHT_WGSL,
            &simple_bgl,
            HDR_FORMAT,
        );
        let bloom_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bloom_uniform"),
            size: std::mem::size_of::<BloomParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let downsample_pipeline = make_simple_pipeline(
            device,
            "bloom_downsample",
            DOWNSAMPLE_WGSL,
            &simple_bgl,
            HDR_FORMAT,
        );

        // ── lum ─────────────────────────────────────────────────────────
        let lum_log_pipeline = make_simple_pipeline(
            device,
            "lum_log",
            LUM_LOG_WGSL,
            &simple_bgl,
            wgpu::TextureFormat::R16Float,
        );
        let lum_avg_pipeline = make_simple_pipeline(
            device,
            "lum_avg",
            LUM_AVG_WGSL,
            &simple_bgl,
            wgpu::TextureFormat::R16Float,
        );

        // ── restore ─────────────────────────────────────────────────────
        let restore_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("restore_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let restore_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("restore_live"),
            source: wgpu::ShaderSource::Wgsl(RESTORESCENE_LIVE_WGSL.into()),
        });
        let restore_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("restore_pl"),
            bind_group_layouts: &[&restore_bgl],
            push_constant_ranges: &[],
        });
        let restore_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("restore_pipeline"),
            layout: Some(&restore_pl),
            vertex: wgpu::VertexState {
                module: &restore_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &restore_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: swap_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let restore_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("restore_uniform"),
            size: std::mem::size_of::<RestoreParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let srgb_target_flag = if swap_format.is_srgb() { 1.0 } else { 0.0 };

        let mut chain = Self {
            mode: PostProcessMode::default(),
            calibration: PostProcessCalibration::default(),
            debug_view: PostProcessDebugView::default(),
            simple_bgl,
            sampler,
            bloom_pipeline,
            bloom_uniform,
            bloom_targets: Vec::new(),
            bloom_bind_groups: Vec::new(),
            downsample_pipeline,
            downsample_uniforms: Vec::new(),
            lum_log_pipeline,
            lum_avg_pipeline,
            lum_uniforms: Vec::new(),
            lum_targets: Vec::new(),
            lum_bind_groups: Vec::new(),
            restore_bgl,
            restore_pipeline,
            restore_uniform,
            // restore_bind_group placeholder filled below
            restore_bind_group: device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("restore_bg_placeholder"),
                layout: &device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("placeholder_bgl"),
                    entries: &[],
                }),
                entries: &[],
            }),
            hdr_w,
            hdr_h,
            swap_format,
            srgb_target_flag,
        };
        chain.rebuild_targets(device, hdr_view, hdr_w, hdr_h);
        chain
    }

    /// HDR resize 时调用：重新创建所有 ping-pong 目标 + 重绑 bind group。
    pub fn cycle_debug_view(&mut self) -> PostProcessDebugView {
        self.debug_view = self.debug_view.next();
        self.debug_view
    }

    pub fn rebuild_targets(
        &mut self,
        device: &wgpu::Device,
        hdr_view: &wgpu::TextureView,
        hdr_w: u32,
        hdr_h: u32,
    ) {
        self.hdr_w = hdr_w;
        self.hdr_h = hdr_h;

        // ── bloom targets (level 0 = W/2 × H/2; halve each step) ────────
        self.bloom_targets.clear();
        let mut w = (hdr_w / 2).max(2);
        let mut h = (hdr_h / 2).max(2);
        for lvl in 0..BLOOM_LEVELS {
            let label = format!("bloom_lvl{}", lvl);
            self.bloom_targets
                .push(make_target(device, &label, w, h, HDR_FORMAT));
            w = (w / 2).max(2);
            h = (h / 2).max(2);
        }

        // 先一次性创建 downsample uniform buffers（lvl1..N），等下再绑 bind group。
        self.downsample_uniforms.clear();
        for lvl in 1..BLOOM_LEVELS {
            let _ = lvl;
            let buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("downsample_uniform"),
                size: std::mem::size_of::<DownParams>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.downsample_uniforms.push(buf);
        }

        // bloom bind groups: lvl0 reads hdr; lvl1..3 read previous level
        self.bloom_bind_groups.clear();
        for lvl in 0..BLOOM_LEVELS {
            let src_view = if lvl == 0 {
                hdr_view
            } else {
                &self.bloom_targets[lvl - 1].view
            };
            let uniform = if lvl == 0 {
                &self.bloom_uniform
            } else {
                &self.downsample_uniforms[lvl - 1]
            };
            let bg = make_simple_bind_group(
                device,
                &self.simple_bgl,
                src_view,
                &self.sampler,
                uniform,
                &format!("bloom_bg_lvl{}", lvl),
            );
            self.bloom_bind_groups.push(bg);
        }

        // ── luminance reduction targets ─────────────────────────────────
        // lvl0 reads HDR full-res, outputs to W/16 × H/16 (R16Float)
        // lvl1..3 average down towards 1×1
        self.lum_targets.clear();
        let mut lw = (hdr_w / 16).max(2);
        let mut lh = (hdr_h / 16).max(2);
        for lvl in 0..LUM_LEVELS {
            let label = format!("lum_lvl{}", lvl);
            self.lum_targets.push(make_target(
                device,
                &label,
                lw,
                lh,
                wgpu::TextureFormat::R16Float,
            ));
            // 下一级再 ÷ 4
            lw = (lw / 4).max(1);
            lh = (lh / 4).max(1);
        }

        // 一次性创建 lum uniforms。
        self.lum_uniforms.clear();
        for _ in 0..LUM_LEVELS {
            let uniform = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("lum_uniform"),
                size: std::mem::size_of::<LumParams>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.lum_uniforms.push(uniform);
        }

        self.lum_bind_groups.clear();
        for lvl in 0..LUM_LEVELS {
            let src_view = if lvl == 0 {
                hdr_view
            } else {
                &self.lum_targets[lvl - 1].view
            };
            let bg = make_simple_bind_group(
                device,
                &self.simple_bgl,
                src_view,
                &self.sampler,
                &self.lum_uniforms[lvl],
                &format!("lum_bg_lvl{}", lvl),
            );
            self.lum_bind_groups.push(bg);
        }

        // ── restore bind group ──────────────────────────────────────────
        let bloom_final_view = &self.bloom_targets[BLOOM_LEVELS - 1].view;
        let lum_final_view = &self.lum_targets[LUM_LEVELS - 1].view;
        self.restore_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("restore_bg"),
            layout: &self.restore_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(hdr_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(bloom_final_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(lum_final_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self.restore_uniform.as_entire_binding(),
                },
            ],
        });
    }

    /// 每帧调一次，更新所有 uniform。
    pub fn prepare(&self, queue: &wgpu::Queue) {
        // bloom bright pass：threshold 抬到 1.05 → 仅真正 HDR 高光（>1.0）参与 bloom，
        // 防止 LDR 区域整体染上一层"奶白"提亮。strength 0.6 让 9-tap 模糊不超过场景亮度。
        let bp = BloomParams {
            bright_threshold: self.calibration.bloom_bright_threshold,
            bloom_strength: self.calibration.bloom_prefilter_strength,
            inv_size_x: 1.0 / self.hdr_w as f32,
            inv_size_y: 1.0 / self.hdr_h as f32,
        };
        queue.write_buffer(&self.bloom_uniform, 0, bytemuck::bytes_of(&bp));

        // downsample uniforms（每级用上一级尺寸）
        for (i, buf) in self.downsample_uniforms.iter().enumerate() {
            let src = &self.bloom_targets[i];
            let dp = DownParams {
                inv_src_size: [1.0 / src.width as f32, 1.0 / src.height as f32],
                _pad: [0.0; 2],
            };
            queue.write_buffer(buf, 0, bytemuck::bytes_of(&dp));
        }

        // lum uniforms
        for (i, buf) in self.lum_uniforms.iter().enumerate() {
            let (sw, sh) = if i == 0 {
                (self.hdr_w as f32, self.hdr_h as f32)
            } else {
                (
                    self.lum_targets[i - 1].width as f32,
                    self.lum_targets[i - 1].height as f32,
                )
            };
            let lp = LumParams {
                inv_src_size: [1.0 / sw, 1.0 / sh],
                _pad: [0.0; 2],
            };
            queue.write_buffer(buf, 0, bytemuck::bytes_of(&lp));
        }

        // restore params（摄影标准 18% 中灰 + 微弱 bloom 叠加 + 轻 vignette）。
        // 之前 middle_grey=0.5 把"目标显示亮度"定在 50% gray，配合自动曝光把当前
        // 平均场景值（典型 ~0.2）×2.5 倍后 ACES 仍偏亮 → 整体过曝。
        // 0.18 是行业标准（sRGB ~0.46），与 vanilla `MiddleGray` 同 ballpark。
        let rp = RestoreParams {
            middle_grey: self.calibration.middle_grey,
            bloom_strength: self.calibration.final_bloom_strength,
            vignette_strength: self.calibration.vignette_strength,
            srgb_target: self.srgb_target_flag,
            exposure_min: self.calibration.exposure_min,
            exposure_max: self.calibration.exposure_max,
            aces_input_scale: self.calibration.aces_input_scale,
            saturation: self.calibration.saturation,
            debug_view: self.debug_view.as_shader_value(),
            bloom_debug_gain: self.calibration.bloom_debug_gain,
            _pad0: [0.0; 2],
            center_uv: [0.5, 0.5],
            _pad: [0.0; 2],
        };
        queue.write_buffer(&self.restore_uniform, 0, bytemuck::bytes_of(&rp));
    }

    /// 在已开 encoder 中提交完整链。
    ///
    /// `final_target` = swap-chain 当前帧 view。
    pub fn render(&self, encoder: &mut wgpu::CommandEncoder, final_target: &wgpu::TextureView) {
        // ── bloom: lvl0 = bright pass; lvl1..3 = downsample ──
        for lvl in 0..BLOOM_LEVELS {
            let pipeline = if lvl == 0 {
                &self.bloom_pipeline
            } else {
                &self.downsample_pipeline
            };
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(if lvl == 0 {
                    "bloom_bright"
                } else {
                    "bloom_downsample"
                }),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.bloom_targets[lvl].view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            rp.set_pipeline(pipeline);
            rp.set_bind_group(0, &self.bloom_bind_groups[lvl], &[]);
            rp.draw(0..3, 0..1);
        }

        // ── luminance reduction ──
        for lvl in 0..LUM_LEVELS {
            let pipeline = if lvl == 0 {
                &self.lum_log_pipeline
            } else {
                &self.lum_avg_pipeline
            };
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(if lvl == 0 { "lum_log" } else { "lum_avg" }),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.lum_targets[lvl].view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            rp.set_pipeline(pipeline);
            rp.set_bind_group(0, &self.lum_bind_groups[lvl], &[]);
            rp.draw(0..3, 0..1);
        }

        // ── restorescene final composite ──
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("restorescene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: final_target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            rp.set_pipeline(&self.restore_pipeline);
            rp.set_bind_group(0, &self.restore_bind_group, &[]);
            rp.draw(0..3, 0..1);
        }
    }
}

impl super::Pass for PostProcessChain {
    fn name(&self) -> &'static str {
        "post_process_chain"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_strings_nonempty() {
        assert!(!BLOOM_BRIGHT_WGSL.is_empty());
        assert!(!DOWNSAMPLE_WGSL.is_empty());
        assert!(!LUM_LOG_WGSL.is_empty());
        assert!(LUM_AVG_WGSL.contains("vs_main"));
        assert!(RESTORESCENE_LIVE_WGSL.contains("aces_tonemap"));
        assert!(RESTORESCENE_LIVE_WGSL.contains("srgb_target"));
        assert!(RESTORESCENE_LIVE_WGSL.contains("debug_view"));
        assert!(RESTORESCENE_LIVE_WGSL.contains("bloom_debug_gain"));
    }

    #[test]
    fn uniform_sizes_align_16() {
        // wgpu uniform buffers must be at least 16 byte aligned in size.
        assert!(std::mem::size_of::<BloomParams>() % 16 == 0);
        assert!(std::mem::size_of::<DownParams>() % 16 == 0);
        assert!(std::mem::size_of::<LumParams>() % 16 == 0);
        assert!(std::mem::size_of::<RestoreParams>() % 16 == 0);
    }

    #[test]
    fn phase9_calibration_is_conservative() {
        let calibration = PostProcessCalibration::phase9_high();
        assert!((calibration.middle_grey - 0.18).abs() < f32::EPSILON);
        assert!(calibration.exposure_min >= 0.5);
        assert!(calibration.exposure_max <= 1.5);
        assert!(calibration.final_bloom_strength <= 0.08);
        assert!(calibration.saturation <= 1.0);
    }

    #[test]
    fn postprocess_debug_view_cycles_through_phase9_views() {
        let mut view = PostProcessDebugView::Final;
        let mut seen = Vec::new();
        for _ in 0..PostProcessDebugView::ALL.len() {
            seen.push(view);
            view = view.next();
        }
        assert_eq!(seen, PostProcessDebugView::ALL);
        assert_eq!(view, PostProcessDebugView::Final);
        assert_eq!(PostProcessDebugView::BloomOnly.name(), "bloom_only");
    }

    #[test]
    fn restore_shader_validates() {
        let module = naga::front::wgsl::parse_str(RESTORESCENE_LIVE_WGSL)
            .expect("restore shader should parse");
        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        );
        validator
            .validate(&module)
            .expect("restore shader should validate");
    }
}
