use crate::*;

pub(crate) struct FrameHudInput<'a> {
    pub(crate) hud_enabled: bool,
    pub(crate) map_phase0_debug_lines: &'a [String],
    pub(crate) render_quality_preset: MapQualityPreset,
    pub(crate) chain_label: &'static str,
    pub(crate) postprocess_lut_selection: passes::PostProcessLutSelection,
    pub(crate) use_full_chain: bool,
    pub(crate) water_final_color: bool,
    pub(crate) water_refraction_available: bool,
    pub(crate) selected_water_effect_name: &'static str,
    pub(crate) semantic_overlays: crate::map_renderer::SemanticOverlayPlan,
    pub(crate) map_fallback_report: crate::map_renderer::MapPassFallbackReport,
    pub(crate) zoom_factor: f32,
    pub(crate) started_at: Instant,
}

pub(crate) struct FrameHudOutput {
    pub(crate) quality_label: String,
    pub(crate) profile_hud_build_ms: f32,
}

impl App {
    pub(crate) fn build_frame_hud(
        &mut self,
        s: &mut RenderState,
        input: FrameHudInput<'_>,
    ) -> FrameHudOutput {
        let FrameHudInput {
            hud_enabled,
            map_phase0_debug_lines,
            render_quality_preset,
            chain_label,
            postprocess_lut_selection,
            use_full_chain,
            water_final_color,
            water_refraction_available,
            selected_water_effect_name,
            semantic_overlays,
            map_fallback_report,
            zoom_factor,
            started_at,
        } = input;
        // F4 ??????????????pass ?????/ ????????????DebugOverlay???I ???????text_pass ??????
        let quality_label = if render_quality_preset == self.render_toggles.map_quality_preset {
            self.render_toggles.map_quality_preset.as_str().to_string()
        } else {
            format!(
                "{}->{}",
                self.render_toggles.map_quality_preset.as_str(),
                render_quality_preset.as_str()
            )
        };
        let postprocess_summary = s.post_process.calibration.summary();
        let postprocess_lut_summary = s
            .post_process
            .runtime_lut_summary(postprocess_lut_selection);
        s.debug_render_overlay.refresh(
            &s.pass_registry,
            s.hdr_target.width,
            s.hdr_target.height,
            &format!(
                "{} quality={} post_debug={} {} {} terrain_debug={} water_debug={} border_debug={} overlay_budget={:.2}/{:.2}/{:.2} map_fallbacks={} degraded={}",
                chain_label,
                quality_label,
                s.post_process.debug_view.name(),
                postprocess_summary,
                postprocess_lut_summary,
                self.render_toggles.terrain_debug_view.name(),
                self.render_toggles.water_debug_view.name(),
                self.render_toggles.border_debug_view.name(),
                semantic_overlays.budget.passive,
                semantic_overlays.budget.active,
                semantic_overlays.budget.map_mode,
                map_fallback_report.fallback_count(),
                map_fallback_report.is_degraded()
            ),
        );
        s.debug_render_overlay.append_lines([format!(
            "water_owner={} water_force={} water_refraction={}/{:?} available={} effect={} target={}x{} memory={:.1}MiB producer=fullscreen_9tap",
            if water_final_color {
                "water_pass"
            } else {
                "terrain_fallback"
            },
            self.render_toggles.force_water_pass,
            s.water_refraction_target.resolution_label(),
            s.water_refraction_target.format,
            water_refraction_available,
            selected_water_effect_name,
            s.water_refraction_target.width,
            s.water_refraction_target.height,
            s.water_refraction_target.memory_bytes() as f32 / (1024.0 * 1024.0)
        )]);
        let gpu_status = s
            .gpu_profiler
            .as_ref()
            .map(|profiler| profiler.status())
            .unwrap_or(GpuProfilerStatus::UNSUPPORTED);
        let texture_memory_bytes =
            estimate_frame_texture_memory_bytes(s.config.width, s.config.height, use_full_chain);
        let phase10_lines = phase10_overlay_lines(
            Phase10OverlayInput {
                preset: render_quality_preset,
                width: s.config.width,
                height: s.config.height,
                last_frame_cpu_ms: self.perf.last_frame_cpu_ms,
                cpu_prepare_ms: self.perf.last_map_prepare_cpu_ms,
                texture_memory_bytes,
                gpu_status,
            },
            &s.pass_registry,
        );
        s.debug_render_overlay.append_lines(phase10_lines);

        s.panel_pass.clear();
        let dpi = s.ui_scale_factor();
        let logical_sw = s.config.width as f32 / dpi.max(0.0001);
        let logical_sh = s.config.height as f32 / dpi.max(0.0001);
        s.panel_pass
            .update_screen_size(&s.queue, logical_sw, logical_sh);

        // Phase 4.2: Render different UI based on game phase.
        s.text_pass.clear();
        if !map_phase0_debug_lines.is_empty() {
            for (idx, line) in map_phase0_debug_lines.iter().enumerate() {
                let size = if idx == 0 {
                    TextSize::Heading
                } else {
                    TextSize::Body
                };
                s.text_pass.draw_text_sized(
                    line,
                    24.0,
                    36.0 + idx as f32 * 24.0,
                    TextAlign::Left,
                    logical_sw - 48.0,
                    size,
                );
            }
        }
        if !hud_enabled {
            return FrameHudOutput {
                quality_label,
                profile_hud_build_ms: started_at.elapsed().as_secs_f32() * 1000.0,
            };
        }
        // Phase 4.2 (redesign): Switch between menu rendering and topbar rendering.
        if self.view.game_phase != GamePhase::Playing {
            let dpi = s.ui_scale_factor();
            let sw = s.config.width as f32 / dpi.max(0.0001);
            let sh = s.config.height as f32 / dpi.max(0.0001);

            s.panel_pass
                .push(panel_pass::Panel::full_screen_dim(sw, sh, 0.92));

            match self.view.game_phase {
                GamePhase::MainMenu => {
                    let layout = menu_pass::draw_main_menu(
                        &mut s.panel_pass,
                        &mut s.text_pass,
                        sw,
                        sh,
                        self.view.menu_hovered_btn,
                    );
                    self.view.last_main_buttons = layout.buttons;
                    self.view.last_country_layout = None;
                }
                GamePhase::CountrySelect => {
                    let layout = menu_pass::draw_country_select(
                        &mut s.panel_pass,
                        &mut s.text_pass,
                        sw,
                        sh,
                        &self.view.available_countries,
                        self.view.country_select_idx,
                        self.view.menu_hovered_btn,
                        self.view.menu_hovered_row,
                    );
                    // ?????????
                    if let Some(entry) = self
                        .view
                        .available_countries
                        .get(self.view.country_select_idx)
                    {
                        let world_idx = self
                            .world
                            .countries
                            .tags
                            .iter()
                            .position(|t| t == &entry.tag);
                        let (mp, civ, mil, dock, stab, ws) = if let Some(i) = world_idx {
                            (
                                recruitable_manpower(&self.world, &self.v6_db, CountryId(i as u16)),
                                0u32,
                                0u32,
                                0u32,
                                self.world
                                    .countries
                                    .stability
                                    .get(i)
                                    .copied()
                                    .unwrap_or(0.5),
                                self.world
                                    .countries
                                    .war_support
                                    .get(i)
                                    .copied()
                                    .unwrap_or(0.5),
                            )
                        } else {
                            (0, 0, 0, 0, 0.5, 0.5)
                        };
                        menu_pass::draw_country_detail(
                            &mut s.text_pass,
                            layout.detail_rect,
                            &entry.name,
                            &entry.tag,
                            &entry.ideology,
                            mp,
                            civ,
                            mil,
                            dock,
                            stab,
                            ws,
                        );
                    }
                    self.view.last_country_layout = Some(layout);
                    self.view.last_main_buttons.clear();
                }
                GamePhase::Playing => unreachable!(),
            }
        } else {
            self.view.last_main_buttons.clear();
            self.view.last_country_layout = None;
        }

        // Playing phase - topbar HUD
        // ????????? Phase 3.5 / 3.10.3: Country name labels ????????????????????????????????????????????????????????????
        // When the 3D atlas baked successfully (`mapname_pass.is_some()`) the
        // 3D pass above has already drawn country names; the 2D HUD path
        // below is only a fallback for systems where no system font could
        // be found.
        if self.view.game_phase == GamePhase::Playing && s.mapname_pass.is_none() {
            // LOD: at low zoom (zoomed out), only show countries with many provinces;
            // at high zoom, show all.
            {
                let view_proj = self.camera.view_proj();
                let sw = s.config.width as f32;
                let sh = s.config.height as f32;
                // zoom_factor 0 = far / 1 = near. Threshold of provinces required:
                let min_provinces = if zoom_factor < 0.20 {
                    40 // very far: only big countries
                } else if zoom_factor < 0.50 {
                    15
                } else {
                    3 // near: most countries
                };
                for (idx, label_opt) in s.country_labels.iter().enumerate() {
                    let Some(label) = label_opt else { continue };
                    if label.province_count < min_provinces {
                        continue;
                    }
                    // Lift label slightly above terrain so it sits ABOVE the surface.
                    // Use a fixed height of HEIGHT_SCALE * 0.5 above heightmap origin ???                // visually appears just above the country's territory.
                    let world_pos =
                        Vec4::new(label.world_x, HEIGHT_SCALE * 0.5, label.world_z, 1.0);
                    let clip = view_proj * world_pos;
                    if clip.w <= 0.01 {
                        continue; // behind camera
                    }
                    let ndc_x = clip.x / clip.w;
                    let ndc_y = clip.y / clip.w;
                    if ndc_x < -1.2 || ndc_x > 1.2 || ndc_y < -1.2 || ndc_y > 1.2 {
                        continue; // off-screen
                    }
                    let sx = (ndc_x + 1.0) * 0.5 * sw;
                    let sy = (1.0 - ndc_y) * 0.5 * sh;
                    // Don't draw over topbar (y < TOPBAR_HEIGHT).
                    if sy < TOPBAR_HEIGHT + 4.0 {
                        continue;
                    }
                    let tag = self
                        .world
                        .countries
                        .tags
                        .get(idx)
                        .cloned()
                        .unwrap_or_default();
                    if tag.is_empty() {
                        continue;
                    }
                    // Use Heading size (28 px) for country names.
                    s.text_pass.draw_text_sized(
                        &tag,
                        sx,
                        sy,
                        TextAlign::Center,
                        160.0,
                        TextSize::Heading,
                    );
                }
            }
        } // end mapname 2D fallback

        // 4.3 Step B (2026-05-18): ?????`politics_pass::draw_politics_overlay`
        // ?????????????????????????????????????????olitical_title / ideology / focus
        // ???????vanilla `countrypoliticsview.gui` ?????textbox widget ???????        // ??????????????`crate::binding::WorldBinding::query_string(widget_name)`
        // V5 ????????1 debug overlay ???????GuiRt-removed widget ??????????????
        // CR-4.6: Off-screen indicators for selected provinces with counters off-screen.
        if self.view.game_phase == GamePhase::Playing
            && s.hoi3_counter_pass.enabled()
            && !self.interaction.selected_province_ids.is_empty()
        {
            let dpi = s.ui_scale_factor();
            let inv_dpi = 1.0 / dpi.max(1.0);
            let lw = s.config.width as f32 * inv_dpi;
            let lh = s.config.height as f32 * inv_dpi;
            let view_proj = self.camera.view_proj();
            let time_secs = self.start_time.elapsed().as_secs_f32();
            for c in self._cached_hoi3_counter_upload.iter() {
                if (c.flags & flag_bits::IS_UNDERLAY) != 0 {
                    continue;
                }
                let pid = c.province_id() as u32;
                if !self.interaction.selected_province_ids.contains(&pid) {
                    continue;
                }
                let Some(screen_pos) = project_counter_screen_pos(
                    c,
                    &view_proj,
                    s.config.width as f32,
                    s.config.height as f32,
                    time_secs,
                ) else {
                    continue;
                };
                let cx = (screen_pos[0] + c.size[0] * 0.5) * inv_dpi;
                let cy = (screen_pos[1] + c.size[1] * 0.5) * inv_dpi;
                if cx >= 0.0 && cx <= lw && cy >= 0.0 && cy <= lh {
                    continue;
                }
                // Off-screen: clamp to edge and draw arrow
                let ex = cx.clamp(16.0, lw - 16.0);
                let ey = cy.clamp(16.0, lh - 16.0);
                let arrow = if cx < 0.0 {
                    "<"
                } else if cx > lw {
                    ">"
                } else if cy < 0.0 {
                    "^"
                } else {
                    "v"
                };
                s.text_pass
                    .draw_text_aligned(arrow, ex, ey, TextAlign::Center, 32.0);
            }
        }

        // Phase 3.12.1: F4 ?????????????????????ass ?????/ HDR ?????/ ????????????
        if s.debug_render_overlay.enabled {
            let mut y = 8.0;
            for line in s.debug_render_overlay.latest_lines() {
                s.text_pass.draw_text(line, 8.0, y);
                y += 16.0;
            }
        }

        if self.view.game_phase == GamePhase::Playing {
            let view_proj = self.camera.view_proj();
            let dpi = s.ui_scale_factor();
            let sw = s.config.width as f32 / dpi.max(0.0001);
            let sh = s.config.height as f32 / dpi.max(0.0001);
            let player_cid = hoi4_state::CountryId(self.view.player_country as u16);
            let centroids = &s.unit_counter_centroids;
            for army in &self.world.player_armies {
                if army.owner != player_cid && !self.ui_state.settings.show_all_units {
                    continue;
                }
                if army.owner != player_cid && self.ui_state.settings.hide_ai_frontlines {
                    continue;
                }
                if let Some(ref order) = army.order {
                    if order.path.is_empty() {
                        continue;
                    }
                    let mut sum_wx = 0.0f32;
                    let mut sum_wz = 0.0f32;
                    let mut count = 0u32;
                    for &pid in &order.path {
                        let (cx, cy) = centroids.get(pid.0 as usize).copied().unwrap_or((0.0, 0.0));
                        if cx == 0.0 && cy == 0.0 {
                            continue;
                        }
                        sum_wx += cx * WORLD_SCALE;
                        sum_wz += cy * WORLD_SCALE;
                        count += 1;
                    }
                    if count == 0 {
                        continue;
                    }
                    let avg_x = sum_wx / count as f32;
                    let avg_z = sum_wz / count as f32;
                    let world_y = 0.3f32;
                    let clip = view_proj * glam::Vec4::new(avg_x, world_y, avg_z, 1.0);
                    if clip.w <= 0.0 {
                        continue;
                    }
                    let ndc_x = clip.x / clip.w;
                    let ndc_y = clip.y / clip.w;
                    let screen_x = (ndc_x * 0.5 + 0.5) * sw;
                    let screen_y = (1.0 - (ndc_y * 0.5 + 0.5)) * sh;
                    if screen_x < -200.0
                        || screen_x > sw + 200.0
                        || screen_y < -50.0
                        || screen_y > sh + 50.0
                    {
                        continue;
                    }
                    let label = format!("{} - {} units", army.name, army.members.len());
                    let label_w = (label.chars().count() as f32 * 9.0 + 16.0).clamp(80.0, 200.0);
                    let label_h = 20.0;
                    let cy = screen_y - 14.0;
                    s.panel_pass.push(panel_pass::Panel {
                        x: screen_x - label_w * 0.5,
                        y: cy - label_h * 0.5,
                        w: label_w,
                        h: label_h,
                        radius: 4.0,
                        fill_top: [0.10, 0.07, 0.04, 0.85],
                        fill_bottom: [0.06, 0.04, 0.02, 0.85],
                        border: [0.79, 0.65, 0.36, 0.7],
                        border_width: 1.0,
                        shadow_offset: 1.0,
                        shadow_alpha: 0.35,
                    });
                    s.text_pass.draw_text_sized(
                        &label,
                        screen_x,
                        cy - 2.0,
                        TextAlign::Center,
                        label_w,
                        TextSize::Body,
                    );
                }
            }
        }

        if self.view.game_phase == GamePhase::Playing && self.interaction.selection_box.active {
            let [x, y, w, h] = self.interaction.selection_box.rect();
            if w > 1.0 && h > 1.0 {
                s.panel_pass.push(panel_pass::Panel {
                    x,
                    y,
                    w,
                    h,
                    radius: 0.0,
                    fill_top: [0.20, 0.45, 0.95, 0.16],
                    fill_bottom: [0.20, 0.45, 0.95, 0.10],
                    border: [0.65, 0.85, 1.0, 0.85],
                    border_width: 1.0,
                    shadow_offset: 0.0,
                    shadow_alpha: 0.0,
                });
            }
        }

        FrameHudOutput {
            quality_label,
            profile_hud_build_ms: started_at.elapsed().as_secs_f32() * 1000.0,
        }
    }
}
