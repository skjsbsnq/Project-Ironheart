//! 科技系统：研发槽 + 每日 tick + 解锁。
//!
//! V6.F 重写：
//! - 科技数据**全部从 vanilla TXT 迁出**，改为 V6 RON 定义（`V6Database.technologies`）
//! - 科技不再"解锁 equipment archetype"，而是"解锁 PM / 解锁建筑大类 / 解锁法律档位 / 解锁商品"
//! - 研究槽位数量由 Civil Rights 法决定（2-5），取代 vanilla 的固定 2 + idea modifier
//! - 研究每日扣 cash_rm；财政归零时研究停滞（§4.7.2）
//! - Clerk 占用：每队列需常驻 100 名 Clerk 在 University 建筑就业；不够则 speed = 0
//! - 计划经济下五年计划方向 ±% 调速（§4.6.4）
//!
//! 与 vanilla loader 的关系：vanilla `GameData.technologies` 仍用于前置条件解析
//! （vanilla 科技可能有 OR-path 前置），V6 科技的前置用 `TechDef.prereqs` (AND-logic)。

use hoi4_content::{TechDef, TechUnlockDef, V6Database};
use hoi4_data::GameData;
use hoi4_state::{CountryId, PopClass, World};

pub mod constants {
    pub const BASE_RESEARCH_SPEED: f32 = 1.0;
    pub const COST_TO_DAYS: f32 = 100.0;
    pub const AHEAD_OF_TIME_FACTOR_PER_YEAR: f32 = 0.5;
    pub const SLOT_BASE_COST_RM: f64 = 500_000.0;
    pub const CLERK_PER_SLOT: u32 = 100;
}

#[derive(Debug, Clone, Default)]
pub enum ResearchSlot {
    #[default]
    Idle,
    Active {
        tech_key: String,
        progress: f32,
        total_cost: f32,
    },
}

impl ResearchSlot {
    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    pub fn current_tech(&self) -> Option<&str> {
        match self {
            Self::Active { tech_key, .. } => Some(tech_key.as_str()),
            Self::Idle => None,
        }
    }

    pub fn completion(&self) -> f32 {
        match self {
            Self::Active {
                progress,
                total_cost,
                ..
            } => {
                if *total_cost <= 0.0 {
                    0.0
                } else {
                    (progress / total_cost).clamp(0.0, 1.0)
                }
            }
            Self::Idle => 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResearchError {
    UnknownTech(String),
    AlreadyResearched(String),
    NoFreeSlot,
    PrereqNotMet(String),
    DuplicateActiveTech(String),
    InsufficientClerks,
    TreasuryEmpty,
}

impl std::fmt::Display for ResearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownTech(k) => write!(f, "unknown technology `{}`", k),
            Self::AlreadyResearched(k) => write!(f, "tech `{}` already researched", k),
            Self::NoFreeSlot => write!(f, "no idle research slot available"),
            Self::PrereqNotMet(k) => write!(f, "missing prerequisite for `{}`", k),
            Self::DuplicateActiveTech(k) => write!(f, "`{}` is already being researched", k),
            Self::InsufficientClerks => write!(f, "insufficient Clerk POP at University"),
            Self::TreasuryEmpty => write!(f, "treasury empty, cannot start research"),
        }
    }
}

impl std::error::Error for ResearchError {}

pub struct ResearchState {
    pub count: usize,
    pub slots: Vec<Vec<ResearchSlot>>,
    pub total_completed: Vec<u32>,
}

impl ResearchState {
    pub fn new(world: &World) -> Self {
        let n = world.countries.count;
        let mut slots = Vec::with_capacity(n);
        for i in 0..n {
            let cnt = world.countries.research_slots[i].max(1) as usize;
            slots.push(vec![ResearchSlot::Idle; cnt]);
        }
        Self {
            count: n,
            slots,
            total_completed: vec![0; n],
        }
    }

    pub fn ensure_capacity(&mut self, world: &World) {
        let n = world.countries.count;
        while self.slots.len() < n {
            let i = self.slots.len();
            let cnt = world
                .countries
                .research_slots
                .get(i)
                .copied()
                .unwrap_or(1)
                .max(1) as usize;
            self.slots.push(vec![ResearchSlot::Idle; cnt]);
        }
        while self.total_completed.len() < n {
            self.total_completed.push(0);
        }
        self.count = n;
    }

    pub fn start(
        &mut self,
        world: &World,
        country: CountryId,
        tech_key: &str,
        db: &V6Database,
    ) -> Result<usize, ResearchError> {
        if country.is_none() {
            return Err(ResearchError::NoFreeSlot);
        }
        let i = country.0 as usize;
        if i >= self.count {
            return Err(ResearchError::NoFreeSlot);
        }

        let slot_idx = self.slots[i]
            .iter()
            .position(|s| s.is_idle())
            .ok_or(ResearchError::NoFreeSlot)?;

        let tech = db
            .technologies
            .iter()
            .find(|t| t.id == tech_key)
            .ok_or_else(|| ResearchError::UnknownTech(tech_key.to_owned()))?;

        if world.countries.completed_techs[i]
            .iter()
            .any(|t| t == tech_key)
        {
            return Err(ResearchError::AlreadyResearched(tech_key.to_owned()));
        }

        for slot in &self.slots[i] {
            if let ResearchSlot::Active { tech_key: t, .. } = slot {
                if t == tech_key {
                    return Err(ResearchError::DuplicateActiveTech(tech_key.to_owned()));
                }
            }
        }

        if !tech.prereqs.is_empty() {
            let all_prereqs_met = tech
                .prereqs
                .iter()
                .all(|p| world.countries.completed_techs[i].iter().any(|t| t == p));
            if !all_prereqs_met {
                return Err(ResearchError::PrereqNotMet(tech_key.to_owned()));
            }
        }

        let total_cost = tech.research_cost * constants::COST_TO_DAYS;
        self.slots[i][slot_idx] = ResearchSlot::Active {
            tech_key: tech_key.to_owned(),
            progress: 0.0,
            total_cost,
        };
        Ok(slot_idx)
    }

    pub fn cancel(&mut self, country: CountryId, slot_idx: usize) {
        if country.is_none() {
            return;
        }
        let i = country.0 as usize;
        if let Some(slots) = self.slots.get_mut(i) {
            if let Some(slot) = slots.get_mut(slot_idx) {
                *slot = ResearchSlot::Idle;
            }
        }
    }

    pub fn active_techs(&self, country: CountryId) -> Vec<String> {
        if country.is_none() {
            return Vec::new();
        }
        let i = country.0 as usize;
        self.slots
            .get(i)
            .map(|slots| {
                slots
                    .iter()
                    .filter_map(|s| match s {
                        ResearchSlot::Active { tech_key, .. } => Some(tech_key.clone()),
                        ResearchSlot::Idle => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

fn tech_bonus_for_category(
    map: &std::collections::HashMap<String, f32>,
    category: &hoi4_content::TechCategoryDef,
) -> f32 {
    let keys = match category {
        hoi4_content::TechCategoryDef::Industry => &["industry", "Industry"][..],
        hoi4_content::TechCategoryDef::Chemistry => &["chemistry", "Chemistry"],
        hoi4_content::TechCategoryDef::Electrical => &["electrical", "Electrical"],
        hoi4_content::TechCategoryDef::Metallurgy => &["metallurgy", "Metallurgy"],
        hoi4_content::TechCategoryDef::MilitaryDoctrine => {
            &["land_doctrine", "MilitaryDoctrine", "armor"]
        }
        hoi4_content::TechCategoryDef::Aviation => {
            &["light_air", "medium_air", "air_doctrine", "Aviation"]
        }
        hoi4_content::TechCategoryDef::Naval => &["naval", "Naval"],
        hoi4_content::TechCategoryDef::SocialScience => &["social_science", "SocialScience"],
        hoi4_content::TechCategoryDef::InformationControl => {
            &["information_control", "InformationControl"]
        }
    };
    let mut best = 0.0f32;
    for key in keys {
        if let Some(&v) = map.get(*key) {
            best = best.max(v);
        }
    }
    best
}

fn daily_speed(tech: &TechDef, year: u16) -> f32 {
    let mut speed = constants::BASE_RESEARCH_SPEED;
    let ahead = if year < tech.start_year {
        (tech.start_year - year) as i32
    } else {
        0
    };
    if ahead > 0 {
        let factor = constants::AHEAD_OF_TIME_FACTOR_PER_YEAR.powi(ahead);
        speed *= factor;
    }
    speed.max(0.001)
}

fn university_clerks_by_country(world: &World) -> Vec<u32> {
    let mut totals = vec![0u32; world.countries.count];
    for pg in &world.countries.pops.groups {
        if pg.class != PopClass::Clerk {
            continue;
        }
        let Some(building_id) = pg.employed_at else {
            continue;
        };
        let Some(building) = world
            .countries
            .buildings_v6
            .buildings
            .get(building_id.0 as usize)
        else {
            continue;
        };
        if building.building_def_id != "university" {
            continue;
        }
        let state_idx = building.state.0 as usize;
        let Some(owner) = world.states.owners.get(state_idx).copied() else {
            continue;
        };
        if owner.is_none() {
            continue;
        }
        if let Some(total) = totals.get_mut(owner.0 as usize) {
            *total += pg.size;
        }
    }
    totals
}

pub fn apply_v6_tech_unlocks(world: &mut World, ci: usize, tech: &TechDef) {
    for unlock in &tech.unlocks {
        match unlock {
            TechUnlockDef::Good(good_id) => {
                let market = &mut world.countries.market.markets[ci];
                market.price.entry(good_id.clone()).or_insert(0.0);
                market.supply.entry(good_id.clone()).or_insert(0.0);
                market.demand.entry(good_id.clone()).or_insert(0.0);
                market.stockpile.entry(good_id.clone()).or_insert(0.0);
                market.imports.entry(good_id.clone()).or_insert(0.0);
                market.exports.entry(good_id.clone()).or_insert(0.0);
            }
            TechUnlockDef::PM(pm_id) => {
                let _ = pm_id;
            }
            TechUnlockDef::Building(building_id) => {
                world.countries.unlocked_buildings[ci].insert(building_id.clone());
            }
            TechUnlockDef::Law(category, law_id) => {
                let _ = (category, law_id);
            }
        }
    }
}

pub fn tick_daily_v6(
    world: &mut World,
    state: &mut ResearchState,
    db: &V6Database,
    _data: &GameData,
    _research_speed_factor: &[f32],
) {
    let year = world.date.year;
    let n = world.countries.count;

    if state.slots.len() < n {
        state.ensure_capacity(world);
    }

    let mut completions: Vec<(usize, usize, String)> = Vec::new();
    let university_clerks = university_clerks_by_country(world);

    for ci in 0..n {
        if ci >= state.slots.len() {
            break;
        }

        let clerk_total = university_clerks.get(ci).copied().unwrap_or(0);
        let active_slots = state.slots[ci]
            .iter()
            .filter(|s| matches!(s, ResearchSlot::Active { .. }))
            .count();
        let required_clerks = (active_slots as u32) * constants::CLERK_PER_SLOT;
        let clerk_ratio = if required_clerks > 0 {
            (clerk_total as f32 / required_clerks as f32).min(1.0)
        } else {
            1.0
        };

        let slot_cost = constants::SLOT_BASE_COST_RM;
        let total_research_cost = slot_cost * active_slots as f64;

        let treasury = &world.countries.treasury.treasuries[ci];
        let cash_rm = treasury.cash_rm;
        let can_afford_full = cash_rm >= total_research_cost;
        let research_funding = if can_afford_full {
            1.0
        } else if total_research_cost > 0.0 {
            (cash_rm / total_research_cost).clamp(0.0, 1.0) as f32
        } else {
            1.0
        };

        if total_research_cost > 0.0 && cash_rm > 0.0 {
            let actual_cost = total_research_cost.min(cash_rm);
            let _ = treasury;
            world.countries.treasury.treasuries[ci].pay(actual_cost, "research");
        }

        let tech_bonus_map = &world.countries.tech_bonus[ci];

        let slots = &mut state.slots[ci];
        for (si, slot) in slots.iter_mut().enumerate() {
            if let ResearchSlot::Active {
                tech_key,
                progress,
                total_cost,
            } = slot
            {
                let tech = match db.technologies.iter().find(|t| t.id == *tech_key) {
                    Some(t) => t,
                    None => continue,
                };

                let base_speed = daily_speed(tech, year);
                let planned_mod =
                    hoi4_content::planned_research_direction_modifier(world, ci, tech_key, db);
                let tech_bonus = tech_bonus_for_category(tech_bonus_map, &tech.category);
                let speed =
                    base_speed * (1.0 + planned_mod + tech_bonus) * clerk_ratio * research_funding;

                *progress += speed;
                if *progress >= *total_cost {
                    completions.push((ci, si, tech_key.clone()));
                }
            }
        }
    }

    for (ci, si, tech_key) in completions {
        let tech = db.technologies.iter().find(|t| t.id == tech_key);
        let cid = CountryId(ci as u16);
        if world.mark_tech_completed(cid, &tech_key) {
            state.total_completed[ci] = state.total_completed[ci].saturating_add(1);
        }
        if let Some(tech_def) = tech {
            apply_v6_tech_unlocks(world, ci, tech_def);
        }
        state.slots[ci][si] = ResearchSlot::Idle;
    }
}

/// Legacy vanilla tick -- kept for backward compatibility during V6.F transition.
/// Delegates to `tick_daily_v6` when V6 technologies are available.
pub fn tick_daily(
    world: &mut World,
    state: &mut ResearchState,
    data: &GameData,
    research_speed_factor: &[f32],
) {
    let db = V6Database::load();
    if !db.technologies.is_empty() {
        tick_daily_v6(world, state, &db, data, research_speed_factor);
        return;
    }
    let year = world.date.year;
    let n = world.countries.count;

    if state.slots.len() < n {
        state.ensure_capacity(world);
    }

    let mut completions: Vec<(usize, usize, String)> = Vec::new();

    for ci in 0..n {
        if ci >= state.slots.len() {
            break;
        }
        let slots = &mut state.slots[ci];
        let country_bonus = research_speed_factor.get(ci).copied().unwrap_or(0.0);
        for (si, slot) in slots.iter_mut().enumerate() {
            if let ResearchSlot::Active {
                tech_key,
                progress,
                total_cost,
            } = slot
            {
                let Some(tech) = data.technologies.get(tech_key) else {
                    continue;
                };
                let speed = daily_speed_vanilla(tech, year) * (1.0 + country_bonus);
                *progress += speed;
                if *progress >= *total_cost {
                    completions.push((ci, si, tech_key.clone()));
                }
            }
        }
    }

    for (ci, si, tech_key) in completions {
        let cid = CountryId(ci as u16);
        if world.mark_tech_completed(cid, &tech_key) {
            state.total_completed[ci] = state.total_completed[ci].saturating_add(1);
        }
        state.slots[ci][si] = ResearchSlot::Idle;
    }
}

fn daily_speed_vanilla(tech: &hoi4_data::Technology, year: u16) -> f32 {
    let mut speed = constants::BASE_RESEARCH_SPEED;
    let ahead = tech.years_ahead(year);
    if ahead > 0 {
        let factor = constants::AHEAD_OF_TIME_FACTOR_PER_YEAR.powi(ahead);
        speed *= factor;
    }
    speed.max(0.001)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_default_is_idle() {
        let s = ResearchSlot::Idle;
        assert!(s.is_idle());
        assert!(s.current_tech().is_none());
        assert_eq!(s.completion(), 0.0);
    }

    #[test]
    fn slot_completion_math() {
        let s = ResearchSlot::Active {
            tech_key: "x".into(),
            progress: 50.0,
            total_cost: 100.0,
        };
        assert!((s.completion() - 0.5).abs() < 1e-6);
        assert_eq!(s.current_tech(), Some("x"));
    }

    #[test]
    fn v6_tech_database_loads() {
        let db = V6Database::load();
        assert!(!db.technologies.is_empty(), "V6 technologies must load");
        let categories: Vec<String> = db
            .technologies
            .iter()
            .map(|t| format!("{:?}", t.category))
            .collect();
        assert!(categories.iter().any(|c| c.contains("Industry")));
        assert!(categories.iter().any(|c| c.contains("Chemistry")));
        assert!(categories.iter().any(|c| c.contains("Electrical")));
    }

    #[test]
    fn v6_tech_unlocks_reference_valid_entities() {
        let db = V6Database::load();
        for tech in &db.technologies {
            for unlock in &tech.unlocks {
                match unlock {
                    TechUnlockDef::Good(good_id) => {
                        assert!(
                            db.goods.iter().any(|g| g.id == *good_id),
                            "tech '{}' unlocks Good '{}' but no such good in V6 database",
                            tech.id,
                            good_id
                        );
                    }
                    TechUnlockDef::PM(pm_id) => {
                        assert!(
                            db.production_methods.iter().any(|pm| pm.id == *pm_id),
                            "tech '{}' unlocks PM '{}' but no such PM in V6 database",
                            tech.id,
                            pm_id
                        );
                    }
                    TechUnlockDef::Building(building_id) => {
                        assert!(
                            db.buildings.iter().any(|b| b.id == *building_id),
                            "tech '{}' unlocks Building '{}' but no such building in V6 database",
                            tech.id,
                            building_id
                        );
                    }
                    TechUnlockDef::Law(_, law_id) => {
                        let found = db.conscription_laws.iter().any(|l| l.id == *law_id)
                            || db.economy_laws.iter().any(|l| l.id == *law_id)
                            || db.trade_laws.iter().any(|l| l.id == *law_id)
                            || db.taxation_laws.iter().any(|l| l.id == *law_id)
                            || db.civil_rights_laws.iter().any(|l| l.id == *law_id)
                            || db.information_control_laws.iter().any(|l| l.id == *law_id);
                        assert!(
                            found,
                            "tech '{}' unlocks Law '{}' but no such law in V6 database",
                            tech.id, law_id
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn v6_tech_prereqs_reference_valid_techs() {
        let db = V6Database::load();
        let tech_ids: Vec<&str> = db.technologies.iter().map(|t| t.id.as_str()).collect();
        for tech in &db.technologies {
            for prereq in &tech.prereqs {
                assert!(
                    tech_ids.contains(&prereq.as_str()),
                    "tech '{}' prereqs '{}' but no such tech in V6 database",
                    tech.id,
                    prereq
                );
            }
        }
    }

    #[test]
    fn v6_tech_unlocked_by_keys_match_tech_ids() {
        let db = V6Database::load();
        let tech_ids: Vec<&str> = db.technologies.iter().map(|t| t.id.as_str()).collect();
        for good in &db.goods {
            if let Some(ref unlocked_by) = good.unlocked_by {
                assert!(
                    tech_ids.contains(&unlocked_by.as_str()),
                    "good '{}' unlocked_by '{}' but no such tech in V6 database",
                    good.id,
                    unlocked_by
                );
            }
        }
        for pm in &db.production_methods {
            if let Some(ref unlocked_by) = pm.unlocked_by {
                assert!(
                    tech_ids.contains(&unlocked_by.as_str()),
                    "PM '{}' unlocked_by '{}' but no such tech in V6 database",
                    pm.id,
                    unlocked_by
                );
            }
        }
    }
}
