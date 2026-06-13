use std::path::PathBuf;

use crate::*;

pub(crate) struct FrameSubmitInput<'a> {
    pub(crate) render_started: Instant,
    pub(crate) profile_mark: Instant,
    pub(crate) frame: render_frame::prepare::SurfaceFrameGuard,
    pub(crate) output_view: &'a wgpu::TextureView,
    pub(crate) capture_texture: Option<&'a wgpu::Texture>,
    pub(crate) map_phase0_capture_path: Option<PathBuf>,
    pub(crate) quality_label: String,
    pub(crate) chain_label: &'static str,
    pub(crate) profile_world_plan_ms: f32,
    pub(crate) profile_visual_updates_ms: f32,
    pub(crate) profile_ui_data_ms: f32,
    pub(crate) profile_egui_begin_ms: f32,
    pub(crate) profile_ui_commands_ms: f32,
    pub(crate) profile_params_buckets_ms: f32,
    pub(crate) profile_surface_ms: f32,
    pub(crate) profile_shadow_global_ms: f32,
    pub(crate) profile_map_prepare_ms: f32,
    pub(crate) profile_vanilla_targets_ms: f32,
    pub(crate) profile_pass_params_ms: f32,
    pub(crate) profile_map_draw_ms: f32,
    pub(crate) profile_hud_build_ms: f32,
    pub(crate) egui_current_stats: hoi4_ui::UiFrameStats,
    pub(crate) map_phase0_active: bool,
    pub(crate) ui_enabled: bool,
}

impl App {
    pub(crate) fn submit_render_frame(
        &mut self,
        mut s: RenderState,
        mut enc: wgpu::CommandEncoder,
        ui_command_output: ui_driver::commands::UiCommandApplyOutput,
        input: FrameSubmitInput<'_>,
    ) {
        let FrameSubmitInput {
            render_started,
            profile_mark,
            frame,
            output_view,
            capture_texture,
            map_phase0_capture_path,
            quality_label,
            chain_label,
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
            profile_map_draw_ms,
            profile_hud_build_ms,
            egui_current_stats,
            map_phase0_active,
            ui_enabled,
        } = input;
        let mut profile_mark = profile_mark;
        let ui_started = Instant::now();
        s.text_pass.prepare(&s.queue);
        s.panel_pass.prepare(&s.queue);
        // Legacy UI command rendering is removed; panel/text/flag passes remain.
        let dpi = s.ui_scale_factor();
        let logical_sw = s.config.width as f32 / dpi.max(0.0001);
        let logical_sh = s.config.height as f32 / dpi.max(0.0001);
        s.queue.write_buffer(
            &s.flag_uniform_buffer,
            0,
            bytemuck::bytes_of(&[logical_sw, logical_sh]),
        );
        let flag_to_draw: Option<(String, String, [f32; 4])> = match self.view.game_phase {
            GamePhase::CountrySelect => self.view.last_country_layout.as_ref().and_then(|layout| {
                self.view
                    .available_countries
                    .get(self.view.country_select_idx)
                    .map(|entry| {
                        let (fx, fy, fw, fh) = layout.flag_rect;
                        (entry.tag.clone(), entry.ideology.clone(), [fx, fy, fw, fh])
                    })
            }),
            GamePhase::Playing | GamePhase::MainMenu => None,
        };

        if let Some((_, _, [fx, fy, fw, fh])) = &flag_to_draw {
            let v: [[f32; 4]; 6] = [
                [*fx, *fy, 0.0, 0.0],
                [*fx + *fw, *fy, 1.0, 0.0],
                [*fx, *fy + *fh, 0.0, 1.0],
                [*fx + *fw, *fy, 1.0, 0.0],
                [*fx + *fw, *fy + *fh, 1.0, 1.0],
                [*fx, *fy + *fh, 0.0, 1.0],
            ];
            s.queue
                .write_buffer(&s.flag_vertex_buffer, 0, bytemuck::cast_slice(&v));
        }

        let ui_cbufs = if ui_enabled {
            let ui_gpu_token = s
                .gpu_profiler
                .as_mut()
                .and_then(|profiler| profiler.begin_encoder_span(&mut enc, "ui"));
            {
                let mut ui_rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("ui_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: output_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load, // preserve 3D content
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    ..Default::default()
                });
                s.panel_pass.render(&mut ui_rp);

                // 4.1.bis.10 fix-fix (2026-05-16): the country flag draws AFTER
                // ui_pass ???vanilla's `GFX_shield_medium` is the metallic shield
                // frame and our renderer blits it as a single opaque quad, so
                // drawing the flag underneath would just be hidden. The correct
                // approximation (until we wire up `maskedflag.lua` masked composite)
                // is: paint the flag on top, sized to fill the shield frame.
                if let Some((tag, ideology, _)) = &flag_to_draw {
                    let flag_bg = s.flag_bank.get_or_load(
                        &s.device,
                        &s.queue,
                        &self.path_cfg,
                        &s.flag_bgl,
                        &s.flag_uniform_buffer,
                        tag,
                        ideology,
                    );
                    ui_rp.set_pipeline(&s.flag_pipeline);
                    ui_rp.set_bind_group(0, flag_bg, &[]);
                    ui_rp.set_vertex_buffer(0, s.flag_vertex_buffer.slice(..));
                    ui_rp.draw(0..6, 0..1);
                }

                s.text_pass.render(&mut ui_rp);
            }

            let ui_cbufs = s.ui.paint(
                &s.device,
                &s.queue,
                &mut enc,
                &s.window,
                output_view,
                [s.config.width, s.config.height],
            );
            if let (Some(profiler), Some(token)) = (s.gpu_profiler.as_mut(), ui_gpu_token) {
                profiler.end_encoder_span(&mut enc, token);
            }
            s.pass_registry.record_draw_calls("ui", 3);
            ui_cbufs
        } else {
            s.pass_registry.record_draw_calls("ui", 0);
            Vec::new()
        };
        s.pass_registry
            .record_cpu_ms("ui", ui_started.elapsed().as_secs_f32() * 1000.0);
        let profile_ui_render_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();

        let pending_readback =
            if let (Some(texture), Some(path)) = (capture_texture, map_phase0_capture_path) {
                Some(enqueue_png_readback(
                    &s.device,
                    &mut enc,
                    texture,
                    s.config.format,
                    s.config.width,
                    s.config.height,
                    path,
                ))
            } else {
                None
            };

        if let Some(profiler) = s.gpu_profiler.as_mut() {
            profiler.finish_frame(&mut enc);
        }

        let profile_pre_submit_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        profile_mark = Instant::now();
        s.queue
            .submit(ui_cbufs.into_iter().chain(std::iter::once(enc.finish())));
        let profile_submit_ms = profile_mark.elapsed().as_secs_f32() * 1000.0;
        if let Some(profiler) = s.gpu_profiler.as_mut() {
            profiler.map_pending_readback();
        }
        let frame_submit_us = render_started.elapsed().as_micros() as u64;
        let frame_time_ms = frame_submit_us as f32 / 1000.0;
        let present_started = Instant::now();
        frame.present();
        let profile_present_ms = present_started.elapsed().as_secs_f32() * 1000.0;
        self.last_redraw_at = Instant::now();
        self.perf.last_frame_cpu_ms = frame_time_ms;
        if (frame_time_ms >= 25.0 || profile_present_ms >= 10.0)
            && self.perf.last_render_profile_log.elapsed().as_millis() >= 250
        {
            let speed = match self.world.speed {
                GameSpeed::Paused => "P",
                GameSpeed::Speed1 => "1",
                GameSpeed::Speed2 => "2",
                GameSpeed::Speed3 => "3",
                GameSpeed::Speed4 => "4",
                GameSpeed::Speed5 => "5",
            };
            println!(
                "[render-prof] speed={} quality={} chain={} date={} frame={:.2}ms present={:.2}ms world={:.2}ms visual={:.2}ms ui_data={:.2}ms egui_begin={:.2}ms egui_input={:.2}ms egui_run={:.2}ms ui_cmd={:.2}ms params_buckets={:.2}ms surface={:.2}ms shadow_global={:.2}ms map_prepare={:.2}ms vanilla_targets={:.2}ms pass_params={:.2}ms map_draw={:.2}ms hud_build={:.2}ms ui_render={:.2}ms pre_submit={:.2}ms submit={:.2}ms",
                speed,
                quality_label,
                chain_label,
                self.world.date,
                frame_time_ms,
                profile_present_ms,
                profile_world_plan_ms,
                profile_visual_updates_ms,
                profile_ui_data_ms,
                profile_egui_begin_ms,
                egui_current_stats.input_us as f32 / 1000.0,
                egui_current_stats.run_us as f32 / 1000.0,
                profile_ui_commands_ms,
                profile_params_buckets_ms,
                profile_surface_ms,
                profile_shadow_global_ms,
                profile_map_prepare_ms,
                profile_vanilla_targets_ms,
                profile_pass_params_ms,
                profile_map_draw_ms,
                profile_hud_build_ms,
                profile_ui_render_ms,
                profile_pre_submit_ms,
                profile_submit_ms,
            );
            self.perf.last_render_profile_log = Instant::now();
        }
        let capture_result =
            pending_readback.map(|pending| finish_png_readback(&s.device, pending));
        self.record_edge_pan_test_frame(
            frame_time_ms,
            profile_surface_ms,
            profile_present_ms,
            profile_egui_begin_ms,
            profile_vanilla_targets_ms,
        );
        self.state = Some(s);
        ui_driver::commands::apply_deferred_ui_commands(self, ui_command_output);
        if let Some(result) = capture_result {
            self.map_phase0_after_capture(frame_time_ms, result);
        } else if map_phase0_active {
            self.map_phase0_after_uncaptured_frame();
        }
        self.perf.render_us = self.perf.render_us.saturating_add(frame_submit_us);
        self.perf.render_frames = self.perf.render_frames.saturating_add(1);
    }
}
