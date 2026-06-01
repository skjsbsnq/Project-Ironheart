//! Building icon placement + POI icon generation.
//!
//! Generates icon instances per state for buildings and resources.
//! `generate_buildings` is the original Phase 3.5 path (colored squares).
//! `generate_poi_icons` is the Phase 14 vanilla POI icon path (textured
//! sprites via `mapsymbol.wgsl`).

use hoi4_data::resource::ResourceKind;
use hoi4_state::World;

pub const BUILDING_KIND_INDUSTRIAL: u8 = 0;
pub const BUILDING_KIND_MILITARY: u8 = 1;
pub const BUILDING_KIND_DOCKYARD: u8 = 2;
pub const BUILDING_KIND_AIR_BASE: u8 = 3;
pub const BUILDING_KIND_NAVAL_BASE: u8 = 4;
pub const BUILDING_KIND_RADAR: u8 = 5;
pub const BUILDING_KIND_ANTI_AIR: u8 = 6;
pub const BUILDING_KIND_BUNKER: u8 = 7;
pub const BUILDING_KIND_COASTAL_BUNKER: u8 = 8;
pub const BUILDING_KIND_REFINERY: u8 = 9;
pub const BUILDING_KIND_FUEL_SILO: u8 = 10;
pub const BUILDING_KIND_NUCLEAR_REACTOR: u8 = 11;
pub const BUILDING_KIND_ROCKET_SITE: u8 = 12;

/// One building icon instance (Phase 3.5 legacy).
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct BuildingInstance {
    pub pos: [f32; 3],
    /// Encoded `BUILDING_KIND_*` value.
    pub kind: f32,
}

/// POI icon category — determines which `GFX_*` sprite to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PoiIconKind {
    IndustrialComplex = 0,
    ArmsFactory = 1,
    Dockyard = 2,
    AirBase = 3,
    NavalBase = 4,
    Bunker = 5,
    CoastalBunker = 6,
    AntiAir = 7,
    RadarStation = 8,
    SyntheticRefinery = 9,
    FuelSilo = 10,
    NuclearReactor = 11,
    RocketSite = 12,
    Oil = 13,
    Aluminium = 14,
    Rubber = 15,
    Tungsten = 16,
    Steel = 17,
    Chromium = 18,
    Coal = 19,
}

impl PoiIconKind {
    pub const COUNT: u8 = 20;

    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::IndustrialComplex,
            1 => Self::ArmsFactory,
            2 => Self::Dockyard,
            3 => Self::AirBase,
            4 => Self::NavalBase,
            5 => Self::Bunker,
            6 => Self::CoastalBunker,
            7 => Self::AntiAir,
            8 => Self::RadarStation,
            9 => Self::SyntheticRefinery,
            10 => Self::FuelSilo,
            11 => Self::NuclearReactor,
            12 => Self::RocketSite,
            13 => Self::Oil,
            14 => Self::Aluminium,
            15 => Self::Rubber,
            16 => Self::Tungsten,
            17 => Self::Steel,
            18 => Self::Chromium,
            19 => Self::Coal,
            _ => Self::IndustrialComplex,
        }
    }

    pub fn sprite_name(self) -> &'static str {
        match self {
            Self::IndustrialComplex => "GFX_onmap_industrial_complex_icon",
            Self::ArmsFactory => "GFX_onmap_arms_factory_icon",
            Self::Dockyard => "GFX_onmap_dockyard_icon",
            Self::AirBase => "GFX_onmap_air_base_icon",
            Self::NavalBase => "GFX_onmap_naval_base_icon",
            Self::Bunker => "GFX_onmap_bunker_icon",
            Self::CoastalBunker => "GFX_onmap_coastal_bunker_icon",
            Self::AntiAir => "GFX_onmap_anti_air_icon",
            Self::RadarStation => "GFX_onmap_radar_station_icon",
            Self::SyntheticRefinery => "GFX_onmap_synthetic_refinery_icon",
            Self::FuelSilo => "GFX_onmap_fuel_silo_icon",
            Self::NuclearReactor => "GFX_onmap_nuclear_reactor_icon",
            Self::RocketSite => "GFX_onmap_rocket_site_icon",
            Self::Oil => "GFX_onmap_resource_oil_icon",
            Self::Aluminium => "GFX_onmap_resource_aluminium_icon",
            Self::Rubber => "GFX_onmap_resource_rubber_icon",
            Self::Tungsten => "GFX_onmap_resource_tungsten_icon",
            Self::Steel => "GFX_onmap_resource_steel_icon",
            Self::Chromium => "GFX_onmap_resource_chromium_icon",
            Self::Coal => "GFX_onmap_resource_coal_icon",
        }
    }

    pub fn all() -> &'static [PoiIconKind] {
        &[
            Self::IndustrialComplex,
            Self::ArmsFactory,
            Self::Dockyard,
            Self::AirBase,
            Self::NavalBase,
            Self::Bunker,
            Self::CoastalBunker,
            Self::AntiAir,
            Self::RadarStation,
            Self::SyntheticRefinery,
            Self::FuelSilo,
            Self::NuclearReactor,
            Self::RocketSite,
            Self::Oil,
            Self::Aluminium,
            Self::Rubber,
            Self::Tungsten,
            Self::Steel,
            Self::Chromium,
            Self::Coal,
        ]
    }
}

impl From<ResourceKind> for PoiIconKind {
    fn from(rk: ResourceKind) -> Self {
        match rk {
            ResourceKind::Oil => Self::Oil,
            ResourceKind::Aluminium => Self::Aluminium,
            ResourceKind::Rubber => Self::Rubber,
            ResourceKind::Tungsten => Self::Tungsten,
            ResourceKind::Steel => Self::Steel,
            ResourceKind::Chromium => Self::Chromium,
            ResourceKind::Coal => Self::Coal,
        }
    }
}

/// One POI icon instance for the vanilla-style 3D world-space pass.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct PoiIconInstance {
    pub pos: [f32; 3],
    pub kind: f32,
    pub level: f32,
}

fn compute_state_world_pos(
    world: &World,
    centroids: &[(f32, f32)],
    state_idx: usize,
    world_scale: f32,
    height_scale: f32,
) -> Option<[f32; 3]> {
    let pid = world.states.provinces.get(state_idx)?.first()?.0 as usize;
    if pid >= centroids.len() {
        return None;
    }
    let (px, py) = centroids[pid];
    if px == 0.0 && py == 0.0 {
        return None;
    }
    let heightmap = &world.map.heightmap;
    let w = heightmap.width;
    let xi = (px as u32).min(w.saturating_sub(1));
    let yi = (py as u32).min(heightmap.height.saturating_sub(1));
    let raw_h = heightmap.pixels[(yi * w + xi) as usize];
    if raw_h <= 95 {
        return None;
    }
    let world_y = (raw_h as f32 / 255.0) * height_scale + 0.15;
    Some([px * world_scale, world_y, py * world_scale])
}

/// Generate building icon instances from world state (Phase 3.5 legacy).
pub fn generate_buildings(
    world: &World,
    centroids: &[(f32, f32)],
    world_scale: f32,
    height_scale: f32,
) -> Vec<BuildingInstance> {
    let mut out = Vec::new();

    for state_idx in 0..world.states.count {
        let state = hoi4_state::StateId(state_idx as u16);
        let mut levels = [0u16; (BUILDING_KIND_ROCKET_SITE as usize) + 1];
        for building in &world.countries.buildings_v6.buildings {
            if building.state != state || building.level == 0 {
                continue;
            }
            if let Some(kind) =
                object_kind_for_building(building.building_def_id.as_str(), building.kind)
            {
                levels[kind as usize] = levels[kind as usize].saturating_add(building.level as u16);
            }
        }
        if levels.iter().all(|&level| level == 0) {
            continue;
        }
        let pos =
            match compute_state_world_pos(world, centroids, state_idx, world_scale, height_scale) {
                Some(p) => p,
                None => continue,
            };

        let mut col = 0i32;
        let offset_step = 0.075;
        for (kind, level) in levels.iter().copied().enumerate() {
            if level == 0 {
                continue;
            }
            for _ in 0..visual_instance_count(level) {
                let x_offset = (col - 3) as f32 * offset_step;
                let z_offset = ((col % 3) - 1) as f32 * offset_step * 0.65;
                out.push(BuildingInstance {
                    pos: [pos[0] + x_offset, pos[1], pos[2] + z_offset],
                    kind: kind as f32,
                });
                col += 1;
            }
        }
    }
    out
}

fn visual_instance_count(level: u16) -> u16 {
    match level {
        0 => 0,
        1..=3 => 1,
        4..=8 => 2,
        _ => 3,
    }
}

fn object_kind_for_building(def_id: &str, kind: hoi4_state::BuildingKind) -> Option<u8> {
    match def_id {
        "shipyard" | "dockyard" => Some(BUILDING_KIND_DOCKYARD),
        "air_base" | "airbase" | "v6_air_base" | "airport" => Some(BUILDING_KIND_AIR_BASE),
        "naval_base" | "v6_naval_base" | "port" => Some(BUILDING_KIND_NAVAL_BASE),
        "radar_station" | "v6_radar" => Some(BUILDING_KIND_RADAR),
        "anti_air_building" | "v6_anti_air" => Some(BUILDING_KIND_ANTI_AIR),
        "bunker" | "v6_bunker" => Some(BUILDING_KIND_BUNKER),
        "coastal_bunker" => Some(BUILDING_KIND_COASTAL_BUNKER),
        "synthetic_refinery" | "oil_refinery" | "rubber_factory" => Some(BUILDING_KIND_REFINERY),
        "fuel_silo" => Some(BUILDING_KIND_FUEL_SILO),
        "nuclear_reactor" | "commercial_nuclear_reactor" => Some(BUILDING_KIND_NUCLEAR_REACTOR),
        "rocket_site" => Some(BUILDING_KIND_ROCKET_SITE),
        _ if kind == hoi4_state::BuildingKind::Military => Some(BUILDING_KIND_MILITARY),
        _ if kind != hoi4_state::BuildingKind::MilitaryBase => Some(BUILDING_KIND_INDUSTRIAL),
        _ => None,
    }
}

/// Generate POI icon instances from world state (Phase 14 — vanilla POI icons).
///
/// Emits one instance per building type and resource type present in each state,
/// with slight X-offsets per kind so icons don't stack on top of each other.
/// `level` is the building count (1–10+) or resource amount.
pub fn generate_poi_icons(
    world: &World,
    centroids: &[(f32, f32)],
    world_scale: f32,
    height_scale: f32,
) -> Vec<PoiIconInstance> {
    let mut out = Vec::new();

    for state_idx in 0..world.states.count {
        let pos =
            match compute_state_world_pos(world, centroids, state_idx, world_scale, height_scale) {
                Some(p) => p,
                None => continue,
            };

        let state = hoi4_state::StateId(state_idx as u16);
        let mut levels = [0.0_f32; PoiIconKind::COUNT as usize];
        for building in &world.countries.buildings_v6.buildings {
            if building.state != state || building.level == 0 {
                continue;
            }
            let Some(kind) =
                poi_kind_for_building(building.building_def_id.as_str(), building.kind)
            else {
                continue;
            };
            levels[kind as usize] += building.level as f32;
        }

        let mut col = 0i32;
        let offset_step = 0.10;
        let base_x = pos[0];

        for kind in PoiIconKind::all()
            .iter()
            .copied()
            .filter(|kind| !poi_kind_is_resource(*kind))
        {
            let level = levels[kind as usize];
            if level <= 0.0 {
                continue;
            }
            out.push(PoiIconInstance {
                pos: [base_x + col as f32 * offset_step, pos[1], pos[2]],
                kind: kind as u8 as f32,
                level,
            });
            col += 1;
        }

        if let Some(state_data) = world.data.states.get(state_idx) {
            for (rk, amount) in &state_data.resources {
                if *amount > 0.0 {
                    let kind: PoiIconKind = (*rk).into();
                    out.push(PoiIconInstance {
                        pos: [base_x + col as f32 * offset_step, pos[1], pos[2]],
                        kind: kind as u8 as f32,
                        level: *amount,
                    });
                    col += 1;
                }
            }
        }
    }
    out
}

fn poi_kind_for_building(def_id: &str, kind: hoi4_state::BuildingKind) -> Option<PoiIconKind> {
    match object_kind_for_building(def_id, kind)? {
        BUILDING_KIND_INDUSTRIAL => Some(PoiIconKind::IndustrialComplex),
        BUILDING_KIND_MILITARY => Some(PoiIconKind::ArmsFactory),
        BUILDING_KIND_DOCKYARD => Some(PoiIconKind::Dockyard),
        BUILDING_KIND_AIR_BASE => Some(PoiIconKind::AirBase),
        BUILDING_KIND_NAVAL_BASE => Some(PoiIconKind::NavalBase),
        BUILDING_KIND_RADAR => Some(PoiIconKind::RadarStation),
        BUILDING_KIND_ANTI_AIR => Some(PoiIconKind::AntiAir),
        BUILDING_KIND_BUNKER => Some(PoiIconKind::Bunker),
        BUILDING_KIND_COASTAL_BUNKER => Some(PoiIconKind::CoastalBunker),
        BUILDING_KIND_REFINERY => Some(PoiIconKind::SyntheticRefinery),
        BUILDING_KIND_FUEL_SILO => Some(PoiIconKind::FuelSilo),
        BUILDING_KIND_NUCLEAR_REACTOR => Some(PoiIconKind::NuclearReactor),
        BUILDING_KIND_ROCKET_SITE => Some(PoiIconKind::RocketSite),
        _ => None,
    }
}

fn poi_kind_is_resource(kind: PoiIconKind) -> bool {
    (kind as u8) >= PoiIconKind::Oil as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_size() {
        assert_eq!(std::mem::size_of::<BuildingInstance>(), 16);
    }

    #[test]
    fn poi_instance_size() {
        assert_eq!(std::mem::size_of::<PoiIconInstance>(), 20);
    }

    #[test]
    fn poi_icon_kind_count() {
        assert_eq!(PoiIconKind::COUNT, 20);
        assert_eq!(PoiIconKind::all().len(), PoiIconKind::COUNT as usize);
    }

    #[test]
    fn object_kind_maps_vanilla_and_v6_ids() {
        assert_eq!(
            object_kind_for_building("v6_air_base", hoi4_state::BuildingKind::MilitaryBase),
            Some(BUILDING_KIND_AIR_BASE)
        );
        assert_eq!(
            object_kind_for_building("port", hoi4_state::BuildingKind::Infrastructure),
            Some(BUILDING_KIND_NAVAL_BASE)
        );
        assert_eq!(
            object_kind_for_building("v6_radar", hoi4_state::BuildingKind::MilitaryBase),
            Some(BUILDING_KIND_RADAR)
        );
        assert_eq!(
            object_kind_for_building("v6_bunker", hoi4_state::BuildingKind::MilitaryBase),
            Some(BUILDING_KIND_BUNKER)
        );
        assert_eq!(
            poi_kind_for_building("port", hoi4_state::BuildingKind::Infrastructure),
            Some(PoiIconKind::NavalBase)
        );
        assert_eq!(
            poi_kind_for_building("coastal_bunker", hoi4_state::BuildingKind::MilitaryBase),
            Some(PoiIconKind::CoastalBunker)
        );
        assert_eq!(
            object_kind_for_building("steel_mill", hoi4_state::BuildingKind::Industrial),
            Some(BUILDING_KIND_INDUSTRIAL)
        );
        assert_eq!(
            object_kind_for_building("arms_industry", hoi4_state::BuildingKind::Military),
            Some(BUILDING_KIND_MILITARY)
        );
    }

    #[test]
    fn visual_instance_count_caps_dense_states() {
        assert_eq!(visual_instance_count(0), 0);
        assert_eq!(visual_instance_count(1), 1);
        assert_eq!(visual_instance_count(4), 2);
        assert_eq!(visual_instance_count(12), 3);
    }
}
