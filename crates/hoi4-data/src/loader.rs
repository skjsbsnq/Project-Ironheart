use crate::air::{baseline_for_aircraft, AircraftDef};
use crate::building::{BuildingDef, BuildingScope};
use crate::characters::{load_characters_from_paths, CharacterDef};
use crate::country::{Color, Country, CountryTag};
use crate::equipment::{EquipmentCategory, EquipmentDef};
use crate::history::{
    load_country_histories_from_paths, load_oob_air_from_paths, load_oob_land_from_paths,
    load_oob_naval_from_paths, CountryHistory, OobAir, OobLand, OobNaval,
};
use crate::military::{CombatTactic, DivisionTemplate, SubunitDef};
use crate::naval::{baseline_for_class, ShipClassDef};
use crate::politics::{DecisionCategoryDef, DecisionDef, FocusTree, IdeaDef, Ideology};
use crate::resource::{ResourceDef, ResourceKind};
use crate::state::State;
use crate::technology::{TechPath, Technology};
use clausewitz_parser::{parse, Block, Operator, Value};
use hoi4_paths::PathConfig;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct StateOwner1936Override {
    state_id: u16,
    owner: String,
    cores: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CustomCountry1936Def {
    tag: String,
    color: Color1936Def,
    capital: u16,
    ruling_party: String,
}

#[derive(Debug, Deserialize)]
struct Color1936Def {
    r: u8,
    g: u8,
    b: u8,
}

/// State category definition (local_building_slots cap).
#[derive(Clone)]
pub struct StateCategoryDef {
    pub name: String,
    pub local_building_slots: u8,
}

/// All loaded game data
#[derive(Default, Clone)]
pub struct GameData {
    pub countries: HashMap<CountryTag, Country>,
    pub states: Vec<State>,
    /// province_id → owner tag (derived from states)
    pub province_owners: HashMap<u16, CountryTag>,
    /// 建筑类型表（key → 定义）
    pub buildings: HashMap<String, BuildingDef>,
    /// 资源类型表
    pub resources: HashMap<ResourceKind, ResourceDef>,
    /// 装备类型表（key → 定义）
    pub equipment: HashMap<String, EquipmentDef>,
    /// 科技表（key → 定义）
    pub technologies: HashMap<String, Technology>,
    /// 反向索引：tech_key → 它的前置依赖列表（其它 tech 的 paths.leads_to 指向本 tech）
    pub tech_prereqs: HashMap<String, Vec<String>>,
    /// 意识形态表
    pub ideologies: HashMap<String, Ideology>,
    /// 国家精神表（key → 定义；跨 country/political_advisor/etc. 类别合并）
    pub ideas: HashMap<String, IdeaDef>,
    /// 国策树（tree_id → 树）
    pub focus_trees: HashMap<String, FocusTree>,
    /// 反向索引：focus_id → 所在树 id（便于查找）
    pub focus_to_tree: HashMap<String, String>,
    /// 子单位（兵种）表
    pub subunits: HashMap<String, SubunitDef>,
    /// 战斗战术表
    pub combat_tactics: HashMap<String, CombatTactic>,
    /// 师编制模板（tag → 该国所有模板）
    pub division_templates: HashMap<String, Vec<DivisionTemplate>>,
    /// 舰类表（destroyer / battleship 等 → 定义）
    pub ship_classes: HashMap<String, ShipClassDef>,
    /// 飞机类表（fighter / cas / strategic_bomber 等 → 定义）
    pub aircraft: HashMap<String, AircraftDef>,
    /// Phase 1.1：陆军 OOB（key = 文件 stem，例 "GER_1936"）
    pub oob_land: HashMap<String, OobLand>,
    /// Phase 1.1：海军 OOB（key 同上，例 "GER_1936_naval"）
    pub oob_naval: HashMap<String, OobNaval>,
    /// Phase 1.1：空军 OOB（key 同上，例 "GER_1936_air_legacy"）
    pub oob_air: HashMap<String, OobAir>,
    /// Phase 1.1：每国 1936 启动时的初始 ideas / focus / vars / flags / set_oob 引用
    pub country_histories: HashMap<String, CountryHistory>,
    /// Phase 4.3：决议类别元数据（category_id → 定义）
    pub decision_categories: HashMap<String, DecisionCategoryDef>,
    /// Phase 4.3：决议定义（decision_id → 定义）
    pub decisions: HashMap<String, DecisionDef>,
    /// J.1.1：所有 country 的角色（按 (tag, key) 字典序稳定排序）。
    pub characters: Vec<CharacterDef>,
    /// J.1.1：tag → 该国所有角色在 [`Self::characters`] 中的索引。
    pub characters_by_tag: HashMap<CountryTag, Vec<usize>>,
    /// J.3: state category → building slot cap
    pub state_categories: HashMap<String, StateCategoryDef>,
    /// J.1.3：`localisation/english/parties_l_english.yml` 等的 party 名映射。
    /// key = vanilla loc key，例如 `GER_fascism_party_long`；value = 字面字符串。
    pub party_names: HashMap<String, String>,
    /// J.1.3：`localisation/english/*characters*l_english*.yml` 中所有 character
    /// 名映射；key = vanilla loc key（`GER_adolf_hitler`）；value = 字面字符串
    /// （`Adolf Hitler`）。
    pub character_names: HashMap<String, String>,
}

#[derive(Debug, Default, Clone)]
pub struct LoadedCounts {
    pub countries: usize,
    pub states: usize,
    pub buildings: usize,
    pub resources: usize,
    pub equipment: usize,
    pub technologies: usize,
    pub ideas: usize,
    pub subunits: usize,
    pub division_templates: usize,
    pub characters: usize,
}

#[derive(Debug, Clone)]
pub struct LoadWarning {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Default, Clone)]
pub struct GameDataLoadReport {
    pub loaded_counts: LoadedCounts,
    pub warnings: Vec<LoadWarning>,
    pub optional_missing: Vec<String>,
}

impl GameDataLoadReport {
    fn record_optional<T>(&mut self, label: &str, result: Result<T, String>) -> T
    where
        T: Default,
    {
        match result {
            Ok(value) => value,
            Err(message) => {
                self.warnings.push(LoadWarning {
                    path: label.to_owned(),
                    message,
                });
                T::default()
            }
        }
    }
}

fn data_dirs(paths: &PathConfig, relative: &str) -> Vec<PathBuf> {
    let mut dirs = paths.find_all(relative);
    // Merge low priority first so higher-priority mods overwrite vanilla keys.
    dirs.reverse();
    dirs
}

impl GameData {
    pub fn load(game_path: &Path) -> Result<Self, String> {
        Self::load_from_paths(&PathConfig::with_game_path(game_path)).map(|(data, _)| data)
    }

    pub fn load_from_paths(paths: &PathConfig) -> Result<(Self, GameDataLoadReport), String> {
        let mut report = GameDataLoadReport::default();
        let country_tags = load_country_tags(paths)?;
        let colors = load_country_colors(paths)?;
        let mut countries = load_countries(paths, &country_tags, &colors)?;
        apply_custom_country_1936_defs(&mut countries)?;
        let mut states = load_states(paths)?;
        apply_state_owner_1936_overrides(&mut states)?;
        let buildings = report.record_optional("common/buildings", load_buildings(paths));
        let resources =
            report.record_optional("common/resources/00_resources.txt", load_resources(paths));
        let equipment = report.record_optional("common/units/equipment", load_equipment(paths));
        let technologies = report.record_optional("common/technologies", load_technologies(paths));
        let tech_prereqs = build_tech_prereqs(&technologies);
        let ideologies = report.record_optional(
            "common/ideologies/00_ideologies.txt",
            load_ideologies(paths),
        );
        let mut ideas = report.record_optional("common/ideas", load_ideas(paths));
        inject_sino_japanese_war_ideas(&mut ideas);
        let focus_trees =
            report.record_optional("common/national_focus", load_focus_trees(paths.game_path()));
        let focus_to_tree = build_focus_index(&focus_trees);
        let subunits = report.record_optional("common/units", load_subunits(paths));
        let combat_tactics =
            report.record_optional("common/combat_tactics.txt", load_combat_tactics(paths));
        let division_templates =
            report.record_optional("history/units", load_division_templates(paths));
        let ship_classes = report.record_optional("common/units", load_ship_classes(paths));
        let aircraft = report.record_optional("common/units", load_aircraft(paths));
        let oob_land = report.record_optional("history/units", load_oob_land_from_paths(paths));
        let oob_naval = report.record_optional("history/units", load_oob_naval_from_paths(paths));
        let oob_air = report.record_optional("history/units", load_oob_air_from_paths(paths));
        let country_histories = report.record_optional(
            "history/countries",
            load_country_histories_from_paths(paths),
        );
        let decision_categories = report.record_optional(
            "common/decisions/categories",
            load_decision_categories(paths.game_path()),
        );
        let decisions: HashMap<String, DecisionDef> = HashMap::new();
        let (characters, characters_by_tag) = load_characters_from_paths(paths);
        let party_names = load_party_names(paths);
        let character_names = load_character_names(paths);
        let state_categories =
            report.record_optional("common/state_category", load_state_categories(paths));

        let mut province_owners = HashMap::new();
        for state in &states {
            for &prov in &state.provinces {
                province_owners.insert(prov, state.owner.clone());
            }
        }

        report.loaded_counts = LoadedCounts {
            countries: countries.len(),
            states: states.len(),
            buildings: buildings.len(),
            resources: resources.len(),
            equipment: equipment.len(),
            technologies: technologies.len(),
            ideas: ideas.len(),
            subunits: subunits.len(),
            division_templates: division_templates.values().map(Vec::len).sum(),
            characters: characters.len(),
        };

        Ok((
            Self {
                countries,
                states,
                province_owners,
                buildings,
                resources,
                equipment,
                technologies,
                tech_prereqs,
                ideologies,
                ideas,
                focus_trees,
                focus_to_tree,
                subunits,
                combat_tactics,
                division_templates,
                ship_classes,
                aircraft,
                oob_land,
                oob_naval,
                oob_air,
                country_histories,
                decision_categories,
                decisions,
                characters,
                characters_by_tag,
                party_names,
                character_names,
                state_categories,
            },
            report,
        ))
    }
}

fn apply_custom_country_1936_defs(
    countries: &mut HashMap<CountryTag, Country>,
) -> Result<(), String> {
    let custom_countries: Vec<CustomCountry1936Def> =
        ron::from_str(include_str!("../content/history_1936/custom_countries.ron"))
            .map_err(|err| format!("failed to parse history_1936/custom_countries.ron: {err}"))?;

    for entry in custom_countries {
        let tag = CountryTag::new(&entry.tag);
        countries.insert(
            tag.clone(),
            Country {
                tag,
                color: Color {
                    r: entry.color.r,
                    g: entry.color.g,
                    b: entry.color.b,
                },
                graphical_culture: String::new(),
                capital: entry.capital,
                ruling_party: entry.ruling_party,
                technologies: Vec::new(),
            },
        );
    }
    Ok(())
}

fn apply_state_owner_1936_overrides(states: &mut [State]) -> Result<(), String> {
    let overrides: Vec<StateOwner1936Override> =
        ron::from_str(include_str!("../content/history_1936/state_owners.ron"))
            .map_err(|err| format!("failed to parse history_1936/state_owners.ron: {err}"))?;

    for entry in overrides {
        let Some(state) = states.iter_mut().find(|state| state.id == entry.state_id) else {
            return Err(format!(
                "history_1936/state_owners.ron references missing state {}",
                entry.state_id
            ));
        };
        state.owner = CountryTag::new(&entry.owner);
        state.cores = entry.cores.iter().map(|tag| CountryTag::new(tag)).collect();
    }
    Ok(())
}

fn load_country_tags(paths: &PathConfig) -> Result<HashMap<String, String>, String> {
    let mut tags = HashMap::new();
    let tag_dirs = data_dirs(paths, "common/country_tags");
    if tag_dirs.is_empty() {
        return Err("missing required directory `common/country_tags`".to_owned());
    }
    for tags_dir in tag_dirs {
        for entry in fs::read_dir(&tags_dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let content = fs::read_to_string(entry.path()).map_err(|e| e.to_string())?;
            let block = parse(&content);
            for e in &block.entries {
                if e.key.len() == 3 {
                    if let Value::String(path) = &e.value {
                        tags.insert(e.key.clone(), path.clone());
                    }
                }
            }
        }
    }
    Ok(tags)
}

fn load_country_colors(paths: &PathConfig) -> Result<HashMap<String, Color>, String> {
    let path = paths
        .find("common/countries/colors.txt")
        .ok_or_else(|| "missing required file `common/countries/colors.txt`".to_owned())?;
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let block = parse(&content);
    let mut colors = HashMap::new();

    for entry in &block.entries {
        if let Value::Block(country_block) = &entry.value {
            // Look for `color = rgb { R G B }` or `color = HSV { H S V }`
            // The parser sees: color_entry.value = Block with "rgb" or "HSV" key pointing to a block of values
            // Actually the parser sees: key="color", value=Block containing entries/values
            // Let's try to extract from the color block
            if let Some(color_val) = country_block.get("color") {
                let (r, g, b) = extract_color(color_val);
                colors.insert(entry.key.clone(), Color { r, g, b });
            }
        }
    }
    Ok(colors)
}

fn extract_color(value: &Value) -> (u8, u8, u8) {
    match value {
        Value::Block(b) => {
            // Could be `rgb { R G B }` → parser sees key="rgb" value=Block{values=[R,G,B]}
            if let Some(Value::Block(rgb_block)) = b.get("rgb") {
                let vals: Vec<f64> = rgb_block
                    .values
                    .iter()
                    .filter_map(|v| match v {
                        Value::Integer(i) => Some(*i as f64),
                        Value::Float(f) => Some(*f),
                        _ => None,
                    })
                    .collect();
                if vals.len() >= 3 {
                    return (vals[0] as u8, vals[1] as u8, vals[2] as u8);
                }
            }
            // Could be `HSV { H S V }` → convert to RGB
            if let Some(Value::Block(hsv_block)) = b.get("HSV").or_else(|| b.get("hsv")) {
                let vals: Vec<f64> = hsv_block
                    .values
                    .iter()
                    .filter_map(|v| match v {
                        Value::Float(f) => Some(*f),
                        Value::Integer(i) => Some(*i as f64),
                        _ => None,
                    })
                    .collect();
                if vals.len() >= 3 {
                    return hsv_to_rgb(vals[0], vals[1], vals[2]);
                }
            }
            // Bare values { R G B } (no rgb/HSV prefix)
            let vals: Vec<f64> = b
                .values
                .iter()
                .filter_map(|v| match v {
                    Value::Integer(i) => Some(*i as f64),
                    Value::Float(f) => Some(*f),
                    _ => None,
                })
                .collect();
            if vals.len() >= 3 {
                if vals[0] <= 1.0 && vals[1] <= 1.0 && vals[2] <= 1.0 {
                    return (
                        (vals[0] * 255.0) as u8,
                        (vals[1] * 255.0) as u8,
                        (vals[2] * 255.0) as u8,
                    );
                }
                return (vals[0] as u8, vals[1] as u8, vals[2] as u8);
            }
            (128, 128, 128)
        }
        _ => (128, 128, 128),
    }
}

fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h * 6.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

fn load_countries(
    paths: &PathConfig,
    tags: &HashMap<String, String>,
    colors: &HashMap<String, Color>,
) -> Result<HashMap<CountryTag, Country>, String> {
    let mut countries = HashMap::new();
    let history_dirs = data_dirs(paths, "history/countries");

    for (tag, _country_file) in tags {
        // Find history file matching this tag
        let history_file = find_history_file(&history_dirs, tag);
        let (capital, ruling_party, technologies) = if let Some(path) = history_file {
            let content = fs::read_to_string(&path).unwrap_or_default();
            let block = parse(&content);
            let capital = block.get_int("capital").unwrap_or(0) as u16;
            let ruling_party = block
                .get_string("set_politics")
                .map(|s| s.to_owned())
                .unwrap_or_default();
            // Extract technologies from set_technology blocks
            let mut techs = Vec::new();
            for tech_block in block.get_all("set_technology") {
                if let Value::Block(b) = tech_block {
                    for e in &b.entries {
                        if let Value::Integer(1) = &e.value {
                            techs.push(e.key.clone());
                        }
                    }
                }
            }
            (capital, ruling_party, techs)
        } else {
            (0, String::new(), Vec::new())
        };

        let color = colors.get(tag).copied().unwrap_or(Color {
            r: 128,
            g: 128,
            b: 128,
        });
        let graphical_culture = String::new(); // loaded from common/countries/ file

        let country_tag = CountryTag::new(tag);
        countries.insert(
            country_tag.clone(),
            Country {
                tag: country_tag,
                color,
                graphical_culture,
                capital,
                ruling_party,
                technologies,
            },
        );
    }
    Ok(countries)
}

fn find_history_file(dirs: &[std::path::PathBuf], tag: &str) -> Option<std::path::PathBuf> {
    let prefix = format!("{} -", tag);
    let prefix2 = format!("{} ", tag); // some files have space without dash
    for dir in dirs {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(&prefix) || name.starts_with(&prefix2) {
                    return Some(entry.path());
                }
            }
        }
    }
    None
}

fn load_states(paths: &PathConfig) -> Result<Vec<State>, String> {
    let state_dirs = data_dirs(paths, "history/states");
    if state_dirs.is_empty() {
        return Err("missing required directory `history/states`".to_owned());
    }
    let mut states = Vec::new();

    for states_dir in state_dirs {
        for entry in fs::read_dir(&states_dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.path().extension().map_or(false, |e| e == "txt") {
                continue;
            }

            let content = fs::read_to_string(entry.path()).unwrap_or_default();
            let block = parse(&content);

            let state_block = match block.get_block("state") {
                Some(b) => b,
                None => continue,
            };

            let id = state_block.get_int("id").unwrap_or(0) as u16;
            let name = state_block.get_string("name").unwrap_or("").to_owned();
            let manpower = state_block.get_int("manpower").unwrap_or(0) as u64;
            let category = state_block
                .get_string("state_category")
                .unwrap_or("")
                .to_owned();

            let provinces: Vec<u16> = state_block
                .get_block("provinces")
                .map(|b| {
                    b.values
                        .iter()
                        .filter_map(|v| match v {
                            Value::Integer(i) => Some(*i as u16),
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default();

            let history = state_block.get_block("history");
            let owner = history
                .and_then(|h| top_level_history_string(h, "owner"))
                .unwrap_or("---")
                .to_owned();

            let cores: Vec<CountryTag> = history
                .map(|h| {
                    top_level_history_all_strings(h, "add_core_of")
                        .into_iter()
                        .map(|s| CountryTag::new(&s))
                        .collect()
                })
                .unwrap_or_default();

            let mut infrastructure = 0u8;
            if let Some(buildings) = history.and_then(|h| top_level_history_block(h, "buildings")) {
                infrastructure = buildings.get_int("infrastructure").unwrap_or(0) as u8;
            }

            let mut victory_points = Vec::new();
            if let Some(h) = history {
                for vp in h.get_all("victory_points") {
                    if let Value::Block(b) = vp {
                        let vals: Vec<i64> = b
                            .values
                            .iter()
                            .filter_map(|v| match v {
                                Value::Integer(i) => Some(*i),
                                _ => None,
                            })
                            .collect();
                        if vals.len() >= 2 {
                            victory_points.push((vals[0] as u16, vals[1] as u8));
                        }
                    }
                }
            }

            let resources: Vec<(ResourceKind, f32)> = state_block
                .get_block("resources")
                .map(|b| {
                    b.entries
                        .iter()
                        .filter_map(|e| {
                            let kind = ResourceKind::from_str(&e.key)?;
                            let amount = match &e.value {
                                Value::Integer(i) => *i as f32,
                                Value::Float(f) => *f as f32,
                                _ => return None,
                            };
                            Some((kind, amount))
                        })
                        .collect()
                })
                .unwrap_or_default();

            states.push(State {
                id,
                name,
                manpower,
                owner: CountryTag::new(&owner),
                cores,
                provinces,
                category,
                infrastructure,
                victory_points,
                resources,
            });
        }
    }

    states.sort_by_key(|s| s.id);
    Ok(states)
}

fn top_level_history_string<'a>(history: &'a Block, key: &str) -> Option<&'a str> {
    history.entries.iter().find_map(|entry| {
        if entry.key != key {
            return None;
        }
        if matches!(entry.op, Operator::Eq) {
            if let Value::String(s) = &entry.value {
                return Some(s.as_str());
            }
        }
        None
    })
}

fn top_level_history_all_strings(history: &Block, key: &str) -> Vec<String> {
    history
        .entries
        .iter()
        .filter_map(|entry| {
            if entry.key != key || !matches!(entry.op, Operator::Eq) {
                return None;
            }
            if let Value::String(s) = &entry.value {
                return Some(s.clone());
            }
            None
        })
        .collect()
}

fn top_level_history_block<'a>(history: &'a Block, key: &str) -> Option<&'a Block> {
    history.entries.iter().find_map(|entry| {
        if entry.key != key || !matches!(entry.op, Operator::Eq) {
            return None;
        }
        if let Value::Block(b) = &entry.value {
            return Some(b);
        }
        None
    })
}

// ─── 建筑加载 ───────────────────────────────────────────────────────

fn load_buildings(paths: &PathConfig) -> Result<HashMap<String, BuildingDef>, String> {
    let dirs = data_dirs(paths, "common/buildings");
    let mut out = HashMap::new();
    for dir in dirs {
        for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.path().extension().map_or(false, |e| e == "txt") {
                continue;
            }
            let content = fs::read_to_string(entry.path()).unwrap_or_default();
            let block = parse(&content);
            let buildings_block = match block.get_block("buildings") {
                Some(b) => b,
                None => continue,
            };
            for e in &buildings_block.entries {
                let Value::Block(b) = &e.value else { continue };

                let base_cost = b.get_float("base_cost").unwrap_or(0.0) as f32;
                let per_level_extra_cost =
                    b.get_float("per_level_extra_cost").unwrap_or(0.0) as f32;
                let is_infrastructure = b.get_bool("infrastructure").unwrap_or(false);
                let air_base_flag = b.get_bool("air_base").unwrap_or(false);
                let is_naval_base = b.get_bool("is_port").unwrap_or(false);
                let production = b.get_int("military_production").unwrap_or(0);
                let general_production = b.get_int("general_production").unwrap_or(0);
                // naval yards 在 vanilla 中以 specialization 区分；HOI4 默认有 dockyard 入口
                let is_dockyard = e.key == "dockyard";
                let is_military = production > 0 || e.key == "arms_factory";
                let is_civilian = general_production > 0 || e.key == "industrial_complex";

                let mut state_max: Option<u8> = None;
                let mut province_max: Option<u8> = None;
                let mut shares_slots = false;
                if let Some(cap) = b.get_block("level_cap") {
                    if let Some(v) = cap.get_int("state_max") {
                        state_max = Some(v.clamp(0, 255) as u8);
                    }
                    if let Some(v) = cap.get_int("province_max") {
                        province_max = Some(v.clamp(0, 255) as u8);
                    }
                    shares_slots = cap.get_bool("shares_slots").unwrap_or(false);
                }
                let scope = if province_max.is_some() {
                    BuildingScope::Province
                } else {
                    BuildingScope::State
                };

                out.insert(
                    e.key.clone(),
                    BuildingDef {
                        key: e.key.clone(),
                        base_cost,
                        per_level_extra_cost,
                        is_infrastructure,
                        is_military,
                        is_civilian,
                        is_dockyard,
                        is_air_base: air_base_flag,
                        is_naval_base,
                        scope,
                        state_max,
                        province_max,
                        shares_slots,
                    },
                );
            }
        }
    }
    // dockyard 在原版没有显式定义，硬编码兜底
    out.entry("dockyard".to_owned()).or_insert(BuildingDef {
        key: "dockyard".to_owned(),
        base_cost: 7200.0,
        per_level_extra_cost: 0.0,
        is_infrastructure: false,
        is_military: false,
        is_civilian: false,
        is_dockyard: true,
        is_air_base: false,
        is_naval_base: false,
        scope: BuildingScope::State,
        state_max: Some(20),
        province_max: None,
        shares_slots: true,
    });
    Ok(out)
}

// ─── 资源加载 ───────────────────────────────────────────────────────

fn load_resources(paths: &PathConfig) -> Result<HashMap<ResourceKind, ResourceDef>, String> {
    let path = paths
        .find("common/resources/00_resources.txt")
        .ok_or_else(|| "missing optional file `common/resources/00_resources.txt`".to_owned())?;
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let block = parse(&content);
    let resources_block = block
        .get_block("resources")
        .ok_or_else(|| "missing `resources={}` block".to_owned())?;

    let mut out = HashMap::new();
    for e in &resources_block.entries {
        let Value::Block(b) = &e.value else { continue };
        let Some(kind) = ResourceKind::from_str(&e.key) else {
            continue;
        };
        let cic = b.get_float("cic").unwrap_or(0.125) as f32;
        let convoys = b.get_float("convoys").unwrap_or(0.1) as f32;
        out.insert(kind, ResourceDef { kind, cic, convoys });
    }
    Ok(out)
}

// ─── 装备加载 ───────────────────────────────────────────────────────

fn load_equipment(paths: &PathConfig) -> Result<HashMap<String, EquipmentDef>, String> {
    let dirs = data_dirs(paths, "common/units/equipment");
    let mut out = HashMap::new();
    if dirs.is_empty() {
        return Ok(out);
    }
    for dir in dirs {
        for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.path().extension().map_or(false, |e| e == "txt") {
                continue;
            }
            let content = fs::read_to_string(entry.path()).unwrap_or_default();
            let block = parse(&content);
            let eq_block = match block.get_block("equipments") {
                Some(b) => b,
                None => continue,
            };
            // 第一遍：收集所有定义
            let mut local: Vec<EquipmentDef> = Vec::new();
            for e in &eq_block.entries {
                let Value::Block(b) = &e.value else { continue };
                let key = e.key.clone();
                let is_archetype = b.get_bool("is_archetype").unwrap_or(false);
                let archetype = b.get_string("archetype").map(|s| s.to_owned());
                let parent = b.get_string("parent").map(|s| s.to_owned());
                let is_buildable = b.get_bool("is_buildable").unwrap_or(true);
                let category_str = b.get_string("type").unwrap_or("");
                let category = EquipmentCategory::from_str(category_str);
                let year = b.get_int("year").unwrap_or(1936) as u16;
                let build_cost_ic = b.get_float("build_cost_ic").unwrap_or(0.0) as f32;

                let mut resources = HashMap::new();
                if let Some(rb) = b.get_block("resources") {
                    for re in &rb.entries {
                        let amount = match &re.value {
                            Value::Integer(i) => *i as f32,
                            Value::Float(f) => *f as f32,
                            _ => continue,
                        };
                        resources.insert(re.key.clone(), amount);
                    }
                }

                local.push(EquipmentDef {
                    key,
                    is_archetype,
                    archetype,
                    parent,
                    category,
                    year,
                    is_buildable,
                    build_cost_ic,
                    resources,
                });
            }
            // 第二遍：从原型继承缺失的字段
            let archetype_index: HashMap<String, usize> = local
                .iter()
                .enumerate()
                .filter(|(_, d)| d.is_archetype)
                .map(|(i, d)| (d.key.clone(), i))
                .collect();
            for i in 0..local.len() {
                if local[i].is_archetype {
                    continue;
                }
                let arch_key = local[i].archetype.clone();
                let Some(arch_key) = arch_key else { continue };
                let Some(&arch_idx) = archetype_index.get(&arch_key) else {
                    continue;
                };
                // 拷贝原型的值；同时合并 resources
                if local[i].build_cost_ic == 0.0 {
                    local[i].build_cost_ic = local[arch_idx].build_cost_ic;
                }
                if matches!(local[i].category, EquipmentCategory::Other) {
                    local[i].category = local[arch_idx].category;
                }
                if local[i].resources.is_empty() {
                    local[i].resources = local[arch_idx].resources.clone();
                }
            }
            for d in local {
                out.insert(d.key.clone(), d);
            }
        }
    }
    Ok(out)
}

// ─── 科技加载 ───────────────────────────────────────────────────────

fn load_technologies(paths: &PathConfig) -> Result<HashMap<String, Technology>, String> {
    let dirs = data_dirs(paths, "common/technologies");
    let mut out = HashMap::new();
    if dirs.is_empty() {
        return Ok(out);
    }
    for dir in dirs {
        for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.path().extension().map_or(false, |e| e == "txt") {
                continue;
            }
            let content = fs::read_to_string(entry.path()).unwrap_or_default();
            let block = parse(&content);
            let techs_block = match block.get_block("technologies") {
                Some(b) => b,
                None => continue,
            };

            for e in &techs_block.entries {
                // 跳过 `@1936 = 2` 这种位置常量声明（key 以 @ 开头）
                if e.key.starts_with('@') {
                    continue;
                }
                let Value::Block(b) = &e.value else { continue };

                let key = e.key.clone();
                let research_cost = b.get_float("research_cost").unwrap_or(1.0) as f32;
                let start_year = b.get_int("start_year").unwrap_or(1936) as u16;

                // path = { leads_to_tech = X research_cost_coeff = N } — 可能多条
                let mut paths = Vec::new();
                for path_val in b.get_all("path") {
                    if let Value::Block(pb) = path_val {
                        if let Some(leads_to) = pb.get_string("leads_to_tech") {
                            let coeff = pb.get_float("research_cost_coeff").unwrap_or(1.0) as f32;
                            paths.push(TechPath {
                                leads_to: leads_to.to_owned(),
                                research_cost_coeff: coeff,
                            });
                        }
                    }
                }

                // categories
                let mut categories = Vec::new();
                if let Some(cb) = b.get_block("categories") {
                    for v in &cb.values {
                        if let Value::String(s) = v {
                            categories.push(s.clone());
                        }
                    }
                    // categories 也可能用 entries 表达（doctrine 子分类）
                    for ce in &cb.entries {
                        categories.push(ce.key.clone());
                    }
                }

                // folder
                let folder = b
                    .get_block("folder")
                    .and_then(|f| f.get_string("name"))
                    .map(|s| s.to_owned());

                // doctrine 判断：folder 名包含 "doctrine"
                let is_doctrine = folder
                    .as_deref()
                    .map(|f| f.contains("doctrine"))
                    .unwrap_or(false)
                    || categories.iter().any(|c| c.contains("doctrine"));

                let enable_equipments = collect_string_list(b, "enable_equipments");
                let enable_subunits = collect_string_list(b, "enable_subunits");
                let enable_equipment_modules = collect_string_list(b, "enable_equipment_modules");
                let enable_building = b
                    .get_block("enable_building")
                    .map(|eb| eb.entries.iter().map(|e| e.key.clone()).collect::<Vec<_>>())
                    .unwrap_or_default();

                out.insert(
                    key.clone(),
                    Technology {
                        key,
                        research_cost,
                        start_year,
                        paths,
                        categories,
                        folder,
                        enable_equipments,
                        enable_subunits,
                        enable_equipment_modules,
                        enable_building,
                        is_doctrine,
                    },
                );
            }
        }
    }
    Ok(out)
}

fn collect_string_list(b: &clausewitz_parser::parser::Block, key: &str) -> Vec<String> {
    b.get_block(key)
        .map(|inner| {
            inner
                .values
                .iter()
                .filter_map(|v| match v {
                    Value::String(s) => Some(s.clone()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// 反向索引：tech_key → 它的前置（直接父）列表
fn build_tech_prereqs(techs: &HashMap<String, Technology>) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for tech in techs.values() {
        for p in &tech.paths {
            out.entry(p.leads_to.clone())
                .or_default()
                .push(tech.key.clone());
        }
    }
    out
}

// ─── 政治：意识形态加载 ─────────────────────────────────────────────

fn load_ideologies(paths: &PathConfig) -> Result<HashMap<String, Ideology>, String> {
    let Some(path) = paths.find("common/ideologies/00_ideologies.txt") else {
        return Ok(HashMap::new());
    };
    let content = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return Ok(HashMap::new()),
    };
    let block = parse(&content);
    let ideologies_block = match block.get_block("ideologies") {
        Some(b) => b,
        None => return Ok(HashMap::new()),
    };

    let mut out = HashMap::new();
    for e in &ideologies_block.entries {
        let Value::Block(b) = &e.value else { continue };
        let key = e.key.clone();
        let mut types = Vec::new();
        if let Some(types_block) = b.get_block("types") {
            for te in &types_block.entries {
                types.push(te.key.clone());
            }
        }
        // color = { R G B }
        let mut color = [128, 128, 128];
        if let Some(Value::Block(cb)) = b.get("color") {
            let vals: Vec<i64> = cb
                .values
                .iter()
                .filter_map(|v| match v {
                    Value::Integer(i) => Some(*i),
                    _ => None,
                })
                .collect();
            if vals.len() >= 3 {
                color = [
                    vals[0].clamp(0, 255) as u8,
                    vals[1].clamp(0, 255) as u8,
                    vals[2].clamp(0, 255) as u8,
                ];
            }
        }
        out.insert(key.clone(), Ideology { key, types, color });
    }
    Ok(out)
}

// ─── 政治：国家精神加载 ─────────────────────────────────────────────

fn load_ideas(paths: &PathConfig) -> Result<HashMap<String, IdeaDef>, String> {
    let dirs = data_dirs(paths, "common/ideas");
    let mut out = HashMap::new();
    if dirs.is_empty() {
        return Ok(out);
    }
    for dir in dirs {
        for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.path().extension().map_or(false, |e| e == "txt") {
                continue;
            }
            let content = fs::read_to_string(entry.path()).unwrap_or_default();
            let block = parse(&content);
            let ideas_block = match block.get_block("ideas") {
                Some(b) => b,
                None => continue,
            };
            // 顶层有若干 category（country / political_advisor / theorist / mobilisation_laws…）
            for cat_entry in &ideas_block.entries {
                let Value::Block(cat_block) = &cat_entry.value else {
                    continue;
                };
                let category = cat_entry.key.clone();
                // category 下可能直接是 idea，也可能是 group
                for idea_entry in &cat_block.entries {
                    let Value::Block(idea_block) = &idea_entry.value else {
                        continue;
                    };
                    // 跳过 `law = yes` 等元数据
                    if matches!(
                        idea_block.get(idea_entry.key.as_str()),
                        Some(Value::Bool(_))
                    ) {
                        continue;
                    }

                    let key = idea_entry.key.clone();
                    let removal_cost = idea_block.get_int("removal_cost").unwrap_or(0) as i32;
                    let picture = idea_block.get_string("picture").map(str::to_owned);

                    let mut modifiers = HashMap::new();
                    if let Some(mb) = idea_block.get_block("modifier") {
                        for me in &mb.entries {
                            let val = match &me.value {
                                Value::Float(f) => *f as f32,
                                Value::Integer(i) => *i as f32,
                                _ => continue,
                            };
                            modifiers.insert(me.key.clone(), val);
                        }
                    }

                    let mut rules = Vec::new();
                    if let Some(rb) = idea_block.get_block("rule") {
                        for re in &rb.entries {
                            rules.push(re.key.clone());
                        }
                    }

                    out.insert(
                        key.clone(),
                        IdeaDef {
                            key,
                            category: category.clone(),
                            removal_cost,
                            picture,
                            modifiers,
                            rules,
                        },
                    );
                }
            }
        }
    }
    Ok(out)
}

fn inject_sino_japanese_war_ideas(ideas: &mut HashMap<String, IdeaDef>) {
    let defs = [
        (
            "chi_united_front_coordination_difficulties",
            "GFX_idea_chinese_united_front",
            [
                ("army_attack_factor", -0.12),
                ("army_organisation_factor", -0.06),
                ("reinforce_rate", -0.04),
            ],
        ),
        (
            "chi_regional_command_autonomy",
            "GFX_idea_chinese_warlord_autonomy",
            [
                ("army_attack_factor", -0.08),
                ("army_organisation_factor", -0.04),
                ("reinforce_rate", -0.03),
            ],
        ),
        (
            "chi_theater_command_delay",
            "GFX_idea_chinese_defense",
            [
                ("army_attack_factor", -0.06),
                ("army_defence_factor", -0.03),
                ("reinforce_rate", -0.04),
            ],
        ),
        (
            "chi_national_war_mobilization",
            "GFX_idea_chinese_united_front",
            [
                ("army_defence_factor", 0.08),
                ("army_organisation_factor", 0.04),
                ("war_support_factor", 0.05),
            ],
        ),
        (
            "chi_protracted_war_policy",
            "GFX_idea_chinese_defense",
            [
                ("army_defence_factor", 0.10),
                ("army_organisation_factor", 0.06),
                ("attrition", -0.04),
            ],
        ),
        (
            "chi_rear_industry_relocation",
            "GFX_idea_chinese_industry_relocation",
            [
                ("production_speed_buildings_factor", 0.05),
                ("consumer_goods_factor", -0.02),
                ("research_speed_factor", 0.02),
            ],
        ),
        (
            "jap_continental_offensive_momentum",
            "GFX_idea_japanese_offensive",
            [
                ("army_attack_factor", 0.08),
                ("army_organisation_factor", 0.03),
                ("army_org_regain", 0.04),
            ],
        ),
        (
            "jap_north_china_expeditionary_expansion",
            "GFX_idea_japanese_army",
            [
                ("army_attack_factor", 0.04),
                ("reinforce_rate", 0.03),
                ("front_demand_factor", 0.08),
            ],
        ),
        (
            "jap_china_incident_expansion",
            "GFX_idea_japanese_militarism",
            [
                ("war_support_factor", 0.04),
                ("front_demand_factor", 0.06),
                ("army_organisation_factor", 0.02),
            ],
        ),
        (
            "jap_occupation_security_pressure",
            "GFX_idea_japanese_occupation",
            [
                ("army_attack_factor", -0.04),
                ("army_organisation_factor", -0.02),
                ("attrition", 0.03),
            ],
        ),
        (
            "jap_extended_continental_supply_lines",
            "GFX_idea_japanese_supply",
            [
                ("army_org_regain", -0.05),
                ("army_organisation_factor", -0.03),
                ("attrition", 0.04),
            ],
        ),
        (
            "jap_forces_dispersed_in_china",
            "GFX_idea_japanese_garrison",
            [
                ("army_attack_factor", -0.06),
                ("army_defence_factor", -0.02),
                ("front_demand_factor", -0.05),
            ],
        ),
    ];

    for (key, picture, modifiers) in defs {
        let modifiers = modifiers
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect();
        ideas.insert(
            key.to_owned(),
            IdeaDef {
                key: key.to_owned(),
                category: "country".to_owned(),
                removal_cost: -1,
                picture: Some(picture.to_owned()),
                modifiers,
                rules: Vec::new(),
            },
        );
    }
}

// ─── 政治：国策树加载 ──────────────────────────────────────────────

// V5 收口（2026-05-18，§3.4）：vanilla `vanilla-focus-dir-removed/*.txt` 加载已停用。
// 未来 GER 自研 focus 树走 RON schema（阶段 D）；此处保留同名 stub 让 `GameData::load`
// 调用点不爆。
fn load_focus_trees(_game_path: &Path) -> Result<HashMap<String, FocusTree>, String> {
    Ok(HashMap::new())
}

fn build_focus_index(_trees: &HashMap<String, FocusTree>) -> HashMap<String, String> {
    HashMap::new()
}

// ─── 军事：子单位（兵种） ──────────────────────────────────────────

fn load_subunits(paths: &PathConfig) -> Result<HashMap<String, SubunitDef>, String> {
    let dirs = data_dirs(paths, "common/units");
    let mut out = HashMap::new();
    if dirs.is_empty() {
        return Ok(out);
    }
    for dir in dirs {
        for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.path().is_dir() {
                continue;
            }
            if !entry.path().extension().map_or(false, |e| e == "txt") {
                continue;
            }
            let content = fs::read_to_string(entry.path()).unwrap_or_default();
            let block = parse(&content);
            let sub_block = match block.get_block("sub_units") {
                Some(b) => b,
                None => continue,
            };
            for e in &sub_block.entries {
                let Value::Block(b) = &e.value else { continue };
                let key = e.key.clone();

                let mut types = Vec::new();
                if let Some(tb) = b.get_block("type") {
                    for v in &tb.values {
                        if let Value::String(s) = v {
                            types.push(s.clone());
                        }
                    }
                    for ee in &tb.entries {
                        types.push(ee.key.clone());
                    }
                }

                let mut categories = Vec::new();
                if let Some(cb) = b.get_block("categories") {
                    for v in &cb.values {
                        if let Value::String(s) = v {
                            categories.push(s.clone());
                        }
                    }
                }

                let mut need = HashMap::new();
                if let Some(nb) = b.get_block("need") {
                    for ne in &nb.entries {
                        let v = match &ne.value {
                            Value::Integer(i) => *i as u32,
                            Value::Float(f) => *f as u32,
                            _ => continue,
                        };
                        need.insert(ne.key.clone(), v);
                    }
                }

                out.insert(
                    key.clone(),
                    SubunitDef {
                        key,
                        abbreviation: b.get_string("abbreviation").unwrap_or("").to_owned(),
                        group: b.get_string("group").unwrap_or("").to_owned(),
                        types,
                        combat_width: b.get_float("combat_width").unwrap_or(2.0) as f32,
                        soft_attack: b.get_float("soft_attack").unwrap_or(0.0) as f32,
                        hard_attack: b.get_float("hard_attack").unwrap_or(0.0) as f32,
                        defense: b.get_float("defense").unwrap_or(0.0) as f32,
                        breakthrough: b.get_float("breakthrough").unwrap_or(0.0) as f32,
                        armor_value: b.get_float("armor_value").unwrap_or(0.0) as f32,
                        ap_attack: b.get_float("ap_attack").unwrap_or(0.0) as f32,
                        hardness: b.get_float("hardness").unwrap_or(0.0) as f32,
                        max_strength: b.get_float("max_strength").unwrap_or(25.0) as f32,
                        max_organisation: b.get_float("max_organisation").unwrap_or(60.0) as f32,
                        default_morale: b.get_float("default_morale").unwrap_or(0.3) as f32,
                        manpower: b.get_int("manpower").unwrap_or(1000) as u32,
                        training_time: b.get_int("training_time").unwrap_or(90) as u32,
                        suppression: b.get_float("suppression").unwrap_or(0.0) as f32,
                        supply_consumption: b.get_float("supply_consumption").unwrap_or(0.06)
                            as f32,
                        weight: b.get_float("weight").unwrap_or(0.5) as f32,
                        need,
                        categories,
                    },
                );
            }
        }
    }
    Ok(out)
}

// ─── 军事：战斗战术 ────────────────────────────────────────────────

fn load_combat_tactics(paths: &PathConfig) -> Result<HashMap<String, CombatTactic>, String> {
    let Some(path) = paths.find("common/combat_tactics.txt") else {
        return Ok(HashMap::new());
    };
    let content = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return Ok(HashMap::new()),
    };
    let block = parse(&content);

    let mut out = HashMap::new();
    // 顶层是若干 entries：tactic_basic_attack = { ... }
    // 跳过 phases = { ... } 等
    for e in &block.entries {
        if !e.key.starts_with("tactic_") {
            continue;
        }
        let Value::Block(b) = &e.value else { continue };

        let key = e.key.clone();
        let is_attacker = b.get_bool("is_attacker").unwrap_or(false);
        let attacker_bonus = b.get_float("attacker").unwrap_or(0.0) as f32;
        let defender_bonus = b.get_float("defender").unwrap_or(0.0) as f32;
        let movement_bonus = b.get_float("movement").unwrap_or(0.0) as f32;
        // base = { factor = N }
        let base_weight = b
            .get_block("base")
            .and_then(|bb| bb.get_float("factor"))
            .unwrap_or(1.0) as f32;

        let countered_by: Vec<String> = b
            .get_string("countered_by")
            .map(|s| vec![s.to_owned()])
            .unwrap_or_default();

        out.insert(
            key.clone(),
            CombatTactic {
                key,
                is_attacker,
                base_weight,
                attacker_bonus,
                defender_bonus,
                movement_bonus,
                countered_by,
            },
        );
    }
    Ok(out)
}

// ─── 军事：师编制模板 ──────────────────────────────────────────────

fn load_division_templates(
    paths: &PathConfig,
) -> Result<HashMap<String, Vec<DivisionTemplate>>, String> {
    let dirs = data_dirs(paths, "history/units");
    let mut out: HashMap<String, Vec<DivisionTemplate>> = HashMap::new();
    if dirs.is_empty() {
        return Ok(out);
    }

    for dir in dirs {
        for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if !path.extension().map_or(false, |e| e == "txt") {
                continue;
            }
            // 文件名形如 GER_1936.txt / SOV_1939_naval.txt — 跳过 air/naval/air_bba 等
            let file_stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_owned();
            // 仅取 1936 陆军模板：包含 _1936 但不含 air/naval
            if !file_stem.contains("_1936") {
                continue;
            }
            if file_stem.contains("air") || file_stem.contains("naval") {
                continue;
            }
            let tag = file_stem.split('_').next().unwrap_or("").to_owned();
            if tag.len() != 3 {
                continue;
            }

            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            if content.len() > 2_000_000 {
                continue;
            }
            let block = parse(&content);

            let mut templates = Vec::new();
            for tmpl_val in block.get_all("division_template") {
                let Value::Block(tb) = tmpl_val else { continue };
                let name = tb.get_string("name").unwrap_or("Unknown").to_owned();
                let division_names_group =
                    tb.get_string("division_names_group").map(|s| s.to_owned());

                let mut regiments = Vec::new();
                if let Some(rb) = tb.get_block("regiments") {
                    for re in &rb.entries {
                        // entry 是 `infantry = { x = 0 y = 0 }`
                        regiments.push(re.key.clone());
                    }
                }

                let mut support = Vec::new();
                if let Some(sb) = tb.get_block("support") {
                    for se in &sb.entries {
                        support.push(se.key.clone());
                    }
                }

                templates.push(DivisionTemplate {
                    name,
                    country_tag: Some(tag.clone()),
                    regiments,
                    support,
                    division_names_group,
                });
            }

            if !templates.is_empty() {
                // 同一国可能有 _1936 + _1936_nsb (DLC)；以先到者为准
                out.entry(tag).or_default().extend(templates);
            }
        }
    }
    Ok(out)
}

// ─── 海军：舰类加载 ────────────────────────────────────────────────

fn load_ship_classes(paths: &PathConfig) -> Result<HashMap<String, ShipClassDef>, String> {
    // 检测原版 sub_units 中存在哪些 ship class，再用 baseline_for_class 给属性
    let candidate_files = [
        "destroyer.txt",
        "light_cruiser.txt",
        "heavy_cruiser.txt",
        "battlecruiser.txt",
        "battleship.txt",
        "carrier.txt",
        // submarine 和 convoy 的 sub_unit 在另外的 equipment 文件里 —
        // 我们直接按 key 加 baseline，即使 vanilla 中文件不存在
    ];
    let dirs = data_dirs(paths, "common/units");
    let mut existing = std::collections::HashSet::<String>::new();
    for dir in dirs {
        for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.path().is_dir() {
                continue;
            }
            let Some(stem) = entry
                .path()
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_owned())
            else {
                continue;
            };
            if candidate_files
                .iter()
                .any(|c| c.trim_end_matches(".txt") == stem)
            {
                let content = fs::read_to_string(entry.path()).unwrap_or_default();
                let block = parse(&content);
                if let Some(sb) = block.get_block("sub_units") {
                    for e in &sb.entries {
                        existing.insert(e.key.clone());
                    }
                }
            }
        }
    }

    let mut out = HashMap::new();
    // 先基于检测到的 vanilla class
    for k in existing {
        if let Some(def) = baseline_for_class(&k) {
            out.insert(k, def);
        }
    }
    // 始终包含 submarine 和 convoy（HOI4 这两个在不同地方定义；为简化我们硬编码）
    for k in ["submarine", "convoy"] {
        if !out.contains_key(k) {
            if let Some(def) = baseline_for_class(k) {
                out.insert(k.to_owned(), def);
            }
        }
    }
    Ok(out)
}

// ─── 空军：飞机加载 ────────────────────────────────────────────────

fn load_aircraft(paths: &PathConfig) -> Result<HashMap<String, AircraftDef>, String> {
    // 检测原版 sub_units / equipment 中存在哪些 air class，再注入 baseline。
    // HOI4 中飞机 sub_unit 文件名形如 `fighter.txt` / `cas.txt` / `strategic_bomber.txt`。
    // 与 ship 不同，没有 modules，所以本身字段就有 air_attack 等 — 但我们继续用
    // baseline 以保证语义一致 + 简化（即使 vanilla 文件不存在也能跑）。
    let candidate_files = [
        "fighter.txt",
        "heavy_fighter.txt",
        "cas.txt",
        "tactical_bomber.txt",
        "strategic_bomber.txt",
        "naval_bomber.txt",
        "transport_plane.txt",
    ];
    let dirs = data_dirs(paths, "common/units");
    let mut existing = std::collections::HashSet::<String>::new();
    for dir in dirs {
        for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.path().is_dir() {
                continue;
            }
            let Some(stem) = entry
                .path()
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_owned())
            else {
                continue;
            };
            if candidate_files
                .iter()
                .any(|c| c.trim_end_matches(".txt") == stem)
            {
                let content = fs::read_to_string(entry.path()).unwrap_or_default();
                let block = parse(&content);
                if let Some(sb) = block.get_block("sub_units") {
                    for e in &sb.entries {
                        existing.insert(e.key.clone());
                    }
                }
            }
        }
    }

    let mut out = HashMap::new();
    for k in existing {
        if let Some(def) = baseline_for_aircraft(&k) {
            out.insert(k, def);
        }
    }
    // 始终保证 5 个核心型号存在（即使 vanilla 文件加载失败）
    for k in [
        "fighter",
        "cas",
        "tactical_bomber",
        "strategic_bomber",
        "naval_bomber",
    ] {
        if !out.contains_key(k) {
            if let Some(def) = baseline_for_aircraft(k) {
                out.insert(k.to_owned(), def);
            }
        }
    }
    Ok(out)
}

// V5 收口（2026-05-18，§3.4）：vanilla `vanilla-decisions-dir-removed/{,categories}/*.txt` 加载已停用。
// `crates/hoi4-script/src/decisions.rs` 保留为骨架，等待自研 RON 决议接入（阶段 F.2）。
fn load_decision_categories(
    _game_path: &Path,
) -> Result<HashMap<String, DecisionCategoryDef>, String> {
    Ok(HashMap::new())
}

// ─── J.3: State categories ─────────────────────────────────────────

fn load_state_categories(paths: &PathConfig) -> Result<HashMap<String, StateCategoryDef>, String> {
    let dirs = data_dirs(paths, "common/state_category");
    let mut out = HashMap::new();
    if dirs.is_empty() {
        return Ok(out);
    }
    for dir in dirs {
        for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.path().extension().map_or(false, |e| e == "txt") {
                continue;
            }
            let content = fs::read_to_string(entry.path()).unwrap_or_default();
            let block = parse(&content);
            let cats_block = match block.get_block("state_categories") {
                Some(b) => b,
                None => continue,
            };
            for e in &cats_block.entries {
                let Value::Block(b) = &e.value else { continue };
                let slots = b.get_int("local_building_slots").unwrap_or(0) as u8;
                out.insert(
                    e.key.clone(),
                    StateCategoryDef {
                        name: e.key.clone(),
                        local_building_slots: slots,
                    },
                );
            }
        }
    }
    Ok(out)
}

// ─── J.1.3：本地化解析（parties + characters） ──────────────────────
//
// vanilla `localisation/english/*.yml` 不是标准 YAML：它是 Paradox 自定义格式，
// 大致形如：
//
// ```text
//     l_english:
//      GER_fascism_party:0 "NSDAP"
//      GER_fascism_party_long:0 "Nationalsozialistische Deutsche Arbeiterpartei"
//     ```
//
// 每行 1 条。前导空格 + key `:<version>` + 引号字符串。version 数字可省略。
// 文件顶部可能含 UTF-8 BOM。我们用一个最小行解析器手解，避免引入 yaml crate。

/// 解析 `parties_l_english.yml` 进 `(loc_key → value)`。
fn load_party_names(paths: &PathConfig) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let Some(path) = paths.find("localisation/english/parties_l_english.yml") else {
        return out;
    };
    let Ok(text) = fs::read_to_string(&path) else {
        return out;
    };
    parse_paradox_yml_into(&text, &mut out);
    out
}

/// 扫描 `localisation/english/*characters*l_english*.yml` 全部文件，合并为一个 map。
/// vanilla DLC 把 character 名分散在 `nsb_characters_l_english.yml` /
/// `aat_characters_l_english.yml` 等多文件中。
fn load_character_names(paths: &PathConfig) -> HashMap<String, String> {
    let dirs = data_dirs(paths, "localisation/english");
    let mut out = HashMap::new();
    if dirs.is_empty() {
        return out;
    };
    for dir in dirs {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            // 收 `*characters*l_english*.yml`
            let lower = name.to_ascii_lowercase();
            if !lower.ends_with(".yml") {
                continue;
            }
            if !lower.contains("characters") {
                continue;
            }
            if !lower.contains("l_english") {
                continue;
            }
            if let Ok(text) = fs::read_to_string(&path) {
                parse_paradox_yml_into(&text, &mut out);
            }
        }
    }
    out
}

/// Paradox `.yml` 行解析器：把 `key:version "value"` 形式的行写入 `out`。
///
/// - 跳过 `l_english:` 头、空行、`#` 注释行。
/// - 自动处理 UTF-8 BOM。
/// - 已存在的 key 会被覆盖（DLC 顺序加载时的预期行为）。
fn parse_paradox_yml_into(text: &str, out: &mut HashMap<String, String>) {
    // strip BOM
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    for raw_line in text.lines() {
        let line = raw_line.trim_start();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        if line.starts_with("l_") {
            // l_english: 等头部
            continue;
        }
        // key:version "value"  或  key: "value"
        let Some(colon) = line.find(':') else {
            continue;
        };
        let key = &line[..colon];
        if key.is_empty() {
            continue;
        }
        let after = &line[colon + 1..];
        // 跳过可选 version 数字
        let after = after.trim_start_matches(|c: char| c.is_ascii_digit());
        // 找首个引号
        let Some(q1) = after.find('"') else {
            continue;
        };
        let rest = &after[q1 + 1..];
        // 找末尾引号；vanilla 偶有内嵌 `\"`，简化用 rfind
        let Some(q2) = rest.rfind('"') else {
            continue;
        };
        let value = &rest[..q2];
        out.insert(key.to_owned(), value.to_owned());
    }
}

#[cfg(test)]
mod loader_yml_tests {
    use super::*;
    use hoi4_paths::ModEntry;

    fn reset_temp(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(name);
        let _ = fs::remove_dir_all(&root);
        root
    }

    #[test]
    fn paradox_yml_basic() {
        let txt = "\u{feff}l_english:\n GER_fascism_party:0 \"NSDAP\"\n \
                   GER_fascism_party_long:0 \"Nationalsozialistische Deutsche Arbeiterpartei\"\n \
                   # comment\n  empty:0\n SOV_iosif_stalin:0 \"Iosif Stalin\"\n";
        let mut out = HashMap::new();
        parse_paradox_yml_into(txt, &mut out);
        assert_eq!(
            out.get("GER_fascism_party").map(String::as_str),
            Some("NSDAP")
        );
        assert_eq!(
            out.get("GER_fascism_party_long").map(String::as_str),
            Some("Nationalsozialistische Deutsche Arbeiterpartei")
        );
        assert_eq!(
            out.get("SOV_iosif_stalin").map(String::as_str),
            Some("Iosif Stalin")
        );
        // 空 value 跳过
        assert!(!out.contains_key("empty"));
    }

    #[test]
    fn paradox_yml_no_version() {
        let txt = " GER_fascism_party_long: \"Nationalsozialistische Deutsche Arbeiterpartei\"\n";
        let mut out = HashMap::new();
        parse_paradox_yml_into(txt, &mut out);
        assert_eq!(
            out.get("GER_fascism_party_long").map(String::as_str),
            Some("Nationalsozialistische Deutsche Arbeiterpartei")
        );
    }

    #[test]
    fn load_buildings_merges_vanilla_then_mod_override() {
        let root = reset_temp("ironheart_data_loader_mod_override");
        let game = root.join("game");
        let mod_a = root.join("mod_a");
        fs::create_dir_all(game.join("common/buildings")).unwrap();
        fs::create_dir_all(mod_a.join("common/buildings")).unwrap();
        fs::write(
            game.join("common/buildings/00_buildings.txt"),
            "buildings = { industrial_complex = { base_cost = 100 general_production = 5 } }",
        )
        .unwrap();
        fs::write(
            mod_a.join("common/buildings/00_buildings.txt"),
            "buildings = { industrial_complex = { base_cost = 20 general_production = 9 } }",
        )
        .unwrap();

        let cfg = PathConfig::with_game_path(&game).with_mods(vec![ModEntry {
            name: "override".to_owned(),
            root: mod_a,
            replace_paths: vec![],
        }]);

        let buildings = load_buildings(&cfg).unwrap();
        let industrial = buildings.get("industrial_complex").unwrap();
        assert_eq!(industrial.base_cost, 20.0);
    }

    #[test]
    fn load_buildings_replace_path_blocks_vanilla() {
        let root = reset_temp("ironheart_data_loader_replace_path");
        let game = root.join("game");
        let mod_a = root.join("mod_a");
        fs::create_dir_all(game.join("common/buildings")).unwrap();
        fs::create_dir_all(&mod_a).unwrap();
        fs::write(
            game.join("common/buildings/00_buildings.txt"),
            "buildings = { industrial_complex = { base_cost = 100 general_production = 5 } }",
        )
        .unwrap();

        let cfg = PathConfig::with_game_path(&game).with_mods(vec![ModEntry {
            name: "replace".to_owned(),
            root: mod_a,
            replace_paths: vec![PathBuf::from("common/buildings")],
        }]);

        let buildings = load_buildings(&cfg).unwrap();
        assert!(!buildings.contains_key("industrial_complex"));
        assert!(buildings.contains_key("dockyard"));
    }
}
