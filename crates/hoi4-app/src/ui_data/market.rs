// Market and industry-chain panel DTO builders.
use hoi4_state::World;

use super::names::{DisplayNameKind, DisplayNameResolver};

pub fn market_panel(
    world: &World,
    v6_db: &hoi4_content::V6Database,
    econ: &hoi4_logic::economy::EconomyState,
    player: usize,
) -> Option<hoi4_ui::market_panel::MarketPanelData> {
    build_market_panel_data(world, v6_db, econ, player)
}

pub fn build_market_panel_data(
    world: &World,
    v6_db: &hoi4_content::V6Database,
    econ: &hoi4_logic::economy::EconomyState,
    player: usize,
) -> Option<hoi4_ui::market_panel::MarketPanelData> {
    let name_resolver = DisplayNameResolver::new(None);
    let market = world.countries.market.markets.get(player)?;
    let player_id = hoi4_state::CountryId(player as u16);
    let mut producers: std::collections::HashMap<
        String,
        Vec<hoi4_ui::market_panel::GoodFlowSource>,
    > = std::collections::HashMap::new();
    let mut consumers: std::collections::HashMap<
        String,
        Vec<hoi4_ui::market_panel::GoodFlowSource>,
    > = std::collections::HashMap::new();
    for building in &world.countries.buildings_v6.buildings {
        let state_idx = building.state.0 as usize;
        if state_idx >= world.states.count
            || world.states.owners[state_idx] != player_id
            || building.level == 0
        {
            continue;
        }
        let state_name = state_name(&name_resolver, world, state_idx);
        let building_name = building_name(&name_resolver, v6_db, &building.building_def_id);
        let label = if state_name.is_empty() {
            building_name
        } else {
            format!("{} - {}", state_name, building_name)
        };
        let pms = hoi4_content::active_pms_for_building(building, v6_db);
        let throughput = building.production_rate.max(0.0).max(1.0);
        for pm in pms {
            let pm_throughput = pm.throughput_modifier.max(0.0) * throughput;
            for (i, good_id) in pm.output_good_ids.iter().enumerate() {
                let amount = pm.output_good_amounts.get(i).copied().unwrap_or(0.0)
                    * building.level as f32
                    * pm_throughput;
                if amount > 0.0 {
                    producers.entry(good_id.clone()).or_default().push(
                        hoi4_ui::market_panel::GoodFlowSource {
                            name: label.clone(),
                            amount,
                        },
                    );
                }
            }
            for (i, good_id) in pm.input_good_ids.iter().enumerate() {
                let amount =
                    pm.input_good_amounts.get(i).copied().unwrap_or(0.0) * building.level as f32;
                if amount > 0.0 {
                    consumers.entry(good_id.clone()).or_default().push(
                        hoi4_ui::market_panel::GoodFlowSource {
                            name: label.clone(),
                            amount,
                        },
                    );
                }
            }
        }
    }
    for rows in producers.values_mut() {
        rows.sort_by(|a, b| {
            b.amount
                .partial_cmp(&a.amount)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
    for rows in consumers.values_mut() {
        rows.sort_by(|a, b| {
            b.amount
                .partial_cmp(&a.amount)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
    let economy_law = world.countries.law_store.law_sets[player].0
        [hoi4_state::LawCategory::Economy.index()]
    .current
    .clone();
    let division_count = world
        .divisions
        .owners
        .iter()
        .filter(|owner| **owner == player_id)
        .count() as f32;
    let base_procurement = match economy_law.as_str() {
        "corporatist_war_economy" => 8.0,
        "war_economy" => 3.0,
        "interventionism" => 1.0,
        _ => 0.0,
    };
    let mefo_credit_mult = if economy_law == "corporatist_war_economy" {
        world
            .countries
            .treasury
            .treasuries
            .get(player)
            .map(|t| {
                let mefo_room_ratio = if t.gdp_rm > 0.0 {
                    ((t.gdp_rm * 0.30 - t.mefo_debt_rm).max(0.0) / t.gdp_rm) as f32
                } else {
                    0.30
                };
                1.0 + mefo_room_ratio * 10.0
            })
            .unwrap_or(1.0)
    } else {
        1.0
    };
    let procurement_mult = base_procurement * (1.0 + division_count * 0.05) * mefo_credit_mult;
    let mut government_orders: std::collections::HashMap<
        String,
        Vec<hoi4_ui::market_panel::GoodFlowSource>,
    > = std::collections::HashMap::new();
    if procurement_mult > 0.0 {
        for g in &v6_db.goods {
            if g.category == hoi4_content::v6_loader::GoodCategoryDef::MilitaryIntermediate {
                government_orders.entry(g.id.clone()).or_default().push(
                    hoi4_ui::market_panel::GoodFlowSource {
                        name: "政府军需采购".to_owned(),
                        amount: procurement_mult,
                    },
                );
            }
        }
        for pm in &v6_db.production_methods {
            if pm.equipment_output.is_some() {
                for (i, good_id) in pm.input_good_ids.iter().enumerate() {
                    let amount =
                        pm.input_good_amounts.get(i).copied().unwrap_or(0.0) * procurement_mult;
                    if amount > 0.0 {
                        government_orders.entry(good_id.clone()).or_default().push(
                            hoi4_ui::market_panel::GoodFlowSource {
                                name: name_resolver.content_name(
                                    DisplayNameKind::ProductionMethod,
                                    &pm.id,
                                    &pm.name,
                                ),
                                amount,
                            },
                        );
                    }
                }
            }
        }
    }
    let mut pop_consumption: std::collections::HashMap<
        String,
        Vec<hoi4_ui::market_panel::GoodFlowSource>,
    > = std::collections::HashMap::new();
    for class_idx in 0..hoi4_state::PopClass::COUNT {
        let Some(class) = hoi4_state::PopClass::from_index(class_idx) else {
            continue;
        };
        let needs = v6_db.pop_need_entries_for_class(class);
        if needs.is_empty() {
            continue;
        }
        let total_pop: f32 = world
            .countries
            .pops
            .groups
            .iter()
            .filter(|pop| {
                pop.class == class
                    && pop.employed_at.is_some()
                    && world
                        .states
                        .owners
                        .get(pop.state.0 as usize)
                        .copied()
                        .unwrap_or(hoi4_state::CountryId::NONE)
                        == player_id
            })
            .map(|pop| {
                let si = pop.state.0 as usize;
                let factor = world
                    .states
                    .integration_status
                    .get(si)
                    .copied()
                    .map(hoi4_logic::economy::finance_tick::integration_consumption_factor)
                    .unwrap_or(1.0);
                pop.size as f32 * factor
            })
            .sum();
        let pop_millions = total_pop / 1_000_000.0;
        if pop_millions > 0.0 {
            for need in needs {
                let amount = pop_millions * need.amount_per_million;
                if amount <= 0.0 {
                    continue;
                }
                pop_consumption
                    .entry(need.good_id.clone())
                    .or_default()
                    .push(hoi4_ui::market_panel::GoodFlowSource {
                        name: name_resolver.pop_class_name(class),
                        amount,
                    });
            }
        }
    }
    let chain_graph =
        hoi4_logic::economy::production_chain::ProductionChainGraph::from_database_and_market(
            v6_db,
            Some(market),
        );
    let construction_demand = {
        let cp = hoi4_logic::economy::construction_tick::construction_cp_pool(world, player);
        if econ
            .construction
            .get(player)
            .map(|q| q.items.is_empty())
            .unwrap_or(true)
        {
            0.0
        } else {
            cp * 0.05
        }
    };
    let any_blockaded = world
        .countries
        .trade
        .routes
        .iter()
        .any(|r| (r.importer == player_id || r.exporter == player_id) && r.is_blockaded);
    let exchange_rate = world
        .countries
        .treasury
        .exchange_rates
        .get(player)
        .map(|er| er.rm_per_gbp)
        .unwrap_or(12.5);
    let paid_procurement_rm = world
        .countries
        .treasury
        .treasuries
        .get(player)
        .map(|t| t.daily_budget.expense_military_procurement_rm)
        .unwrap_or(0.0);
    let total_government_order_amount: f32 = government_orders
        .values()
        .flat_map(|rows| rows.iter())
        .map(|row| row.amount)
        .sum();
    let goods: Vec<hoi4_ui::market_panel::GoodEntry> = v6_db
        .goods
        .iter()
        .map(|g| {
            let price = market.price.get(&g.id).copied().unwrap_or(g.base_price_rm);
            let supply = market.supply.get(&g.id).copied().unwrap_or(0.0);
            let demand = market.demand.get(&g.id).copied().unwrap_or(0.0);
            let stockpile = market.stockpile.get(&g.id).copied().unwrap_or(0.0);
            let stockpile_coverage_days = market
                .stockpile_coverage_days
                .get(&g.id)
                .copied()
                .unwrap_or_else(|| {
                    if demand > 0.0 {
                        stockpile / demand
                    } else {
                        0.0
                    }
                });
            let unmet_demand = market.unmet_demand.get(&g.id).copied().unwrap_or(0.0);
            let imports = market.imports.get(&g.id).copied().unwrap_or(0.0);
            let exports = market.exports.get(&g.id).copied().unwrap_or(0.0);
            let mut supply_sources: Vec<hoi4_ui::market_panel::GoodSupplySourceEntry> = Vec::new();
            let good_producers = producers.get(&g.id).cloned().unwrap_or_default();
            let good_consumers = consumers.get(&g.id).cloned().unwrap_or_default();
            let domestic_production: f32 = good_producers.iter().map(|row| row.amount).sum();
            if domestic_production > 0.0 {
                supply_sources.push(hoi4_ui::market_panel::GoodSupplySourceEntry {
                    kind: hoi4_ui::market_panel::GoodSupplySourceKind::Domestic,
                    label: "国内生产".to_owned(),
                    amount: domestic_production,
                });
            }
            let building_input_demand: f32 = good_consumers.iter().map(|row| row.amount).sum();
            let affected_pop_classes = pop_consumption.get(&g.id).cloned().unwrap_or_default();
            let pop_consumption_demand: f32 =
                affected_pop_classes.iter().map(|row| row.amount).sum();
            let good_government_orders = government_orders.get(&g.id).cloned().unwrap_or_default();
            let military_order_demand: f32 =
                good_government_orders.iter().map(|row| row.amount).sum();
            let construction_demand_for_good = if g.id == "machinery" {
                construction_demand
            } else {
                0.0
            };
            let stockpile_draw = stockpile.min(demand.max(0.0) * 0.25);
            if stockpile_draw > 0.0 {
                supply_sources.push(hoi4_ui::market_panel::GoodSupplySourceEntry {
                    kind: hoi4_ui::market_panel::GoodSupplySourceKind::Stockpile,
                    label: "库存释放".to_owned(),
                    amount: stockpile_draw,
                });
            }
            for route in world.countries.trade.routes.iter().filter(|route| {
                route.importer == player_id && route.good_id.as_deref() == Some(g.id.as_str())
            }) {
                if route.throughput <= 0.0 {
                    continue;
                }
                let (kind, label) = if route.exporter.is_none() {
                    (
                        hoi4_ui::market_panel::GoodSupplySourceKind::WorldSpot,
                        "世界市场".to_owned(),
                    )
                } else {
                    let tag = world
                        .countries
                        .tags
                        .get(route.exporter.0 as usize)
                        .cloned()
                        .unwrap_or_default();
                    let country_name = name_resolver.country_name(tag.as_str(), tag.as_str());
                    if world.diplomacy.is_subject_of(route.exporter, player_id) {
                        (
                            hoi4_ui::market_panel::GoodSupplySourceKind::Subject,
                            country_name,
                        )
                    } else if world
                        .countries
                        .market
                        .country_bloc
                        .get(player)
                        .copied()
                        .flatten()
                        == world
                            .countries
                            .market
                            .country_bloc
                            .get(route.exporter.0 as usize)
                            .copied()
                            .flatten()
                    {
                        (
                            hoi4_ui::market_panel::GoodSupplySourceKind::MarketBloc,
                            country_name,
                        )
                    } else {
                        (
                            hoi4_ui::market_panel::GoodSupplySourceKind::WorldSpot,
                            country_name,
                        )
                    }
                };
                supply_sources.push(hoi4_ui::market_panel::GoodSupplySourceEntry {
                    kind,
                    label,
                    amount: route.throughput,
                });
            }
            let traded = if military_order_demand > 0.0 {
                military_order_demand.min((supply + stockpile_draw).max(0.0))
            } else {
                demand.min((supply + stockpile_draw).max(0.0))
            };
            let paid_rm = if total_government_order_amount > 0.0 && military_order_demand > 0.0 {
                paid_procurement_rm * (military_order_demand / total_government_order_amount) as f64
            } else {
                0.0
            };
            let display_good_name = good_name(&name_resolver, v6_db, &g.id);
            let shortage = unmet_demand.max((demand - supply).max(0.0));
            let graph_impact = chain_graph.shortage_impact(&g.id);
            let mut affected_buildings = if shortage > 0.0 {
                good_consumers.clone()
            } else {
                Vec::new()
            };
            if shortage > 0.0 {
                for building in graph_impact.affected_buildings.iter().take(8) {
                    let name = building_name(&name_resolver, v6_db, &building.building_id);
                    if !affected_buildings.iter().any(|row| row.name == name) {
                        affected_buildings.push(hoi4_ui::market_panel::GoodFlowSource {
                            name,
                            amount: building.amount_per_level,
                        });
                    }
                }
            }
            affected_buildings.sort_by(|a, b| {
                b.amount
                    .partial_cmp(&a.amount)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let affected_demand_buckets: Vec<hoi4_ui::market_panel::GoodFlowSource> =
                if shortage > 0.0 {
                    graph_impact
                        .affected_demand_buckets
                        .iter()
                        .map(|bucket| hoi4_ui::market_panel::GoodFlowSource {
                            name: demand_bucket_label(bucket.bucket).to_owned(),
                            amount: bucket.unmet.max(bucket.requested.min(shortage)),
                        })
                        .collect()
                } else {
                    Vec::new()
                };
            let diagnosis = chain_graph.diagnose_shortage(
                &g.id,
                hoi4_logic::economy::production_chain::ProductionShortageContext {
                    demand,
                    supply,
                    imports,
                    exports,
                    stockpile,
                    stockpile_coverage_days,
                    domestic_production,
                    building_input_demand,
                    pop_consumption_demand,
                    military_order_demand,
                    construction_demand: construction_demand_for_good,
                    is_blockaded: any_blockaded && imports > 0.0,
                    has_market_bloc_supply: supply_sources.iter().any(|source| {
                        source.kind == hoi4_ui::market_panel::GoodSupplySourceKind::MarketBloc
                            && source.amount > 0.0
                    }),
                    has_subject_supply: supply_sources.iter().any(|source| {
                        source.kind == hoi4_ui::market_panel::GoodSupplySourceKind::Subject
                            && source.amount > 0.0
                    }),
                    world_spot_available: world.countries.market.world_spot.available_for(&g.id),
                    has_active_construction_queue: econ
                        .construction
                        .get(player)
                        .map(|queue| queue.items.iter().any(|item| !item.paused))
                        .unwrap_or(false),
                },
            );
            let actionable_fixes: Vec<hoi4_ui::market_panel::MarketActionEntry> = diagnosis
                .actions
                .iter()
                .map(|action| {
                    market_action_entry_for(
                        &name_resolver,
                        v6_db,
                        action,
                        &display_good_name,
                        diagnosis.shortage_amount,
                    )
                })
                .collect();
            let shortage_reason =
                shortage_reason_for(&display_good_name, &diagnosis, demand, supply);
            let category = match g.category {
                hoi4_content::v6_loader::GoodCategoryDef::RawMaterial => {
                    hoi4_ui::market_panel::GoodCategory::RawMaterial
                }
                hoi4_content::v6_loader::GoodCategoryDef::Intermediate => {
                    hoi4_ui::market_panel::GoodCategory::Intermediate
                }
                hoi4_content::v6_loader::GoodCategoryDef::Consumer => {
                    hoi4_ui::market_panel::GoodCategory::Consumer
                }
                hoi4_content::v6_loader::GoodCategoryDef::Luxury => {
                    hoi4_ui::market_panel::GoodCategory::Luxury
                }
                hoi4_content::v6_loader::GoodCategoryDef::Service => {
                    hoi4_ui::market_panel::GoodCategory::Service
                }
                hoi4_content::v6_loader::GoodCategoryDef::MilitaryIntermediate => {
                    hoi4_ui::market_panel::GoodCategory::MilitaryIntermediate
                }
            };
            let good_name = display_good_name;
            hoi4_ui::market_panel::GoodEntry {
                id: g.id.clone(),
                name: good_name,
                category,
                price,
                base_price: g.base_price_rm,
                supply,
                demand,
                traded,
                stockpile,
                stockpile_coverage_days,
                unmet_demand,
                domestic_production,
                stockpile_draw,
                building_input_demand,
                pop_consumption_demand,
                military_order_demand,
                supply_sources,
                producers: good_producers,
                consumers: good_consumers,
                government_orders: good_government_orders,
                construction_demand: construction_demand_for_good,
                imports,
                exports,
                is_blockaded: any_blockaded && imports > 0.0,
                affected_buildings,
                affected_pop_classes: if demand > supply {
                    affected_pop_classes
                } else {
                    Vec::new()
                },
                affected_demand_buckets,
                upstream_goods: chain_graph.upstream_goods(&g.id),
                downstream_goods: chain_graph.downstream_goods(&g.id),
                shortage_reason,
                actionable_fixes,
                paid_rm,
                clearing_fulfilled: market
                    .clearing_sheet
                    .results
                    .get(&g.id)
                    .map(|r| r.total_fulfilled)
                    .unwrap_or(0.0),
                clearing_unmet: market
                    .clearing_sheet
                    .results
                    .get(&g.id)
                    .map(|r| r.total_unmet)
                    .unwrap_or(0.0),
                clearing_shortage_ratio: market
                    .clearing_sheet
                    .results
                    .get(&g.id)
                    .map(|r| r.shortage_ratio)
                    .unwrap_or(0.0),
            }
        })
        .collect();
    let total_shortage_value_rm: f64 = goods
        .iter()
        .map(|good| {
            good.unmet_demand.max((good.demand - good.supply).max(0.0)) as f64 * good.price as f64
        })
        .sum();
    let total_import_value_gbp: f64 = goods
        .iter()
        .map(|good| good.imports as f64 * good.price as f64 / exchange_rate.max(0.01) as f64)
        .sum();
    let total_export_value_gbp: f64 = goods
        .iter()
        .map(|good| good.exports as f64 * good.price as f64 / exchange_rate.max(0.01) as f64)
        .sum();
    let pop_demand_total: f32 = goods.iter().map(|good| good.pop_consumption_demand).sum();
    let pop_shortage_total: f32 = goods
        .iter()
        .filter(|good| good.pop_consumption_demand > 0.0)
        .map(|good| good.unmet_demand.min(good.pop_consumption_demand))
        .sum();
    let pop_needs_fulfillment = if pop_demand_total > 0.0 {
        (1.0 - pop_shortage_total / pop_demand_total).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let military_order_total: f32 = goods.iter().map(|good| good.military_order_demand).sum();
    let military_unmet_total: f32 = goods
        .iter()
        .filter(|good| good.military_order_demand > 0.0)
        .map(|good| (good.military_order_demand - good.traded).max(0.0))
        .sum();
    let military_supply_pressure = if military_order_total > 0.0 {
        (military_unmet_total / military_order_total).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut alerts: Vec<hoi4_ui::market_panel::MarketAlertEntry> = goods
        .iter()
        .filter(|good| good.unmet_demand.max((good.demand - good.supply).max(0.0)) > 0.0)
        .take(5)
        .map(|good| hoi4_ui::market_panel::MarketAlertEntry {
            severity: if good.stockpile_coverage_days <= 3.0 {
                hoi4_ui::market_panel::MarketAlertSeverity::Critical
            } else {
                hoi4_ui::market_panel::MarketAlertSeverity::Warning
            },
            good_id: good.id.clone(),
            title: format!("{} 短缺", good.name),
            description: format!(
                "当前缺口 {:.1}/日，库存可支撑 {:.1} 天。",
                good.unmet_demand.max((good.demand - good.supply).max(0.0)),
                good.stockpile_coverage_days
            ),
        })
        .collect();
    if alerts.is_empty() {
        alerts.push(hoi4_ui::market_panel::MarketAlertEntry {
            severity: hoi4_ui::market_panel::MarketAlertSeverity::Info,
            good_id: String::new(),
            title: "市场供需稳定".to_owned(),
            description: "暂无需要立即处理的商品短缺。".to_owned(),
        });
    }
    let cash_rm = world
        .countries
        .treasury
        .treasuries
        .get(player)
        .map(|t| t.cash_rm)
        .unwrap_or(0.0);
    let bloc = world
        .countries
        .market
        .bloc_for_country(player_id)
        .map(|bloc| {
            let member_set: std::collections::HashSet<hoi4_state::CountryId> =
                bloc.members.iter().copied().collect();
            let price_rm = |good_id: &str| -> f32 {
                v6_db
                    .goods
                    .iter()
                    .find(|good| good.id == good_id)
                    .map(|good| good.base_price_rm)
                    .unwrap_or(1.0)
            };
            let route_value_gbp = |route: &hoi4_state::TradeRoute| -> f64 {
                let price = route.good_id.as_deref().map(price_rm).unwrap_or(1.0);
                route.throughput as f64 * price as f64 / exchange_rate.max(0.01) as f64
            };
            let internal_trade_value_gbp: f64 = world
                .countries
                .trade
                .routes
                .iter()
                .filter(|route| {
                    member_set.contains(&route.importer) && member_set.contains(&route.exporter)
                })
                .map(route_value_gbp)
                .sum();
            let external_trade_value_gbp: f64 = world
                .countries
                .trade
                .routes
                .iter()
                .filter(|route| {
                    member_set.contains(&route.importer) ^ member_set.contains(&route.exporter)
                })
                .map(route_value_gbp)
                .sum();
            let tag_of = |country: hoi4_state::CountryId| -> String {
                world
                    .countries
                    .tags
                    .get(country.0 as usize)
                    .cloned()
                    .unwrap_or_default()
            };
            let members = bloc
                .members
                .iter()
                .map(|&member| {
                    let market = world.countries.market.markets.get(member.0 as usize);
                    let contribution_supply_value_rm = market
                        .map(|market| {
                            market
                                .supply
                                .iter()
                                .map(|(good_id, amount)| *amount as f64 * price_rm(good_id) as f64)
                                .sum()
                        })
                        .unwrap_or(0.0);
                    let contribution_demand_value_rm = market
                        .map(|market| {
                            market
                                .demand
                                .iter()
                                .map(|(good_id, amount)| *amount as f64 * price_rm(good_id) as f64)
                                .sum()
                        })
                        .unwrap_or(0.0);
                    let mut strategic_goods: Vec<hoi4_ui::market_panel::GoodFlowSource> = market
                        .map(|market| {
                            ["oil", "rubber", "steel", "grain", "fuel", "machinery"]
                                .iter()
                                .filter_map(|good_id| {
                                    let mut amount =
                                        market.supply.get(*good_id).copied().unwrap_or(0.0);
                                    if let Some(autonomy) = world.diplomacy.autonomy.get(&member) {
                                        if autonomy.master == bloc.leader {
                                            amount *= autonomy.level.master_resource_share();
                                        }
                                    }
                                    if amount <= 0.0 {
                                        return None;
                                    }
                                    let name = good_name(&name_resolver, v6_db, good_id);
                                    Some(hoi4_ui::market_panel::GoodFlowSource { name, amount })
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    strategic_goods.sort_by(|a, b| {
                        b.amount
                            .partial_cmp(&a.amount)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                    let relation = if member == bloc.leader {
                        "集团领袖".to_owned()
                    } else if member == player_id {
                        "本国".to_owned()
                    } else if let Some(autonomy) = world.diplomacy.autonomy.get(&member) {
                        if autonomy.master == bloc.leader {
                            market_autonomy_level_label(autonomy.level).to_owned()
                        } else {
                            "成员国".to_owned()
                        }
                    } else {
                        "成员国".to_owned()
                    };
                    let market_access = if world.countries.trade.routes.iter().any(|route| {
                        (route.importer == member || route.exporter == member)
                            && route.kind.uses_sea_lanes()
                            && route.is_blockaded
                    }) {
                        0.5
                    } else {
                        1.0
                    };
                    hoi4_ui::market_panel::MarketBlocMemberEntry {
                        tag: tag_of(member),
                        relation,
                        market_access,
                        contribution_supply_value_rm,
                        contribution_demand_value_rm,
                        strategic_goods,
                    }
                })
                .collect();
            hoi4_ui::market_panel::MarketBlocPanelData {
                name: bloc.name.clone(),
                kind: match bloc.kind {
                    hoi4_state::MarketBlocKind::ImperialPreference => "帝国特惠市场".to_owned(),
                    hoi4_state::MarketBlocKind::FactionMarket => "阵营共同市场".to_owned(),
                    hoi4_state::MarketBlocKind::ColonialEmpire => "殖民帝国市场".to_owned(),
                    hoi4_state::MarketBlocKind::BilateralSphere => "双边势力圈".to_owned(),
                },
                leader_tag: tag_of(bloc.leader),
                members,
                internal_trade_value_gbp,
                external_trade_value_gbp,
            }
        });
    let subjects: Vec<hoi4_ui::market_panel::MarketSubjectEntry> = world
        .diplomacy
        .autonomy
        .values()
        .filter(|autonomy| autonomy.master == player_id)
        .map(|autonomy| {
            let tag = world
                .countries
                .tags
                .get(autonomy.subject.0 as usize)
                .cloned()
                .unwrap_or_default();
            let subject_market = world
                .countries
                .market
                .markets
                .get(autonomy.subject.0 as usize);
            let mut resource_contribution: Vec<hoi4_ui::market_panel::GoodFlowSource> =
                ["oil", "rubber", "steel", "grain", "fuel", "coal"]
                    .iter()
                    .filter_map(|good_id| {
                        let amount = subject_market
                            .and_then(|market| market.exports.get(*good_id).copied())
                            .or_else(|| {
                                subject_market.and_then(|market| {
                                    market.supply.get(*good_id).copied().map(|supply| {
                                        supply * autonomy.level.master_resource_share()
                                    })
                                })
                            })
                            .unwrap_or(0.0);
                        if amount <= 0.0 {
                            return None;
                        }
                        let name = good_name(&name_resolver, v6_db, good_id);
                        Some(hoi4_ui::market_panel::GoodFlowSource { name, amount })
                    })
                    .collect();
            resource_contribution.sort_by(|a, b| {
                b.amount
                    .partial_cmp(&a.amount)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let fiscal_contribution_gbp: f64 = resource_contribution
                .iter()
                .map(|row| row.amount as f64 * 0.05)
                .sum();
            let autonomy_level = match autonomy.level {
                hoi4_state::AutonomyLevel::Integrated => "整合属地",
                hoi4_state::AutonomyLevel::IntegratedPuppet => "整合傀儡",
                hoi4_state::AutonomyLevel::Puppet => "傀儡国",
                hoi4_state::AutonomyLevel::Dominion => "自治领",
                hoi4_state::AutonomyLevel::Satellite => "卫星国",
                hoi4_state::AutonomyLevel::FreedomAssociation => "自由联合",
            }
            .to_owned();
            let subject_states = world.country_state_ids(autonomy.subject);
            let avg_resistance = if subject_states.is_empty() {
                0.0
            } else {
                subject_states
                    .iter()
                    .map(|state| world.states.resistance[state.0 as usize])
                    .sum::<f32>()
                    / subject_states.len() as f32
            };
            let avg_compliance = if subject_states.is_empty() {
                0.0
            } else {
                subject_states
                    .iter()
                    .map(|state| world.states.compliance[state.0 as usize])
                    .sum::<f32>()
                    / subject_states.len() as f32
            };
            let market_access =
                hoi4_logic::occupation::country_governance_market_access(world, autonomy.subject);
            let risk = if avg_resistance >= 50.0 {
                format!(
                    "抵抗度 {:.0}%，资源和市场通道不稳定，实际市场接入约 {:.0}%。",
                    avg_resistance,
                    market_access * 100.0
                )
            } else if autonomy.level.master_resource_share() >= 0.5 {
                format!(
                    "资源抽取比例较高，顺从度 {:.0}%，市场接入约 {:.0}%。",
                    avg_compliance,
                    market_access * 100.0
                )
            } else {
                format!(
                    "属地供给稳定，顺从度 {:.0}%，市场接入约 {:.0}%。",
                    avg_compliance,
                    market_access * 100.0
                )
            };
            hoi4_ui::market_panel::MarketSubjectEntry {
                tag,
                autonomy_level,
                master_resource_share: autonomy.level.master_resource_share(),
                resource_contribution,
                fiscal_contribution_gbp,
                risk,
            }
        })
        .collect();
    let mut actions: Vec<hoi4_ui::market_panel::MarketActionEntry> = goods
        .iter()
        .filter(|good| good.unmet_demand.max((good.demand - good.supply).max(0.0)) > 0.0)
        .take(5)
        .enumerate()
        .map(|(idx, good)| {
            let shortage = good.unmet_demand.max((good.demand - good.supply).max(0.0));
            let (title, description) = if good.imports > 0.0 && good.is_blockaded {
                (
                    format!("恢复 {} 进口通道", good.name),
                    format!(
                        "{} 仍缺 {:.1}/日，受封锁影响的进口量为 {:.1}/日。",
                        good.name, shortage, good.imports
                    ),
                )
            } else if good.domestic_production <= 0.0 && good.imports <= 0.0 {
                (
                    format!("建立 {} 供给来源", good.name),
                    format!("{} 没有国内产出或进口，需要建设产能或开辟贸易。", good.name),
                )
            } else if good.pop_consumption_demand > good.building_input_demand {
                (
                    format!("保障 POP 的 {} 消费", good.name),
                    format!(
                        "POP 消费需求 {:.1}/日，是当前短缺的主要压力。",
                        good.pop_consumption_demand
                    ),
                )
            } else {
                (
                    format!("处理 {} 短缺", good.name),
                    format!("当前缺口 {:.1}/日，优先检查生产链和库存。", shortage),
                )
            };
            hoi4_ui::market_panel::MarketActionEntry {
                title,
                description,
                related_good_id: Some(good.id.clone()),
                priority: idx as u8,
            }
        })
        .collect();
    for good in goods
        .iter()
        .filter(|good| !good.actionable_fixes.is_empty())
    {
        for action in good.actionable_fixes.iter().take(2) {
            if actions.iter().any(|existing| {
                existing.title == action.title && existing.related_good_id == action.related_good_id
            }) {
                continue;
            }
            let mut action = action.clone();
            action.priority = (actions.len() as u8).saturating_add(1);
            actions.push(action);
        }
    }
    if actions.is_empty() && any_blockaded {
        actions.push(hoi4_ui::market_panel::MarketActionEntry {
            title: "恢复被封锁的进口".to_owned(),
            description: "进口线路受阻，相关商品和建筑链路需要优先处理。".to_owned(),
            related_good_id: None,
            priority: 0,
        });
    }
    Some(hoi4_ui::market_panel::MarketPanelData {
        goods,
        bloc,
        exchange_rate,
        cash_rm,
        total_shortage_value_rm,
        total_import_value_gbp,
        total_export_value_gbp,
        pop_needs_fulfillment,
        military_supply_pressure,
        subjects,
        actions,
        alerts,
    })
}

pub fn trade_panel(
    world: &World,
    v6_db: &hoi4_content::V6Database,
    player: usize,
) -> Option<hoi4_ui::trade_panel::TradePanelData> {
    build_trade_panel_data(world, v6_db, player)
}

pub fn build_trade_panel_data(
    world: &World,
    v6_db: &hoi4_content::V6Database,
    player: usize,
) -> Option<hoi4_ui::trade_panel::TradePanelData> {
    let name_resolver = DisplayNameResolver::new(None);
    let market = world.countries.market.markets.get(player)?;
    let treasury = world.countries.treasury.treasuries.get(player);
    let current_trade_law = world.countries.law_store.law_sets[player].0
        [hoi4_state::LawCategory::Trade.index()]
    .current
    .clone();
    let trade_def = v6_db
        .trade_laws
        .iter()
        .find(|law| law.id == current_trade_law);
    let import_tariff_rate = trade_def.map(|law| law.import_tariff_rate).unwrap_or(0.0);
    let export_tariff_rate = trade_def.map(|law| law.export_tariff_rate).unwrap_or(0.0);
    let fx_control = trade_def
        .map(|law| law.foreign_exchange_control)
        .unwrap_or(false);
    let trade_law_name = trade_def
        .map(|law| name_resolver.content_name(DisplayNameKind::Law, &law.id, &law.name))
        .unwrap_or_default();
    let player_id = hoi4_state::CountryId(player as u16);
    let any_blockaded = world.countries.trade.routes.iter().any(|route| {
        (route.importer == player_id || route.exporter == player_id) && route.is_blockaded
    });
    let blockade_affected = if any_blockaded {
        blockade_affected_buildings(&name_resolver, world, v6_db, player)
    } else {
        Vec::new()
    };
    let flows: Vec<hoi4_ui::trade_panel::TradeFlowEntry> = v6_db
        .goods
        .iter()
        .map(|good| {
            let imports = market.imports.get(&good.id).copied().unwrap_or(0.0);
            let exports = market.exports.get(&good.id).copied().unwrap_or(0.0);
            hoi4_ui::trade_panel::TradeFlowEntry {
                good_id: good.id.clone(),
                good_name: good_name(&name_resolver, v6_db, &good.id),
                imports,
                exports,
                failure_reason: world.countries.trade.import_failures.get(&good.id).cloned(),
                import_tariff_rate,
                export_tariff_rate,
            }
        })
        .collect();
    let routes: Vec<hoi4_ui::trade_panel::TradeRouteEntry> = world
        .countries
        .trade
        .routes
        .iter()
        .filter(|route| route.importer == player_id || route.exporter == player_id)
        .map(|route| {
            let good_id = route.good_id.clone().unwrap_or_default();
            let good_name = if good_id.is_empty() {
                "未知商品".to_owned()
            } else {
                good_name(&name_resolver, v6_db, &good_id)
            };
            let kind = match route.kind {
                hoi4_state::TradeRouteKind::Sea => "海运".to_owned(),
                hoi4_state::TradeRouteKind::Land => "陆运".to_owned(),
                hoi4_state::TradeRouteKind::Transit => "转运".to_owned(),
                hoi4_state::TradeRouteKind::ImperialPreference => "帝国特惠".to_owned(),
            };
            let partner = if route.importer == player_id {
                route.exporter
            } else {
                route.importer
            };
            let partner_tag = world
                .countries
                .tags
                .get(partner.0 as usize)
                .cloned()
                .unwrap_or_default();
            let partner_name =
                name_resolver.country_name(partner_tag.as_str(), partner_tag.as_str());
            hoi4_ui::trade_panel::TradeRouteEntry {
                good_id,
                good_name,
                kind,
                throughput: route.throughput,
                is_blockaded: route.is_blockaded,
                historical: route.historical,
                partner_tag,
                partner_name,
                affected: if route.is_blockaded {
                    blockade_affected.clone()
                } else {
                    Vec::new()
                },
            }
        })
        .collect();
    let reserve_gbp = treasury.map(|t| t.reserve_gbp).unwrap_or(0.0);
    let exchange_rate = world
        .countries
        .treasury
        .exchange_rates
        .get(player)
        .map(|er| er.rm_per_gbp)
        .unwrap_or(12.5);
    let trade_balance_gbp = treasury.map(|t| t.daily_trade_balance_gbp).unwrap_or(0.0);
    let trade_capacity = trade_capacity(world, player_id);
    let trade_capacity_used = market.imports.values().sum();

    Some(hoi4_ui::trade_panel::TradePanelData {
        flows,
        routes,
        reserve_gbp,
        exchange_rate,
        tariff_income_daily_rm: 0.0,
        trade_balance_daily_gbp: trade_balance_gbp,
        is_fx_control: fx_control,
        is_blockaded: any_blockaded,
        current_trade_law: current_trade_law.clone(),
        current_trade_law_name: trade_law_name,
        trade_capacity,
        trade_capacity_used,
    })
}

fn good_name(
    name_resolver: &DisplayNameResolver<'_>,
    v6_db: &hoi4_content::V6Database,
    good_id: &str,
) -> String {
    v6_db
        .goods
        .iter()
        .find(|good| good.id == good_id)
        .map(|good| name_resolver.content_name(DisplayNameKind::Good, &good.id, &good.name))
        .unwrap_or_else(|| name_resolver.content_name(DisplayNameKind::Good, good_id, good_id))
}

fn building_name(
    name_resolver: &DisplayNameResolver<'_>,
    v6_db: &hoi4_content::V6Database,
    building_id: &str,
) -> String {
    v6_db
        .buildings
        .iter()
        .find(|building| building.id == building_id)
        .map(|building| {
            name_resolver.content_name(DisplayNameKind::Building, &building.id, &building.name)
        })
        .unwrap_or_else(|| {
            name_resolver.content_name(DisplayNameKind::Building, building_id, building_id)
        })
}

fn state_name(name_resolver: &DisplayNameResolver<'_>, world: &World, state_idx: usize) -> String {
    world
        .states
        .names
        .get(state_idx)
        .map(|raw| name_resolver.state_name(raw, state_idx))
        .unwrap_or_else(|| name_resolver.state_name("", state_idx))
}

fn demand_bucket_label(bucket: hoi4_state::market::DemandBucketKind) -> &'static str {
    match bucket {
        hoi4_state::market::DemandBucketKind::BuildingInput => "建筑投入",
        hoi4_state::market::DemandBucketKind::PopBasicConsumption => "POP 基础消费",
        hoi4_state::market::DemandBucketKind::PopNonBasicConsumption => "POP 非基础消费",
        hoi4_state::market::DemandBucketKind::GovernmentProcurement => "政府采购",
        hoi4_state::market::DemandBucketKind::MilitaryInput => "军工投入",
        hoi4_state::market::DemandBucketKind::ConstructionInput => "建设投入",
        hoi4_state::market::DemandBucketKind::Export => "出口订单",
    }
}

fn shortage_reason_for(
    good_name: &str,
    diagnosis: &hoi4_logic::economy::production_chain::ProductionShortageDiagnosis,
    demand: f32,
    supply: f32,
) -> String {
    use hoi4_logic::economy::production_chain::{
        ProductionShortagePressureKind, ProductionShortageSupplyCondition,
    };

    let shortage = diagnosis.shortage_amount;
    if shortage <= 0.0 {
        return format!(
            "{good_name}: 供给 {:.1}/日覆盖需求 {:.1}/日。",
            supply, demand
        );
    }

    let primary_bucket = diagnosis
        .primary_bucket
        .map(demand_bucket_label)
        .unwrap_or("总需求");
    let primary_use = match diagnosis.primary_pressure {
        ProductionShortagePressureKind::BuildingInput => "建筑生产投入",
        ProductionShortagePressureKind::PopConsumption => "POP 消费",
        ProductionShortagePressureKind::MilitaryOrders => "军工/政府订单",
        ProductionShortagePressureKind::Construction => "建设材料",
        ProductionShortagePressureKind::ExportOrders => "出口订单",
        ProductionShortagePressureKind::GeneralDemand => "总需求",
    };
    let affected = if diagnosis.affected_building_count > 0 {
        format!("，影响 {} 个建筑链路", diagnosis.affected_building_count)
    } else if diagnosis.affected_bucket_count > 0 {
        format!("，影响 {} 个需求桶", diagnosis.affected_bucket_count)
    } else {
        String::new()
    };
    let supply_note = match diagnosis.supply_condition {
        ProductionShortageSupplyCondition::Stable => "",
        ProductionShortageSupplyCondition::DomesticMissing => "；没有稳定国内产出",
        ProductionShortageSupplyCondition::ImportsBlocked => "；进口受封锁影响，外部补给不稳定",
        ProductionShortageSupplyCondition::ImportsInsufficient => "；已有进口补给但仍不足",
        ProductionShortageSupplyCondition::StockpileBuffering => "；库存正在缓冲但无法完全覆盖缺口",
        ProductionShortageSupplyCondition::ProductionInsufficient => "；国内产能低于当前需求",
    };

    format!(
        "{good_name}: 缺口 {:.1}/日，供给 {:.1}/日低于需求 {:.1}/日；主要压力来自 {primary_bucket}/{primary_use}{affected}{supply_note}。",
        shortage, supply, demand
    )
}

fn market_action_entry_for(
    name_resolver: &DisplayNameResolver<'_>,
    v6_db: &hoi4_content::V6Database,
    action: &hoi4_logic::economy::production_chain::ProductionChainRecommendedAction,
    good_name: &str,
    shortage: f32,
) -> hoi4_ui::market_panel::MarketActionEntry {
    use hoi4_logic::economy::production_chain::ProductionChainRecommendedActionKind as Kind;

    let (title, description) = match action.kind {
        Kind::BuildProducer => {
            let building_label = action
                .building_id
                .as_deref()
                .map(|id| building_name(name_resolver, v6_db, id))
                .or_else(|| action.building_name.clone())
                .unwrap_or_else(|| "生产建筑".to_owned());
            let pm_label = action
                .production_method_id
                .as_deref()
                .map(|id| {
                    name_resolver.content_name(
                        DisplayNameKind::ProductionMethod,
                        id,
                        action.production_method_name.as_deref().unwrap_or(id),
                    )
                })
                .or_else(|| action.production_method_name.clone())
                .unwrap_or_else(|| "当前生产方法".to_owned());
            (
                format!("建设 {building_label}"),
                format!(
                    "{pm_label} 每级可提供 {:.1}/日，用于缓解 {good_name} 短缺。",
                    action.amount_per_level
                ),
            )
        }
        Kind::RestoreImportRoute => (
            format!("恢复 {good_name} 进口通道"),
            format!(
                "{good_name} 仍缺 {:.1}/日，优先处理封锁和运输中断。",
                shortage
            ),
        ),
        Kind::OpenImport => (
            format!("开辟 {good_name} 进口"),
            format!("{good_name} 国内供给不足，可通过世界市场或贸易路线临时补足。"),
        ),
        Kind::UseMarketBloc => (
            format!("调动市场圈 {good_name} 供给"),
            format!("优先检查同一市场圈成员的富余 {good_name}，降低外汇和运输压力。"),
        ),
        Kind::UseSubjectSupply => (
            format!("调动属地 {good_name} 供给"),
            format!("属地或傀儡供应可作为 {good_name} 短缺的中期缓冲。"),
        ),
        Kind::ReleaseStockpile => (
            format!("释放 {good_name} 库存"),
            format!("当前库存可缓冲部分缺口，同时需要补充产能或进口。"),
        ),
        Kind::PauseConstruction => (
            "暂停低优先级建设".to_owned(),
            format!("{good_name} 正被建设队列消耗，暂停低优先级项目可释放材料。"),
        ),
        Kind::ProtectPopConsumption => (
            format!("保障 POP 的 {good_name} 消费"),
            format!("短缺主要冲击民用消费，优先保供可降低满意度和收入链条压力。"),
        ),
        Kind::CutExports => (
            format!("削减 {good_name} 出口"),
            format!("出口订单正在占用供给，削减出口可先满足国内生产和消费。"),
        ),
    };

    hoi4_ui::market_panel::MarketActionEntry {
        title,
        description,
        related_good_id: Some(action.good_id.clone()),
        priority: action.priority,
    }
}

fn blockade_affected_buildings(
    name_resolver: &DisplayNameResolver<'_>,
    world: &World,
    v6_db: &hoi4_content::V6Database,
    player: usize,
) -> Vec<String> {
    let player_id = hoi4_state::CountryId(player as u16);
    let Some(market) = world.countries.market.markets.get(player) else {
        return Vec::new();
    };
    let imported_goods: Vec<String> = market
        .imports
        .iter()
        .filter(|(_, amount)| **amount > 0.0)
        .map(|(good_id, _)| good_id.clone())
        .collect();
    if imported_goods.is_empty() {
        return Vec::new();
    }

    let mut affected = Vec::new();
    for building in &world.countries.buildings_v6.buildings {
        let state_idx = building.state.0 as usize;
        if state_idx >= world.states.count
            || world.states.owners[state_idx] != player_id
            || building.level == 0
        {
            continue;
        }
        let pms = hoi4_content::active_pms_for_building(building, v6_db);
        let blocked_inputs: Vec<String> = pms
            .iter()
            .flat_map(|pm| pm.input_good_ids.iter())
            .filter(|good_id| imported_goods.contains(good_id))
            .map(|good_id| good_name(name_resolver, v6_db, good_id))
            .collect();
        if blocked_inputs.is_empty() {
            continue;
        }
        let state_name = state_name(name_resolver, world, state_idx);
        let building_name = building_name(name_resolver, v6_db, &building.building_def_id);
        let label = if state_name.is_empty() {
            format!("{} 缺少 {}", building_name, blocked_inputs.join("/"))
        } else {
            format!(
                "{} {} 缺少 {}",
                state_name,
                building_name,
                blocked_inputs.join("/")
            )
        };
        if !affected.contains(&label) {
            affected.push(label);
        }
        if affected.len() >= 6 {
            break;
        }
    }
    affected
}

fn trade_capacity(world: &World, country: hoi4_state::CountryId) -> f32 {
    let port_lvl: f32 = world
        .countries
        .buildings_v6
        .buildings
        .iter()
        .filter(|building| {
            let state_idx = building.state.0 as usize;
            state_idx < world.states.count
                && world.states.owners[state_idx] == country
                && building.building_def_id == "port"
        })
        .map(|building| building.level as f32)
        .sum();
    let rail_lvl: f32 = world
        .countries
        .buildings_v6
        .buildings
        .iter()
        .filter(|building| {
            let state_idx = building.state.0 as usize;
            state_idx < world.states.count
                && world.states.owners[state_idx] == country
                && building.building_def_id == "railway"
        })
        .map(|building| building.level as f32)
        .sum();
    if port_lvl > 0.0 || rail_lvl > 0.0 {
        (port_lvl * 10.0 + rail_lvl * 5.0).max(1.0)
    } else {
        5.0
    }
}

fn market_autonomy_level_label(level: hoi4_state::AutonomyLevel) -> &'static str {
    match level {
        hoi4_state::AutonomyLevel::Integrated => "整合属地",
        hoi4_state::AutonomyLevel::IntegratedPuppet => "整合傀儡",
        hoi4_state::AutonomyLevel::Puppet => "傀儡国",
        hoi4_state::AutonomyLevel::Dominion => "自治领",
        hoi4_state::AutonomyLevel::Satellite => "卫星国",
        hoi4_state::AutonomyLevel::FreedomAssociation => "自由联合",
    }
}
