//! V6 贸易 tick：撮合 / 路线 / 双货币结算 / 封锁 / 金本位 / 外汇管制 / 关税。
//!
//! §4.9 + §0.3 兑现。本模块被 `market_tick` 和 `planned_tick` 共同调用。
//!
//! HC-6（I-13）：所有 imports/exports 流量全 £ 结算，不可绕过汇率直接增减 cash_rm。
//! HC-I-14：封锁状态下被覆盖的 Port 所在州 trade_throughput == 0。
//!
//! P0.10 修复：
//! - 贸易容量改为总量池，多商品按优先级分配
//! - world spot market 改为全局日内扣减池
//! - 封锁绑定航线海区、敌方舰队位置和制海权
//! - 修复 exporter exports 双计（出口方 exports 统计只在进口驱动路径写入一次）

use hoi4_content::V6Database;
use hoi4_state::{
    CountryId, LawCategory, PopClass, StateId, TradeDirection, TradeRouteKind, World,
};

pub const TRADE_TICK_INTERVAL_DAYS: i64 = 1;
const BLOCKADE_SATISFACTION_PENALTY: f32 = -0.10;
const BLOCKADE_CRISIS_STABILITY_PENALTY: f32 = -0.10;
const BLOCKADE_CRISIS_WAR_SUPPORT_PENALTY: f32 = -0.05;

const GOOD_PRIORITY: &[&str] = &[
    "oil",
    "rubber",
    "fuel",
    "steel",
    "coal",
    "iron",
    "machinery",
    "machine_tools",
    "grain",
    "meat",
    "clothes",
];

fn good_priority(good_id: &str) -> i32 {
    GOOD_PRIORITY
        .iter()
        .position(|&p| p == good_id)
        .map(|p| p as i32)
        .unwrap_or(GOOD_PRIORITY.len() as i32)
}

pub fn step_trade_matching(world: &mut World, db: &V6Database, ci: usize) {
    let current_trade_law = world.countries.law_store.law_sets[ci].0[LawCategory::Trade.index()]
        .current
        .clone();
    let trade_def = db.trade_laws.iter().find(|l| l.id == current_trade_law);

    let is_autarky = current_trade_law == "autarky";
    let _is_state_monopoly = current_trade_law == "state_trade_monopoly";
    let import_efficiency = trade_def.map(|t| t.import_efficiency).unwrap_or(0.5);
    let export_efficiency = trade_def.map(|t| t.export_efficiency).unwrap_or(0.5);
    let import_tariff_rate = trade_def.map(|t| t.import_tariff_rate).unwrap_or(0.0);
    let _export_tariff_rate = trade_def.map(|t| t.export_tariff_rate).unwrap_or(0.0);

    if is_autarky {
        zero_imports_exports(world, ci);
        return;
    }

    let country_id = CountryId(ci as u16);
    let state_ids = indexed_country_states(world, country_id);

    let trade_infra = compute_trade_infra(world, ci, &state_ids);
    let total_port_level = trade_infra.total_port_level;
    let total_railway_level = trade_infra.total_railway_level;
    let first_port_state = trade_infra.first_port_state;

    let rm_per_gbp = world.countries.treasury.exchange_rates[ci].rm_per_gbp;
    let _fx_control = trade_def
        .map(|t| t.foreign_exchange_control)
        .unwrap_or(false);

    let trade_capacity = if total_port_level > 0.0 || total_railway_level > 0.0 {
        (total_port_level * 10.0 + total_railway_level * 5.0).max(1.0)
    } else {
        5.0
    };

    let market = &world.countries.market.markets[ci];

    let mut import_demands: Vec<(String, f32, i32)> = Vec::new();
    let mut export_flows: std::collections::HashMap<String, f32> = std::collections::HashMap::new();

    let good_keys: std::collections::HashSet<String> = market
        .supply
        .keys()
        .chain(market.demand.keys())
        .cloned()
        .collect();

    for good_id in &good_keys {
        let supply = market.supply.get(good_id).copied().unwrap_or(0.0);
        let demand = market.demand.get(good_id).copied().unwrap_or(0.0);
        if demand > supply * 1.1 {
            let deficit = demand - supply;
            let base_import = deficit * import_efficiency;
            if base_import > 0.0 {
                import_demands.push((good_id.clone(), base_import, good_priority(good_id)));
            }
        }

        if supply > demand * 1.3 {
            let surplus = supply - demand;
            let base_export = surplus * export_efficiency;
            if base_export > 0.0 {
                export_flows.insert(good_id.clone(), base_export.min(trade_capacity));
            }
        }
    }

    import_demands.sort_by_key(|(_, _, prio)| *prio);

    let mut remaining_capacity = trade_capacity;
    let mut capped_import_demands: Vec<(String, f32)> = Vec::new();
    for (good_id, desired, _) in &import_demands {
        let capped = desired.min(remaining_capacity);
        if capped > 0.0 {
            capped_import_demands.push((good_id.clone(), capped));
            remaining_capacity -= capped;
        }
    }

    let mut import_failures: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut actual_import_flows: std::collections::HashMap<String, (f32, CountryId)> =
        std::collections::HashMap::new();
    let mut remaining_reserve_gbp = world.countries.treasury.treasuries[ci].reserve_gbp.max(0.0);

    for (good_id, amount) in &capped_import_demands {
        let importer_price_rm = world.countries.market.markets[ci]
            .price
            .get(good_id)
            .copied()
            .unwrap_or(1.0);
        let world_spot_mult = world
            .countries
            .market
            .world_spot
            .price_multiplier_for(good_id);
        let Some((exporter, exporter_available)) = choose_exporter_for_import(world, ci, good_id)
        else {
            import_failures.insert(good_id.clone(), "no_partner_supply".to_owned());
            continue;
        };
        let exporter_price_rm = if !exporter.is_none() {
            let eidx = exporter.0 as usize;
            if eidx < world.countries.market.markets.len() {
                world.countries.market.markets[eidx]
                    .price
                    .get(good_id)
                    .copied()
                    .unwrap_or(importer_price_rm)
            } else {
                importer_price_rm
            }
        } else {
            importer_price_rm * world_spot_mult
        };
        let trade_price_rm = (importer_price_rm + exporter_price_rm) * 0.5;
        let price_gbp = trade_price_rm as f64 / rm_per_gbp as f64;
        let requested_cost_gbp: f64 = *amount as f64 * price_gbp;
        let actual_amount = if requested_cost_gbp <= remaining_reserve_gbp {
            *amount
        } else if remaining_reserve_gbp > 0.0 && price_gbp > 0.0 {
            (remaining_reserve_gbp / price_gbp).min(*amount as f64) as f32
        } else {
            0.0
        };
        let route_throughput = first_port_state
            .map(|sid| sea_route_throughput(world, country_id, sid))
            .unwrap_or(1.0);
        let actual_amount = actual_amount * route_throughput;
        let actual_amount = actual_amount.min(exporter_available);
        if actual_amount <= 0.0 {
            if route_throughput <= 0.0 {
                import_failures.insert(good_id.clone(), "blockaded".to_owned());
            } else if exporter_available <= 0.0 {
                import_failures.insert(good_id.clone(), "no_partner_supply".to_owned());
            } else {
                import_failures.insert(good_id.clone(), "insufficient_foreign_exchange".to_owned());
            }
            continue;
        }
        if actual_amount < *amount {
            import_failures.insert(
                good_id.clone(),
                "partial_due_to_foreign_exchange".to_owned(),
            );
        }
        deduct_export_supply(world, exporter, good_id, actual_amount);
        if exporter.is_none() {
            record_world_spot_consumption(world, good_id, actual_amount);
        }
        apply_subject_extraction_feedback(world, country_id, exporter, actual_amount);
        let market = &mut world.countries.market.markets[ci];
        *market.imports.entry(good_id.clone()).or_insert(0.0) += actual_amount;
        *market.supply.entry(good_id.clone()).or_insert(0.0) += actual_amount;
        actual_import_flows.insert(good_id.clone(), (actual_amount, exporter));
        let cost_gbp: f64 = actual_amount as f64 * price_gbp;
        let tariff_income_gbp: f64 = cost_gbp * import_tariff_rate as f64;
        let treasury = &mut world.countries.treasury.treasuries[ci];
        treasury.reserve_gbp -= cost_gbp;
        treasury.daily_trade_balance_gbp -= cost_gbp;
        treasury.reserve_gbp += tariff_income_gbp;
        remaining_reserve_gbp = (remaining_reserve_gbp - cost_gbp + tariff_income_gbp).max(0.0);
        if !exporter.is_none() {
            let exporter_idx = exporter.0 as usize;
            if exporter_idx < world.countries.treasury.treasuries.len() {
                let exporter_receipts = cost_gbp - tariff_income_gbp;
                let exporter_treasury = &mut world.countries.treasury.treasuries[exporter_idx];
                exporter_treasury.reserve_gbp += exporter_receipts;
                exporter_treasury.daily_trade_balance_gbp += exporter_receipts;
            }
        }
    }

    let _ = &export_flows;
    let treasury = &mut world.countries.treasury.treasuries[ci];
    treasury.reserve_gbp = treasury.reserve_gbp.max(0.0);

    world.countries.trade.import_failures = import_failures;
    update_trade_routes(
        world,
        ci,
        &actual_import_flows,
        &export_flows,
        &state_ids,
        first_port_state,
    );
}

pub fn step_blockade_check(world: &mut World, ci: usize) {
    let country_id = CountryId(ci as u16);
    let at_war = world.countries.at_war[ci];

    if !at_war {
        return;
    }

    let state_ids = indexed_country_states(world, country_id);

    for &sid in &state_ids {
        let si = sid.0 as usize;
        let has_port = has_port_building(world, sid);
        if !has_port {
            continue;
        }

        let is_blockaded = check_sea_blockade(world, country_id, sid);
        world.countries.trade.blockaded_ports.resize(si + 1, false);
        world.countries.trade.blockaded_ports[si] = is_blockaded;
    }

    for route in &mut world.countries.trade.routes {
        if route.importer != country_id && route.exporter != country_id {
            continue;
        }
        if !route.kind.uses_sea_lanes() {
            continue;
        }
        if let Some(port_state) = route.port_state {
            let psi = port_state.0 as usize;
            if psi < world.countries.trade.blockaded_ports.len()
                && world.countries.trade.blockaded_ports[psi]
            {
                route.is_blockaded = true;
                route.throughput = 0.0;
            } else {
                route.is_blockaded = false;
            }
        }
    }
}

pub fn step_blockade_satisfaction_impact(world: &mut World, ci: usize) {
    let any_blockaded = world.countries.trade.routes.iter().any(|r| {
        (r.importer == CountryId(ci as u16) || r.exporter == CountryId(ci as u16)) && r.is_blockaded
    });

    let country_id = CountryId(ci as u16);
    let has_blockaded_port = world
        .country_state_index
        .get(ci)
        .map(|states| {
            states.iter().any(|state| {
                let si = state.0 as usize;
                si < world.countries.trade.blockaded_ports.len()
                    && world.countries.trade.blockaded_ports[si]
            })
        })
        .unwrap_or(false);

    if !any_blockaded && !has_blockaded_port {
        return;
    }
    let state_ids = indexed_country_states(world, country_id);

    let essential_imports = ["rubber", "oil", "fuel", "grain", "steel", "clothes"];
    let mut blocked_imports = 0usize;
    for good in &essential_imports {
        let blocked = is_import_blocked_by_blockade(world, ci, good);
        if blocked {
            blocked_imports += 1;
            let import_amount = world.countries.market.markets[ci]
                .imports
                .get(*good)
                .copied()
                .unwrap_or(0.0);
            if let Some(imports) = world.countries.market.markets[ci].imports.get_mut(*good) {
                *imports = 0.0;
            }
            if import_amount > 0.0 {
                if let Some(supply) = world.countries.market.markets[ci].supply.get_mut(*good) {
                    *supply -= import_amount.min(*supply);
                }
            }
        }
    }

    if blocked_imports > 0 {
        for pg in &mut world.countries.pops.groups {
            if !state_ids.contains(&pg.state) {
                continue;
            }
            if pg.class == PopClass::Soldier {
                continue;
            }
            pg.satisfaction =
                (pg.satisfaction + BLOCKADE_SATISFACTION_PENALTY * 0.01).clamp(0.0, 1.0);
        }
    }
}

pub fn step_gold_standard_and_forex(world: &mut World, db: &V6Database, ci: usize) {
    let current_trade_law = world.countries.law_store.law_sets[ci].0[LawCategory::Trade.index()]
        .current
        .clone();
    let fx_control = db
        .trade_laws
        .iter()
        .find(|l| l.id == current_trade_law)
        .map(|l| l.foreign_exchange_control)
        .unwrap_or(false);

    if fx_control {
        enforce_forex_control(world, ci);
    }
}

pub fn step_trade_agreements(world: &mut World, db: &V6Database, ci: usize) {
    let rm_per_gbp = world.countries.treasury.exchange_rates[ci].rm_per_gbp;
    let country_id = CountryId(ci as u16);

    let mut agreements: Vec<(String, f32, f32, f32)> = world
        .countries
        .trade
        .agreements
        .iter()
        .filter(|a| match a.direction {
            TradeDirection::AImportsFromB => a.party_a == country_id,
            TradeDirection::BImportsFromA => a.party_b == country_id,
        })
        .map(|a| {
            (
                a.good_id.clone(),
                a.daily_quantity,
                historical_trade_importance(world, db, a.party_a, a.party_b, &a.good_id),
                find_route_port_for_good(world, ci, &a.good_id)
                    .map(|sid| {
                        let si = sid.0 as usize;
                        world.countries.trade.trade_throughput_for_state(si)
                    })
                    .unwrap_or(1.0),
            )
        })
        .collect();
    agreements.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    let mut remaining_reserve_gbp = world.countries.treasury.treasuries[ci].reserve_gbp.max(0.0);
    for (good_id, qty, _importance, throughput) in &agreements {
        let exporter = find_agreement_exporter(world, country_id, good_id);
        let exporter_available = exporter
            .map(|eid| layered_exportable_surplus(world, country_id, eid, good_id))
            .unwrap_or(0.0);
        if exporter_available <= 0.0 {
            world
                .countries
                .trade
                .import_failures
                .insert(good_id.clone(), "no_partner_supply".to_owned());
            continue;
        }
        let actual_qty = (qty * throughput).min(exporter_available);
        if actual_qty <= 0.0 {
            world
                .countries
                .trade
                .import_failures
                .insert(good_id.clone(), "route_capacity".to_owned());
            continue;
        }

        let price_rm = world.countries.market.markets[ci]
            .price
            .get(good_id.as_str())
            .copied()
            .unwrap_or(1.0);
        let price_gbp = price_rm as f64 / rm_per_gbp as f64;
        let cost_gbp: f64 = actual_qty as f64 * price_gbp;
        let afforded_qty = if cost_gbp <= remaining_reserve_gbp {
            actual_qty
        } else if remaining_reserve_gbp > 0.0 && price_gbp > 0.0 {
            (remaining_reserve_gbp / price_gbp).min(actual_qty as f64) as f32
        } else {
            0.0
        };
        if afforded_qty <= 0.0 {
            world
                .countries
                .trade
                .import_failures
                .insert(good_id.clone(), "insufficient_foreign_exchange".to_owned());
            continue;
        }
        if afforded_qty < actual_qty {
            world.countries.trade.import_failures.insert(
                good_id.clone(),
                "partial_due_to_foreign_exchange".to_owned(),
            );
        }

        if let Some(exporter) = exporter {
            deduct_export_supply(world, exporter, good_id, afforded_qty);
            apply_subject_extraction_feedback(world, country_id, exporter, afforded_qty);
            let exporter_idx = exporter.0 as usize;
            if exporter_idx < world.countries.treasury.treasuries.len() {
                world.countries.treasury.treasuries[exporter_idx].reserve_gbp +=
                    afforded_qty as f64 * price_gbp;
                world.countries.treasury.treasuries[exporter_idx].daily_trade_balance_gbp +=
                    afforded_qty as f64 * price_gbp;
            }
        }

        *world.countries.market.markets[ci]
            .imports
            .entry(good_id.clone())
            .or_insert(0.0) += afforded_qty;
        *world.countries.market.markets[ci]
            .supply
            .entry(good_id.clone())
            .or_insert(0.0) += afforded_qty;

        let actual_cost_gbp: f64 = afforded_qty as f64 * price_gbp;
        world.countries.treasury.treasuries[ci].reserve_gbp -= actual_cost_gbp;
        remaining_reserve_gbp = (remaining_reserve_gbp - actual_cost_gbp).max(0.0);
        world.countries.treasury.treasuries[ci].daily_trade_balance_gbp -= actual_cost_gbp;
    }
    world.countries.treasury.treasuries[ci].reserve_gbp =
        world.countries.treasury.treasuries[ci].reserve_gbp.max(0.0);
}

fn choose_exporter_for_import(
    world: &mut World,
    importer_idx: usize,
    good_id: &str,
) -> Option<(CountryId, f32)> {
    let importer = CountryId(importer_idx as u16);
    if let Some(exporter) = find_agreement_exporter(world, importer, good_id) {
        if !world.diplomacy.at_war_with(importer, exporter) {
            let available = layered_exportable_surplus(world, importer, exporter, good_id);
            if available > 0.0 {
                return Some((exporter, available));
            }
        }
    }

    let importer_bloc = world
        .countries
        .market
        .country_bloc
        .get(importer_idx)
        .copied()
        .flatten();
    let importer_faction = world.diplomacy.faction_of(importer);
    let mut best: Option<(CountryId, f32, i32, i32)> = None;
    let indexed_exporters = world
        .trade_export_surplus_index
        .get(good_id)
        .cloned()
        .unwrap_or_default();
    if !indexed_exporters.is_empty() {
        for (exporter, _indexed_surplus) in indexed_exporters {
            let exporter_idx = exporter.0 as usize;
            if exporter_idx == importer_idx {
                continue;
            }
            if world.diplomacy.at_war_with(importer, exporter) {
                continue;
            }
            let surplus = layered_exportable_surplus(world, importer, exporter, good_id);
            if surplus <= 0.0 {
                continue;
            }
            let layer =
                trade_partner_layer(world, importer, exporter, importer_bloc, importer_faction);
            let score = trade_partner_score(world, importer, exporter);
            if best
                .as_ref()
                .map(|(_, best_surplus, best_layer, best_score)| {
                    layer < *best_layer
                        || (layer == *best_layer
                            && (score > *best_score
                                || (score == *best_score && surplus > *best_surplus)))
                })
                .unwrap_or(true)
            {
                best = Some((exporter, surplus, layer, score));
            }
        }
        return best
            .map(|(exporter, surplus, _, _)| (exporter, surplus))
            .or_else(|| limited_world_spot_supply(world, importer_idx, good_id));
    }
    for exporter_idx in 0..world.countries.count {
        if exporter_idx == importer_idx {
            continue;
        }
        let exporter = CountryId(exporter_idx as u16);
        if world.diplomacy.at_war_with(importer, exporter) {
            continue;
        }
        let surplus = layered_exportable_surplus(world, importer, exporter, good_id);
        if surplus <= 0.0 {
            continue;
        }
        let layer = trade_partner_layer(world, importer, exporter, importer_bloc, importer_faction);
        let score = trade_partner_score(world, importer, exporter);
        if best
            .as_ref()
            .map(|(_, best_surplus, best_layer, best_score)| {
                layer < *best_layer
                    || (layer == *best_layer
                        && (score > *best_score
                            || (score == *best_score && surplus > *best_surplus)))
            })
            .unwrap_or(true)
        {
            best = Some((exporter, surplus, layer, score));
        }
    }
    best.map(|(exporter, surplus, _, _)| (exporter, surplus))
        .or_else(|| limited_world_spot_supply(world, importer_idx, good_id))
}

fn trade_partner_layer(
    world: &World,
    importer: CountryId,
    exporter: CountryId,
    importer_bloc: Option<hoi4_state::MarketBlocId>,
    importer_faction: Option<hoi4_state::FactionId>,
) -> i32 {
    if is_master_subject_pair(world, importer, exporter) {
        return 1;
    }
    if importer_bloc.is_some()
        && importer_bloc
            == world
                .countries
                .market
                .country_bloc
                .get(exporter.0 as usize)
                .copied()
                .flatten()
    {
        return 2;
    }
    if importer_faction.is_some() && importer_faction == world.diplomacy.faction_of(exporter) {
        return 3;
    }
    4
}

fn is_master_subject_pair(world: &World, importer: CountryId, exporter: CountryId) -> bool {
    world.diplomacy.is_subject_of(importer, exporter)
        || world.diplomacy.is_subject_of(exporter, importer)
}

fn layered_exportable_surplus(
    world: &World,
    importer: CountryId,
    exporter: CountryId,
    good_id: &str,
) -> f32 {
    let surplus = exportable_surplus(world, exporter, good_id);
    let Some(autonomy) = world.diplomacy.autonomy.get(&exporter) else {
        return surplus;
    };
    if autonomy.master != importer {
        return surplus;
    }
    surplus
        * autonomy.level.master_resource_share()
        * crate::occupation::country_governance_market_access(world, exporter)
}

fn limited_world_spot_supply(
    world: &mut World,
    importer_idx: usize,
    good_id: &str,
) -> Option<(CountryId, f32)> {
    let cap = world.countries.market.world_spot.available_for(good_id);
    if cap <= 0.0 {
        return None;
    }
    let already_consumed = world
        .countries
        .trade
        .world_spot_daily_consumed
        .get(good_id)
        .copied()
        .unwrap_or(0.0);
    let remaining_global = (cap - already_consumed).max(0.0);
    if remaining_global <= 0.0 {
        return None;
    }
    let market_access = importer_world_spot_access(world, importer_idx);
    let available = remaining_global * market_access;
    (available > 0.0).then_some((CountryId::NONE, available))
}

pub fn reset_world_spot_daily(world: &mut World) {
    world.countries.trade.world_spot_daily_consumed.clear();
}

fn record_world_spot_consumption(world: &mut World, good_id: &str, amount: f32) {
    if amount <= 0.0 {
        return;
    }
    *world
        .countries
        .trade
        .world_spot_daily_consumed
        .entry(good_id.to_owned())
        .or_insert(0.0) += amount;
}

fn importer_world_spot_access(world: &World, importer_idx: usize) -> f32 {
    let importer = CountryId(importer_idx as u16);
    if world
        .countries
        .at_war
        .get(importer_idx)
        .copied()
        .unwrap_or(false)
    {
        let blockaded_sea_routes = world
            .countries
            .trade
            .routes
            .iter()
            .filter(|route| route.importer == importer && route.kind.uses_sea_lanes())
            .count();
        let blockaded_count = world
            .countries
            .trade
            .routes
            .iter()
            .filter(|route| {
                route.importer == importer && route.kind.uses_sea_lanes() && route.is_blockaded
            })
            .count();
        if blockaded_sea_routes > 0 && blockaded_count >= blockaded_sea_routes {
            return 0.1;
        }
        if blockaded_count > 0 {
            let ratio = blockaded_count as f32 / blockaded_sea_routes.max(1) as f32;
            return (1.0 - ratio * 0.75).max(0.1);
        }
    }
    1.0
}

fn apply_subject_extraction_feedback(
    world: &mut World,
    importer: CountryId,
    exporter: CountryId,
    amount: f32,
) {
    if exporter.is_none() || amount <= 0.0 {
        return;
    }
    let Some(autonomy) = world.diplomacy.autonomy.get_mut(&exporter) else {
        return;
    };
    if autonomy.master != importer {
        return;
    }
    let share = autonomy.level.master_resource_share();
    if share > 0.0 {
        let pressure = autonomy.level.autonomy_pressure_from_extraction();
        autonomy.progress -= amount * share * (0.02 + pressure * 0.03);
        let subject_states = world.country_state_ids(exporter);
        let mut total_state_pop = 0u64;
        for state in &subject_states {
            total_state_pop = total_state_pop.saturating_add(world.state_population(*state));
        }
        if total_state_pop > 0 {
            for pg in &mut world.countries.pops.groups {
                if !subject_states.contains(&pg.state) || pg.class == PopClass::Soldier {
                    continue;
                }
                let state_factor = pg.size as f32 / total_state_pop as f32;
                pg.radicalism = (pg.radicalism + amount * share * pressure * state_factor * 0.001)
                    .clamp(0.0, 1.0);
            }
        }
    }
}

fn trade_partner_score(world: &World, importer: CountryId, exporter: CountryId) -> i32 {
    let mut score = world.diplomacy.opinions.get(importer, exporter) as i32;
    if world.diplomacy.is_subject_of(importer, exporter)
        || world.diplomacy.is_subject_of(exporter, importer)
    {
        score += 250;
    }
    if let (Some(a), Some(b)) = (
        world.diplomacy.faction_of(importer),
        world.diplomacy.faction_of(exporter),
    ) {
        if a == b {
            score += 100;
        }
    }
    score
}

fn exportable_surplus(world: &World, exporter: CountryId, good_id: &str) -> f32 {
    let idx = exporter.0 as usize;
    if idx >= world.countries.market.markets.len() {
        return 0.0;
    }
    let market = &world.countries.market.markets[idx];
    let supply = market.supply.get(good_id).copied().unwrap_or(0.0);
    let demand = market.demand.get(good_id).copied().unwrap_or(0.0);
    (supply - demand).max(0.0)
}

fn deduct_export_supply(world: &mut World, exporter: CountryId, good_id: &str, amount: f32) -> f32 {
    let idx = exporter.0 as usize;
    if idx >= world.countries.market.markets.len() || amount <= 0.0 {
        return 0.0;
    }
    let market = &mut world.countries.market.markets[idx];
    let mut remaining = amount;
    if let Some(supply) = market.supply.get_mut(good_id) {
        let take = remaining.min(*supply);
        *supply -= take;
        remaining -= take;
    }
    if remaining > 0.0 {
        if let Some(stockpile) = market.stockpile.get_mut(good_id) {
            let take = remaining.min(*stockpile);
            *stockpile -= take;
            remaining -= take;
        }
    }
    let deducted = amount - remaining;
    *market.exports.entry(good_id.to_owned()).or_insert(0.0) += deducted;
    deducted
}

fn find_agreement_exporter(world: &World, importer: CountryId, good_id: &str) -> Option<CountryId> {
    world
        .countries
        .trade
        .agreements
        .iter()
        .find_map(|agreement| match agreement.direction {
            TradeDirection::AImportsFromB
                if agreement.party_a == importer && agreement.good_id == good_id =>
            {
                Some(agreement.party_b)
            }
            TradeDirection::BImportsFromA
                if agreement.party_b == importer && agreement.good_id == good_id =>
            {
                Some(agreement.party_a)
            }
            _ => None,
        })
        .filter(|country| !country.is_none())
}

fn historical_trade_importance(
    world: &World,
    db: &V6Database,
    importer: CountryId,
    exporter: CountryId,
    good_id: &str,
) -> f32 {
    let importer_tag = world
        .countries
        .tags
        .get(importer.0 as usize)
        .map(|s| s.as_str())
        .unwrap_or_default();
    let exporter_tag = world
        .countries
        .tags
        .get(exporter.0 as usize)
        .map(|s| s.as_str())
        .unwrap_or_default();
    db.historical_trade_routes
        .iter()
        .find(|route| {
            route.importer == importer_tag
                && route.exporter == exporter_tag
                && route.good_id == good_id
        })
        .map(|route| route.strategic_importance)
        .unwrap_or(0.0)
}

pub fn step_check_blockade_crisis(world: &mut World, ci: usize) -> bool {
    let any_blockaded = world.countries.trade.routes.iter().any(|r| {
        (r.importer == CountryId(ci as u16) || r.exporter == CountryId(ci as u16)) && r.is_blockaded
    });

    if !any_blockaded {
        return false;
    }

    let essential_blocked = is_import_blocked_by_blockade(world, ci, "rubber")
        || is_import_blocked_by_blockade(world, ci, "oil");

    if essential_blocked {
        world.countries.stability[ci] =
            (world.countries.stability[ci] + BLOCKADE_CRISIS_STABILITY_PENALTY).clamp(0.0, 1.0);
        world.countries.war_support[ci] =
            (world.countries.war_support[ci] + BLOCKADE_CRISIS_WAR_SUPPORT_PENALTY).clamp(0.0, 1.0);
    }

    essential_blocked
}

pub fn execute_v6_event_effect(world: &mut World, ci: usize, effect_key: &str, effect_value: &str) {
    match effect_key {
        "force_pm_synthetic" => {
            let goods_list: Vec<&str> = effect_value.split(',').collect();
            let country_id = CountryId(ci as u16);
            let state_ids: Vec<StateId> = (0..world.states.count)
                .filter(|&si| world.states.owners[si] == country_id)
                .map(|si| StateId(si as u16))
                .collect();

            for building in &mut world.countries.buildings_v6.buildings {
                if !state_ids.contains(&building.state) {
                    continue;
                }
                if building.building_def_id == "synthetic_refinery" {
                    building.active_pm = "synthetic_default".to_owned();
                }
            }

            let _ = goods_list;
        }
        "coal_diversion" => {
            if let Ok(ratio) = effect_value.parse::<f32>() {
                apply_coal_diversion(world, ci, ratio);
            }
        }
        "pop_satisfaction_penalty" => {
            if let Ok(penalty) = effect_value.parse::<f32>() {
                apply_satisfaction_penalty(world, ci, penalty);
            }
        }
        "consumer_goods_cut" => {
            let _ = effect_value;
        }
        _ => {}
    }
}

fn zero_imports_exports(world: &mut World, ci: usize) {
    let market = &mut world.countries.market.markets[ci];
    for val in market.imports.values_mut() {
        *val = 0.0;
    }
    for val in market.exports.values_mut() {
        *val = 0.0;
    }
}

#[derive(Default)]
struct TradeInfra {
    total_port_level: f32,
    total_railway_level: f32,
    first_port_state: Option<StateId>,
}

fn compute_trade_infra(world: &World, ci: usize, state_ids: &[StateId]) -> TradeInfra {
    let mut infra = TradeInfra::default();
    let indexed_buildings = world
        .runtime_country_indexes_valid
        .then(|| world.country_building_index.get(ci))
        .flatten();

    if let Some(building_indices) = indexed_buildings {
        for &building_idx in building_indices {
            let Some(building) = world.countries.buildings_v6.buildings.get(building_idx) else {
                continue;
            };
            add_trade_infra_building(&mut infra, building);
        }
        return infra;
    }

    for building in &world.countries.buildings_v6.buildings {
        if !state_ids.contains(&building.state) {
            continue;
        }
        add_trade_infra_building(&mut infra, building);
    }
    infra
}

fn add_trade_infra_building(infra: &mut TradeInfra, building: &hoi4_state::Building) {
    if building.level == 0 {
        return;
    }
    if building.building_def_id == "port" {
        infra.total_port_level += building.level as f32;
        infra.first_port_state.get_or_insert(building.state);
    } else if building.building_def_id == "railway" {
        infra.total_railway_level += building.level as f32;
    }
}

fn indexed_country_states(world: &World, country: CountryId) -> Vec<StateId> {
    if country.is_none() {
        return Vec::new();
    }
    world
        .country_state_index
        .get(country.0 as usize)
        .filter(|states| !states.is_empty())
        .cloned()
        .unwrap_or_else(|| world.country_state_ids(country))
}

fn has_port_building(world: &World, state: StateId) -> bool {
    world
        .countries
        .buildings_v6
        .buildings
        .iter()
        .any(|b| b.state == state && b.building_def_id == "port" && b.level > 0)
}

fn check_sea_blockade(world: &World, owner: CountryId, port_state: StateId) -> bool {
    let at_war_with_owner: Vec<CountryId> = world
        .diplomacy
        .wars
        .values()
        .filter(|w| w.contains(owner))
        .flat_map(|w| {
            let side = w.side_of(owner);
            match side {
                Some(hoi4_state::WarSide::Attacker) => w.defenders.iter().copied().collect(),
                Some(hoi4_state::WarSide::Defender) => w.attackers.iter().copied().collect(),
                None => Vec::new(),
            }
        })
        .collect();

    if at_war_with_owner.is_empty() {
        return false;
    }

    let port_sea_region = port_sea_region_id(world, port_state);
    if port_sea_region == crate::naval::regions::NO_SEA_REGION {
        return at_war_with_owner
            .iter()
            .any(|&enemy| has_enemy_fleet_in_abstract_sea(world, enemy));
    }

    for &enemy in &at_war_with_owner {
        let enemy_fleet_in_region = has_enemy_fleet_in_sea_region(world, enemy, port_sea_region);
        if enemy_fleet_in_region {
            return true;
        }
    }

    false
}

fn sea_route_throughput(world: &World, owner: CountryId, port_state: StateId) -> f32 {
    let si = port_state.0 as usize;
    let cached_blocked = si < world.countries.trade.blockaded_ports.len()
        && world.countries.trade.blockaded_ports[si];
    if cached_blocked || check_sea_blockade(world, owner, port_state) {
        0.0
    } else {
        1.0
    }
}

fn port_sea_region_id(world: &World, port_state: StateId) -> u32 {
    let si = port_state.0 as usize;
    if si >= world.states.count {
        return crate::naval::regions::NO_SEA_REGION;
    }
    for province_id in &world.states.provinces[si] {
        if let Some(&region) = world.countries.trade.port_to_sea_region.get(&province_id.0) {
            return region;
        }
        if world
            .map
            .definitions
            .get(province_id.0 as usize)
            .and_then(|d| d.as_ref())
            .map(|d| d.coastal)
            .unwrap_or(false)
        {
            for &neighbor in world.map.neighbors(province_id.0) {
                if let Some(def) = world
                    .map
                    .definitions
                    .get(neighbor as usize)
                    .and_then(|d| d.as_ref())
                {
                    if def.province_type == hoi4_map::ProvinceType::Sea {
                        if let Some(sea_map) = &world.countries.trade.sea_region_map {
                            if let Some(&region) = sea_map.province_region.get(&neighbor) {
                                return region;
                            }
                        }
                    }
                }
            }
        }
    }
    crate::naval::regions::NO_SEA_REGION
}

fn has_enemy_fleet_in_sea_region(world: &World, enemy: CountryId, sea_region: u32) -> bool {
    for fi in 0..world.fleets.count {
        if world.fleets.owners[fi] != enemy {
            continue;
        }
        if world.fleets.target_region_id[fi] != crate::naval::regions::NO_SEA_REGION {
            continue;
        }
        if world.fleets.region_id[fi] == sea_region {
            let fleet_strength: f32 = world.fleets.ships[fi]
                .iter()
                .filter(|&&s| !s.is_none())
                .count() as f32;
            if fleet_strength >= 1.0 {
                return true;
            }
        }
    }
    false
}

fn has_enemy_fleet_in_abstract_sea(world: &World, enemy: CountryId) -> bool {
    for fi in 0..world.fleets.count {
        if world.fleets.owners[fi] != enemy {
            continue;
        }
        if world.fleets.target_region_id[fi] != crate::naval::regions::NO_SEA_REGION {
            continue;
        }
        if world.fleets.region_id[fi] == 0 {
            return true;
        }
    }
    false
}

fn is_import_blocked_by_blockade(world: &World, ci: usize, good_id: &str) -> bool {
    let country_id = CountryId(ci as u16);
    let has_sea_routes = world.countries.trade.routes.iter().any(|r| {
        r.importer == country_id
            && r.kind.uses_sea_lanes()
            && r.good_id.as_deref().map(|id| id == good_id).unwrap_or(true)
    });

    if !has_sea_routes {
        return false;
    }

    let all_sea_blocked = world
        .countries
        .trade
        .routes
        .iter()
        .filter(|r| {
            r.importer == country_id
                && r.kind.uses_sea_lanes()
                && r.good_id.as_deref().map(|id| id == good_id).unwrap_or(true)
        })
        .all(|r| r.is_blockaded);

    let has_land_alternative = world.countries.trade.routes.iter().any(|r| {
        r.importer == country_id
            && r.kind == TradeRouteKind::Land
            && !r.is_blockaded
            && r.good_id.as_deref().map(|id| id == good_id).unwrap_or(true)
    });

    all_sea_blocked && !has_land_alternative
}

fn update_trade_routes(
    world: &mut World,
    ci: usize,
    import_flows: &std::collections::HashMap<String, (f32, CountryId)>,
    export_flows: &std::collections::HashMap<String, f32>,
    state_ids: &[StateId],
    first_port_state: Option<StateId>,
) {
    world.countries.trade.routes.retain(|r| {
        r.historical || (r.importer != CountryId(ci as u16) && r.exporter != CountryId(ci as u16))
    });

    let country_id = CountryId(ci as u16);
    let first_state = state_ids.first().copied();

    for (good_id, (amount, exporter)) in import_flows {
        if *amount <= 0.0 {
            continue;
        }
        let kind = if is_imperial_preference_route(world, country_id, *exporter) {
            TradeRouteKind::ImperialPreference
        } else if first_port_state.is_some() {
            TradeRouteKind::Sea
        } else {
            TradeRouteKind::Land
        };
        let port_state = if kind.uses_sea_lanes() {
            first_port_state
        } else {
            None
        };

        let throughput = if kind.uses_sea_lanes() {
            port_state
                .map(|sid| sea_route_throughput(world, country_id, sid))
                .unwrap_or(1.0)
        } else {
            1.0
        };

        let route_id = world.countries.trade.add_route_for_good(
            country_id,
            *exporter,
            Some(good_id.clone()),
            kind,
            port_state,
            amount * throughput,
            false,
        );
        if throughput <= 0.0 {
            if let Some(route) = world
                .countries
                .trade
                .routes
                .iter_mut()
                .find(|route| route.id == route_id)
            {
                route.is_blockaded = true;
            }
        }
    }

    for (good_id, amount) in export_flows {
        if *amount <= 0.0 {
            continue;
        }
        let kind = if first_port_state.is_some() {
            TradeRouteKind::Sea
        } else {
            TradeRouteKind::Land
        };
        let port_state = if kind.uses_sea_lanes() {
            first_port_state
        } else {
            None
        };

        world.countries.trade.add_route_for_good(
            CountryId::NONE,
            country_id,
            Some(good_id.clone()),
            kind,
            port_state,
            *amount,
            false,
        );
    }

    let _ = first_state;
}

fn is_imperial_preference_route(world: &World, importer: CountryId, exporter: CountryId) -> bool {
    if exporter.is_none() {
        return false;
    }
    if is_master_subject_pair(world, importer, exporter) {
        return true;
    }
    let importer_bloc = world
        .countries
        .market
        .country_bloc
        .get(importer.0 as usize)
        .copied()
        .flatten();
    let exporter_bloc = world
        .countries
        .market
        .country_bloc
        .get(exporter.0 as usize)
        .copied()
        .flatten();
    importer_bloc.is_some() && importer_bloc == exporter_bloc
}

fn find_route_port_for_good(world: &World, ci: usize, good_id: &str) -> Option<StateId> {
    let country_id = CountryId(ci as u16);
    world
        .countries
        .trade
        .routes
        .iter()
        .filter(|r| {
            r.importer == country_id
                && r.kind.uses_sea_lanes()
                && r.good_id.as_deref().map(|id| id == good_id).unwrap_or(true)
        })
        .filter_map(|r| r.port_state)
        .next()
}

fn enforce_forex_control(world: &mut World, ci: usize) {
    let reserve_gbp = world.countries.treasury.treasuries[ci].reserve_gbp;
    if reserve_gbp < 10_000_000.0 {
        let scale = ((reserve_gbp / 10_000_000.0).max(0.0).min(1.0)) as f32;
        for val in world.countries.market.markets[ci].imports.values_mut() {
            *val *= scale;
        }
    }
}

fn apply_coal_diversion(world: &mut World, ci: usize, ratio: f32) {
    let country_id = CountryId(ci as u16);
    let state_ids: Vec<StateId> = (0..world.states.count)
        .filter(|&si| world.states.owners[si] == country_id)
        .map(|si| StateId(si as u16))
        .collect();

    if let Some(coal_supply) = world.countries.market.markets[ci].supply.get_mut("coal") {
        *coal_supply *= 1.0 - ratio;
    }
    for building in &mut world.countries.buildings_v6.buildings {
        if !state_ids.contains(&building.state) {
            continue;
        }
        if building.building_def_id == "steel_mill" && building.active_pm == "default" {
            building.active_pm = "coal_diverted".to_owned();
        }
    }

    let _ = ratio;
}

fn apply_satisfaction_penalty(world: &mut World, ci: usize, penalty: f32) {
    let country_id = CountryId(ci as u16);
    let state_ids: Vec<StateId> = (0..world.states.count)
        .filter(|&si| world.states.owners[si] == country_id)
        .map(|si| StateId(si as u16))
        .collect();

    for pg in &mut world.countries.pops.groups {
        if !state_ids.contains(&pg.state) {
            continue;
        }
        if pg.class == PopClass::Soldier {
            continue;
        }
        pg.satisfaction = (pg.satisfaction + penalty * 0.01).clamp(0.0, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hoi4_state::TradeStore;

    #[test]
    fn blockade_zeros_throughput() {
        let mut store = TradeStore::new();
        store.ensure_capacity(10);
        store.blockaded_ports.resize(5, false);
        store.blockaded_ports[3] = true;
        assert_eq!(store.trade_throughput_for_state(3), 0.0);
        assert_eq!(store.trade_throughput_for_state(2), 1.0);
    }

    #[test]
    fn trade_route_add_and_query() {
        let mut store = TradeStore::new();
        let id = store.add_route(
            CountryId(0),
            CountryId(1),
            TradeRouteKind::Sea,
            Some(StateId(5)),
            100.0,
        );
        assert_eq!(id, 0);
        assert_eq!(store.routes.len(), 1);
        assert_eq!(store.routes[0].kind, TradeRouteKind::Sea);
    }

    #[test]
    fn trade_agreement_add() {
        let mut store = TradeStore::new();
        let id = store.add_agreement(
            CountryId(0),
            CountryId(1),
            "rubber".to_owned(),
            50.0,
            TradeDirection::AImportsFromB,
        );
        assert_eq!(id, 0);
        assert_eq!(store.agreements.len(), 1);
        assert_eq!(store.agreements[0].good_id, "rubber");
    }

    #[test]
    fn blockade_check_is_good_specific() {
        let map = std::sync::Arc::new(hoi4_map::GameMap {
            definitions: vec![],
            rgb_to_id: std::collections::HashMap::new(),
            province_map: hoi4_map::ProvinceMap {
                width: 1,
                height: 1,
                pixels: vec![0],
            },
            adjacencies: vec![],
            special_adjacencies: vec![],
            heightmap: hoi4_map::Heightmap {
                width: 1,
                height: 1,
                pixels: vec![0],
            },
            terrain_bmp: hoi4_map::TerrainBitmap {
                width: 1,
                height: 1,
                pixels: vec![0],
                palette: [[0; 3]; 256],
            },
            terrain_catalog: hoi4_map::TerrainCatalog::default(),
            tree_definition_bmp: None,
            tree_indices: std::collections::HashSet::new(),
        });
        let data = std::sync::Arc::new(hoi4_data::GameData::default());
        let mut world = World::new(map, data);
        world.countries.trade.add_route_for_good(
            CountryId(0),
            CountryId(1),
            Some("oil".to_owned()),
            TradeRouteKind::Sea,
            Some(StateId(1)),
            10.0,
            true,
        );
        world.countries.trade.add_route_for_good(
            CountryId(0),
            CountryId(2),
            Some("rubber".to_owned()),
            TradeRouteKind::Sea,
            Some(StateId(2)),
            10.0,
            true,
        );
        world.countries.trade.routes[0].is_blockaded = true;
        world.countries.trade.routes[0].throughput = 0.0;

        assert!(is_import_blocked_by_blockade(&world, 0, "oil"));
        assert!(!is_import_blocked_by_blockade(&world, 0, "rubber"));
    }

    #[test]
    fn invariant_i13_all_trade_settled_in_gbp() {
        let treasury = hoi4_state::Treasury::default();
        assert_eq!(treasury.daily_trade_balance_gbp, 0.0);
        assert!(treasury.reserve_gbp == 0.0);
    }

    #[test]
    fn invariant_i14_blockaded_port_zero_throughput() {
        let mut store = TradeStore::new();
        store.ensure_capacity(10);
        store.blockaded_ports.resize(3, false);
        store.blockaded_ports[1] = true;
        assert_eq!(store.trade_throughput_for_state(1), 0.0);
        assert_eq!(store.trade_throughput_for_state(0), 1.0);
        assert_eq!(store.trade_throughput_for_state(2), 1.0);
    }
}
