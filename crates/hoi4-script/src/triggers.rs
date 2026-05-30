//! Trigger 求值器：把 Paradox Script 块当作"条件"求值为 bool。
//!
//! Trigger 的语法形式：
//! ```ignore
//! AND = { has_war = yes  has_government = fascism }
//! OR  = { has_war_with = ENG  has_war_with = FRA }
//! NOT = { has_country_flag = peace }
//! has_war_with = ENG
//! num_of_factories > 50
//! ```
//!
//! 整个 trigger block 的语义是 **AND**（所有 entries 必须为真）。
//! 顶层调用 [`eval_trigger_block`]。

use std::collections::HashMap;

use clausewitz_parser::{Block, Value};
use hoi4_data::GameData;
use hoi4_state::{CountryId, StateId, World};

use crate::scope::{Scope, ScopeChain};
use crate::value::Compare;
use crate::vars::{Flags, Variables};

/// trigger 求值时的全部输入
pub struct TriggerInput<'a> {
    pub world: &'a World,
    pub data: &'a GameData,
    pub vars: &'a Variables,
    pub flags: &'a Flags,
    pub chain: &'a ScopeChain,
    /// 比较运算符（仅当 trigger entry 是 `key OP value` 形式时有意义）
    pub op: Compare,
    /// trigger entry 的右值
    pub value: &'a Value,
}

/// trigger 处理函数签名
pub type TriggerFn = fn(&TriggerInput<'_>) -> bool;

/// trigger 注册表
#[derive(Default)]
pub struct TriggerRegistry {
    triggers: HashMap<String, TriggerFn>,
}

impl TriggerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, name: impl Into<String>, f: TriggerFn) {
        self.triggers.insert(name.into(), f);
    }

    pub fn get(&self, name: &str) -> Option<TriggerFn> {
        self.triggers.get(name).copied()
    }

    /// 内置 25 个常用 trigger
    pub fn with_builtins() -> Self {
        let mut r = Self::new();
        // ── 国家级 ──
        r.register("has_war", t_has_war);
        r.register("has_war_with", t_has_war_with);
        r.register("is_in_faction", t_is_in_faction);
        r.register("is_in_faction_with", t_is_in_faction_with);
        r.register("is_faction_leader", t_is_faction_leader);
        r.register("has_government", t_has_government);
        r.register("num_of_factories", t_num_of_factories);
        r.register("has_country_flag", t_has_country_flag);
        r.register("check_variable", t_check_variable);
        r.register("political_power", t_political_power);
        r.register("manpower", t_manpower);
        r.register("stability", t_stability);
        r.register("war_support", t_war_support);
        r.register("has_idea", t_has_idea);
        r.register("has_completed_focus", t_has_completed_focus);
        r.register("has_tech", t_has_tech);
        r.register("controls_state", t_controls_state);
        r.register("owns_state", t_owns_state);
        r.register("is_subject_of", t_is_subject_of);
        r.register("is_puppet", t_is_puppet);
        r.register("tag", t_tag);

        // ── State 级 ──
        r.register("is_owned_by", t_is_owned_by);
        r.register("is_controlled_by", t_is_controlled_by);
        r.register("is_core_of", t_is_core_of);
        r.register("infrastructure", t_infrastructure);
        r.register("has_state_flag", t_has_state_flag);

        // ── 全局级 ──
        r.register("has_global_flag", t_has_global_flag);
        r.register("threat", t_world_tension); // 别名
        r.register("world_tension", t_world_tension);

        r
    }
}

/// 求值一个 trigger block。`block` 整体语义为 AND。
///
/// 顶层 entry 的 key 可能是：
/// - 逻辑组合：`AND` / `OR` / `NOT`
/// - 注册的 trigger（`has_war_with` / `num_of_factories`）
/// - scope changer（`any_country` / `owner` / 等；放在 [`scope_change_trigger`]）
///
/// 未识别的 key 视为 false（保守）。
pub fn eval_trigger_block(
    block: &Block,
    world: &World,
    data: &GameData,
    vars: &Variables,
    flags: &Flags,
    chain: &ScopeChain,
    registry: &TriggerRegistry,
) -> bool {
    for entry in &block.entries {
        let op = Compare::from_operator(&entry.op);
        match entry.key.as_str() {
            // 逻辑组合
            "AND" | "and" => {
                if let Value::Block(b) = &entry.value {
                    if !eval_trigger_block(b, world, data, vars, flags, chain, registry) {
                        return false;
                    }
                }
            }
            "OR" | "or" => {
                if let Value::Block(b) = &entry.value {
                    if !eval_or_block(b, world, data, vars, flags, chain, registry) {
                        return false;
                    }
                }
            }
            "NOT" | "not" => {
                if let Value::Block(b) = &entry.value {
                    // NOT = { ... } 内部的 entries 间也是 AND；NOT 取反
                    if eval_trigger_block(b, world, data, vars, flags, chain, registry) {
                        return false;
                    }
                }
            }
            // scope 切换 trigger
            "any_country"
            | "all_country"
            | "any_other_country"
            | "any_state"
            | "all_state"
            | "any_owned_state"
            | "any_controlled_state"
            | "owner"
            | "controller"
            | "FROM"
            | "ROOT"
            | "PREV"
            | "THIS" => {
                if !eval_scope_change_trigger(
                    &entry.key,
                    &entry.value,
                    world,
                    data,
                    vars,
                    flags,
                    chain,
                    registry,
                ) {
                    return false;
                }
            }
            // 注册的 trigger
            name => {
                let Some(f) = registry.get(name) else {
                    // 未知 trigger 视为 false
                    return false;
                };
                let input = TriggerInput {
                    world,
                    data,
                    vars,
                    flags,
                    chain,
                    op,
                    value: &entry.value,
                };
                if !f(&input) {
                    return false;
                }
            }
        }
    }
    true
}

/// OR 块：任一 entry 为真即真
fn eval_or_block(
    block: &Block,
    world: &World,
    data: &GameData,
    vars: &Variables,
    flags: &Flags,
    chain: &ScopeChain,
    registry: &TriggerRegistry,
) -> bool {
    for entry in &block.entries {
        // 用 single-entry block 包装，复用 eval_trigger_block 的语义
        let single = Block {
            entries: vec![entry.clone()],
            values: vec![],
        };
        if eval_trigger_block(&single, world, data, vars, flags, chain, registry) {
            return true;
        }
    }
    false
}

/// scope 切换 trigger（some_scope = { trigger... }）
fn eval_scope_change_trigger(
    key: &str,
    value: &Value,
    world: &World,
    data: &GameData,
    vars: &Variables,
    flags: &Flags,
    chain: &ScopeChain,
    registry: &TriggerRegistry,
) -> bool {
    let inner = match value {
        Value::Block(b) => b,
        _ => return false,
    };

    // FROM / ROOT / PREV / THIS 是直接切到对应 scope
    let mut new_chain = chain.clone();
    let switched_scope = match key {
        "FROM" => Some(chain.from()),
        "ROOT" => Some(chain.root()),
        "THIS" => Some(chain.this()),
        "PREV" => Some(chain.prev(1)),
        _ => None,
    };
    if let Some(s) = switched_scope {
        new_chain.push(s);
        return eval_trigger_block(inner, world, data, vars, flags, &new_chain, registry);
    }

    // any_X / all_X / owner / controller — 需要按 list 迭代
    let countries = collect_country_iter(key, world, chain);
    let states = collect_state_iter(key, world, chain);

    if !countries.is_empty() {
        let any = key.starts_with("any_");
        if any {
            for c in countries {
                let mut nc = chain.clone();
                nc.push(Scope::Country(c));
                if eval_trigger_block(inner, world, data, vars, flags, &nc, registry) {
                    return true;
                }
            }
            false
        } else {
            // all_country
            for c in countries {
                let mut nc = chain.clone();
                nc.push(Scope::Country(c));
                if !eval_trigger_block(inner, world, data, vars, flags, &nc, registry) {
                    return false;
                }
            }
            true
        }
    } else if !states.is_empty() {
        let any = key.starts_with("any_");
        if any {
            for s in states {
                let mut nc = chain.clone();
                nc.push(Scope::State(s));
                if eval_trigger_block(inner, world, data, vars, flags, &nc, registry) {
                    return true;
                }
            }
            false
        } else {
            for s in states {
                let mut nc = chain.clone();
                nc.push(Scope::State(s));
                if !eval_trigger_block(inner, world, data, vars, flags, &nc, registry) {
                    return false;
                }
            }
            true
        }
    } else {
        // owner / controller — single-target scope change
        match key {
            "owner" => {
                let target = match chain.this() {
                    Scope::State(s) => Some(Scope::Country(world.state_owner(s))),
                    Scope::Province(p) => Some(Scope::Country(world.province_owner(p))),
                    _ => None,
                };
                if let Some(s) = target {
                    let mut nc = chain.clone();
                    nc.push(s);
                    return eval_trigger_block(inner, world, data, vars, flags, &nc, registry);
                }
                false
            }
            "controller" => {
                let target = match chain.this() {
                    Scope::State(s) => world
                        .states
                        .controllers
                        .get(s.0 as usize)
                        .copied()
                        .map(Scope::Country),
                    _ => None,
                };
                if let Some(s) = target {
                    let mut nc = chain.clone();
                    nc.push(s);
                    return eval_trigger_block(inner, world, data, vars, flags, &nc, registry);
                }
                false
            }
            _ => false,
        }
    }
}

fn collect_country_iter(key: &str, world: &World, chain: &ScopeChain) -> Vec<CountryId> {
    match key {
        "any_country" | "all_country" => (0..world.countries.count)
            .map(|i| CountryId(i as u16))
            .collect(),
        "any_other_country" => {
            let me = chain.this().as_country();
            (0..world.countries.count)
                .map(|i| CountryId(i as u16))
                .filter(|&c| Some(c) != me)
                .collect()
        }
        _ => vec![],
    }
}

fn collect_state_iter(key: &str, world: &World, chain: &ScopeChain) -> Vec<StateId> {
    match key {
        "any_state" | "all_state" => (0..world.states.count).map(|i| StateId(i as u16)).collect(),
        "any_owned_state" => {
            let me = chain.this().as_country();
            (0..world.states.count)
                .filter(|&i| Some(world.states.owners[i]) == me)
                .map(|i| StateId(i as u16))
                .collect()
        }
        "any_controlled_state" => {
            let me = chain.this().as_country();
            (0..world.states.count)
                .filter(|&i| Some(world.states.controllers[i]) == me)
                .map(|i| StateId(i as u16))
                .collect()
        }
        _ => vec![],
    }
}

// ─── 内置 trigger 实现 ────────────────────────────────────────

fn this_country(input: &TriggerInput<'_>) -> Option<CountryId> {
    input.chain.this().as_country()
}

fn this_state(input: &TriggerInput<'_>) -> Option<StateId> {
    input.chain.this().as_state()
}

fn require_yes_no(input: &TriggerInput<'_>) -> Option<bool> {
    crate::value::ScriptValue::from(input.value).as_bool()
}

fn require_str<'a>(value: &'a Value) -> Option<&'a str> {
    match value {
        Value::String(s) => Some(s.as_str()),
        _ => None,
    }
}

fn require_num(value: &Value) -> Option<f64> {
    match value {
        Value::Integer(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        _ => None,
    }
}

fn t_has_war(input: &TriggerInput<'_>) -> bool {
    let want = require_yes_no(input).unwrap_or(true);
    let Some(c) = this_country(input) else {
        return false;
    };
    input.world.diplomacy.is_at_war(c) == want
}

fn t_has_war_with(input: &TriggerInput<'_>) -> bool {
    let Some(me) = this_country(input) else {
        return false;
    };
    let Some(other_tag) = require_str(input.value) else {
        return false;
    };
    let Some(other) = input.world.country(other_tag) else {
        return false;
    };
    input.world.diplomacy.at_war_with(me, other)
}

fn t_is_in_faction(input: &TriggerInput<'_>) -> bool {
    let want = require_yes_no(input).unwrap_or(true);
    let Some(me) = this_country(input) else {
        return false;
    };
    input.world.diplomacy.faction_of(me).is_some() == want
}

fn t_is_in_faction_with(input: &TriggerInput<'_>) -> bool {
    let Some(me) = this_country(input) else {
        return false;
    };
    let Some(other_tag) = require_str(input.value) else {
        return false;
    };
    let Some(other) = input.world.country(other_tag) else {
        return false;
    };
    let Some(my_f) = input.world.diplomacy.faction_of(me) else {
        return false;
    };
    input.world.diplomacy.faction_of(other) == Some(my_f)
}

fn t_is_faction_leader(input: &TriggerInput<'_>) -> bool {
    let want = require_yes_no(input).unwrap_or(true);
    let Some(me) = this_country(input) else {
        return false;
    };
    let leader = input
        .world
        .diplomacy
        .faction_of(me)
        .and_then(|fid| input.world.diplomacy.faction(fid))
        .map(|f| f.leader == me)
        .unwrap_or(false);
    leader == want
}

fn t_has_government(input: &TriggerInput<'_>) -> bool {
    let Some(me) = this_country(input) else {
        return false;
    };
    let Some(want) = require_str(input.value) else {
        return false;
    };
    let i = me.0 as usize;
    if i >= input.world.countries.count {
        return false;
    }
    input.world.countries.ruling_party[i] == want
}

fn t_num_of_factories(input: &TriggerInput<'_>) -> bool {
    let Some(me) = this_country(input) else {
        return false;
    };
    let Some(rhs) = require_num(input.value) else {
        return false;
    };
    let (civ, mil, dock) = input.world.country_industry(me);
    let total = (civ + mil + dock) as f64;
    input.op.apply(total, rhs)
}

fn t_has_country_flag(input: &TriggerInput<'_>) -> bool {
    let Some(me) = this_country(input) else {
        return false;
    };
    let Some(name) = require_str(input.value) else {
        return false;
    };
    input.flags.has_country_flag(me, name)
}

fn t_check_variable(input: &TriggerInput<'_>) -> bool {
    // 形式 1：check_variable = { var = X value = Y compare = greater_than }
    // 形式 2：check_variable = { foo > 5 }（不规范但常见）
    let Value::Block(b) = input.value else {
        return false;
    };
    let var_name = b
        .get_string("var")
        .or_else(|| b.entries.first().map(|e| e.key.as_str()))
        .map(|s| s.to_owned());
    let Some(var_name) = var_name else {
        return false;
    };

    let value = b.get_float("value").or_else(|| {
        b.entries.first().and_then(|e| match &e.value {
            Value::Integer(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            _ => None,
        })
    });
    let Some(rhs) = value else {
        return false;
    };

    let compare = b
        .get_string("compare")
        .map(|s| match s {
            "greater_than" => Compare::Gt,
            "less_than" => Compare::Lt,
            "greater_than_or_equals" => Compare::Ge,
            "less_than_or_equals" => Compare::Le,
            "equals" | "equal" => Compare::Eq,
            "not_equal" => Compare::Ne,
            _ => Compare::Eq,
        })
        .unwrap_or_else(|| {
            // 用 entry 的运算符
            b.entries
                .first()
                .map(|e| Compare::from_operator(&e.op))
                .unwrap_or(Compare::Eq)
        });

    let lhs = match input.chain.this() {
        Scope::Country(c) => input.vars.get_country(c, &var_name),
        Scope::State(s) => input.vars.get_state(s, &var_name),
        _ => input.vars.get_global(&var_name),
    };
    compare.apply(lhs, rhs)
}

fn t_political_power(input: &TriggerInput<'_>) -> bool {
    let Some(me) = this_country(input) else {
        return false;
    };
    let Some(rhs) = require_num(input.value) else {
        return false;
    };
    let i = me.0 as usize;
    let pp = input.world.countries.political_power[i] as f64;
    input.op.apply(pp, rhs)
}

fn t_manpower(input: &TriggerInput<'_>) -> bool {
    let Some(me) = this_country(input) else {
        return false;
    };
    let Some(rhs) = require_num(input.value) else {
        return false;
    };
    let mp = input.world.manpower(me) as f64;
    input.op.apply(mp, rhs)
}

fn t_stability(input: &TriggerInput<'_>) -> bool {
    let Some(me) = this_country(input) else {
        return false;
    };
    let Some(rhs) = require_num(input.value) else {
        return false;
    };
    let v = input.world.countries.stability[me.0 as usize] as f64;
    input.op.apply(v, rhs)
}

fn t_war_support(input: &TriggerInput<'_>) -> bool {
    let Some(me) = this_country(input) else {
        return false;
    };
    let Some(rhs) = require_num(input.value) else {
        return false;
    };
    let v = input.world.countries.war_support[me.0 as usize] as f64;
    input.op.apply(v, rhs)
}

fn t_has_idea(input: &TriggerInput<'_>) -> bool {
    let Some(me) = this_country(input) else {
        return false;
    };
    let Some(name) = require_str(input.value) else {
        return false;
    };
    input.world.countries.ideas[me.0 as usize]
        .iter()
        .any(|i| i == name)
}

fn t_has_completed_focus(input: &TriggerInput<'_>) -> bool {
    let Some(me) = this_country(input) else {
        return false;
    };
    let Some(name) = require_str(input.value) else {
        return false;
    };
    input.world.countries.completed_focuses[me.0 as usize].contains(name)
}

fn t_has_tech(input: &TriggerInput<'_>) -> bool {
    let Some(me) = this_country(input) else {
        return false;
    };
    let Some(name) = require_str(input.value) else {
        return false;
    };
    input.world.countries.completed_techs[me.0 as usize]
        .iter()
        .any(|t| t == name)
}

fn t_controls_state(input: &TriggerInput<'_>) -> bool {
    let Some(me) = this_country(input) else {
        return false;
    };
    let Some(rhs) = require_num(input.value) else {
        return false;
    };
    let game_id = rhs as u16;
    let Some(&sid) = input.world.state_id_lookup.get(&game_id) else {
        return false;
    };
    input.world.states.controllers[sid.0 as usize] == me
}

fn t_owns_state(input: &TriggerInput<'_>) -> bool {
    let Some(me) = this_country(input) else {
        return false;
    };
    let Some(rhs) = require_num(input.value) else {
        return false;
    };
    let game_id = rhs as u16;
    let Some(&sid) = input.world.state_id_lookup.get(&game_id) else {
        return false;
    };
    input.world.states.owners[sid.0 as usize] == me
}

fn t_is_subject_of(input: &TriggerInput<'_>) -> bool {
    let Some(me) = this_country(input) else {
        return false;
    };
    let Some(other_tag) = require_str(input.value) else {
        return false;
    };
    let Some(other) = input.world.country(other_tag) else {
        return false;
    };
    input.world.diplomacy.is_subject_of(me, other)
}

fn t_is_puppet(input: &TriggerInput<'_>) -> bool {
    let want = require_yes_no(input).unwrap_or(true);
    let Some(me) = this_country(input) else {
        return false;
    };
    input.world.diplomacy.autonomy.contains_key(&me) == want
}

fn t_tag(input: &TriggerInput<'_>) -> bool {
    let Some(me) = this_country(input) else {
        return false;
    };
    let Some(want) = require_str(input.value) else {
        return false;
    };
    input
        .world
        .country_tag(me)
        .map(|s| s == want)
        .unwrap_or(false)
}

// ─── State 级 ──

fn t_is_owned_by(input: &TriggerInput<'_>) -> bool {
    let Some(s) = this_state(input) else {
        return false;
    };
    let Some(tag) = require_str(input.value) else {
        return false;
    };
    let Some(c) = input.world.country(tag) else {
        return false;
    };
    input.world.states.owners[s.0 as usize] == c
}

fn t_is_controlled_by(input: &TriggerInput<'_>) -> bool {
    let Some(s) = this_state(input) else {
        return false;
    };
    let Some(tag) = require_str(input.value) else {
        return false;
    };
    let Some(c) = input.world.country(tag) else {
        return false;
    };
    input.world.states.controllers[s.0 as usize] == c
}

fn t_is_core_of(input: &TriggerInput<'_>) -> bool {
    let Some(s) = this_state(input) else {
        return false;
    };
    let Some(tag) = require_str(input.value) else {
        return false;
    };
    let Some(c) = input.world.country(tag) else {
        return false;
    };
    input.world.states.cores[s.0 as usize]
        .iter()
        .any(|&x| x == c)
}

fn t_infrastructure(input: &TriggerInput<'_>) -> bool {
    let Some(s) = this_state(input) else {
        return false;
    };
    let Some(rhs) = require_num(input.value) else {
        return false;
    };
    let v = input.world.states.infrastructure[s.0 as usize] as f64;
    input.op.apply(v, rhs)
}

fn t_has_state_flag(input: &TriggerInput<'_>) -> bool {
    let Some(s) = this_state(input) else {
        return false;
    };
    let Some(name) = require_str(input.value) else {
        return false;
    };
    input.flags.has_state_flag(s, name)
}

// ─── 全局 ──

fn t_has_global_flag(input: &TriggerInput<'_>) -> bool {
    let Some(name) = require_str(input.value) else {
        return false;
    };
    input.flags.has_global_flag(name)
}

fn t_world_tension(input: &TriggerInput<'_>) -> bool {
    let Some(rhs) = require_num(input.value) else {
        return false;
    };
    input
        .op
        .apply(input.world.diplomacy.world_tension as f64, rhs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_25_plus_triggers() {
        let r = TriggerRegistry::with_builtins();
        assert!(r.triggers.len() >= 25);
        assert!(r.get("has_war_with").is_some());
        assert!(r.get("num_of_factories").is_some());
        assert!(r.get("has_country_flag").is_some());
    }
}
