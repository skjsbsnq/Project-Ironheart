//! Effect 鎵ц鍣細鎶?Paradox Script 鍧楀綋浣?鏁堟灉"鎵ц锛屼慨鏀?World 鐘舵€併€?
use std::collections::{HashMap, HashSet};

use clausewitz_parser::{Block, Value};
use hoi4_data::GameData;
use hoi4_state::{Building, BuildingKind, BuildingOwner, CountryId, PopClass, StateId, World};

use crate::scope::{Scope, ScopeChain};
use crate::vars::{Flags, Variables};

pub struct EffectInput<'a> {
    pub world: &'a mut World,
    pub data: &'a GameData,
    pub vars: &'a mut Variables,
    pub flags: &'a mut Flags,
    pub chain: &'a ScopeChain,
    pub value: &'a Value,
    pub report: &'a mut EffectReport,
}

pub type EffectFn = fn(&mut EffectInput<'_>);

#[derive(Debug, Clone, PartialEq)]
pub enum ScriptCommand {
    AddEquipmentToStockpile {
        country: CountryId,
        equipment: String,
        amount: f32,
    },
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct EffectReport {
    pub executed: usize,
    pub unknown: Vec<String>,
    pub noop: Vec<String>,
    pub commands: Vec<ScriptCommand>,
}

impl EffectReport {
    pub fn is_empty(&self) -> bool {
        self.executed == 0
            && self.unknown.is_empty()
            && self.noop.is_empty()
            && self.commands.is_empty()
    }

    pub fn extend(&mut self, other: EffectReport) {
        self.executed += other.executed;
        self.unknown.extend(other.unknown);
        self.noop.extend(other.noop);
        self.commands.extend(other.commands);
    }
}

#[derive(Default)]
pub struct EffectRegistry {
    effects: HashMap<String, EffectFn>,
    noop_effects: HashSet<String>,
}

impl EffectRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, name: impl Into<String>, f: EffectFn) {
        self.effects.insert(name.into(), f);
    }

    pub fn register_noop(&mut self, name: impl Into<String>) {
        let name = name.into();
        self.effects.insert(name.clone(), e_noop);
        self.noop_effects.insert(name);
    }

    pub fn get(&self, name: &str) -> Option<EffectFn> {
        self.effects.get(name).copied()
    }

    pub fn is_noop(&self, name: &str) -> bool {
        self.noop_effects.contains(name)
    }

    pub fn with_builtins() -> Self {
        let mut r = Self::new();
        r.register("add_political_power", e_add_pp);
        r.register("political_power", e_add_pp);
        r.register("add_stability", e_add_stability);
        r.register("add_war_support", e_add_war_support);
        r.register("army_experience", e_army_xp);
        r.register("navy_experience", e_navy_xp);
        r.register("air_experience", e_air_xp);
        r.register("add_manpower", e_add_manpower);
        r.register("set_country_flag", e_set_country_flag);
        r.register("clr_country_flag", e_clr_country_flag);
        r.register("set_state_flag", e_set_state_flag);
        r.register("clr_state_flag", e_clr_state_flag);
        r.register("set_global_flag", e_set_global_flag);
        r.register("clr_global_flag", e_clr_global_flag);
        r.register("set_variable", e_set_variable);
        r.register("add_to_variable", e_add_to_variable);
        r.register("set_politics", e_set_politics);
        r.register("add_popularity", e_add_popularity);
        r.register("add_ideas", e_add_ideas);
        r.register("add_idea", e_add_ideas);
        r.register("remove_ideas", e_remove_ideas);
        r.register("remove_idea", e_remove_ideas);
        r.register("transfer_state", e_transfer_state);
        r.register("add_named_threat", e_add_named_threat);
        r.register_noop("add_opinion_modifier");
        r.register_noop("country_event");
        r.register_noop("set_cosmetic_tag");
        r.register_noop("create_faction");
        // J.5.2: 12 economic lever effects
        r.register("add_building_construction", e_add_building);
        r.register("add_extra_state_shared_building_slots", e_add_building_slot);
        r.register("add_resource", e_add_resource);
        r.register("add_manpower_to_state", e_add_manpower_state);
        r.register("add_equipment_to_stockpile", e_add_equipment_stockpile);
        r.register("set_state_category", e_set_state_category);
        r.register("add_offsite_building", e_add_building);
        r.register("add_civilian_factory", e_add_civ);
        r.register("add_military_factory", e_add_mil);
        r.register("add_dockyard", e_add_dock);
        r.register("add_infrastructure", e_add_infra);
        r.register("add_research_slot", e_add_research_slot);
        // J.5.3: 6 political lever effects
        r.register("swap_ruling_party", e_swap_ruling_party);
        r.register("add_party_popularity", e_add_popularity);
        r.register("set_country_flag_with_days", e_set_country_flag);
        r.register_noop("unlock_focus_branch");
        r.register("add_timed_idea", e_add_ideas);
        r.register("retire_country_leader", e_noop);
        r
    }
}

/// Execute one effect block.
pub fn run_effect_block(
    block: &Block,
    world: &mut World,
    data: &GameData,
    vars: &mut Variables,
    flags: &mut Flags,
    chain: &ScopeChain,
    registry: &EffectRegistry,
) -> EffectReport {
    let mut report = EffectReport::default();
    for entry in &block.entries {
        match entry.key.as_str() {
            // scope changers
            "every_country" | "random_country" => {
                if let Value::Block(b) = &entry.value {
                    for i in 0..world.countries.count {
                        let mut nc = chain.clone();
                        nc.push(Scope::Country(CountryId(i as u16)));
                        report.extend(run_effect_block(b, world, data, vars, flags, &nc, registry));
                    }
                }
            }
            "every_owned_state" | "random_owned_state" => {
                if let Value::Block(b) = &entry.value {
                    let me = chain.this().as_country();
                    for i in 0..world.states.count {
                        if Some(world.states.owners[i]) == me {
                            let mut nc = chain.clone();
                            nc.push(Scope::State(StateId(i as u16)));
                            report.extend(run_effect_block(
                                b, world, data, vars, flags, &nc, registry,
                            ));
                        }
                    }
                }
            }
            "owner" | "controller" => {
                if let Value::Block(b) = &entry.value {
                    let target = match entry.key.as_str() {
                        "owner" => match chain.this() {
                            Scope::State(s) => Some(Scope::Country(world.state_owner(s))),
                            _ => None,
                        },
                        "controller" => match chain.this() {
                            Scope::State(s) => world
                                .states
                                .controllers
                                .get(s.0 as usize)
                                .copied()
                                .map(Scope::Country),
                            _ => None,
                        },
                        _ => None,
                    };
                    if let Some(s) = target {
                        let mut nc = chain.clone();
                        nc.push(s);
                        report.extend(run_effect_block(b, world, data, vars, flags, &nc, registry));
                    }
                }
            }
            name => {
                if let Some(f) = registry.get(name) {
                    report.executed += 1;
                    if registry.is_noop(name) {
                        report.noop.push(name.to_owned());
                    }
                    let mut input = EffectInput {
                        world,
                        data,
                        vars,
                        flags,
                        chain,
                        value: &entry.value,
                        report: &mut report,
                    };
                    f(&mut input);
                } else {
                    report.unknown.push(name.to_owned());
                }
            }
        }
    }
    report
}

// helpers

fn num_val(v: &Value) -> Option<f64> {
    match v {
        Value::Integer(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        _ => None,
    }
}

fn str_val(v: &Value) -> Option<&str> {
    match v {
        Value::String(s) => Some(s.as_str()),
        _ => None,
    }
}

fn this_country(input: &EffectInput<'_>) -> Option<CountryId> {
    input.chain.this().as_country()
}

fn this_state(input: &EffectInput<'_>) -> Option<StateId> {
    input.chain.this().as_state()
}

// effect impls

fn e_noop(_input: &mut EffectInput<'_>) {}

fn e_add_pp(input: &mut EffectInput<'_>) {
    let Some(c) = this_country(input) else { return };
    let Some(v) = num_val(input.value) else {
        return;
    };
    let i = c.0 as usize;
    input.world.countries.political_power[i] =
        (input.world.countries.political_power[i] + v as f32).clamp(-1000.0, 1000.0);
}

fn e_add_stability(input: &mut EffectInput<'_>) {
    let Some(c) = this_country(input) else { return };
    let Some(v) = num_val(input.value) else {
        return;
    };
    let i = c.0 as usize;
    input.world.countries.stability[i] =
        (input.world.countries.stability[i] + v as f32).clamp(0.0, 1.0);
}

fn e_add_war_support(input: &mut EffectInput<'_>) {
    let Some(c) = this_country(input) else { return };
    let Some(v) = num_val(input.value) else {
        return;
    };
    let i = c.0 as usize;
    input.world.countries.war_support[i] =
        (input.world.countries.war_support[i] + v as f32).clamp(0.0, 1.0);
}

fn e_army_xp(input: &mut EffectInput<'_>) {
    let Some(c) = this_country(input) else { return };
    let Some(v) = num_val(input.value) else {
        return;
    };
    input.world.countries.army_xp[c.0 as usize] += v as f32;
}

fn e_navy_xp(input: &mut EffectInput<'_>) {
    let Some(c) = this_country(input) else { return };
    let Some(v) = num_val(input.value) else {
        return;
    };
    input.world.countries.navy_xp[c.0 as usize] += v as f32;
}

fn e_air_xp(input: &mut EffectInput<'_>) {
    let Some(c) = this_country(input) else { return };
    let Some(v) = num_val(input.value) else {
        return;
    };
    input.world.countries.air_xp[c.0 as usize] += v as f32;
}

fn e_add_manpower(input: &mut EffectInput<'_>) {
    let Some(c) = this_country(input) else { return };
    let Some(v) = num_val(input.value) else {
        return;
    };
    if v >= 0.0 {
        add_manpower_to_pops(input.world, c, v as u64);
    } else {
        subtract_manpower_from_pops(input.world, c, (-v) as u64);
    }
}

fn e_set_country_flag(input: &mut EffectInput<'_>) {
    let Some(c) = this_country(input) else { return };
    let Some(name) = str_val(input.value) else {
        return;
    };
    input.flags.set_country_flag(c, name);
}

fn e_clr_country_flag(input: &mut EffectInput<'_>) {
    let Some(c) = this_country(input) else { return };
    let Some(name) = str_val(input.value) else {
        return;
    };
    input.flags.clear_country_flag(c, name);
}

fn e_set_state_flag(input: &mut EffectInput<'_>) {
    let Some(s) = this_state(input) else { return };
    let Some(name) = str_val(input.value) else {
        return;
    };
    input.flags.set_state_flag(s, name);
}

fn e_clr_state_flag(input: &mut EffectInput<'_>) {
    let Some(s) = this_state(input) else { return };
    let Some(name) = str_val(input.value) else {
        return;
    };
    input.flags.clear_state_flag(s, name);
}

fn e_set_global_flag(input: &mut EffectInput<'_>) {
    let Some(name) = str_val(input.value) else {
        return;
    };
    input.flags.set_global_flag(name);
}

fn e_clr_global_flag(input: &mut EffectInput<'_>) {
    let Some(name) = str_val(input.value) else {
        return;
    };
    input.flags.clear_global_flag(name);
}

fn e_set_variable(input: &mut EffectInput<'_>) {
    let Value::Block(b) = input.value else { return };
    let Some(var) = b
        .get_string("var")
        .or_else(|| b.entries.first().map(|e| e.key.as_str()))
    else {
        return;
    };
    let val = b
        .get_float("value")
        .or_else(|| {
            b.entries.first().and_then(|e| match &e.value {
                Value::Integer(i) => Some(*i as f64),
                Value::Float(f) => Some(*f),
                _ => None,
            })
        })
        .unwrap_or(0.0);
    match input.chain.this() {
        Scope::Country(c) => input.vars.set_country(c, var, val),
        Scope::State(s) => input.vars.set_state(s, var, val),
        _ => input.vars.set_global(var, val),
    }
}

fn e_add_to_variable(input: &mut EffectInput<'_>) {
    let Value::Block(b) = input.value else { return };
    let Some(var) = b
        .get_string("var")
        .or_else(|| b.entries.first().map(|e| e.key.as_str()))
    else {
        return;
    };
    let val = b
        .get_float("value")
        .or_else(|| {
            b.entries.first().and_then(|e| match &e.value {
                Value::Integer(i) => Some(*i as f64),
                Value::Float(f) => Some(*f),
                _ => None,
            })
        })
        .unwrap_or(0.0);
    match input.chain.this() {
        Scope::Country(c) => {
            input.vars.add_country(c, var, val);
        }
        Scope::State(s) => {
            input.vars.add_state(s, var, val);
        }
        _ => {
            input.vars.add_global(var, val);
        }
    }
}

fn e_set_politics(input: &mut EffectInput<'_>) {
    let Some(c) = this_country(input) else { return };
    let Value::Block(b) = input.value else { return };
    if let Some(rp) = b.get_string("ruling_party") {
        input.world.countries.ruling_party[c.0 as usize] = rp.to_owned();
        // J.1.8锛氭墽鏀垮厷鍙樺寲 鈫?閲嶉€夊厓棣栬倴鍍?        input.world.refresh_country_leader(c);
    }
}

fn e_add_popularity(input: &mut EffectInput<'_>) {
    let Some(c) = this_country(input) else { return };
    let Value::Block(b) = input.value else { return };
    let Some(ideology) = b.get_string("ideology") else {
        return;
    };
    let pop = b.get_float("popularity").unwrap_or(0.0) as f32;
    let i = c.0 as usize;
    let real = if ideology == "ROOT" {
        input.world.countries.ruling_party[i].clone()
    } else {
        ideology.to_owned()
    };
    let entry = input.world.countries.party_popularity[i]
        .entry(real)
        .or_insert(0.0);
    *entry = (*entry + pop).clamp(0.0, 1.0);
}

fn e_add_ideas(input: &mut EffectInput<'_>) {
    let Some(c) = this_country(input) else { return };
    let i = c.0 as usize;
    match input.value {
        Value::String(s) => {
            if !input.world.countries.ideas[i].contains(s) {
                input.world.countries.ideas[i].push(s.clone());
            }
        }
        Value::Block(b) => {
            for v in &b.values {
                if let Value::String(s) = v {
                    if !input.world.countries.ideas[i].contains(s) {
                        input.world.countries.ideas[i].push(s.clone());
                    }
                }
            }
        }
        _ => {}
    }
}

fn e_remove_ideas(input: &mut EffectInput<'_>) {
    let Some(c) = this_country(input) else { return };
    let i = c.0 as usize;
    match input.value {
        Value::String(s) => {
            input.world.countries.ideas[i].retain(|x| x != s);
        }
        Value::Block(b) => {
            for v in &b.values {
                if let Value::String(s) = v {
                    input.world.countries.ideas[i].retain(|x| x != s);
                }
            }
        }
        _ => {}
    }
}

fn e_transfer_state(input: &mut EffectInput<'_>) {
    let Some(c) = this_country(input) else { return };
    let Some(state_num) = num_val(input.value) else {
        return;
    };
    let game_id = state_num as u16;
    let Some(&sid) = input.world.state_id_lookup.get(&game_id) else {
        return;
    };
    input.world.transfer_state_to_country(sid, c);
}

fn e_add_named_threat(input: &mut EffectInput<'_>) {
    let Value::Block(b) = input.value else { return };
    let threat = b.get_float("threat").unwrap_or(0.0) as f32;
    input.world.diplomacy.world_tension =
        (input.world.diplomacy.world_tension + threat).clamp(0.0, 100.0);
}

// 鈹€鈹€鈹€ J.5.2: Economic lever effects 鈹€鈹€

/// add_building_construction = { type = arms_factory level = 2 ... }
/// or add_offsite_building = { type = arms_factory level = 1 }
fn e_add_building(input: &mut EffectInput<'_>) {
    let Value::Block(b) = input.value else { return };
    let Some(btype) = b.get_string("type") else {
        return;
    };
    let level = b.get_int("level").unwrap_or(1) as u8;
    let si = match this_state(input) {
        Some(s) => s.0 as usize,
        None => return,
    };
    if si >= input.world.states.count {
        return;
    }
    match btype {
        "industrial_complex" => add_v6_building(
            input.world,
            StateId(si as u16),
            "steel_mill",
            BuildingKind::Industrial,
            level,
        ),
        "arms_factory" => add_v6_building(
            input.world,
            StateId(si as u16),
            "arms_industry",
            BuildingKind::Military,
            level,
        ),
        "dockyard" => add_v6_building(
            input.world,
            StateId(si as u16),
            "shipyard",
            BuildingKind::Military,
            level,
        ),
        "infrastructure" => {
            input.world.states.infrastructure[si] =
                input.world.states.infrastructure[si].saturating_add(level)
        }
        _ => {}
    }
    input.world.recalc_country_caches();
}

/// add_extra_state_shared_building_slots = N
fn e_add_building_slot(input: &mut EffectInput<'_>) {
    let Some(s) = this_state(input) else { return };
    let Some(v) = num_val(input.value) else {
        return;
    };
    let si = s.0 as usize;
    if si >= input.world.states.count {
        return;
    }
    input.world.states.category_slots[si] =
        input.world.states.category_slots[si].saturating_add(v as u8);
}

/// add_resource = { type = steel amount = 5 }
fn e_add_resource(input: &mut EffectInput<'_>) {
    let Value::Block(b) = input.value else { return };
    let Some(_rtype) = b.get_string("type") else {
        return;
    };
    let _amount = b.get_float("amount").unwrap_or(0.0);
    // Resources are stored in GameData.states[si].resources (immutable at runtime).
    // For now this is a no-op; a proper implementation would need mutable state resources.
}

/// add_manpower_to_state = N (state-scoped, adds to state manpower pool)
fn e_add_manpower_state(input: &mut EffectInput<'_>) {
    let Some(s) = this_state(input) else { return };
    let Some(v) = num_val(input.value) else {
        return;
    };
    let si = s.0 as usize;
    if si >= input.world.states.count {
        return;
    }
    if v >= 0.0 {
        input.world.states.manpower_pool[si] =
            input.world.states.manpower_pool[si].saturating_add(v as u32);
    } else {
        input.world.states.manpower_pool[si] =
            input.world.states.manpower_pool[si].saturating_sub((-v) as u32);
    }
}

/// add_equipment_to_stockpile = { type = infantry_equipment amount = 500 }
fn e_add_equipment_stockpile(input: &mut EffectInput<'_>) {
    let Value::Block(b) = input.value else { return };
    let Some(etype) = b.get_string("type") else {
        return;
    };
    let amount = b.get_float("amount").unwrap_or(0.0) as f32;
    let Some(country) = this_country(input) else {
        return;
    };
    input
        .report
        .commands
        .push(ScriptCommand::AddEquipmentToStockpile {
            country,
            equipment: etype.to_owned(),
            amount,
        });
}

/// set_state_category = "large_city"
fn e_set_state_category(input: &mut EffectInput<'_>) {
    let Some(s) = this_state(input) else { return };
    let Some(cat_name) = str_val(input.value) else {
        return;
    };
    let si = s.0 as usize;
    if si >= input.world.states.count {
        return;
    }
    if let Some(cat) = input.data.state_categories.get(cat_name) {
        input.world.states.category_slots[si] = cat.local_building_slots;
    }
}

/// add_civilian_factory = N (shorthand, state-scoped)
fn e_add_civ(input: &mut EffectInput<'_>) {
    let Some(s) = this_state(input) else { return };
    let Some(v) = num_val(input.value) else {
        return;
    };
    let si = s.0 as usize;
    if si >= input.world.states.count {
        return;
    }
    add_v6_building(
        input.world,
        s,
        "steel_mill",
        BuildingKind::Industrial,
        v as u8,
    );
    input.world.recalc_country_caches();
}

/// add_military_factory = N (shorthand, state-scoped)
fn e_add_mil(input: &mut EffectInput<'_>) {
    let Some(s) = this_state(input) else { return };
    let Some(v) = num_val(input.value) else {
        return;
    };
    let si = s.0 as usize;
    if si >= input.world.states.count {
        return;
    }
    add_v6_building(
        input.world,
        s,
        "arms_industry",
        BuildingKind::Military,
        v as u8,
    );
    input.world.recalc_country_caches();
}

/// add_dockyard = N (shorthand, state-scoped)
fn e_add_dock(input: &mut EffectInput<'_>) {
    let Some(s) = this_state(input) else { return };
    let Some(v) = num_val(input.value) else {
        return;
    };
    let si = s.0 as usize;
    if si >= input.world.states.count {
        return;
    }
    add_v6_building(input.world, s, "shipyard", BuildingKind::Military, v as u8);
    input.world.recalc_country_caches();
}

fn add_v6_building(world: &mut World, state: StateId, def_id: &str, kind: BuildingKind, level: u8) {
    if level == 0 {
        return;
    }
    if let Some(existing) = world
        .countries
        .buildings_v6
        .buildings
        .iter_mut()
        .find(|building| building.state == state && building.building_def_id == def_id)
    {
        existing.level = existing.level.saturating_add(level);
        return;
    }
    world.countries.buildings_v6.buildings.push(Building {
        kind,
        building_def_id: def_id.to_owned(),
        state,
        level,
        active_pm: "default".to_owned(),
        employment: [0; 6],
        owner: BuildingOwner::State,
        requires_law: None,
        built_progress: 1.0,
        ..Building::runtime_defaults()
    });
}

/// add_infrastructure = N (shorthand, state-scoped)
fn e_add_infra(input: &mut EffectInput<'_>) {
    let Some(s) = this_state(input) else { return };
    let Some(v) = num_val(input.value) else {
        return;
    };
    let si = s.0 as usize;
    if si >= input.world.states.count {
        return;
    }
    input.world.states.infrastructure[si] =
        input.world.states.infrastructure[si].saturating_add(v as u8);
}

/// add_research_slot = N (country-scoped)
fn e_add_research_slot(input: &mut EffectInput<'_>) {
    let Some(c) = this_country(input) else { return };
    let Some(v) = num_val(input.value) else {
        return;
    };
    let i = c.0 as usize;
    input.world.countries.research_slots[i] =
        input.world.countries.research_slots[i].saturating_add(v as u8);
}

// 鈹€鈹€鈹€ J.5.3: Political lever effects 鈹€鈹€

/// swap_ruling_party = "democratic" (changes ruling party + refreshes leader)
fn e_swap_ruling_party(input: &mut EffectInput<'_>) {
    let Some(c) = this_country(input) else { return };
    let Some(ideology) = str_val(input.value) else {
        return;
    };
    let i = c.0 as usize;
    input.world.countries.ruling_party[i] = ideology.to_owned();
    input.world.refresh_country_leader(c);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use clausewitz_parser::parse;
    use hoi4_data::{Color, Country, CountryTag, State};
    use hoi4_map::{GameMap, Heightmap, ProvinceMap, TerrainBitmap, TerrainCatalog};
    use std::collections::HashMap;

    fn test_world() -> (World, Arc<GameData>) {
        let map = Arc::new(GameMap {
            definitions: vec![None; 4],
            rgb_to_id: HashMap::new(),
            province_map: ProvinceMap {
                width: 1,
                height: 1,
                pixels: vec![0],
            },
            adjacencies: vec![Vec::new(); 4],
            special_adjacencies: Vec::new(),
            heightmap: Heightmap {
                width: 1,
                height: 1,
                pixels: vec![0],
            },
            terrain_bmp: TerrainBitmap {
                width: 1,
                height: 1,
                pixels: vec![0],
                palette: [[0; 3]; 256],
            },
            terrain_catalog: TerrainCatalog::default(),
            tree_definition_bmp: None,
            tree_indices: Default::default(),
        });
        let mut data = GameData::default();
        let ger = CountryTag::new("GER");
        data.countries.insert(
            ger.clone(),
            Country {
                tag: ger.clone(),
                color: Color {
                    r: 80,
                    g: 80,
                    b: 80,
                },
                graphical_culture: "western_european_gfx".to_owned(),
                capital: 1,
                ruling_party: "neutrality".to_owned(),
                technologies: Vec::new(),
            },
        );
        data.states.push(State {
            id: 1,
            name: "Test State".to_owned(),
            manpower: 1000,
            owner: ger.clone(),
            cores: vec![ger],
            provinces: vec![1],
            category: "city".to_owned(),
            infrastructure: 0,
            victory_points: Vec::new(),
            resources: Vec::new(),
        });
        let data = Arc::new(data);
        (World::new(map, data.clone()), data)
    }

    #[test]
    fn registry_has_25_plus_effects() {
        let r = EffectRegistry::with_builtins();
        assert!(r.effects.len() >= 25);
        assert!(r.get("add_political_power").is_some());
        assert!(r.get("set_country_flag").is_some());
        assert!(r.get("set_variable").is_some());
        // J.5 new effects
        assert!(r.get("add_building_construction").is_some());
        assert!(r.get("swap_ruling_party").is_some());
        assert!(r.get("add_research_slot").is_some());
        assert!(r.get("add_extra_state_shared_building_slots").is_some());
    }

    #[test]
    fn effect_report_tracks_unknown_noop_and_commands() {
        let (mut world, data) = test_world();
        let ger = world.country("GER").unwrap();
        let chain = ScopeChain::new(Scope::Country(ger));
        let mut vars = Variables::default();
        let mut flags = Flags::default();
        let reg = EffectRegistry::with_builtins();
        let block = parse(
            r#"
            add_opinion_modifier = { target = GER modifier = test }
            totally_unknown_effect = yes
            add_equipment_to_stockpile = { type = infantry_equipment amount = 250 }
            "#,
        );

        let report = run_effect_block(
            &block, &mut world, &data, &mut vars, &mut flags, &chain, &reg,
        );

        assert_eq!(report.executed, 2);
        assert_eq!(report.unknown, vec!["totally_unknown_effect"]);
        assert_eq!(report.noop, vec!["add_opinion_modifier"]);
        assert_eq!(
            report.commands,
            vec![ScriptCommand::AddEquipmentToStockpile {
                country: ger,
                equipment: "infantry_equipment".to_owned(),
                amount: 250.0,
            }]
        );
    }
}

// 鈹€鈹€鈹€ J.5.5: Effect summary for tooltip 鈹€鈹€

/// Generate a brief human-readable summary of an effect for tooltip display.
pub fn summary_for_tooltip(key: &str, value: &Value) -> Option<String> {
    match key {
        "add_political_power" | "political_power" => {
            num_val(value).map(|v| format!("PP {:+.0}", v))
        }
        "add_stability" => num_val(value).map(|v| format!("Stability {:+.0}%", v * 100.0)),
        "add_war_support" => num_val(value).map(|v| format!("War Support {:+.0}%", v * 100.0)),
        "army_experience" => num_val(value).map(|v| format!("Army XP {:+.0}", v)),
        "navy_experience" => num_val(value).map(|v| format!("Navy XP {:+.0}", v)),
        "air_experience" => num_val(value).map(|v| format!("Air XP {:+.0}", v)),
        "add_manpower" => num_val(value).map(|v| format!("Manpower {:+.0}", v)),
        "add_ideas" | "add_idea" => str_val(value).map(|s| format!("Add idea: {}", s)),
        "remove_ideas" | "remove_idea" => str_val(value).map(|s| format!("Remove idea: {}", s)),
        "add_research_slot" => num_val(value).map(|v| format!("Research slots {:+.0}", v)),
        "add_civilian_factory" => num_val(value).map(|v| format!("Civilian factories {:+.0}", v)),
        "add_military_factory" => num_val(value).map(|v| format!("Military factories {:+.0}", v)),
        "add_dockyard" => num_val(value).map(|v| format!("Dockyards {:+.0}", v)),
        "add_infrastructure" => num_val(value).map(|v| format!("Infrastructure {:+.0}", v)),
        "add_extra_state_shared_building_slots" => {
            num_val(value).map(|v| format!("Building slots {:+.0}", v))
        }
        "swap_ruling_party" => str_val(value).map(|s| format!("Ruling party 鈫?{}", s)),
        "set_politics" => Some("Change government".to_string()),
        "transfer_state" => num_val(value).map(|v| format!("Gain state {:.0}", v)),
        "add_named_threat" => Some("Increase world tension".to_string()),
        "set_country_flag" | "set_global_flag" => None, // flags are internal, don't show
        _ => None,
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

fn subtract_manpower_from_pops(world: &mut World, country: CountryId, amount: u64) {
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
            let sub = remaining.min(pg.size as u64);
            pg.size = pg.size.saturating_sub(sub as u32);
            remaining = remaining.saturating_sub(sub);
        }
    }
}
