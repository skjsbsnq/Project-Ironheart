// POP 面板 DTO 迁移目标模块。
// G02 后续把 main.rs 中的只读 POP 聚合迁入这里。

use std::collections::{HashMap, HashSet};

use hoi4_state::{
    BuildingId, CountryId, PopClass, PopGroup, StateId, StateIntegrationStatus, World,
};

use super::names::{DisplayNameKind, DisplayNameResolver};

pub fn panel_data(
    world: &World,
    v6_db: &hoi4_content::V6Database,
    loc_catalog: &hoi4_ui::loc::LocCatalog,
    player_country: usize,
) -> Option<hoi4_ui::pop_panel::PopPanelData> {
    build_pop_panel_data(world, v6_db, loc_catalog, player_country)
}

pub fn build_pop_panel_data(
    world: &World,
    v6_db: &hoi4_content::V6Database,
    loc_catalog: &hoi4_ui::loc::LocCatalog,
    player_country: usize,
) -> Option<hoi4_ui::pop_panel::PopPanelData> {
    let player = CountryId(player_country as u16);
    let state_ids = world.country_state_ids(player);
    let name_resolver = DisplayNameResolver::new(Some(loc_catalog));

    let mut total = PopAgg::default();
    let mut classes = [PopAgg::default(); PopClass::COUNT];
    let mut soldier_pool = 0_u64;
    let mut state_aggs: HashMap<StateId, PopAgg> = HashMap::new();
    let mut state_class_sizes: HashMap<StateId, [u64; PopClass::COUNT]> = HashMap::new();

    let state_id_set: HashSet<StateId> = state_ids.iter().copied().collect();

    for pg in world.countries.pops.groups.iter() {
        let state_idx = pg.state.0 as usize;
        if state_idx >= world.states.count
            || world.states.owners[state_idx] != player
            || !state_id_set.contains(&pg.state)
        {
            continue;
        }
        total.add(pg);
        classes[pg.class.index()].add(pg);
        state_aggs.entry(pg.state).or_default().add(pg);
        state_class_sizes.entry(pg.state).or_default()[pg.class.index()] += pg.size as u64;
        if pg.class == PopClass::Soldier && pg.employed_at.is_none() {
            soldier_pool += pg.size as u64;
        }
    }

    let class_entries: Vec<hoi4_ui::pop_panel::PopClassEntry> = (0..PopClass::COUNT)
        .filter_map(|idx| {
            let class = PopClass::from_index(idx)?;
            let agg = classes[idx];
            if agg.size == 0 {
                return None;
            }
            Some(hoi4_ui::pop_panel::PopClassEntry {
                class_name: name_resolver.pop_class_name(class),
                size: agg.size,
                employed: agg.employed,
                unemployed: agg.size.saturating_sub(agg.employed),
                avg_wage_rm: agg.avg_wage(),
                avg_tax_burden: agg.avg_tax(),
                avg_income_rm: agg.avg_income(),
                avg_tax_paid_rm: agg.avg_tax_paid(),
                avg_disposable_income_rm: agg.avg_disposable_income(),
                avg_satisfaction: agg.avg_satisfaction(),
                avg_loyalty: agg.avg_loyalty(),
                avg_standard_of_living: agg.avg_standard_of_living(),
                literacy: agg.avg_literacy(),
                skilled_ratio: agg.avg_skilled(),
                needs_fulfillment: agg.avg_needs(),
                essential_needs_fulfillment: agg.avg_essential_needs(),
                normal_needs_fulfillment: agg.avg_normal_needs(),
                luxury_needs_fulfillment: agg.avg_luxury_needs(),
                radicalism: agg.avg_radicalism(),
            })
        })
        .collect();

    let mut state_entries: Vec<hoi4_ui::pop_panel::PopStateEntry> = state_ids
        .iter()
        .map(|state_id| {
            let agg = state_aggs.get(state_id).copied().unwrap_or_default();
            let state_idx = state_id.0 as usize;
            let state_name = world
                .states
                .names
                .get(state_idx)
                .map(|raw| name_resolver.state_name(raw, state_idx))
                .unwrap_or_else(|| name_resolver.state_name("", state_idx));
            let integration_status = world.states.integration_status[state_idx];
            let integration_kind = if integration_status.is_colonial_or_occupied() {
                hoi4_ui::pop_panel::PopIntegrationKind::Colonial
            } else {
                hoi4_ui::pop_panel::PopIntegrationKind::Domestic
            };
            let dominant_class = state_class_sizes
                .get(state_id)
                .and_then(|sizes| {
                    sizes
                        .iter()
                        .enumerate()
                        .max_by_key(|(_, size)| **size)
                        .and_then(|(idx, _)| PopClass::from_index(idx))
                })
                .map(|class| name_resolver.pop_class_name(class))
                .unwrap_or_else(|| "未知人群".to_owned());
            hoi4_ui::pop_panel::PopStateEntry {
                state_name,
                state_id: state_id.0,
                integration_kind,
                integration_label: integration_label(integration_status).to_owned(),
                population: agg.size,
                employed: agg.employed,
                unemployment_rate: if agg.size > 0 {
                    agg.size.saturating_sub(agg.employed) as f32 / agg.size as f32
                } else {
                    0.0
                },
                avg_satisfaction: agg.avg_satisfaction(),
                avg_wage_rm: agg.avg_wage(),
                avg_income_rm: agg.avg_income(),
                avg_disposable_income_rm: agg.avg_disposable_income(),
                dominant_class,
            }
        })
        .collect();
    let zero_pop_alert: Vec<String> = state_entries
        .iter()
        .filter(|state| state.population == 0)
        .map(|state| state.state_name.clone())
        .collect();
    let zero_pop_count = zero_pop_alert.len();
    state_entries.sort_by(|a, b| b.population.cmp(&a.population));

    let mut building_employment =
        build_building_employment_entries(world, v6_db, &name_resolver, player);
    building_employment.sort_by(|a, b| {
        a.employment_rate
            .partial_cmp(&b.employment_rate)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a.qualification_rate
                    .partial_cmp(&b.qualification_rate)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| b.demand.cmp(&a.demand))
    });

    let workforce = total
        .size
        .saturating_sub(classes[PopClass::Soldier.index()].size);
    let employed = total.employed;
    let unemployed = total.size.saturating_sub(total.employed);
    let unemployment_rate = if total.size > 0 {
        unemployed as f32 / total.size as f32
    } else {
        0.0
    };
    let radicalism = total.avg_radicalism();
    let strike_risk = ((radicalism - 0.25) / 0.50).clamp(0.0, 1.0);
    let draft_resistance = ((radicalism - 0.15) / 0.55).clamp(0.0, 1.0);
    let mut political_pressures = Vec::new();
    if total.avg_essential_needs() < 0.8 {
        political_pressures.push(hoi4_ui::pop_panel::PopPoliticalPressureEntry {
            source: "生活必需品不足".to_owned(),
            pressure: ((0.8 - total.avg_essential_needs()) / 0.8).clamp(0.0, 1.0),
            description: format!(
                "基础需求满足度仅 {:.0}%，生活压力正在推高不满。",
                total.avg_essential_needs() * 100.0
            ),
        });
    }
    if unemployment_rate > 0.10 {
        political_pressures.push(hoi4_ui::pop_panel::PopPoliticalPressureEntry {
            source: "失业压力".to_owned(),
            pressure: ((unemployment_rate - 0.10) / 0.40).clamp(0.0, 1.0),
            description: format!(
                "失业率达到 {:.0}%，需要新增就业或降低劳动力冲击。",
                unemployment_rate * 100.0
            ),
        });
    }
    if total.avg_tax() > 0.50 {
        political_pressures.push(hoi4_ui::pop_panel::PopPoliticalPressureEntry {
            source: "税负过高".to_owned(),
            pressure: ((total.avg_tax() - 0.50) / 0.50).clamp(0.0, 1.0),
            description: format!(
                "平均税负达到 {:.0}%，可支配收入被明显压缩。",
                total.avg_tax() * 100.0
            ),
        });
    }
    if total.avg_satisfaction() < 0.45 {
        political_pressures.push(hoi4_ui::pop_panel::PopPoliticalPressureEntry {
            source: "满意度偏低".to_owned(),
            pressure: ((0.45 - total.avg_satisfaction()) / 0.45).clamp(0.0, 1.0),
            description: format!(
                "平均满意度仅 {:.0}%，社会稳定风险上升。",
                total.avg_satisfaction() * 100.0
            ),
        });
    }
    let avg_income = total.avg_income();
    let avg_disposable = total.avg_disposable_income();
    if avg_income > 0.0 && avg_disposable / avg_income < 0.5 {
        political_pressures.push(hoi4_ui::pop_panel::PopPoliticalPressureEntry {
            source: "可支配收入不足".to_owned(),
            pressure: ((0.5 - avg_disposable / avg_income) / 0.5).clamp(0.0, 1.0),
            description: format!(
                "税后可支配收入仅占收入 {:.0}%，消费能力不足。",
                avg_disposable / avg_income * 100.0
            ),
        });
    }
    let needs = vec![
        hoi4_ui::pop_panel::PopNeedEntry {
            tier_name: "基础需求".to_owned(),
            fulfillment: total.avg_essential_needs(),
            description: "粮食、燃料和基本生活品的满足情况。".to_owned(),
        },
        hoi4_ui::pop_panel::PopNeedEntry {
            tier_name: "普通需求".to_owned(),
            fulfillment: total.avg_normal_needs(),
            description: "日常消费品和服务的满足情况。".to_owned(),
        },
        hoi4_ui::pop_panel::PopNeedEntry {
            tier_name: "奢侈需求".to_owned(),
            fulfillment: total.avg_luxury_needs(),
            description: "高收入人群奢侈品和高级服务的满足情况。".to_owned(),
        },
    ];
    Some(hoi4_ui::pop_panel::PopPanelData {
        total_population: total.size,
        workforce,
        employed,
        unemployed,
        unemployment_rate,
        average_wage_rm: total.avg_wage(),
        average_income_rm: total.avg_income(),
        average_disposable_income_rm: total.avg_disposable_income(),
        average_satisfaction: total.avg_satisfaction(),
        average_loyalty: total.avg_loyalty(),
        average_standard_of_living: total.avg_standard_of_living(),
        literacy: total.avg_literacy(),
        skilled_ratio: total.avg_skilled(),
        needs_fulfillment: total.avg_needs(),
        essential_needs_fulfillment: total.avg_essential_needs(),
        normal_needs_fulfillment: total.avg_normal_needs(),
        luxury_needs_fulfillment: total.avg_luxury_needs(),
        radicalism,
        strike_risk,
        draft_resistance,
        soldier_pool,
        classes: class_entries,
        states: state_entries,
        building_employment,
        needs,
        political_pressures,
        alerts: if total.size == 0 {
            vec!["缺少人口数据".to_owned()]
        } else if zero_pop_count > 0 {
            vec![format!(
                "{} 个州缺少人口数据：{}",
                zero_pop_count,
                zero_pop_alert.join(", ")
            )]
        } else {
            Vec::new()
        },
    })
}

fn build_building_employment_entries(
    world: &World,
    v6_db: &hoi4_content::V6Database,
    name_resolver: &DisplayNameResolver<'_>,
    player: CountryId,
) -> Vec<hoi4_ui::pop_panel::PopBuildingEmploymentEntry> {
    world
        .countries
        .buildings_v6
        .buildings
        .iter()
        .enumerate()
        .filter_map(|(building_idx, building)| {
            let state_idx = building.state.0 as usize;
            if state_idx >= world.states.count
                || world.states.owners[state_idx] != player
                || building.level == 0
            {
                return None;
            }

            let pms = hoi4_content::active_pms_for_building(building, v6_db);
            let demand_per_level: u32 = pms
                .iter()
                .flat_map(|pm| pm.employment_demand.iter())
                .copied()
                .sum();
            let demand = demand_per_level.saturating_mul(building.level as u32);
            let employed: u32 = building.employment.iter().copied().sum();
            if demand == 0 && employed == 0 {
                return None;
            }

            let building_name = building_name(name_resolver, v6_db, &building.building_def_id);
            let state_name = state_name(name_resolver, world, state_idx);
            let (qualification_rate, skill_gap_label) =
                building_qualification(world, building_idx, &pms);

            Some(hoi4_ui::pop_panel::PopBuildingEmploymentEntry {
                building_name,
                state_name,
                level: building.level,
                employed,
                demand,
                employment_rate: if demand > 0 {
                    employed as f32 / demand as f32
                } else {
                    1.0
                }
                .clamp(0.0, 1.0),
                qualification_rate,
                skill_gap_label,
            })
        })
        .collect()
}

fn building_name(
    name_resolver: &DisplayNameResolver<'_>,
    v6_db: &hoi4_content::V6Database,
    building_id: &str,
) -> String {
    v6_db
        .buildings
        .iter()
        .find(|def| def.id == building_id)
        .map(|def| {
            name_resolver.content_name(DisplayNameKind::Building, &def.id, def.name.as_str())
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

fn building_qualification(
    world: &World,
    building_idx: usize,
    pms: &[&hoi4_content::ProductionMethodDef],
) -> (f32, String) {
    let required_literacy = pms
        .iter()
        .map(|pm| pm.required_literacy)
        .fold(0.0_f32, f32::max);
    let required_skilled = pms
        .iter()
        .map(|pm| pm.required_skilled_ratio)
        .fold(0.0_f32, f32::max);
    if required_literacy <= 0.0 && required_skilled <= 0.0 {
        return (1.0, "无".to_owned());
    }

    let building_id = BuildingId(building_idx as u32);
    let mut total = 0.0_f32;
    let mut literacy = 0.0_f32;
    let mut skilled = 0.0_f32;
    for pg in &world.countries.pops.groups {
        if pg.employed_at != Some(building_id) {
            continue;
        }
        let weight = pg.size as f32;
        total += weight;
        literacy += pg.literacy * weight;
        skilled += pg.skilled_ratio * weight;
    }
    if total <= 0.0 {
        return (0.0, "缺少合格工人".to_owned());
    }

    let avg_literacy = literacy / total;
    let avg_skilled = skilled / total;
    let literacy_rate = if required_literacy > 0.0 {
        avg_literacy / required_literacy
    } else {
        1.0
    };
    let skilled_rate = if required_skilled > 0.0 {
        avg_skilled / required_skilled
    } else {
        1.0
    };
    let rate = literacy_rate.min(skilled_rate).clamp(0.0, 1.0);
    let label = if literacy_rate < 1.0 && literacy_rate <= skilled_rate {
        "识字率不足"
    } else if skilled_rate < 1.0 {
        "熟练工不足"
    } else {
        "达标"
    };
    (rate, label.to_owned())
}

fn integration_label(status: StateIntegrationStatus) -> &'static str {
    match status {
        StateIntegrationStatus::Metropole => "本土核心",
        StateIntegrationStatus::Incorporated => "整合州",
        StateIntegrationStatus::Colony => "殖民地",
        StateIntegrationStatus::Protectorate => "保护领",
        StateIntegrationStatus::Mandate => "委任统治地",
        StateIntegrationStatus::Concession => "租借地",
        StateIntegrationStatus::Occupied => "占领区",
    }
}

#[derive(Clone, Copy, Default)]
struct PopAgg {
    size: u64,
    employed: u64,
    wage_weighted: f64,
    tax_weighted: f64,
    income_weighted: f64,
    tax_paid_weighted: f64,
    disposable_income_weighted: f64,
    satisfaction_weighted: f64,
    loyalty_weighted: f64,
    standard_of_living_weighted: f64,
    literacy_weighted: f64,
    skilled_weighted: f64,
    needs_weighted: f64,
    essential_needs_weighted: f64,
    normal_needs_weighted: f64,
    luxury_needs_weighted: f64,
    radicalism_weighted: f64,
}

impl PopAgg {
    fn add(&mut self, pg: &PopGroup) {
        let size = pg.size as u64;
        self.size += size;
        if pg.employed_at.is_some() || pg.class == PopClass::Soldier {
            self.employed += size;
        }
        let weight = pg.size as f64;
        self.wage_weighted += pg.wage_rm as f64 * weight;
        self.tax_weighted += pg.tax_burden as f64 * weight;
        self.income_weighted += pg.income_rm as f64 * weight;
        self.tax_paid_weighted += pg.tax_paid_rm as f64 * weight;
        self.disposable_income_weighted += pg.disposable_income_rm as f64 * weight;
        self.satisfaction_weighted += pg.satisfaction as f64 * weight;
        self.loyalty_weighted += pg.political_loyalty as f64 * weight;
        self.standard_of_living_weighted += pg.standard_of_living as f64 * weight;
        self.literacy_weighted += pg.literacy as f64 * weight;
        self.skilled_weighted += pg.skilled_ratio as f64 * weight;
        self.needs_weighted += pg.needs_fulfillment as f64 * weight;
        self.essential_needs_weighted += pg.essential_needs_fulfillment as f64 * weight;
        self.normal_needs_weighted += pg.normal_needs_fulfillment as f64 * weight;
        self.luxury_needs_weighted += pg.luxury_needs_fulfillment as f64 * weight;
        self.radicalism_weighted += pg.radicalism as f64 * weight;
    }

    fn weighted_avg(self, value: f64) -> f32 {
        if self.size > 0 {
            (value / self.size as f64) as f32
        } else {
            0.0
        }
    }

    fn avg_wage(self) -> f32 {
        self.weighted_avg(self.wage_weighted)
    }

    fn avg_tax(self) -> f32 {
        self.weighted_avg(self.tax_weighted)
    }

    fn avg_income(self) -> f32 {
        self.weighted_avg(self.income_weighted)
    }

    fn avg_tax_paid(self) -> f32 {
        self.weighted_avg(self.tax_paid_weighted)
    }

    fn avg_disposable_income(self) -> f32 {
        self.weighted_avg(self.disposable_income_weighted)
    }

    fn avg_satisfaction(self) -> f32 {
        self.weighted_avg(self.satisfaction_weighted)
    }

    fn avg_loyalty(self) -> f32 {
        self.weighted_avg(self.loyalty_weighted)
    }

    fn avg_standard_of_living(self) -> f32 {
        self.weighted_avg(self.standard_of_living_weighted)
    }

    fn avg_needs(self) -> f32 {
        self.weighted_avg(self.needs_weighted)
    }

    fn avg_literacy(self) -> f32 {
        self.weighted_avg(self.literacy_weighted)
    }

    fn avg_skilled(self) -> f32 {
        self.weighted_avg(self.skilled_weighted)
    }

    fn avg_essential_needs(self) -> f32 {
        self.weighted_avg(self.essential_needs_weighted)
    }

    fn avg_normal_needs(self) -> f32 {
        self.weighted_avg(self.normal_needs_weighted)
    }

    fn avg_luxury_needs(self) -> f32 {
        self.weighted_avg(self.luxury_needs_weighted)
    }

    fn avg_radicalism(self) -> f32 {
        self.weighted_avg(self.radicalism_weighted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hoi4_content::v6_loader::{
        BuildingDef, BuildingEmploymentProfileDef, BuildingGameplayClassDef,
        BuildingGdpComponentDef, BuildingGdpRuleDef, BuildingKindDef, ConstructionRecipeDef,
        EconomicSectorDef, OwnerDef,
    };
    use hoi4_content::{ProductionMethodDef, V6Database};
    use hoi4_state::{Building, BuildingId, BuildingKind, BuildingOwner};
    use std::sync::Arc;

    fn empty_world() -> World {
        let mut data = hoi4_data::GameData::default();
        let tag = hoi4_data::CountryTag::new("TST");
        data.countries.insert(
            tag.clone(),
            hoi4_data::Country {
                tag: tag.clone(),
                color: hoi4_data::Color {
                    r: 80,
                    g: 80,
                    b: 80,
                },
                graphical_culture: "test_gfx".to_owned(),
                capital: 1,
                ruling_party: "neutrality".to_owned(),
                technologies: vec![],
            },
        );
        data.states.push(hoi4_data::State {
            id: 1,
            name: "Test State".to_owned(),
            manpower: 1_000_000,
            owner: tag.clone(),
            cores: vec![tag],
            provinces: vec![],
            category: "city".to_owned(),
            infrastructure: 0,
            victory_points: vec![],
            resources: vec![],
        });

        World::new(
            Arc::new(hoi4_map::GameMap {
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
            }),
            Arc::new(data),
        )
    }

    #[test]
    fn pop_panel_exposes_state_class_and_building_employment() {
        let mut world = empty_world();
        world.countries.buildings_v6.buildings.push(Building {
            kind: BuildingKind::Industrial,
            building_def_id: "advanced_plant".to_owned(),
            state: StateId(0),
            level: 1,
            active_pm: "advanced_plant_default".to_owned(),
            employment: [0, 50, 0, 0, 0, 0],
            owner: BuildingOwner::Private,
            requires_law: None,
            built_progress: 1.0,
            ..Building::runtime_defaults()
        });
        world.countries.pops.groups.push(PopGroup {
            class: PopClass::Worker,
            state: StateId(0),
            size: 50,
            employed_at: Some(BuildingId(0)),
            wage_rm: 5.0,
            tax_burden: 0.20,
            income_rm: 5.0,
            tax_paid_rm: 1.0,
            disposable_income_rm: 4.0,
            basic_consumption_budget: 3.0,
            satisfaction_law_modifier: 0.0,
            loyalty_coefficient: 1.0,
            loyalty_decay_mult: 1.0,
            satisfaction: 0.65,
            political_loyalty: 0.2,
            literacy: 0.80,
            skilled_ratio: 0.50,
            standard_of_living: 0.6,
            needs_fulfillment: 0.75,
            essential_needs_fulfillment: 0.90,
            normal_needs_fulfillment: 0.70,
            luxury_needs_fulfillment: 0.40,
            radicalism: 0.05,
        });
        world.countries.pops.groups.push(PopGroup {
            class: PopClass::Worker,
            state: StateId(0),
            size: 50,
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
            satisfaction: 0.45,
            political_loyalty: 0.0,
            literacy: 0.55,
            skilled_ratio: 0.18,
            standard_of_living: 0.4,
            needs_fulfillment: 0.60,
            essential_needs_fulfillment: 0.70,
            normal_needs_fulfillment: 0.55,
            luxury_needs_fulfillment: 0.20,
            radicalism: 0.15,
        });

        let mut db = V6Database::default();
        db.buildings.push(BuildingDef {
            id: "advanced_plant".to_owned(),
            name: "Advanced Plant".to_owned(),
            description: String::new(),
            economic_sector: EconomicSectorDef::Secondary,
            gameplay_class: BuildingGameplayClassDef::HeavyIndustry,
            gdp_rule: BuildingGdpRuleDef {
                component: BuildingGdpComponentDef::SecondaryOutput,
                value_added_multiplier: 1.0,
            },
            kind: BuildingKindDef::Industrial,
            max_level: 5,
            owner_default: OwnerDef::Private,
            buildable: true,
            group: String::new(),
            state_limit_kind: None,
            requires_law: None,
            employment_profile: BuildingEmploymentProfileDef {
                peasants: 0,
                workers: 100,
                clerks: 0,
                capitalists: 0,
                aristocrats: 0,
                soldiers: 0,
            },
            construction_recipe: ConstructionRecipeDef {
                cp_cost: 1.0,
                funds_rm: 1.0,
                materials: vec![],
                labor: 0,
                engineering: 0,
                regional_restrictions: vec![],
            },
        });
        db.production_methods.push(ProductionMethodDef {
            id: "advanced_plant_default".to_owned(),
            name: "Advanced Plant Default".to_owned(),
            building_id: "advanced_plant".to_owned(),
            group: "base".to_owned(),
            group_name: "Base".to_owned(),
            input_good_ids: vec![],
            input_good_amounts: vec![],
            output_good_ids: vec![],
            output_good_amounts: vec![],
            employment_demand: [0, 100, 0, 0, 0, 0],
            unlocked_by: None,
            required_law: None,
            throughput_modifier: 1.0,
            automation_modifier: 1.0,
            required_literacy: 0.60,
            required_skilled_ratio: 0.40,
            equipment_output: None,
        });

        let data = build_pop_panel_data(&world, &db, &hoi4_ui::loc::LocCatalog::new(), 0).unwrap();

        assert_eq!(data.states.len(), 1);
        assert_eq!(data.states[0].population, 100);
        assert!((data.states[0].unemployment_rate - 0.5).abs() < 0.01);

        let worker = data
            .classes
            .iter()
            .find(|entry| entry.size == 100)
            .expect("worker class should be visible");
        assert!(worker.avg_income_rm > 0.0);
        assert!(worker.avg_tax_paid_rm > 0.0);
        assert!(worker.avg_disposable_income_rm > 0.0);
        assert!(worker.avg_satisfaction > 0.0);
        assert!(worker.literacy > 0.0);
        assert!(worker.skilled_ratio > 0.0);

        let building = data
            .building_employment
            .iter()
            .find(|entry| entry.building_name == "Advanced Plant")
            .expect("building employment should be visible");
        assert_eq!(building.demand, 100);
        assert_eq!(building.employed, 50);
        assert!((building.employment_rate - 0.5).abs() < 0.01);
        assert!((building.qualification_rate - 1.0).abs() < 0.01);
    }
}
