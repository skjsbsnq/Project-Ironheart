//! Building icon placement + POI icon generation.
//!
//! Generates icon instances per state for buildings and resources.
//! `generate_buildings` is the original Phase 3.5 path (colored squares).
//! `generate_poi_icons` is the Phase 14 vanilla POI icon path (textured
//! sprites via `mapsymbol.wgsl`).

use hoi4_data::resource::ResourceKind;
use hoi4_state::World;

/// One building icon instance (Phase 3.5 legacy).
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct BuildingInstance {
    pub pos: [f32; 3],
    /// 0 = civilian, 1 = military, 2 = dockyard.
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
    RadarStation = 4,
    SyntheticRefinery = 5,
    FuelSilo = 6,
    NuclearReactor = 7,
    RocketSite = 8,
    Oil = 9,
    Aluminium = 10,
    Rubber = 11,
    Tungsten = 12,
    Steel = 13,
    Chromium = 14,
    Coal = 15,
}

impl PoiIconKind {
    pub const COUNT: u8 = 16;

    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::IndustrialComplex,
            1 => Self::ArmsFactory,
            2 => Self::Dockyard,
            3 => Self::AirBase,
            4 => Self::RadarStation,
            5 => Self::SyntheticRefinery,
            6 => Self::FuelSilo,
            7 => Self::NuclearReactor,
            8 => Self::RocketSite,
            9 => Self::Oil,
            10 => Self::Aluminium,
            11 => Self::Rubber,
            12 => Self::Tungsten,
            13 => Self::Steel,
            14 => Self::Chromium,
            15 => Self::Coal,
            _ => Self::IndustrialComplex,
        }
    }

    pub fn sprite_name(self) -> &'static str {
        match self {
            Self::IndustrialComplex => "GFX_onmap_industrial_complex_icon",
            Self::ArmsFactory => "GFX_onmap_arms_factory_icon",
            Self::Dockyard => "GFX_onmap_dockyard_icon",
            Self::AirBase => "GFX_onmap_air_base_icon",
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
        let has_civilian = world.countries.buildings_v6.buildings.iter().any(|b| {
            b.state == state
                && b.level > 0
                && b.kind != hoi4_state::BuildingKind::Military
                && b.kind != hoi4_state::BuildingKind::MilitaryBase
                && b.building_def_id != "shipyard"
        });
        let has_military = world.countries.buildings_v6.buildings.iter().any(|b| {
            b.state == state && b.level > 0 && b.kind == hoi4_state::BuildingKind::Military
        });
        let has_shipyard = world
            .countries
            .buildings_v6
            .buildings
            .iter()
            .any(|b| b.state == state && b.level > 0 && b.building_def_id == "shipyard");
        if !has_civilian && !has_military && !has_shipyard {
            continue;
        }
        let pos =
            match compute_state_world_pos(world, centroids, state_idx, world_scale, height_scale) {
                Some(p) => p,
                None => continue,
            };

        if has_civilian {
            out.push(BuildingInstance {
                pos: [pos[0] - 0.08, pos[1], pos[2]],
                kind: 0.0,
            });
        }
        if has_military {
            out.push(BuildingInstance {
                pos: [pos[0], pos[1], pos[2]],
                kind: 1.0,
            });
        }
        if has_shipyard {
            out.push(BuildingInstance {
                pos: [pos[0] + 0.08, pos[1], pos[2]],
                kind: 2.0,
            });
        }
    }
    out
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

        let infra = world.states.infrastructure[state_idx];
        let state = hoi4_state::StateId(state_idx as u16);
        let civilian_level: u8 = world
            .countries
            .buildings_v6
            .buildings
            .iter()
            .filter(|b| {
                b.state == state
                    && b.kind != hoi4_state::BuildingKind::Military
                    && b.kind != hoi4_state::BuildingKind::MilitaryBase
                    && b.building_def_id != "shipyard"
            })
            .map(|b| b.level)
            .sum();
        let military_level: u8 = world
            .countries
            .buildings_v6
            .buildings
            .iter()
            .filter(|b| b.state == state && b.kind == hoi4_state::BuildingKind::Military)
            .map(|b| b.level)
            .sum();
        let shipyard_level: u8 = world
            .countries
            .buildings_v6
            .buildings
            .iter()
            .filter(|b| b.state == state && b.building_def_id == "shipyard")
            .map(|b| b.level)
            .sum();

        let mut col = 0i32;
        let offset_step = 0.10;
        let base_x = pos[0];

        if civilian_level > 0 {
            out.push(PoiIconInstance {
                pos: [base_x + col as f32 * offset_step, pos[1], pos[2]],
                kind: PoiIconKind::IndustrialComplex as u8 as f32,
                level: civilian_level as f32,
            });
            col += 1;
        }
        if military_level > 0 {
            out.push(PoiIconInstance {
                pos: [base_x + col as f32 * offset_step, pos[1], pos[2]],
                kind: PoiIconKind::ArmsFactory as u8 as f32,
                level: military_level as f32,
            });
            col += 1;
        }
        if shipyard_level > 0 {
            out.push(PoiIconInstance {
                pos: [base_x + col as f32 * offset_step, pos[1], pos[2]],
                kind: PoiIconKind::Dockyard as u8 as f32,
                level: shipyard_level as f32,
            });
            col += 1;
        }

        if infra > 0 {
            out.push(PoiIconInstance {
                pos: [base_x + col as f32 * offset_step, pos[1], pos[2]],
                kind: PoiIconKind::AirBase as u8 as f32,
                level: infra as f32,
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
        assert_eq!(PoiIconKind::COUNT, 16);
    }
}
