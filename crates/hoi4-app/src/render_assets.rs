use wgpu::util::DeviceExt;

use hoi4_paths::PathConfig;
use hoi4_render::trees::TreeInstance;
use hoi4_render::trees_mesh::{
    build_tree_mesh, filter_instances_for_type, TreeMeshInstance, TreeMeshVertex,
};

use crate::vanilla_resource_views::{self, VanillaResourceViews};

pub(crate) fn setup_tree_mesh_pipeline(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path_cfg: &PathConfig,
    tree_instances: &[TreeInstance],
    camera_buffer: &wgpu::Buffer,
    params_buffer: &wgpu::Buffer,
    surface_format: wgpu::TextureFormat,
    depth_format: wgpu::TextureFormat,
) -> (
    wgpu::RenderPipeline,
    Vec<wgpu::BindGroup>,
    Vec<wgpu::Buffer>,
    Vec<wgpu::Buffer>,
    Vec<wgpu::Buffer>,
    Vec<u32>,
    Vec<u32>,
) {
    use hoi4_assets::{AssetDb, DdsImage, FsAssetDb, PdxMesh};

    let db = FsAssetDb::new(path_cfg.clone());

    let tree_defs: [(&str, &str); 3] = [
        (
            "gfx/models/mapitems/trees/beech.mesh",
            "gfx/models/mapitems/trees/beech_diffuse.dds",
        ),
        (
            "gfx/models/mapitems/trees/Pine_01.mesh",
            "gfx/models/mapitems/trees/pinetree_diffuse.dds",
        ),
        (
            "gfx/models/mapitems/trees/palmer.mesh",
            "gfx/models/mapitems/trees/palm_lod_diffuse.dds",
        ),
    ];

    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("trees_mesh_bgl"),
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
        ],
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        ..Default::default()
    });

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("trees_mesh_shader"),
        source: wgpu::ShaderSource::Wgsl(hoi4_render::SHADER_TREES_MESH_WGSL.into()),
    });
    let pl_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&bgl],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("trees_mesh_pipeline"),
        layout: Some(&pl_layout),
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
                    array_stride: std::mem::size_of::<TreeMeshInstance>() as u64,
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
                            format: wgpu::VertexFormat::Unorm8x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 22,
                            shader_location: 6,
                            format: wgpu::VertexFormat::Snorm8x2,
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
                format: surface_format,
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

    let mut bind_groups = Vec::new();
    let mut vertex_buffers = Vec::new();
    let mut index_buffers = Vec::new();
    let mut instance_buffers = Vec::new();
    let mut index_counts = Vec::new();
    let mut instance_counts = Vec::new();

    let make_white = || -> wgpu::TextureView {
        let t = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("w1"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &t,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255u8, 255, 255, 255],
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
        t.create_view(&Default::default())
    };

    for (type_idx, (mesh_path, tex_path)) in tree_defs.iter().enumerate() {
        let mesh_opt = db
            .open(*mesh_path)
            .ok()
            .and_then(|bytes| match PdxMesh::parse(&bytes) {
                Ok(m) => {
                    if m.meshes.is_empty() {
                        eprintln!("[trees_mesh] {} parsed OK but 0 submeshes", mesh_path);
                        None
                    } else {
                        let sub = m.meshes.into_iter().next().unwrap();
                        eprintln!(
                            "[trees_mesh] {} submesh: {} pos, {} idx",
                            mesh_path,
                            sub.positions.len(),
                            sub.indices.len()
                        );
                        build_tree_mesh(&sub.positions, &sub.normals, &sub.uvs, &sub.indices)
                    }
                }
                Err(e) => {
                    eprintln!("[trees_mesh] {} parse error: {}", mesh_path, e);
                    None
                }
            });

        let mesh_data = match mesh_opt {
            Some(d) => {
                let (mut mn, mut mx) = ([f32::MAX; 3], [f32::MIN; 3]);
                for v in &d.vertices {
                    for i in 0..3 {
                        mn[i] = mn[i].min(v.position[i]);
                        mx[i] = mx[i].max(v.position[i]);
                    }
                }
                println!(
                    "[trees_mesh] {}  ?{} verts, {} idx, bounds [{:.2},{:.2},{:.2}]???{:.2},{:.2},{:.2}]",
                    mesh_path,
                    d.vertex_count,
                    d.index_count,
                    mn[0],
                    mn[1],
                    mn[2],
                    mx[0],
                    mx[1],
                    mx[2]
                );
                d
            }
            None => {
                eprintln!("[trees_mesh] failed to load {}", mesh_path);
                let tv = make_white();
                bind_groups.push(device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &bgl,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: camera_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: params_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(&tv),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                    ],
                }));
                vertex_buffers.push(
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: None,
                        contents: &[0u8; 32],
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
                );
                index_buffers.push(
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: None,
                        contents: &[0u8; 4],
                        usage: wgpu::BufferUsages::INDEX,
                    }),
                );
                instance_buffers.push(device.create_buffer_init(
                    &wgpu::util::BufferInitDescriptor {
                        label: None,
                        contents: &[0u8; 24],
                        usage: wgpu::BufferUsages::VERTEX,
                    },
                ));
                index_counts.push(0);
                instance_counts.push(0);
                continue;
            }
        };

        let tex_view = db
            .open(*tex_path)
            .ok()
            .and_then(|bytes| DdsImage::parse(&bytes).ok())
            .map(|dds| {
                let fmt = match dds.format {
                    hoi4_assets::DdsFormat::Bc1 => wgpu::TextureFormat::Bc1RgbaUnormSrgb,
                    hoi4_assets::DdsFormat::Bc3 => wgpu::TextureFormat::Bc3RgbaUnormSrgb,
                    _ => wgpu::TextureFormat::Bc3RgbaUnormSrgb,
                };
                let valid_mips = dds
                    .mips
                    .iter()
                    .take_while(|m| m.width >= 4 && m.height >= 4)
                    .count() as u32;
                let mip_count = valid_mips.max(1);
                let tex = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("tree_tex"),
                    size: wgpu::Extent3d {
                        width: dds.width,
                        height: dds.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: mip_count,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: fmt,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                for (i, mip) in dds.mips.iter().take(mip_count as usize).enumerate() {
                    let data = &dds.data[mip.offset..mip.offset + mip.size];
                    let bw = (mip.width + 3) / 4;
                    let bpb: u32 = match dds.format {
                        hoi4_assets::DdsFormat::Bc1 => 8,
                        _ => 16,
                    };
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &tex,
                            mip_level: i as u32,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        data,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(bw * bpb),
                            rows_per_image: None,
                        },
                        wgpu::Extent3d {
                            width: mip.width,
                            height: mip.height,
                            depth_or_array_layers: 1,
                        },
                    );
                }
                println!(
                    "[trees_mesh] tex {} ({}x{} {:?} {} mips)",
                    tex_path, dds.width, dds.height, dds.format, mip_count
                );
                tex.create_view(&Default::default())
            })
            .unwrap_or_else(|| make_white());

        bind_groups.push(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&tex_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        }));

        vertex_buffers.push(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tree_vb"),
                contents: bytemuck::cast_slice(&mesh_data.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
        );
        index_buffers.push(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tree_ib"),
                contents: bytemuck::cast_slice(&mesh_data.indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
        );
        index_counts.push(mesh_data.index_count);

        let raw_instances = filter_instances_for_type(tree_instances, type_idx as u8);
        let pre_cap = raw_instances.len();
        let instances = hoi4_render::trees_mesh::cap_instances(
            raw_instances,
            hoi4_render::trees_mesh::INSTANCE_CAP_PER_TYPE,
        );
        println!(
            "[trees_mesh] type {}  ?{} instances (pre-cap {}, cap {})",
            type_idx,
            instances.len(),
            pre_cap,
            hoi4_render::trees_mesh::INSTANCE_CAP_PER_TYPE,
        );
        instance_counts.push(instances.len() as u32);
        instance_buffers.push(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tree_inst"),
                contents: if instances.is_empty() {
                    &[0u8; 24]
                } else {
                    bytemuck::cast_slice(&instances)
                },
                usage: wgpu::BufferUsages::VERTEX,
            }),
        );
    }

    (
        pipeline,
        bind_groups,
        vertex_buffers,
        index_buffers,
        instance_buffers,
        index_counts,
        instance_counts,
    )
}

pub(crate) fn load_tree_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path_cfg: &PathConfig,
) -> (wgpu::TextureView, wgpu::Sampler) {
    use hoi4_assets::{AssetDb, DdsImage, FsAssetDb};
    let db = FsAssetDb::new(path_cfg.clone());

    let tree_paths = [
        "gfx/models/mapitems/trees/beech_diffuse.dds",
        "gfx/models/mapitems/trees/pinetree_diffuse.dds",
        "gfx/models/mapitems/trees/palm_lod_diffuse.dds",
    ];

    let mut images: Vec<DdsImage> = Vec::new();
    for p in &tree_paths {
        match db.open(*p) {
            Ok(bytes) => match DdsImage::parse(&bytes) {
                Ok(d) => {
                    println!(
                        "[trees] loaded {} ({}x{} {:?})",
                        p, d.width, d.height, d.format
                    );
                    images.push(d);
                }
                Err(e) => {
                    eprintln!("[trees] failed to parse {}: {}", p, e);
                    break;
                }
            },
            Err(e) => {
                eprintln!("[trees] not found {}: {}", p, e);
                break;
            }
        }
    }

    if images.len() < 3 {
        eprintln!("[trees] using 1?? white fallback (shader will use procedural trees)");
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tree_atlas_fallback"),
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
            &[255u8, 255, 255, 255],
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
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        return (tex.create_view(&Default::default()), sampler);
    }

    let target_w: u32 = 256;
    let target_h: u32 = 256;
    let atlas_h = target_h * 3;
    let blocks_per_row = target_w / 4;
    let blocks_per_col = target_h / 4;
    let bpb: u32 = 16;
    let layer_bytes = (blocks_per_row * blocks_per_col * bpb) as usize;
    let total_bytes = layer_bytes * 3;
    let mut atlas_data = vec![0u8; total_bytes];

    for (i, img) in images.iter().enumerate() {
        let mip0 = &img.data[img.mips[0].offset..img.mips[0].offset + img.mips[0].size];
        let dst_offset = i * layer_bytes;

        if img.width == target_w && img.height == target_h {
            let copy_len = mip0.len().min(layer_bytes);
            atlas_data[dst_offset..dst_offset + copy_len].copy_from_slice(&mip0[..copy_len]);
        } else {
            let src_bw = img.width / 4;
            let src_bh = img.height / 4;
            for by in 0..blocks_per_col {
                for bx in 0..blocks_per_row {
                    let src_bx = bx % src_bw;
                    let src_by = by % src_bh;
                    let src_idx = ((src_by * src_bw + src_bx) * bpb) as usize;
                    let dst_idx = dst_offset + ((by * blocks_per_row + bx) * bpb) as usize;
                    if src_idx + bpb as usize <= mip0.len()
                        && dst_idx + bpb as usize <= atlas_data.len()
                    {
                        atlas_data[dst_idx..dst_idx + bpb as usize]
                            .copy_from_slice(&mip0[src_idx..src_idx + bpb as usize]);
                    }
                }
            }
        }
    }

    println!(
        "[trees] atlas: {}x{} BC3 (3 rows of {}x{})",
        target_w, atlas_h, target_w, target_h
    );

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("tree_atlas"),
        size: wgpu::Extent3d {
            width: target_w,
            height: atlas_h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Bc3RgbaUnormSrgb,
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
        &atlas_data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(blocks_per_row * bpb),
            rows_per_image: None,
        },
        wgpu::Extent3d {
            width: target_w,
            height: atlas_h,
            depth_or_array_layers: 1,
        },
    );

    let view = texture.create_view(&Default::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    });

    (view, sampler)
}

pub(crate) fn load_terrain_atlas_phase1(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    resources: &VanillaResourceViews,
) -> (
    wgpu::TextureView,
    wgpu::Sampler,
    vanilla_resource_views::BindingAuditEntry,
) {
    let mut warnings = Vec::new();
    let uploaded = vanilla_resource_views::upload_dds_or_fallback(
        device,
        queue,
        resources,
        vanilla_resource_views::DdsUploadRequest {
            role: hoi4_assets::MapResRole::TerrainAtlas(0),
            label: "terrain_atlas",
            fallback_rgba: [255, 255, 255, 255],
            srgb: true,
            critical: true,
            pass: "terrain",
            binding: "terrain_atlas",
            visual_impact: "terrain diffuse atlas falls back to a white texture",
        },
        &mut warnings,
    );
    for warning in warnings {
        eprintln!("{warning}");
    }
    let view = uploaded.view;
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("terrain_atlas_sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        anisotropy_clamp: 8,
        ..Default::default()
    });
    drop(uploaded.texture);
    (view, sampler, uploaded.audit)
}

pub(crate) fn load_colormap_phase1(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    resources: &VanillaResourceViews,
) -> (
    wgpu::TextureView,
    wgpu::Sampler,
    vanilla_resource_views::BindingAuditEntry,
) {
    let mut warnings = Vec::new();
    let uploaded = vanilla_resource_views::upload_dds_or_fallback(
        device,
        queue,
        resources,
        vanilla_resource_views::DdsUploadRequest {
            role: hoi4_assets::MapResRole::ColormapEmissive,
            label: "colormap",
            fallback_rgba: [128, 128, 128, 255],
            srgb: true,
            critical: true,
            pass: "terrain",
            binding: "colormap",
            visual_impact: "terrain natural color base falls back to neutral gray",
        },
        &mut warnings,
    );
    for warning in warnings {
        eprintln!("{warning}");
    }
    let view = uploaded.view;
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("colormap_sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    });
    drop(uploaded.texture);
    (view, sampler, uploaded.audit)
}

pub(crate) fn load_rivers_texture_phase1(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    resources: &VanillaResourceViews,
) -> (
    wgpu::TextureView,
    wgpu::Sampler,
    vanilla_resource_views::BindingAuditEntry,
) {
    use hoi4_assets::MapResRole;
    use hoi4_map::rivers::parse_rivers_bmp;

    let role = MapResRole::Rivers;
    let parsed = resources
        .bytes(role)
        .ok_or_else(|| "missing_resource".to_string())
        .and_then(|bytes| parse_rivers_bmp(bytes).map_err(|err| format!("bmp_parse_failed:{err}")));

    let rivers = match parsed {
        Ok(rivers) => rivers,
        Err(reason) => {
            eprintln!(
                "[rivers] {} unavailable; using empty fallback: {}",
                role.relative_path(),
                reason
            );
            let (view, sampler) = rivers_fallback(device, queue);
            return (
                view,
                sampler,
                vanilla_resource_views::BindingAuditEntry::vanilla(
                    "terrain",
                    "rivers_bmp",
                    role,
                    false,
                    false,
                    Some(reason),
                    "river mask is unavailable",
                ),
            );
        }
    };

    let (w, h) = (rivers.width, rivers.height);
    let bytes = rivers.to_rgba_level_flow();
    println!(
        "[rivers] loaded {}x{} {} river pixels",
        w,
        h,
        rivers.river_pixel_count()
    );

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("rivers_tex"),
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
        &bytes,
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
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("rivers_sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    });
    drop(texture);
    (
        view,
        sampler,
        vanilla_resource_views::BindingAuditEntry::vanilla(
            "terrain",
            "rivers_bmp",
            role,
            true,
            false,
            None,
            "river mask and river pass visibility",
        ),
    )
}

#[allow(dead_code)]
pub(crate) fn load_terrain_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path_cfg: &PathConfig,
) -> (wgpu::TextureView, wgpu::Sampler) {
    use hoi4_assets::{AssetDb, DdsImage, FsAssetDb};
    let db = FsAssetDb::new(path_cfg.clone());

    let atlas_paths = [
        "map/terrain/atlas0.dds",
        "map/terrain/atlas1.dds",
        "map/terrain/atlas2.dds",
    ];

    let mut loaded: Option<DdsImage> = None;
    let mut chosen_path = "";
    for p in &atlas_paths {
        if let Ok(bytes) = db.open(*p) {
            if let Ok(d) = DdsImage::parse(&bytes) {
                chosen_path = *p;
                loaded = Some(d);
                break;
            }
        }
    }

    let dds = match loaded {
        Some(d) => d,
        None => {
            eprintln!("[terrain] no atlas found, using 1?? white fallback");
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("terrain_atlas_fallback"),
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
                &[255u8, 255, 255, 255],
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
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
            return (tex.create_view(&Default::default()), sampler);
        }
    };

    println!(
        "[terrain] loaded {} ({}x{} {:?} mips={})",
        chosen_path,
        dds.width,
        dds.height,
        dds.format,
        dds.mip_count()
    );

    let wgpu_fmt = match dds.format {
        hoi4_assets::DdsFormat::Bc1 => wgpu::TextureFormat::Bc1RgbaUnormSrgb,
        hoi4_assets::DdsFormat::Bc3 => wgpu::TextureFormat::Bc3RgbaUnormSrgb,
        hoi4_assets::DdsFormat::Bc5 => wgpu::TextureFormat::Bc5RgUnorm,
        hoi4_assets::DdsFormat::Bgra8 => wgpu::TextureFormat::Bgra8UnormSrgb,
        _ => wgpu::TextureFormat::Bgra8UnormSrgb,
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("terrain_atlas"),
        size: wgpu::Extent3d {
            width: dds.width,
            height: dds.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: dds.mip_count(),
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu_fmt,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    for (i, mip) in dds.mips.iter().enumerate() {
        let data = &dds.data[mip.offset..mip.offset + mip.size];
        let block_dim = if matches!(dds.format, hoi4_assets::DdsFormat::Bgra8) {
            1u32
        } else {
            4u32
        };
        let blocks_wide = (mip.width + block_dim - 1) / block_dim;
        let blocks_tall = (mip.height + block_dim - 1) / block_dim;
        let bpb = match dds.format {
            hoi4_assets::DdsFormat::Bc1 => 8u32,
            hoi4_assets::DdsFormat::Bc3 | hoi4_assets::DdsFormat::Bc5 => 16,
            _ => 4,
        };
        let copy_w = blocks_wide * block_dim;
        let copy_h = blocks_tall * block_dim;
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
                width: copy_w,
                height: copy_h,
                depth_or_array_layers: 1,
            },
        );
    }

    let view = texture.create_view(&Default::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("terrain_atlas_sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        anisotropy_clamp: 8,
        ..Default::default()
    });

    drop(texture);

    (view, sampler)
}

#[allow(dead_code)]
pub(crate) fn load_colormap(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path_cfg: &PathConfig,
) -> (wgpu::TextureView, wgpu::Sampler) {
    use hoi4_assets::{AssetDb, DdsImage, FsAssetDb, MapResRole};
    let db = FsAssetDb::new(path_cfg.clone());

    let colormap_emissive = MapResRole::ColormapEmissive.relative_path();
    let colormap_paths = [
        colormap_emissive.as_str(),
        "map/terrain/colormap.dds",
        "map/terrain/colormap_rgb.dds",
    ];

    let mut loaded: Option<DdsImage> = None;
    let mut chosen_path = "";
    for p in &colormap_paths {
        if let Ok(bytes) = db.open(*p) {
            if let Ok(d) = DdsImage::parse(&bytes) {
                chosen_path = *p;
                loaded = Some(d);
                break;
            }
        }
    }

    let dds = match loaded {
        Some(d) => d,
        None => {
            eprintln!("[terrain] no colormap found, using 1?? neutral fallback");
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("colormap_fallback"),
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
                &[128u8, 128, 128, 255],
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
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            });
            return (tex.create_view(&Default::default()), sampler);
        }
    };

    println!(
        "[terrain] loaded colormap {} ({}x{} {:?} mips={})",
        chosen_path,
        dds.width,
        dds.height,
        dds.format,
        dds.mip_count()
    );

    let wgpu_fmt = match dds.format {
        hoi4_assets::DdsFormat::Bc1 => wgpu::TextureFormat::Bc1RgbaUnormSrgb,
        hoi4_assets::DdsFormat::Bc3 => wgpu::TextureFormat::Bc3RgbaUnormSrgb,
        hoi4_assets::DdsFormat::Bc5 => wgpu::TextureFormat::Bc5RgUnorm,
        hoi4_assets::DdsFormat::Bgra8 => wgpu::TextureFormat::Bgra8UnormSrgb,
        _ => wgpu::TextureFormat::Bgra8UnormSrgb,
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("colormap"),
        size: wgpu::Extent3d {
            width: dds.width,
            height: dds.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: dds.mip_count(),
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu_fmt,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    for (i, mip) in dds.mips.iter().enumerate() {
        let data = &dds.data[mip.offset..mip.offset + mip.size];
        let block_dim = if matches!(dds.format, hoi4_assets::DdsFormat::Bgra8) {
            1u32
        } else {
            4u32
        };
        let blocks_wide = (mip.width + block_dim - 1) / block_dim;
        let blocks_tall = (mip.height + block_dim - 1) / block_dim;
        let bpb = match dds.format {
            hoi4_assets::DdsFormat::Bc1 => 8u32,
            hoi4_assets::DdsFormat::Bc3 | hoi4_assets::DdsFormat::Bc5 => 16,
            _ => 4,
        };
        let copy_w = blocks_wide * block_dim;
        let copy_h = blocks_tall * block_dim;
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
                width: copy_w,
                height: copy_h,
                depth_or_array_layers: 1,
            },
        );
    }

    let view = texture.create_view(&Default::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("colormap_sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    });

    drop(texture);

    (view, sampler)
}

#[allow(dead_code)]
pub(crate) fn load_rivers_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path_cfg: &PathConfig,
) -> (wgpu::TextureView, wgpu::Sampler) {
    use hoi4_map::rivers::load_rivers_bmp;

    let path = path_cfg.find("map/rivers.bmp");
    let rivers = match path.as_ref().map(|p| load_rivers_bmp(p)) {
        Some(Ok(r)) => r,
        Some(Err(e)) => {
            eprintln!("[rivers] parse error: {} ???using empty fallback", e);
            return rivers_fallback(device, queue);
        }
        None => {
            eprintln!("[rivers] map/rivers.bmp not found ???using empty fallback");
            return rivers_fallback(device, queue);
        }
    };

    let (w, h) = (rivers.width, rivers.height);
    let bytes = rivers.to_rgba_level_flow();
    println!(
        "[rivers] loaded {}x{}  ?{} river pixels (level???)",
        w,
        h,
        rivers.river_pixel_count()
    );

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("rivers_tex"),
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
        &bytes,
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
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("rivers_sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    });
    drop(texture);
    (view, sampler)
}

pub(crate) fn rivers_fallback(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::TextureView, wgpu::Sampler) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("rivers_fallback"),
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
        &[0u8, 128, 255, 255],
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
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
    (tex.create_view(&Default::default()), sampler)
}
