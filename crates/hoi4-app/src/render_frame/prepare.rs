use crate::*;

pub(crate) struct FramePrepareInput {
    pub(crate) render_quality_preset: MapQualityPreset,
    pub(crate) map_layer_mask: map_baseline::MapLayerMask,
    pub(crate) map_phase0_active: bool,
}

pub(crate) struct FramePrepareOutput {
    pub(crate) params: RenderParams,
    pub(crate) zoom_factor: f32,
    pub(crate) time: f32,
    pub(crate) date: hoi4_state::GameDate,
    pub(crate) season_result: hoi4_map::SeasonResult,
    pub(crate) vanilla_map_space: VanillaMapSpace,
}

pub(crate) struct FrameSurfaceTarget {
    pub(crate) frame: SurfaceFrameGuard,
    pub(crate) surface_view: wgpu::TextureView,
    pub(crate) capture_texture: Option<wgpu::Texture>,
    pub(crate) capture_view: Option<wgpu::TextureView>,
    pub(crate) enc: wgpu::CommandEncoder,
}

pub(crate) struct FrameMapPrepareOutput {
    pub(crate) map_frame_plan: crate::map_renderer::MapFramePlan,
    pub(crate) prepared_map_frame: crate::map_renderer::MapPreparedFrame,
    pub(crate) draw_3d_map: bool,
}

pub(crate) struct FramePassParamsInput<'a> {
    pub(crate) render_quality_preset: MapQualityPreset,
    pub(crate) map_frame_plan: &'a crate::map_renderer::MapFramePlan,
    pub(crate) world_objects: WorldObjectPlan,
    pub(crate) static_decals: crate::map_renderer::StaticMapDecalPlan,
    pub(crate) semantic_overlays: crate::map_renderer::SemanticOverlayPlan,
    pub(crate) season_result: &'a hoi4_map::SeasonResult,
    pub(crate) vanilla_map_space: &'a VanillaMapSpace,
    pub(crate) terrain_ownership: crate::map_renderer::TerrainMaterialOwnership,
    pub(crate) water_ownership: crate::map_renderer::WaterMaterialOwnership,
    pub(crate) draw_3d_map: bool,
    pub(crate) zoom_factor: f32,
}

pub(crate) struct FramePassParamsOutput {
    pub(crate) water_refraction_available: bool,
    pub(crate) selected_water_effect_name: &'static str,
}

pub(crate) struct FrameUiPhaseOutput {
    pub(crate) render_started: Instant,
    pub(crate) render_quality_preset: MapQualityPreset,
    pub(crate) prepare_started: Instant,
    pub(crate) map_layer_mask: map_baseline::MapLayerMask,
    pub(crate) map_phase0_active: bool,
    pub(crate) map_phase0_debug_lines: Vec<String>,
    pub(crate) map_phase0_capture_path: Option<std::path::PathBuf>,
    pub(crate) ui_command_output: ui_driver::commands::UiCommandApplyOutput,
    pub(crate) egui_current_stats: hoi4_ui::UiFrameStats,
    pub(crate) profile_world_plan_ms: f32,
    pub(crate) profile_visual_updates_ms: f32,
    pub(crate) profile_ui_data_ms: f32,
    pub(crate) profile_egui_begin_ms: f32,
    pub(crate) profile_ui_commands_ms: f32,
    pub(crate) profile_mark: Instant,
}

pub(crate) struct FrameDrawHudInput<'a> {
    pub(crate) map_phase0_debug_lines: &'a [String],
    pub(crate) render_quality_preset: MapQualityPreset,
    pub(crate) map_frame_plan: &'a crate::map_renderer::MapFramePlan,
    pub(crate) draw_3d_map: bool,
    pub(crate) semantic_overlays: crate::map_renderer::SemanticOverlayPlan,
    pub(crate) map_fallback_report: crate::map_renderer::MapPassFallbackReport,
    pub(crate) water_final_color: bool,
    pub(crate) pass_params_output: FramePassParamsOutput,
    pub(crate) vanilla_map_space: &'a VanillaMapSpace,
    pub(crate) zoom_factor: f32,
    pub(crate) profile_mark: Instant,
}

pub(crate) struct FrameDrawHudOutput {
    pub(crate) quality_label: String,
    pub(crate) chain_label: &'static str,
    pub(crate) profile_map_draw_ms: f32,
    pub(crate) profile_hud_build_ms: f32,
}

pub(crate) struct FrameSubmitPhaseInput<'a> {
    pub(crate) ui_phase: FrameUiPhaseOutput,
    pub(crate) frame: SurfaceFrameGuard,
    pub(crate) output_view: &'a wgpu::TextureView,
    pub(crate) capture_texture: Option<&'a wgpu::Texture>,
    pub(crate) draw_hud_output: FrameDrawHudOutput,
    pub(crate) profile_params_buckets_ms: f32,
    pub(crate) profile_surface_ms: f32,
    pub(crate) profile_shadow_global_ms: f32,
    pub(crate) profile_map_prepare_ms: f32,
    pub(crate) profile_vanilla_targets_ms: f32,
    pub(crate) profile_pass_params_ms: f32,
}

pub(crate) struct SurfaceFrameGuard {
    frame: Option<wgpu::SurfaceTexture>,
}

impl SurfaceFrameGuard {
    pub(crate) fn new(frame: wgpu::SurfaceTexture) -> Self {
        Self { frame: Some(frame) }
    }

    pub(crate) fn texture(&self) -> Option<&wgpu::Texture> {
        self.frame.as_ref().map(|frame| &frame.texture)
    }

    pub(crate) fn present(mut self) {
        if let Some(frame) = self.frame.take() {
            frame.present();
        }
    }
}

impl Drop for SurfaceFrameGuard {
    fn drop(&mut self) {
        let _ = self.frame.take();
    }
}

struct TakenRenderStateGuard {
    slot: *mut Option<RenderState>,
    state: Option<RenderState>,
}

impl TakenRenderStateGuard {
    fn take_from(slot: &mut Option<RenderState>) -> Option<Self> {
        let state = slot.take()?;
        Some(Self {
            slot: slot as *mut Option<RenderState>,
            state: Some(state),
        })
    }

    fn as_ref(&self) -> &RenderState {
        self.state.as_ref().expect("render state guard is empty")
    }

    fn as_mut(&mut self) -> &mut RenderState {
        self.state.as_mut().expect("render state guard is empty")
    }

    fn into_inner(mut self) -> RenderState {
        self.state.take().expect("render state guard is empty")
    }
}

impl Drop for TakenRenderStateGuard {
    fn drop(&mut self) {
        // SAFETY: the guard is created from `self.state` in `render_pipeline`,
        // the app is not moved while the guard is alive, and this pointer is
        // only dereferenced here to restore the slot during unwinding.
        unsafe {
            if (*self.slot).is_none() {
                *self.slot = self.state.take();
            }
        }
    }
}

impl App {
    pub(crate) fn render_pipeline(&mut self) {
        let ui_phase = match self.prepare_visual_ui_phase(Instant::now()) {
            Some(output) => output,
            None => return,
        };
        let mut profile_mark = ui_phase.profile_mark;
        let mut state_guard = match TakenRenderStateGuard::take_from(&mut self.state) {
            Some(guard) => guard,
            None => return,
        };

        let frame_prepare_input = FramePrepareInput {
            render_quality_preset: ui_phase.render_quality_preset,
            map_layer_mask: ui_phase.map_layer_mask,
            map_phase0_active: ui_phase.map_phase0_active,
        };
        let frame_prepare = self.prepare_render_params(state_guard.as_ref(), &frame_prepare_input);
        let mut params = frame_prepare.params;
        let zoom_factor = frame_prepare.zoom_factor;
        let time = frame_prepare.time;
        let date = frame_prepare.date;
        let season_result = frame_prepare.season_result;
        let vanilla_map_space = frame_prepare.vanilla_map_space;

        self.update_terrain_buckets(state_guard.as_mut());

        let profile_params_buckets_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();

        let surface_target = match self
            .acquire_surface_and_capture_target(state_guard.as_mut(), ui_phase.map_phase0_active)
        {
            Some(target) => target,
            None => return,
        };
        let FrameSurfaceTarget {
            frame,
            surface_view,
            capture_texture,
            capture_view,
            mut enc,
        } = surface_target;
        let output_view = capture_view.as_ref().unwrap_or(&surface_view);
        let profile_surface_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();

        self.update_global_uniforms(
            state_guard.as_mut(),
            &params,
            &vanilla_map_space,
            time,
            season_result.season_blend,
        );
        let profile_shadow_global_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();

        let map_prepare_output = self.prepare_map_renderer_frame(
            state_guard.as_ref(),
            &frame_prepare_input,
            date,
            zoom_factor,
            time,
            ui_phase.prepare_started,
        );
        let map_frame_plan = map_prepare_output.map_frame_plan;
        let draw_3d_map = map_prepare_output.draw_3d_map;
        let static_decals = map_frame_plan.static_decals;
        let semantic_overlays = map_frame_plan.semantic_overlays;
        let world_objects = map_frame_plan.world_objects;
        let prepared_map_frame = map_prepare_output.prepared_map_frame;
        let terrain_ownership = prepared_map_frame.terrain_ownership;
        let water_ownership = prepared_map_frame.water_ownership;
        let map_fallback_report = prepared_map_frame.fallback_report;
        let profile_map_prepare_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();
        let runtime_player_country = if self.view.player_country < self.world.countries.count {
            Some(hoi4_state::CountryId(self.view.player_country as u16))
        } else {
            None
        };
        self.update_vanilla_targets(
            state_guard.as_mut(),
            semantic_overlays,
            runtime_player_country,
            params.season_snow_offset,
            season_result.season_blend,
        );
        let profile_vanilla_targets_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();

        let pass_params_output = self.update_pass_params(
            state_guard.as_mut(),
            &mut params,
            FramePassParamsInput {
                render_quality_preset: ui_phase.render_quality_preset,
                map_frame_plan: &map_frame_plan,
                world_objects,
                static_decals,
                semantic_overlays,
                season_result: &season_result,
                vanilla_map_space: &vanilla_map_space,
                terrain_ownership,
                water_ownership,
                draw_3d_map,
                zoom_factor,
            },
        );
        let profile_pass_params_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();
        let draw_hud_output = self.draw_map_and_hud(
            state_guard.as_mut(),
            &mut enc,
            output_view,
            FrameDrawHudInput {
                map_phase0_debug_lines: &ui_phase.map_phase0_debug_lines,
                render_quality_preset: ui_phase.render_quality_preset,
                map_frame_plan: &map_frame_plan,
                draw_3d_map,
                semantic_overlays,
                map_fallback_report,
                water_final_color: water_ownership.final_color,
                pass_params_output,
                vanilla_map_space: &vanilla_map_space,
                zoom_factor,
                profile_mark,
            },
        );

        self.submit_prepared_render_frame(
            state_guard.into_inner(),
            enc,
            FrameSubmitPhaseInput {
                ui_phase,
                frame,
                output_view,
                capture_texture: capture_texture.as_ref(),
                draw_hud_output,
                profile_params_buckets_ms,
                profile_surface_ms,
                profile_shadow_global_ms,
                profile_map_prepare_ms,
                profile_vanilla_targets_ms,
                profile_pass_params_ms,
            },
        );
    }

    pub(crate) fn submit_prepared_render_frame(
        &mut self,
        s: RenderState,
        enc: wgpu::CommandEncoder,
        input: FrameSubmitPhaseInput<'_>,
    ) {
        let FrameSubmitPhaseInput {
            ui_phase,
            frame,
            output_view,
            capture_texture,
            draw_hud_output,
            profile_params_buckets_ms,
            profile_surface_ms,
            profile_shadow_global_ms,
            profile_map_prepare_ms,
            profile_vanilla_targets_ms,
            profile_pass_params_ms,
        } = input;
        let FrameUiPhaseOutput {
            render_started,
            map_layer_mask,
            map_phase0_active,
            map_phase0_capture_path,
            ui_command_output,
            egui_current_stats,
            profile_world_plan_ms,
            profile_visual_updates_ms,
            profile_ui_data_ms,
            profile_egui_begin_ms,
            profile_ui_commands_ms,
            ..
        } = ui_phase;

        self.submit_render_frame(
            s,
            enc,
            ui_command_output,
            render_frame::submit::FrameSubmitInput {
                render_started,
                profile_mark: Instant::now(),
                frame,
                output_view,
                capture_texture,
                map_phase0_capture_path,
                quality_label: draw_hud_output.quality_label,
                chain_label: draw_hud_output.chain_label,
                profile_world_plan_ms,
                profile_visual_updates_ms,
                profile_ui_data_ms,
                profile_egui_begin_ms,
                profile_ui_commands_ms,
                profile_params_buckets_ms,
                profile_surface_ms,
                profile_shadow_global_ms,
                profile_map_prepare_ms,
                profile_vanilla_targets_ms,
                profile_pass_params_ms,
                profile_map_draw_ms: draw_hud_output.profile_map_draw_ms,
                profile_hud_build_ms: draw_hud_output.profile_hud_build_ms,
                egui_current_stats,
                map_phase0_active,
                ui_enabled: !map_phase0_active || map_layer_mask.ui,
            },
        );
    }

    pub(crate) fn draw_map_and_hud(
        &mut self,
        s: &mut RenderState,
        enc: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
        input: FrameDrawHudInput<'_>,
    ) -> FrameDrawHudOutput {
        let FrameDrawHudInput {
            map_phase0_debug_lines,
            render_quality_preset,
            map_frame_plan,
            draw_3d_map,
            semantic_overlays,
            map_fallback_report,
            water_final_color,
            pass_params_output,
            vanilla_map_space,
            zoom_factor,
            profile_mark,
        } = input;

        let postprocess_lut_selection =
            postprocess_lut_selection_for(&self.camera, &self.world, vanilla_map_space);
        let map_renderer = s.map_renderer.clone();
        let terrain_counts = s.terrain_bucket_counts;
        let map_draw_output = map_renderer.render_frame(
            s,
            enc,
            map_draw::MapDrawInput {
                frame_plan: map_frame_plan,
                terrain_counts,
                draw_3d_map,
                show_province_names: self.render_toggles.show_province_names,
                zoom_factor,
                postprocess_debug_view: self.render_toggles.postprocess_debug_view,
                postprocess_chain_enabled: render_quality_preset.controls().postprocess_chain,
                postprocess_lut_selection,
                output_view,
            },
        );
        let profile_map_draw_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        let hud_started = Instant::now();

        let hud_output = self.build_frame_hud(
            s,
            render_frame::hud::FrameHudInput {
                hud_enabled: !self.audit.map_phase0.is_some() || self.current_map_layer_mask().ui,
                map_phase0_debug_lines,
                render_quality_preset,
                chain_label: map_draw_output.chain_label,
                postprocess_lut_selection,
                use_full_chain: map_draw_output.use_full_chain,
                water_final_color,
                water_refraction_available: pass_params_output.water_refraction_available,
                selected_water_effect_name: pass_params_output.selected_water_effect_name,
                semantic_overlays,
                map_fallback_report,
                zoom_factor,
                started_at: hud_started,
            },
        );

        FrameDrawHudOutput {
            quality_label: hud_output.quality_label,
            chain_label: map_draw_output.chain_label,
            profile_map_draw_ms,
            profile_hud_build_ms: hud_output.profile_hud_build_ms,
        }
    }

    pub(crate) fn prepare_visual_ui_phase(
        &mut self,
        render_started: Instant,
    ) -> Option<FrameUiPhaseOutput> {
        let render_quality_preset = self.effective_render_quality_preset(render_started);
        let mut profile_mark = render_started;
        let prepare_started = Instant::now();
        self.prepare_map_phase0_capture();
        let map_layer_mask = self.current_map_layer_mask();
        let map_phase0_active = self.audit.map_phase0.is_some();
        let map_phase0_debug_lines = if map_layer_mask.asset_fallback_debug {
            self.map_phase0_debug_lines()
        } else {
            Vec::new()
        };
        let map_phase0_capture_path = if self.map_phase0_capture_ready() {
            self.map_phase0_capture_path()
        } else {
            None
        };
        let pre_frame_world_objects = WorldObjectSystem::plan(
            map_frame::MapFrameInput {
                draw_3d_map: self.view.game_phase == GamePhase::Playing,
                enable_3d_terrain: self.ui_state.settings.enable_3d_terrain,
                map_mode: self.map_mode,
                date: self.world.date,
                selected_province_id: self.selected_province_id,
                hovered_province_id: self.hovered_province_id,
                zoom_factor: {
                    let world_extent = self.camera.world_size.x.max(self.camera.world_size.y);
                    let max_dist = world_extent * 4.0;
                    let zoom_fade_dist = max_dist * 0.7;
                    (1.0 - self.camera.distance / zoom_fade_dist).clamp(0.0, 1.0)
                },
                time_seconds: self.start_time.elapsed().as_secs_f32(),
                screen_size: self
                    .state
                    .as_ref()
                    .map(|s| [s.config.width as f32, s.config.height as f32])
                    .unwrap_or([1.0, 1.0]),
                layer_mask: map_layer_mask,
                quality_preset: render_quality_preset,
            }
            .context(),
        );
        let profile_world_plan_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();
        let counter_started = Instant::now();
        self.update_hoi3_counter_pass(pre_frame_world_objects);
        let counter_update_ms = counter_started.elapsed().as_secs_f32() * 1000.0;
        self.perf.counter_update_us = self
            .perf
            .counter_update_us
            .saturating_add((counter_update_ms * 1000.0) as u64);
        let arrow_started = Instant::now();
        self.update_frontline_overlay();
        self.update_frontline_arrows();
        self.update_trade_routes_overlay();
        let arrow_update_ms = arrow_started.elapsed().as_secs_f32() * 1000.0;
        self.perf.arrow_update_us = self
            .perf
            .arrow_update_us
            .saturating_add((arrow_update_ms * 1000.0) as u64);
        let profile_visual_updates_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();

        let app_ui_enabled = !map_phase0_active || map_layer_mask.ui;
        let ui_data = ui_driver::build::build_ui_data(self, app_ui_enabled);
        let profile_ui_data_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();
        {
            let s = self.state.as_mut()?;
            if let Some(profiler) = s.gpu_profiler.as_mut() {
                profiler.poll_readback(&s.device, &mut s.pass_registry);
            }
            s.pass_registry.begin_frame_stats();
            s.pass_registry
                .record_cpu_ms("hoi3_counter_v3", counter_update_ms);
            s.pass_registry
                .record_cpu_ms("3d_maparrow", arrow_update_ms);
            if let Some(profiler) = s.gpu_profiler.as_mut() {
                profiler.begin_frame();
            }
        }
        let ui_output = ui_driver::render::render_ui(self, ui_data);
        let profile_egui_begin_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();
        let ui_command_output = ui_driver::commands::apply_ui_render_output(self, ui_output);
        let profile_ui_commands_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();
        let egui_current_stats = ui_command_output.egui_current_stats;

        Some(FrameUiPhaseOutput {
            render_started,
            render_quality_preset,
            prepare_started,
            map_layer_mask,
            map_phase0_active,
            map_phase0_debug_lines,
            map_phase0_capture_path,
            ui_command_output,
            egui_current_stats,
            profile_world_plan_ms,
            profile_visual_updates_ms,
            profile_ui_data_ms,
            profile_egui_begin_ms,
            profile_ui_commands_ms,
            profile_mark,
        })
    }

    pub(crate) fn prepare_render_params(
        &self,
        s: &RenderState,
        _input: &FramePrepareInput,
    ) -> FramePrepareOutput {
        let world_extent = self.camera.world_size.x.max(self.camera.world_size.y);
        let max_dist = world_extent * 4.0;
        let zoom_fade_dist = max_dist * 0.7;
        let zoom_factor = (1.0 - self.camera.distance / zoom_fade_dist).clamp(0.0, 1.0);
        let time = self.start_time.elapsed().as_secs_f32();
        let date = self.world.date;
        let mut params = RenderParams::new();
        params.selected_province_id = self.selected_province_id;
        params.zoom_factor = zoom_factor;
        params.time = time;
        params.screen_width = s.config.width as f32;
        params.screen_height = s.config.height as f32;
        params.vignette_strength = 0.05;
        params.sun_dir = RenderParams::shadow_sun_dir();
        params.month_phase = RenderParams::compute_month_phase(date.month, date.day);
        params.season_snow_offset = RenderParams::compute_season_snow_offset(params.month_phase);
        params.map_mode_terrain_blend = map_mode_terrain_blend_for(self.map_mode);
        params.diplomacy_mode = match self.map_mode {
            MapMode::Terrain => 0,
            _ => 1,
        };

        FramePrepareOutput {
            params,
            zoom_factor,
            time,
            date,
            season_result: self
                .seasons
                .season_for_date(date.month as u32, date.day as u32),
            vanilla_map_space: VanillaMapSpace::from_world_size([
                self.camera.world_size.x,
                self.camera.world_size.y,
            ]),
        }
    }

    pub(crate) fn update_terrain_buckets(&self, s: &mut RenderState) {
        let terrain_bucket_stats = build_wrapped_instance_buckets_into(
            &s.chunk_grid,
            &self.camera,
            &mut s.terrain_buckets,
        );
        for lod in 0..3 {
            let bucket_sig = terrain_bucket_stats.signatures[lod];
            let bucket_count = terrain_bucket_stats.counts[lod];
            if s.terrain_bucket_signature[lod] == bucket_sig
                && s.terrain_bucket_counts[lod] == bucket_count
            {
                continue;
            }
            s.terrain_bucket_signature[lod] = bucket_sig;
            s.terrain_bucket_counts[lod] = bucket_count;
            if s.terrain_buckets[lod].is_empty() {
                continue;
            }
            let needed = bucket_count;
            if needed > s.instance_capacity[lod] {
                s.instance_buffers[lod] = s.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("chunk_instances"),
                    size: (needed as u64) * std::mem::size_of::<ChunkInstance>() as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                s.instance_capacity[lod] = needed;
            }
            s.queue.write_buffer(
                &s.instance_buffers[lod],
                0,
                bytemuck::cast_slice(&s.terrain_buckets[lod]),
            );
        }
    }

    pub(crate) fn acquire_surface_and_capture_target(
        &self,
        s: &mut RenderState,
        map_phase0_active: bool,
    ) -> Option<FrameSurfaceTarget> {
        let frame = SurfaceFrameGuard::new(s.surface.get_current_texture().ok()?);
        let surface_view = frame.texture()?.create_view(&Default::default());
        let capture_texture = if map_phase0_active {
            Some(s.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("map_phase0_output"),
                size: wgpu::Extent3d {
                    width: s.config.width,
                    height: s.config.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: s.config.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            }))
        } else {
            None
        };
        let capture_view = capture_texture
            .as_ref()
            .map(|texture| texture.create_view(&Default::default()));
        let enc = s.device.create_command_encoder(&Default::default());
        Some(FrameSurfaceTarget {
            frame,
            surface_view,
            capture_texture,
            capture_view,
            enc,
        })
    }

    pub(crate) fn update_global_uniforms(
        &self,
        s: &mut RenderState,
        params: &RenderParams,
        vanilla_map_space: &VanillaMapSpace,
        time: f32,
        season_blend: f32,
    ) {
        let shadow_vp = s.shadow_pass.update_shadow_view_proj(
            &s.queue,
            self.camera.target,
            self.camera.world_size,
            HEIGHT_SCALE,
        );
        let mut gu = GlobalFrameUniform::default();
        gu.view_proj = self.camera.view_proj().to_cols_array_2d();
        let cam_eye = self.camera.eye();
        gu.cam_pos = [cam_eye.x, cam_eye.y, cam_eye.z];
        gu.vanilla_map_size_world_size = [
            vanilla_map_space.map_size_px[0],
            vanilla_map_space.map_size_px[1],
            vanilla_map_space.world_size[0],
            vanilla_map_space.world_size[1],
        ];
        let cam_map_px = vanilla_map_space.world_xz_to_map_px([cam_eye.x, cam_eye.z]);
        gu.cam_pos_map_px = [cam_map_px[0], cam_map_px[1], 0.0, 0.0];
        gu.cam_look_at_dir = {
            let d = (self.camera.target - cam_eye).normalize_or_zero();
            [d.x, d.y, d.z]
        };
        gu.global_time = time;
        gu.fow_opacity_time_snow_max_speed =
            [params.season_snow_offset.max(0.0), time, season_blend, 5.0];
        gu.day_night_hour_sun_dir = {
            let sd = RenderParams::compute_sun_dir(12, self.world.date.month);
            let hour = (self.visual_day_night_hour / 24.0).rem_euclid(1.0);
            [hour, sd[0], sd[1], sd[2]]
        };
        gu.screen_size = [s.config.width as f32, s.config.height as f32];
        gu.cubemap_intensity = 1.35;
        gu.sun_specular_intensity = 1.45;
        gu.sun_diffuse_intensity = [1.08, 1.05, 0.98, 1.0];
        gu.shadow_view_proj = shadow_vp.to_cols_array_2d();
        s.global_uniform_buf.write(&s.queue, &gu);
    }

    pub(crate) fn prepare_map_renderer_frame(
        &mut self,
        s: &RenderState,
        input: &FramePrepareInput,
        date: hoi4_state::GameDate,
        zoom_factor: f32,
        time: f32,
        prepare_started: Instant,
    ) -> FrameMapPrepareOutput {
        let draw_3d_map = self.view.game_phase == GamePhase::Playing;
        let render_quality_preset = if self.render_toggles.force_water_pass {
            MapQualityPreset::High
        } else {
            input.render_quality_preset
        };
        let map_frame_plan = s.map_renderer.build_frame_plan(
            crate::map_renderer::MapFrameContext {
                draw_3d_map: draw_3d_map && self.ui_state.settings.enable_3d_terrain,
                map_mode: self.map_mode,
                date,
                selected_province_id: self.selected_province_id,
                hovered_province_id: self.hovered_province_id,
                zoom_factor,
                time_seconds: time,
                screen_size: [s.config.width as f32, s.config.height as f32],
                settings: crate::map_renderer::MapRenderSettings::with_quality(
                    input.map_layer_mask,
                    render_quality_preset,
                ),
            },
            &s.pass_registry,
        );
        self.perf.last_map_prepare_cpu_ms = prepare_started.elapsed().as_secs_f32() * 1000.0;
        let prepared_map_frame = s.map_renderer.prepare_frame(
            &map_frame_plan,
            MapPrepareFrameInput {
                layer_mask: input.map_layer_mask,
                dedicated_water_loaded: s.water_pass.any_loaded
                    || self.render_toggles.force_water_pass,
                dedicated_river_loaded: s.river_pass.any_loaded,
                dedicated_border_loaded: s.border_pass.any_loaded,
            },
        );

        FrameMapPrepareOutput {
            map_frame_plan,
            prepared_map_frame,
            draw_3d_map,
        }
    }

    pub(crate) fn update_vanilla_targets(
        &self,
        s: &mut RenderState,
        semantic_overlays: crate::map_renderer::SemanticOverlayPlan,
        runtime_player_country: Option<hoi4_state::CountryId>,
        season_snow_offset: f32,
        season_blend: f32,
    ) {
        s.vanilla_targets.update_frame(
            &s.queue,
            &self.world,
            &VanillaRuntimeTargetFrameParams {
                selected_province_id: self.selected_province_id,
                hovered_province_id: semantic_overlays.hovered_province_id,
                map_mode: self.map_mode,
                player_country: runtime_player_country,
                battle_plan_opacity: semantic_overlays
                    .frontlines
                    .opacity
                    .max(semantic_overlays.arrows.opacity),
                naval_dominance_opacity: semantic_overlays.straits.opacity,
                occupation_opacity: 0.0,
                selected_opacity: semantic_overlays.selected_province_pulse.opacity,
                hover_opacity: semantic_overlays.hover_highlight.opacity,
                map_mode_overlay_opacity: semantic_overlays.map_mode_overlay.opacity,
                season_snow_offset,
                season_blend,
            },
        );
    }

    pub(crate) fn update_pass_params(
        &self,
        s: &mut RenderState,
        params: &mut RenderParams,
        input: FramePassParamsInput<'_>,
    ) -> FramePassParamsOutput {
        let FramePassParamsInput {
            render_quality_preset,
            map_frame_plan,
            world_objects,
            static_decals,
            semantic_overlays,
            season_result,
            vanilla_map_space,
            terrain_ownership,
            water_ownership,
            draw_3d_map,
            zoom_factor,
        } = input;

        {
            let cam_dist_norm = 1.0 - zoom_factor;
            let sel_intensity = if self.selected_province_id != u32::MAX {
                0.6
            } else {
                0.0
            };
            let enabled_mask =
                if self.render_toggles.border_debug_view == passes::BorderDebugView::Off {
                    passes::BorderParams::DEFAULT_VISIBLE_MASK
                } else {
                    passes::BorderParams::ALL_VISIBLE_MASK
                };
            let bp = passes::BorderParams {
                cam_distance_norm: cam_dist_norm,
                selection_intensity: sel_intensity,
                enabled_mask,
                selected_province_id: self.selected_province_id,
                debug_view: self.render_toggles.border_debug_view.as_shader_value(),
                screen_width: s.config.width as f32,
                screen_height: s.config.height as f32,
                camera_distance_world: self.camera.distance,
                world_size: vanilla_map_space.world_size,
                map_size_px: vanilla_map_space.map_size_px,
            };
            s.border_pass.update_params(&s.queue, &bp);
        }

        s.hoi3_counter_pass.update_opacity(
            &s.queue,
            world_objects.counters.opacity,
            s.config.width as f32,
            s.config.height as f32,
            self.camera.view_proj().to_cols_array_2d(),
            self.start_time.elapsed().as_secs_f32(),
        );

        params.object_opacity = world_objects.trees.opacity;
        params.object_scale = world_objects.trees.scale;
        s.queue
            .write_buffer(&s.params_buffer, 0, bytemuck::bytes_of(params));

        if let Some(tf) = s.tree_full_pass.as_mut() {
            let tf_params = passes::TreeFullParams {
                season_lerp: season_result.season_lerp,
                season_column: season_result.season_column,
                fade_start: 24.0 + 10.0 * (1.0 - world_objects.trees.opacity),
                fade_end: 38.0 + 12.0 * (1.0 - world_objects.trees.opacity),
                world_w: vanilla_map_space.world_size[0],
                world_d: vanilla_map_space.world_size[1],
                season_column_next: season_result.season_column_next,
                season_blend: season_result.season_blend,
                opacity: world_objects.trees.opacity,
                scale: world_objects.trees.scale,
                _pad0: 0.0,
                _pad1: 0.0,
            };
            let eye = self.camera.eye();
            let cam_pos = [eye.x, eye.y, eye.z];
            let last = s.tree_lod_last_cam_pos;
            let moved_sq = (cam_pos[0] - last[0]).powi(2)
                + (cam_pos[1] - last[1]).powi(2)
                + (cam_pos[2] - last[2]).powi(2);
            let should_upload_trees = !s.tree_lod_uploaded || moved_sq > 25.0;
            tf.update_params(&s.queue, &tf_params);
            if should_upload_trees {
                tf.upload_instances(&s.device, &s.queue, cam_pos);
                s.tree_lod_last_cam_pos = cam_pos;
                s.tree_lod_uploaded = true;
            }
        }

        let water_runtime_quality = if self.render_toggles.force_water_pass {
            MapQualityPreset::High
        } else {
            render_quality_preset
        };
        let water_refraction_available = draw_3d_map
            && map_frame_plan.draw.water_refraction
            && map_frame_plan.draw.water
            && (s.water_pass.any_loaded || self.render_toggles.force_water_pass)
            && (render_quality_preset.water_refraction_enabled()
                || self.render_toggles.force_water_pass);
        let selected_water_effect = s
            .water_pass
            .update_runtime_effect(water_refraction_available, water_runtime_quality);
        s.water_pass.update_params(
            &s.queue,
            &passes::WaterParams {
                world_w: vanilla_map_space.world_size[0],
                world_d: vanilla_map_space.world_size[1],
                height_scale: HEIGHT_SCALE,
                selected_province_id: self.selected_province_id,
                debug_view: self.render_toggles.water_debug_view.as_shader_value(),
                final_water_owner: if water_ownership.final_color { 1 } else { 0 },
                effect_variant: selected_water_effect.as_shader_value(),
                refraction_available: u32::from(water_refraction_available),
                ..passes::WaterParams::default()
            },
        );
        s.river_pass.update_params(
            &s.queue,
            &passes::RiverParams {
                world_w: vanilla_map_space.world_size[0],
                world_d: vanilla_map_space.world_size[1],
                height_scale: HEIGHT_SCALE,
                zoom_factor,
                ..passes::RiverParams::default()
            },
        );

        s.queue.write_buffer(
            &s.railways_params_buffer,
            0,
            bytemuck::bytes_of(&RailwayParams {
                alpha: static_decals.railways.opacity,
                zoom_factor,
                ..RailwayParams::default()
            }),
        );
        s.queue.write_buffer(
            &s.frontlines_params_buffer,
            0,
            bytemuck::bytes_of(&FrontlineParams {
                opacity: semantic_overlays.frontlines.opacity,
                _pad: [0.0; 3],
            }),
        );
        s.queue.write_buffer(
            &s.buildings_params_buffer,
            0,
            bytemuck::bytes_of(&BuildingParams {
                opacity: world_objects.buildings.opacity,
                scale: world_objects.buildings.scale,
                brightness: 0.86,
                _pad0: 0.0,
            }),
        );
        s.pdxmesh_pass.update_phase8_controls(
            &s.queue,
            world_objects.buildings.opacity,
            world_objects.buildings.scale,
            0.86,
            season_result
                .season_blend
                .max(params.season_snow_offset.max(0.0)),
        );
        {
            let eye = self.camera.eye();
            s.pdxmesh_pass
                .set_lod_bias(render_quality_preset.controls().object_lod_bias);
            s.pdxmesh_pass
                .ensure_lod_uploaded(&s.device, &s.queue, [eye.x, eye.y, eye.z]);
        }

        let particle_quality = render_quality_preset.controls().particle_density;
        s.particle_pass.update_params(
            &s.queue,
            80.0 / particle_quality.max(0.25),
            400.0 / particle_quality.max(0.25),
        );
        s.maparrow_pass.update_params(
            &s.queue,
            &passes::maparrow::ArrowParams {
                overlay_opacity: semantic_overlays.arrows.opacity,
                frontline_opacity: semantic_overlays.frontlines.opacity,
                ..passes::maparrow::ArrowParams::default()
            },
        );
        s.traderoute_pass.update_params(
            &s.queue,
            &passes::traderoute::TradeRouteParams {
                opacity: semantic_overlays.trade_routes.opacity,
                ..passes::traderoute::TradeRouteParams::default()
            },
        );
        s.strait_pass.update_params(
            &s.queue,
            &passes::strait::StraitParams {
                opacity: semantic_overlays.straits.opacity,
                ..passes::strait::StraitParams::default()
            },
        );
        if let Some(mnp) = s.mapname_pass.as_ref() {
            mnp.update_params(
                &s.queue,
                world_objects.country_names.opacity,
                world_objects.country_names.scale,
            );
        }
        if let Some(pnp) = s.province_name_pass.as_ref() {
            pnp.update_params(
                &s.queue,
                world_objects.province_names.opacity,
                world_objects.province_names.scale,
            );
        }
        if let Some(poi) = s.poi_icon_pass.as_ref() {
            poi.update_params(
                &s.queue,
                &passes::poi_icon::PoiIconParams {
                    opacity: world_objects.poi_icons.opacity,
                    scale: world_objects.poi_icons.scale,
                    outline_strength: 0.82,
                    _pad0: 0.0,
                },
            );
        }
        if let Some(poi) = s.poi_icon_pass.as_mut() {
            if s.poi_zoom_bucket != world_objects.poi_detail_level {
                poi.upload(
                    &s.device,
                    &s.queue,
                    &s.poi_icon_instances,
                    world_objects.poi_detail_level,
                );
                s.poi_zoom_bucket = world_objects.poi_detail_level;
            }
        }

        let selected_state_id = if self.selected_province_id != u32::MAX {
            let pi = self.selected_province_id as usize;
            self.world
                .provinces
                .state_of
                .get(pi)
                .copied()
                .filter(|sid| !sid.is_none())
                .map(|sid| sid.0 as u32)
                .unwrap_or(u32::MAX)
        } else {
            u32::MAX
        };
        let pdx_params = passes::PdxMapParams {
            selected_province_id: self.selected_province_id,
            selected_state_id,
            hovered_province_id: semantic_overlays.hovered_province_id,
            terrain_blend: params.map_mode_terrain_blend,
            screen_width: params.screen_width,
            screen_height: params.screen_height,
            vignette_strength: 0.0,
            zoom_factor: params.zoom_factor,
            border_country_px: if terrain_ownership.terrain_sdf_borders {
                params.border_country_px
            } else {
                0.0
            },
            border_province_px: if terrain_ownership.terrain_sdf_borders {
                params.border_province_px
            } else {
                0.0
            },
            season_lerp: season_result.season_lerp,
            map_mode_terrain_blend: params.map_mode_terrain_blend,
            world_size_xy_height_lat: [
                vanilla_map_space.world_size[0],
                vanilla_map_space.world_size[1],
                HEIGHT_SCALE,
                LAT_CORRECTION,
            ],
            season_params: [
                season_result.season_column,
                params.season_snow_offset,
                season_result.season_column_next,
                season_result.season_blend,
            ],
            terrain_controls: [
                self.render_toggles.terrain_debug_view.as_shader_value(),
                if terrain_ownership.terrain_water_final_color {
                    1.0
                } else {
                    0.0
                },
                if terrain_ownership.terrain_sdf_borders {
                    1.0
                } else {
                    0.0
                },
                if terrain_ownership.terrain_overlays {
                    1.0
                } else {
                    0.0
                },
            ],
            overlay_controls: [
                0.0,
                semantic_overlays.selected_province_pulse.opacity,
                semantic_overlays.hover_highlight.opacity,
                semantic_overlays.map_mode_overlay.opacity,
            ],
            feature_flags: [
                1.0,
                if terrain_ownership.terrain_overlays
                    && map_frame_plan.draw.river
                    && !s.river_pass.any_loaded
                {
                    1.0
                } else {
                    0.0
                },
                0.0,
                0.0,
            ],
            atlas_idx_array: {
                let src = self.world.map.terrain_catalog.atlas_idx_array_256();
                std::array::from_fn::<[u32; 4], 64, _>(|r| {
                    std::array::from_fn::<u32, 4, _>(|c| src[r * 4 + c] as u32)
                })
            },
            terrain_flags_array: {
                let src = self.world.map.terrain_catalog.terrain_flags_array_256();
                std::array::from_fn::<[u32; 4], 64, _>(|r| {
                    std::array::from_fn::<u32, 4, _>(|c| src[r * 4 + c] as u32)
                })
            },
        };
        s.terrain_pass.update_params(&s.queue, &pdx_params);

        FramePassParamsOutput {
            water_refraction_available,
            selected_water_effect_name: selected_water_effect.effect_name(),
        }
    }
}
