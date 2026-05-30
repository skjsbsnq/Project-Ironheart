//! Phase 1.1：history/* 加载。
//!
//! 解析三组与"1936-01-01 启动状态"相关的脚本文件：
//!
//! 1. `history/units/<NAME>.txt` ——
//!    陆军 OOB（`units = { division = { ... } }`）。返回 [`OobLand`]。
//! 2. `history/units/<NAME>_naval[_mtg].txt` ——
//!    海军 OOB（`units = { fleet = { task_force = { ship = ... } } }`）。返回 [`OobNaval`]。
//! 3. `history/units/<NAME>_air_legacy.txt` / `_air_bba.txt` ——
//!    空军 OOB（`air_wings = { <state_id> = { fighter_equipment_0 = { ... } name = "..." } }`）。
//!    返回 [`OobAir`]。
//! 4. `history/countries/<TAG> - <Name>.txt` ——
//!    国家初始 ideas / focus / variables / flags / set_oob 引用 等。返回 [`CountryHistory`]。
//!
//! ## 选择性跳过：脚本块 `if`
//! vanilla 在国家文件大量使用 `if = { limit = { date >= "1939.x" } ... }`、
//! `if = { limit = { has_dlc = "..." } ... }`。Phase 1.1 没有完整的 trigger
//! 引擎，所以采用这条规则：
//!
//! * 顶层语句 → 收集。
//! * `if` 块的 `limit` 含 `date >= ...` → 跳过（属于"未来事件"，留给脚本引擎）。
//! * 其它 `if` 块 → **递归**进入，仍按本规则处理（不检查 has_dlc / has_country_flag /
//!   etc.；保守地全部接受，因为 1936 启动时这些条件大多是初始为假的）。
//!
//! 这是一个保守近似：宁可多加几个不该加的初始 idea，也不要漏掉关键的初始 set_oob。

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use clausewitz_parser::{parse, Block, Value};
use hoi4_paths::PathConfig;

/// 一个师在 OOB 中的描述（位置 + 模板名 + 起始经验等）。
#[derive(Debug, Clone)]
pub struct DivisionInstance {
    /// 师名（来自 `division_name` 块）。`name_order` 转字符串，待 World 应用时
    /// 与模板的 `division_names_group` 拼成"3. Infanterie-Division"等真实名称。
    pub name_order: Option<u32>,
    /// 模板名（必须在 `data.division_templates[tag]` 中存在）
    pub template_name: String,
    /// 部署省份
    pub location: u16,
    /// 起始经验因子（0..1）
    pub start_experience_factor: f32,
    /// 起始装备因子（0..1）。vanilla 多数省略时按 1.0
    pub start_equipment_factor: Option<f32>,
}

/// 一个 OOB 文件解析后的陆军组件。
#[derive(Debug, Clone, Default)]
pub struct OobLand {
    /// OOB 名（去掉 `.txt` 后缀的文件名 stem）
    pub name: String,
    /// 师列表，按文件中出现顺序
    pub divisions: Vec<DivisionInstance>,
}

/// 海军舰队中的一艘舰。
#[derive(Debug, Clone)]
pub struct ShipSpec {
    pub name: String,
    /// 类别 key（"destroyer" / "battleship" / "submarine" / ...）
    pub definition: String,
    /// 装备 key（"destroyer_1" / "battleship_1" / ...）；用于将来挂装备
    pub equipment_key: Option<String>,
    pub pride_of_the_fleet: bool,
    pub owner_tag: String,
}

/// 一个 task_force：分舰队，挂在 fleet 下。
#[derive(Debug, Clone)]
pub struct TaskForceSpec {
    pub name: String,
    /// 港口 / 海域 province id
    pub location: u16,
    pub ships: Vec<ShipSpec>,
}

/// 一个 fleet：编队总名 + 多个 task_force。
#[derive(Debug, Clone)]
pub struct FleetSpec {
    pub name: String,
    /// 母港 province id
    pub naval_base: Option<u16>,
    pub task_forces: Vec<TaskForceSpec>,
}

#[derive(Debug, Clone, Default)]
pub struct OobNaval {
    pub name: String,
    pub fleets: Vec<FleetSpec>,
}

/// 一个空军条目：某 state 上的某飞机型号 + 数量。一个 state 可有多个条目（fighter +
/// bomber 等），所以最终是 `Vec<AirWingEntry>`。
#[derive(Debug, Clone)]
pub struct AirWingEntry {
    /// 部署 state 游戏 ID
    pub state_id: u16,
    /// 装备 key（"fighter_equipment_0" / "tac_bomber_equipment_0" / "CAS_equipment_1" / ...）
    pub equipment_key: String,
    /// 飞机数
    pub amount: u32,
    pub owner_tag: String,
    /// 联队名（同一 state 内多个条目共享一个 name；vanilla 可能在每个 sub-block 之后
    /// 写一行 name = "..."；我们捕获最近的一行）
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct OobAir {
    pub name: String,
    pub wings: Vec<AirWingEntry>,
}

/// 1936-01-01 国家初始静态状态。
///
/// 不含完整脚本评估（focus 完成时机 / DLC 条件 / 战争状态等），只把"启动时立即应用"
/// 的字段提取出来，由 [`crate::GameData::populate_country_histories`] 写回 World。
#[derive(Debug, Clone, Default)]
pub struct CountryHistory {
    pub tag: String,
    /// `add_ideas = { idea1 idea2 }` / `add_ideas = idea1` 都收集
    pub initial_ideas: Vec<String>,
    /// `set_country_flag = X`
    pub initial_flags: Vec<String>,
    /// `set_variable = { x = 1.0 }` 提取的 (name, value)
    pub initial_variables: Vec<(String, f32)>,
    /// `set_oob = "..."` —— 第一个匹配（陆军 OOB 引用）
    pub set_oob: Option<String>,
    /// `set_naval_oob = "..."`
    pub set_naval_oob: Option<String>,
    /// `set_air_oob = "..."`
    pub set_air_oob: Option<String>,
    /// `complete_national_focus = X`
    pub completed_focuses: Vec<String>,
    /// `unlock_national_focus = X`
    pub unlocked_focuses: Vec<String>,
    /// `set_research_slots = N`
    pub research_slots: Option<u8>,
    /// `set_stability = 0.5` （0..1）
    pub stability: Option<f32>,
    /// `set_war_support = 0.5`
    pub war_support: Option<f32>,
    /// `set_politics = { ruling_party = ... }` 中的 ruling_party 字段
    pub set_ruling_party: Option<String>,
    /// `set_popularities = { democratic = 50 ... }` 百分比 → 0..1
    pub party_popularities: HashMap<String, f32>,
    /// `recruit_character = TAG_name` in initial history order.
    ///
    /// Vanilla country leader selection depends on this order when several
    /// recruited country leaders match the same top-level ideology.
    pub recruited_characters: Vec<String>,
}

// ─── Loader ────────────────────────────────────────────────────────────

/// 解析 `history/units/*.txt` 中的陆军 OOB（不含 _naval / _air）。
pub fn load_oob_land(game_path: &Path) -> Result<HashMap<String, OobLand>, String> {
    load_oob_land_from_paths(&PathConfig::with_game_path(game_path))
}

pub fn load_oob_land_from_paths(paths: &PathConfig) -> Result<HashMap<String, OobLand>, String> {
    let dirs = data_dirs(paths, "history/units");
    let mut out = HashMap::new();
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
            let stem = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s.to_owned(),
                None => continue,
            };
            if stem.contains("_naval") || stem.contains("_air") {
                continue;
            }
            let content = fs::read_to_string(&path).unwrap_or_default();
            let block = parse(&content);
            let mut oob = OobLand {
                name: stem.clone(),
                divisions: Vec::new(),
            };
            if let Some(units) = block.get_block("units") {
                for e in &units.entries {
                    if e.key != "division" {
                        continue;
                    }
                    let Value::Block(b) = &e.value else { continue };
                    if let Some(d) = parse_division_instance(b) {
                        oob.divisions.push(d);
                    }
                }
            }
            if !oob.divisions.is_empty() {
                out.insert(stem, oob);
            }
        }
    }
    Ok(out)
}

fn parse_division_instance(b: &Block) -> Option<DivisionInstance> {
    let template_name = b.get_string("division_template")?.to_owned();
    let location = b.get_int("location")? as u16;
    let start_experience_factor = b.get_float("start_experience_factor").unwrap_or(0.0) as f32;
    let start_equipment_factor = b.get_float("start_equipment_factor").map(|v| v as f32);
    let name_order = b
        .get_block("division_name")
        .and_then(|nb| nb.get_int("name_order"))
        .map(|v| v as u32);
    Some(DivisionInstance {
        name_order,
        template_name,
        location,
        start_experience_factor,
        start_equipment_factor,
    })
}

// ─── Naval ────────────────────────────────────────────────────────────

/// 解析 `history/units/*_naval[_mtg].txt`。
pub fn load_oob_naval(game_path: &Path) -> Result<HashMap<String, OobNaval>, String> {
    load_oob_naval_from_paths(&PathConfig::with_game_path(game_path))
}

pub fn load_oob_naval_from_paths(paths: &PathConfig) -> Result<HashMap<String, OobNaval>, String> {
    let dirs = data_dirs(paths, "history/units");
    let mut out = HashMap::new();
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
            let stem = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s.to_owned(),
                None => continue,
            };
            if !stem.contains("_naval") {
                continue;
            }
            let content = fs::read_to_string(&path).unwrap_or_default();
            let block = parse(&content);
            let mut oob = OobNaval {
                name: stem.clone(),
                fleets: Vec::new(),
            };
            if let Some(units) = block.get_block("units") {
                for e in &units.entries {
                    if e.key != "fleet" {
                        continue;
                    }
                    let Value::Block(fb) = &e.value else { continue };
                    if let Some(fleet) = parse_fleet(fb) {
                        oob.fleets.push(fleet);
                    }
                }
            }
            if !oob.fleets.is_empty() {
                out.insert(stem, oob);
            }
        }
    }
    Ok(out)
}

fn parse_fleet(b: &Block) -> Option<FleetSpec> {
    let name = b.get_string("name").unwrap_or("Fleet").to_owned();
    let naval_base = b.get_int("naval_base").map(|v| v as u16);
    let mut task_forces = Vec::new();
    for e in &b.entries {
        if e.key != "task_force" {
            continue;
        }
        let Value::Block(tb) = &e.value else { continue };
        let tf_name = tb.get_string("name").unwrap_or("Task Force").to_owned();
        let tf_loc = tb.get_int("location").unwrap_or(0) as u16;
        let mut ships = Vec::new();
        for se in &tb.entries {
            if se.key != "ship" {
                continue;
            }
            let Value::Block(sb) = &se.value else {
                continue;
            };
            if let Some(s) = parse_ship(sb) {
                ships.push(s);
            }
        }
        task_forces.push(TaskForceSpec {
            name: tf_name,
            location: tf_loc,
            ships,
        });
    }
    Some(FleetSpec {
        name,
        naval_base,
        task_forces,
    })
}

fn parse_ship(b: &Block) -> Option<ShipSpec> {
    let name = b.get_string("name").unwrap_or("Ship").to_owned();
    let definition = b.get_string("definition")?.to_owned();
    let pride_of_the_fleet = b
        .get_string("pride_of_the_fleet")
        .map(|s| s == "yes")
        .or_else(|| b.get_bool("pride_of_the_fleet"))
        .unwrap_or(false);
    let mut equipment_key = None;
    let mut owner_tag = String::new();
    if let Some(eq) = b.get_block("equipment") {
        if let Some(first) = eq.entries.first() {
            equipment_key = Some(first.key.clone());
            if let Value::Block(eb) = &first.value {
                if let Some(o) = eb.get_string("owner") {
                    owner_tag = o.to_owned();
                }
            }
        }
    }
    Some(ShipSpec {
        name,
        definition,
        equipment_key,
        pride_of_the_fleet,
        owner_tag,
    })
}

// ─── Air ──────────────────────────────────────────────────────────────

/// 解析 `history/units/*_air_legacy.txt` / `*_air_bba.txt`。
pub fn load_oob_air(game_path: &Path) -> Result<HashMap<String, OobAir>, String> {
    load_oob_air_from_paths(&PathConfig::with_game_path(game_path))
}

pub fn load_oob_air_from_paths(paths: &PathConfig) -> Result<HashMap<String, OobAir>, String> {
    let dirs = data_dirs(paths, "history/units");
    let mut out = HashMap::new();
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
            let stem = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s.to_owned(),
                None => continue,
            };
            if !stem.contains("_air") {
                continue;
            }
            let content = fs::read_to_string(&path).unwrap_or_default();
            let block = parse(&content);
            let mut oob = OobAir {
                name: stem.clone(),
                wings: Vec::new(),
            };
            if let Some(aw) = block.get_block("air_wings") {
                // air_wings = { <state_id> = { fighter_equipment_0 = { owner amount } name = "X"
                //              tac_bomber_equipment_0 = { ... } name = "Y" ... } }
                for e in &aw.entries {
                    let Ok(state_id) = e.key.parse::<u16>() else {
                        continue;
                    };
                    let Value::Block(state_block) = &e.value else {
                        continue;
                    };
                    parse_air_wings_in_state(state_id, state_block, &mut oob.wings);
                }
            }
            if !oob.wings.is_empty() {
                out.insert(stem, oob);
            }
        }
    }
    Ok(out)
}

/// 将一个 state 块（vanilla 中 `air_wings = { 64 = { ... } }` 的内层）解析成多个
/// [`AirWingEntry`] 并 push 到 `out`。
///
/// vanilla 写法：每个装备块对应一个 wing；name 出现在自己的 wing 之后（有时之前）。
/// 我们按"name 与其前后最近的装备块绑定"近似：保留最后看到的 name；遇到新装备块
/// 暂存索引，遇到 name 把待绑定全部赋予。
fn parse_air_wings_in_state(state_id: u16, state_block: &Block, out: &mut Vec<AirWingEntry>) {
    let mut last_name: Option<String> = None;
    let mut pending_indices: Vec<usize> = Vec::new();
    for sub in &state_block.entries {
        match sub.key.as_str() {
            "name" => {
                if let Value::String(s) = &sub.value {
                    let s = strip_quotes(s);
                    last_name = Some(s.clone());
                    for &idx in &pending_indices {
                        out[idx].name = Some(s.clone());
                    }
                    pending_indices.clear();
                }
            }
            eq_key if eq_key.contains("equipment_") => {
                let Value::Block(eb) = &sub.value else {
                    continue;
                };
                let amount = eb.get_int("amount").unwrap_or(0) as u32;
                let owner_tag = eb
                    .get_string("owner")
                    .unwrap_or("")
                    .trim_matches('"')
                    .to_owned();
                if amount == 0 {
                    continue;
                }
                let entry = AirWingEntry {
                    state_id,
                    equipment_key: eq_key.to_owned(),
                    amount,
                    owner_tag,
                    name: last_name.clone(),
                };
                pending_indices.push(out.len());
                out.push(entry);
            }
            _ => {}
        }
    }
}

// ─── Country histories ───────────────────────────────────────────────

/// 解析 `history/countries/<TAG> - <Name>.txt` 全部国家文件。
pub fn load_country_histories(game_path: &Path) -> Result<HashMap<String, CountryHistory>, String> {
    load_country_histories_from_paths(&PathConfig::with_game_path(game_path))
}

pub fn load_country_histories_from_paths(
    paths: &PathConfig,
) -> Result<HashMap<String, CountryHistory>, String> {
    let dirs = data_dirs(paths, "history/countries");
    let mut out = HashMap::new();
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
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            // 文件名 "GER - Germany.txt" → tag = "GER"
            let tag = stem.split_whitespace().next().unwrap_or("").to_owned();
            if tag.len() != 3 {
                continue;
            }
            let content = fs::read_to_string(&path).unwrap_or_default();
            let block = parse(&content);
            let mut hist = CountryHistory {
                tag: tag.clone(),
                ..Default::default()
            };
            collect_country_history(&block, &mut hist);
            out.insert(tag, hist);
        }
    }
    Ok(out)
}

fn data_dirs(paths: &PathConfig, relative: &str) -> Vec<std::path::PathBuf> {
    let mut dirs = paths.find_all(relative);
    dirs.reverse();
    dirs
}

/// 递归收集顶层 / `if` 块内适用于 1936-01-01 的语句。
fn collect_country_history(block: &Block, out: &mut CountryHistory) {
    for e in &block.entries {
        // vanilla mixes `if` / `IF` / `If` —— 不区分大小写匹配。
        let key_lower_keyword = e.key.to_ascii_lowercase();
        match key_lower_keyword.as_str() {
            "set_oob" => {
                if out.set_oob.is_none() {
                    if let Value::String(s) = &e.value {
                        out.set_oob = Some(strip_quotes(s));
                    }
                }
            }
            "set_naval_oob" => {
                if out.set_naval_oob.is_none() {
                    if let Value::String(s) = &e.value {
                        out.set_naval_oob = Some(strip_quotes(s));
                    }
                }
            }
            "set_air_oob" => {
                if out.set_air_oob.is_none() {
                    if let Value::String(s) = &e.value {
                        out.set_air_oob = Some(strip_quotes(s));
                    }
                }
            }
            "set_research_slots" => {
                if let Some(n) = block_int(&e.value) {
                    out.research_slots = Some(n.clamp(0, 255) as u8);
                }
            }
            "set_stability" => {
                if let Some(v) = block_float(&e.value) {
                    out.stability = Some(v as f32);
                }
            }
            "set_war_support" => {
                if let Some(v) = block_float(&e.value) {
                    out.war_support = Some(v as f32);
                }
            }
            "add_ideas" => match &e.value {
                Value::String(s) => out.initial_ideas.push(strip_quotes(s)),
                Value::Block(b) => {
                    for v in &b.values {
                        if let Value::String(s) = v {
                            out.initial_ideas.push(strip_quotes(s));
                        }
                    }
                    // entries form (e.g. `legacy_oob = { hidden = yes }`) —— 罕见，跳过。
                }
                _ => {}
            },
            "set_country_flag" => match &e.value {
                Value::String(s) => out.initial_flags.push(strip_quotes(s)),
                Value::Block(b) => {
                    if let Some(s) = b.get_string("flag") {
                        out.initial_flags.push(strip_quotes(s));
                    }
                }
                _ => {}
            },
            "set_variable" => {
                if let Value::Block(b) = &e.value {
                    if let Some(first) = b.entries.first() {
                        let val = block_float(&first.value).unwrap_or(0.0) as f32;
                        out.initial_variables.push((first.key.clone(), val));
                    }
                }
            }
            "complete_national_focus" => {
                if let Value::String(s) = &e.value {
                    out.completed_focuses.push(strip_quotes(s));
                }
            }
            "unlock_national_focus" => {
                if let Value::String(s) = &e.value {
                    out.unlocked_focuses.push(strip_quotes(s));
                }
            }
            "set_politics" => {
                if let Value::Block(pb) = &e.value {
                    if let Some(rp) = pb.get_string("ruling_party") {
                        out.set_ruling_party = Some(strip_quotes(rp));
                    }
                }
            }
            "set_popularities" => {
                if let Value::Block(pb) = &e.value {
                    for sub in &pb.entries {
                        if let Some(v) = block_float(&sub.value) {
                            out.party_popularities
                                .insert(sub.key.clone(), (v as f32) / 100.0);
                        }
                    }
                }
            }
            "recruit_character" => {
                if let Value::String(s) = &e.value {
                    out.recruited_characters.push(strip_quotes(s));
                }
            }
            "if" => {
                if let Value::Block(ib) = &e.value {
                    let pass = match find_block_ci(ib, "limit") {
                        Some(limit) => limit_passes_no_dlc(limit),
                        None => true, // 没 limit 视为恒真
                    };
                    if pass {
                        let then_block = strip_limit(ib);
                        collect_country_history(&then_block, out);
                    } else if let Some(else_b) = find_else_block(ib) {
                        collect_country_history(else_b, out);
                    }
                }
            }
            // 也支持 `oob = "X"`（USA / 部分国家用此简写）
            "oob" => {
                if out.set_oob.is_none() {
                    if let Value::String(s) = &e.value {
                        out.set_oob = Some(strip_quotes(s));
                    }
                }
            }
            _ => {}
        }
    }
}

/// 兼容大小写：`else` / `ELSE`。
fn find_else_block(ib: &Block) -> Option<&Block> {
    for e in &ib.entries {
        if e.key.eq_ignore_ascii_case("else") {
            if let Value::Block(b) = &e.value {
                return Some(b);
            }
        }
    }
    None
}

/// 求值 `if = { limit = { ... } ... }` 的 `limit` 子块，假设 1936-01-01 启动 + 无 DLC。
///
/// 规则（保守近似，不实现完整 trigger 引擎）：
/// - 顶层 `has_dlc = "X"` → false（我们不带 DLC）
/// - 顶层 `date >= "1939.x"` 或非 1936 起始日期 → false
/// - 顶层 `NOT = { ... }` → true（NOT 内的 has_dlc / date 反转后通常为真）
/// - 顶层 `OR = { ... }` → 任一子条件为真即可（递归求值）；无可识别 → true（容忍）
/// - 顶层 `AND = { ... }` → 全部为真（递归）
/// - 其它未识别条件 → true（容忍未知 trigger，让 1936 启动尽量多走入 then 分支）
fn limit_passes_no_dlc(limit: &Block) -> bool {
    for e in &limit.entries {
        let k_lower = e.key.to_ascii_lowercase();
        match k_lower.as_str() {
            "has_dlc" => return false,
            "date" => {
                if let Value::String(s) = &e.value {
                    if !s.starts_with("1936") {
                        return false;
                    }
                }
            }
            "not" => {
                // NOT = { has_dlc=X has_dlc=Y date=... } 在无 DLC + 1936 时全部翻转为真。
                // 简化：直接当作 true。
            }
            "or" => {
                if let Value::Block(orb) = &e.value {
                    if !or_passes_no_dlc(orb) {
                        return false;
                    }
                }
            }
            "and" => {
                if let Value::Block(ab) = &e.value {
                    if !limit_passes_no_dlc(ab) {
                        return false;
                    }
                }
            }
            _ => {
                // 未识别条件假定通过（保守接受）
            }
        }
    }
    true
}

fn or_passes_no_dlc(orb: &Block) -> bool {
    // OR 通过：任一子条件求值为真。我们对每个子条件构造单条 limit 求值。
    // 简化：把每个 entry 单独包成一个 Block 再 limit_passes_no_dlc。
    if orb.entries.is_empty() {
        return true;
    }
    for e in &orb.entries {
        let mut single = Block::new();
        single.entries.push(e.clone());
        if limit_passes_no_dlc(&single) {
            return true;
        }
    }
    false
}

/// 判断 `if` 块的 `limit` 是否包含"日期 >= 1937 以后"——这种 if 在 1936-01-01 不应进入。
fn _unused_legacy_if_check(_ib: &Block) {
    // 旧实现 if_limit_excludes_1936 已被 limit_passes_no_dlc 替代。
}

/// 把 `if` 块去掉 `limit` 子块，返回"主体"以便递归收集。
fn strip_limit(ib: &Block) -> Block {
    let mut out = Block::new();
    for e in &ib.entries {
        let k = &e.key;
        if k.eq_ignore_ascii_case("limit") || k.eq_ignore_ascii_case("else") {
            continue;
        }
        out.entries.push(e.clone());
    }
    out.values = ib.values.clone();
    out
}

fn find_block_ci<'a>(b: &'a Block, key: &str) -> Option<&'a Block> {
    for e in &b.entries {
        if e.key.eq_ignore_ascii_case(key) {
            if let Value::Block(b) = &e.value {
                return Some(b);
            }
        }
    }
    None
}

fn strip_quotes(s: &str) -> String {
    s.trim_matches('"').to_owned()
}

fn block_int(v: &Value) -> Option<i64> {
    match v {
        Value::Integer(i) => Some(*i),
        Value::Float(f) => Some(*f as i64),
        _ => None,
    }
}

fn block_float(v: &Value) -> Option<f64> {
    match v {
        Value::Integer(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_division_instance_from_block() {
        let src = r#"
            division = {
                division_name = { is_name_ordered = yes name_order = 25 }
                location = 6334
                division_template = "Infanterie-Division"
                start_experience_factor = 0.3
            }
        "#;
        let b = parse(src);
        let div_block = match &b.entries[0].value {
            Value::Block(b) => b,
            _ => panic!(),
        };
        let d = parse_division_instance(div_block).unwrap();
        assert_eq!(d.template_name, "Infanterie-Division");
        assert_eq!(d.location, 6334);
        assert!((d.start_experience_factor - 0.3).abs() < 1e-6);
        assert_eq!(d.name_order, Some(25));
    }

    #[test]
    fn parse_country_history_skips_future_dates() {
        let src = r#"
            capital = 64
            add_ideas = { idea_a idea_b }
            set_oob = "GER_1936"
            if = {
                limit = { date >= "1939.1.1" }
                add_ideas = { future_idea }
                set_country_flag = future_flag
            }
            if = {
                limit = { has_dlc = "X" }
                set_oob = "GER_1936_dlc"
                else = {
                    set_oob = "GER_1936_alt"
                }
            }
            set_country_flag = top_flag
            set_variable = { my_var = 0.5 }
            complete_national_focus = my_focus
        "#;
        let b = parse(src);
        let mut h = CountryHistory {
            tag: "GER".into(),
            ..Default::default()
        };
        collect_country_history(&b, &mut h);
        assert_eq!(h.initial_ideas, vec!["idea_a", "idea_b"]);
        assert_eq!(h.set_oob.as_deref(), Some("GER_1936"));
        assert!(h.initial_flags.contains(&"top_flag".to_owned()));
        // 未来 idea 不应进入
        assert!(!h.initial_ideas.contains(&"future_idea".to_owned()));
        assert!(!h.initial_flags.contains(&"future_flag".to_owned()));
        assert_eq!(h.initial_variables, vec![("my_var".to_owned(), 0.5)]);
        assert_eq!(h.completed_focuses, vec!["my_focus"]);
    }

    #[test]
    fn parse_air_wing_block_orders_names_correctly() {
        let src = r#"
            air_wings = {
                64 = {
                    fighter_equipment_0 = { owner = "GER" amount = 80 }
                    name = "Jagdgeschwader 132"
                    tac_bomber_equipment_0 = { owner = "GER" amount = 80 }
                    name = "Kampfgeschwader 153"
                }
            }
        "#;
        let b = parse(src);
        let aw = b.get_block("air_wings").unwrap();
        let mut wings = Vec::new();
        for e in &aw.entries {
            let state_id = e.key.parse::<u16>().unwrap();
            let sb = match &e.value {
                Value::Block(b) => b,
                _ => panic!(),
            };
            parse_air_wings_in_state(state_id, sb, &mut wings);
        }
        assert_eq!(wings.len(), 2);
        assert_eq!(wings[0].state_id, 64);
        assert!(wings[0].equipment_key.contains("fighter"));
        assert_eq!(wings[0].name.as_deref(), Some("Jagdgeschwader 132"));
        assert!(wings[1].equipment_key.contains("tac_bomber"));
        assert_eq!(wings[1].name.as_deref(), Some("Kampfgeschwader 153"));
        assert_eq!(wings[0].owner_tag, "GER");
    }

    #[test]
    fn missing_directory_returns_empty_no_panic() {
        let p = std::path::Path::new("__definitely_does_not_exist_phase11__");
        assert!(load_oob_land(p).unwrap().is_empty());
        assert!(load_oob_naval(p).unwrap().is_empty());
        assert!(load_oob_air(p).unwrap().is_empty());
        assert!(load_country_histories(p).unwrap().is_empty());
    }

    #[test]
    fn limit_passes_handles_common_shapes() {
        // 顶层 has_dlc → 不通过
        let l = parse("has_dlc = \"X\"");
        assert!(!limit_passes_no_dlc(&l));

        // NOT = { has_dlc = X } → 通过
        let l = parse("NOT = { has_dlc = \"X\" }");
        assert!(limit_passes_no_dlc(&l));

        // 空 limit → 通过
        let l = parse("");
        assert!(limit_passes_no_dlc(&l));

        // 未来日期 → 不通过
        let l = parse("date = \"1939.1.1\"");
        assert!(!limit_passes_no_dlc(&l));

        // 未知 trigger → 通过（容忍）
        let l = parse("has_government = fascism");
        assert!(limit_passes_no_dlc(&l));
    }

    #[test]
    fn case_insensitive_if_and_else() {
        let src = r#"
            IF = {
                limit = { has_dlc = "X" }
                set_oob = "TST_dlc"
                ELSE = {
                    set_oob = "TST_base"
                }
            }
        "#;
        let b = parse(src);
        let mut h = CountryHistory {
            tag: "TST".into(),
            ..Default::default()
        };
        collect_country_history(&b, &mut h);
        assert_eq!(h.set_oob.as_deref(), Some("TST_base"));
    }
}
