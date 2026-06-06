use crate::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PainterMode {
    Idle,
    ArmyPainter(hoi4_state::ArmyId),
    ArrowPainter(hoi4_state::ArmyId, hoi4_state::ProvinceId),
}

pub(crate) struct FrontlinePainterState {
    pub(crate) mode: PainterMode,
    pub(crate) samples: Vec<hoi4_state::ProvinceId>,
    pub(crate) last_sample_at: std::time::Instant,
}

impl Default for FrontlinePainterState {
    fn default() -> Self {
        Self {
            mode: PainterMode::Idle,
            samples: Vec::new(),
            last_sample_at: std::time::Instant::now(),
        }
    }
}

pub(crate) fn reconstruct_sample_bridge(
    parent: &HashMap<u16, u16>,
    start: u16,
    end: u16,
) -> Vec<hoi4_state::ProvinceId> {
    let mut raw = end;
    let mut path = vec![hoi4_state::ProvinceId(raw)];
    while raw != start {
        let Some(&p) = parent.get(&raw) else {
            break;
        };
        raw = p;
        path.push(hoi4_state::ProvinceId(raw));
    }
    path.reverse();
    path
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SelectionBoxState {
    pub(crate) active: bool,
    pub(crate) start: [f32; 2],
    pub(crate) current: [f32; 2],
    pub(crate) pressed_at: Instant,
}

impl Default for SelectionBoxState {
    fn default() -> Self {
        Self {
            active: false,
            start: [0.0; 2],
            current: [0.0; 2],
            pressed_at: Instant::now(),
        }
    }
}

impl SelectionBoxState {
    pub(crate) fn rect(self) -> [f32; 4] {
        let x0 = self.start[0].min(self.current[0]);
        let y0 = self.start[1].min(self.current[1]);
        let x1 = self.start[0].max(self.current[0]);
        let y1 = self.start[1].max(self.current[1]);
        [x0, y0, x1 - x0, y1 - y0]
    }

    pub(crate) fn large_enough(self) -> bool {
        let r = self.rect();
        r[2] >= UNIT_BOX_SELECT_THRESHOLD_PX && r[3] >= UNIT_BOX_SELECT_THRESHOLD_PX
    }

    pub(crate) fn held_long_enough(self) -> bool {
        self.pressed_at.elapsed().as_millis() >= UNIT_BOX_SELECT_HOLD_MS
    }
}

impl App {
    pub(crate) fn append_frontline_painter_sample(&mut self, prov: hoi4_state::ProvinceId) -> bool {
        let Some(&last) = self.interaction.frontline_painter.samples.last() else {
            self.interaction.frontline_painter.samples.push(prov);
            return true;
        };
        if last == prov {
            return false;
        }

        let bridge = self
            .short_land_sample_bridge(last, prov, 14)
            .unwrap_or_else(|| vec![last, prov]);
        let mut changed = false;
        for pid in bridge.into_iter().skip(1) {
            if self.interaction.frontline_painter.samples.last() == Some(&pid) {
                continue;
            }
            if self.interaction.frontline_painter.samples.len() >= hoi4_logic::military::frontline::MAX_SAMPLES
            {
                self.thin_frontline_painter_samples();
            }
            if self.interaction.frontline_painter.samples.len() < hoi4_logic::military::frontline::MAX_SAMPLES {
                self.interaction.frontline_painter.samples.push(pid);
                changed = true;
            }
        }
        changed
    }

    pub(crate) fn thin_frontline_painter_samples(&mut self) {
        let len = self.interaction.frontline_painter.samples.len();
        if len <= 2 {
            return;
        }
        let mut thinned = Vec::with_capacity(len / 2 + 2);
        for (idx, &pid) in self.interaction.frontline_painter.samples.iter().enumerate() {
            if idx == 0 || idx + 1 == len || idx % 2 == 0 {
                if thinned.last() != Some(&pid) {
                    thinned.push(pid);
                }
            }
        }
        self.interaction.frontline_painter.samples = thinned;
    }

    pub(crate) fn short_land_sample_bridge(
        &self,
        from: hoi4_state::ProvinceId,
        to: hoi4_state::ProvinceId,
        max_depth: u32,
    ) -> Option<Vec<hoi4_state::ProvinceId>> {
        if from == to {
            return Some(vec![from]);
        }
        let raw_from = from.0 as usize;
        let raw_to = to.0 as usize;
        if raw_from >= self.world.map.adjacencies.len()
            || raw_to >= self.world.map.adjacencies.len()
            || !self.is_land_province_raw(from.0)
            || !self.is_land_province_raw(to.0)
        {
            return None;
        }

        let mut parent: HashMap<u16, u16> = HashMap::new();
        let mut visited: HashSet<u16> = HashSet::new();
        let mut queue: VecDeque<(u16, u32)> = VecDeque::new();
        visited.insert(from.0);
        queue.push_back((from.0, 0));

        while let Some((node, depth)) = queue.pop_front() {
            if depth >= max_depth || (node as usize) >= self.world.map.adjacencies.len() {
                continue;
            }
            for &nb in &self.world.map.adjacencies[node as usize] {
                if !self.is_land_province_raw(nb) || !visited.insert(nb) {
                    continue;
                }
                parent.insert(nb, node);
                if nb == to.0 {
                    return Some(reconstruct_sample_bridge(&parent, from.0, to.0));
                }
                queue.push_back((nb, depth + 1));
            }
        }
        None
    }

    pub(crate) fn is_land_province_raw(&self, raw: u16) -> bool {
        self.world
            .map
            .definitions
            .get(raw as usize)
            .and_then(|def| def.as_ref())
            .map(|def| matches!(def.province_type, hoi4_map::ProvinceType::Land))
            .unwrap_or(false)
    }

    /// Pure province pick: returns the province ID under the current cursor,
    /// or `u32::MAX` if nothing is hit. No side effects.
    pub(crate) fn pick_province_at_cursor(&self) -> u32 {
        let (cw, ch) = match &self.state {
            Some(s) => {
                let dpi = s.window.scale_factor() as f32;
                (s.config.width as f32 / dpi, s.config.height as f32 / dpi)
            }
            None => return u32::MAX,
        };
        let cursor_x = self.last_mouse[0];
        let cursor_y = self.last_mouse[1];

        let ndc_x = (cursor_x / cw) * 2.0 - 1.0;
        let ndc_y = 1.0 - (cursor_y / ch) * 2.0;
        let ndc = Vec2::new(ndc_x, ndc_y);
        let hmap = &self.world.map.heightmap;
        let world_size = self.camera.world_size;

        if let Some(world_xz) = self.camera.pick_world_xz_at_height(
            ndc,
            hoi4_map::Heightmap::SEA_LEVEL as f32 / 255.0 * HEIGHT_SCALE,
        ) {
            let u = world_xz.x.rem_euclid(world_size.x) / world_size.x;
            let v = world_xz.y / world_size.y;
            if (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v) {
                let pmap = &self.world.map.province_map;
                let px = ((u * pmap.width as f32) as u32).min(pmap.width - 1);
                let py = ((v * pmap.height as f32) as u32).min(pmap.height - 1);
                let pid = pmap.pixels[(py * pmap.width + px) as usize] as u32;
                if self
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
                    })
                    .unwrap_or(false)
                {
                    return pid;
                }
            }
        }

        let mut ground_y = HEIGHT_SCALE * 0.3;
        let mut world_xz = match self.camera.pick_world_xz_at_height(ndc, ground_y) {
            Some(v) => v,
            None => return u32::MAX,
        };

        for _ in 0..3 {
            let u = world_xz.x.rem_euclid(world_size.x) / world_size.x;
            let v = world_xz.y / world_size.y;
            if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
                break;
            }
            let hx = ((u * hmap.width as f32) as u32).min(hmap.width - 1);
            let hy = ((v * hmap.height as f32) as u32).min(hmap.height - 1);
            let raw_h = hmap.pixels[(hy * hmap.width + hx) as usize];
            ground_y = HEIGHT_SCALE * (raw_h as f32 / 255.0);
            world_xz = match self.camera.pick_world_xz_at_height(ndc, ground_y) {
                Some(v) => v,
                None => return u32::MAX,
            };
        }

        let u = world_xz.x.rem_euclid(world_size.x) / world_size.x;
        let v = world_xz.y / world_size.y;
        if (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v) {
            let pmap = &self.world.map.province_map;
            let px = ((u * pmap.width as f32) as u32).min(pmap.width - 1);
            let py = ((v * pmap.height as f32) as u32).min(pmap.height - 1);
            pmap.pixels[(py * pmap.width + px) as usize] as u32
        } else {
            u32::MAX
        }
    }

    pub(crate) fn pick_counter_province_at_cursor(&mut self) -> Option<u32> {
        self.refresh_counter_hit_regions_for_current_frame();
        let [mx, my] = self.last_mouse;
        let dpi = self
            .state
            .as_ref()
            .map(|s| s.window.scale_factor() as f32)
            .unwrap_or(1.0);
        hit_test(&self._cached_hoi3_hit_regions, mx * dpi, my * dpi).map(|pid| pid as u32)
    }

    pub(crate) fn refresh_counter_hit_regions_for_current_frame(&mut self) {
        let Some(s) = self.state.as_ref() else {
            self._cached_hoi3_hit_regions.clear();
            return;
        };
        let view_proj = self.camera.view_proj();
        let sw = s.config.width as f32;
        let sh = s.config.height as f32;
        let time_secs = self.start_time.elapsed().as_secs_f32();
        self.refresh_cached_hoi3_counter_screen_positions(&view_proj, sw, sh, time_secs);
    }

    pub(crate) fn select_counter_stack_at_province(
        &mut self,
        pid: u32,
        ctrl_held: bool,
        shift_held: bool,
    ) {
        if ctrl_held {
            if self.interaction.selected_province_ids.contains(&pid) {
                self.interaction.selected_province_ids.remove(&pid);
            } else {
                self.interaction.selected_province_ids.insert(pid);
            }
        } else {
            let was_selected = self.interaction.selected_province_ids.contains(&pid);
            self.interaction.selected_province_ids.clear();
            self.interaction.expanded_stacks.clear();
            self.interaction.selected_province_ids.insert(pid);
            if was_selected {
                let has_stack = self._cached_hoi3_counter_upload.iter().any(|c| {
                    c.province_id() as u32 == pid
                        && (c.flags & flag_bits::IS_UNDERLAY) == 0
                        && c.stack_count > 1
                });
                if has_stack {
                    self.interaction.expanded_stacks.insert(pid);
                }
            }
        }

        let player = hoi4_state::CountryId(self.view.player_country as u16);
        let prov = hoi4_state::ProvinceId(pid as u16);
        if ctrl_held {
            for i in 0..self.world.divisions.count {
                if self.world.divisions.owners[i] == player
                    && self.world.divisions.locations[i] == prov
                {
                    if let Some(pos) = self.interaction.selected_divisions.iter().position(|&x| x == i) {
                        self.interaction.selected_divisions.remove(pos);
                    } else {
                        self.interaction.selected_divisions.push(i);
                    }
                }
            }
        } else if shift_held {
            self.interaction.selected_divisions.clear();
            for i in 0..self.world.divisions.count {
                if self.world.divisions.owners[i] == player {
                    self.interaction.selected_divisions.push(i);
                }
            }
        } else {
            self.interaction.selected_divisions.clear();
            for i in 0..self.world.divisions.count {
                if self.world.divisions.owners[i] == player
                    && self.world.divisions.locations[i] == prov
                {
                    self.interaction.selected_divisions.push(i);
                }
            }
        }
        self.interaction.selected_army_id = None;
        if let Some(s) = self.state.as_mut() {
            s.window.request_redraw();
        }
        self.refresh_lut();
    }

    pub(crate) fn select_counter_stacks_in_rect(
        &mut self,
        rect_logical: [f32; 4],
        additive: bool,
    ) -> bool {
        self.refresh_counter_hit_regions_for_current_frame();
        let Some(s) = self.state.as_ref() else {
            return false;
        };
        let dpi = s.window.scale_factor() as f32;
        let rect = [
            rect_logical[0] * dpi,
            rect_logical[1] * dpi,
            rect_logical[2] * dpi,
            rect_logical[3] * dpi,
        ];
        let rx1 = rect[0] + rect[2];
        let ry1 = rect[1] + rect[3];
        let player = hoi4_state::CountryId(self.view.player_country as u16);
        let mut province_hits = HashSet::new();

        for region in &self._cached_hoi3_hit_regions {
            let [x, y, w, h] = region.rect;
            let cx = x + w * 0.5;
            let cy = y + h * 0.5;
            if cx >= rect[0] && cx <= rx1 && cy >= rect[1] && cy <= ry1 {
                province_hits.insert(region.province_id as u32);
            }
        }

        if !additive {
            self.interaction.selected_divisions.clear();
            self.interaction.selected_province_ids.clear();
            self.interaction.expanded_stacks.clear();
        }

        for pid in province_hits {
            let prov = hoi4_state::ProvinceId(pid as u16);
            let mut any_here = false;
            for i in 0..self.world.divisions.count {
                if self.world.divisions.owners[i] == player
                    && self.world.divisions.locations[i] == prov
                    && !self.interaction.selected_divisions.contains(&i)
                {
                    self.interaction.selected_divisions.push(i);
                    any_here = true;
                }
            }
            if any_here {
                self.interaction.selected_province_ids.insert(pid);
            }
        }

        self.interaction.selected_army_id = None;
        self.refresh_lut();
        if let Some(s) = self.state.as_ref() {
            s.window.request_redraw();
        }
        !self.interaction.selected_divisions.is_empty()
    }

    /// Attempt to pick a province under the current cursor position.
    /// On hit, sets `selected_province_id` and triggers a redraw.
    pub(crate) fn try_pick_province(&mut self) {
        // Construction placement mode: clicking a province places the building
        if let Some(ref building_key) = self.ui_state.construction_mode.clone() {
            let pid = self.pick_province_at_cursor();
            if pid != u32::MAX {
                let sid = self.world.provinces.state_of[pid as usize];
                if !sid.is_none() {
                    let si = sid.0 as usize;
                    let player_cid = hoi4_state::CountryId(self.view.player_country as u16);
                    if si < self.world.states.count && self.world.states.owners[si] == player_cid {
                        let used: u8 = self
                            .world
                            .countries
                            .buildings_v6
                            .buildings
                            .iter()
                            .filter(|building| building.state == sid && building.level > 0)
                            .map(|building| building.level)
                            .sum();
                        let max = Self::v6_state_building_capacity(&self.world, sid);
                        if (used as u16) < max {
                            let target_level =
                                Self::v6_next_construction_level(&self.world, building_key, sid);
                            let order = hoi4_logic::economy::BuildOrder::new(building_key, sid)
                                .with_level(target_level);
                            match self.runtime.econ.enqueue_construction_checked(
                                player_cid,
                                order,
                                &self.world,
                                &self.v6_db,
                            ) {
                                Ok(()) => {
                                    self.ui_state.ui_sounds
                                        .play_with_fallback(UiSound::OptionClick, UiSound::Click);
                                }
                                Err(reason) => {
                                    println!(
                                        "[construction] could not queue {} in {}: {:?}",
                                        building_key, self.world.states.names[si], reason
                                    );
                                    self.ui_state.ui_sounds
                                        .play_with_fallback(UiSound::Click, UiSound::Click);
                                }
                            }
                        } else {
                            println!(
                                "[construction] state {} is full ({}/{})",
                                self.world.states.names[si], used, max
                            );
                            self.ui_state.ui_sounds
                                .play_with_fallback(UiSound::Click, UiSound::Click);
                        }
                    }
                }
            }
            if let Some(s) = &self.state {
                s.window.request_redraw();
            }
            return;
        }

        // CR-4.5: If pending_move_command, this click sets destination
        if self.interaction.pending_move_command {
            let dest_pid = self.pick_province_at_cursor();
            if dest_pid != u32::MAX {
                let dest = hoi4_state::ProvinceId(dest_pid as u16);
                self.issue_manual_move_to_selected_divisions(dest);
            }
            self.interaction.pending_move_command = false;
            self.interaction.counter_right_click_province = None;
            if let Some(s) = &self.state {
                s.window.request_redraw();
            }
            return;
        }

        if let Some(fleet_id) = self.interaction.pending_naval_move_fleet {
            let pid = self.pick_province_at_cursor();
            if pid != u32::MAX {
                let selected_sea_region = self
                    .world
                    .map
                    .definitions
                    .get(pid as usize)
                    .and_then(|def| def.as_ref())
                    .filter(|def| def.province_type == hoi4_map::ProvinceType::Sea)
                    .map(|def| def.id as u32);
                if let Some(region) = selected_sea_region {
                    let now = self.world.date.hours_since_epoch().max(0) as u64;
                    let _ = hoi4_logic::naval::movement::order_move_to_region(
                        &mut self.world,
                        hoi4_state::FleetId(fleet_id),
                        region,
                        now,
                    );
                    self.interaction.pending_naval_move_fleet = None;
                }
            }
            if let Some(s) = &self.state {
                s.window.request_redraw();
            }
            return;
        }

        if let Some(wing_id) = self.interaction.pending_air_transfer_wing {
            let pid = self.pick_province_at_cursor();
            if pid != u32::MAX {
                let selected_state = self
                    .world
                    .provinces
                    .state_of
                    .get(pid as usize)
                    .copied()
                    .unwrap_or(hoi4_state::StateId::NONE);
                if !selected_state.is_none() {
                    let now = self.world.date.hours_since_epoch().max(0) as u64;
                    let _ = hoi4_logic::air::operations::order_transfer_to_base(
                        &mut self.world,
                        hoi4_state::AirWingId(wing_id),
                        selected_state,
                        selected_state.0 as u32,
                        now,
                    );
                    self.interaction.pending_air_transfer_wing = None;
                }
            }
            if let Some(s) = &self.state {
                s.window.request_redraw();
            }
            return;
        }

        let new_pid = self.pick_province_at_cursor();

        // CR-4.3: Multi-select with Ctrl
        let ctrl_held = self.keys_held.contains(&KeyCode::ControlLeft)
            || self.keys_held.contains(&KeyCode::ControlRight);

        if ctrl_held {
            if new_pid != u32::MAX {
                if self.interaction.selected_province_ids.contains(&new_pid) {
                    self.interaction.selected_province_ids.remove(&new_pid);
                } else {
                    self.interaction.selected_province_ids.insert(new_pid);
                }
            }
        } else {
            self.interaction.selected_province_ids.clear();
            self.interaction.expanded_stacks.clear();
            if new_pid != u32::MAX {
                self.interaction.selected_province_ids.insert(new_pid);
            }
        }

        self.selected_province_id = new_pid;
        if new_pid != u32::MAX && !ctrl_held {
            let pi = new_pid as usize;
            if pi < self.world.provinces.count {
                let sid = self.world.provinces.state_of[pi];
                let si = sid.0 as usize;
                if si < self.world.states.count {
                    for province in &self.world.states.provinces[si] {
                        self.interaction.selected_province_ids.insert(province.0 as u32);
                    }
                }
            }
        }

        let date_s = self.world.date.to_string();
        let mode_s = self.map_mode.name().to_string();

        if let Some(s) = self.state.as_mut() {
            s.window.request_redraw();
            s.window.set_title(&format!(
                "HOI4 Rust 3D | selected province {} | {} | {} | edge/arrows pan, wheel zoom, M map mode, ESC quit",
                new_pid, date_s, mode_s,
            ));
        }
        self.refresh_lut();

        // Select player's divisions in this province
        let player = hoi4_state::CountryId(self.view.player_country as u16);
        let prov = hoi4_state::ProvinceId(new_pid as u16);
        let shift_held = self.keys_held.contains(&KeyCode::ShiftLeft)
            || self.keys_held.contains(&KeyCode::ShiftRight);

        if shift_held {
            self.interaction.selected_divisions.clear();
            for i in 0..self.world.divisions.count {
                if self.world.divisions.owners[i] == player {
                    self.interaction.selected_divisions.push(i);
                }
            }
        } else if ctrl_held {
            for i in 0..self.world.divisions.count {
                if self.world.divisions.owners[i] == player
                    && self.world.divisions.locations[i] == prov
                {
                    if let Some(pos) = self.interaction.selected_divisions.iter().position(|&x| x == i) {
                        self.interaction.selected_divisions.remove(pos);
                    } else {
                        self.interaction.selected_divisions.push(i);
                    }
                }
            }
        } else {
            self.interaction.selected_divisions.clear();
            for i in 0..self.world.divisions.count {
                if self.world.divisions.owners[i] == player
                    && self.world.divisions.locations[i] == prov
                {
                    self.interaction.selected_divisions.push(i);
                }
            }
        }

        // 11.2: If clicked province is on a player army's frontline, select that army
        let clicked_army = self.world.player_armies.iter().find(|a| {
            a.owner == player
                && a.order
                    .as_ref()
                    .map_or(false, |o| o.active && o.path.contains(&prov))
        });
        if new_pid != u32::MAX {
            self.interaction.selected_army_id = clicked_army.map(|a| a.id);
        }

        // J.2: Populate province info card
        if new_pid != u32::MAX {
            let pi = new_pid as usize;
            let state_id_from_province = if pi < self.world.provinces.count {
                self.world.provinces.state_of[pi]
            } else {
                hoi4_state::StateId::NONE
            };
            let si = state_id_from_province.0 as usize;
            let state_name = if si < self.world.states.count {
                self.state_display_name(si)
            } else {
                hoi4_ui::i18n::tr("unknown").to_owned()
            };
            let owner_id = if si < self.world.states.count {
                self.world.states.owners[si]
            } else {
                hoi4_state::CountryId::NONE
            };
            let state_ctrl_id = if si < self.world.states.count {
                self.world.states.controllers[si]
            } else {
                hoi4_state::CountryId::NONE
            };
            let prov_owner_id = if pi < self.world.provinces.count {
                self.world.provinces.owners[pi]
            } else {
                hoi4_state::CountryId::NONE
            };
            let prov_ctrl_id = if pi < self.world.provinces.count {
                self.world.provinces.controllers[pi]
            } else {
                hoi4_state::CountryId::NONE
            };
            let owner_tag = if (owner_id.0 as usize) < self.world.countries.count {
                self.world.countries.tags[owner_id.0 as usize].clone()
            } else {
                String::new()
            };
            let state_controller_tag = if (state_ctrl_id.0 as usize) < self.world.countries.count {
                self.world.countries.tags[state_ctrl_id.0 as usize].clone()
            } else {
                String::new()
            };
            let province_owner_tag = if (prov_owner_id.0 as usize) < self.world.countries.count {
                self.world.countries.tags[prov_owner_id.0 as usize].clone()
            } else {
                String::new()
            };
            let province_controller_tag = if (prov_ctrl_id.0 as usize) < self.world.countries.count
            {
                self.world.countries.tags[prov_ctrl_id.0 as usize].clone()
            } else {
                String::new()
            };
            let owner_name = self.country_display_name(owner_id);
            let state_controller_name = self.country_display_name(state_ctrl_id);
            let province_owner_name = self.country_display_name(prov_owner_id);
            let province_controller_name = self.country_display_name(prov_ctrl_id);
            // Static metadata from GameData; resource output below uses V6 buildings.
            let (category, vp) = self
                .world
                .data
                .states
                .iter()
                .find(|s| s.provinces.contains(&(new_pid as u16)))
                .map(|s| {
                    (
                        s.category.clone(),
                        s.victory_points
                            .iter()
                            .find(|(pid, _)| *pid == new_pid as u16)
                            .map(|(_, v)| *v)
                            .unwrap_or(0),
                    )
                })
                .unwrap_or_default();
            let vp_key = format!("VICTORY_POINTS_{}", new_pid);
            let vp_loc = self.localize_key(&vp_key);
            let vp_name = (vp_loc != vp_key).then_some(vp_loc.as_str());
            let province_name = self.province_display_name(new_pid, vp_name, Some(&state_name));
            let province_def = self.world.map.get_province(new_pid as u16);
            let province_type = Self::province_type_name(province_def);
            let terrain = province_def
                .map(|def| Self::terrain_display_name(def.terrain.as_str()))
                .unwrap_or_else(|| hoi4_ui::i18n::tr("unknown").to_owned());
            let coastal = province_def.map(|def| def.coastal).unwrap_or(false);
            let supply = self.world.provinces.supply.get(pi).copied().unwrap_or(0.0);
            let mut strategic_nodes = Vec::new();
            if vp > 0 {
                strategic_nodes.push(format!(
                    "{} {}",
                    hoi4_ui::i18n::tr("victory_points_label"),
                    vp
                ));
            }
            if coastal {
                strategic_nodes.push(hoi4_ui::i18n::tr("coastal_province").to_owned());
            }
            if province_def
                .map(|def| def.terrain == "urban")
                .unwrap_or(false)
            {
                strategic_nodes.push("鍩庡競".to_owned());
            }
            let adjacent_enemy = self.world.map.neighbors(new_pid as u16).iter().any(|&adj| {
                let adj_pi = adj as usize;
                adj_pi < self.world.provinces.count
                    && self.world.provinces.controllers[adj_pi] != prov_ctrl_id
                    && !self.world.provinces.controllers[adj_pi].is_none()
            });
            if adjacent_enemy {
                strategic_nodes.push("鎺ユ晫杈瑰".to_owned());
            }
            // Divisions in this province (all countries visible)
            let div_names: Vec<String> = (0..self.world.divisions.count)
                .filter(|&i| self.world.divisions.locations[i] == prov)
                .map(|i| self.world.divisions.names[i].clone())
                .collect();
            self.ui_state.province_info_card.open = true;
            let state_id = hoi4_state::StateId(si as u16);
            let mut v6_outputs: HashMap<String, (u32, f32)> = HashMap::new();
            if si < self.world.states.count {
                for building in self
                    .world
                    .countries
                    .buildings_v6
                    .buildings
                    .iter()
                    .filter(|building| building.state == state_id && building.level > 0)
                {
                    if !matches!(
                        building.kind,
                        hoi4_state::BuildingKind::Resource | hoi4_state::BuildingKind::Agriculture
                    ) {
                        continue;
                    }
                    for pm in hoi4_content::active_pms_for_building(building, &self.v6_db) {
                        let throughput =
                            pm.throughput_modifier.max(0.0) * building.production_rate.max(0.0);
                        for (i, good_id) in pm.output_good_ids.iter().enumerate() {
                            let amount = pm.output_good_amounts.get(i).copied().unwrap_or(0.0)
                                * building.level as f32
                                * throughput;
                            if amount <= 0.0 {
                                continue;
                            }
                            let entry = v6_outputs.entry(good_id.clone()).or_default();
                            entry.0 = entry.0.saturating_add(building.level as u32);
                            entry.1 += amount;
                        }
                    }
                }
            }
            let mut resources: Vec<(String, f32)> = v6_outputs
                .iter()
                .map(|(good_id, (level, _))| {
                    (Self::v6_good_name(&self.v6_db, good_id), *level as f32)
                })
                .collect();
            resources.sort_by(|a, b| a.0.cmp(&b.0));
            let mut resources_output: Vec<(String, f32)> = v6_outputs
                .iter()
                .map(|(good_id, (_, amount))| (Self::v6_good_name(&self.v6_db, good_id), *amount))
                .collect();
            resources_output.sort_by(|a, b| a.0.cmp(&b.0));
            let owner_ci = if owner_id.is_none() {
                usize::MAX
            } else {
                owner_id.0 as usize
            };
            let state_population = if si < self.world.states.count {
                self.state_population(state_id)
            } else {
                0
            };
            let state_buildings = if si < self.world.states.count {
                self.world
                    .countries
                    .buildings_v6
                    .buildings
                    .iter()
                    .filter(|building| building.state == state_id && building.level > 0)
                    .map(|building| {
                        let employment_gap =
                            Self::v6_employment_gap_for_building(&self.v6_db, building);
                        let mut warnings = Vec::new();
                        if employment_gap.iter().any(|&gap| gap > 0) {
                            warnings.push(format!(
                                "{} {}",
                                hoi4_ui::i18n::tr("labor_shortage"),
                                employment_gap.iter().sum::<u32>()
                            ));
                        }
                        hoi4_ui::province_info::StateBuildingInfo {
                            name: Self::v6_building_name(&self.v6_db, &building.building_def_id),
                            level: building.level,
                            employment_rate: building.production_rate,
                            profit_rm_weekly: building.profit_rm * 7.0,
                            warnings,
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let construction_projects = if owner_ci < self.runtime.econ.count {
                self.runtime.econ.construction[owner_ci]
                    .items
                    .iter()
                    .filter(|item| item.target_state == state_id)
                    .map(|item| {
                        let current_level = Self::v6_current_building_level(
                            &self.world,
                            &item.building_key,
                            item.target_state,
                        );
                        let target_level = if item.target_level == 0 {
                            current_level.saturating_add(1)
                        } else {
                            item.target_level
                        };
                        hoi4_ui::province_info::StateConstructionProjectInfo {
                            building_name: Self::v6_building_name(&self.v6_db, &item.building_key),
                            current_level,
                            target_level,
                            progress: item.completion(),
                            estimated_days_remaining: estimate_construction_days_remaining(
                                item.progress,
                                item.cost,
                            ),
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            };
            self.ui_state.province_info_data = hoi4_ui::province_info::ProvinceInfoData {
                province: hoi4_ui::province_info::ProvinceTacticalInfo {
                    province_id: new_pid,
                    province_name,
                    province_type,
                    terrain,
                    coastal,
                    owner_tag: province_owner_tag,
                    owner_name: province_owner_name,
                    controller_tag: province_controller_tag,
                    controller_name: province_controller_name,
                    state_name: state_name.clone(),
                    supply,
                    victory_points: vp,
                    strategic_nodes,
                    divisions: div_names,
                },
                state: hoi4_ui::province_info::StateEconomicInfo {
                    state_id: state_id.0,
                    state_name,
                    owner_tag,
                    owner_name,
                    controller_tag: state_controller_tag,
                    controller_name: state_controller_name,
                    population: state_population,
                    state_category: category,
                    infrastructure: if si < self.world.states.count {
                        self.world.states.infrastructure[si]
                    } else {
                        0
                    },
                    resources,
                    resources_output,
                    buildings: state_buildings,
                    construction_projects,
                    slots_used: if si < self.world.states.count {
                        self.world.state_building_levels(state_id) as u8
                    } else {
                        0
                    },
                    slots_max: if si < self.world.states.count {
                        self.world.states.category_slots[si]
                    } else {
                        0
                    },
                },
            };
            self.ui_state.province_info_card.open = false;
            self.ui_state.active_detail_panel = Some(hoi4_ui::ActiveDetailPanel::Province(
                hoi4_ui::ProvinceDetailTarget {
                    province_id: new_pid,
                },
            ));
        }
    }

    /// Phase 4.3: Handle mouse click within an open in-game panel.
    pub(crate) fn handle_panel_click(&mut self, _mx: f32, _my: f32) -> bool {
        false
    }

    /// Block map clicks whenever an overlay or modal is meant to own input.
    /// Map interaction modes such as construction placement / frontline painting
    /// are exempt so they keep working while the relevant panel is open.
    pub(crate) fn ui_blocks_map_clicks(&self) -> bool {
        let panel_blocks_map = self
            .ui_state
            .open_panel
            .is_some_and(|panel| !matches!(panel, InGamePanel::Air | InGamePanel::Naval));
        self.view.game_phase == GamePhase::Playing
            && self.ui_state.construction_mode.is_none()
            && self.interaction.pending_move_command == false
            && self.interaction.frontline_painter.mode == PainterMode::Idle
            && self.interaction.counter_right_click_province.is_none()
            && (panel_blocks_map
                || self
                    .ui_state
                    .province_info_card
                    .contains_point(self.last_mouse[0], self.last_mouse[1])
                || self.ui_state.country_info_panel.open
                || self.ui_state.settings_panel.open
                || self.ui_state.save_browser.open
                || (self.ui_state.end_screen.triggered && !self.ui_state.end_screen.continued))
    }
}
