//! 地图模式 — 决定如何给省份上色。

use hoi4_map::definition::ProvinceType;
use hoi4_state::{CountryId, World};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapMode {
    /// 政治模式 — 国家颜色
    Political,
    /// 地形模式 — 按地形类型上色
    Terrain,
    /// 人力模式 — 州人力多少（红色梯度）
    Manpower,
    /// 工厂模式 — 民用+军用+船坞总数
    Factories,
    /// 核心州 — 显示当前选中国家的核心
    Cores,
    /// 基建等级 — 绿色梯度
    Infrastructure,
    /// 意识形态 — 按执政党意识形态上色
    Ideology,
    /// 补给 — 蓝色梯度（深蓝=高补给）
    Supply,
    /// 抵抗 — 红绿梯度（红=高抵抗，绿=高顺从）
    Resistance,
}

impl MapMode {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Political => "Political",
            Self::Terrain => "Terrain",
            Self::Manpower => "Manpower",
            Self::Factories => "Factories",
            Self::Cores => "Cores",
            Self::Infrastructure => "Infrastructure",
            Self::Ideology => "Ideology",
            Self::Supply => "Supply",
            Self::Resistance => "Resistance",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Political => Self::Terrain,
            Self::Terrain => Self::Manpower,
            Self::Manpower => Self::Factories,
            Self::Factories => Self::Infrastructure,
            Self::Infrastructure => Self::Cores,
            Self::Cores => Self::Ideology,
            Self::Ideology => Self::Supply,
            Self::Supply => Self::Resistance,
            Self::Resistance => Self::Political,
        }
    }
}

/// 构建颜色查找表 (Vec<u8> RGBA, 长度 = max_province_id * 4)
pub fn build_color_lut(
    world: &World,
    mode: MapMode,
    player_country: Option<hoi4_state::CountryId>,
) -> Vec<u8> {
    let count = world.map.definitions.len();
    let mut lut = vec![0u8; count * 4];

    match mode {
        MapMode::Political => fill_political(world, &mut lut),
        MapMode::Terrain => fill_terrain(world, &mut lut),
        MapMode::Manpower => fill_manpower(world, &mut lut),
        MapMode::Factories => fill_factories(world, &mut lut),
        MapMode::Infrastructure => fill_infrastructure(world, &mut lut),
        MapMode::Cores => fill_cores(world, &mut lut, player_country),
        MapMode::Ideology => fill_ideology(world, &mut lut),
        MapMode::Supply => fill_supply(world, &mut lut),
        MapMode::Resistance => fill_resistance(world, &mut lut),
    }

    fill_default_water(world, &mut lut);
    lut
}

pub fn color_lut_entry(
    world: &World,
    mode: MapMode,
    player_country: Option<hoi4_state::CountryId>,
    province_idx: usize,
) -> [u8; 4] {
    let mut entry = match mode {
        MapMode::Political => political_entry(world, province_idx),
        MapMode::Terrain => terrain_entry(world, province_idx),
        MapMode::Manpower => manpower_entry(world, province_idx),
        MapMode::Factories => factories_entry(world, province_idx),
        MapMode::Infrastructure => infrastructure_entry(world, province_idx),
        MapMode::Cores => cores_entry(world, player_country, province_idx),
        MapMode::Ideology => ideology_entry(world, province_idx),
        MapMode::Supply => supply_entry(world, province_idx),
        MapMode::Resistance => resistance_entry(world, province_idx),
    };
    if entry[3] == 0 {
        entry = default_province_entry(world, province_idx);
    }
    entry
}

fn write_pixel(lut: &mut [u8], idx: usize, r: u8, g: u8, b: u8) {
    let o = idx * 4;
    if o + 3 < lut.len() {
        lut[o] = r;
        lut[o + 1] = g;
        lut[o + 2] = b;
        lut[o + 3] = 255;
    }
}

fn rgba(r: u8, g: u8, b: u8) -> [u8; 4] {
    [r, g, b, 255]
}

fn political_lut_color(color: [u8; 3]) -> (u8, u8, u8) {
    (color[0], color[1], color[2])
}

fn country_for_controller_color(
    world: &World,
    owner: CountryId,
    controller: CountryId,
) -> CountryId {
    let country = if !controller.is_none() {
        controller
    } else {
        owner
    };
    world
        .diplomacy
        .autonomy
        .get(&country)
        .map(|autonomy| autonomy.master)
        .unwrap_or(country)
}

fn controller_country(owner: CountryId, controller: CountryId) -> CountryId {
    if !controller.is_none() {
        controller
    } else {
        owner
    }
}

fn fill_political(world: &World, lut: &mut [u8]) {
    for prov_idx in 0..world.provinces.count {
        let owner = world.provinces.owners[prov_idx];
        if owner.is_none() {
            continue;
        }
        let country =
            country_for_controller_color(world, owner, world.provinces.controllers[prov_idx]);
        if country.0 as usize >= world.countries.count {
            continue;
        }
        let c = world.countries.colors[country.0 as usize];
        let (r, g, b) = political_lut_color(c);
        write_pixel(lut, prov_idx, r, g, b);
    }
}

fn fill_terrain(world: &World, lut: &mut [u8]) {
    for def in world.map.definitions.iter().flatten() {
        let (r, g, b) = match def.terrain.as_str() {
            "plains" => (180, 200, 130),
            "forest" => (50, 110, 50),
            "hills" => (140, 130, 90),
            "mountain" => (100, 90, 70),
            "desert" => (220, 200, 130),
            "marsh" => (90, 110, 80),
            "jungle" => (30, 100, 30),
            "urban" => (160, 160, 160),
            _ => (140, 140, 140),
        };
        write_pixel(lut, def.id as usize, r, g, b);
    }
}

fn fill_manpower(world: &World, lut: &mut [u8]) {
    // 找最大值用于归一化
    let max_mp = world
        .states
        .manpower_pool
        .iter()
        .copied()
        .max()
        .unwrap_or(1)
        .max(1);
    for state_idx in 0..world.states.count {
        let mp = world.states.manpower_pool[state_idx];
        let t = (mp as f32 / max_mp as f32).sqrt(); // 平方根让低值更可见
        let r = (50.0 + t * 200.0) as u8;
        let g = (50.0 + (1.0 - t) * 100.0) as u8;
        let b = 50;
        for &prov in &world.states.provinces[state_idx] {
            write_pixel(lut, prov.0 as usize, r, g, b);
        }
    }
}

fn fill_factories(world: &World, lut: &mut [u8]) {
    let max_total: u32 = (0..world.states.count)
        .map(|i| world.state_building_levels(hoi4_state::StateId(i as u16)) as u32)
        .max()
        .unwrap_or(1)
        .max(1);

    for state_idx in 0..world.states.count {
        let total = world.state_building_levels(hoi4_state::StateId(state_idx as u16)) as u32;
        let t = (total as f32 / max_total as f32).sqrt();
        let r = 50;
        let g = (50.0 + t * 180.0) as u8;
        let b = (200.0 - t * 100.0) as u8;
        for &prov in &world.states.provinces[state_idx] {
            write_pixel(lut, prov.0 as usize, r, g, b);
        }
    }
}

fn fill_infrastructure(world: &World, lut: &mut [u8]) {
    for state_idx in 0..world.states.count {
        let infra = world.states.infrastructure[state_idx];
        let t = (infra as f32) / 10.0;
        let r = (200.0 - t * 150.0) as u8;
        let g = (50.0 + t * 200.0) as u8;
        let b = 50;
        for &prov in &world.states.provinces[state_idx] {
            write_pixel(lut, prov.0 as usize, r, g, b);
        }
    }
}

fn fill_cores(world: &World, lut: &mut [u8], player_country: Option<hoi4_state::CountryId>) {
    // 先填政治色
    fill_political(world, lut);
    // 高亮当前选中国家的核心州
    if let Some(country_id) = player_country {
        for state_idx in 0..world.states.count {
            if world.states.cores[state_idx].contains(&country_id) {
                let owner = world.states.owners[state_idx];
                let is_owned = owner == country_id;
                for &prov in &world.states.provinces[state_idx] {
                    if is_owned {
                        write_pixel(lut, prov.0 as usize, 200, 60, 60);
                    } else {
                        write_pixel(lut, prov.0 as usize, 120, 30, 30);
                    }
                }
            }
        }
    }
}

/// 意识形态地图模式 — 按执政党意识形态上色
fn fill_ideology(world: &World, lut: &mut [u8]) {
    // HOI4 标准意识形态颜色
    let ideology_color = |party: &str| -> (u8, u8, u8) {
        match party {
            "democratic" => (60, 90, 170),   // 蓝色
            "communism" => (180, 30, 30),    // 红色
            "fascism" => (80, 60, 40),       // 棕色
            "neutrality" => (140, 140, 140), // 灰色
            _ => (100, 100, 100),
        }
    };
    for prov_idx in 0..world.provinces.count {
        let owner = world.provinces.owners[prov_idx];
        if owner.is_none() {
            continue;
        }
        let country = controller_country(owner, world.provinces.controllers[prov_idx]);
        if country.0 as usize >= world.countries.count {
            continue;
        }
        let party = &world.countries.ruling_party[country.0 as usize];
        let (r, g, b) = ideology_color(party);
        write_pixel(lut, prov_idx, r, g, b);
    }
}

/// 补给地图模式 — 按省份补给等级上色（蓝色梯度）
fn fill_supply(world: &World, lut: &mut [u8]) {
    for prov_idx in 0..world.provinces.count {
        let supply = world.provinces.supply[prov_idx];
        let t = (supply / 100.0).clamp(0.0, 1.0);
        // 深蓝(高补给) → 浅蓝(低补给)
        let r = (30.0 + (1.0 - t) * 170.0) as u8;
        let g = (60.0 + (1.0 - t) * 120.0) as u8;
        let b = (180.0 - (1.0 - t) * 60.0) as u8;
        write_pixel(lut, prov_idx, r, g, b);
    }
}

/// 抵抗地图模式 — 按州抵抗/顺从上色
fn fill_resistance(world: &World, lut: &mut [u8]) {
    for state_idx in 0..world.states.count {
        let _resistance = world.states.resistance[state_idx];
        let compliance = world.states.compliance[state_idx];
        // 抵抗高 → 红色，顺从高 → 绿色，中性 → 黄色
        let t = compliance; // 0..1, 1=fully compliant
        let r = (50.0 + (1.0 - t) * 200.0) as u8;
        let g = (50.0 + t * 200.0) as u8;
        let b = 50u8;
        for &prov in &world.states.provinces[state_idx] {
            write_pixel(lut, prov.0 as usize, r, g, b);
        }
    }
}

fn political_entry(world: &World, province_idx: usize) -> [u8; 4] {
    let Some(&owner) = world.provinces.owners.get(province_idx) else {
        return [0; 4];
    };
    if owner.is_none() {
        return [0; 4];
    }
    let controller = world
        .provinces
        .controllers
        .get(province_idx)
        .copied()
        .unwrap_or(CountryId::NONE);
    let country = country_for_controller_color(world, owner, controller);
    if country.0 as usize >= world.countries.count {
        return [0; 4];
    }
    let (r, g, b) = political_lut_color(world.countries.colors[country.0 as usize]);
    rgba(r, g, b)
}

fn terrain_entry(world: &World, province_idx: usize) -> [u8; 4] {
    let Some(def) = world
        .map
        .definitions
        .get(province_idx)
        .and_then(|def| def.as_ref())
    else {
        return [0; 4];
    };
    let (r, g, b) = match def.terrain.as_str() {
        "plains" => (180, 200, 130),
        "forest" => (50, 110, 50),
        "hills" => (140, 130, 90),
        "mountain" => (100, 90, 70),
        "desert" => (220, 200, 130),
        "marsh" => (90, 110, 80),
        "jungle" => (30, 100, 30),
        "urban" => (160, 160, 160),
        _ => (140, 140, 140),
    };
    rgba(r, g, b)
}

fn manpower_entry(world: &World, province_idx: usize) -> [u8; 4] {
    let Some(state_idx) = state_idx_for_province(world, province_idx) else {
        return [0; 4];
    };
    let max_mp = world
        .states
        .manpower_pool
        .iter()
        .copied()
        .max()
        .unwrap_or(1)
        .max(1);
    let mp = world.states.manpower_pool[state_idx];
    let t = (mp as f32 / max_mp as f32).sqrt();
    rgba(
        (50.0 + t * 200.0) as u8,
        (50.0 + (1.0 - t) * 100.0) as u8,
        50,
    )
}

fn factories_entry(world: &World, province_idx: usize) -> [u8; 4] {
    let Some(state_idx) = state_idx_for_province(world, province_idx) else {
        return [0; 4];
    };
    let max_total: u32 = (0..world.states.count)
        .map(|i| world.state_building_levels(hoi4_state::StateId(i as u16)) as u32)
        .max()
        .unwrap_or(1)
        .max(1);
    let total = world.state_building_levels(hoi4_state::StateId(state_idx as u16)) as u32;
    let t = (total as f32 / max_total as f32).sqrt();
    rgba(50, (50.0 + t * 180.0) as u8, (200.0 - t * 100.0) as u8)
}

fn infrastructure_entry(world: &World, province_idx: usize) -> [u8; 4] {
    let Some(state_idx) = state_idx_for_province(world, province_idx) else {
        return [0; 4];
    };
    let t = (world.states.infrastructure[state_idx] as f32) / 10.0;
    rgba((200.0 - t * 150.0) as u8, (50.0 + t * 200.0) as u8, 50)
}

fn cores_entry(
    world: &World,
    player_country: Option<hoi4_state::CountryId>,
    province_idx: usize,
) -> [u8; 4] {
    let Some(country_id) = player_country else {
        return political_entry(world, province_idx);
    };
    let Some(state_idx) = state_idx_for_province(world, province_idx) else {
        return political_entry(world, province_idx);
    };
    if !world
        .states
        .cores
        .get(state_idx)
        .is_some_and(|cores| cores.contains(&country_id))
    {
        return political_entry(world, province_idx);
    }
    let is_owned = world
        .states
        .owners
        .get(state_idx)
        .copied()
        .is_some_and(|owner| owner == country_id);
    if is_owned {
        rgba(200, 60, 60)
    } else {
        rgba(120, 30, 30)
    }
}

fn ideology_entry(world: &World, province_idx: usize) -> [u8; 4] {
    let Some(&owner) = world.provinces.owners.get(province_idx) else {
        return [0; 4];
    };
    if owner.is_none() {
        return [0; 4];
    }
    let controller = world
        .provinces
        .controllers
        .get(province_idx)
        .copied()
        .unwrap_or(CountryId::NONE);
    let country = controller_country(owner, controller);
    if country.0 as usize >= world.countries.count {
        return [0; 4];
    }
    let (r, g, b) = match world.countries.ruling_party[country.0 as usize].as_str() {
        "democratic" => (60, 90, 170),
        "communism" => (180, 30, 30),
        "fascism" => (80, 60, 40),
        "neutrality" => (140, 140, 140),
        _ => (100, 100, 100),
    };
    rgba(r, g, b)
}

fn supply_entry(world: &World, province_idx: usize) -> [u8; 4] {
    let Some(&supply) = world.provinces.supply.get(province_idx) else {
        return [0; 4];
    };
    let t = (supply / 100.0).clamp(0.0, 1.0);
    rgba(
        (30.0 + (1.0 - t) * 170.0) as u8,
        (60.0 + (1.0 - t) * 120.0) as u8,
        (180.0 - (1.0 - t) * 60.0) as u8,
    )
}

fn resistance_entry(world: &World, province_idx: usize) -> [u8; 4] {
    let Some(state_idx) = state_idx_for_province(world, province_idx) else {
        return [0; 4];
    };
    let t = world.states.compliance[state_idx];
    rgba(
        (50.0 + (1.0 - t) * 200.0) as u8,
        (50.0 + t * 200.0) as u8,
        50,
    )
}

fn state_idx_for_province(world: &World, province_idx: usize) -> Option<usize> {
    let state = world.provinces.state_of.get(province_idx).copied()?;
    if state.is_none() {
        return None;
    }
    let state_idx = state.0 as usize;
    (state_idx < world.states.count).then_some(state_idx)
}

fn default_province_entry(world: &World, province_idx: usize) -> [u8; 4] {
    let Some(def) = world
        .map
        .definitions
        .get(province_idx)
        .and_then(|def| def.as_ref())
    else {
        return [0; 4];
    };
    let (r, g, b) = match def.province_type {
        ProvinceType::Sea => (40, 60, 120),
        ProvinceType::Lake => (60, 80, 140),
        ProvinceType::Land => (80, 80, 80),
    };
    rgba(r, g, b)
}

/// Build the legacy occupation overlay LUT.
///
/// Layout matches `build_color_lut` (Vec<u8> RGBA, indexed by province ID),
/// but all entries are transparent because the main colour LUT already uses
/// controller colours for occupied land.
/// provinces alpha is 0 → the shader treats this as "no overlay".
///
/// This legacy texture is still bound for compatibility, but alpha remains zero.
/// * If owner and controller are at war → red (`220, 50, 50`).
/// * Else (peaceful occupation, e.g. via decision) → muted gray (`130, 130, 140`).
pub fn build_occupation_lut(world: &World) -> Vec<u8> {
    let count = world.map.definitions.len();
    // Controller colour already carries occupation; keep this legacy overlay transparent.
    vec![0u8; count * 4]
}

fn fill_default_water(world: &World, lut: &mut [u8]) {
    for def in world.map.definitions.iter().flatten() {
        let o = def.id as usize * 4;
        if o + 3 >= lut.len() {
            continue;
        }
        if lut[o + 3] == 0 {
            let (r, g, b) = match def.province_type {
                ProvinceType::Sea => (40, 60, 120),
                ProvinceType::Lake => (60, 80, 140),
                ProvinceType::Land => (80, 80, 80),
            };
            write_pixel(lut, def.id as usize, r, g, b);
        }
    }
}

// ─── 外交边界纹理 ──────────────────────────────────────────────────

/// Border type encoded per pixel for diplomacy-colored borders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DiplomacyBorderType {
    /// No border or peaceful border (same faction or peace).
    None = 0,
    /// War border (attacker vs defender).
    War = 1,
    /// Same faction border.
    Faction = 2,
}

/// Compute a per-pixel diplomacy border type texture.
///
/// For each pixel on a country border (where the controller changes between
/// adjacent pixels), encode the diplomatic relationship between the two
/// countries: war, same faction, or neutral.
///
/// Returns `Vec<u8>` with one byte per pixel (map_width × map_height).
pub fn compute_diplomacy_border_texture(world: &World) -> Vec<u8> {
    let pmap = &world.map.province_map;
    let w = pmap.width as usize;
    let h = pmap.height as usize;
    let pixels = &pmap.pixels;
    let controllers = &world.provinces.controllers;
    let count = world.countries.count;

    let owner_of = |pid: u16| -> hoi4_state::CountryId {
        controllers
            .get(pid as usize)
            .copied()
            .unwrap_or(hoi4_state::CountryId::NONE)
    };

    // Precompute war lookup: (a, b) → true if at war.
    // Use a flat bitset indexed by (a * count + b) for O(1) lookup.
    let war_bits = {
        let sz = count * count;
        let mut bits = vec![false; sz];
        for war in world.diplomacy.wars.values() {
            for &atk in &war.attackers {
                for &def in &war.defenders {
                    if (atk.0 as usize) < count && (def.0 as usize) < count {
                        bits[atk.0 as usize * count + def.0 as usize] = true;
                        bits[def.0 as usize * count + atk.0 as usize] = true;
                    }
                }
            }
        }
        bits
    };

    let at_war_with = |a: hoi4_state::CountryId, b: hoi4_state::CountryId| -> bool {
        let ai = a.0 as usize;
        let bi = b.0 as usize;
        if ai < count && bi < count {
            war_bits[ai * count + bi]
        } else {
            false
        }
    };

    let same_faction = |a: hoi4_state::CountryId, b: hoi4_state::CountryId| -> bool {
        let fa = world.diplomacy.faction_of(a);
        let fb = world.diplomacy.faction_of(b);
        fa.is_some() && fa == fb
    };

    let mut out = vec![0u8; w * h];

    for y in 0..h {
        for x in 0..w {
            let me = pixels[y * w + x];
            let ca = owner_of(me);
            if ca.is_none() {
                continue;
            }

            let check_neighbor = |nx: usize, ny: usize| -> u8 {
                let n_pid = pixels[ny * w + nx];
                let cb = owner_of(n_pid);
                if cb.is_none() || cb == ca {
                    return 0;
                }
                if at_war_with(ca, cb) {
                    DiplomacyBorderType::War as u8
                } else if same_faction(ca, cb) {
                    DiplomacyBorderType::Faction as u8
                } else {
                    0 // neutral border — no special colour
                }
            };

            let mut border = 0u8;
            if x + 1 < w {
                border = border.max(check_neighbor(x + 1, y));
            }
            if y + 1 < h {
                border = border.max(check_neighbor(x, y + 1));
            }
            if x > 0 {
                border = border.max(check_neighbor(x - 1, y));
            }
            if y > 0 {
                border = border.max(check_neighbor(x, y - 1));
            }
            out[y * w + x] = border;
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;

    use hoi4_data::{Color, Country, CountryTag, GameData, State};
    use hoi4_map::{GameMap, Heightmap, ProvinceDefinition, ProvinceMap, TerrainBitmap};
    use hoi4_state::{Autonomy, AutonomyLevel, World};

    fn test_map() -> Arc<GameMap> {
        let mut definitions = vec![None; 3];
        for id in 1..=2u16 {
            definitions[id as usize] = Some(ProvinceDefinition {
                id,
                r: id as u8,
                g: 0,
                b: 0,
                province_type: ProvinceType::Land,
                coastal: false,
                terrain: "plains".to_owned(),
                continent: 1,
            });
        }

        Arc::new(GameMap {
            definitions,
            rgb_to_id: HashMap::new(),
            province_map: ProvinceMap {
                width: 2,
                height: 1,
                pixels: vec![1, 2],
            },
            adjacencies: vec![vec![], vec![2], vec![1]],
            special_adjacencies: vec![],
            heightmap: Heightmap {
                width: 2,
                height: 1,
                pixels: vec![0, 0],
            },
            terrain_bmp: TerrainBitmap {
                width: 2,
                height: 1,
                pixels: vec![0, 0],
                palette: [[0; 3]; 256],
            },
            terrain_catalog: hoi4_map::TerrainCatalog::default(),
            tree_definition_bmp: None,
            tree_indices: HashSet::new(),
        })
    }

    fn test_world() -> World {
        let mut data = GameData::default();
        for (tag, color, party) in [
            (
                "FRA",
                Color {
                    r: 30,
                    g: 70,
                    b: 180,
                },
                "democratic",
            ),
            (
                "GER",
                Color {
                    r: 90,
                    g: 90,
                    b: 90,
                },
                "fascism",
            ),
        ] {
            let country_tag = CountryTag::new(tag);
            data.countries.insert(
                country_tag.clone(),
                Country {
                    tag: country_tag,
                    color,
                    graphical_culture: "western_european_gfx".to_owned(),
                    capital: 1,
                    ruling_party: party.to_owned(),
                    technologies: Vec::new(),
                },
            );
        }

        data.states.push(State {
            id: 1,
            name: "Test State".to_owned(),
            manpower: 0,
            owner: CountryTag::new("FRA"),
            cores: vec![CountryTag::new("FRA")],
            provinces: vec![1, 2],
            category: "test".to_owned(),
            infrastructure: 1,
            victory_points: Vec::new(),
            resources: Vec::new(),
        });

        let mut world = World::new(test_map(), Arc::new(data));
        let ger = world.country("GER").unwrap();
        world.provinces.controllers[1] = ger;
        world
    }

    fn rgb_at(lut: &[u8], province_id: usize) -> [u8; 3] {
        let o = province_id * 4;
        [lut[o], lut[o + 1], lut[o + 2]]
    }

    fn rgba_at(lut: &[u8], province_id: usize) -> [u8; 4] {
        let o = province_id * 4;
        [lut[o], lut[o + 1], lut[o + 2], lut[o + 3]]
    }

    #[test]
    fn political_lut_uses_controller_color_and_owner_fallback() {
        let world = test_world();
        let fra = world.country("FRA").unwrap();
        let ger = world.country("GER").unwrap();

        let lut = build_color_lut(&world, MapMode::Political, None);

        let ger_color = political_lut_color(world.countries.colors[ger.0 as usize]);
        let fra_color = political_lut_color(world.countries.colors[fra.0 as usize]);
        assert_eq!(rgb_at(&lut, 1), [ger_color.0, ger_color.1, ger_color.2]);
        assert_eq!(rgb_at(&lut, 2), [fra_color.0, fra_color.1, fra_color.2]);
    }

    #[test]
    fn political_lut_paints_subjects_with_master_color() {
        let mut world = test_world();
        let fra = world.country("FRA").unwrap();
        let ger = world.country("GER").unwrap();
        world.diplomacy.autonomy.insert(
            fra,
            Autonomy {
                master: ger,
                subject: fra,
                level: AutonomyLevel::Puppet,
                progress: 0.0,
                since_hour: 0,
            },
        );

        let lut = build_color_lut(&world, MapMode::Political, None);
        let ger_color = political_lut_color(world.countries.colors[ger.0 as usize]);

        assert_eq!(rgb_at(&lut, 2), [ger_color.0, ger_color.1, ger_color.2]);
    }

    #[test]
    fn political_lut_keeps_source_country_color_before_postprocess() {
        assert_eq!(political_lut_color([60, 90, 170]), (60, 90, 170));
        assert_eq!(political_lut_color([210, 205, 180]), (210, 205, 180));
    }

    #[test]
    fn ideology_lut_uses_controller_party() {
        let world = test_world();
        let lut = build_color_lut(&world, MapMode::Ideology, None);

        assert_eq!(rgb_at(&lut, 1), [80, 60, 40]);
        assert_eq!(rgb_at(&lut, 2), [60, 90, 170]);
    }

    #[test]
    fn color_lut_entry_matches_full_lut_for_controller_sensitive_modes() {
        let world = test_world();
        let fra = world.country("FRA").unwrap();

        for mode in [MapMode::Political, MapMode::Cores, MapMode::Ideology] {
            let lut = build_color_lut(&world, mode, Some(fra));
            for province_id in 1..=2 {
                assert_eq!(
                    color_lut_entry(&world, mode, Some(fra), province_id),
                    rgba_at(&lut, province_id),
                    "mode {mode:?} province {province_id}"
                );
            }
        }
    }

    #[test]
    fn occupation_overlay_lut_is_transparent() {
        let world = test_world();
        let lut = build_occupation_lut(&world);
        let occupied_alpha = lut[1 * 4 + 3];
        let legal_owner_alpha = lut[2 * 4 + 3];

        assert_eq!(occupied_alpha, 0);
        assert_eq!(legal_owner_alpha, 0);
    }
}
