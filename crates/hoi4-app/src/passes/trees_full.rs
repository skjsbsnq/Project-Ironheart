//! Phase 3.12.8 — `TreeFullPass`：vanilla tree.shader 完整版集成。
//!
//! **替代谁**：`trees_mesh.wgsl`（Lambert + 距离淡出）+ main.rs 内嵌的
//! `setup_tree_mesh_pipeline` 函数 + `trees_mesh_*` 字段组。
//!
//! **本 pass** 用独立 pipeline 画 3D mesh 树木，功能等价于 vanilla `tree.shader`：
//!
//! 1. **季节染色**：`Tree_season.bmp` atlas 采样 + `seasons.txt` 驱动 `season_lerp` / `season_column`
//! 2. **个性化 tint**：`Tree_tint.bmp` 采样 + overlay 混合
//! 3. **法线贴图**：per-species normal map (beech_normal / pinetree_normal / palmblad_normal)
//! 4. **阴影接收**：`shadow_pcf` from shadow map via `sampler_comparison`
//! 5. **昼夜 + 距离雾**：复用 `shader_lib.wgsl` 公共函数
//! 6. **远景剔除**：`TreeMaskTexture` 采样 + 距离 alpha 衰减
//!
//! ## 实例布局
//!
//! 使用 48 字节实例，携带 vanilla tree.shader 所需的全量 per-tree 数据：
//!
//! ```text
//! offset  size  format       field
//! 0       12    Float32x3    pos
//! 12       4    Float32      scale
//! 16       4    Float32      tree_type (f32 for vec4 packing)
//! 20       8    Float32x2    slope_xz
//! 28       8    Float32x2    tint_uv (world_pos.xz / world_size → Tree_tint.bmp UV)
//! 36       4    Float32      season_row (latitude-based V into Tree_season.bmp atlas)
//! 40       8    unused pad   —
//! ```
//!
//! `tint_uv` 在 `generate_trees` 时从 `world_pos.xz / world_size` 预计算，
//! 对应 `Tree_tint.bmp` 采样坐标；`season_row` 取 `world_z / world_d` 反映
//! 纬度行差异（与 vanilla `vTexCoord0_TintUV.w` = per-tree Y in atlas 一致）。

#![allow(dead_code)]

use hoi4_assets::{AssetDb, DdsImage, FsAssetDb, MapResRole, PdxMesh};
use hoi4_paths::PathConfig;
use hoi4_render::trees::TreeInstance;
use hoi4_render::trees_mesh::{build_tree_mesh, TreeMeshVertex, INSTANCE_CAP_PER_TYPE};
use std::collections::HashSet;
use wgpu::util::DeviceExt;

use crate::passes::HDR_FORMAT;
use crate::vanilla_resource_views::{
    create_dynamic_target_1x1, upload_dds_or_fallback, BindingAudit, BindingAuditEntry,
    DdsUploadRequest, VanillaResourceViews,
};
use crate::vanilla_targets::VanillaRuntimeTargets;

const SHADER_WGSL: &str = include_str!("trees_full.wgsl");

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct TreeFullParams {
    pub season_lerp: f32,
    pub season_column: f32,
    pub fade_start: f32,
    pub fade_end: f32,
    pub world_w: f32,
    pub world_d: f32,
    pub season_column_next: f32,
    pub season_blend: f32,
    pub opacity: f32,
    pub scale: f32,
    pub _pad0: f32,
    pub _pad1: f32,
}

impl Default for TreeFullParams {
    fn default() -> Self {
        Self {
            season_lerp: 0.0,
            season_column: 2.0,
            fade_start: 30.0,
            fade_end: 45.0,
            world_w: 112.0,
            world_d: 41.0,
            season_column_next: 2.0,
            season_blend: 0.0,
            opacity: 1.0,
            scale: 1.0,
            _pad0: 0.0,
            _pad1: 0.0,
        }
    }
}

const _: () = assert!(std::mem::size_of::<TreeFullParams>() == 48);

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct TreeFullInstance {
    pub pos: [f32; 3],
    pub scale: f32,
    pub tree_type: f32,
    pub slope_x: f32,
    pub slope_z: f32,
    pub tint_uv: [f32; 2],
    pub season_row: f32,
    pub _pad0: f32,
    pub _pad1: f32,
}

const _: () = assert!(std::mem::size_of::<TreeFullInstance>() == 48);

fn convert_instances(
    instances: &[TreeInstance],
    target_type: u8,
    world_w: f32,
    world_d: f32,
) -> Vec<TreeFullInstance> {
    instances
        .iter()
        .filter(|t| t.tree_type == target_type)
        .map(|t| {
            let tint_uv = [t.pos[0] / world_w, t.pos[2] / world_d];
            let season_row = t.pos[2] / world_d;
            TreeFullInstance {
                pos: t.pos,
                scale: t.scale,
                tree_type: t.tree_type as f32,
                slope_x: t.slope_x as f32 / 127.0,
                slope_z: t.slope_z as f32 / 127.0,
                tint_uv,
                season_row,
                _pad0: 0.0,
                _pad1: 0.0,
            }
        })
        .collect()
}

struct TreeLodMesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    instance_buffer: wgpu::Buffer,
    instance_count: u32,
    instance_capacity: u32,
}

const MAX_TREE_LODS: usize = 3;
const LOD_DISTANCES: [f32; MAX_TREE_LODS] = [15.0, 30.0, 1000.0];

struct TreeTypeData {
    lods: Vec<TreeLodMesh>,
    bind_group: wgpu::BindGroup,
    diffuse_view: wgpu::TextureView,
    normal_view: wgpu::TextureView,
    _diffuse_tex: wgpu::Texture,
    _normal_tex: Option<wgpu::Texture>,
    all_instances: Vec<TreeFullInstance>,
    lod_distances: [f32; MAX_TREE_LODS],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TreeSharedMaterialComponent {
    Mask,
    Season,
    Tint,
    Colormap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TreeSharedTextureSpec {
    role: MapResRole,
    binding: &'static str,
    fallback_rgba: [u8; 4],
    component: TreeSharedMaterialComponent,
    critical: bool,
    visual_impact: &'static str,
}

pub struct TreeMaterialSystem;

impl TreeMaterialSystem {
    const SHARED_TEXTURES: [TreeSharedTextureSpec; 4] = [
        TreeSharedTextureSpec {
            role: MapResRole::TreesMask,
            binding: "tree_mask",
            fallback_rgba: [0, 0, 0, 255],
            component: TreeSharedMaterialComponent::Mask,
            critical: true,
            visual_impact: "distant tree clipping and forest density mask disappear",
        },
        TreeSharedTextureSpec {
            role: MapResRole::TreeSeason,
            binding: "tree_season",
            fallback_rgba: [255, 255, 255, 255],
            component: TreeSharedMaterialComponent::Season,
            critical: true,
            visual_impact: "seasonal foliage colors fall back to summer-neutral",
        },
        TreeSharedTextureSpec {
            role: MapResRole::TreeTint,
            binding: "tree_tint",
            fallback_rgba: [128, 128, 128, 255],
            component: TreeSharedMaterialComponent::Tint,
            critical: true,
            visual_impact: "per-region foliage tint falls back to neutral gray",
        },
        TreeSharedTextureSpec {
            role: MapResRole::ColormapEmissive,
            binding: "tree_colormap",
            fallback_rgba: [128, 128, 128, 255],
            component: TreeSharedMaterialComponent::Colormap,
            critical: true,
            visual_impact: "tree color no longer follows terrain ColorMap/ColorMapSecond",
        },
    ];

    fn shared_texture_specs() -> &'static [TreeSharedTextureSpec; 4] {
        &Self::SHARED_TEXTURES
    }
}

pub struct TreeFullPass {
    pipeline: wgpu::RenderPipeline,
    bind_group_g0: wgpu::BindGroup,
    bind_group_g1: wgpu::BindGroup,
    params_buffer: wgpu::Buffer,
    types: Vec<TreeTypeData>,
    pub any_loaded: bool,
    pub load_warnings: Vec<String>,
    pub binding_audit: BindingAudit,
    _season_tex: wgpu::Texture,
    _tint_tex: wgpu::Texture,
    _mask_tex: wgpu::Texture,
    _colormap_tex: wgpu::Texture,
    _light_data_tex: wgpu::Texture,
    _light_index_tex: wgpu::Texture,
    _shadow_tex_held: bool,
    _owned_samplers: Vec<wgpu::Sampler>,
}

pub struct TreeFullPassInputs<'a> {
    pub global_uniform_buffer: &'a wgpu::Buffer,
    pub shadow_depth_view: &'a wgpu::TextureView,
    pub shadow_compare_sampler: &'a wgpu::Sampler,
    pub depth_format: wgpu::TextureFormat,
    pub world_size: [f32; 2],
    pub season_lerp: f32,
    pub season_column: f32,
    pub runtime_targets: &'a VanillaRuntimeTargets,
    pub vanilla_resources: &'a VanillaResourceViews,
    pub tree_indices: &'a HashSet<u8>,
}

impl TreeFullPass {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path_cfg: &PathConfig,
        tree_instances: &[TreeInstance],
        inputs: TreeFullPassInputs<'_>,
    ) -> Self {
        let mut warnings = Vec::new();
        let mut owned_samplers = Vec::new();
        let mut binding_audit = BindingAudit::new();
        binding_audit.extend(
            inputs
                .runtime_targets
                .binding_audit_entries_for_pass("tree"),
        );
        binding_audit.extend([
            BindingAuditEntry::dynamic_target_blocker(
                "tree",
                "light_data",
                "light_data_empty_target",
                "Vanilla point light render target is not generated yet",
                "tree material does not receive local night highlights until Phase 5",
            ),
            BindingAuditEntry::dynamic_target_blocker(
                "tree",
                "light_index",
                "light_index_empty_target",
                "Vanilla point light index target is not generated yet",
                "tree point light lookup is disabled until Phase 5",
            ),
        ]);

        let composed = hoi4_render::shader_rt::compose_shader(SHADER_WGSL, true, true);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("trees_full_shader"),
            source: wgpu::ShaderSource::Wgsl(composed.into()),
        });

        let params_init = TreeFullParams {
            season_lerp: inputs.season_lerp,
            season_column: inputs.season_column,
            world_w: inputs.world_size[0],
            world_d: inputs.world_size[1],
            ..TreeFullParams::default()
        };
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tree_full_params"),
            contents: bytemuck::bytes_of(&params_init),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let db = FsAssetDb::new(path_cfg.clone());
        let active_tree_pixels = inputs
            .vanilla_resources
            .bytes(MapResRole::TreesMask)
            .and_then(|bytes| hoi4_map::parse_trees_bmp(bytes).ok())
            .map(|bmp| bmp.active_pixel_count(inputs.tree_indices));
        match active_tree_pixels {
            Some(count) => println!(
                "[trees_full] trees.bmp active pixels for configured tree indices: {}",
                count
            ),
            None => warnings.push(
                "[trees_full] trees.bmp unavailable for active-pixel density audit".to_string(),
            ),
        }

        let (season_view, season_tex, season_audit) = load_tree_bmp_or_fallback(
            device,
            queue,
            inputs.vanilla_resources,
            MapResRole::TreeSeason,
            "tree_season",
            [255, 255, 255, 255],
            true,
            "seasonal foliage colors fall back to summer-neutral",
            &mut warnings,
        );
        binding_audit.extend([season_audit]);
        let (tint_view, tint_tex, tint_audit) = load_tree_bmp_or_fallback(
            device,
            queue,
            inputs.vanilla_resources,
            MapResRole::TreeTint,
            "tree_tint",
            [128, 128, 128, 255],
            true,
            "per-region foliage tint falls back to neutral gray",
            &mut warnings,
        );
        binding_audit.extend([tint_audit]);

        let (mask_view, mask_tex, mask_audit) = load_tree_bmp_or_fallback(
            device,
            queue,
            inputs.vanilla_resources,
            MapResRole::TreesMask,
            "tree_mask",
            [0, 0, 0, 255],
            true,
            "distant tree clipping and forest density mask disappear",
            &mut warnings,
        );
        binding_audit.extend([mask_audit]);
        let colormap = upload_dds_or_fallback(
            device,
            queue,
            inputs.vanilla_resources,
            DdsUploadRequest {
                role: MapResRole::ColormapEmissive,
                label: "tree_colormap",
                fallback_rgba: [128, 128, 128, 255],
                srgb: true,
                critical: true,
                pass: "tree",
                binding: "tree_colormap",
                visual_impact: "tree color no longer follows terrain ColorMap/ColorMapSecond",
            },
            &mut warnings,
        );
        binding_audit.extend([colormap.audit.clone()]);
        let colormap_tex = colormap.texture;
        let colormap_view = colormap.view;

        let (light_data_tex, light_data_view) =
            create_dynamic_target_1x1(device, queue, "light_data_empty_target", [0, 0, 0, 0]);
        let (light_index_tex, light_index_view) = create_dynamic_target_1x1(
            device,
            queue,
            "light_index_empty_target",
            [255, 255, 255, 255],
        );

        let tree_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("tree_sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let map_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("tree_map_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        owned_samplers.push(tree_sampler);
        owned_samplers.push(map_sampler.clone());

        // BGL g0: global frame + tree params
        let bgl_g0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tree_full_bgl_g0"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
                uniform_entry(1, wgpu::ShaderStages::VERTEX_FRAGMENT),
            ],
        });
        let bind_group_g0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tree_full_bg_g0"),
            layout: &bgl_g0,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: inputs.global_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        });

        // BGL g1: shadow + vanilla tree shared maps + runtime targets + blocker light maps
        let bgl_g1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tree_full_bgl_g1"),
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
                fragment_tex_entry(2),
                fragment_tex_entry(3),
                fragment_tex_entry(4),
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                fragment_tex_entry(6),
                fragment_tex_entry(7),
                fragment_tex_entry(8),
                fragment_tex_entry(9),
                fragment_tex_entry(10),
                fragment_tex_entry(11),
                fragment_tex_entry(12),
                fragment_tex_entry(13),
                fragment_tex_entry(14),
            ],
        });
        let bind_group_g1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tree_full_bg_g1"),
            layout: &bgl_g1,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(inputs.shadow_depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(inputs.shadow_compare_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&season_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&tint_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&mask_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&map_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.runtime_targets.gradient_border.ch1.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.runtime_targets.gradient_border.ch2.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.runtime_targets.gradient_border.ch3.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.runtime_targets.province_secondary_color.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::TextureView(&inputs.runtime_targets.fow.view),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::TextureView(
                        &inputs.runtime_targets.mud_snow.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 12,
                    resource: wgpu::BindingResource::TextureView(&colormap_view),
                },
                wgpu::BindGroupEntry {
                    binding: 13,
                    resource: wgpu::BindingResource::TextureView(&light_data_view),
                },
                wgpu::BindGroupEntry {
                    binding: 14,
                    resource: wgpu::BindingResource::TextureView(&light_index_view),
                },
            ],
        });

        // BGL g2: per-type diffuse + normal + sampler
        let bgl_g2 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tree_full_bgl_g2"),
            entries: &[
                fragment_tex_entry(0),
                fragment_tex_entry(1),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let tree_defs: [(&str, &str, &str); 3] = [
            (
                "gfx/models/mapitems/trees/beech.mesh",
                "gfx/models/mapitems/trees/beech_diffuse.dds",
                "gfx/models/mapitems/trees/beech_normal.dds",
            ),
            (
                "gfx/models/mapitems/trees/Pine_01.mesh",
                "gfx/models/mapitems/trees/pinetree_diffuse.dds",
                "gfx/models/mapitems/trees/pinetree_normal.dds",
            ),
            (
                "gfx/models/mapitems/trees/palmer.mesh",
                "gfx/models/mapitems/trees/palm_lod_diffuse.dds",
                "gfx/models/mapitems/trees/palmblad_normal.dds",
            ),
        ];

        let mut types = Vec::new();
        let mut any_loaded = false;

        for (type_idx, (mesh_path, diff_path, norm_path)) in tree_defs.iter().enumerate() {
            let parsed_mesh =
                db.open(*mesh_path)
                    .ok()
                    .and_then(|bytes| match PdxMesh::parse(&bytes) {
                        Ok(m) if !m.meshes.is_empty() => Some(m),
                        Ok(_) => {
                            eprintln!("[trees_full] {} parsed OK but 0 submeshes", mesh_path);
                            None
                        }
                        Err(e) => {
                            eprintln!("[trees_full] {} parse error: {}", mesh_path, e);
                            None
                        }
                    });

            let parsed_mesh = match parsed_mesh {
                Some(m) => m,
                None => {
                    eprintln!("[trees_full] failed to load {}", mesh_path);
                    warnings.push(format!("{} mesh load failed", mesh_path));
                    continue;
                }
            };

            let mut lod_meshes = Vec::new();
            for lod_idx in 0..MAX_TREE_LODS {
                let sub = parsed_mesh.pick_lod(lod_idx);
                let mesh_data =
                    sub.and_then(|s| build_tree_mesh(&s.positions, &s.normals, &s.uvs, &s.indices));
                if let Some(d) = mesh_data {
                    let (mut mn, mut mx) = ([f32::MAX; 3], [f32::MIN; 3]);
                    for v in &d.vertices {
                        for i in 0..3 {
                            mn[i] = mn[i].min(v.position[i]);
                            mx[i] = mx[i].max(v.position[i]);
                        }
                    }
                    println!(
                        "[trees_full] {} LOD{} ?{} verts, {} idx, bounds [{:.2},{:.2},{:.2}]→[{:.2},{:.2},{:.2}]",
                        mesh_path,
                        lod_idx,
                        d.vertex_count,
                        d.index_count,
                        mn[0],
                        mn[1],
                        mn[2],
                        mx[0],
                        mx[1],
                        mx[2]
                    );
                    let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(&format!("tree_full_vb_lod{}", lod_idx)),
                        contents: bytemuck::cast_slice(&d.vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(&format!("tree_full_ib_lod{}", lod_idx)),
                        contents: bytemuck::cast_slice(&d.indices),
                        usage: wgpu::BufferUsages::INDEX,
                    });
                    let cap = if lod_idx == 0 {
                        INSTANCE_CAP_PER_TYPE
                    } else {
                        INSTANCE_CAP_PER_TYPE / 2
                    };
                    let inst_buf = device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some(&format!("tree_full_inst_lod{}", lod_idx)),
                        size: (cap as u64) * std::mem::size_of::<TreeFullInstance>() as u64,
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                    lod_meshes.push(TreeLodMesh {
                        vertex_buffer: vb,
                        index_buffer: ib,
                        index_count: d.index_count,
                        instance_buffer: inst_buf,
                        instance_count: 0,
                        instance_capacity: cap as u32,
                    });
                } else {
                    break;
                }
            }

            if lod_meshes.is_empty() {
                eprintln!("[trees_full] {} has no loadable LOD meshes", mesh_path);
                warnings.push(format!("{} no loadable LOD meshes", mesh_path));
                continue;
            }

            let (diff_view, diff_tex) = load_dds_or_fallback(
                device,
                queue,
                &db,
                diff_path,
                [255, 255, 255, 255],
                true,
                &mut warnings,
            );
            let (norm_view, norm_tex) = load_dds_or_fallback(
                device,
                queue,
                &db,
                norm_path,
                [128, 128, 255, 255],
                false,
                &mut warnings,
            );

            let raw_instances = convert_instances(
                tree_instances,
                type_idx as u8,
                inputs.world_size[0],
                inputs.world_size[1],
            );
            let pre_cap = raw_instances.len();
            let instances = cap_tree_instances(raw_instances, INSTANCE_CAP_PER_TYPE);
            println!(
                "[trees_full] type {} ?{} instances (pre-cap {}, cap {})",
                type_idx,
                instances.len(),
                pre_cap,
                INSTANCE_CAP_PER_TYPE
            );

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("tree_full_bg_g2"),
                layout: &bgl_g2,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&diff_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&norm_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&owned_samplers[0]),
                    },
                ],
            });

            let mut lod_dist = LOD_DISTANCES;
            if !parsed_mesh.lod_distances.is_empty() {
                for (i, &d) in parsed_mesh.lod_distances.iter().enumerate() {
                    if i < MAX_TREE_LODS && d > 0.0 {
                        lod_dist[i] = d * inputs.world_size[0] / 5632.0;
                    }
                }
            }

            any_loaded = true;

            types.push(TreeTypeData {
                lods: lod_meshes,
                bind_group,
                diffuse_view: diff_view,
                normal_view: norm_view,
                _diffuse_tex: diff_tex,
                _normal_tex: Some(norm_tex),
                all_instances: instances,
                lod_distances: lod_dist,
            });
        }

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tree_full_pl"),
            bind_group_layouts: &[&bgl_g0, &bgl_g1, &bgl_g2],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tree_full_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<TreeMeshVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[
                            wgpu::VertexAttribute {
                                offset: 0,
                                shader_location: 0,
                                format: wgpu::VertexFormat::Float32x3,
                            },
                            wgpu::VertexAttribute {
                                offset: 12,
                                shader_location: 1,
                                format: wgpu::VertexFormat::Float32x3,
                            },
                            wgpu::VertexAttribute {
                                offset: 24,
                                shader_location: 2,
                                format: wgpu::VertexFormat::Float32x2,
                            },
                        ],
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<TreeFullInstance>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[
                            wgpu::VertexAttribute {
                                offset: 0,
                                shader_location: 3,
                                format: wgpu::VertexFormat::Float32x3,
                            },
                            wgpu::VertexAttribute {
                                offset: 12,
                                shader_location: 4,
                                format: wgpu::VertexFormat::Float32,
                            },
                            wgpu::VertexAttribute {
                                offset: 16,
                                shader_location: 5,
                                format: wgpu::VertexFormat::Float32,
                            },
                            wgpu::VertexAttribute {
                                offset: 20,
                                shader_location: 6,
                                format: wgpu::VertexFormat::Float32x2,
                            },
                            wgpu::VertexAttribute {
                                offset: 28,
                                shader_location: 7,
                                format: wgpu::VertexFormat::Float32x2,
                            },
                            wgpu::VertexAttribute {
                                offset: 36,
                                shader_location: 8,
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
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: inputs.depth_format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            bind_group_g0,
            bind_group_g1,
            params_buffer,
            types,
            any_loaded,
            load_warnings: warnings,
            binding_audit,
            _season_tex: season_tex,
            _tint_tex: tint_tex,
            _mask_tex: mask_tex,
            _colormap_tex: colormap_tex,
            _light_data_tex: light_data_tex,
            _light_index_tex: light_index_tex,
            _shadow_tex_held: false,
            _owned_samplers: owned_samplers,
        }
    }

    pub fn update_params(&self, queue: &wgpu::Queue, params: &TreeFullParams) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(params));
    }

    pub fn upload_instances(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        cam_pos: [f32; 3],
    ) {
        for td in &mut self.types {
            let num_lods = td.lods.len();
            if num_lods == 0 || td.all_instances.is_empty() {
                continue;
            }

            let mut lod_buckets: Vec<Vec<TreeFullInstance>> =
                (0..num_lods).map(|_| Vec::new()).collect();

            for inst in &td.all_instances {
                let dx = inst.pos[0] - cam_pos[0];
                let dz = inst.pos[2] - cam_pos[2];
                let dist = (dx * dx + dz * dz).sqrt();

                let mut assigned = num_lods - 1;
                for (lod_idx, &threshold) in td.lod_distances.iter().enumerate() {
                    if lod_idx >= num_lods {
                        break;
                    }
                    if dist < threshold {
                        assigned = lod_idx;
                        break;
                    }
                }
                lod_buckets[assigned].push(*inst);
            }

            for (lod_idx, bucket) in lod_buckets.iter().enumerate() {
                if lod_idx >= td.lods.len() {
                    break;
                }
                let lod = &mut td.lods[lod_idx];
                let count = bucket.len() as u32;
                if count == 0 {
                    lod.instance_count = 0;
                    continue;
                }
                if count > lod.instance_capacity {
                    let new_cap = count.next_power_of_two();
                    lod.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some(&format!("tree_inst_lod{}_grow", lod_idx)),
                        size: (new_cap as u64) * std::mem::size_of::<TreeFullInstance>() as u64,
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                    lod.instance_capacity = new_cap;
                }
                queue.write_buffer(&lod.instance_buffer, 0, bytemuck::cast_slice(bucket));
                lod.instance_count = count;
            }
        }
    }

    pub fn render<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if !self.any_loaded {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group_g0, &[]);
        pass.set_bind_group(1, &self.bind_group_g1, &[]);
        for td in &self.types {
            if td.lods.is_empty() || td.all_instances.is_empty() {
                continue;
            }
            pass.set_bind_group(2, &td.bind_group, &[]);
            for lod in &td.lods {
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
}

fn cap_tree_instances(instances: Vec<TreeFullInstance>, cap: usize) -> Vec<TreeFullInstance> {
    if instances.len() <= cap || cap == 0 {
        return instances;
    }
    let n = instances.len();
    let step = (n + cap - 1) / cap;
    let mut picked = Vec::with_capacity(cap);
    let mut i = 0usize;
    while i < n && picked.len() < cap {
        picked.push(instances[i]);
        i += step;
    }
    picked
}

fn uniform_entry(binding: u32, vis: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: vis,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn fragment_tex_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn load_dds_or_fallback(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    db: &FsAssetDb,
    path: &str,
    fallback_rgba: [u8; 4],
    srgb: bool,
    warnings: &mut Vec<String>,
) -> (wgpu::TextureView, wgpu::Texture) {
    let result: Option<(wgpu::Texture, wgpu::TextureView)> = (|| {
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
            _ => return None,
        };
        let valid_mips = match dds.format {
            hoi4_assets::DdsFormat::Bc1
            | hoi4_assets::DdsFormat::Bc3
            | hoi4_assets::DdsFormat::Bc5 => dds
                .mips
                .iter()
                .take_while(|m| m.width >= 4 && m.height >= 4)
                .count() as u32,
            _ => dds.mips.len() as u32,
        };
        if valid_mips == 0 {
            return None;
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(path),
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
                _ => return None,
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
        let view = texture.create_view(&Default::default());
        Some((texture, view))
    })();

    match result {
        Some((tex, view)) => (view, tex),
        None => {
            warnings.push(format!(
                "[trees_full] {} missing — using 1×1 fallback",
                path
            ));
            let (tex, view) = create_1x1_rgba(device, queue, fallback_rgba, srgb);
            (view, tex)
        }
    }
}

fn load_tree_bmp_or_fallback(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    resources: &VanillaResourceViews,
    role: MapResRole,
    binding: &'static str,
    fallback_rgba: [u8; 4],
    critical: bool,
    visual_impact: &'static str,
    warnings: &mut Vec<String>,
) -> (wgpu::TextureView, wgpu::Texture, BindingAuditEntry) {
    let path = role.relative_path();
    let result: Option<(wgpu::Texture, wgpu::TextureView)> = (|| {
        let bytes = resources.bytes(role)?;
        let (w, h, rgba) = parse_bmp_to_rgba(bytes)?;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&path),
            size: wgpu::Extent3d {
                width: w,
                height: h,
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
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&Default::default());
        Some((texture, view))
    })();

    match result {
        Some((tex, view)) => {
            let audit = BindingAuditEntry::vanilla(
                "tree",
                binding,
                role,
                true,
                critical,
                None,
                visual_impact,
            );
            (view, tex, audit)
        }
        None => {
            let reason = resources
                .bytes(role)
                .map(|_| "bmp_parse_failed".to_string())
                .unwrap_or_else(|| "missing_resource".to_string());
            warnings.push(format!(
                "[trees_full] {} missing or invalid - using 1x1 fallback: {}",
                path, reason
            ));
            let (tex, view) = create_1x1_rgba(device, queue, fallback_rgba, false);
            let audit = BindingAuditEntry::vanilla(
                "tree",
                binding,
                role,
                false,
                critical,
                Some(reason),
                visual_impact,
            );
            (view, tex, audit)
        }
    }
}

fn load_bmp_or_fallback(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    db: &FsAssetDb,
    role: MapResRole,
    fallback_rgba: [u8; 4],
    warnings: &mut Vec<String>,
) -> (wgpu::TextureView, wgpu::Texture) {
    let path = role.relative_path();
    let result: Option<(wgpu::Texture, wgpu::TextureView)> = (|| {
        let bytes = db.open(&path).ok()?;
        let (w, h, rgba) = parse_bmp_to_rgba(&bytes)?;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&path),
            size: wgpu::Extent3d {
                width: w,
                height: h,
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
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&Default::default());
        Some((texture, view))
    })();

    match result {
        Some((tex, view)) => (view, tex),
        None => {
            warnings.push(format!(
                "[trees_full] {} missing — using 1×1 fallback",
                path
            ));
            let (tex, view) = create_1x1_rgba(device, queue, fallback_rgba, false);
            (view, tex)
        }
    }
}

fn parse_bmp_to_rgba(data: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    if data.len() < 54 {
        return None;
    }
    let offset = u32::from_le_bytes(data[10..14].try_into().ok()?) as usize;
    let w = i32::from_le_bytes(data[18..22].try_into().ok()?) as u32;
    let h_raw = i32::from_le_bytes(data[22..26].try_into().ok()?);
    let bpp = u16::from_le_bytes(data[28..30].try_into().ok()?) as usize;
    let _compression = u32::from_le_bytes(data[30..34].try_into().ok()?);
    if bpp != 8 && bpp != 24 && bpp != 32 {
        return None;
    }
    let top_down = h_raw < 0;
    let h = h_raw.unsigned_abs();
    if w == 0 || h == 0 {
        return None;
    }

    let palette_size = if bpp == 8 && offset > 54 {
        ((offset - 54) / 4).min(256)
    } else {
        0
    };
    let mut palette = [[0u8; 4]; 256];
    if palette_size > 0 {
        for i in 0..palette_size {
            let base = 54 + i * 4;
            if base + 3 < data.len() {
                palette[i] = [data[base + 2], data[base + 1], data[base], 255];
            }
        }
    }

    let row_bytes = match bpp {
        8 => ((w + 3) & !3) as usize,
        24 => (((w * 3) + 3) & !3) as usize,
        32 => (w * 4) as usize,
        _ => return None,
    };
    let mut rgba = vec![0u8; (w * h * 4) as usize];

    for row in 0..h {
        let src_row = if top_down { row } else { h - 1 - row };
        let src_off = offset + src_row as usize * row_bytes;
        for col in 0..w as usize {
            let dst = (row as usize * w as usize + col) * 4;
            match bpp {
                8 => {
                    let idx = data.get(src_off + col).copied().unwrap_or(0) as usize;
                    let c = palette.get(idx).copied().unwrap_or([0, 0, 0, 255]);
                    rgba[dst..dst + 4].copy_from_slice(&c);
                }
                24 => {
                    let so = src_off + col * 3;
                    if so + 2 < data.len() {
                        rgba[dst] = data[so + 2];
                        rgba[dst + 1] = data[so + 1];
                        rgba[dst + 2] = data[so];
                        rgba[dst + 3] = 255;
                    }
                }
                32 => {
                    let so = src_off + col * 4;
                    if so + 3 < data.len() {
                        rgba[dst] = data[so + 2];
                        rgba[dst + 1] = data[so + 1];
                        rgba[dst + 2] = data[so];
                        rgba[dst + 3] = data[so + 3];
                    }
                }
                _ => {}
            }
        }
    }
    Some((w, h, rgba))
}

fn create_1x1_rgba(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    rgba: [u8; 4],
    srgb: bool,
) -> (wgpu::Texture, wgpu::TextureView) {
    let format = if srgb {
        wgpu::TextureFormat::Rgba8UnormSrgb
    } else {
        wgpu::TextureFormat::Rgba8Unorm
    };
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("tree_fallback_1x1"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
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
            rows_per_image: Some(1),
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    let view = tex.create_view(&Default::default());
    (tex, view)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_full_instance_size_is_48_bytes() {
        assert_eq!(std::mem::size_of::<TreeFullInstance>(), 48);
    }

    #[test]
    fn tree_full_params_size_is_48_bytes() {
        assert_eq!(std::mem::size_of::<TreeFullParams>(), 48);
    }

    #[test]
    fn tree_material_system_declares_phase8_shared_bindings() {
        let specs = TreeMaterialSystem::shared_texture_specs();
        assert_eq!(specs.len(), 4);
        assert!(specs.iter().any(|spec| {
            spec.role == MapResRole::TreesMask
                && spec.component == TreeSharedMaterialComponent::Mask
                && spec.critical
        }));
        assert!(specs.iter().any(|spec| {
            spec.role == MapResRole::TreeSeason
                && spec.component == TreeSharedMaterialComponent::Season
                && spec.critical
        }));
        assert!(specs.iter().any(|spec| {
            spec.role == MapResRole::TreeTint
                && spec.component == TreeSharedMaterialComponent::Tint
                && spec.critical
        }));
        assert!(specs.iter().any(|spec| {
            spec.role == MapResRole::ColormapEmissive
                && spec.component == TreeSharedMaterialComponent::Colormap
                && spec.critical
        }));
    }

    #[test]
    fn cap_tree_instances_passthrough() {
        let v: Vec<TreeFullInstance> = (0..50)
            .map(|_| TreeFullInstance {
                pos: [0.0, 0.0, 0.0],
                scale: 1.0,
                tree_type: 0.0,
                slope_x: 0.0,
                slope_z: 0.0,
                tint_uv: [0.0, 0.0],
                season_row: 0.5,
                _pad0: 0.0,
                _pad1: 0.0,
            })
            .collect();
        let r = cap_tree_instances(v, 200);
        assert_eq!(r.len(), 50);
    }

    #[test]
    fn cap_tree_instances_subsamples() {
        let v: Vec<TreeFullInstance> = (0..100)
            .map(|i| TreeFullInstance {
                pos: [i as f32, 0.0, 0.0],
                scale: 1.0,
                tree_type: 0.0,
                slope_x: 0.0,
                slope_z: 0.0,
                tint_uv: [0.0, 0.0],
                season_row: 0.5,
                _pad0: 0.0,
                _pad1: 0.0,
            })
            .collect();
        let r = cap_tree_instances(v, 10);
        assert!(r.len() <= 10);
        assert!(!r.is_empty());
    }

    #[test]
    fn convert_instances_filters_by_type() {
        let src = vec![
            TreeInstance {
                pos: [1.0, 0.0, 0.0],
                scale: 0.02,
                tint: [255, 255, 255, 255],
                tree_type: 0,
                _pad0: 0,
                slope_x: 0,
                slope_z: 0,
            },
            TreeInstance {
                pos: [2.0, 0.0, 0.0],
                scale: 0.02,
                tint: [255, 255, 255, 255],
                tree_type: 1,
                _pad0: 0,
                slope_x: 0,
                slope_z: 0,
            },
            TreeInstance {
                pos: [3.0, 0.0, 0.0],
                scale: 0.02,
                tint: [255, 255, 255, 255],
                tree_type: 0,
                _pad0: 0,
                slope_x: 0,
                slope_z: 0,
            },
        ];
        let r = convert_instances(&src, 0, 112.0, 41.0);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].pos[0], 1.0);
        assert_eq!(r[1].pos[0], 3.0);
        assert!((r[0].tint_uv[0] - 1.0 / 112.0).abs() < 1e-6);
        assert!((r[0].season_row - 0.0).abs() < 1e-6);
    }

    #[test]
    fn trees_full_wgsl_naga_parses() {
        let composed = hoi4_render::shader_rt::compose_shader(SHADER_WGSL, true, true);
        let module = naga::front::wgsl::parse_str(&composed)
            .unwrap_or_else(|err| panic!("{}", err.emit_to_string(&composed)));
        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        );
        if let Err(err) = validator.validate(&module) {
            panic!("{}", err.emit_to_string(&composed));
        }
    }

    #[test]
    fn trees_full_wgsl_references_phase8_bindings() {
        for token in [
            "tree_mask_tex",
            "season_map_tex",
            "tint_map_tex",
            "tree_colormap_tex",
            "gradient_border_ch1",
            "gradient_border_ch2",
            "gradient_border_ch3",
            "province_secondary_color",
            "mud_snow_tex",
            "fow_tex",
            "light_data_tex",
            "light_index_tex",
            "apply_tree_snow",
            "calculate_point_lights_tree",
        ] {
            assert!(
                SHADER_WGSL.contains(token),
                "missing Phase 8 token: {token}"
            );
        }
    }
}
