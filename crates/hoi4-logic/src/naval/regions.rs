//! Strategic sea regions and port mapping for the naval MVP.

use std::collections::{HashMap, HashSet, VecDeque};

use hoi4_map::ProvinceType;
use hoi4_state::{ProvinceId, World};

pub const NO_SEA_REGION: u32 = u32::MAX;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeaRegionDef {
    pub id: u32,
    pub name: String,
    pub provinces: Vec<ProvinceId>,
}

#[derive(Debug, Clone, Default)]
pub struct SeaRegionMap {
    pub regions: Vec<SeaRegionDef>,
    pub province_region: HashMap<ProvinceId, u32>,
    pub port_region: HashMap<ProvinceId, u32>,
}

impl SeaRegionMap {
    /// Build deterministic sea regions from contiguous sea provinces. This is a
    /// fallback until vanilla strategic regions are parsed, but it replaces the
    /// old naval-base-province placeholder with stable sea-region ids.
    pub fn from_world(world: &World) -> Self {
        let mut visited: HashSet<u16> = HashSet::new();
        let mut regions = Vec::new();
        let mut province_region = HashMap::new();

        for (pid, def) in world.map.definitions.iter().enumerate() {
            let Some(def) = def else { continue };
            if def.province_type != ProvinceType::Sea || visited.contains(&(pid as u16)) {
                continue;
            }

            let region_id = regions.len() as u32;
            let mut queue = VecDeque::from([pid as u16]);
            let mut provinces = Vec::new();
            visited.insert(pid as u16);

            while let Some(current) = queue.pop_front() {
                provinces.push(ProvinceId(current));
                province_region.insert(ProvinceId(current), region_id);
                for &next in world.map.neighbors(current) {
                    if visited.contains(&next) {
                        continue;
                    }
                    let Some(Some(next_def)) = world.map.definitions.get(next as usize) else {
                        continue;
                    };
                    if next_def.province_type == ProvinceType::Sea {
                        visited.insert(next);
                        queue.push_back(next);
                    }
                }
            }

            regions.push(SeaRegionDef {
                id: region_id,
                name: format!("Sea Region {}", region_id + 1),
                provinces,
            });
        }

        let mut out = Self {
            regions,
            province_region,
            port_region: HashMap::new(),
        };
        out.rebuild_port_regions(world);
        out
    }

    pub fn rebuild_port_regions(&mut self, world: &World) {
        self.port_region.clear();
        for pid in 0..world.map.definitions.len() {
            let Some(Some(def)) = world.map.definitions.get(pid) else {
                continue;
            };
            if def.province_type != ProvinceType::Land || !def.coastal {
                continue;
            }
            if let Some(region) = self.nearest_region_for_port(world, ProvinceId(pid as u16)) {
                self.port_region.insert(ProvinceId(pid as u16), region);
            }
        }
    }

    pub fn region_for_port(&self, port: ProvinceId) -> Option<u32> {
        self.port_region.get(&port).copied()
    }

    pub fn nearest_region_for_port(&self, world: &World, port: ProvinceId) -> Option<u32> {
        if port.is_none() {
            return None;
        }
        for &neighbor in world.map.neighbors(port.0) {
            if let Some(region) = self.province_region.get(&ProvinceId(neighbor)).copied() {
                return Some(region);
            }
        }
        None
    }
}

pub fn assign_fleet_home_port(
    world: &mut World,
    regions: &SeaRegionMap,
    fleet: hoi4_state::FleetId,
    port: ProvinceId,
) -> bool {
    if fleet.is_none() || port.is_none() {
        return false;
    }
    let fi = fleet.0 as usize;
    if fi >= world.fleets.count {
        return false;
    }
    let Some(region) = regions.region_for_port(port) else {
        return false;
    };
    world.fleets.home_port[fi] = port.0;
    world.fleets.region_id[fi] = region;
    world.fleets.repair_state[fi] = hoi4_state::NavalRepairState::InPort;
    true
}
