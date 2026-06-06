use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::*;
use wgpu::util::DeviceExt;

#[derive(Debug, Clone, Copy)]
pub(crate) struct VisualDivisionMotion {
    pub(crate) from: hoi4_state::ProvinceId,
    pub(crate) to: hoi4_state::ProvinceId,
    pub(crate) elapsed: f32,
    pub(crate) duration: f32,
}

#[derive(Default)]
pub(crate) struct CounterVisibilityCache {
    valid: bool,
    signature: u64,
    visible: Option<HashSet<CountryId>>,
    spotted: Option<HashSet<u16>>,
}

impl App {
    /// 3.12.15 fog-of-war for the on-map counter pass: the player only sees
    /// their own units, faction allies, neighbours, and active war
    /// belligerents. Returns `None` until a country has been selected ???that
    /// disables the filter (observer mode shows everyone, useful for
    /// development and during the main-menu phase).
    fn counter_visibility_signature(&self) -> u64 {
        let mut h = DefaultHasher::new();
        self.game_phase.hash(&mut h);
        self.settings.show_all_units.hash(&mut h);
        self.player_country.hash(&mut h);
        (self.world.elapsed_hours / 24).hash(&mut h);
        self.world.provinces.count.hash(&mut h);
        self.world.divisions.count.hash(&mut h);
        self.world.diplomacy.wars.len().hash(&mut h);
        self.world.diplomacy.factions.len().hash(&mut h);
        h.finish()
    }

    fn cached_counter_visibility(&mut self) -> (Option<HashSet<CountryId>>, Option<HashSet<u16>>) {
        if self.game_phase != GamePhase::Playing
            || self.settings.show_all_units
            || self.player_country >= self.world.countries.count
        {
            self.counter_visibility_cache.valid = false;
            return (None, None);
        }

        let sig = self.counter_visibility_signature();
        if self.counter_visibility_cache.valid && self.counter_visibility_cache.signature == sig {
            return (
                self.counter_visibility_cache.visible.clone(),
                self.counter_visibility_cache.spotted.clone(),
            );
        }

        let player = CountryId(self.player_country as u16);
        let visible = Some(hoi4_render::units::visibility::visible_countries(
            &self.world,
            player,
        ));
        let spotted = Some(hoi4_render::units::visibility::spotted_provinces(
            &self.world,
            player,
        ));
        self.counter_visibility_cache = CounterVisibilityCache {
            valid: true,
            signature: sig,
            visible: visible.clone(),
            spotted: spotted.clone(),
        };
        (visible, spotted)
    }

    pub(crate) fn update_division_motion(&mut self, dt: f32, simulation_advanced: bool) {
        if self.last_division_locations.len() != self.world.divisions.count {
            self.last_division_locations = self.world.divisions.locations.clone();
            self.division_motion.clear();
            return;
        }

        if simulation_advanced {
            let centroids = self
                .state
                .as_ref()
                .map(|state| state.unit_counter_centroids.as_slice());
            for i in 0..self.world.divisions.count {
                let old = self.last_division_locations[i];
                let new = self.world.divisions.locations[i];
                if old == new {
                    continue;
                }
                if self
                    .division_motion
                    .get(&i)
                    .is_some_and(|motion| motion.from == old && motion.to == new)
                {
                    self.last_division_locations[i] = new;
                    continue;
                }

                let should_animate =
                    centroids.is_some() && self.should_animate_division_step(old, new);

                if should_animate {
                    let duration = match self.world.speed {
                        GameSpeed::Paused => 0.5,
                        speed => (speed.seconds_per_hour() * 24.0).clamp(0.08, 5.0),
                    };
                    self.division_motion.insert(
                        i,
                        VisualDivisionMotion {
                            from: old,
                            to: new,
                            elapsed: 0.0,
                            duration,
                        },
                    );
                } else {
                    self.division_motion.remove(&i);
                }
                self.last_division_locations[i] = new;
            }
        }

        if dt > 0.0 {
            self.division_motion.retain(|_, motion| {
                motion.elapsed += dt;
                motion.elapsed < motion.duration
            });
        }
    }

    fn visual_division_motion_overrides(&self) -> HashMap<usize, CounterMotionOverride> {
        let mut overrides = HashMap::new();
        let Some(state) = self.state.as_ref() else {
            return overrides;
        };
        for (&div_idx, motion) in &self.division_motion {
            let Some(&(from_x, from_y)) = state.unit_counter_centroids.get(motion.from.0 as usize)
            else {
                continue;
            };
            let Some(&(to_x, to_y)) = state.unit_counter_centroids.get(motion.to.0 as usize) else {
                continue;
            };
            if (from_x + from_y) == 0.0 || (to_x + to_y) == 0.0 {
                continue;
            }
            let t = (motion.elapsed / motion.duration.max(0.001)).clamp(0.0, 1.0);
            overrides.insert(
                div_idx,
                CounterMotionOverride {
                    current: (from_x + (to_x - from_x) * t, from_y + (to_y - from_y) * t),
                    target: (to_x, to_y),
                    remaining_secs: (motion.duration - motion.elapsed).max(0.0),
                },
            );
        }
        overrides
    }

    /// Generate and upload HOI3-style screen-space counters each frame.
    pub(crate) fn update_hoi3_counter_pass(&mut self, world_objects: WorldObjectPlan) {
        let view_proj = self.camera.view_proj();
        let view_proj_uniform = view_proj.to_cols_array_2d();
        let time_secs = self.start_time.elapsed().as_secs_f32();
        let (sw, sh) = match self.state.as_ref() {
            Some(s) => (s.config.width as f32, s.config.height as f32),
            None => return,
        };

        if !world_objects.counters.visible {
            if let Some(s) = self.state.as_mut() {
                if s.hoi3_counter_pass.instance_count() > 0 {
                    s.hoi3_counter_pass.upload(
                        &s.device,
                        &s.queue,
                        &[],
                        sw,
                        sh,
                        view_proj_uniform,
                        time_secs,
                    );
                    s.hoi3_counter_pass.update_opacity(
                        &s.queue,
                        0.0,
                        sw,
                        sh,
                        view_proj_uniform,
                        time_secs,
                    );
                }
            }
            self.cached_hoi3_counter_sig = 0;
            self.cached_hoi3_counter_layout_sig = 0;
            self.cached_hoi3_counter_layout_offsets.clear();
            self.perf_counter_instances = 0;
            self._cached_hoi3_counter_upload.clear();
            self._cached_hoi3_hit_regions.clear();
            return;
        }

        let sig = self.hoi3_counter_signature(world_objects);
        if sig == self.cached_hoi3_counter_sig {
            self.perf_counter_cache_hits = self.perf_counter_cache_hits.saturating_add(1);
            return;
        }

        self.cached_hoi3_counter_sig = sig;
        self.perf_counter_rebuilds = self.perf_counter_rebuilds.saturating_add(1);

        let (visible, spotted) = self.cached_counter_visibility();
        let player = if self.player_country < self.world.countries.count {
            hoi4_state::CountryId(self.player_country as u16)
        } else {
            hoi4_state::CountryId::NONE
        };
        let visual_motion_overrides = self.visual_division_motion_overrides();
        let unit_counter_centroids = match self.state.as_ref() {
            Some(s) => s.unit_counter_centroids.clone(),
            None => return,
        };
        let mut counter_selected_province_ids = self.selected_province_ids.clone();
        if self.selected_province_id != u32::MAX {
            counter_selected_province_ids.insert(self.selected_province_id);
        }
        let mut counters = generate_hoi3_counters_cr3(
            &self.world,
            &unit_counter_centroids,
            WORLD_SCALE,
            HEIGHT_SCALE,
            &view_proj,
            sw,
            sh,
            &counter_selected_province_ids,
            self.camera.distance,
            visible.as_ref(),
            spotted.as_ref(),
            player,
            Some(&visual_motion_overrides),
            time_secs,
        );
        if world_objects.counter_selected_only {
            counters.retain(|counter| {
                (counter.flags & hoi4_render::counter_v3::flag_bits::SELECTED) != 0
                    || counter_selected_province_ids.contains(&(counter.province_id() as u32))
            });
        }
        let counter_scale = world_objects.counters.scale;
        if (counter_scale - 1.0).abs() > 0.001 {
            for counter in &mut counters {
                let old = counter.size;
                counter.size = [old[0] * counter_scale, old[1] * counter_scale];
                counter.screen_pos[0] -= (counter.size[0] - old[0]) * 0.5;
                counter.screen_pos[1] -= (counter.size[1] - old[1]) * 0.5;
            }
        }

        // CR-4: Build layout counters from top counters (skip underlays), run layout, write back.
        use hoi4_render::counter_v3::flag_bits;
        let top_indices: Vec<usize> = counters
            .iter()
            .enumerate()
            .filter(|(_, c)| (c.flags & flag_bits::IS_UNDERLAY) == 0)
            .map(|(i, _)| i)
            .collect();
        let mut layout_counts: HashMap<u16, u16> = HashMap::new();
        let layout_keys: Vec<(u16, u16)> = top_indices
            .iter()
            .map(|&i| {
                let province_id = counters[i].province_id();
                let occurrence = layout_counts.entry(province_id).or_insert(0);
                let key = (province_id, *occurrence);
                *occurrence = occurrence.saturating_add(1);
                key
            })
            .collect();
        let mut layout_counters: Vec<LayoutCounter> = top_indices
            .iter()
            .map(|&i| {
                let c = &counters[i];
                LayoutCounter {
                    pos: c.screen_pos,
                    size: c.size,
                    anchor: c.screen_pos,
                    province_id: c.province_id(),
                }
            })
            .collect();
        // Close-up counters should be camera-stable: screen-space overlap
        // solving changes offsets as perspective changes, so panning the camera
        // can make only some counters appear to slide. Reserve layout solving
        // for the zoomed-out density-control views.
        let layout_threshold = 38.0 + 12.0 * world_objects.counter_layout_density;
        if self.camera.distance > layout_threshold {
            self.apply_or_rebuild_counter_layout(&mut layout_counters, &layout_keys, sw, sh);
        }
        // Write back adjusted positions
        for (li, &ti) in top_indices.iter().enumerate() {
            let delta = [
                layout_counters[li].pos[0] - counters[ti].screen_pos[0],
                layout_counters[li].pos[1] - counters[ti].screen_pos[1],
            ];
            counters[ti].screen_pos = layout_counters[li].pos;
            // Also shift underlays that precede this top counter
            if ti > 0 {
                let mut j = ti - 1;
                loop {
                    if (counters[j].flags & flag_bits::IS_UNDERLAY) != 0 {
                        counters[j].screen_pos[0] += delta[0];
                        counters[j].screen_pos[1] += delta[1];
                    } else {
                        break;
                    }
                    if j == 0 {
                        break;
                    }
                    j -= 1;
                }
            }
        }
        self._cached_hoi3_hit_regions = build_hit_regions(&layout_counters);

        // CR-4.4: Fan-out expanded stacks
        if !self.expanded_stacks.is_empty() {
            let mut expanded_counters: Vec<Hoi3CounterInstance> = Vec::new();
            for c in counters.iter() {
                let pid = c.province_id() as u32;
                if (c.flags & flag_bits::IS_UNDERLAY) != 0 {
                    // Skip underlays of expanded parents
                    if self.expanded_stacks.contains(&pid) {
                        continue;
                    }
                    expanded_counters.push(*c);
                    continue;
                }
                if self.expanded_stacks.contains(&pid) && c.stack_count > 1 {
                    // Fan out: generate one child per division in this province
                    let child_size = [c.size[0] * 0.85, c.size[1] * 0.85];
                    let player_cid = hoi4_state::CountryId(self.player_country as u16);
                    let prov = hoi4_state::ProvinceId(pid as u16);
                    let mut offset_x = 0.0f32;
                    for di in 0..self.world.divisions.count {
                        if self.world.divisions.locations[di] == prov
                            && self.world.divisions.owners[di] == player_cid
                        {
                            let mut child = *c;
                            child.screen_pos = [c.screen_pos[0] + offset_x, c.screen_pos[1]];
                            child.size = child_size;
                            child.stack_count = 1;
                            child.flags =
                                (child.flags & !flag_bits::IS_UNDERLAY) | flag_bits::EXPANDED_CHILD;
                            expanded_counters.push(child);
                            offset_x += child_size[0] + 4.0;
                        }
                    }
                    if offset_x == 0.0 {
                        // No divisions found, keep original
                        expanded_counters.push(*c);
                    }
                } else {
                    expanded_counters.push(*c);
                }
            }
            counters = expanded_counters;
        }

        self.prepare_hoi3_counter_instances_for_upload(
            &mut counters,
            &view_proj,
            sw,
            sh,
            time_secs,
        );
        let s = match self.state.as_mut() {
            Some(s) => s,
            None => return,
        };
        s.hoi3_counter_pass.upload(
            &s.device,
            &s.queue,
            &counters,
            sw,
            sh,
            view_proj_uniform,
            time_secs,
        );
        s.hoi3_counter_pass.update_opacity(
            &s.queue,
            world_objects.counters.opacity,
            sw,
            sh,
            view_proj_uniform,
            time_secs,
        );
        self.perf_counter_instances = counters.len();
        self._cached_hoi3_counter_upload = counters;
        self.refresh_cached_hoi3_counter_screen_positions(&view_proj, sw, sh, time_secs);
    }

    fn prepare_hoi3_counter_instances_for_upload(
        &self,
        counters: &mut [Hoi3CounterInstance],
        view_proj: &glam::Mat4,
        screen_w: f32,
        screen_h: f32,
        time_secs: f32,
    ) {
        for counter in counters {
            if let Some(anchor) =
                project_counter_anchor_screen(counter, view_proj, screen_w, screen_h, time_secs)
            {
                counter.screen_offset = [
                    counter.screen_pos[0] - anchor[0],
                    counter.screen_pos[1] - anchor[1],
                ];
            } else {
                counter.screen_offset = counter.screen_pos;
            }
        }
    }

    pub(crate) fn refresh_cached_hoi3_counter_screen_positions(
        &mut self,
        view_proj: &glam::Mat4,
        screen_w: f32,
        screen_h: f32,
        time_secs: f32,
    ) {
        if self._cached_hoi3_counter_upload.is_empty() {
            self._cached_hoi3_hit_regions.clear();
            return;
        }
        let mut layout_counters = Vec::new();
        for counter in &mut self._cached_hoi3_counter_upload {
            counter.screen_pos =
                project_counter_screen_pos(counter, view_proj, screen_w, screen_h, time_secs)
                    .unwrap_or([-100000.0, -100000.0]);
            if (counter.flags & flag_bits::IS_UNDERLAY) == 0 {
                layout_counters.push(LayoutCounter {
                    pos: counter.screen_pos,
                    size: counter.size,
                    anchor: counter.screen_pos,
                    province_id: counter.province_id(),
                });
            }
        }
        self._cached_hoi3_hit_regions = build_hit_regions(&layout_counters);
    }

    fn apply_or_rebuild_counter_layout(
        &mut self,
        counters: &mut [LayoutCounter],
        keys: &[(u16, u16)],
        screen_w: f32,
        screen_h: f32,
    ) {
        debug_assert_eq!(counters.len(), keys.len());
        let layout_sig = self.hoi3_counter_layout_signature(counters, keys, screen_w, screen_h);
        if layout_sig == self.cached_hoi3_counter_layout_sig {
            for (counter, key) in counters.iter_mut().zip(keys.iter()) {
                if let Some(offset) = self.cached_hoi3_counter_layout_offsets.get(key) {
                    counter.pos = [counter.anchor[0] + offset[0], counter.anchor[1] + offset[1]];
                }
            }
            return;
        }

        layout_screen_space(counters, screen_w, screen_h);
        self.cached_hoi3_counter_layout_sig = layout_sig;
        self.cached_hoi3_counter_layout_offsets.clear();
        for (counter, key) in counters.iter().zip(keys.iter()) {
            self.cached_hoi3_counter_layout_offsets.insert(
                *key,
                [
                    counter.pos[0] - counter.anchor[0],
                    counter.pos[1] - counter.anchor[1],
                ],
            );
        }
    }

    fn hoi3_counter_layout_signature(
        &self,
        counters: &[LayoutCounter],
        keys: &[(u16, u16)],
        screen_w: f32,
        screen_h: f32,
    ) -> u64 {
        let mut h = DefaultHasher::new();
        self.game_phase.hash(&mut h);
        self.player_country.hash(&mut h);
        self.settings.show_all_units.hash(&mut h);
        ((self.camera.distance * 4.0) as i32).hash(&mut h);
        ((screen_w / 4.0) as i32).hash(&mut h);
        ((screen_h / 4.0) as i32).hash(&mut h);
        self.selected_province_ids.len().hash(&mut h);
        for pid in &self.selected_province_ids {
            pid.hash(&mut h);
        }
        self.hovered_province_id.hash(&mut h);
        self.expanded_stacks.len().hash(&mut h);
        for pid in &self.expanded_stacks {
            pid.hash(&mut h);
        }
        keys.hash(&mut h);
        for counter in counters {
            counter.size[0].to_bits().hash(&mut h);
            counter.size[1].to_bits().hash(&mut h);
        }
        h.finish()
    }

    fn hoi3_counter_signature(&self, world_objects: WorldObjectPlan) -> u64 {
        let mut h = DefaultHasher::new();
        self.game_phase.hash(&mut h);
        self.player_country.hash(&mut h);
        ((self.camera.distance * 2.0) as i32).hash(&mut h);
        if let Some(s) = self.state.as_ref() {
            ((s.config.width as f32 / 4.0) as i32).hash(&mut h);
            ((s.config.height as f32 / 4.0) as i32).hash(&mut h);
        }
        self.selected_province_id.hash(&mut h);
        world_objects.counter_selected_only.hash(&mut h);
        ((world_objects.counters.scale * 100.0) as i32).hash(&mut h);
        ((world_objects.counters.opacity * 100.0) as i32).hash(&mut h);
        ((world_objects.counter_layout_density * 100.0) as i32).hash(&mut h);
        for pid in &self.selected_province_ids {
            pid.hash(&mut h);
        }
        for pid in &self.expanded_stacks {
            pid.hash(&mut h);
        }
        self.world.divisions.count.hash(&mut h);
        self.division_motion.len().hash(&mut h);
        for (div_idx, motion) in &self.division_motion {
            div_idx.hash(&mut h);
            motion.from.hash(&mut h);
            motion.to.hash(&mut h);
            ((motion.duration * 120.0) as i32).hash(&mut h);
        }
        for i in 0..self.world.divisions.count {
            self.world.divisions.locations[i].hash(&mut h);
            self.world.divisions.owners[i].hash(&mut h);
            self.world.divisions.in_combat[i].hash(&mut h);
            ((self.world.divisions.organisation[i] * 10.0) as i32).hash(&mut h);
            ((self.world.divisions.strength[i] * 100.0) as i32).hash(&mut h);
        }
        h.finish()
    }

    fn frontline_arrow_signature(&self) -> u64 {
        let mut h = DefaultHasher::new();
        self.game_phase.hash(&mut h);
        self.player_country.hash(&mut h);
        self.settings.show_all_units.hash(&mut h);
        self.settings.hide_ai_frontlines.hash(&mut h);
        self.frontline_overlay_visible.hash(&mut h);

        match self.frontline_painter.mode {
            PainterMode::Idle => 0u8.hash(&mut h),
            PainterMode::ArmyPainter(id) => {
                1u8.hash(&mut h);
                id.hash(&mut h);
            }
            PainterMode::ArrowPainter(id, anchor) => {
                2u8.hash(&mut h);
                id.hash(&mut h);
                anchor.hash(&mut h);
            }
        }
        self.frontline_painter.samples.hash(&mut h);

        self.world.player_armies.len().hash(&mut h);
        for army in &self.world.player_armies {
            army.id.hash(&mut h);
            army.owner.hash(&mut h);
            army.members.hash(&mut h);
            if let Some(order) = &army.order {
                true.hash(&mut h);
                order.path.hash(&mut h);
                order.anchor.hash(&mut h);
                order.active.hash(&mut h);
                order.executing.hash(&mut h);
                if let Some(arrow) = &order.arrow {
                    true.hash(&mut h);
                    arrow.provinces.hash(&mut h);
                } else {
                    false.hash(&mut h);
                }
            } else {
                false.hash(&mut h);
            }
        }

        self.selected_divisions.hash(&mut h);
        self.world.divisions.count.hash(&mut h);
        for &div_idx in &self.selected_divisions {
            div_idx.hash(&mut h);
            if div_idx < self.world.divisions.count {
                self.world.divisions.owners[div_idx].hash(&mut h);
                self.world.divisions.locations[div_idx].hash(&mut h);
                self.world.divisions.destinations[div_idx].hash(&mut h);
            }
        }

        h.finish()
    }

    fn collect_order_arrows(&self) -> Vec<passes::maparrow::ArrowInstance> {
        let Some(state) = self.state.as_ref() else {
            return Vec::new();
        };
        let painter_frontline_owner = match self.frontline_painter.mode {
            PainterMode::ArmyPainter(army_id) => self
                .world
                .player_armies
                .iter()
                .find(|army| army.id == army_id)
                .map(|army| army.owner),
            _ => None,
        };
        render_collect::collect_order_arrows(
            &self.world,
            self.player_country,
            self.settings.show_all_units,
            self.settings.hide_ai_frontlines,
            self.frontline_overlay_visible,
            &self.selected_divisions,
            &self.frontline_painter.samples,
            painter_frontline_owner,
            &state.unit_counter_centroids,
            &state.province_pixel_bounds,
        )
    }

    pub(crate) fn update_frontline_arrows(&mut self) {
        let force_rebuild =
            self.prev_armies_hash == 0 || self.frontline_painter.mode != PainterMode::Idle;
        if !self.high_speed_visual_rebuild_due(self.last_frontline_arrow_rebuild_at, force_rebuild)
        {
            return;
        }
        let sig = self.frontline_arrow_signature();
        if sig == self.prev_armies_hash {
            return;
        }
        self.prev_armies_hash = sig;
        self.last_frontline_arrow_rebuild_at = Instant::now();

        let arrows = self.collect_order_arrows();
        let s = match self.state.as_mut() {
            Some(s) => s,
            None => return,
        };
        s.maparrow_pass.set_arrows(&s.device, &s.queue, &arrows);
    }

    pub(crate) fn update_trade_routes_overlay(&mut self) {
        let sig = map_trade_routes::signature(&self.world);
        if sig == self.trade_routes_hash {
            return;
        }
        self.trade_routes_hash = sig;

        let centroids = match self.state.as_ref() {
            Some(s) => s.unit_counter_centroids.clone(),
            None => return,
        };
        let routes = passes::traderoute::generate_trade_route_vertices(
            &self.world,
            &centroids,
            WORLD_SCALE,
            HEIGHT_SCALE,
        );
        let s = match self.state.as_mut() {
            Some(s) => s,
            None => return,
        };
        s.traderoute_pass.set_routes(&s.device, &s.queue, &routes);
    }

    fn frontline_overlay_signature(&self) -> u64 {
        let mut h = DefaultHasher::new();
        self.world.diplomacy.wars.len().hash(&mut h);
        for (id, war) in &self.world.diplomacy.wars {
            id.hash(&mut h);
            let mut attackers: Vec<_> = war.attackers.iter().copied().collect();
            let mut defenders: Vec<_> = war.defenders.iter().copied().collect();
            attackers.sort_by_key(|country| country.0);
            defenders.sort_by_key(|country| country.0);
            attackers.hash(&mut h);
            defenders.hash(&mut h);
        }
        self.world.provinces.controllers.hash(&mut h);
        h.finish()
    }

    pub(crate) fn update_frontline_overlay(&mut self) {
        if self
            .state
            .as_ref()
            .is_some_and(|s| !s.pass_registry.is_enabled("3d_frontlines"))
        {
            return;
        }
        let force_rebuild = self.frontline_overlay_hash == 0;
        if !self
            .high_speed_visual_rebuild_due(self.last_frontline_overlay_rebuild_at, force_rebuild)
        {
            return;
        }
        let sig = self.frontline_overlay_signature();
        if sig == self.frontline_overlay_hash {
            return;
        }
        self.frontline_overlay_hash = sig;
        self.last_frontline_overlay_rebuild_at = Instant::now();

        let centroids = match self.state.as_ref() {
            Some(s) => s.unit_counter_centroids.clone(),
            None => return,
        };
        let front_verts =
            generate_frontline_vertices(&self.world, &centroids, WORLD_SCALE, HEIGHT_SCALE);
        let s = match self.state.as_mut() {
            Some(s) => s,
            None => return,
        };
        s.frontlines_vertex_count = front_verts.len() as u32;
        s.frontlines_buffer = s
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("frontline_verts_dynamic"),
                contents: if front_verts.is_empty() {
                    &[0u8; 16]
                } else {
                    bytemuck::cast_slice(&front_verts)
                },
                usage: wgpu::BufferUsages::VERTEX,
            });
    }

    fn high_speed_visual_rebuild_due(&self, last_rebuild_at: Instant, force: bool) -> bool {
        if force {
            return true;
        }
        if !matches!(self.world.speed, GameSpeed::Speed4 | GameSpeed::Speed5) {
            return true;
        }
        last_rebuild_at.elapsed().as_secs_f32() >= FAST_VISUAL_REBUILD_INTERVAL_SECS
    }

    pub(crate) fn effective_render_quality_preset(&self, now: Instant) -> MapQualityPreset {
        let Some(last_interaction_at) = self.last_viewport_interaction_at else {
            return self.map_quality_preset;
        };
        let high_speed_interaction =
            matches!(self.world.speed, GameSpeed::Speed4 | GameSpeed::Speed5)
                && now
                    .saturating_duration_since(last_interaction_at)
                    .as_secs_f32()
                    <= INTERACTIVE_RENDER_QUALITY_HOLD_SECS;
        if high_speed_interaction
            && matches!(
                self.map_quality_preset,
                MapQualityPreset::High | MapQualityPreset::Ultra
            )
        {
            MapQualityPreset::LowEnd
        } else {
            self.map_quality_preset
        }
    }
}
