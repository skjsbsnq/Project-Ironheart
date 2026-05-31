use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use hoi4_logic::naval::regions::SeaRegionMap;
use hoi4_logic::SeaControl;
use hoi4_render::map_mode::{build_color_lut, MapMode};
use hoi4_state::{CountryId, World};

use super::VanillaRuntimeTargetFrameParams;

pub fn generate(world: &World, params: &VanillaRuntimeTargetFrameParams) -> Vec<u8> {
    let province_count = world.map.definitions.len().max(world.provinces.count);
    let mut per_province = vec![[0.0f32; 4]; province_count];

    let map_mode_lut =
        if params.map_mode != MapMode::Terrain && params.map_mode_overlay_opacity > 0.001 {
            Some(build_color_lut(
                world,
                params.map_mode,
                params.player_country,
            ))
        } else {
            None
        };

    for pid in 0..province_count {
        if let Some(lut) = map_mode_lut.as_ref() {
            let o = pid * 4;
            if o + 3 < lut.len() && lut[o + 3] > 0 {
                blend_overlay(
                    &mut per_province[pid],
                    [lut[o], lut[o + 1], lut[o + 2]],
                    (34.0 * params.map_mode_overlay_opacity).clamp(0.0, 96.0) as u8,
                );
            }
        }

        if pid < world.provinces.count {
            let owner = world.provinces.owners[pid];
            let controller = world.provinces.controllers[pid];
            if !owner.is_none() && !controller.is_none() && owner != controller {
                let at_war = world.diplomacy.at_war_with(owner, controller);
                let color = if at_war {
                    [220, 50, 50]
                } else {
                    [130, 130, 140]
                };
                blend_overlay(
                    &mut per_province[pid],
                    color,
                    (72.0 * params.occupation_opacity).clamp(0.0, 128.0) as u8,
                );
            }
        }
    }

    apply_battle_plan_overlays(world, params, &mut per_province);
    apply_naval_dominance_overlays(world, params, &mut per_province);

    for pid in 0..province_count {
        if pid as u32 == params.selected_province_id {
            blend_overlay(
                &mut per_province[pid],
                [255, 226, 92],
                (176.0 * params.selected_opacity).clamp(0.0, 210.0) as u8,
            );
        } else if pid as u32 == params.hovered_province_id {
            blend_overlay(
                &mut per_province[pid],
                [255, 244, 202],
                (108.0 * params.hover_opacity).clamp(0.0, 160.0) as u8,
            );
        }
    }

    expand_to_map_pixels(world, &per_province)
}

pub fn signature(world: &World, params: &VanillaRuntimeTargetFrameParams) -> u64 {
    let mut h = DefaultHasher::new();
    params.selected_province_id.hash(&mut h);
    params.hovered_province_id.hash(&mut h);
    map_mode_code(params.map_mode).hash(&mut h);
    params
        .player_country
        .map(|id| id.raw())
        .unwrap_or(CountryId::NONE.raw())
        .hash(&mut h);
    quantize_opacity(params.occupation_opacity).hash(&mut h);
    quantize_opacity(params.selected_opacity).hash(&mut h);
    quantize_opacity(params.hover_opacity).hash(&mut h);
    quantize_opacity(params.map_mode_overlay_opacity).hash(&mut h);
    quantize_opacity(params.battle_plan_opacity).hash(&mut h);
    quantize_opacity(params.naval_dominance_opacity).hash(&mut h);
    for owner in &world.provinces.owners {
        owner.raw().hash(&mut h);
    }
    for controller in &world.provinces.controllers {
        controller.raw().hash(&mut h);
    }
    if params.battle_plan_opacity > 0.001 {
        for army in &world.player_armies {
            army.owner.raw().hash(&mut h);
            if let Some(player) = params.player_country {
                if army.owner != player {
                    continue;
                }
            }
            if let Some(order) = &army.order {
                order.active.hash(&mut h);
                order.executing.hash(&mut h);
                for province in &order.path {
                    province.raw().hash(&mut h);
                }
                if let Some(arrow) = &order.arrow {
                    for province in &arrow.provinces {
                        province.raw().hash(&mut h);
                    }
                }
            }
        }
    }
    if params.naval_dominance_opacity > 0.001 && params.player_country.is_some() {
        world.fleets.count.hash(&mut h);
        for fi in 0..world.fleets.count {
            world.fleets.owners[fi].raw().hash(&mut h);
            world.fleets.region_id[fi].hash(&mut h);
            world.fleets.target_region_id[fi].hash(&mut h);
            for ship in &world.fleets.ships[fi] {
                ship.raw().hash(&mut h);
            }
        }
        world.ships.count.hash(&mut h);
        for si in 0..world.ships.count {
            world.ships.owners[si].raw().hash(&mut h);
            quantize_4k(world.ships.hp[si]).hash(&mut h);
            quantize_4k(world.ships.max_hp[si]).hash(&mut h);
            world.ships.class_keys[si].hash(&mut h);
        }
    }
    h.finish()
}

fn apply_battle_plan_overlays(
    world: &World,
    params: &VanillaRuntimeTargetFrameParams,
    per_province: &mut [[f32; 4]],
) {
    let opacity = params.battle_plan_opacity.clamp(0.0, 1.0);
    if opacity <= 0.001 {
        return;
    }
    for army in &world.player_armies {
        if let Some(player) = params.player_country {
            if army.owner != player {
                continue;
            }
        }
        let Some(order) = &army.order else {
            continue;
        };
        let path_alpha = if order.active { 72.0 } else { 42.0 };
        for province in &order.path {
            let pid = province.raw() as usize;
            if let Some(dst) = per_province.get_mut(pid) {
                blend_overlay(
                    dst,
                    [82, 174, 255],
                    (path_alpha * opacity).clamp(0.0, 128.0) as u8,
                );
            }
        }
        if let Some(arrow) = &order.arrow {
            let arrow_alpha = if order.executing { 120.0 } else { 88.0 };
            for province in &arrow.provinces {
                let pid = province.raw() as usize;
                if let Some(dst) = per_province.get_mut(pid) {
                    blend_overlay(
                        dst,
                        [255, 184, 62],
                        (arrow_alpha * opacity).clamp(0.0, 160.0) as u8,
                    );
                }
            }
        }
    }
}

fn apply_naval_dominance_overlays(
    world: &World,
    params: &VanillaRuntimeTargetFrameParams,
    per_province: &mut [[f32; 4]],
) {
    let opacity = params.naval_dominance_opacity.clamp(0.0, 1.0);
    let Some(player) = params.player_country else {
        return;
    };
    if opacity <= 0.001 {
        return;
    }

    let regions = SeaRegionMap::from_world(world);
    if regions.regions.is_empty() {
        return;
    }
    let sea_control = SeaControl::recompute(world, &world.data);
    for region in &regions.regions {
        let player_control = sea_control.control(region.id, player);
        let dominant = sea_control.dominant(region.id);
        if player_control <= 0.001 && dominant.is_none() {
            continue;
        }
        let (color, strength) = match dominant {
            Some((country, value)) if country == player => ([55, 154, 255], value),
            Some((_country, value)) => ([222, 69, 74], value),
            None => ([126, 136, 148], player_control),
        };
        let alpha = ((28.0 + 72.0 * strength.clamp(0.0, 1.0)) * opacity).clamp(0.0, 128.0) as u8;
        for province in &region.provinces {
            let pid = province.raw() as usize;
            if let Some(dst) = per_province.get_mut(pid) {
                blend_overlay(dst, color, alpha);
            }
        }
    }
}

fn expand_to_map_pixels(world: &World, per_province: &[[f32; 4]]) -> Vec<u8> {
    let pmap = &world.map.province_map;
    let mut data = vec![0u8; pmap.width as usize * pmap.height as usize * 4];
    for (i, pid) in pmap.pixels.iter().copied().enumerate() {
        let o = i * 4;
        let Some(rgba) = per_province.get(pid as usize) else {
            continue;
        };
        data[o] = (rgba[0].clamp(0.0, 1.0) * 255.0).round() as u8;
        data[o + 1] = (rgba[1].clamp(0.0, 1.0) * 255.0).round() as u8;
        data[o + 2] = (rgba[2].clamp(0.0, 1.0) * 255.0).round() as u8;
        data[o + 3] = (rgba[3].clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    data
}

fn blend_overlay(dst: &mut [f32; 4], color: [u8; 3], alpha: u8) {
    if alpha == 0 {
        return;
    }
    let a = alpha as f32 / 255.0;
    let dst_a = dst[3];
    let out_a = a + dst_a * (1.0 - a);
    if out_a <= f32::EPSILON {
        return;
    }
    for i in 0..3 {
        let src = color[i] as f32 / 255.0;
        dst[i] = (src * a + dst[i] * dst_a * (1.0 - a)) / out_a;
    }
    dst[3] = out_a;
}

fn quantize_opacity(value: f32) -> u16 {
    (value.clamp(0.0, 1.0) * 1024.0).round() as u16
}

fn quantize_4k(value: f32) -> i32 {
    (value.clamp(-1_000_000.0, 1_000_000.0) * 4096.0).round() as i32
}

pub fn map_mode_code(mode: MapMode) -> u8 {
    match mode {
        MapMode::Political => 0,
        MapMode::Terrain => 1,
        MapMode::Manpower => 2,
        MapMode::Factories => 3,
        MapMode::Cores => 4,
        MapMode::Infrastructure => 5,
        MapMode::Ideology => 6,
        MapMode::Supply => 7,
        MapMode::Resistance => 8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_blend_accumulates_alpha() {
        let mut dst = [0.0; 4];
        blend_overlay(&mut dst, [255, 0, 0], 128);
        blend_overlay(&mut dst, [0, 0, 255], 128);
        assert!(dst[3] > 0.70 && dst[3] < 0.76);
        assert!(dst[0] > 0.30);
        assert!(dst[2] > 0.60);
    }

    #[test]
    fn map_mode_codes_are_stable() {
        assert_eq!(map_mode_code(MapMode::Political), 0);
        assert_eq!(map_mode_code(MapMode::Resistance), 8);
    }
}
