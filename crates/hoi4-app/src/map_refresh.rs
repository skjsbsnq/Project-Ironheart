use crate::*;

impl App {
    // V5 ???????026-05-18????????? rebuild_topbar_runtime ?????vanilla topbar.gui /
    // countrypoliticsview.gui ??????????????????topbar / ?????????????????????????B??
    pub(crate) fn refresh_lut(&self) {
        let s = match &self.state {
            Some(s) => s,
            None => return,
        };
        let player_cid = self.player_country_id();
        let lut_data = build_color_lut(&self.world, self.map_mode, player_cid);
        let mut padded = lut_data;

        // P1: selecting a province also highlights its whole state. The single
        // clicked province still gets the shader pulse; this LUT tint makes the
        // administrative state boundary readable without adding another pass.
        for pid in self
            .interaction.selected_province_ids
            .iter()
            .chain(self.ui_state.construction_highlight_province_ids.iter())
        {
            let o = *pid as usize * 4;
            if o + 3 < padded.len() {
                padded[o] = padded[o].saturating_add(42);
                padded[o + 1] = padded[o + 1].saturating_add(32);
                padded[o + 2] = padded[o + 2].saturating_sub(18);
            }
        }

        padded.resize((s.lut_width * s.lut_height * 4) as usize, 0);
        upload_lut(&s.queue, &s.lut_texture, &padded, s.lut_width, s.lut_height);
        s.window.request_redraw();
    }

    pub(crate) fn player_country_id(&self) -> Option<hoi4_state::CountryId> {
        if self.view.player_country < self.world.countries.count {
            Some(hoi4_state::CountryId(self.view.player_country as u16))
        } else {
            None
        }
    }

    pub(crate) fn lut_entry_with_highlights(
        &self,
        province_idx: usize,
        player_cid: Option<hoi4_state::CountryId>,
    ) -> [u8; 4] {
        let mut entry = color_lut_entry(&self.world, self.map_mode, player_cid, province_idx);
        let pid = province_idx as u32;
        if self.interaction.selected_province_ids.contains(&pid)
            || self.ui_state.construction_highlight_province_ids.contains(&pid)
        {
            entry[0] = entry[0].saturating_add(42);
            entry[1] = entry[1].saturating_add(32);
            entry[2] = entry[2].saturating_sub(18);
        }
        entry
    }

    pub(crate) fn controller_changes_affect_lut(mode: MapMode) -> bool {
        matches!(
            mode,
            MapMode::Political | MapMode::Cores | MapMode::Ideology
        )
    }

    pub(crate) fn refresh_lut_entries(&self, province_indices: &[usize]) {
        if province_indices.is_empty() || !Self::controller_changes_affect_lut(self.map_mode) {
            return;
        }
        let s = match &self.state {
            Some(s) => s,
            None => return,
        };
        if s.lut_width == 0 || s.lut_height == 0 {
            return;
        }

        let max_entries = (s.lut_width * s.lut_height) as usize;
        let mut indices: Vec<usize> = province_indices
            .iter()
            .copied()
            .filter(|pid| *pid < max_entries)
            .collect();
        if indices.is_empty() {
            return;
        }
        indices.sort_unstable();
        indices.dedup();

        let player_cid = self.player_country_id();
        let lut_width = s.lut_width as usize;
        let mut run_start = 0usize;
        let mut run_row = 0usize;
        let mut prev_pid = 0usize;
        let mut run_data: Vec<u8> = Vec::with_capacity(64);
        let mut wrote_any = false;

        for pid in indices {
            let row = pid / lut_width;
            let contiguous = !run_data.is_empty() && row == run_row && pid == prev_pid + 1;
            if !contiguous {
                if !run_data.is_empty() {
                    upload_lut_span(&s.queue, &s.lut_texture, &run_data, s.lut_width, run_start);
                    wrote_any = true;
                    run_data.clear();
                }
                run_start = pid;
                run_row = row;
            }

            run_data.extend_from_slice(&self.lut_entry_with_highlights(pid, player_cid));
            prev_pid = pid;
        }

        if !run_data.is_empty() {
            upload_lut_span(&s.queue, &s.lut_texture, &run_data, s.lut_width, run_start);
            wrote_any = true;
        }

        if wrote_any {
            s.window.request_redraw();
        }
    }

    pub(crate) fn rebuild_country_labels_and_refresh(&mut self) {
        let s = match &mut self.state {
            Some(s) => s,
            None => {
                println!("[map] rebuild_country_labels_and_refresh: state=None, SKIP");
                return;
            }
        };
        let centroids = &s.unit_counter_centroids;
        let country_count = self.world.countries.count;
        let owners_for_labels: Vec<Option<usize>> = self
            .world
            .provinces
            .owners
            .iter()
            .map(|oid| {
                if oid.is_none() {
                    None
                } else {
                    Some(oid.0 as usize)
                }
            })
            .collect();
        s.country_labels = hoi4_render::mapname::compute_country_labels(
            centroids,
            &owners_for_labels,
            country_count,
            WORLD_SCALE,
        );

        // Rebuild 3D country label instances from current owners.
        if let (Some(mapname_pass), Some(atlas)) =
            (s.mapname_pass.as_mut(), s.mapname_atlas.as_ref())
        {
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
                    si < self.world.states.cores.len()
                        && self.world.states.cores[si].contains(&owner)
                })
                .collect();
            let new_obbs = hoi4_render::mapname_3d::compute_country_obbs(
                &self.world.map.province_map,
                &owners_for_labels,
                &province_is_core,
                country_count,
            );
            mapname_pass.rebuild_instances(
                &s.device,
                &s.queue,
                &new_obbs,
                atlas,
                WORLD_SCALE,
                HEIGHT_SCALE * 0.5,
            );
        }
        // Now refresh the LUT
        let player_cid = if self.view.player_country < self.world.countries.count {
            Some(hoi4_state::CountryId(self.view.player_country as u16))
        } else {
            None
        };
        let lut_data = build_color_lut(&self.world, self.map_mode, player_cid);
        let mut padded = lut_data;
        for pid in self
            .interaction.selected_province_ids
            .iter()
            .chain(self.ui_state.construction_highlight_province_ids.iter())
        {
            let o = *pid as usize * 4;
            if o + 3 < padded.len() {
                padded[o] = padded[o].saturating_add(42);
                padded[o + 1] = padded[o + 1].saturating_add(32);
                padded[o + 2] = padded[o + 2].saturating_sub(18);
            }
        }
        padded.resize((s.lut_width * s.lut_height * 4) as usize, 0);
        upload_lut(&s.queue, &s.lut_texture, &padded, s.lut_width, s.lut_height);

        s.window.request_redraw();
    }

    pub(crate) fn refresh_map_if_province_ownership_changed(&mut self) {
        let owners_changed = self.map_refresh_owners != self.world.provinces.owners;
        let controllers_changed = self.map_refresh_controllers != self.world.provinces.controllers;
        if !owners_changed && !controllers_changed {
            return;
        }

        let changed_controller_provinces: Vec<usize> = if !owners_changed && controllers_changed {
            self.map_refresh_controllers
                .iter()
                .zip(self.world.provinces.controllers.iter())
                .enumerate()
                .filter_map(|(pid, (old, new))| (old != new).then_some(pid))
                .collect()
        } else {
            Vec::new()
        };

        self.map_refresh_owners
            .clone_from(&self.world.provinces.owners);
        self.map_refresh_controllers
            .clone_from(&self.world.provinces.controllers);

        if owners_changed {
            self.rebuild_country_labels_and_refresh();
        } else if !changed_controller_provinces.is_empty() {
            self.refresh_lut_entries(&changed_controller_provinces);
        }
    }
}

pub(crate) fn upload_lut(queue: &wgpu::Queue, tex: &wgpu::Texture, data: &[u8], w: u32, h: u32) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        data,
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
}

pub(crate) fn upload_lut_span(
    queue: &wgpu::Queue,
    tex: &wgpu::Texture,
    data: &[u8],
    lut_width: u32,
    start_idx: usize,
) {
    if data.is_empty() || lut_width == 0 {
        return;
    }
    let width = (data.len() / 4) as u32;
    if width == 0 {
        return;
    }
    let x = (start_idx as u32) % lut_width;
    let y = (start_idx as u32) / lut_width;
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: tex,
            mip_level: 0,
            origin: wgpu::Origin3d { x, y, z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(1),
        },
        wgpu::Extent3d {
            width,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
}
