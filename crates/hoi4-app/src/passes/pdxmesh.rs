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

use hoi4_assets::{AssetDb, DdsImage, FsAssetDb, PdxMesh as PdxMeshAsset};
use hoi4_paths::PathConfig;
use hoi4_render::buildings::BuildingInstance;
use wgpu::util::DeviceExt;

use crate::passes::HDR_FORMAT;

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
            // 关闭 normal/spec/emissive，保留小 snow_factor=0
            feature_flags: [0.0, 0.0, 0.0, 0.0],
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

/// `BuildingInstance` (16 bytes) → `PdxMeshInstance` (32 bytes)：
/// 按 `kind` 路由到三类 instance vec，加 hash-based 朝向抖动让建筑群不雷同。
pub fn split_buildings_by_kind(buildings: &[BuildingInstance]) -> [Vec<PdxMeshInstance>; 3] {
    let mut civ = Vec::new();
    let mut mil = Vec::new();
    let mut dock = Vec::new();
    for (i, b) in buildings.iter().enumerate() {
        // 简单 hash 抖动：i * golden ratio 取小数 → [0, 2π)
        let h = ((i as u32).wrapping_mul(2654435761)) as f32 / u32::MAX as f32;
        let rotation_y = h * std::f32::consts::TAU;
        let inst = PdxMeshInstance {
            pos: b.pos,
            scale: 0.022, // 建筑视觉高度约 0.04 世界单位 (~1.5 个像素的高)
            tint: tint_for_kind(b.kind),
            rotation_y,
            _pad: [0.0; 2],
        };
        let k = b.kind as i32;
        match k {
            0 => civ.push(inst),
            1 => mil.push(inst),
            2 => dock.push(inst),
            _ => {}
        }
    }
    [civ, mil, dock]
}

fn tint_for_kind(kind: f32) -> [u8; 4] {
    match kind as i32 {
        0 => [180, 220, 180, 255], // 民用：略冷的浅绿
        1 => [220, 180, 180, 255], // 军事：略暖的浅红
        2 => [180, 200, 220, 255], // 船坞：略冷的浅蓝
        _ => [255, 255, 255, 255],
    }
}

// ─── 单个 mesh 类型的 GPU 资源 ────────────────────────────────────────────

struct MeshTypeResources {
    /// vertex buffer（pos+normal+uv，stride 32 bytes）
    vertex_buffer: wgpu::Buffer,
    /// index buffer（u32）
    index_buffer: wgpu::Buffer,
    index_count: u32,
    /// 每实例 buffer（pos+scale+tint+rotation_y+pad，stride 32 bytes）
    instance_buffer: wgpu::Buffer,
    instance_count: u32,
    /// per-mesh 纹理 bind group（@group(2)）
    bind_group_2: wgpu::BindGroup,
    /// 是否成功加载（mesh + 至少 diffuse）
    loaded: bool,
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
    /// 每类建筑各一份资源
    types: [MeshTypeResources; 3],
    /// `MeshMaterial` GPU buffer（让 main.rs 可调 `update_material` 覆盖默认）
    material_buffer: wgpu::Buffer,
    /// 至少 1 类 mesh + diffuse 都加载成功；否则 main.rs 应回退到旧 buildings_pipeline
    pub any_loaded: bool,
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

        // ── BGL 1：shadow + envmap ──────────────────────────────────────
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

        // ── 加载 3 类 mesh ───────────────────────────────────────────────
        let db = FsAssetDb::new(path_cfg.clone());

        // (mesh_path, fallback_diffuse_path, fallback_normal_path, fallback_specular_path)
        // material 自己提取的优先于 fallback 路径。
        let defs: [(&str, &str, &str, &str); 3] = [
            (
                "gfx/models/buildings/civ_factory.mesh",
                "gfx/models/buildings/factory_d.dds",
                "gfx/models/buildings/factory_n.dds",
                "gfx/models/buildings/factory_s.dds",
            ),
            (
                "gfx/models/buildings/factory.mesh",
                "gfx/models/buildings/factory_d.dds",
                "gfx/models/buildings/factory_n.dds",
                "gfx/models/buildings/factory_s.dds",
            ),
            (
                "gfx/models/buildings/dock_01.mesh",
                "gfx/models/buildings/dock_diffuse.dds",
                "gfx/models/buildings/dock_normal.dds",
                "gfx/models/buildings/dock_specular.dds",
            ),
        ];

        let mut types_vec: Vec<MeshTypeResources> = Vec::with_capacity(3);
        let mut any_loaded = false;
        for (i, (mesh_path, fb_diff, fb_norm, fb_spec)) in defs.iter().enumerate() {
            let res = load_mesh_type(
                device,
                queue,
                &db,
                mesh_path,
                fb_diff,
                fb_norm,
                fb_spec,
                &bgl2,
                &mat_sampler,
            );
            if res.loaded {
                any_loaded = true;
                println!(
                    "[pdxmesh] type {} loaded ({} verts, {} idx)",
                    i,
                    res.index_count / 3,
                    res.index_count
                );
            } else {
                eprintln!(
                    "[pdxmesh] type {} ({}) FAILED — falling back to empty buffers",
                    i, mesh_path
                );
            }
            types_vec.push(res);
        }
        let types: [MeshTypeResources; 3] = types_vec
            .try_into()
            .unwrap_or_else(|_| panic!("[pdxmesh] internal: 3 types expected"));

        Self {
            pipeline,
            bind_group_0,
            bind_group_1,
            bgl_1: bgl1,
            env_sampler: env_sampler.clone(),
            types,
            material_buffer,
            any_loaded,
        }
    }

    /// 设置 / 更新建筑实例数据。按 `kind` 拆分到三类 instance buffer。
    pub fn set_buildings(&mut self, device: &wgpu::Device, buildings: &[BuildingInstance]) {
        let split = split_buildings_by_kind(buildings);
        for (i, inst_vec) in split.iter().enumerate() {
            // 为空时仍创建一个 1-instance dummy 占位（不画 — count = 0）
            let bytes: &[u8] = if inst_vec.is_empty() {
                &[0u8; std::mem::size_of::<PdxMeshInstance>()]
            } else {
                bytemuck::cast_slice(inst_vec)
            };
            self.types[i].instance_buffer =
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("pdxmesh_instances"),
                    contents: bytes,
                    usage: wgpu::BufferUsages::VERTEX,
                });
            self.types[i].instance_count = inst_vec.len() as u32;
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
    ) {
        let material = MeshMaterial {
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
            if !ty.loaded || ty.instance_count == 0 || ty.index_count == 0 {
                continue;
            }
            pass.set_bind_group(2, &ty.bind_group_2, &[]);
            pass.set_vertex_buffer(0, ty.vertex_buffer.slice(..));
            pass.set_vertex_buffer(1, ty.instance_buffer.slice(..));
            pass.set_index_buffer(ty.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..ty.index_count, 0, 0..ty.instance_count);
        }
    }

    /// 总实例数（3 类之和）— 用于调试 banner。
    pub fn total_instances(&self) -> u32 {
        self.types.iter().map(|t| t.instance_count).sum()
    }
}

// ─── 私有 helpers ─────────────────────────────────────────────────────────

fn load_mesh_type(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    db: &FsAssetDb,
    mesh_path: &str,
    fb_diff: &str,
    fb_norm: &str,
    fb_spec: &str,
    bgl2: &wgpu::BindGroupLayout,
    mat_sampler: &wgpu::Sampler,
) -> MeshTypeResources {
    let parsed = match db.open(mesh_path) {
        Ok(bytes) => match PdxMeshAsset::parse(&bytes) {
            Ok(m) => Some(m),
            Err(e) => {
                eprintln!("[pdxmesh] {} parse error: {}", mesh_path, e);
                None
            }
        },
        Err(e) => {
            eprintln!("[pdxmesh] {} open failed: {}", mesh_path, e);
            None
        }
    };

    let (vertices, indices, mat) = match parsed {
        Some(m) if !m.meshes.is_empty() => {
            // 取第一个 SubMesh（最高 LOD）
            let sub = m.meshes.into_iter().next().unwrap();
            let mut verts = Vec::with_capacity(sub.positions.len());
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
                verts.push(MeshVertex {
                    pos,
                    _pad0: 0.0,
                    normal,
                    _pad1: 0.0,
                    uv,
                    _pad2: [0.0; 2],
                });
            }
            (verts, sub.indices, Some(sub.material))
        }
        _ => (Vec::new(), Vec::new(), None),
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
        .map(|m| resolve_path(m.diffuse.as_ref(), fb_diff))
        .unwrap_or_else(|| fb_diff.to_string());
    let _normal_path = mat
        .as_ref()
        .map(|m| resolve_path(m.normal.as_ref(), fb_norm))
        .unwrap_or_else(|| fb_norm.to_string());
    let _specular_path = mat
        .as_ref()
        .map(|m| resolve_path(m.specular.as_ref(), fb_spec))
        .unwrap_or_else(|| fb_spec.to_string());

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

    // 加载 diffuse；其他先用 1×1 fallback（feature_flags 关）
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
    // normal / spec / emissive 先全占位 — 3.12.5 不接（feature_flags 关），3.12 后续修
    let normal_view = create_1x1_normal(device, queue);
    let spec_view = create_1x1_white(device, queue);
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

    if vertices.is_empty() || indices.is_empty() {
        loaded = false;
    }

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
            bytemuck::cast_slice(&indices)
        },
        usage: wgpu::BufferUsages::INDEX,
    });
    let index_count = indices.len() as u32;
    let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("pdxmesh_instances_init"),
        contents: &[0u8; std::mem::size_of::<PdxMeshInstance>()],
        usage: wgpu::BufferUsages::VERTEX,
    });

    MeshTypeResources {
        vertex_buffer,
        index_buffer,
        index_count,
        instance_buffer,
        instance_count: 0,
        bind_group_2,
        loaded,
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
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
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
/// - 顶点输入只取 pos / normal / uv（3D 建筑 mesh 普遍无 tangent；
///   `feature_flags.x` 关掉 normal 贴图）
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
    let diffuse_albedo = diffuse_sample.rgb * material.diffuse_tint.rgb * in.tint * material.phase8_controls.z;

    // 法线（无 tangent → 仅用 vertex normal）
    let normal = normalize(in.normal);

    // Specular
    let specular_color = vec3<f32>(material.pbr_packed.x);
    let glossiness = material.pbr_packed.y;
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

    // Emissive（建筑窗户夜光）— feature_flags.z = 0 时跳过
    if (material.feature_flags.z > 0.5) {
        let emit = textureSample(emissive_tex, mat_sampler, in.uv).rgb;
        let globe_n = calc_globe_normal(in.world_pos.xz, frame.day_night_hour_sun_dir.x);
        let night = day_night_factor(globe_n, frame.day_night_hour_sun_dir.yzw, 1.0);
        lit += emit * material.pbr_packed.w * (0.2 + night * 0.8);
    } else {
        // 即使没贴图也给一个固定亮度的"窗户夜光"模拟（建筑被夜半球时整体提亮）
        let globe_n = calc_globe_normal(in.world_pos.xz, frame.day_night_hour_sun_dir.x);
        let night = day_night_factor(globe_n, frame.day_night_hour_sun_dir.yzw, 1.0);
        lit += diffuse_albedo * material.pbr_packed.w * 0.25 * night;
    }

    // Rim light
    let rim = smoothstep(0.55, 0.6, 1.0 - max(dot(normal, to_camera), 0.0));
    lit += rim * material.rim_color.rgb * material.rim_color.a;

    // Snow accumulation (feature_flags.w = 0 时跳过)
    if (material.feature_flags.w > 0.0) {
        let up_factor = max(normal.y, 0.0);
        let snow = up_factor * material.feature_flags.w;
        lit = mix(lit, SNOW_COLOR_LIB, clamp(snow, 0.0, 0.85));
    }

    // 距离雾
    lit = apply_distance_fog(lit, in.world_pos, frame.cam_pos);

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
        let [civ, mil, dock] = split_buildings_by_kind(&buildings);
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
        let [civ, _, _] = split_buildings_by_kind(&buildings);
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
        let civ_tint = tint_for_kind(0.0);
        let mil_tint = tint_for_kind(1.0);
        let dock_tint = tint_for_kind(2.0);
        assert_ne!(civ_tint, mil_tint);
        assert_ne!(mil_tint, dock_tint);
        assert_ne!(civ_tint, dock_tint);
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
}
