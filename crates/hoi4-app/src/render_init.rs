use super::*;
use hoi4_render::map_mode::build_occupation_lut;
use hoi4_render::trees::TreeInstance;
use wgpu::util::DeviceExt;

impl App {
    pub(crate) fn init_render(&mut self, window: Arc<Window>) {
        let size = window.inner_size();
        // 4.1.bis.6: HiDPI handling. winit's `inner_size` returns physical pixels;
        // GUI layout (and our 2D shaders) must run in logical pixels so absolute
        // coordinates from `.gui` files (designed for 1920脳1080) resolve to the
        // right on-screen size regardless of the OS DPI scale.
        let dpi = self.ui_state.settings.display_scale.max(0.0001);
        let logical_w = size.width as f32 / dpi;
        let logical_h = size.height as f32 / dpi;
        // logical_w / logical_h are consumed when we build UI-pass-removed / TextPass / PanelPass below.
        println!(
            "[init] window physical={}x{} logical={:.0}x{:.0} game_scale={:.2} system_dpi={:.2}",
            size.width,
            size.height,
            logical_w,
            logical_h,
            dpi,
            window.scale_factor()
        );
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone()).unwrap();

        let (adapter, device, queue) = pollster::block_on(async {
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    compatible_surface: Some(&surface),
                    ..Default::default()
                })
                .await
                .unwrap();
            let supported = adapter.features();
            let want = wgpu::Features::TEXTURE_COMPRESSION_BC
                | wgpu::Features::TEXTURE_FORMAT_16BIT_NORM
                | wgpu::Features::TIMESTAMP_QUERY
                | wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES
                | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
            let required = wgpu::Features::TEXTURE_COMPRESSION_BC;
            let features = supported & want | required;
            if !features.contains(wgpu::Features::TEXTURE_FORMAT_16BIT_NORM) {
                eprintln!(
                    "[gpu] TEXTURE_FORMAT_16BIT_NORM not supported, heightmap will use R8Unorm fallback"
                );
            }
            if supported.contains(wgpu::Features::TIMESTAMP_QUERY) {
                eprintln!(
                    "[gpu] TIMESTAMP_QUERY supported; Phase 10 timing overlay will use GPU timestamps when pass/encoder writes are available"
                );
            } else {
                eprintln!(
                    "[gpu] TIMESTAMP_QUERY not supported; Phase 10 overlay will show CPU/draw-call budgets only"
                );
            }
            let required_limits = parity_required_limits(adapter.limits());
            if required_limits.max_sampled_textures_per_shader_stage
                < MIN_FRAGMENT_SAMPLED_TEXTURES_FOR_PARITY
            {
                eprintln!(
                    "[gpu] adapter only supports {} sampled textures per shader stage; WaterPass Phase 6 requires {} and may be disabled by validation",
                    required_limits.max_sampled_textures_per_shader_stage,
                    MIN_FRAGMENT_SAMPLED_TEXTURES_FOR_PARITY
                );
            }
            let (device, queue) = adapter
                .request_device(
                    &wgpu::DeviceDescriptor {
                        required_features: features,
                        required_limits,
                        ..Default::default()
                    },
                    None,
                )
                .await
                .unwrap();
            (adapter, device, queue)
        });
        self.heightmap_r16_supported = device
            .features()
            .contains(wgpu::Features::TEXTURE_FORMAT_16BIT_NORM);

        let caps = surface.get_capabilities(&adapter);
        let format = select_captured_restore_surface_format(&caps.formats);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            // The app drives its own 60 Hz redraw cadence. Avoid blocking CPU
            // submission on vblank; high-DPI/high-quality frames otherwise show
            // up as 33/50 ms present stalls when one refresh interval is missed.
            present_mode: wgpu::PresentMode::AutoNoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 1,
        };
        surface.configure(&device, &config);

        // Province index texture (R16Uint)
        let province_tex = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("province_map"),
                size: wgpu::Extent3d {
                    width: self.world.map.province_map.width,
                    height: self.world.map.province_map.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R16Uint,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            bytemuck::cast_slice(&self.world.map.province_map.pixels),
        );
        let province_view = province_tex.create_view(&Default::default());

        let terrain_flags = self.world.map.terrain_catalog.terrain_flags_array_256();
        let water_mask_pixels: Vec<u8> = self
            .world
            .map
            .province_map
            .pixels
            .iter()
            .enumerate()
            .map(|(idx, &pid)| {
                let province_water = self
                    .world
                    .map
                    .definitions
                    .get(pid as usize)
                    .and_then(|def| def.as_ref())
                    .map(|def| {
                        matches!(
                            def.province_type,
                            hoi4_map::ProvinceType::Sea | hoi4_map::ProvinceType::Lake
                        )
                    });
                let terrain_water = self
                    .world
                    .map
                    .terrain_bmp
                    .pixels
                    .get(idx)
                    .map(|&terrain_idx| (terrain_flags[terrain_idx as usize] & 2) != 0)
                    .unwrap_or(false);
                if province_water.unwrap_or(terrain_water) {
                    255
                } else {
                    0
                }
            })
            .collect();
        let water_mask_tex = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("water_mask"),
                size: wgpu::Extent3d {
                    width: self.world.map.province_map.width,
                    height: self.world.map.province_map.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Uint,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &water_mask_pixels,
        );
        let water_mask_view = water_mask_tex.create_view(&Default::default());

        // Heightmap texture. Phase 11.3: upgraded from R8Unorm to R16Unorm when
        // the GPU supports TEXTURE_FORMAT_16BIT_NORM. The source BMP is 8-bit;
        // each pixel is upcast to 16-bit (value << 8) to eliminate the 1/256
        // stepping that causes visible terraces on flat terrain at height_scale=4.0.
        let height_view = if self.heightmap_r16_supported {
            let heightmap_r16: Vec<u16> = self
                .world
                .map
                .heightmap
                .pixels
                .iter()
                .map(|&b| (b as u16) << 8)
                .collect();
            let height_tex = device.create_texture_with_data(
                &queue,
                &wgpu::TextureDescriptor {
                    label: Some("heightmap"),
                    size: wgpu::Extent3d {
                        width: self.world.map.heightmap.width,
                        height: self.world.map.heightmap.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R16Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                bytemuck::cast_slice(&heightmap_r16),
            );
            height_tex.create_view(&Default::default())
        } else {
            let height_tex = device.create_texture_with_data(
                &queue,
                &wgpu::TextureDescriptor {
                    label: Some("heightmap"),
                    size: wgpu::Extent3d {
                        width: self.world.map.heightmap.width,
                        height: self.world.map.heightmap.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                &self.world.map.heightmap.pixels,
            );
            height_tex.create_view(&Default::default())
        };

        // Terrain index texture (R8Uint) - per-pixel terrain.bmp index.
        let terrain_idx_tex = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("terrain_idx"),
                size: wgpu::Extent3d {
                    width: self.world.map.terrain_bmp.width,
                    height: self.world.map.terrain_bmp.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Uint,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &self.world.map.terrain_bmp.pixels,
        );
        let terrain_idx_view = terrain_idx_tex.create_view(&Default::default());

        // 鈹€鈹€鈹€ 5.4 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
        // SDF distance fields for borders. Computed once at startup.
        println!("Computing border SDFs (Chamfer 2-pass)...");
        let t_sdf = Instant::now();
        let country_sdf_data = compute_country_sdf(
            &self.world.map.province_map,
            &self.world.provinces.controllers,
        );
        let province_sdf_data = compute_province_sdf(&self.world.map.province_map);
        // 3.12.18 (2026-05-18): keep coast SDF in **raw pixel units**, not
        // normalized to its max. The shaders multiply `sample.r * 255` to
        // recover pixel distance ???that convention only works if the u8
        // value already *is* pixel distance (as country/province SDFs are).
        // Normalizing inflated `coast_dist_px` by `255 / coast_max`, which
        // turned `foam_band = 5 px` into a 30-50 px white ring at coasts
        // and produced the chunky "white edge" seen in zoom-out screenshots.
        let coast_sdf_data = compute_coast_sdf(&self.world.map.heightmap, 95);
        println!("  SDFs done in {:.2}s", t_sdf.elapsed().as_secs_f32());
        let map_w = self.world.map.province_map.width;
        let map_h = self.world.map.province_map.height;

        let coast_sdf_tex = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("coast_sdf"),
                size: wgpu::Extent3d {
                    width: map_w,
                    height: map_h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &coast_sdf_data,
        );
        let coast_sdf_view = coast_sdf_tex.create_view(&Default::default());

        // Phase 3.5: Load vanilla terrain atlas (map/terrain/atlas0.dds  ?2048x2048 BC3, 4脳4 tiles)
        let vanilla_resources = VanillaResourceViews::load_for_audit(&self.path_cfg);
        let mut binding_audit = vanilla_resource_views::BindingAudit::new();
        let vanilla_targets = VanillaRuntimeTargets::new(
            &device,
            &queue,
            VanillaRuntimeTargetInputs {
                world: &self.world,
                country_sdf: &country_sdf_data,
                province_sdf: &province_sdf_data,
                coast_sdf: &coast_sdf_data,
                world_scale: WORLD_SCALE,
                height_scale: HEIGHT_SCALE,
                default_map_mode_code: crate::vanilla_targets::province_secondary::map_mode_code(
                    self.map_mode,
                ),
            },
        );
        binding_audit
            .extend(vanilla_targets.binding_audit_entries_for_pass("projected_fow_shadow"));
        let (terrain_atlas_view, _terrain_atlas_sampler, terrain_atlas_audit) =
            load_terrain_atlas_phase1(&device, &queue, &vanilla_resources);
        binding_audit.extend([terrain_atlas_audit]);

        // Phase 3.6.1: Load colormap (map/terrain/colormap.dds - continent natural color base)
        let (colormap_view, _colormap_sampler, colormap_audit) =
            load_colormap_phase1(&device, &queue, &vanilla_resources);
        binding_audit.extend([colormap_audit]);

        // Phase 7: Load rivers.bmp as RGBA level/flow texture. Terrain reads
        // the R channel for fallback/debug; RiverPass reads R/G/B/A for
        // visibility, stable animation direction, and palette-index debug.
        let (rivers_view, _rivers_sampler, rivers_audit) =
            load_rivers_texture_phase1(&device, &queue, &vanilla_resources);
        binding_audit.extend([rivers_audit]);

        // Occupation overlay LUT (same layout as colour LUT but holds stripe colour + alpha).
        let occ_lut_data = build_occupation_lut(&self.world);
        // Use the same 256-wide layout as the colour LUT.
        let occ_lut_width: u32 = 256;
        let occ_lut_height: u32 =
            (occ_lut_data.len() as u32 / 4 + occ_lut_width - 1) / occ_lut_width;
        let occupation_lut_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("occupation_lut"),
            size: wgpu::Extent3d {
                width: occ_lut_width,
                height: occ_lut_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let mut occ_padded = occ_lut_data;
        occ_padded.resize((occ_lut_width * occ_lut_height * 4) as usize, 0);
        upload_lut(
            &queue,
            &occupation_lut_texture,
            &occ_padded,
            occ_lut_width,
            occ_lut_height,
        );
        let occupation_lut_view = occupation_lut_texture.create_view(&Default::default());

        // RenderParams uniform buffer.
        let initial_params = RenderParams::new();
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("render_params"),
            contents: bytemuck::bytes_of(&initial_params),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Color LUT - 2D texture, 256 wide
        let player_cid = if self.view.player_country < self.world.countries.count {
            Some(hoi4_state::CountryId(self.view.player_country as u16))
        } else {
            None
        };
        let lut_data = build_color_lut(&self.world, self.map_mode, player_cid);
        let lut_width: u32 = 256;
        let lut_height: u32 = (lut_data.len() as u32 / 4 + lut_width - 1) / lut_width;
        let mut padded_lut = lut_data;
        padded_lut.resize((lut_width * lut_height * 4) as usize, 0);

        let lut_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("color_lut"),
            size: wgpu::Extent3d {
                width: lut_width,
                height: lut_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        upload_lut(&queue, &lut_texture, &padded_lut, lut_width, lut_height);
        let lut_view = lut_texture.create_view(&Default::default());

        // Camera uniform
        // 4.1.bis.6 fix: aspect is unitless so logical vs physical math is the
        // same ???but use a single source (logical) to avoid future mismatches.
        self.camera.aspect = logical_w / logical_h.max(1.0);
        let cam_uniform = CameraUniform::from_camera(&self.camera, HEIGHT_SCALE, LAT_CORRECTION);
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("camera"),
            contents: bytemuck::bytes_of(&cam_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Per-LOD chunk uniforms (just hold the grid count).
        let chunk_uniforms: [wgpu::Buffer; 3] = std::array::from_fn(|i| {
            let u = ChunkUniform {
                grid: LOD_GRID[i],
                _pad: [0; 3],
            };
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("chunk_uniform"),
                contents: bytemuck::bytes_of(&u),
                usage: wgpu::BufferUsages::UNIFORM,
            })
        });

        // Instance buffer (initial capacity = total chunks).
        let total_chunks = (CHUNKS_X * CHUNKS_Z) as u64;
        let instance_buffers: [wgpu::Buffer; 3] = std::array::from_fn(|_| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("chunk_instances"),
                size: total_chunks * std::mem::size_of::<ChunkInstance>() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });

        // Phase 3.12.3 ???shadow caster pipeline銆傚???camera_buffer + heightmap +
        // ???LOD chunk_uniform锛屽啓鍒颁竴寮犱笓???D32Float 2048脳2048 depth RT??
        let shadow_pass = passes::ShadowPass::new(
            &device,
            format,
            &camera_buffer,
            &height_view,
            &chunk_uniforms,
        );

        // Pipeline (with depth-stencil)
        let depth_format = wgpu::TextureFormat::Depth24PlusStencil8;
        let depth_view =
            make_depth_view(&device, size.width.max(1), size.height.max(1), depth_format);

        // 鈹€鈹€鈹€ Phase 3.12.4 ???pdxmap-equivalent TerrainPass 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
        // 鎶婂垰鎵嶅垱寤虹殑 19 涓棫 binding 瑙嗗浘浣滀负杈撳叆鍐嶆缁勭粐???vanilla pdxmap ???        // 3-bind-group 甯冨眬锛涘悓鏃舵寜 `MapResRole` 鍔犺浇 atlas_normal{0} +
        // world_normal.bmp + colormap_emissive + citylights_0 ???4 寮犳柊璐村浘???        // 褰撲换涓€鍔犺浇澶辫触鏃惰 pass 浼氳嚜鍔ㄧ敤 1脳1 fallback鈥斺€旀瀯閫犳案杩滄垚鍔??
        let global_uniform_buf = GlobalUniformBuffer::new(&device);
        let terrain_pass = {
            let inputs = passes::TerrainPassInputs {
                global_uniform_buffer: &global_uniform_buf.buffer,
                depth_format,
                lod_grid: LOD_GRID,
                shadow_map_view: &vanilla_targets.projected_shadow_fow.view,
                shadow_sampler: &shadow_pass.compare_sampler,
                colormap_view: &colormap_view,
                coast_sdf_view: &coast_sdf_view,
                occupation_lut_view: &occupation_lut_view,
                rivers_view: &rivers_view,
                heightmap_view: &height_view,
                province_view: &province_view,
                water_mask_view: &water_mask_view,
                terrain_idx_view: &terrain_idx_view,
                terrain_atlas_view: &terrain_atlas_view,
                country_color_lut_view: &lut_view,
                vanilla_resources: &vanilla_resources,
                runtime_targets: &vanilla_targets,
            };
            let terrain_pass = passes::TerrainPass::new(&device, &queue, &self.path_cfg, inputs);
            for w in &terrain_pass.load_warnings {
                println!("{}", w);
            }
            binding_audit.extend(terrain_pass.binding_audit.entries.clone());
            terrain_pass
        };

        // Build chunk grid from heightmap.
        let world_size = self.world_size();
        let chunk_grid = ChunkGrid::build(
            &self.world.map.heightmap,
            world_size,
            HEIGHT_SCALE,
            CHUNKS_X,
            CHUNKS_Z,
        );

        // 鈹€鈹€鈹€ 5.6 trees 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
        println!("Generating tree instances...");
        let t_trees = Instant::now();
        // Phase 3.10.2: use map/trees.bmp + default.map's `tree = {3,4,7,10}`
        // when present; fall back to the legacy terrain.bmp routing only if
        // trees.bmp failed to load.
        let tree_data = if let Some(tree_bmp) = &self.world.map.tree_definition_bmp {
            let (trees, stats) = generate_trees_with_stats(
                tree_bmp,
                &self.world.map.tree_indices,
                &self.world.map.heightmap,
                WORLD_SCALE,
                HEIGHT_SCALE,
                3, // forest stride on trees.bmp (1650 wide)
                2, // jungle stride
            );
            println!(
                "[trees] trees.bmp {}x{} active={} generated={} ratio={:.3} by_type={:?} generated_by_type={:?} stride_skip={} sea_skip={}",
                stats.tree_bitmap_size[0],
                stats.tree_bitmap_size[1],
                stats.active_pixels_total,
                stats.generated_instances_total,
                stats.placement_ratio(),
                stats.active_pixels_by_type,
                stats.generated_instances_by_type,
                stats.skipped_by_stride,
                stats.skipped_below_sea
            );
            trees
        } else {
            eprintln!("[trees] trees.bmp unavailable ???placing 0 trees");
            Vec::new()
        };
        println!(
            "  {} trees generated in {:.2}s",
            tree_data.len(),
            t_trees.elapsed().as_secs_f32()
        );
        let trees_count = tree_data.len() as u32;
        let trees_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tree_instances"),
            contents: bytemuck::cast_slice(&tree_data),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Trees use camera + render params (procedural billboard, no texture needed).
        let trees_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("trees_bgl"),
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
        let trees_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("trees_bg"),
            layout: &trees_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        });

        let trees_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("trees_shader"),
            source: wgpu::ShaderSource::Wgsl(hoi4_render::SHADER_TREES_WGSL.into()),
        });
        let trees_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("trees_pipeline_layout"),
                bind_group_layouts: &[&trees_bgl],
                push_constant_ranges: &[],
            });
        let trees_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("trees_pipeline"),
            layout: Some(&trees_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &trees_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<TreeInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        // pos: vec3<f32> @ offset 0
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        // scale: f32 @ offset 12
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32,
                            offset: 12,
                            shader_location: 1,
                        },
                        // tint: 4xu8 -> vec4<f32> via Unorm8x4 @ offset 16
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Unorm8x4,
                            offset: 16,
                            shader_location: 2,
                        },
                        // Phase 3.10.2: tree_type + pad as Uint8x2 @ offset 20.
                        // Shader reads `.x` for the species index.
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Uint8x2,
                            offset: 20,
                            shader_location: 3,
                        },
                        // Phase 3.10.2: slope (Snorm8x2) @ offset 22.
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Snorm8x2,
                            offset: 22,
                            shader_location: 4,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &trees_shader,
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
                format: depth_format,
                depth_write_enabled: false, // alpha-blended - no depth write
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        // 鈹€鈹€鈹€ 5.6 railways 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
        println!("Generating railway vertices...");
        let rail_path = self
            .path_cfg
            .find("map/railways.txt")
            .unwrap_or_else(|| self.path_cfg.game_path().join("map/railways.txt"));
        let rail_text = std::fs::read_to_string(&rail_path).unwrap_or_default();
        let routes = parse_railways(&rail_text);
        let centroids = compute_province_centroids(&self.world.map.province_map);

        // Phase 3.5: Pre-compute per-country world-space label centroids.
        // O(num_provinces) one-shot; reused every frame for screen projection.
        let country_count = self.world.countries.count;
        let owners_for_labels: Vec<Option<usize>> = self
            .world
            .provinces
            .owners
            .iter()
            .map(|c| {
                if c.is_none() {
                    None
                } else {
                    Some(c.0 as usize)
                }
            })
            .collect();
        let country_labels = hoi4_render::mapname::compute_country_labels(
            &centroids,
            &owners_for_labels,
            country_count,
            WORLD_SCALE,
        );
        let labelled = country_labels.iter().filter(|l| l.is_some()).count();
        println!(
            "[mapname] computed {} country labels ({}/{} countries with owned provinces)",
            labelled, labelled, country_count
        );

        // Phase 3.10.3: per-country oriented bounding box (PCA over owned-
        // province pixels). Drives the 3D label quad's orientation + size.
        // O(map pixels), once at startup.
        let t_obb = Instant::now();
        let province_is_core: Vec<bool> = (0..self.world.provinces.count)
            .map(|pid| {
                let owner = self.world.provinces.owners[pid];
                if owner.is_none() {
                    return false;
                }
                let sid = self.world.provinces.state_of[pid];
                if sid.is_none() {
                    return false;
                }
                let si = sid.0 as usize;
                si < self.world.states.cores.len() && self.world.states.cores[si].contains(&owner)
            })
            .collect();
        let country_obbs = hoi4_render::mapname_3d::compute_country_obbs(
            &self.world.map.province_map,
            &owners_for_labels,
            &province_is_core,
            country_count,
        );
        let n_obb = country_obbs.iter().filter(|o| o.is_some()).count();
        println!(
            "[mapname_3d] OBB pass: {} countries with valid OBB in {:.2}s",
            n_obb,
            t_obb.elapsed().as_secs_f32()
        );

        // Phase 5 map parity: use player-facing country names, not three-letter tags.
        // Map labels prefer the same tag-level names used by country profiles,
        // then fall back to vanilla localisation for tags missing in our table.
        let t_atlas = Instant::now();
        let map_label_language = self.ui_state.settings.language;
        let country_loc = load_country_label_loc_catalog(&self.path_cfg, map_label_language);
        let english_country_loc =
            load_country_label_loc_catalog(&self.path_cfg, hoi4_ui::i18n::Language::English);
        let names: Vec<Option<String>> = self
            .world
            .countries
            .tags
            .iter()
            .enumerate()
            .map(|(idx, tag)| {
                if tag.is_empty() {
                    None
                } else {
                    let ruling_party = self
                        .world
                        .countries
                        .ruling_party
                        .get(idx)
                        .map(String::as_str)
                        .unwrap_or_default();
                    Some(map_country_label(
                        tag,
                        ruling_party,
                        map_label_language,
                        &country_loc,
                        &english_country_loc,
                    ))
                }
            })
            .collect();
        let mapname_atlas_opt =
            mapname_atlas::bake_country_name_atlas_with_paths(&names, 48.0, Some(&self.path_cfg));
        if let Some(atlas) = &mapname_atlas_opt {
            println!(
                "[mapname_3d] atlas: {}x{} R8, {} entries baked in {:.2}s, font={}",
                atlas.width,
                atlas.height,
                atlas.count_baked(),
                t_atlas.elapsed().as_secs_f32(),
                atlas.font_source
            );
        } else {
            eprintln!("[mapname_3d] atlas bake failed (no system font?); 2D HUD fallback active");
        }

        let rail_rivers = vanilla_resources
            .bytes(hoi4_assets::MapResRole::Rivers)
            .and_then(|bytes| match hoi4_map::rivers::parse_rivers_bmp(bytes) {
                Ok(rivers) => Some(rivers),
                Err(err) => {
                    eprintln!(
                        "[railways] map/rivers.bmp parse failed; bridge markers disabled: {err}"
                    );
                    None
                }
            });
        let (rail_verts, railway_bridge_count) = build_railway_vertices_with_bridges(
            &routes,
            &centroids,
            &self.world.map.heightmap,
            rail_rivers.as_ref(),
            WORLD_SCALE,
            HEIGHT_SCALE,
            0.06, // lift above terrain
        );
        println!(
            "  {} railway segments ({} routes, {} bridge markers)",
            rail_verts.len() / 2,
            routes.len(),
            railway_bridge_count
        );
        let railways_vertex_count = rail_verts.len() as u32;
        let railways_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("railway_verts"),
            contents: if rail_verts.is_empty() {
                &[0u8; 16]
            } else {
                bytemuck::cast_slice(&rail_verts)
            },
            usage: wgpu::BufferUsages::VERTEX,
        });

        let railways_params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("railway_params"),
            contents: bytemuck::bytes_of(&RailwayParams::default()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let frontlines_params_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("frontline_params"),
                contents: bytemuck::bytes_of(&FrontlineParams {
                    opacity: 1.0,
                    _pad: [0.0; 3],
                }),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        // Railways/frontlines use the shared camera uniform plus per-frame opacity.
        let rail_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rail_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
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
        let frontlines_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frontline_bg"),
            layout: &rail_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: frontlines_params_buffer.as_entire_binding(),
                },
            ],
        });
        let rail_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rail_bg"),
            layout: &rail_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: railways_params_buffer.as_entire_binding(),
                },
            ],
        });
        let rail_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rail_shader"),
            source: wgpu::ShaderSource::Wgsl(hoi4_render::SHADER_RAILWAYS_WGSL.into()),
        });
        let rail_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&rail_bgl],
            push_constant_ranges: &[],
        });
        let railways_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("railways_pipeline"),
            layout: Some(&rail_pl),
            vertex: wgpu::VertexState {
                module: &rail_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<RailVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32,
                            offset: 12,
                            shader_location: 1,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &rail_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: 1,
                    slope_scale: 0.5,
                    clamp: 0.0,
                },
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        // 鈹€鈹€鈹€ 5.7 frontlines 鈹€鈹€鈹€鈹€鈹€
        println!("Generating frontlines...");
        let front_verts =
            generate_frontline_vertices(&self.world, &centroids, WORLD_SCALE, HEIGHT_SCALE);
        println!("  {} frontline border quads", front_verts.len() / 6);

        // Phase 2.9: Buildings - data generated, now with own pipeline (Phase 3.5).
        let building_instances =
            generate_buildings(&self.world, &centroids, WORLD_SCALE, HEIGHT_SCALE);
        let buildings_count = building_instances.len() as u32;
        let buildings_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("buildings"),
            contents: if building_instances.is_empty() {
                &[0u8; 16]
            } else {
                bytemuck::cast_slice(&building_instances)
            },
            usage: wgpu::BufferUsages::VERTEX,
        });
        println!("  {} building icons", buildings_count);

        let buildings_params_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("building_params"),
                contents: bytemuck::bytes_of(&BuildingParams::default()),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let buildings_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("buildings_bg"),
            layout: &rail_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buildings_params_buffer.as_entire_binding(),
                },
            ],
        });

        // Buildings pipeline (16-byte per-instance: [f32;3] + f32)
        let buildings_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("buildings_shader"),
            source: wgpu::ShaderSource::Wgsl(hoi4_render::SHADER_BUILDINGS_WGSL.into()),
        });
        let buildings_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("buildings_pipeline"),
            layout: Some(&rail_pl),
            vertex: wgpu::VertexState {
                module: &buildings_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 16, // [f32;3] + f32 = 16 bytes
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &buildings_shader,
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
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        // 鈹€鈹€鈹€ Phase I (CR-1.2 / CR-5) 鈥?HOI3 椋庢牸灞忓箷绌洪棿鍏电墝 pass锛堥粯璁ゅ惎鐢級 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
        let hoi3_counter_pass = Hoi3CounterPass::new(
            &device,
            &queue,
            HDR_FORMAT,
            depth_format,
            Hoi3CounterPass::DEFAULT_INITIAL_CAPACITY,
        );
        println!(
            "[hoi3_counter_v3] capacity = {}, enabled = {} (toggle: F8)",
            Hoi3CounterPass::DEFAULT_INITIAL_CAPACITY,
            hoi3_counter_pass.enabled()
        );

        // 鈹€鈹€鈹€ Phase 14 ???POI icon pass (factories / ports / airbases / resources) 鈹€
        let poi_icon_instances =
            generate_poi_icons(&self.world, &centroids, WORLD_SCALE, HEIGHT_SCALE);
        let poi_icon_pass = {
            let poi_instances = &poi_icon_instances;
            println!("  {} POI icon instances", poi_instances.len());
            let mut pass = PoiIconPass::new(
                &device,
                &queue,
                HDR_FORMAT,
                depth_format,
                &camera_buffer,
                poi_instances.len().next_power_of_two().max(64) as u32,
            );
            for w in &pass.load_warnings {
                println!("{}", w);
            }
            pass.upload(&device, &queue, poi_instances, 3);
            println!(
                "[poi_icon] enabled = {}, {} instances uploaded",
                pass.enabled(),
                pass.instance_count()
            );
            Some(pass)
        };

        // Frontlines pipeline - actual contact-border strip triangles with per-vertex colour.
        let frontlines_vertex_count = front_verts.len() as u32;
        let frontlines_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("frontline_verts"),
            contents: if front_verts.is_empty() {
                &[0u8; 16]
            } else {
                bytemuck::cast_slice(&front_verts)
            },
            usage: wgpu::BufferUsages::VERTEX,
        });
        let front_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("front_shader"),
            source: wgpu::ShaderSource::Wgsl(hoi4_render::SHADER_FRONTLINES_WGSL.into()),
        });
        let frontlines_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("frontlines_pipeline"),
            layout: Some(&rail_pl),
            vertex: wgpu::VertexState {
                module: &front_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<FrontVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Unorm8x4,
                            offset: 12,
                            shader_location: 1,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &front_shader,
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
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: 4,
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        // V5 鏀跺彛锛歎I-pass-removed 宸插垹闄わ紙vanilla GUI command 娓叉煋绠＄嚎锛???        // Phase 3.5: Text pass
        let text_pass = TextPass::new(&device, format, logical_w, logical_h);

        // Phase 4.2 (redesign): Panel pass (鍦嗚鐭╁舰 SDF) + Flag bank (TGA 鍔犺浇)
        let panel_pass = PanelPass::new(&device, format, logical_w, logical_h);
        let mut flag_bank = FlagBank::new(&device, &queue);

        // Flag pipeline: 澶嶇敤 UI shader 姒傚康浣嗙嫭绔嬪疄渚嬶紝鍥犱负姣忎釜 flag 鐢ㄧ嫭绔嬬汗??
        let flag_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("flag_shader"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
                struct U { screen_size: vec2<f32> };
                @group(0) @binding(0) var<uniform> u: U;
                @group(0) @binding(1) var t: texture_2d<f32>;
                @group(0) @binding(2) var s: sampler;
                struct VsIn { @location(0) pos: vec2<f32>, @location(1) uv: vec2<f32> };
                struct VsOut { @builtin(position) clip_pos: vec4<f32>, @location(0) uv: vec2<f32> };
                @vertex fn vs_main(in: VsIn) -> VsOut {
                    var o: VsOut;
                    let nx = (in.pos.x / u.screen_size.x) * 2.0 - 1.0;
                    let ny = 1.0 - (in.pos.y / u.screen_size.y) * 2.0;
                    o.clip_pos = vec4<f32>(nx, ny, 0.0, 1.0);
                    o.uv = in.uv;
                    return o;
                }
                @fragment fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
                    return textureSample(t, s, in.uv);
                }
            "#
                .into(),
            ),
        });
        let flag_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("flag_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
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
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let flag_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&flag_bgl],
            push_constant_ranges: &[],
        });
        let flag_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("flag_pipeline"),
            layout: Some(&flag_pl),
            vertex: wgpu::VertexState {
                module: &flag_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 16,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &flag_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
        // 4.1.bis.6 fix (2026-05-16): flag layout coords come from
        // `last_country_layout` which is logical-space, so the shader needs
        // logical screen size to NDC-divide correctly.
        let flag_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("flag_uniforms"),
            contents: bytemuck::bytes_of(&[logical_w, logical_h]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let flag_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("flag_verts"),
            size: 6 * 16,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        flag_bank.init_fallback(&device, &flag_bgl, &flag_uniform_buffer);

        // Phase 3.6.3: 3D mesh trees
        let (
            trees_mesh_pipeline,
            trees_mesh_bind_groups,
            trees_mesh_vertex_buffers,
            trees_mesh_index_buffers,
            trees_mesh_instance_buffers,
            trees_mesh_index_counts,
            trees_mesh_instance_counts,
        ) = setup_tree_mesh_pipeline(
            &device,
            &queue,
            &self.path_cfg,
            &tree_data,
            &camera_buffer,
            &params_buffer,
            HDR_FORMAT,
            depth_format,
        );

        // Phase 3.12.8 ???vanilla tree.shader full integration.
        // Constructed AFTER shadow_pass + global_uniform_buf so it can consume
        // the shared shadow depth view + comparison sampler. When all 3 tree
        // mesh types load successfully it replaces the old trees_mesh path.
        let (season_lerp, season_column) = {
            let sr = self.seasons.season_for_date(1, 1); // 1936-01-01 start
            (sr.season_lerp, sr.season_column)
        };
        let tree_full_pass = passes::TreeFullPass::new(
            &device,
            &queue,
            &self.path_cfg,
            &tree_data,
            passes::TreeFullPassInputs {
                global_uniform_buffer: &global_uniform_buf.buffer,
                shadow_depth_view: &shadow_pass.depth_view,
                shadow_compare_sampler: &shadow_pass.compare_sampler,
                depth_format,
                world_size: [self.camera.world_size.x, self.camera.world_size.y],
                season_lerp,
                season_column,
                runtime_targets: &vanilla_targets,
                vanilla_resources: &vanilla_resources,
                tree_indices: &self.world.map.tree_indices,
            },
        );
        binding_audit.extend(tree_full_pass.binding_audit.entries.clone());
        println!(
            "[trees_full] TreeFullPass ready (any_loaded={}, {} warnings)",
            tree_full_pass.any_loaded,
            tree_full_pass.load_warnings.len()
        );
        for w in &tree_full_pass.load_warnings {
            eprintln!("  {}", w);
        }
        let tree_full_pass = if tree_full_pass.any_loaded {
            Some(tree_full_pass)
        } else {
            None
        };

        // Phase 9: pdxmesh pass for vanilla 3D map objects.
        // Constructed AFTER both `shadow_pass` (to share its depth view +
        // compare sampler) and `global_uniform_buf` (Phase 3.12.1 鍏变韩
        // GlobalFrameUniform). Resolves buildings.gfx pdxmesh records and
        // falls back to known vanilla building mesh paths when an entry is
        // missing.
        let mut pdxmesh_pass = passes::PdxMeshPass::new(
            &device,
            &queue,
            &self.path_cfg,
            &global_uniform_buf.buffer,
            depth_format,
            &shadow_pass.depth_view,
            &shadow_pass.compare_sampler,
            &vanilla_targets,
        );
        // Push map object instance data through (split by object kind).
        pdxmesh_pass.set_buildings(&device, &building_instances);
        let cam_eye = self.camera.eye();
        pdxmesh_pass.upload_lod_instances(&device, &queue, [cam_eye.x, cam_eye.y, cam_eye.z]);
        println!(
            "[pdxmesh] {} map object instances split across {} mesh types (any_loaded={})",
            pdxmesh_pass.total_instances(),
            pdxmesh_pass.mesh_type_count(),
            pdxmesh_pass.any_loaded
        );

        // Phase 3.12.7 ???vanilla river pass.
        // Reuses the same per-LOD instance buffers as the terrain pass; loads
        // 7 vanilla river textures (3 diffuse + 3 normal + masks) via FsAssetDb
        // with 1x1 fallback. Drawn after water and before borders; terrain's
        // navy-blue river overlay is only a fallback when RiverPass is unavailable.
        let river_pass = passes::RiverPass::new(
            &device,
            &queue,
            &self.path_cfg,
            passes::RiverPassInputs {
                global_uniform_buffer: &global_uniform_buf.buffer,
                depth_format,
                lod_grid: LOD_GRID,
                heightmap_view: &height_view,
                rivers_view: &rivers_view,
                world_size: [self.camera.world_size.x, self.camera.world_size.y],
                height_scale: HEIGHT_SCALE,
                vanilla_resources: &vanilla_resources,
                runtime_targets: &vanilla_targets,
            },
        );
        println!(
            "[river] RiverPass ready (any_loaded={}, {} warnings)",
            river_pass.any_loaded,
            river_pass.load_warnings.len()
        );
        for w in &river_pass.load_warnings {
            eprintln!("  {}", w);
        }
        binding_audit.extend(river_pass.binding_audit.entries.clone());

        let water_refraction_target = WaterRefractionTarget::for_quality(
            &device,
            config.width,
            config.height,
            self.render_toggles.map_quality_preset,
        );

        // P5 vanilla pdxwater pass. Reuses the same per-LOD instance buffers as
        // terrain, binds runtime ShadowMap/gradient/secondary targets, and no
        // longer consumes coast_sdf as a default parity input.
        let mut water_pass = passes::WaterPass::new(
            &device,
            &queue,
            &self.path_cfg,
            passes::WaterPassInputs {
                global_uniform_buffer: &global_uniform_buf.buffer,
                depth_format,
                lod_grid: LOD_GRID,
                heightmap_view: &height_view,
                province_view: &province_view,
                water_mask_view: &water_mask_view,
                world_size: [self.camera.world_size.x, self.camera.world_size.y],
                height_scale: HEIGHT_SCALE,
                vanilla_resources: &vanilla_resources,
                runtime_targets: &vanilla_targets,
                water_refraction_view: &water_refraction_target.view,
                water_refraction_sampler: &water_refraction_target.sampler,
                refraction_available: true,
                quality_preset: self.render_toggles.map_quality_preset,
            },
        );
        println!(
            "[water] WaterPass ready (any_loaded={}, loaded={}, fallback={}, critical_missing={}, {} warnings)",
            water_pass.any_loaded,
            water_pass.texture_load_stats.loaded,
            water_pass.texture_load_stats.fallback,
            water_pass.texture_load_stats.critical_missing,
            water_pass.load_warnings.len()
        );
        for w in &water_pass.load_warnings {
            eprintln!("  {}", w);
        }
        // Phase 3.12.9 (redesign) ???vanilla border pass using strip meshes.
        // CPU extracts border edges from province bitmap ???generates thin
        // quad-strip meshes that hug actual boundaries.
        let border_edges = hoi4_render::border_extract::extract_border_edges(&self.world);
        let border_meshes = hoi4_render::border_extract::generate_border_meshes(
            &border_edges,
            &self.world.map.heightmap,
            &hoi4_render::border_extract::StripParams {
                world_scale: WORLD_SCALE,
                height_scale: HEIGHT_SCALE,
                half_width: 0.009,
                y_bias: 0.018,
                tile_factor: 0.20,
                ..Default::default()
            },
        );
        println!(
            "[border] extracted {} edges ???{} mesh groups",
            border_edges.len(),
            border_meshes.len()
        );

        let border_pass = passes::BorderPass::new(
            &device,
            &queue,
            passes::BorderPassInputs {
                global_uniform_buffer: &global_uniform_buf.buffer,
                depth_format,
                meshes: &border_meshes,
                path_cfg: &self.path_cfg,
            },
        );
        println!(
            "[border] BorderPass ready (any_loaded={}, {} warnings)",
            border_pass.any_loaded,
            border_pass.load_warnings.len()
        );
        for w in &border_pass.load_warnings {
            eprintln!("  {}", w);
        }

        // Phase 3.12.11: Sky pass + EnvironmentMap cubemap.
        let sky_pass = passes::SkyPass::new(
            &device,
            &queue,
            &global_uniform_buf.buffer,
            HDR_FORMAT,
            depth_format,
            &self.path_cfg,
        );
        water_pass.set_env_cubemap(&device, &sky_pass.cubemap_view, sky_pass.loaded);
        pdxmesh_pass.set_env_cubemap_with_shadow(
            &device,
            &sky_pass.cubemap_view,
            &shadow_pass.depth_view,
            &shadow_pass.compare_sampler,
            &vanilla_targets,
        );
        println!(
            "[sky] SkyPass ready (cubemap_loaded={}, env_source={})",
            sky_pass.loaded,
            if sky_pass.loaded {
                "dds"
            } else {
                "procedural_fallback"
            }
        );
        binding_audit.extend(water_pass.binding_audit.entries.clone());
        println!("{}", binding_audit.summary_line());
        for entry in binding_audit.critical_entries().take(16) {
            eprintln!(
                "[binding-audit] critical {}.{} source={} reason={}",
                entry.pass,
                entry.binding,
                entry.source_name,
                entry.reason.as_deref().unwrap_or("none")
            );
        }

        // Phase 3.12.10: Particle pass (combat smoke / factory chimneys / scorched earth).
        let particle_pass = passes::ParticlePass::new(
            &device,
            &queue,
            &global_uniform_buf.buffer,
            HDR_FORMAT,
            depth_format,
        );
        println!(
            "[particle] ParticlePass ready (max {})",
            hoi4_render::particles::MAX_PARTICLES
        );

        // Phase 16: Arrows family (maparrow / traderoute / strait).
        let maparrow_pass =
            passes::MapArrowPass::new(&device, &queue, &global_uniform_buf.buffer, depth_format);
        let mut traderoute_pass =
            passes::TradeRoutePass::new(&device, &queue, &global_uniform_buf.buffer, depth_format);
        let mut strait_pass =
            passes::StraitPass::new(&device, &queue, &global_uniform_buf.buffer, depth_format);

        let trade_verts = passes::traderoute::generate_trade_route_vertices(
            &self.world,
            &centroids,
            WORLD_SCALE,
            HEIGHT_SCALE,
        );
        traderoute_pass.set_routes(&device, &queue, &trade_verts);

        // Build strait geometry from adjacency data.
        strait_pass.build_straits(
            &device,
            &queue,
            &self.world.map.special_adjacencies,
            &centroids,
            WORLD_SCALE,
        );
        println!(
            "[arrows] trade_verts={}, strait_verts={}",
            trade_verts.len(),
            strait_pass.any_loaded as u32,
        );
        println!("[arrows] MapArrowPass / TradeRoutePass / StraitPass ready");

        // Phase 3.12.10: vanilla-equivalent 3D country-name label pass.
        let mapname_pass = mapname_atlas_opt.as_ref().map(|atlas| {
            passes::MapnamePass::new(
                &device,
                &queue,
                passes::MapnamePassInputs {
                    global_uniform_buffer: &global_uniform_buf.buffer,
                    depth_format,
                    obbs: &country_obbs,
                    atlas: Some(atlas),
                    heightmap_view: &height_view,
                    world_scale: WORLD_SCALE,
                    label_y: HEIGHT_SCALE * 0.5,
                    height_scale: HEIGHT_SCALE,
                },
            )
        });
        let mapname_count = mapname_pass
            .as_ref()
            .map(|p| p.instance_count())
            .unwrap_or(0);
        println!(
            "[mapname] vanilla pass ready, {} label instances",
            mapname_count
        );

        // Phase 3.12.13: province-name labels (zoom-gated).
        let t_prov_labels = Instant::now();
        let province_labels = hoi4_render::province_labels::compute_province_labels(
            &self.world.map.province_map,
            &self.world.map.definitions,
            8000,
        );
        let n_prov_labels = province_labels.iter().filter(|l| l.is_some()).count();
        println!(
            "[province_name] {} land province labels computed in {:.2}s",
            n_prov_labels,
            t_prov_labels.elapsed().as_secs_f32()
        );

        let t_prov_atlas = Instant::now();
        let prov_names: Vec<Option<String>> = {
            let _defs = &self.world.map.definitions;
            let state_names = &self.world.states.names;
            let state_of = &self.world.provinces.state_of;
            let name_resolver =
                hoi4_app::ui_data::names::DisplayNameResolver::new(Some(&self.loc_catalog));
            let mut out: Vec<Option<String>> = vec![None; province_labels.len()];
            for id in 1..province_labels.len() {
                if province_labels[id].is_none() {
                    continue;
                }
                let sid = if id < state_of.len() {
                    state_of[id]
                } else {
                    hoi4_state::ids::StateId::NONE
                };
                let name = if sid != hoi4_state::ids::StateId::NONE {
                    let si = sid.0 as usize;
                    if si < state_names.len() && !state_names[si].is_empty() {
                        Some(name_resolver.state_name(&state_names[si], si))
                    } else {
                        Some(format!("PROV{}", id))
                    }
                } else {
                    Some(format!("PROV{}", id))
                };
                out[id] = name;
            }
            out
        };
        let prov_atlas_opt = province_name_atlas::bake_province_name_atlas(&prov_names, 14.0);
        if let Some(atlas) = &prov_atlas_opt {
            println!(
                "[province_name] atlas: {}x{} R8, {} entries baked in {:.2}s",
                atlas.width,
                atlas.height,
                atlas.count_baked(),
                t_prov_atlas.elapsed().as_secs_f32()
            );
        } else {
            eprintln!("[province_name] atlas bake failed (no system font?)");
        }

        let prov_instances = if let Some(atlas) = &prov_atlas_opt {
            province_name_atlas::build_province_label_instances(
                &province_labels,
                atlas,
                WORLD_SCALE,
                HEIGHT_SCALE * 0.45,
                100,
            )
        } else {
            Vec::new()
        };
        let prov_inst_count = prov_instances.len();
        let province_name_pass = if prov_atlas_opt.is_some() {
            Some(passes::ProvinceNamePass::new(
                &device,
                &queue,
                passes::province_name::ProvinceNamePassInputs {
                    global_uniform_buffer: &global_uniform_buf.buffer,
                    depth_format,
                    atlas: prov_atlas_opt.as_ref(),
                    heightmap_view: &height_view,
                    instances: &prov_instances,
                    height_scale: HEIGHT_SCALE,
                },
            ))
        } else {
            None
        };
        println!(
            "[province_name] pass ready, {} label instances",
            prov_inst_count
        );

        // 鈹€鈹€鈹€ Phase 3.12.1 鍏叡娓叉煋鍩虹璁炬柦 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
        // 绂诲睆 HDR ???RT锛圧GBA16Float锛???D pass 鍐欏叆杩欓噷锛屼箣鍚庣敱 PostProcessChain
        // ???SimpleBlitPass 妗ユ帴???swap chain??
        let hdr_target = HdrTarget::new(&device, config.width, config.height);
        let water_refraction_pass = WaterRefractionPass::new(
            &device,
            &hdr_target.view,
            water_refraction_target.format,
            hdr_target.width,
            hdr_target.height,
            water_refraction_target.width,
            water_refraction_target.height,
        );
        let simple_blit = SimpleBlitPass::new(&device, format, &hdr_target.view);
        let color_cube_source = ColorCubeSource::load_from_path_config(&self.path_cfg);
        let post_process = PostProcessChain::new(
            &device,
            &queue,
            &hdr_target.view,
            hdr_target.width,
            hdr_target.height,
            format,
            color_cube_source,
        );
        for warning in &post_process.color_cube_warnings {
            eprintln!("[postprocess] {warning}");
        }
        let map_renderer = MapRenderer::new();
        let mut pass_registry = PassRegistry::new();
        map_renderer.register_passes(&mut pass_registry);
        let debug_render_overlay = DebugOverlay::new();
        let gpu_profiler = GpuTimestampProfiler::new(&device, &queue);
        println!(
            "[phase10] quality={} budget={:?} gpu_timestamp={}",
            self.render_toggles.map_quality_preset.as_str(),
            self.render_toggles.map_quality_preset.budget(),
            gpu_profiler
                .as_ref()
                .map(|profiler| profiler.status())
                .unwrap_or(GpuProfilerStatus::UNSUPPORTED)
                .summary()
        );
        println!(
            "[render] HDR offscreen RT ready ({}x{} {:?}); post-process mode = {:?}; {}; color_cube={}",
            hdr_target.width,
            hdr_target.height,
            HDR_FORMAT,
            post_process.mode,
            post_process.calibration.summary(),
            post_process.color_cube_source
        );
        println!(
            "[water_refraction] target ready ({}x{} {:?})",
            water_refraction_target.width,
            water_refraction_target.height,
            water_refraction_target.format
        );

        // Egui UI overlay must target the swapchain format.
        let ui = hoi4_ui::UiState::new(
            &device,
            format,
            1,
            &window,
            self.ui_state.settings.display_scale,
        );
        // Apply vanilla UI theme and tooltip timing.
        let theme_assets = hoi4_ui::theme::apply_vanilla_theme(&ui.ctx);
        hoi4_ui::loc::configure_tooltip_delay(&ui.ctx);
        println!(
            "[render] egui UI overlay ready (target_format={:?}, game_scale={:.2}, system_dpi={:.2})\n[ui] vanilla theme: latin={:?} cjk={:?}",
            format,
            self.ui_state.settings.display_scale,
            window.scale_factor(),
            theme_assets.latin_serif_path,
            theme_assets.cjk_fallback_path,
        );

        // Load vanilla 9-slice window texture with a plain-color fallback.
        let nine_slice_window = match hoi4_ui::nine_slice::NineSlice::load_vanilla(
            &ui.ctx,
            &self.path_cfg,
            "gfx/interface/tiles/tiled_window.dds",
            hoi4_ui::nine_slice::NineSliceEdges::uniform(32.0),
            "vanilla_tiled_window",
        ) {
            Ok(ns) => {
                println!(
                    "[ui] 9-slice tiled_window loaded ({}x{}, edges=32 px)",
                    ns.size.x as u32, ns.size.y as u32
                );
                Some(ns)
            }
            Err(e) => {
                eprintln!("[ui] 9-slice tiled_window load failed: {e} (UI 璧扮函鑹?fallback)");
                None
            }
        };

        // Sprite icon bank lazily loads icons on first use.
        let mut icon_bank = hoi4_ui::icons::IconBank::new(ui.ctx.clone(), self.path_cfg.clone());
        icon_bank.add_search_dir("gfx/interface/ideas");
        icon_bank.add_search_dir("gfx/interface/idea_categories");
        icon_bank.add_search_dir("gfx/event_pictures");
        icon_bank.add_search_dir("gfx/interface");
        // Register DLC leader portrait directories before base-game directories.
        {
            let dlc_dir = self.path_cfg.game_path().join("dlc");
            if dlc_dir.is_dir() {
                if let Ok(dlcs) = std::fs::read_dir(&dlc_dir) {
                    for dlc in dlcs.flatten() {
                        let leaders = dlc.path().join("gfx").join("leaders");
                        if leaders.is_dir() {
                            if let Ok(tags) = std::fs::read_dir(&leaders) {
                                for tag_dir in tags.flatten() {
                                    if tag_dir.path().is_dir() {
                                        let rel = format!(
                                            "dlc/{}/gfx/leaders/{}",
                                            dlc.file_name().to_string_lossy(),
                                            tag_dir.file_name().to_string_lossy()
                                        );
                                        icon_bank.add_search_dir(rel);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        icon_bank.add_leader_dirs(self.world.countries.tags.iter().filter(|t| !t.is_empty()));
        // Preload known GER focus icons used by the startup banner.
        for name in &["GFX_focus_GER_anschluss", "GFX_focus_GER_afrikakorps"] {
            if icon_bank.get_or_load(name).is_some() {
                if let Some(sz) = icon_bank.size_of(name) {
                    println!("[ui] icon preloaded: {name} ({}x{})", sz[0], sz[1]);
                }
            } else if let Some(reason) = icon_bank.missing_reason(name) {
                eprintln!("[ui] icon preload failed: {name}: {reason}");
            }
        }
        hoi4_ui::politics::warm_country_politics_runtime(&mut icon_bank);
        hoi4_ui::law_panel::warm_law_panel_runtime(&mut icon_bank);
        hoi4_ui::decisions_panel::warm_country_decision_runtime(
            &mut icon_bank,
            self.runtime
                .content
                .decision_db
                .decisions
                .iter()
                .map(|decision| decision.icon.as_str()),
        );
        hoi4_ui::focus_tree_panel::warm_national_focus_runtime(
            &mut icon_bank,
            Some(&self.runtime.content.focus_tree),
        );

        // Startup banner: verify country leader coverage for the seven majors.
        {
            let major_tags = ["GER", "SOV", "ENG", "FRA", "ITA", "JAP", "USA"];
            let mut hit = 0u32;
            let mut miss_list: Vec<&str> = Vec::new();
            for tag in &major_tags {
                let cid = self.world.country(tag);
                let has_leader = cid.and_then(|c| self.world.country_leader(c)).is_some();
                if has_leader {
                    hit += 1;
                } else {
                    miss_list.push(tag);
                }
            }
            println!(
                "[J.1] country_leader coverage: {hit}/{} majors hit{}",
                major_tags.len(),
                if miss_list.is_empty() {
                    String::new()
                } else {
                    format!(" 鈥?missing: {}", miss_list.join(", "))
                }
            );
        }

        let province_pixel_bounds =
            render_collect::build_province_pixel_bounds(&self.world.map.province_map);

        self.state = Some(RenderState {
            surface,
            device,
            queue,
            config,
            terrain_pass,
            vanilla_targets,
            camera_buffer,
            params_buffer,
            instance_buffers,
            instance_capacity: [total_chunks as u32; 3],
            terrain_bucket_signature: [0; 3],
            terrain_bucket_counts: [0; 3],
            terrain_buckets: std::array::from_fn(|_| Vec::with_capacity(total_chunks as usize)),
            lut_texture,
            lut_width,
            lut_height,
            occupation_lut_texture,
            depth_view,
            depth_format,
            chunk_grid,
            trees_pipeline,
            trees_bind_group,
            trees_buffer,
            trees_count,
            railways_pipeline,
            railways_bind_group: rail_bg,
            railways_params_buffer,
            railways_buffer,
            railways_vertex_count,
            hoi3_counter_pass,
            unit_counter_centroids: centroids.clone(),
            province_pixel_bounds,
            frontlines_pipeline,
            frontlines_bind_group: frontlines_bg,
            frontlines_buffer,
            frontlines_vertex_count,
            frontlines_params_buffer,
            buildings_buffer,
            buildings_params_buffer,
            buildings_bind_group,
            buildings_count,
            buildings_pipeline,
            pdxmesh_pass,
            water_pass,
            river_pass,
            border_pass,
            sky_pass,
            particle_pass,
            maparrow_pass,
            traderoute_pass,
            strait_pass,
            poi_icon_pass,
            poi_icon_instances,
            poi_zoom_bucket: 3,
            text_pass,
            panel_pass,
            flag_bank,
            flag_pipeline,
            flag_bgl,
            flag_uniform_buffer,
            flag_vertex_buffer,
            trees_mesh_pipeline,
            trees_mesh_bind_groups,
            trees_mesh_vertex_buffers,
            trees_mesh_index_buffers,
            trees_mesh_instance_buffers,
            trees_mesh_index_counts,
            trees_mesh_instance_counts,
            tree_full_pass,
            tree_lod_uploaded: false,
            tree_lod_last_cam_pos: [f32::NAN, f32::NAN, f32::NAN],
            country_labels,
            mapname_pass,
            mapname_atlas: mapname_atlas_opt,
            province_name_pass,
            // Phase 3.12.1
            hdr_target,
            water_refraction_target,
            water_refraction_pass,
            global_uniform_buf,
            simple_blit,
            post_process,
            // Phase 3.12.3
            shadow_pass,
            map_renderer,
            pass_registry,
            debug_render_overlay,
            gpu_profiler,
            ui,
            nine_slice_window,
            icon_bank,
            window,
        });

        // Apply fullscreen setting after render state initialization.
        if self.ui_state.settings.fullscreen {
            if let Some(s) = self.state.as_ref() {
                s.window.set_fullscreen(Some(Fullscreen::Borderless(None)));
            }
        }
        if let Some(s) = self.state.as_mut() {
            // Phase 3.5: Load font for text rendering
            let bgl = TextPass::bind_group_layout(&s.device);
            s.text_pass
                .load_font(&s.device, &s.queue, &self.path_cfg, &bgl);
        }
    }
}

fn load_country_label_loc_catalog(
    path_cfg: &PathConfig,
    language: hoi4_ui::i18n::Language,
) -> hoi4_ui::loc::LocCatalog {
    load_loc_catalog_for_language(path_cfg, language)
}

fn map_country_label(
    tag: &str,
    ruling_party: &str,
    language: hoi4_ui::i18n::Language,
    country_loc: &hoi4_ui::loc::LocCatalog,
    english_loc: &hoi4_ui::loc::LocCatalog,
) -> String {
    let raw = profile_country_label(tag, language)
        .or_else(|| localized_country_label(tag, ruling_party, country_loc))
        .or_else(|| builtin_country_name(tag, language).map(str::to_owned))
        .or_else(|| {
            if language != hoi4_ui::i18n::Language::English {
                profile_country_label(tag, hoi4_ui::i18n::Language::English)
                    .or_else(|| localized_country_label(tag, ruling_party, english_loc))
            } else {
                None
            }
        })
        .or_else(|| builtin_country_name(tag, hoi4_ui::i18n::Language::English).map(str::to_owned))
        .unwrap_or_else(|| tag.to_owned());
    format_map_country_label(&raw)
}

fn profile_country_label(tag: &str, language: hoi4_ui::i18n::Language) -> Option<String> {
    let localized = hoi4_ui::i18n::tr_for_language(tag, language).trim();
    if localized != tag && !localized.is_empty() {
        Some(localized.to_owned())
    } else {
        None
    }
}

fn localized_country_label(
    tag: &str,
    ruling_party: &str,
    loc: &hoi4_ui::loc::LocCatalog,
) -> Option<String> {
    for key in country_label_keys(tag, ruling_party) {
        let localized = loc.tr(&key).trim();
        if localized != key && !localized.is_empty() {
            return Some(localized.to_owned());
        }
    }
    None
}

fn country_label_keys(tag: &str, ruling_party: &str) -> Vec<String> {
    let mut keys = Vec::with_capacity(2);
    if let Some(ideology) = country_label_ideology_key(ruling_party) {
        keys.push(format!("{tag}_{ideology}"));
    }
    keys.push(tag.to_owned());
    keys
}

fn country_label_ideology_key(ruling_party: &str) -> Option<&str> {
    let key = ruling_party.trim();
    match key {
        "fascism" | "democratic" | "communism" | "neutrality" => Some(key),
        "" => None,
        other => Some(other),
    }
}

fn format_map_country_label(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_ascii() {
        trimmed.to_ascii_uppercase()
    } else {
        trimmed.to_owned()
    }
}

fn builtin_country_name(tag: &str, language: hoi4_ui::i18n::Language) -> Option<&'static str> {
    match language {
        hoi4_ui::i18n::Language::Chinese => builtin_chinese_country_name(tag),
        hoi4_ui::i18n::Language::English => builtin_english_country_name(tag),
    }
}

fn builtin_chinese_country_name(tag: &str) -> Option<&'static str> {
    match tag {
        "AUS" => Some("奥地利"),
        "AST" => Some("澳大利亚"),
        "BRA" => Some("巴西"),
        "CAN" => Some("加拿大"),
        "CHI" => Some("中华民国"),
        "CZE" => Some("捷克斯洛伐克"),
        "ENG" => Some("联合王国"),
        "FRA" => Some("法国"),
        "GER" => Some("德意志国"),
        "GXC" => Some("桂系"),
        "ITA" => Some("意大利"),
        "JAP" => Some("日本"),
        "MAN" => Some("满洲国"),
        "MEN" => Some("蒙疆"),
        "POL" => Some("波兰"),
        "PRC" => Some("中共"),
        "RAJ" => Some("英属印度"),
        "ROM" => Some("罗马尼亚"),
        "SAF" => Some("南非"),
        "SHX" => Some("晋系"),
        "SIK" => Some("新疆"),
        "SOV" => Some("苏维埃联盟"),
        "SPR" => Some("西班牙"),
        "SWE" => Some("瑞典"),
        "TIB" => Some("西藏"),
        "USA" => Some("美利坚合众国"),
        "XSM" => Some("马家军"),
        "YUN" => Some("云南"),
        _ => None,
    }
}

fn builtin_english_country_name(tag: &str) -> Option<&'static str> {
    match tag {
        "AUS" => Some("Austria"),
        "AST" => Some("Australia"),
        "BRA" => Some("Brazil"),
        "CAN" => Some("Canada"),
        "CHI" => Some("China"),
        "CZE" => Some("Czechoslovakia"),
        "ENG" => Some("United Kingdom"),
        "FRA" => Some("France"),
        "GER" => Some("German Reich"),
        "GXC" => Some("Guangxi Clique"),
        "ITA" => Some("Italy"),
        "JAP" => Some("Japan"),
        "MAN" => Some("Manchukuo"),
        "MEN" => Some("Mengjiang"),
        "POL" => Some("Poland"),
        "PRC" => Some("Communist China"),
        "RAJ" => Some("British Raj"),
        "ROM" => Some("Romania"),
        "SAF" => Some("South Africa"),
        "SHX" => Some("Shanxi"),
        "SIK" => Some("Sinkiang"),
        "SOV" => Some("Soviet Union"),
        "SPR" => Some("Spain"),
        "SWE" => Some("Sweden"),
        "TIB" => Some("Tibet"),
        "USA" => Some("United States"),
        "XSM" => Some("Ma Clique"),
        "YUN" => Some("Yunnan"),
        _ => None,
    }
}

fn select_captured_restore_surface_format(formats: &[wgpu::TextureFormat]) -> wgpu::TextureFormat {
    let preferred = [
        wgpu::TextureFormat::Bgra8Unorm,
        wgpu::TextureFormat::Rgba8Unorm,
    ];
    preferred
        .into_iter()
        .find(|format| formats.contains(format))
        .or_else(|| formats.iter().copied().find(|format| !format.is_srgb()))
        .or_else(|| formats.first().copied())
        .expect("surface must expose at least one format")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_loc() -> hoi4_ui::loc::LocCatalog {
        hoi4_ui::loc::LocCatalog::new()
    }

    #[test]
    fn chinese_map_country_labels_use_profile_country_names() {
        let empty = empty_loc();
        assert_eq!(
            map_country_label(
                "GER",
                "fascism",
                hoi4_ui::i18n::Language::Chinese,
                &empty,
                &empty,
            ),
            "德国"
        );
        assert_eq!(
            map_country_label(
                "CHI",
                "neutrality",
                hoi4_ui::i18n::Language::Chinese,
                &empty,
                &empty,
            ),
            "中国"
        );
        assert_eq!(
            map_country_label(
                "ENG",
                "democratic",
                hoi4_ui::i18n::Language::Chinese,
                &empty,
                &empty,
            ),
            "英国"
        );
    }

    #[test]
    fn map_country_label_prefers_profile_name_over_ideology_localisation() {
        let root = std::env::temp_dir().join(format!(
            "ironheart_render_init_loc_{}_{}",
            std::process::id(),
            "profile"
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("countries_l_simp_chinese.yml"),
            "l_simp_chinese:\n MAN_neutrality:0 \"大清\"\n MAN:0 \"满洲国\"\n",
        )
        .unwrap();

        let loc = hoi4_ui::loc::LocCatalog::load_from_dir(&root);
        let empty = empty_loc();
        assert_eq!(
            map_country_label(
                "MAN",
                "neutrality",
                hoi4_ui::i18n::Language::Chinese,
                &loc,
                &empty,
            ),
            "满洲国"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn map_country_label_uses_vanilla_localisation_when_profile_name_is_missing() {
        let root = std::env::temp_dir().join(format!(
            "ironheart_render_init_loc_{}_{}",
            std::process::id(),
            "fallback"
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("countries_l_simp_chinese.yml"),
            "l_simp_chinese:\n TST_fascism:0 \"测试国\"\n TST:0 \"测试\"\n",
        )
        .unwrap();

        let loc = hoi4_ui::loc::LocCatalog::load_from_dir(&root);
        let empty = empty_loc();
        assert_eq!(
            map_country_label(
                "TST",
                "fascism",
                hoi4_ui::i18n::Language::Chinese,
                &loc,
                &empty,
            ),
            "测试国"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn english_map_country_labels_remain_uppercase() {
        let empty = empty_loc();
        assert_eq!(
            map_country_label(
                "ENG",
                "democratic",
                hoi4_ui::i18n::Language::English,
                &empty,
                &empty,
            ),
            "UNITED KINGDOM"
        );
    }
}
