//! 生产决策：政府军购订单 + 建设规划。

use hoi4_content::V6Database;
use hoi4_logic::economy::construction_planner::{
    plan_construction, ConstructionCandidateScore, ConstructionPlanningMode,
};
use hoi4_logic::economy::{BuildOrder, EconomyState};
use hoi4_logic::military::collect_military_demand;
use hoi4_state::{CountryId, World};

use crate::intent::NationalIntent;
use crate::profile::AiProfile;

#[derive(Debug, Clone)]
pub struct ProductionDecision {
    pub construction: Vec<BuildOrder>,
    pub government_orders: Vec<(String, f64, u32)>,
    pub production_lines: Vec<(String, u32)>,
}

#[derive(Debug, Clone)]
pub struct ProductionEvaluation {
    pub decision: ProductionDecision,
    pub reasoning: String,
}

pub fn evaluate_production(
    world: &World,
    country: CountryId,
    econ: &EconomyState,
    profile: &AiProfile,
    intent: &NationalIntent,
    db: &V6Database,
) -> ProductionDecision {
    let ci = country.0 as usize;
    if ci >= world.countries.count {
        return ProductionDecision {
            construction: Vec::new(),
            government_orders: Vec::new(),
            production_lines: Vec::new(),
        };
    }

    let military_demand = collect_military_demand(world, world.data.as_ref(), country);
    let government_orders = decide_government_orders(econ, country, &military_demand);
    let construction = decide_construction(world, country, econ, profile, intent, db);
    let production_lines = decide_production_lines(world, country, econ);

    ProductionDecision {
        construction,
        government_orders,
        production_lines,
    }
}

pub fn apply_production_decision(
    econ: &mut EconomyState,
    world: &World,
    country: CountryId,
    decision: &ProductionDecision,
) {
    for (equipment_category, budget_rm, duration_days) in &decision.government_orders {
        econ.upsert_government_order(country, equipment_category, *budget_rm, *duration_days);
    }
    for order in &decision.construction {
        econ.enqueue_construction(country, order.clone(), world);
    }
    for (equip_id, factories) in &decision.production_lines {
        econ.add_production_line(country, equip_id.clone(), *factories);
    }
}

fn decide_government_orders(
    econ: &EconomyState,
    country: CountryId,
    demand: &hoi4_logic::military::MilitaryDemand,
) -> Vec<(String, f64, u32)> {
    let ci = country.0 as usize;
    let stockpile = econ.stockpile.get(ci).cloned().unwrap_or_default();
    let mut orders = Vec::new();

    for (equipment, needed) in &demand.equipment_needed {
        let have = stockpile.get(equipment).copied().unwrap_or(0.0);
        let missing = (needed - have).max(0.0);
        if missing <= 0.0 {
            continue;
        }
        let budget_rm =
            missing as f64 * hoi4_logic::military::equipment_unit_cost_rm(equipment) / 30.0;
        orders.push((equipment.clone(), budget_rm, 30));
    }

    orders.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    orders.truncate(8);
    orders
}

fn decide_construction(
    world: &World,
    country: CountryId,
    econ: &EconomyState,
    profile: &AiProfile,
    intent: &NationalIntent,
    db: &V6Database,
) -> Vec<BuildOrder> {
    let cp = (intent.target_civ_factories + intent.target_mil_factories)
        .max(12)
        .min(120);
    let plan = plan_construction(
        world,
        econ,
        db,
        country,
        ConstructionPlanningMode::AiCountry,
        cp,
    );
    let max_orders = if profile.aggression > 0.7 { 3 } else { 2 };
    scored_candidates_to_orders(world, country, econ, plan.candidates, max_orders)
}

fn decide_production_lines(
    world: &World,
    country: CountryId,
    econ: &EconomyState,
) -> Vec<(String, u32)> {
    let ci = country.0 as usize;
    let (_, total_mil) = v6_industry_counts(world, country);

    if total_mil == 0 {
        return Vec::new();
    }

    let unlocked = &world.countries.unlocked_equipments[ci];
    let allocated: u32 = econ.production[ci]
        .iter()
        .map(|line| line.assigned_factories)
        .sum();
    let free_factories = total_mil.saturating_sub(allocated);

    if econ.production[ci].is_empty() {
        let inf_equip = find_best_equipment(unlocked, &world.data.equipment, "infantry");
        let art_equip = find_best_equipment(unlocked, &world.data.equipment, "artillery");
        let sup_equip = find_best_equipment(unlocked, &world.data.equipment, "support_equipment");

        let inf_need = (total_mil as f32 * 0.60).ceil() as u32;
        let art_need = (total_mil as f32 * 0.20).ceil() as u32;

        let mut lines = Vec::new();
        let mut remaining = total_mil;

        if let Some(eq) = inf_equip {
            let alloc = inf_need.min(remaining);
            if alloc > 0 {
                lines.push((eq, alloc));
                remaining -= alloc;
            }
        }
        if remaining > 0 {
            if let Some(eq) = art_equip {
                let alloc = art_need.min(remaining);
                if alloc > 0 {
                    lines.push((eq, alloc));
                    remaining -= alloc;
                }
            }
        }
        if remaining > 0 {
            if let Some(eq) = sup_equip {
                lines.push((eq, remaining));
            }
        }
        return lines;
    }

    if free_factories > 0 {
        if let Some(inf_equip) = find_best_equipment(unlocked, &world.data.equipment, "infantry") {
            return vec![(inf_equip, free_factories)];
        }
    }

    Vec::new()
}

fn scored_candidates_to_orders(
    world: &World,
    country: CountryId,
    econ: &EconomyState,
    candidates: Vec<ConstructionCandidateScore>,
    max_orders: usize,
) -> Vec<BuildOrder> {
    let ci = country.0 as usize;
    let mut orders = Vec::new();
    for candidate in candidates.into_iter() {
        if orders.len() >= max_orders {
            break;
        }
        if candidate.state.0 as usize >= world.states.count {
            continue;
        }
        let already = econ.construction[ci].items.iter().any(|item| {
            item.building_key == candidate.building_id && item.target_state == candidate.state
        });
        if already {
            continue;
        }
        orders.push(BuildOrder::new(candidate.building_id, candidate.state));
    }
    orders
}

fn v6_industry_counts(world: &World, country: CountryId) -> (u32, u32) {
    let mut civilian = 0u32;
    let mut military = 0u32;
    for building in &world.countries.buildings_v6.buildings {
        let state_idx = building.state.0 as usize;
        if state_idx >= world.states.count
            || world.states.owners[state_idx] != country
            || building.level == 0
        {
            continue;
        }
        if building.kind == hoi4_state::BuildingKind::Military {
            military += building.level as u32;
        } else if building.kind != hoi4_state::BuildingKind::MilitaryBase {
            civilian += building.level as u32;
        }
    }
    (civilian, military)
}

fn find_best_equipment(
    unlocked: &std::collections::HashSet<String>,
    equipment: &std::collections::HashMap<String, hoi4_data::EquipmentDef>,
    category_keyword: &str,
) -> Option<String> {
    use hoi4_data::EquipmentCategory;

    let cat_matches = |cat: &EquipmentCategory, kw: &str| -> bool {
        matches!(
            (cat, kw),
            (EquipmentCategory::Infantry, "infantry")
                | (EquipmentCategory::Artillery, "artillery")
                | (EquipmentCategory::SupportEquipment, "support_equipment")
                | (EquipmentCategory::AntiTank, "anti_tank")
                | (EquipmentCategory::AntiAir, "anti_air")
                | (EquipmentCategory::Motorized, "motorized")
                | (EquipmentCategory::Mechanized, "mechanized")
                | (EquipmentCategory::Tank, "tank")
                | (EquipmentCategory::Plane, "plane")
        )
    };

    let mut candidates: Vec<(&String, u16)> = unlocked
        .iter()
        .filter_map(|key| {
            let def = equipment.get(key)?;
            let key_lower = key.to_lowercase();
            let match_by_key = key_lower.contains(category_keyword);
            let match_by_cat = cat_matches(&def.category, category_keyword);
            if (match_by_key || match_by_cat) && def.is_buildable {
                Some((key, def.year))
            } else {
                None
            }
        })
        .collect();

    candidates.sort_by(|a, b| b.1.cmp(&a.1));
    candidates.first().map(|(k, _)| (*k).clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hoi4_content::{v6_loader::*, V6Database};
    use hoi4_data::{Country, CountryTag, DivisionTemplate, GameData, SubunitDef};
    use hoi4_logic::economy::EconomyState;
    use hoi4_state::{Building, BuildingKind, BuildingOwner, CountryId, StateId, World};
    use std::collections::HashMap;
    use std::sync::Arc;

    fn test_world() -> World {
        let mut data = GameData::default();
        let tag = CountryTag::new("GER");
        data.countries.insert(
            tag.clone(),
            Country {
                tag: tag.clone(),
                color: hoi4_data::Color {
                    r: 80,
                    g: 80,
                    b: 80,
                },
                graphical_culture: "western_european_gfx".to_owned(),
                capital: 1,
                ruling_party: "fascism".to_owned(),
                technologies: vec![],
            },
        );
        data.states.push(hoi4_data::State {
            id: 1,
            name: "Test State".to_owned(),
            manpower: 1_000_000,
            owner: tag.clone(),
            cores: vec![tag.clone()],
            provinces: vec![0],
            category: "city".to_owned(),
            infrastructure: 5,
            victory_points: vec![],
            resources: vec![],
        });
        data.subunits.insert(
            "infantry".to_owned(),
            SubunitDef {
                key: "infantry".to_owned(),
                manpower: 1_000,
                need: HashMap::from([("infantry_equipment".to_owned(), 100)]),
                ..SubunitDef::default()
            },
        );
        data.division_templates.insert(
            "GER".to_owned(),
            vec![DivisionTemplate {
                name: "Infantry Division".to_owned(),
                country_tag: Some("GER".to_owned()),
                regiments: vec!["infantry".to_owned()],
                support: vec![],
                division_names_group: None,
            }],
        );

        let mut world = World::new(
            Arc::new(hoi4_map::GameMap {
                definitions: vec![],
                rgb_to_id: HashMap::new(),
                province_map: hoi4_map::ProvinceMap {
                    width: 1,
                    height: 1,
                    pixels: vec![0],
                },
                adjacencies: vec![vec![]],
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
            }),
            Arc::new(data),
        );
        world.states.controllers[0] = CountryId(0);
        world.states.owners[0] = CountryId(0);
        world.states.provinces[0] = vec![hoi4_state::ProvinceId(0)];
        world.countries.unlocked_equipments[0].insert("infantry_equipment_1".to_owned());
        world.countries.buildings_v6.buildings.push(Building {
            kind: BuildingKind::Military,
            building_def_id: "arms_industry".to_owned(),
            state: StateId(0),
            level: 1,
            active_pm: String::new(),
            active_pm_by_group: vec![],
            employment: [0; 6],
            owner: BuildingOwner::State,
            ownership_shares: vec![],
            requires_law: None,
            production_rate: 0.0,
            output_value_gbp: 0.0,
            wage_rm: 0.0,
            profit_rm: 0.0,
            input_cost_rm: 0.0,
            estimated_profit_rm: 0.0,
            value_added_rm: 0.0,
            cp_cost: 0.0,
            max_level: 5,
            built_progress: 0.0,
        });
        world.divisions.push(
            CountryId(0),
            hoi4_state::ProvinceId(0),
            0,
            30.0,
            100.0,
            "Test Div".to_owned(),
        );
        world
    }

    fn test_v6_db() -> V6Database {
        let mut db = V6Database::default();
        db.buildings.push(BuildingDef {
            id: "arms_industry".to_owned(),
            name: "Arms Industry".to_owned(),
            description: "Test arms industry".to_owned(),
            economic_sector: EconomicSectorDef::Secondary,
            gameplay_class: BuildingGameplayClassDef::Military,
            kind: BuildingKindDef::Military,
            max_level: 5,
            owner_default: OwnerDef::State,
            buildable: true,
            group: String::new(),
            state_limit_kind: None,
            requires_law: None,
            employment_profile: BuildingEmploymentProfileDef {
                peasants: 0,
                workers: 10,
                clerks: 0,
                capitalists: 0,
                aristocrats: 0,
                soldiers: 0,
            },
            construction_recipe: ConstructionRecipeDef {
                cp_cost: 1_000.0,
                funds_rm: 50_000_000.0,
                materials: vec![ConstructionMaterialDef {
                    good_id: "steel".to_owned(),
                    amount: 20.0,
                }],
                labor: 10,
                engineering: 5,
                regional_restrictions: Vec::new(),
            },
        });
        db.production_methods.push(ProductionMethodDef {
            id: "arms_industry_default".to_owned(),
            name: "Default".to_owned(),
            building_id: "arms_industry".to_owned(),
            group: "base".to_owned(),
            group_name: "基础工艺".to_owned(),
            input_good_ids: vec![],
            input_good_amounts: vec![],
            output_good_ids: vec![],
            output_good_amounts: vec![],
            employment_demand: [0; 6],
            unlocked_by: None,
            required_law: None,
            throughput_modifier: 1.0,
            automation_modifier: 1.0,
            required_literacy: 0.0,
            required_skilled_ratio: 0.0,
            equipment_output: Some(EquipmentOutputDef {
                equipment_category: "infantry_equipment".to_owned(),
                daily_per_level: 10.0,
            }),
        });
        db.production_methods.push(ProductionMethodDef {
            id: "steel_mill_default".to_owned(),
            name: "Default".to_owned(),
            building_id: "steel_mill".to_owned(),
            group: "base".to_owned(),
            group_name: "基础工艺".to_owned(),
            input_good_ids: vec![],
            input_good_amounts: vec![],
            output_good_ids: vec!["steel".to_owned()],
            output_good_amounts: vec![10.0],
            employment_demand: [0; 6],
            unlocked_by: None,
            required_law: None,
            throughput_modifier: 1.0,
            automation_modifier: 1.0,
            required_literacy: 0.0,
            required_skilled_ratio: 0.0,
            equipment_output: None,
        });
        db.goods.push(GoodDef {
            id: "steel".to_owned(),
            name: "Steel".to_owned(),
            category: GoodCategoryDef::Intermediate,
            base_price_rm: 1.0,
            unlocked_by: None,
        });
        db.goods.push(GoodDef {
            id: "infantry_equipment".to_owned(),
            name: "Infantry Equipment".to_owned(),
            category: GoodCategoryDef::MilitaryIntermediate,
            base_price_rm: 1.0,
            unlocked_by: None,
        });
        db
    }

    #[test]
    fn production_generates_government_orders() {
        let world = test_world();
        let econ = EconomyState::new(&world);
        let profile = AiProfile::default();
        let intent = NationalIntent::default_at_peace(&profile);
        let decision = evaluate_production(
            &world,
            CountryId(0),
            &econ,
            &profile,
            &intent,
            &test_v6_db(),
        );
        assert!(!decision.government_orders.is_empty());
    }
}
