//! Phase 3.12.5 — `PdxMeshPass`：把 vanilla 3D 建筑 mesh 接到屏幕上。
//!
//! 替换 `shader_rt::FLAT_DIFFUSE_WGSL` fallback 与旧的程序化 `buildings.wgsl`
//! billboard 矩形。本 pass 用 [`hoi4_render::translations::pdxmesh.wgsl`] 同款
//! 风格但加了**每实例**位置/缩放/色调（vanilla 用 `vModelMatrix` per-draw uniform，
//! 我们换成 GPU instancing 显著降低 1k+ 建筑的 draw call 开销）。
//!
//! ## 当前覆盖范围
//!
//! - 民用工厂 → `gfx/models/buildings/civ_factory.mesh`（绿色 tint）
//! - 军事工厂 → `gfx/models/buildings/factory.mesh`（红色 tint）
//! - 船坞      → `gfx/models/buildings/dock_01.mesh`（蓝色 tint）
//!
//! 三种 mesh 分别一次 indexed draw，共享 pipeline + frame uniform + 阴影 +
//! 立方体 reflection；各自一份 vertex / index / instance buffer + per-mesh 纹理
//! bind group。
//!
//! ## 与翻译文件 [`hoi4_render::translations::pdxmesh.wgsl`] 的关系
//!
//! 翻译文件保持 1:1 vanilla（`vModelMatrix` per-draw uniform 风格，给后续 portrait
//! / 单 mesh ambient_object 用）。本 pass 用一个**改写的 instanced 变体**，按
//! Phase 3.12 的"全部 3D pass 都用 GlobalFrameUniform"原则统一接到
//! `@group(0) @binding(0)`，per-instance pos / scale / tint 走 vertex buffer
//! `step_mode = Instance` 的 location 4-6。
//!
//! ## bind group 布局
//!
//! - `@group(0)` 共享：`GlobalFrameUniform` + `MeshMaterial`
//! - `@group(1)` 共享：shadow_map + shadow_sampler + env_cube + env_sampler
//! - `@group(2)` per-mesh：diffuse + normal + spec_gloss + emissive + sampler
//!
//! 当某 mesh 没有 normal / spec / emissive 贴图时，对应位置绑 1×1 fallback
//! （`feature_flags` 同步关掉），fragment 无分支损耗。
//!
//! ## EnvironmentMap cubemap
//!
//! vanilla HOI4 安装目录里**没有** `gfx/cubemaps/EnvironmentMap.dds` —— 这张图
//! 由 vanilla 引擎在运行时根据 `gfx/loadingscreens/sky_*.dds` 烘焙或者干脆走
//! `posteffect_volumes` 的内置 LUT。Phase 3.12.5 用 1×1×6 mid-grey
//! `(0.45, 0.5, 0.6, 1.0)` 立方体作 fallback；3.12.11 sky pass 接入后会替换为
//! sky_*.dds 的 6 个面。
//!
//! ## 性能
//!
//! 1936-01-01 启动时 vanilla 三类工厂总数约 ~1500-1800 个建筑实例（13K 省份 ÷
//! `generate_buildings` 过滤水省 + 仅工业州），分 3 个 indexed draw call。每个
//! mesh ~500-2000 顶点 + ~3K 索引。GPU 端单帧 < 1 ms。

#![allow(dead_code)]

use hoi4_assets::{AssetDb, AssetError, DdsImage, FsAssetDb, GfxIndex, PdxMesh as PdxMeshAsset};
use hoi4_paths::PathConfig;
use hoi4_render::buildings::{
    BuildingInstance, BUILDING_KIND_AIR_BASE, BUILDING_KIND_ANTI_AIR, BUILDING_KIND_BUNKER,
    BUILDING_KIND_COASTAL_BUNKER, BUILDING_KIND_DOCKYARD, BUILDING_KIND_FUEL_SILO,
    BUILDING_KIND_INDUSTRIAL, BUILDING_KIND_MILITARY, BUILDING_KIND_NAVAL_BASE,
    BUILDING_KIND_NUCLEAR_REACTOR, BUILDING_KIND_RADAR, BUILDING_KIND_REFINERY,
    BUILDING_KIND_ROCKET_SITE,
};
use wgpu::util::DeviceExt;

use crate::passes::HDR_FORMAT;
use crate::vanilla_targets::VanillaRuntimeTargets;

const PDXMESH_OBJECT_KIND_COUNT: usize = 13;
const BASE_INSTANCE_SCALE: f32 = 0.022;
const MESH_LOD_WORLD_SCALE: f32 = 0.02;
const DEFAULT_LOD_DISTANCES: [f32; 3] = [14.0, 28.0, 10_000.0];

// ─── 公共材质 uniform（与 wgsl `MeshMaterial` 字面对齐）─────────────────────

/// 与 [`PDXMESH_INSTANCED_WGSL`] `struct MeshMaterial` 字面对齐（80 bytes，
/// std140）。
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshMaterial {
    /// (r, g, b, a) — 全 mesh 的色调乘子。建筑用国家色 / 类型色，留 1.0 这里，
    /// 实际 tint 由 instance 通道传。
    pub diffuse_tint: [f32; 4],
    /// (specular_intensity, glossiness, alpha_cutoff, emissive)
    pub pbr_packed: [f32; 4],
    /// (use_normal_map, use_spec_gloss, use_emissive, snow_factor)
    pub feature_flags: [f32; 4],
    /// 顶点 UV 动画速度 (xy=diffuse, zw=normal)，建筑不动 = 0
    pub animate_uv: [f32; 4],
    /// rim 颜色与强度（rgb + intensity scalar）
    pub rim_color: [f32; 4],
    pub phase8_controls: [f32; 4],
}

impl Default for MeshMaterial {
    fn default() -> Self {
        Self {
            diffuse_tint: [1.0, 1.0, 1.0, 1.0],
            // specular=0.4 / glossiness=0.55 / alpha_cutoff=0.3 / emissive=0.6
            pbr_packed: [0.4, 0.55, 0.3, 0.6],
            // normal/spec use flat fallback textures when a mesh omits them.
            feature_flags: [1.0, 1.0, 0.0, 0.70],
            animate_uv: [0.0; 4],
            // 微弱 rim 蓝光衬建筑轮廓
            rim_color: [0.5, 0.6, 0.8, 0.2],
            phase8_controls: [1.0, 1.0, 1.0, 0.0],
        }
    }
}

// ─── 每实例顶点属性 ────────────────────────────────────────────────────────

/// 单个 mesh 实例的 GPU 顶点属性。32 bytes：pos (12) + scale (4) + tint (4 BGRA)
/// + rotation_y (4) + pad (8)。布局必须与 [`PDXMESH_INSTANCED_WGSL`] 的
/// `InstanceData` 字段顺序一致。
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PdxMeshInstance {
    pub pos: [f32; 3],
    pub scale: f32,
    pub tint: [u8; 4],
    /// 绕 Y 轴自旋（弧度），让相邻同类型建筑不全部一个朝向。
    pub rotation_y: f32,
    pub _pad: [f32; 2],
}

const _: () = assert!(std::mem::size_of::<PdxMeshInstance>() == 32);

/// `BuildingInstance` (16 bytes) -> `PdxMeshInstance` (32 bytes).
///
/// The output vector is indexed by `BUILDING_KIND_*`. Each object kind maps to
/// a vanilla pdxmesh slot, while unknown kinds are ignored.
pub fn split_buildings_by_kind(buildings: &[BuildingInstance]) -> Vec<Vec<PdxMeshInstance>> {
    let mut split: Vec<Vec<PdxMeshInstance>> =
        (0..PDXMESH_OBJECT_KIND_COUNT).map(|_| Vec::new()).collect();
    for (i, b) in buildings.iter().enumerate() {
        // 简单 hash 抖动：i * golden ratio 取小数 → [0, 2π)
        let h = ((i as u32).wrapping_mul(2654435761)) as f32 / u32::MAX as f32;
        let rotation_y = h * std::f32::consts::TAU;
        let kind = b.kind as i32;
        if kind < 0 || kind as usize >= split.len() {
            continue;
        }
        let inst = PdxMeshInstance {
            pos: b.pos,
            scale: BASE_INSTANCE_SCALE * scale_for_kind(b.kind),
            tint: tint_for_kind(b.kind),
            rotation_y,
            _pad: [0.0; 2],
        };
        split[kind as usize].push(inst);
    }
    split
}

fn tint_for_kind(kind: f32) -> [u8; 4] {
    match kind as i32 {
        x if x == BUILDING_KIND_INDUSTRIAL as i32 => [190, 218, 178, 255],
        x if x == BUILDING_KIND_MILITARY as i32 => [222, 178, 172, 255],
        x if x == BUILDING_KIND_DOCKYARD as i32 => [176, 198, 222, 255],
        x if x == BUILDING_KIND_AIR_BASE as i32 => [202, 205, 184, 255],
        x if x == BUILDING_KIND_NAVAL_BASE as i32 => [170, 198, 210, 255],
        x if x == BUILDING_KIND_RADAR as i32 => [190, 214, 226, 255],
        x if x == BUILDING_KIND_ANTI_AIR as i32 => [210, 202, 180, 255],
        x if x == BUILDING_KIND_BUNKER as i32 => [185, 185, 168, 255],
        x if x == BUILDING_KIND_COASTAL_BUNKER as i32 => [182, 194, 178, 255],
        x if x == BUILDING_KIND_REFINERY as i32 => [205, 190, 170, 255],
        x if x == BUILDING_KIND_FUEL_SILO as i32 => [190, 188, 172, 255],
        x if x == BUILDING_KIND_NUCLEAR_REACTOR as i32 => [188, 210, 190, 255],
        x if x == BUILDING_KIND_ROCKET_SITE as i32 => [216, 200, 180, 255],
        _ => [255, 255, 255, 255],
    }
}

fn scale_for_kind(kind: f32) -> f32 {
    match kind as i32 {
        x if x == BUILDING_KIND_NAVAL_BASE as i32 => 1.12,
        x if x == BUILDING_KIND_AIR_BASE as i32 => 1.08,
        x if x == BUILDING_KIND_RADAR as i32 => 1.16,
        x if x == BUILDING_KIND_ANTI_AIR as i32 => 0.95,
        x if x == BUILDING_KIND_BUNKER as i32 => 0.90,
        x if x == BUILDING_KIND_COASTAL_BUNKER as i32 => 0.90,
        x if x == BUILDING_KIND_ROCKET_SITE as i32 => 1.22,
        x if x == BUILDING_KIND_NUCLEAR_REACTOR as i32 => 1.15,
        _ => 1.0,
    }
}

// ─── 单个 mesh 类型的 GPU 资源 ────────────────────────────────────────────

#[derive(Debug, Clone)]
struct MeshTypeSpec {
    kind: u8,
    label: &'static str,
    names: &'static [&'static str],
    fallback_mesh: &'static str,
    fallback_diffuse: &'static str,
    fallback_normal: &'static str,
    fallback_specular: &'static str,
}

#[derive(Debug, Clone)]
struct ResolvedMeshTypeSpec {
    kind: u8,
    label: &'static str,
    mesh_path: String,
    fallback_diffuse: String,
    fallback_normal: String,
    fallback_specular: String,
}

struct MeshLodResources {
    /// vertex buffer（pos+normal+uv，stride 48 bytes）
    vertex_buffer: wgpu::Buffer,
    /// index buffer（u32）
    index_buffer: wgpu::Buffer,
    index_count: u32,
    /// 每 LOD instance buffer（pos+scale+tint+rotation_y+pad，stride 32 bytes）
    instance_buffer: wgpu::Buffer,
    instance_count: u32,
    instance_capacity: u32,
}

struct MeshTypeResources {
    kind: u8,
    label: &'static str,
    /// Highest-to-lowest detail geometry.
    lods: Vec<MeshLodResources>,
    /// Source instance list split by object kind. Per-frame camera LOD upload
    /// buckets these into `lods`.
    all_instances: Vec<PdxMeshInstance>,
    lod_distances: Vec<f32>,
    /// per-mesh 纹理 bind group（@group(2)）
    bind_group_2: wgpu::BindGroup,
    /// 是否成功加载（mesh + 至少 diffuse）
    loaded: bool,
    source_mesh: String,
}

// ─── PdxMeshPass ──────────────────────────────────────────────────────────

pub struct PdxMeshPass {
    pipeline: wgpu::RenderPipeline,
    /// `GlobalFrameUniform` + `MeshMaterial` — 共享
    bind_group_0: wgpu::BindGroup,
    /// shadow + envmap — 共享
    bind_group_1: wgpu::BindGroup,
    bgl_1: wgpu::BindGroupLayout,
    env_sampler: wgpu::Sampler,
    /// 每类地图物件各一份资源
    types: Vec<MeshTypeResources>,
    /// `MeshMaterial` GPU buffer（让 main.rs 可调 `update_material` 覆盖默认）
    material_buffer: wgpu::Buffer,
    /// 至少 1 类 mesh + diffuse 都加载成功；否则 main.rs 应回退到旧 buildings_pipeline
    pub any_loaded: bool,
    lod_uploaded: bool,
    lod_last_cam_pos: [f32; 3],
    lod_bias: u8,
}

/// 单个 mesh 顶点的 GPU 表示（pos + normal + uv，32 字节包含 pad）。
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct MeshVertex {
    pos: [f32; 3],
    _pad0: f32,
    normal: [f32; 3],
    _pad1: f32,
    uv: [f32; 2],
    _pad2: [f32; 2],
}

const _: () = assert!(std::mem::size_of::<MeshVertex>() == 48);

impl PdxMeshPass {
    /// 构造。
    ///
    /// `path_cfg` 用于按 mod 链找 vanilla mesh 与贴图。
    /// `global_uniform_buffer` / `shadow_map_view` / `shadow_compare_sampler` 来自
    /// 主渲染管线（参见 main.rs init_render）。
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path_cfg: &PathConfig,
        global_uniform_buffer: &wgpu::Buffer,
        depth_format: wgpu::TextureFormat,
        shadow_map_view: &wgpu::TextureView,
        shadow_compare_sampler: &wgpu::Sampler,
        runtime_targets: &VanillaRuntimeTargets,
    ) -> Self {
        // Phase 3.12.14: Use ShaderRegistry to validate shader name availability,
        // but still use PDXMESH_INSTANCED_WGSL for the pipeline because the
        // translated pdxmesh.wgsl has a different vertex/fragment interface
        // (requires full 3D pass infrastructure not yet in place).
        let registry = hoi4_render::shader_rt::ShaderRegistry::new();
        let _entry = registry.resolve("pdxmesh");
        let composed = hoi4_render::shader_rt::compose_shader(PDXMESH_INSTANCED_WGSL, true, true);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("pdxmesh_instanced_shader"),
            source: wgpu::ShaderSource::Wgsl(composed.into()),
        });

        // ── material uniform buffer ──────────────────────────────────────
        let mat = MeshMaterial::default();
        let material_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("pdxmesh_material"),
            contents: bytemuck::bytes_of(&mat),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // ── 1×1×6 mid-grey 立方体 fallback ──────────────────────────────
        let env_cube_view = create_grey_cubemap(device, queue);
        let env_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("pdxmesh_env_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // ── BGL 0：frame + material ──────────────────────────────────────
        let bgl0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pdxmesh_bgl0"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let bind_group_0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pdxmesh_bg0"),
            layout: &bgl0,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: global_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: material_buffer.as_entire_binding(),
                },
            ],
        });

        // ── BGL 1：shadow + envmap + shared vanilla runtime targets ─────
        let bgl1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pdxmesh_bgl1"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
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
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 9,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 10,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 11,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let bind_group_1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pdxmesh_bg1"),
            layout: &bgl1,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(shadow_map_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(shadow_compare_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&env_cube_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&env_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(
                        &runtime_targets.gradient_border.ch1.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(
                        &runtime_targets.gradient_border.ch2.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(
                        &runtime_targets.gradient_border.ch3.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(
                        &runtime_targets.province_secondary_color.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&runtime_targets.fow.view),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::TextureView(&runtime_targets.light_data.view),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::TextureView(&runtime_targets.light_index.view),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::TextureView(&runtime_targets.mud_snow.view),
                },
            ],
        });

        // ── BGL 2：per-mesh 纹理 ─────────────────────────────────────────
        let bgl2 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pdxmesh_bgl2"),
            entries: &[
                // 0..3 = diffuse / normal / spec_gloss / emissive
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
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
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
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let mat_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("pdxmesh_mat_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            ..Default::default()
        });

        // ── pipeline layout + pipeline ───────────────────────────────────
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pdxmesh_pl"),
            bind_group_layouts: &[&bgl0, &bgl1, &bgl2],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pdxmesh_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    // Vertex-rate buffer (mesh geometry)
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<MeshVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[
                            wgpu::VertexAttribute {
                                offset: 0,
                                shader_location: 0,
                                format: wgpu::VertexFormat::Float32x3,
                            },
                            wgpu::VertexAttribute {
                                offset: 16,
                                shader_location: 1,
                                format: wgpu::VertexFormat::Float32x3,
                            },
                            wgpu::VertexAttribute {
                                offset: 32,
                                shader_location: 2,
                                format: wgpu::VertexFormat::Float32x2,
                            },
                        ],
                    },
                    // Instance-rate buffer (pos + scale + tint + rotation_y)
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<PdxMeshInstance>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[
                            wgpu::VertexAttribute {
                                offset: 0,
                                shader_location: 4,
                                format: wgpu::VertexFormat::Float32x3,
                            },
                            wgpu::VertexAttribute {
                                offset: 12,
                                shader_location: 5,
                                format: wgpu::VertexFormat::Float32,
                            },
                            wgpu::VertexAttribute {
                                offset: 16,
                                shader_location: 6,
                                format: wgpu::VertexFormat::Unorm8x4,
                            },
                            wgpu::VertexAttribute {
                                offset: 20,
                                shader_location: 7,
                                format: wgpu::VertexFormat::Float32,
                            },
                        ],
                    },
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        // ── 加载 vanilla object meshes ───────────────────────────────────
        let db = FsAssetDb::new(path_cfg.clone());
        let gfx_index = match load_buildings_gfx_index(&db) {
            Some(index) => {
                println!(
                    "[pdxmesh] loaded {} .gfx files ({} meshes, {} entities)",
                    index.files_loaded,
                    index.meshes.len(),
                    index.entities.len()
                );
                Some(index)
            }
            None => {
                eprintln!("[pdxmesh] buildings.gfx unavailable; using hardcoded mesh fallbacks");
                None
            }
        };
        let defs = resolve_mesh_specs(gfx_index.as_ref());

        let mut types: Vec<MeshTypeResources> = Vec::with_capacity(defs.len());
        let mut any_loaded = false;
        for (i, spec) in defs.iter().enumerate() {
            let res = load_mesh_type(device, queue, &db, spec, &bgl2, &mat_sampler);
            if res.loaded {
                any_loaded = true;
                println!(
                    "[pdxmesh] type {} '{}' loaded from {} ({} lods, {} idx)",
                    i,
                    spec.label,
                    spec.mesh_path,
                    res.lods.len(),
                    res.lods.iter().map(|lod| lod.index_count).sum::<u32>()
                );
            } else {
                eprintln!(
                    "[pdxmesh] type {} '{}' ({}) FAILED - empty buffers",
                    i, spec.label, spec.mesh_path
                );
            }
            types.push(res);
        }

        Self {
            pipeline,
            bind_group_0,
            bind_group_1,
            bgl_1: bgl1,
            env_sampler: env_sampler.clone(),
            types,
            material_buffer,
            any_loaded,
            lod_uploaded: false,
            lod_last_cam_pos: [f32::NAN; 3],
            lod_bias: 0,
        }
    }

    /// 设置 / 更新建筑实例数据。按 `kind` 拆分到地图物件 instance buffer。
    pub fn set_buildings(&mut self, _device: &wgpu::Device, buildings: &[BuildingInstance]) {
        let split = split_buildings_by_kind(buildings);
        for ty in &mut self.types {
            ty.all_instances = split
                .get(ty.kind as usize)
                .cloned()
                .unwrap_or_else(Vec::new);
            for lod in &mut ty.lods {
                lod.instance_count = 0;
            }
        }
        self.lod_uploaded = false;
    }

    pub fn upload_lod_instances(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        cam_pos: [f32; 3],
    ) {
        for ty in &mut self.types {
            upload_type_lod_instances(device, queue, ty, cam_pos, self.lod_bias);
        }
        self.lod_uploaded = true;
        self.lod_last_cam_pos = cam_pos;
    }

    pub fn set_lod_bias(&mut self, lod_bias: u8) {
        let lod_bias = lod_bias.min(2);
        if self.lod_bias != lod_bias {
            self.lod_bias = lod_bias;
            self.lod_uploaded = false;
        }
    }

    pub fn ensure_lod_uploaded(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        cam_pos: [f32; 3],
    ) {
        let last = self.lod_last_cam_pos;
        let moved_sq = (cam_pos[0] - last[0]).powi(2)
            + (cam_pos[1] - last[1]).powi(2)
            + (cam_pos[2] - last[2]).powi(2);
        if !self.lod_uploaded || !moved_sq.is_finite() || moved_sq > 4.0 {
            self.upload_lod_instances(device, queue, cam_pos);
        }
    }

    /// 每帧调（材质若需要动态改写则在这里）。当前 default 一次写死即可。
    pub fn prepare(&self, _queue: &wgpu::Queue) {
        // material 当前不变；保留 prepare 形如其他 pass 的接口
    }

    /// Replace environment cubemap — requires passing shadow resources again.
    pub fn update_phase8_controls(
        &self,
        queue: &wgpu::Queue,
        opacity: f32,
        scale: f32,
        brightness: f32,
        snow_factor: f32,
    ) {
        let material = MeshMaterial {
            feature_flags: [1.0, 1.0, 0.0, snow_factor.clamp(0.0, 1.0)],
            phase8_controls: [
                opacity.clamp(0.0, 1.0),
                scale.clamp(0.25, 1.5),
                brightness.clamp(0.25, 2.0),
                0.0,
            ],
            ..MeshMaterial::default()
        };
        queue.write_buffer(&self.material_buffer, 0, bytemuck::bytes_of(&material));
    }

    pub fn set_env_cubemap_with_shadow(
        &mut self,
        device: &wgpu::Device,
        sky_view: &wgpu::TextureView,
        shadow_map_view: &wgpu::TextureView,
        shadow_compare_sampler: &wgpu::Sampler,
        runtime_targets: &VanillaRuntimeTargets,
    ) {
        self.bind_group_1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pdxmesh_bg1_sky"),
            layout: &self.bgl_1,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(shadow_map_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(shadow_compare_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(sky_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.env_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(
                        &runtime_targets.gradient_border.ch1.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(
                        &runtime_targets.gradient_border.ch2.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(
                        &runtime_targets.gradient_border.ch3.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(
                        &runtime_targets.province_secondary_color.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&runtime_targets.fow.view),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::TextureView(&runtime_targets.light_data.view),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::TextureView(&runtime_targets.light_index.view),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::TextureView(&runtime_targets.mud_snow.view),
                },
            ],
        });
    }

    /// 在已开 render pass 里画。caller 负责 `set_pipeline` / `set_bind_group(0)`
    /// 等绑过 — 不，本方法**完整**绑全部。
    pub fn render<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if !self.any_loaded {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group_0, &[]);
        pass.set_bind_group(1, &self.bind_group_1, &[]);
        for ty in &self.types {
            if !ty.loaded {
                continue;
            }
            pass.set_bind_group(2, &ty.bind_group_2, &[]);
            for lod in &ty.lods {
                if lod.instance_count == 0 || lod.index_count == 0 {
                    continue;
                }
                pass.set_vertex_buffer(0, lod.vertex_buffer.slice(..));
                pass.set_vertex_buffer(1, lod.instance_buffer.slice(..));
                pass.set_index_buffer(lod.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..lod.index_count, 0, 0..lod.instance_count);
            }
        }
    }

    /// 总实例数（所有对象类型之和）— 用于调试 banner。
    pub fn total_instances(&self) -> u32 {
        self.types
            .iter()
            .map(|t| t.all_instances.len() as u32)
            .sum()
    }

    pub fn mesh_type_count(&self) -> usize {
        self.types.len()
    }

    pub fn loaded_draw_count(&self) -> u32 {
        self.types
            .iter()
            .filter(|t| t.loaded)
            .flat_map(|t| t.lods.iter())
            .filter(|lod| lod.instance_count > 0 && lod.index_count > 0)
            .count() as u32
    }
}

// ─── 私有 helpers ─────────────────────────────────────────────────────────

const MESH_TYPE_SPECS: &[MeshTypeSpec] = &[
    MeshTypeSpec {
        kind: BUILDING_KIND_INDUSTRIAL,
        label: "industrial",
        names: &["building_industrial_complex"],
        fallback_mesh: "gfx/models/buildings/civ_factory.mesh",
        fallback_diffuse: "gfx/models/buildings/factory_d.dds",
        fallback_normal: "gfx/models/buildings/factory_n.dds",
        fallback_specular: "gfx/models/buildings/factory_s.dds",
    },
    MeshTypeSpec {
        kind: BUILDING_KIND_MILITARY,
        label: "military",
        names: &["building_arms_factory"],
        fallback_mesh: "gfx/models/buildings/factory.mesh",
        fallback_diffuse: "gfx/models/buildings/factory_d.dds",
        fallback_normal: "gfx/models/buildings/factory_n.dds",
        fallback_specular: "gfx/models/buildings/factory_s.dds",
    },
    MeshTypeSpec {
        kind: BUILDING_KIND_DOCKYARD,
        label: "dockyard",
        names: &[
            "building_dockyard_1",
            "building_dockyard_2",
            "building_dockyard_3",
        ],
        fallback_mesh: "gfx/models/buildings/dock_01.mesh",
        fallback_diffuse: "gfx/models/buildings/dock_diffuse.dds",
        fallback_normal: "gfx/models/buildings/dock_normal.dds",
        fallback_specular: "gfx/models/buildings/dock_specular.dds",
    },
    MeshTypeSpec {
        kind: BUILDING_KIND_AIR_BASE,
        label: "air_base",
        names: &["building_air_base"],
        fallback_mesh: "gfx/models/buildings/hangar_flight.mesh",
        fallback_diffuse: "gfx/models/buildings/hangar_flight_diffuse.dds",
        fallback_normal: "gfx/models/buildings/hangar_flight_normal.dds",
        fallback_specular: "gfx/models/buildings/hangar_flight_specular.dds",
    },
    MeshTypeSpec {
        kind: BUILDING_KIND_NAVAL_BASE,
        label: "naval_base",
        names: &[
            "building_naval_base_1",
            "building_naval_base_2",
            "building_naval_base_3",
        ],
        fallback_mesh: "gfx/models/buildings/navalbase_01.mesh",
        fallback_diffuse: "gfx/models/buildings/naval_diffuse.dds",
        fallback_normal: "gfx/models/buildings/naval_normal.dds",
        fallback_specular: "gfx/models/buildings/naval_specular.dds",
    },
    MeshTypeSpec {
        kind: BUILDING_KIND_RADAR,
        label: "radar",
        names: &["building_radar_station_mesh", "building_radar_station"],
        fallback_mesh: "gfx/models/buildings/radar.mesh",
        fallback_diffuse: "gfx/models/buildings/radar_base_d.dds",
        fallback_normal: "gfx/models/buildings/radar_base_n.dds",
        fallback_specular: "gfx/models/buildings/radar_base_s.dds",
    },
    MeshTypeSpec {
        kind: BUILDING_KIND_ANTI_AIR,
        label: "anti_air",
        names: &["building_anti_air_building"],
        fallback_mesh: "gfx/models/buildings/88aagun.mesh",
        fallback_diffuse: "gfx/models/buildings/88_aagun_d.dds",
        fallback_normal: "gfx/models/buildings/88_aagun_n.dds",
        fallback_specular: "gfx/models/buildings/88_aagun_s.dds",
    },
    MeshTypeSpec {
        kind: BUILDING_KIND_BUNKER,
        label: "bunker",
        names: &["building_bunker"],
        fallback_mesh: "gfx/models/buildings/bunker.mesh",
        fallback_diffuse: "gfx/models/buildings/bunkercomplex_diffuse.dds",
        fallback_normal: "gfx/models/buildings/bunkercomplex_normal.dds",
        fallback_specular: "gfx/models/buildings/bunkercomplex_specular.dds",
    },
    MeshTypeSpec {
        kind: BUILDING_KIND_COASTAL_BUNKER,
        label: "coastal_bunker",
        names: &["building_coastal_bunker"],
        fallback_mesh: "gfx/models/buildings/navalfort_01.mesh",
        fallback_diffuse: "gfx/models/buildings/navalfort_d.dds",
        fallback_normal: "gfx/models/buildings/navalfort_d.dds",
        fallback_specular: "gfx/models/buildings/navalfort_specular.dds",
    },
    MeshTypeSpec {
        kind: BUILDING_KIND_REFINERY,
        label: "refinery",
        names: &["building_oil_refinery"],
        fallback_mesh: "gfx/models/buildings/oil_refinery.mesh",
        fallback_diffuse: "gfx/models/buildings/oil_refinery_diffuse.dds",
        fallback_normal: "gfx/models/buildings/oil_refinery_normal.dds",
        fallback_specular: "gfx/models/buildings/oil_refinery_specular.dds",
    },
    MeshTypeSpec {
        kind: BUILDING_KIND_FUEL_SILO,
        label: "fuel_silo",
        names: &["building_fuel_silo"],
        fallback_mesh: "gfx/models/buildings/fuel_silo.mesh",
        fallback_diffuse: "gfx/models/buildings/fuel_silo_diffuse.dds",
        fallback_normal: "gfx/models/buildings/fuel_silo_normal.dds",
        fallback_specular: "gfx/models/buildings/fuel_silo_specular.dds",
    },
    MeshTypeSpec {
        kind: BUILDING_KIND_NUCLEAR_REACTOR,
        label: "nuclear_reactor",
        names: &[
            "building_nuclear_reactor",
            "building_commercial_nuclear_reactor",
        ],
        fallback_mesh: "gfx/models/buildings/nuclear_reactor.mesh",
        fallback_diffuse: "gfx/models/buildings/nuclearreactor_diffuse.dds",
        fallback_normal: "gfx/models/buildings/nuclearreactor_normal.dds",
        fallback_specular: "gfx/models/buildings/nuclearreactor_specular.dds",
    },
    MeshTypeSpec {
        kind: BUILDING_KIND_ROCKET_SITE,
        label: "rocket_site",
        names: &["building_rocket_site"],
        fallback_mesh: "gfx/models/buildings/rocket_site.mesh",
        fallback_diffuse: "gfx/models/buildings/facility_diffuse.dds",
        fallback_normal: "gfx/models/buildings/facility_normal.dds",
        fallback_specular: "gfx/models/buildings/facility_specular.dds",
    },
];

fn load_buildings_gfx_index(db: &FsAssetDb) -> Option<GfxIndex> {
    let mut index = GfxIndex::new();
    if let Err(e) = index.load_file(db, "gfx/entities/buildings.gfx") {
        eprintln!("[pdxmesh] gfx/entities/buildings.gfx load failed: {}", e);
        return None;
    }
    Some(index)
}

fn resolve_mesh_specs(index: Option<&GfxIndex>) -> Vec<ResolvedMeshTypeSpec> {
    MESH_TYPE_SPECS
        .iter()
        .map(|spec| {
            let resolved =
                index.and_then(|idx| idx.first_mesh_for_names(spec.names.iter().copied()));
            let mesh_path = resolved
                .map(|m| m.file.clone())
                .filter(|p| !p.is_empty())
                .unwrap_or_else(|| spec.fallback_mesh.to_string());
            let (fb_diff, fb_norm, fb_spec) =
                resolve_gfx_material_fallbacks(resolved, &mesh_path, spec);
            ResolvedMeshTypeSpec {
                kind: spec.kind,
                label: spec.label,
                mesh_path,
                fallback_diffuse: fb_diff,
                fallback_normal: fb_norm,
                fallback_specular: fb_spec,
            }
        })
        .collect()
}

fn resolve_gfx_material_fallbacks(
    mesh: Option<&hoi4_assets::GfxMeshDef>,
    mesh_path: &str,
    spec: &MeshTypeSpec,
) -> (String, String, String) {
    if let Some(settings) = mesh.and_then(|m| m.meshsettings.first()) {
        let diff = settings
            .texture_diffuse
            .as_ref()
            .map(|p| normalize_mesh_tex_path(p, mesh_path))
            .unwrap_or_else(|| spec.fallback_diffuse.to_string());
        let norm = settings
            .texture_normal
            .as_ref()
            .map(|p| normalize_mesh_tex_path(p, mesh_path))
            .unwrap_or_else(|| spec.fallback_normal.to_string());
        let specular = settings
            .texture_specular
            .as_ref()
            .map(|p| normalize_mesh_tex_path(p, mesh_path))
            .unwrap_or_else(|| spec.fallback_specular.to_string());
        return (diff, norm, specular);
    }
    (
        spec.fallback_diffuse.to_string(),
        spec.fallback_normal.to_string(),
        spec.fallback_specular.to_string(),
    )
}

fn load_mesh_type(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    db: &FsAssetDb,
    spec: &ResolvedMeshTypeSpec,
    bgl2: &wgpu::BindGroupLayout,
    mat_sampler: &wgpu::Sampler,
) -> MeshTypeResources {
    let mesh_path = spec.mesh_path.as_str();
    let parsed = match db.parse_or_get::<PdxMeshAsset, _>(mesh_path, |bytes| {
        PdxMeshAsset::parse(bytes).map_err(|err| AssetError::parse(mesh_path, err.to_string()))
    }) {
        Ok(mesh) => Some(mesh),
        Err(e) => {
            eprintln!("[pdxmesh] {} load failed: {}", mesh_path, e);
            None
        }
    };

    let mut lod_distances = DEFAULT_LOD_DISTANCES.to_vec();
    let (lods, mat) = match parsed {
        Some(m) if !m.meshes.is_empty() => {
            if !m.lod_distances.is_empty() {
                lod_distances.clear();
                for d in &m.lod_distances {
                    lod_distances.push((*d * MESH_LOD_WORLD_SCALE).max(1.0));
                }
                while lod_distances.len() < m.meshes.len() {
                    let next = lod_distances.last().copied().unwrap_or(28.0) * 2.0;
                    lod_distances.push(next);
                }
            }
            let first_mat = m.meshes.first().map(|sub| sub.material.clone());
            let out = create_lod_resources_from_mesh(device, &m);
            (out, first_mat)
        }
        _ => (Vec::new(), None),
    };

    // 解析 material 字符串 → 实际贴图路径（fall back to defaults）
    let resolve_path = |opt: Option<&String>, fallback: &str| -> String {
        match opt {
            Some(s) if !s.is_empty() => normalize_mesh_tex_path(s, mesh_path),
            _ => fallback.to_string(),
        }
    };
    let diffuse_path = mat
        .as_ref()
        .map(|m| resolve_path(m.diffuse.as_ref(), &spec.fallback_diffuse))
        .unwrap_or_else(|| spec.fallback_diffuse.clone());
    let normal_path = mat
        .as_ref()
        .map(|m| resolve_path(m.normal.as_ref(), &spec.fallback_normal))
        .unwrap_or_else(|| spec.fallback_normal.clone());
    let specular_path = mat
        .as_ref()
        .map(|m| resolve_path(m.specular.as_ref(), &spec.fallback_specular))
        .unwrap_or_else(|| spec.fallback_specular.clone());

    // Phase 3.12.14: Log mesh shader name and validate against registry.
    if let Some(ref m) = mat {
        let shader_name = &m.shader;
        let registry = hoi4_render::shader_rt::ShaderRegistry::new();
        if !shader_name.is_empty() {
            let stem = hoi4_render::shader_rt::extract_shader_stem(shader_name);
            if registry.has(stem) {
                eprintln!("[pdxmesh] {} shader='{}' → registry hit", mesh_path, stem);
            } else {
                eprintln!(
                    "[pdxmesh] {} shader='{}' → registry miss (using fallback)",
                    mesh_path, stem
                );
            }
        }
    }

    // 加载 diffuse / normal / spec；缺失时绑定 1x1 fallback。
    let diffuse_view = load_dds_texture(device, queue, db, &diffuse_path, true);
    let mut loaded = false;
    let diffuse_view = match diffuse_view {
        Some(v) => {
            loaded = true;
            v
        }
        None => {
            eprintln!(
                "[pdxmesh] diffuse missing for {} (tried {})",
                mesh_path, diffuse_path
            );
            create_1x1_white(device, queue)
        }
    };
    let normal_view = load_dds_texture(device, queue, db, &normal_path, false)
        .unwrap_or_else(|| create_1x1_normal(device, queue));
    let spec_view = load_dds_texture(device, queue, db, &specular_path, false)
        .unwrap_or_else(|| create_1x1_white(device, queue));
    let emissive_view = create_1x1_black(device, queue);

    let bind_group_2 = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("pdxmesh_bg2"),
        layout: bgl2,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&diffuse_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&normal_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&spec_view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&emissive_view),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::Sampler(mat_sampler),
            },
        ],
    });

    if lods.is_empty() || lods.iter().all(|lod| lod.index_count == 0) {
        loaded = false;
    }

    MeshTypeResources {
        kind: spec.kind,
        label: spec.label,
        lods,
        all_instances: Vec::new(),
        lod_distances,
        bind_group_2,
        loaded,
        source_mesh: mesh_path.to_string(),
    }
}

fn create_lod_resources_from_mesh(
    device: &wgpu::Device,
    mesh: &PdxMeshAsset,
) -> Vec<MeshLodResources> {
    let mut lod_numbers: Vec<u32> = mesh.meshes.iter().map(|sub| sub.lod).collect();
    lod_numbers.sort_unstable();
    lod_numbers.dedup();
    if lod_numbers.len() <= 1 && mesh.meshes.len() > 1 && !mesh.lod_distances.is_empty() {
        let approx_lods = mesh.lod_distances.len().min(mesh.meshes.len());
        return (0..approx_lods)
            .map(|idx| {
                let submeshes = [&mesh.meshes[idx]];
                mesh_lod_from_submeshes(device, &submeshes)
            })
            .collect();
    }
    lod_numbers
        .iter()
        .map(|lod| {
            let matching: Vec<&hoi4_assets::SubMesh> =
                mesh.meshes.iter().filter(|sub| sub.lod == *lod).collect();
            mesh_lod_from_submeshes(device, &matching)
        })
        .collect()
}

fn mesh_lod_from_submeshes(
    device: &wgpu::Device,
    submeshes: &[&hoi4_assets::SubMesh],
) -> MeshLodResources {
    let vertex_count: usize = submeshes.iter().map(|sub| sub.positions.len()).sum();
    let index_count: usize = submeshes.iter().map(|sub| sub.indices.len()).sum();
    let mut vertices = Vec::with_capacity(vertex_count);
    let mut indices = Vec::with_capacity(index_count);
    for sub in submeshes {
        let base = vertices.len() as u32;
        for i in 0..sub.positions.len() {
            let pos = sub.positions[i];
            let normal = if i < sub.normals.len() {
                sub.normals[i]
            } else {
                [0.0, 1.0, 0.0]
            };
            let uv = if i < sub.uvs.len() {
                sub.uvs[i]
            } else {
                [0.0, 0.0]
            };
            vertices.push(MeshVertex {
                pos,
                _pad0: 0.0,
                normal,
                _pad1: 0.0,
                uv,
                _pad2: [0.0; 2],
            });
        }
        indices.extend(sub.indices.iter().map(|idx| idx.saturating_add(base)));
    }
    create_lod_resources(device, vertices, &indices)
}

fn create_lod_resources(
    device: &wgpu::Device,
    vertices: Vec<MeshVertex>,
    indices: &[u32],
) -> MeshLodResources {
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("pdxmesh_verts"),
        contents: if vertices.is_empty() {
            &[0u8; 48]
        } else {
            bytemuck::cast_slice(&vertices)
        },
        usage: wgpu::BufferUsages::VERTEX,
    });
    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("pdxmesh_indices"),
        contents: if indices.is_empty() {
            &[0u8; 4]
        } else {
            bytemuck::cast_slice(indices)
        },
        usage: wgpu::BufferUsages::INDEX,
    });
    let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("pdxmesh_instances_init"),
        contents: &[0u8; std::mem::size_of::<PdxMeshInstance>()],
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    });
    MeshLodResources {
        vertex_buffer,
        index_buffer,
        index_count: indices.len() as u32,
        instance_buffer,
        instance_count: 0,
        instance_capacity: 1,
    }
}

fn upload_type_lod_instances(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    ty: &mut MeshTypeResources,
    cam_pos: [f32; 3],
    lod_bias: u8,
) {
    let num_lods = ty.lods.len();
    if num_lods == 0 {
        return;
    }
    if ty.all_instances.is_empty() {
        for lod in &mut ty.lods {
            lod.instance_count = 0;
        }
        return;
    }

    let mut buckets: Vec<Vec<PdxMeshInstance>> = (0..num_lods).map(|_| Vec::new()).collect();
    for inst in &ty.all_instances {
        let dx = inst.pos[0] - cam_pos[0];
        let dz = inst.pos[2] - cam_pos[2];
        let dist = (dx * dx + dz * dz).sqrt();
        let mut assigned = num_lods - 1;
        for (lod_idx, &threshold) in ty.lod_distances.iter().enumerate() {
            if lod_idx >= num_lods {
                break;
            }
            if dist < threshold {
                assigned = lod_idx;
                break;
            }
        }
        let biased = (assigned + lod_bias as usize).min(num_lods - 1);
        buckets[biased].push(*inst);
    }

    for (lod, bucket) in ty.lods.iter_mut().zip(buckets.iter()) {
        let count = bucket.len() as u32;
        if count == 0 {
            lod.instance_count = 0;
            continue;
        }
        if count > lod.instance_capacity {
            let new_cap = count.next_power_of_two();
            lod.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("pdxmesh_instances_grow"),
                size: (new_cap as u64) * std::mem::size_of::<PdxMeshInstance>() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            lod.instance_capacity = new_cap;
        }
        queue.write_buffer(&lod.instance_buffer, 0, bytemuck::cast_slice(bucket));
        lod.instance_count = count;
    }
}

/// 把 mesh 材质里的相对路径（可能是 `factory_d.dds` 或 `gfx/.../factory_d.dds`）
/// 归一化成 AssetDb 能开的路径。如果是裸文件名就拼 mesh 同目录。
fn normalize_mesh_tex_path(raw: &str, mesh_path: &str) -> String {
    // 已经是带目录的相对路径
    if raw.contains('/') || raw.contains('\\') {
        return raw.replace('\\', "/");
    }
    // 与 mesh 同目录
    let parent = mesh_path
        .rsplit_once('/')
        .map(|(p, _)| p)
        .unwrap_or("gfx/models/buildings");
    format!("{}/{}", parent, raw)
}

fn load_dds_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    db: &FsAssetDb,
    path: &str,
    srgb: bool,
) -> Option<wgpu::TextureView> {
    let bytes = db.open(path).ok()?;
    let dds = DdsImage::parse(&bytes).ok()?;
    let format = match (dds.format, srgb) {
        (hoi4_assets::DdsFormat::Bc1, true) => wgpu::TextureFormat::Bc1RgbaUnormSrgb,
        (hoi4_assets::DdsFormat::Bc1, false) => wgpu::TextureFormat::Bc1RgbaUnorm,
        (hoi4_assets::DdsFormat::Bc3, true) => wgpu::TextureFormat::Bc3RgbaUnormSrgb,
        (hoi4_assets::DdsFormat::Bc3, false) => wgpu::TextureFormat::Bc3RgbaUnorm,
        (hoi4_assets::DdsFormat::Bc5, _) => wgpu::TextureFormat::Bc5RgUnorm,
        (hoi4_assets::DdsFormat::Bgra8, true) => wgpu::TextureFormat::Bgra8UnormSrgb,
        (hoi4_assets::DdsFormat::Bgra8, false) => wgpu::TextureFormat::Bgra8Unorm,
        (hoi4_assets::DdsFormat::Bgr555, _) | (hoi4_assets::DdsFormat::Unknown(_), _) => {
            eprintln!("[pdxmesh] {} unknown DDS format — skipping", path);
            return None;
        }
    };

    // BC 压缩格式跳过 < 4×4 的尾部 mip。
    // 非 2 次幂纹理（如 vanilla 2816×1024）的 mip chain 会出现宽度为 22/11 等
    // 非 4 倍数；wgpu 验证要求 BC copy width 必须是块宽（4）的倍数，否则
    // Queue::write_texture 会 panic。所以同时检查 `% 4 == 0`。
    let valid_mips = match dds.format {
        hoi4_assets::DdsFormat::Bc1 | hoi4_assets::DdsFormat::Bc3 | hoi4_assets::DdsFormat::Bc5 => {
            dds.mips
                .iter()
                .take_while(|m| {
                    m.width >= 4 && m.height >= 4 && m.width % 4 == 0 && m.height % 4 == 0
                })
                .count() as u32
        }
        _ => dds.mips.len() as u32,
    }
    .max(1);

    let sample_type = if format == wgpu::TextureFormat::Bc5RgUnorm {
        wgpu::TextureFormat::Rgba8Unorm
    } else {
        format
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("pdxmesh_tex"),
        size: wgpu::Extent3d {
            width: dds.width,
            height: dds.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: valid_mips,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: sample_type,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    if format == wgpu::TextureFormat::Bc5RgUnorm {
        let mip = &dds.mips[0];
        let expanded = decode_bc5_to_rgba8(
            &dds.data[mip.offset..mip.offset + mip.size],
            mip.width,
            mip.height,
        )?;
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &expanded,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(mip.width * 4),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: mip.width,
                height: mip.height,
                depth_or_array_layers: 1,
            },
        );
        return Some(texture.create_view(&Default::default()));
    }

    for (i, mip) in dds.mips.iter().take(valid_mips as usize).enumerate() {
        let data = &dds.data[mip.offset..mip.offset + mip.size];
        let (block_w, bpb): (u32, u32) = match dds.format {
            hoi4_assets::DdsFormat::Bc1 => (4, 8),
            hoi4_assets::DdsFormat::Bc3 => (4, 16),
            hoi4_assets::DdsFormat::Bc5 => (4, 16),
            hoi4_assets::DdsFormat::Bgra8 => (1, 4),
            hoi4_assets::DdsFormat::Bgr555 | hoi4_assets::DdsFormat::Unknown(_) => return None,
        };
        let blocks_wide = (mip.width + block_w - 1) / block_w;
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: i as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(blocks_wide * bpb),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: mip.width,
                height: mip.height,
                depth_or_array_layers: 1,
            },
        );
    }
    Some(texture.create_view(&Default::default()))
}

fn decode_bc5_to_rgba8(data: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
    if width == 0 || height == 0 {
        return None;
    }
    let blocks_wide = (width + 3) / 4;
    let blocks_high = (height + 3) / 4;
    let expected = blocks_wide as usize * blocks_high as usize * 16;
    if data.len() < expected {
        return None;
    }
    let mut out = vec![0u8; width as usize * height as usize * 4];
    for by in 0..blocks_high {
        for bx in 0..blocks_wide {
            let offset = ((by * blocks_wide + bx) * 16) as usize;
            let r = decode_bc_channel(&data[offset..offset + 8]);
            let g = decode_bc_channel(&data[offset + 8..offset + 16]);
            for y in 0..4 {
                for x in 0..4 {
                    let px = bx * 4 + x;
                    let py = by * 4 + y;
                    if px >= width || py >= height {
                        continue;
                    }
                    let src = (y * 4 + x) as usize;
                    let dst = ((py * width + px) * 4) as usize;
                    out[dst] = r[src];
                    out[dst + 1] = g[src];
                    out[dst + 2] = 255;
                    out[dst + 3] = 255;
                }
            }
        }
    }
    Some(out)
}

fn decode_bc_channel(block: &[u8]) -> [u8; 16] {
    let a0 = block[0];
    let a1 = block[1];
    let mut table = [0u8; 8];
    table[0] = a0;
    table[1] = a1;
    if a0 > a1 {
        for i in 1..6 {
            table[i + 1] = (((6 - i) as u16 * a0 as u16 + i as u16 * a1 as u16) / 7) as u8;
        }
    } else {
        for i in 1..4 {
            table[i + 1] = (((4 - i) as u16 * a0 as u16 + i as u16 * a1 as u16) / 5) as u8;
        }
        table[6] = 0;
        table[7] = 255;
    }

    let mut bits = 0u64;
    for i in 0..6 {
        bits |= (block[2 + i] as u64) << (8 * i);
    }
    let mut out = [0u8; 16];
    for item in &mut out {
        let idx = (bits & 0x7) as usize;
        *item = table[idx];
        bits >>= 3;
    }
    out
}

fn create_1x1_white(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
    create_1x1_rgba(device, queue, [255, 255, 255, 255])
}

fn create_1x1_black(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
    create_1x1_rgba(device, queue, [0, 0, 0, 255])
}

fn create_1x1_normal(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
    // 平坦法线：(0.5, 0.5, 1.0) → (128, 128, 255, 255)
    create_1x1_rgba(device, queue, [128, 128, 255, 255])
}

fn create_1x1_rgba(device: &wgpu::Device, queue: &wgpu::Queue, rgba: [u8; 4]) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("pdxmesh_1x1"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4),
            rows_per_image: None,
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    tex.create_view(&Default::default())
}

fn create_grey_cubemap(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
    // 1×1×6 mid-grey + 蓝调（模拟天空底色）
    let texel: [u8; 4] = [115, 128, 153, 255]; // (0.45, 0.5, 0.6, 1.0) * 255
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("pdxmesh_env_cube_fallback"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 6,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for layer in 0..6 {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: layer,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &texel,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
    }
    tex.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    })
}

// ─── shader 源 ─────────────────────────────────────────────────────────────

/// Phase 3.12.5 — pdxmesh.wgsl 翻译的**实例化**变体。
///
/// 与 [`hoi4_render::translations::pdxmesh.wgsl`] 同款风格但：
///
/// - 顶点输入只取 pos / normal / uv；fragment 端用屏幕导数近似 TBN，
///   因此 vanilla normal map 可以在 instanced path 中生效。
/// - 加 4 个 instance attributes：`instance_pos: vec3` / `instance_scale: f32` /
///   `instance_tint: vec4` (Unorm8x4) / `instance_rotation_y: f32`
/// - 顶点 shader 用 instance 数据把局部 mesh 顶点变换到世界空间（绕 Y 轴自旋
///   + 缩放 + 平移）
const PDXMESH_INSTANCED_WGSL: &str = r#"
//#include "global_uniform.wgsl"
//#include "shader_lib.wgsl"

struct MeshMaterial {
    diffuse_tint: vec4<f32>,
    pbr_packed: vec4<f32>,    // (specular_intensity, glossiness, alpha_cutoff, emissive)
    feature_flags: vec4<f32>, // (use_normal, use_spec_gloss, use_emissive, snow_factor)
    animate_uv: vec4<f32>,
    rim_color: vec4<f32>,
    phase8_controls: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> material: MeshMaterial;

@group(1) @binding(0) var shadow_map_tex: texture_depth_2d;
@group(1) @binding(1) var shadow_sampler: sampler_comparison;
@group(1) @binding(2) var environment_cube: texture_cube<f32>;
@group(1) @binding(3) var environment_sampler: sampler;
@group(1) @binding(4) var gradient_border_ch1: texture_2d<f32>;
@group(1) @binding(5) var gradient_border_ch2: texture_2d<f32>;
@group(1) @binding(6) var gradient_border_ch3: texture_2d<f32>;
@group(1) @binding(7) var province_secondary_color: texture_2d<f32>;
@group(1) @binding(8) var fow_tex: texture_2d<f32>;
@group(1) @binding(9) var light_data_tex: texture_2d<f32>;
@group(1) @binding(10) var light_index_tex: texture_2d<f32>;
@group(1) @binding(11) var mud_snow_tex: texture_2d<f32>;

@group(2) @binding(0) var diffuse_tex: texture_2d<f32>;
@group(2) @binding(1) var normal_tex: texture_2d<f32>;
@group(2) @binding(2) var spec_gloss_tex: texture_2d<f32>;
@group(2) @binding(3) var emissive_tex: texture_2d<f32>;
@group(2) @binding(4) var mat_sampler: sampler;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    // Instance attributes:
    @location(4) instance_pos: vec3<f32>,
    @location(5) instance_scale: f32,
    @location(6) instance_tint: vec4<f32>,
    @location(7) instance_rotation_y: f32,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tint: vec3<f32>,
    @location(4) shadow_proj: vec4<f32>,
    @location(5) map_px: vec2<f32>,
};

fn rotate_y(p: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(
        p.x * c + p.z * s,
        p.y,
        -p.x * s + p.z * c,
    );
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;

    // 局部 mesh 顶点 → 世界（绕 Y 自旋 + 缩放 + 平移）
    let scaled_local = rotate_y(in.pos * in.instance_scale * material.phase8_controls.y, in.instance_rotation_y);
    let world_pos = scaled_local + in.instance_pos;

    // 法线只绕 Y 旋转（uniform scale 不扭曲方向）
    let world_normal = rotate_y(in.normal, in.instance_rotation_y);

    // UV 动画（建筑 = 0）
    let animated_uv = in.uv + material.animate_uv.xy * frame.global_time;

    out.clip_pos = frame.view_proj * vec4<f32>(world_pos, 1.0);
    out.world_pos = world_pos;
    out.normal = normalize(world_normal);
    out.uv = animated_uv;
    out.tint = in.instance_tint.rgb;
    // Shadow projection（接收方采样）
    out.shadow_proj = frame.shadow_view_proj * vec4<f32>(world_pos, 1.0);
    out.map_px = world_xz_to_map_px(world_pos.xz, frame.vanilla_map_size_world_size.zw);
    return out;
}

fn shadow_pcf_local(shadow_proj: vec4<f32>) -> f32 {
    let coords = shadow_proj.xy / shadow_proj.w * vec2<f32>(0.5, -0.5) + 0.5;
    let depth = shadow_proj.z / shadow_proj.w - 0.001;
    if (coords.x < 0.0 || coords.x > 1.0 || coords.y < 0.0 || coords.y > 1.0) {
        return 1.0; // 屏外不阴影
    }
    let s = textureSampleCompare(shadow_map_tex, shadow_sampler, coords, depth);
    return mix(1.0 - frame.shadow_fade_factor, 1.0, s);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let diffuse_sample = textureSample(diffuse_tex, mat_sampler, in.uv);
    if (diffuse_sample.a < material.pbr_packed.z) {
        discard;
    }
    let map_uv = map_px_to_uv(in.map_px);
    let secondary = textureSample(province_secondary_color, environment_sampler, map_uv);
    let diffuse_albedo = mix(
        diffuse_sample.rgb * material.diffuse_tint.rgb * in.tint * material.phase8_controls.z,
        secondary.rgb,
        secondary.a * 0.18,
    );

    // Approximate tangent-space normal mapping from screen derivatives. It is
    // less exact than authored tangents but gives vanilla building normal maps
    // visible relief while keeping the instanced vertex format compact.
    var normal = normalize(in.normal);
    if (material.feature_flags.x > 0.5) {
        let normal_sample = textureSample(normal_tex, mat_sampler, in.uv).rgb;
        let n_local = unpack_normal(normal_sample);
        let dp1 = dpdx(in.world_pos);
        let dp2 = dpdy(in.world_pos);
        let duv1 = dpdx(in.uv);
        let duv2 = dpdy(in.uv);
        let denom = duv1.x * duv2.y - duv1.y * duv2.x;
        if (abs(denom) > 0.00001) {
            let tangent = normalize((dp1 * duv2.y - dp2 * duv1.y) / denom);
            let bitangent = normalize((-dp1 * duv2.x + dp2 * duv1.x) / denom);
            let tbn = mat3x3<f32>(tangent, bitangent, normal);
            normal = normalize(tbn * n_local);
        }
    }

    // Specular
    var specular_color = vec3<f32>(material.pbr_packed.x);
    var glossiness = material.pbr_packed.y;
    if (material.feature_flags.y > 0.5) {
        let sg = textureSample(spec_gloss_tex, mat_sampler, in.uv);
        specular_color = mix(specular_color, sg.rgb * material.pbr_packed.x, 0.85);
        glossiness = mix(glossiness, sg.a, 0.85);
    }
    let non_linear_gloss = get_non_linear_glossiness(glossiness);

    // 主太阳方向（与 vanilla LIGHT_SHADOW_DIRECTION 一致）
    let sun_dir = normalize(vec3<f32>(-0.408, -0.816, 0.408));
    let to_light = -sun_dir;
    let to_camera = normalize(frame.cam_pos - in.world_pos);

    let bp = improved_blinn_phong(
        frame.sun_diffuse_intensity.rgb,
        to_light,
        to_camera,
        normal,
        specular_color,
        non_linear_gloss,
    );
    let shadow = shadow_pcf_local(in.shadow_proj);

    // 环境反射（cubemap fallback = mid-grey）
    let reflect_dir = reflect(-to_camera, normal);
    let env_mip = get_envmap_mip_level(glossiness);
    let env_sample = textureSampleLevel(
        environment_cube, environment_sampler, reflect_dir, env_mip
    ).rgb;

    var lit = (vec3<f32>(0.5) + bp.diffuse * shadow) * diffuse_albedo;
    lit += bp.specular * shadow;
    lit += env_sample * specular_color * frame.cubemap_intensity * 0.25;
    let country_d = textureSample(gradient_border_ch1, environment_sampler, map_uv).r * 255.0;
    let province_d = textureSample(gradient_border_ch2, environment_sampler, map_uv).r * 255.0;
    let semantic_d = textureSample(gradient_border_ch3, environment_sampler, map_uv).r * 255.0;
    let border_hint = 1.0 - smoothstep(0.0, 3.5, min(min(country_d, province_d), semantic_d));
    lit = mix(lit, lit * vec3<f32>(0.82, 0.84, 0.80), border_hint * 0.10);
    let globe_n = calc_globe_normal(in.map_px, frame.day_night_hour_sun_dir.x);
    let night = day_night_factor(globe_n, frame.day_night_hour_sun_dir.yzw, 1.0);

    // Emissive（建筑窗户夜光）— feature_flags.z = 0 时跳过
    if (material.feature_flags.z > 0.5) {
        let emit = textureSample(emissive_tex, mat_sampler, in.uv).rgb;
        lit += emit * material.pbr_packed.w * (0.2 + night * 0.8);
    } else {
        // 即使没贴图也给一个固定亮度的"窗户夜光"模拟（建筑被夜半球时整体提亮）
        let globe_n = calc_globe_normal(in.map_px, frame.day_night_hour_sun_dir.x);
        lit += diffuse_albedo * material.pbr_packed.w * 0.25 * night;
    }

    lit += calculate_point_lights(
        light_data_tex,
        light_index_tex,
        in.map_px,
        in.world_pos,
        normal,
        (0.18 + night * 0.82) * 0.52
    ) * 0.20;

    // Rim light
    let rim = smoothstep(0.55, 0.6, 1.0 - max(dot(normal, to_camera), 0.0));
    lit += rim * material.rim_color.rgb * material.rim_color.a;

    // Snow accumulation (feature_flags.w = 0 时跳过)
    if (material.feature_flags.w > 0.0) {
        let up_factor = max(normal.y, 0.0);
        let mud_snow = textureSample(mud_snow_tex, environment_sampler, map_uv);
        let map_snow = get_snow(mud_snow, frame.fow_opacity_time_snow_max_speed.z);
        let snow = up_factor * material.feature_flags.w * map_snow;
        lit = mix(lit, SNOW_COLOR_LIB, clamp(snow, 0.0, 0.85));
    }

    // 距离雾
    lit = apply_distance_fog(lit, in.world_pos, frame.cam_pos);
    let fow_visibility = textureSample(fow_tex, environment_sampler, map_uv).g;
    lit = mix(lit * 0.55, lit, fow_visibility);

    return vec4<f32>(lit, diffuse_sample.a * material.phase8_controls.x);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_size_matches_wgsl() {
        assert_eq!(std::mem::size_of::<MeshMaterial>(), 96);
    }

    #[test]
    fn instance_size_is_32_bytes() {
        assert_eq!(std::mem::size_of::<PdxMeshInstance>(), 32);
    }

    #[test]
    fn vertex_size_is_48_bytes() {
        assert_eq!(std::mem::size_of::<MeshVertex>(), 48);
    }

    #[test]
    fn split_buildings_routes_by_kind() {
        let buildings = vec![
            BuildingInstance {
                pos: [1.0, 0.0, 0.0],
                kind: 0.0,
            },
            BuildingInstance {
                pos: [2.0, 0.0, 0.0],
                kind: 0.0,
            },
            BuildingInstance {
                pos: [3.0, 0.0, 0.0],
                kind: 1.0,
            },
            BuildingInstance {
                pos: [4.0, 0.0, 0.0],
                kind: 2.0,
            },
            BuildingInstance {
                pos: [5.0, 0.0, 0.0],
                kind: 99.0, // ignored
            },
        ];
        let split = split_buildings_by_kind(&buildings);
        let civ = &split[BUILDING_KIND_INDUSTRIAL as usize];
        let mil = &split[BUILDING_KIND_MILITARY as usize];
        let dock = &split[BUILDING_KIND_DOCKYARD as usize];
        assert_eq!(civ.len(), 2);
        assert_eq!(mil.len(), 1);
        assert_eq!(dock.len(), 1);
        assert_eq!(civ[0].pos, [1.0, 0.0, 0.0]);
        assert_eq!(mil[0].pos, [3.0, 0.0, 0.0]);
        assert_eq!(dock[0].pos, [4.0, 0.0, 0.0]);
    }

    #[test]
    fn split_assigns_unique_rotations() {
        let buildings: Vec<BuildingInstance> = (0..6)
            .map(|i| BuildingInstance {
                pos: [i as f32, 0.0, 0.0],
                kind: 0.0,
            })
            .collect();
        let split = split_buildings_by_kind(&buildings);
        let civ = &split[BUILDING_KIND_INDUSTRIAL as usize];
        // 6 个建筑应有 6 个不同的 rotation_y（hash-based）
        let rotations: Vec<f32> = civ.iter().map(|i| i.rotation_y).collect();
        for i in 0..rotations.len() {
            for j in (i + 1)..rotations.len() {
                assert!(
                    (rotations[i] - rotations[j]).abs() > 1e-4,
                    "rotation_y collision at {} / {}",
                    i,
                    j
                );
            }
        }
    }

    #[test]
    fn tints_distinct_per_kind() {
        let civ_tint = tint_for_kind(BUILDING_KIND_INDUSTRIAL as f32);
        let mil_tint = tint_for_kind(BUILDING_KIND_MILITARY as f32);
        let dock_tint = tint_for_kind(BUILDING_KIND_DOCKYARD as f32);
        assert_ne!(civ_tint, mil_tint);
        assert_ne!(mil_tint, dock_tint);
        assert_ne!(civ_tint, dock_tint);
    }

    #[test]
    fn mesh_specs_cover_object_kinds() {
        assert_eq!(MESH_TYPE_SPECS.len(), PDXMESH_OBJECT_KIND_COUNT);
        for (idx, spec) in MESH_TYPE_SPECS.iter().enumerate() {
            assert_eq!(spec.kind as usize, idx);
            assert!(!spec.names.is_empty());
            assert!(spec.fallback_mesh.ends_with(".mesh"));
        }
    }

    #[test]
    fn normalize_mesh_tex_path_handles_bare_filename() {
        let p = normalize_mesh_tex_path("factory_d.dds", "gfx/models/buildings/factory.mesh");
        assert_eq!(p, "gfx/models/buildings/factory_d.dds");
    }

    #[test]
    fn normalize_mesh_tex_path_keeps_relative_path() {
        let p = normalize_mesh_tex_path(
            "gfx/models/units/ships/light.dds",
            "gfx/models/buildings/factory.mesh",
        );
        assert_eq!(p, "gfx/models/units/ships/light.dds");
    }

    #[test]
    fn normalize_mesh_tex_path_normalises_backslash() {
        let p = normalize_mesh_tex_path(
            "gfx\\models\\buildings\\factory_d.dds",
            "gfx/models/buildings/factory.mesh",
        );
        assert_eq!(p, "gfx/models/buildings/factory_d.dds");
    }

    /// naga static parse — `compose_shader` 注入 lib + uniform 后，shader 必须 parse 通过。
    #[test]
    fn pdxmesh_instanced_wgsl_naga_parses() {
        let composed = hoi4_render::shader_rt::compose_shader(PDXMESH_INSTANCED_WGSL, true, true);
        let result = naga::front::wgsl::parse_str(&composed);
        if let Err(e) = &result {
            eprintln!("=== composed shader (first 500 chars) ===");
            eprintln!("{}", &composed[..composed.len().min(500)]);
            eprintln!("=== parse error ===");
            eprintln!("{}", e);
        }
        assert!(result.is_ok(), "pdxmesh instanced wgsl should parse");
    }

    #[test]
    fn pdxmesh_instanced_wgsl_references_phase5_light_maps() {
        for token in [
            "light_data_tex",
            "light_index_tex",
            "mud_snow_tex",
            "calculate_point_lights(",
            "@group(1) @binding(9)",
            "@group(1) @binding(10)",
            "@group(1) @binding(11)",
        ] {
            assert!(
                PDXMESH_INSTANCED_WGSL.contains(token),
                "missing Phase 5 mesh light token: {token}"
            );
        }
    }
}
