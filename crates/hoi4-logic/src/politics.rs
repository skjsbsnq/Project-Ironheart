//! Politics system: political power, focus progress, ideas, and ideology support.
use clausewitz_parser::parser::{Block, Value};
use hoi4_data::GameData;
use hoi4_state::{CountryId, PopClass, World};

pub mod constants {
    pub const BASE_PP_DAILY_GAIN: f32 = 1.0;

    pub const BASE_FOCUS_DAILY_PROGRESS: f32 = 1.0;
}

// 鈹€鈹€鈹€ 閿欒 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

#[derive(Debug, Clone, PartialEq)]
pub enum PoliticsError {
    UnknownFocus(String),
    AlreadyCompleted(String),
    AlreadyActive(String),
    PrereqNotMet(String),
    MutuallyExclusiveCompleted(String),
    FocusNotInTree(String),
    NoTreeForCountry(String),
    UnknownIdea(String),
}

impl std::fmt::Display for PoliticsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownFocus(k) => write!(f, "unknown focus `{}`", k),
            Self::AlreadyCompleted(k) => write!(f, "focus `{}` already completed", k),
            Self::AlreadyActive(k) => write!(f, "another focus is already active: `{}`", k),
            Self::PrereqNotMet(k) => write!(f, "prerequisites not met for `{}`", k),
            Self::MutuallyExclusiveCompleted(k) => {
                write!(f, "mutually-exclusive focus has been chosen: `{}`", k)
            }
            Self::FocusNotInTree(k) => write!(f, "focus `{}` not found in any tree", k),
            Self::NoTreeForCountry(t) => write!(f, "no focus tree available for country `{}`", t),
            Self::UnknownIdea(k) => write!(f, "unknown idea `{}`", k),
        }
    }
}

impl std::error::Error for PoliticsError {}

// 鈹€鈹€鈹€ 缂撳瓨锛氳仛鍚?idea 鐨?modifier 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

/// 褰撳墠 active ideas 绱姞寰楀埌鐨?country-wide modifier銆?///
/// 鎴戜滑鍙淮鎶ゅ奖鍝?*宸插疄鐜板瓙绯荤粺* 鐨?modifier锛?/// 閬垮厤 hashmap 鏃犻檺鑶ㄨ儉銆傚悗缁ā鍧楀彲鎸夐渶娣诲姞鏂板瓧娈点€?#[derive(Debug, Clone, Default)]
pub struct PoliticsCache {
    /// `political_power_gain` (flat additive to BASE_PP_DAILY_GAIN).
    pub pp_gain_flat: Vec<f32>,
    /// `political_power_factor` / `political_power_gain_factor` (multiplies 1+x).
    pub pp_gain_factor: Vec<f32>,
    /// `national_focus_progress` additive modifier (multiplies 1+x).
    pub focus_speed_factor: Vec<f32>,
    /// `production_speed_buildings_factor` building construction speed bonus.
    /// Read by `hoi4_logic::economy::construction`.
    pub construction_speed_factor: Vec<f32>,
    /// `production_factory_max_efficiency_factor` absolute bonus.
    pub production_efficiency_cap_bonus: Vec<f32>,
    /// `research_speed_factor` multiplier.
    pub research_speed_factor: Vec<f32>,
    /// `consumer_goods_factor` absolute value, 0..1.
    pub consumer_goods_factor: Vec<f32>,
}

impl PoliticsCache {
    pub fn new(country_count: usize) -> Self {
        Self {
            pp_gain_flat: vec![0.0; country_count],
            pp_gain_factor: vec![0.0; country_count],
            focus_speed_factor: vec![0.0; country_count],
            construction_speed_factor: vec![0.0; country_count],
            production_efficiency_cap_bonus: vec![0.0; country_count],
            research_speed_factor: vec![0.0; country_count],
            consumer_goods_factor: vec![0.0; country_count],
        }
    }

    pub fn count(&self) -> usize {
        self.pp_gain_flat.len()
    }
}

// 鈹€鈹€鈹€ 鍏叡 API 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

/// Assign a national focus, replacing any in-progress focus.
pub fn assign_focus(
    world: &mut World,
    country: CountryId,
    focus_id: &str,
    data: &GameData,
) -> Result<(), PoliticsError> {
    if country.is_none() {
        return Err(PoliticsError::UnknownFocus(focus_id.to_owned()));
    }
    let i = country.0 as usize;

    // 1) focus 瀛樺湪
    let tree_id = data
        .focus_to_tree
        .get(focus_id)
        .ok_or_else(|| PoliticsError::FocusNotInTree(focus_id.to_owned()))?;
    let focus = data.focus_trees[tree_id]
        .focuses
        .get(focus_id)
        .ok_or_else(|| PoliticsError::UnknownFocus(focus_id.to_owned()))?;

    // 2) 鏈畬鎴愯繃
    if world.countries.completed_focuses[i].contains(focus_id) {
        return Err(PoliticsError::AlreadyCompleted(focus_id.to_owned()));
    }

    // 3) 浜掓枼椤逛笉鑳藉凡瀹屾垚
    for me in &focus.mutually_exclusive {
        if world.countries.completed_focuses[i].contains(me) {
            return Err(PoliticsError::MutuallyExclusiveCompleted(me.clone()));
        }
    }

    // 4) 鍓嶇疆婊¤冻
    if !focus.prereqs_met(&world.countries.completed_focuses[i]) {
        return Err(PoliticsError::PrereqNotMet(focus_id.to_owned()));
    }

    // 鏇挎崲褰撳墠 focus锛圚OI4 涔熷厑璁镐腑閫斿垏鎹紝浣嗗凡绉疮鐨勮繘搴﹀綊闆讹級
    world.countries.current_focus[i] = Some(focus_id.to_owned());
    world.countries.focus_progress[i] = 0.0;
    Ok(())
}

/// Cancel the active national focus.
pub fn cancel_focus(world: &mut World, country: CountryId) {
    if country.is_none() {
        return;
    }
    let i = country.0 as usize;
    world.countries.current_focus[i] = None;
    world.countries.focus_progress[i] = 0.0;
}

/// Add a national idea.
pub fn add_idea(
    world: &mut World,
    country: CountryId,
    idea_id: &str,
    data: &GameData,
) -> Result<(), PoliticsError> {
    if country.is_none() || !data.ideas.contains_key(idea_id) {
        return Err(PoliticsError::UnknownIdea(idea_id.to_owned()));
    }
    let i = country.0 as usize;
    if !world.countries.ideas[i].iter().any(|x| x == idea_id) {
        world.countries.ideas[i].push(idea_id.to_owned());
    }
    Ok(())
}

/// Remove a national idea.
pub fn remove_idea(world: &mut World, country: CountryId, idea_id: &str) {
    if country.is_none() {
        return;
    }
    let i = country.0 as usize;
    world.countries.ideas[i].retain(|x| x != idea_id);
}

/// Add support to an ideology in a country.
pub fn add_popularity(world: &mut World, country: CountryId, ideology: &str, delta: f32) {
    if country.is_none() {
        return;
    }
    let i = country.0 as usize;
    let entry = world.countries.party_popularity[i]
        .entry(ideology.to_owned())
        .or_insert(0.0);
    *entry = (*entry + delta).clamp(0.0, 1.0);
}

/// 閲嶆柊璁＄畻鎵€鏈夊浗瀹剁殑 idea modifier 缂撳瓨
pub fn recompute_modifiers(world: &World, data: &GameData, cache: &mut PoliticsCache) {
    let n = world.countries.count;
    if cache.count() != n {
        *cache = PoliticsCache::new(n);
    }
    for ci in 0..n {
        let mut pp_flat = 0.0f32;
        let mut pp_factor = 0.0f32;
        let mut focus_speed = 0.0f32;
        let mut construction = 0.0f32;
        let mut prod_eff = 0.0f32;
        let mut research = 0.0f32;
        let mut consumer = 0.0f32;

        for idea_key in &world.countries.ideas[ci] {
            let Some(idea) = data.ideas.get(idea_key) else {
                continue;
            };
            for (k, v) in &idea.modifiers {
                match k.as_str() {
                    "political_power_gain" => pp_flat += v,
                    "political_power_factor" | "political_power_gain_factor" => pp_factor += v,
                    "national_focus_progress" => focus_speed += v,
                    "production_speed_buildings_factor" => construction += v,
                    "production_factory_max_efficiency_factor" => prod_eff += v,
                    "research_speed_factor" => research += v,
                    "consumer_goods_factor" => consumer += v,
                    _ => {} // 鍏跺畠 modifier 鏆備笉澶勭悊
                }
            }
        }

        cache.pp_gain_flat[ci] = pp_flat;
        cache.pp_gain_factor[ci] = pp_factor;
        cache.focus_speed_factor[ci] = focus_speed;
        cache.construction_speed_factor[ci] = construction;
        cache.production_efficiency_cap_bonus[ci] = prod_eff;
        cache.research_speed_factor[ci] = research;
        cache.consumer_goods_factor[ci] = consumer;
    }
}

// 鈹€鈹€鈹€ 姣忔棩 tick 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

/// Advance politics for one day.
/// P0.3：移除 focus progress 推进，仅保留 PP 累加。
/// Focus 进度和完成效果由 RON content daily（`daily_focus_tick`）统一处理。
pub fn tick_daily(world: &mut World, data: &GameData, cache: &PoliticsCache) {
    let _ = data;
    let n = world.countries.count;

    // 1) PP 累加
    for ci in 0..n {
        let base = constants::BASE_PP_DAILY_GAIN;
        let flat = cache.pp_gain_flat.get(ci).copied().unwrap_or(0.0);
        let factor = 1.0 + cache.pp_gain_factor.get(ci).copied().unwrap_or(0.0);
        let gain = (base + flat) * factor;
        world.countries.political_power[ci] =
            (world.countries.political_power[ci] + gain).min(1000.0);
    }
}
// 鈹€鈹€鈹€ Effect runner锛堟渶灏忓彲鐢ㄩ泦鍚堬級 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

/// 鎵ц涓€娈?effect block锛岃瘑鍒笅闈㈣繖缁?key锛涘叾瀹冮潤榛樿烦杩囥€?///
/// - `add_political_power = N` / `political_power = N`
/// - `add_stability = N` / `add_war_support = N`
/// - `army_experience = N` / `navy_experience = N` / `air_experience = N`
/// - `add_manpower = N`
/// - `add_ideas = "<idea>"` 鎴?`add_ideas = { "a" "b" }` 鎴?`add_ideas = { idea = X }`
/// - `add_idea = "<idea>"`
/// - `add_popularity = { ideology = X popularity = N }`
/// - `set_politics = { ruling_party = X }` 鎴?`set_politics = X`锛堢矖鐣ワ級
///
/// Run a small subset of focus effect blocks.
pub fn run_effect_block(world: &mut World, country: CountryId, block: &Block, data: &GameData) {
    if country.is_none() {
        return;
    }
    let i = country.0 as usize;

    for entry in &block.entries {
        match entry.key.as_str() {
            "add_political_power" | "political_power" => {
                if let Some(v) = num_value(&entry.value) {
                    world.countries.political_power[i] =
                        (world.countries.political_power[i] + v).clamp(-1000.0, 1000.0);
                }
            }
            "add_stability" => {
                if let Some(v) = num_value(&entry.value) {
                    world.countries.stability[i] =
                        (world.countries.stability[i] + v).clamp(0.0, 1.0);
                }
            }
            "add_war_support" => {
                if let Some(v) = num_value(&entry.value) {
                    world.countries.war_support[i] =
                        (world.countries.war_support[i] + v).clamp(0.0, 1.0);
                }
            }
            "army_experience" => {
                if let Some(v) = num_value(&entry.value) {
                    world.countries.army_xp[i] += v;
                }
            }
            "navy_experience" => {
                if let Some(v) = num_value(&entry.value) {
                    world.countries.navy_xp[i] += v;
                }
            }
            "air_experience" => {
                if let Some(v) = num_value(&entry.value) {
                    world.countries.air_xp[i] += v;
                }
            }
            "add_manpower" => {
                if let Some(v) = num_value(&entry.value) {
                    let n = v.max(0.0) as u64;
                    add_manpower_to_pops(world, country, n);
                }
            }
            "add_ideas" | "add_idea" => match &entry.value {
                Value::String(s) => {
                    let _ = add_idea(world, country, s, data);
                }
                Value::Block(b) => {
                    for v in &b.values {
                        if let Value::String(s) = v {
                            let _ = add_idea(world, country, s, data);
                        }
                    }
                    for ee in &b.entries {
                        if ee.key == "idea" {
                            if let Value::String(s) = &ee.value {
                                let _ = add_idea(world, country, s, data);
                            }
                        }
                    }
                }
                _ => {}
            },
            "remove_ideas" | "remove_idea" => match &entry.value {
                Value::String(s) => remove_idea(world, country, s),
                Value::Block(b) => {
                    for v in &b.values {
                        if let Value::String(s) = v {
                            remove_idea(world, country, s);
                        }
                    }
                }
                _ => {}
            },
            "add_popularity" => {
                if let Value::Block(b) = &entry.value {
                    let ideology = b.get_string("ideology").unwrap_or("").to_owned();
                    let pop = b.get_float("popularity").map(|v| v as f32).unwrap_or(0.0);
                    if !ideology.is_empty() {
                        let real = if ideology == "ROOT" {
                            world.countries.ruling_party[i].clone()
                        } else {
                            ideology
                        };
                        add_popularity(world, country, &real, pop);
                    }
                }
            }
            "set_politics" => {
                if let Value::Block(b) = &entry.value {
                    if let Some(rp) = b.get_string("ruling_party") {
                        world.countries.ruling_party[i] = rp.to_owned();
                    }
                }
            }
            _ => {}
        }
    }
}

fn add_manpower_to_pops(world: &mut World, country: CountryId, amount: u64) {
    let state_ids = world.country_state_ids(country);
    let indices = world
        .countries
        .pops
        .pops_by_class_in_country_mut(PopClass::Soldier, &state_ids);
    let mut remaining = amount;
    for &idx in &indices {
        if remaining == 0 {
            break;
        }
        let pg = &mut world.countries.pops.groups[idx];
        if pg.employed_at.is_none() {
            let add = remaining.min(u32::MAX as u64) as u32;
            pg.size = pg.size.saturating_add(add);
            remaining = remaining.saturating_sub(add as u64);
        }
    }
    if remaining > 0 {
        if let Some(&sid) = state_ids.first() {
            world.countries.pops.groups.push(hoi4_state::PopGroup {
                class: PopClass::Soldier,
                state: sid,
                size: remaining.min(u32::MAX as u64) as u32,
                employed_at: None,
                wage_rm: 0.0,
                tax_burden: 0.0,
                income_rm: 0.0,
                tax_paid_rm: 0.0,
                disposable_income_rm: 0.0,
                basic_consumption_budget: 0.0,
                satisfaction_law_modifier: 0.0,
                loyalty_coefficient: 1.0,
                loyalty_decay_mult: 1.0,
                satisfaction: 1.0,
                political_loyalty: 0.5,
                literacy: PopClass::Soldier.baseline_literacy(),
                skilled_ratio: PopClass::Soldier.baseline_skilled_ratio(),
                standard_of_living: 0.5,
                needs_fulfillment: 1.0,
                essential_needs_fulfillment: 1.0,
                normal_needs_fulfillment: 1.0,
                luxury_needs_fulfillment: 1.0,
                radicalism: 0.0,
            });
        }
    }
}

fn num_value(v: &Value) -> Option<f32> {
    match v {
        Value::Integer(i) => Some(*i as f32),
        Value::Float(f) => Some(*f as f32),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hoi4_data::NationalFocus;

    #[test]
    fn cache_default_zeroed() {
        let c = PoliticsCache::new(3);
        assert_eq!(c.count(), 3);
        assert!(c.pp_gain_flat.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn focus_cost_math() {
        let f = NationalFocus {
            id: "x".into(),
            tree_id: "t".into(),
            cost_weeks: 10.0,
            prerequisites: vec![],
            mutually_exclusive: vec![],
            completion_reward: None,
            available: None,
            available_if_capitulated: false,
        };
        assert_eq!(f.cost_days(), 70.0);
    }

    #[test]
    fn focus_prereq_or_logic() {
        use std::collections::HashSet;
        let f = NationalFocus {
            id: "x".into(),
            tree_id: "t".into(),
            cost_weeks: 10.0,
            // prerequisite = { focus = A focus = B } (OR)
            prerequisites: vec![vec!["A".into(), "B".into()]],
            mutually_exclusive: vec![],
            completion_reward: None,
            available: None,
            available_if_capitulated: false,
        };
        let mut completed = HashSet::new();
        assert!(!f.prereqs_met(&completed));
        completed.insert("B".into());
        assert!(f.prereqs_met(&completed));
    }
}
