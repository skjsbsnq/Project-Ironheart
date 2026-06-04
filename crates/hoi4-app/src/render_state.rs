use super::*;

pub(crate) struct RenderState {
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) config: wgpu::SurfaceConfiguration,
    /// Vanilla pdxmap-equivalent terrain pipeline. Phase 11 removed the old
    /// archived `shader.wgsl` render fallback; missing assets are now handled
    /// inside TerrainPass via explicit 1x1 texture fallbacks.
    pub(crate) terrain_pass: TerrainPass,
    pub(crate) vanilla_targets: VanillaRuntimeTargets,
    pub(crate) camera_buffer: wgpu::Buffer,
    /// Per-frame render params (selection, zoom, time).
    pub(crate) params_buffer: wgpu::Buffer,
    /// Per-LOD instance buffers (one ChunkInstance per visible chunk in that LOD).
    pub(crate) instance_buffers: [wgpu::Buffer; 3],
    /// Capacity (in instances) of each instance buffer; grown as needed.
    pub(crate) instance_capacity: [u32; 3],
    /// Last uploaded terrain chunk buckets; avoids rewriting identical instance buffers while panning.
    pub(crate) terrain_bucket_signature: [u64; 3],
    pub(crate) terrain_bucket_counts: [u32; 3],
    pub(crate) lut_texture: wgpu::Texture,
    pub(crate) lut_width: u32,
    pub(crate) lut_height: u32,
    /// Occupation overlay LUT - held to keep the bind-group view alive.
    /// Rebuilt when controller changes can affect map colour overlays.
    #[allow(dead_code)]
    pub(crate) occupation_lut_texture: wgpu::Texture,
    pub(crate) depth_view: wgpu::TextureView,
    pub(crate) depth_format: wgpu::TextureFormat,
    pub(crate) chunk_grid: ChunkGrid,
    /// Trees pipeline (5.6). One large vertex buffer + a single instanced draw.
    pub(crate) trees_pipeline: wgpu::RenderPipeline,
    pub(crate) trees_bind_group: wgpu::BindGroup,
    pub(crate) trees_buffer: wgpu::Buffer,
    pub(crate) trees_count: u32,
    /// Railways pipeline (5.6). LineList draw.
    pub(crate) railways_pipeline: wgpu::RenderPipeline,
    pub(crate) railways_bind_group: wgpu::BindGroup,
    pub(crate) railways_params_buffer: wgpu::Buffer,
    pub(crate) railways_buffer: wgpu::Buffer,
    pub(crate) railways_vertex_count: u32,
    /// Phase I (CR-1.2): HOI3-style screen-space procedural counter pass.
    pub(crate) hoi3_counter_pass: Hoi3CounterPass,
    /// Province pixel centroids (heightmap-pixel coords). Cached on init so the
    /// per-frame zoom-driven counter aggregation doesn't need to recompute them.
    pub(crate) unit_counter_centroids: Vec<(f32, f32)>,
    /// Province pixel bounds in heightmap/province-map coordinates. This is
    /// static map data used by frontline arrow collection.
    pub(crate) province_pixel_bounds: Vec<Option<render_collect::ProvincePixelBounds>>,
    /// Frontlines pipeline (5.7). LineList.
    pub(crate) frontlines_pipeline: wgpu::RenderPipeline,
    pub(crate) frontlines_bind_group: wgpu::BindGroup,
    pub(crate) frontlines_buffer: wgpu::Buffer,
    pub(crate) frontlines_vertex_count: u32,
    pub(crate) frontlines_params_buffer: wgpu::Buffer,
    // V5 閺€璺哄經???026-05-18閿涘绱伴崚鐘绘珟 `ui_pass: UI-pass-removed` 鐎涙顔岄妴鍊€opbar / 閺€鎸庝笉闂堛垺婢?    // / 9-slice sprite 濞撳弶鐓嬬粻锛勫殠瀹告彃浠犻悽顭掔礉缁涘绶熼梼鑸殿唽 B ???egui 闁插秴浠???    /// Phase 2.9: Buildings instanced buffer (reuses units pipeline).
    pub(crate) buildings_buffer: wgpu::Buffer,
    pub(crate) buildings_params_buffer: wgpu::Buffer,
    pub(crate) buildings_bind_group: wgpu::BindGroup,
    pub(crate) buildings_count: u32,
    pub(crate) buildings_pipeline: wgpu::RenderPipeline,
    /// Phase 3.12.5 ???`PdxMeshPass` for vanilla 3D building meshes (replaces
    /// the procedural `buildings_pipeline` flat-coloured billboards above when
    /// `pdxmesh_pass.any_loaded` is true).
    pub(crate) pdxmesh_pass: passes::PdxMeshPass,
    /// Phase 6 `WaterPass` for vanilla pdxwater shading (LEAN normals,
    /// SampleWater, refraction, reflection, sun spec, coastal foam, polar ice,
    /// runtime border/secondary/FOW targets, plus point-light blocker bindings).
    /// Drawn after the terrain pass so it overdraws the inline water branch
    /// in `terrain.wgsl` with full pdxwater output. When water vanilla
    /// textures fail to load, falls back to terrain.wgsl's procedural water.
    pub(crate) water_pass: passes::WaterPass,
    /// Phase 3.12.7 ???`RiverPass` for vanilla river.shader rendering (flow
    /// scrolling + diffuse/normal/masks textures + alpha blend). Drawn after
    /// terrain, before water ???replaces inline navy-blue overlay.
    pub(crate) river_pass: passes::RiverPass,
    /// Phase 3.12.9 ???`BorderPass` for vanilla border.shader rendering
    /// (6-type 鑴?3-LOD = 18 pre-baked SDF textures + gradient_border
    /// dual-channel fill). Drawn after water, replacing terrain.wgsl's
    /// inline SDF border code. SDF fallback stays in terrain.wgsl when
    /// border_pass.any_loaded is false.
    pub(crate) border_pass: passes::BorderPass,
    /// Phase 3.12.11 ???`SkyPass` for sky cubemap background.
    pub(crate) sky_pass: passes::SkyPass,
    /// Phase 3.12.10 ???`ParticlePass` for combat smoke / factory chimneys / scorched earth.
    pub(crate) particle_pass: passes::ParticlePass,
    /// Phase 16.1 ???`MapArrowPass` for military order arrows (move / invade / paradrop).
    pub(crate) maparrow_pass: passes::MapArrowPass,
    /// Phase 16.2 ???`TradeRoutePass` for flowing trade route dashed lines.
    pub(crate) traderoute_pass: passes::TradeRoutePass,
    /// Phase 16.3 ???`StraitPass` for strait / canal crossing lines.
    pub(crate) strait_pass: passes::StraitPass,
    /// Phase 14 ???`PoiIconPass` for vanilla POI icons (factories / ports / airbases / resources).
    pub(crate) poi_icon_pass: Option<passes::PoiIconPass>,
    pub(crate) poi_icon_instances: Vec<PoiIconInstance>,
    pub(crate) poi_zoom_bucket: u8,
    /// Phase 3.5: Text rendering pass.
    pub(crate) text_pass: TextPass,
    pub(crate) panel_pass: PanelPass,
    pub(crate) flag_bank: FlagBank,
    pub(crate) flag_pipeline: wgpu::RenderPipeline,
    pub(crate) flag_bgl: wgpu::BindGroupLayout,
    pub(crate) flag_uniform_buffer: wgpu::Buffer,
    pub(crate) flag_vertex_buffer: wgpu::Buffer,
    /// Phase 3.6.3: 3D mesh trees - one draw call per tree type.
    pub(crate) trees_mesh_pipeline: wgpu::RenderPipeline,
    pub(crate) trees_mesh_bind_groups: Vec<wgpu::BindGroup>, // one per tree type (with its texture)
    pub(crate) trees_mesh_vertex_buffers: Vec<wgpu::Buffer>, // mesh geometry per type
    pub(crate) trees_mesh_index_buffers: Vec<wgpu::Buffer>,  // mesh indices per type
    pub(crate) trees_mesh_instance_buffers: Vec<wgpu::Buffer>, // per-tree positions per type
    pub(crate) trees_mesh_index_counts: Vec<u32>,
    pub(crate) trees_mesh_instance_counts: Vec<u32>,
    /// Phase 3.12.8 閳?vanilla tree.shader full integration (season coloring +
    /// tint overlay + shadow receive + day/night). Replaces the old
    /// `trees_mesh_*` path when `tree_full_pass.any_loaded` is true.
    pub(crate) tree_full_pass: Option<passes::TreeFullPass>,
    pub(crate) tree_lod_uploaded: bool,
    pub(crate) tree_lod_last_cam_pos: [f32; 3],
    /// Phase 3.5: Per-country world-space label anchors (centroid of owned provinces).
    /// `Some(CountryLabel)` for countries with at least one owned province.
    pub(crate) country_labels: Vec<Option<hoi4_render::mapname::CountryLabel>>,
    /// Phase 3.12.10: vanilla-equivalent 3D country-name label pass
    /// (GlobalFrameUniform + vDistortedPos + day/night 0.35 + stencil ref=4).
    /// `None` when atlas baking failed at startup; falls back to 2D HUD path.
    pub(crate) mapname_pass: Option<passes::MapnamePass>,
    /// Cached country-name atlas; reused when rebuilding labels after
    /// runtime ownership changes (civil war split, annexation).
    pub(crate) mapname_atlas: Option<mapname_atlas::CountryNameAtlas>,
    /// Phase 3.12.13: province-name label pass (zoom-gated, only land provinces).
    pub(crate) province_name_pass: Option<passes::ProvinceNamePass>,
    // 閳光偓閳光偓閳光偓 Phase 3.12.1 閸忣剙鍙″〒鍙夌厠閸╄櫣顢呯拋鐐煢 閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓閳光偓
    pub(crate) hdr_target: HdrTarget,
    pub(crate) water_refraction_target: passes::WaterRefractionTarget,
    pub(crate) water_refraction_pass: passes::WaterRefractionPass,
    pub(crate) global_uniform_buf: GlobalUniformBuffer,
    pub(crate) simple_blit: SimpleBlitPass,
    pub(crate) post_process: PostProcessChain,
    pub(crate) shadow_pass: passes::ShadowPass,
    pub(crate) map_renderer: MapRenderer,
    pub(crate) pass_registry: PassRegistry,
    pub(crate) debug_render_overlay: DebugOverlay,
    pub(crate) gpu_profiler: Option<GpuTimestampProfiler>,
    pub(crate) ui: hoi4_ui::UiState,
    pub(crate) nine_slice_window: Option<hoi4_ui::nine_slice::NineSlice>,
    pub(crate) icon_bank: hoi4_ui::icons::IconBank,
    pub(crate) window: Arc<Window>,
}
