use std::collections::BTreeMap;
use std::time::Instant;

use hoi4_content::{active_pms_for_building, ProductionMethodDef, V6Database};
use hoi4_integration::{init_simulation, load_world};
use hoi4_runtime::{ContentRuntimeState, HourlyRuntime, SystemSchedule};
use hoi4_state::{
    market::{DemandBucketKind, GoodClearingResult},
    Building, CountryId, PopClass, World,
};

#[test]
#[ignore]
fn germany_production_chain_365d_diagnostic() {
    let mut world = load_world().expect("HOI4 install required");
    let (mut econ, mut research, mut politics_cache, mut script, mut ai) =
        init_simulation(&mut world);
    let db = V6Database::load();
    let mut schedule = SystemSchedule::with_phase1_systems();
    let mut feedback_bus = hoi4_logic::feedback::FeedbackBus::new();

    let ger_idx = world.country("GER").expect("GER must exist").0 as usize;
    let mut content = ContentRuntimeState::empty_for_test(
        CountryId(ger_idx as u16),
        world.date.days_since_epoch(),
    );

    let t0 = Instant::now();
    for day in 1..=365 {
        for _ in 0..24 {
            let _ = hoi4_runtime::tick_one_hour(&mut HourlyRuntime {
                world: &mut world,
                econ: &mut econ,
                research: &mut research,
                politics_cache: &mut politics_cache,
                script: &mut script,
                ai: &mut ai,
                v6_db: &db,
                schedule: &mut schedule,
                feedback_bus: &mut feedback_bus,
                content: &mut content,
            });
        }

        if matches!(day, 1 | 30 | 90 | 180 | 365) {
            print_market_snapshot(&world, ger_idx, day);
        }
    }

    let elapsed = t0.elapsed().as_secs_f64();
    println!(
        "\n[ger-prod] 365 days in {:.3}s ({:.1} ms/day), date={}",
        elapsed,
        elapsed * 1000.0 / 365.0,
        world.date
    );
    println!("[ger-prod] {}", schedule.report_systems());
    println!("[ger-prod] timing {}", schedule.timing_summary_report());

    print_goods_table(&world, ger_idx);
    print_building_table(&world, &db, ger_idx);
    print_equipment_stockpile(&econ, ger_idx);
    print_population_table(&world, ger_idx);
    assert_germany_chain_healthy(&world, &econ, ger_idx);
}

fn assert_germany_chain_healthy(
    world: &World,
    econ: &hoi4_logic::economy::EconomyState,
    ci: usize,
) {
    for good in [
        "steel",
        "machinery",
        "machine_tools",
        "engines",
        "vehicle_parts",
        "rubber_parts",
        "small_arms_parts",
        "gun_barrels",
        "tank_hulls",
        "armor_plate",
        "radio_sets",
        "optics",
        "airframes",
        "aluminium",
    ] {
        assert_good_flow(world, ci, good);
    }

    for equipment in [
        "infantry_equipment",
        "support_equipment",
        "artillery",
        "motorized",
        "armor",
        "aircraft",
    ] {
        let amount = econ.stockpile_of(CountryId(ci as u16), equipment);
        assert!(
            amount >= 0.0,
            "GER {equipment} stockpile went negative after 365 days: {amount:.1}"
        );
    }
}

fn assert_good_flow(world: &World, ci: usize, good: &str) {
    let result = world.countries.market.markets[ci]
        .clearing_sheet
        .results
        .get(good)
        .unwrap_or_else(|| panic!("GER final clearing missing good {good}"));
    assert!(
        result.domestic_production > 0.01,
        "GER {good} has no domestic production after 365 days"
    );
    assert!(
        result.stockpile_coverage_days >= 10.0,
        "GER {good} stockpile cover too low after 365 days: {:.1} days",
        result.stockpile_coverage_days
    );
    assert!(
        result.shortage_ratio < 0.75,
        "GER {good} shortage too high after 365 days: {:.1}%",
        result.shortage_ratio * 100.0
    );
}

fn print_market_snapshot(world: &World, ci: usize, day: u32) {
    let market = &world.countries.market.markets[ci];
    let mut rows: Vec<_> = market
        .clearing_sheet
        .results
        .iter()
        .filter(|(_, result)| result.total_unmet > 0.1 || result.shortage_ratio > 0.05)
        .map(|(good, result)| {
            (
                good.as_str(),
                result.shortage_ratio,
                result.total_unmet,
                result.domestic_production,
                result.imports,
                result.stockpile_closing,
            )
        })
        .collect();
    rows.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal))
    });
    println!("\n[ger-prod] day {day} shortage snapshot");
    for (good, ratio, unmet, domestic, imports, stockpile) in rows.into_iter().take(10) {
        println!(
            "  {good:<18} shortage={:>6.1}% unmet={:>8.2} domestic={:>8.2} imports={:>7.2} stock={:>8.2}",
            ratio * 100.0,
            unmet,
            domestic,
            imports,
            stockpile
        );
    }
}

fn print_goods_table(world: &World, ci: usize) {
    let market = &world.countries.market.markets[ci];
    let mut rows: Vec<_> = market.clearing_sheet.results.iter().collect();
    rows.sort_by(|a, b| {
        b.1.total_unmet
            .partial_cmp(&a.1.total_unmet)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                b.1.shortage_ratio
                    .partial_cmp(&a.1.shortage_ratio)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    println!("\n[ger-prod] final goods clearing");
    println!(
        "{:<20} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8}",
        "good", "prod", "import", "supply", "demand", "unmet", "short%", "stock", "cover"
    );
    for (good, result) in rows.into_iter().filter(|(_, r)| is_interesting_good(r)) {
        println!(
            "{:<20} {:>8.2} {:>8.2} {:>8.2} {:>8.2} {:>8.2} {:>7.1}% {:>8.2} {:>8.1}",
            good,
            result.domestic_production,
            result.imports,
            result.supply_available,
            total_requested(result),
            result.total_unmet,
            result.shortage_ratio * 100.0,
            result.stockpile_closing,
            result.stockpile_coverage_days,
        );
        print_bucket_line(result);
    }
}

fn is_interesting_good(result: &GoodClearingResult) -> bool {
    result.total_unmet > 0.01
        || result.domestic_production > 0.01
        || result.imports > 0.01
        || result.stockpile_closing > 0.01
        || total_requested(result) > 0.01
}

fn print_bucket_line(result: &GoodClearingResult) {
    let mut parts = Vec::new();
    for kind in [
        DemandBucketKind::PopBasicConsumption,
        DemandBucketKind::BuildingInput,
        DemandBucketKind::MilitaryInput,
        DemandBucketKind::GovernmentProcurement,
        DemandBucketKind::ConstructionInput,
        DemandBucketKind::PopNonBasicConsumption,
        DemandBucketKind::Export,
    ] {
        if let Some(bucket) = result.buckets.iter().find(|bucket| bucket.kind == kind) {
            if bucket.requested > 0.01 || bucket.unmet > 0.01 {
                parts.push(format!(
                    "{kind:?} {:.1}/{:.1}",
                    bucket.fulfilled, bucket.requested
                ));
            }
        }
    }
    if !parts.is_empty() {
        println!("  buckets: {}", parts.join(", "));
    }
}

fn total_requested(result: &GoodClearingResult) -> f32 {
    result.buckets.iter().map(|bucket| bucket.requested).sum()
}

#[derive(Default)]
struct BuildingSummary {
    levels: u32,
    count: u32,
    low_prod_levels: u32,
    production_weighted: f32,
    output_gbp: f64,
    profit_rm: f64,
    input_rm: f64,
    min_emp: f32,
    min_qual: f32,
    min_input: f32,
}

fn print_building_table(world: &World, db: &V6Database, ci: usize) {
    let country = CountryId(ci as u16);
    let mut summaries: BTreeMap<String, BuildingSummary> = BTreeMap::new();
    let mut low_rows = Vec::new();

    for (idx, building) in world.countries.buildings_v6.buildings.iter().enumerate() {
        let state_idx = building.state.0 as usize;
        if state_idx >= world.states.count || world.states.owners[state_idx] != country {
            continue;
        }
        if building.level == 0 {
            continue;
        }

        let pms = active_pms_for_building(building, db);
        let employment = employment_ratio(building, &pms);
        let qualification = qualification_ratio(world, idx, &pms);
        let input = input_fulfillment_from_sheet(world, ci, &pms);
        let key = building.building_def_id.clone();
        let entry = summaries
            .entry(key.clone())
            .or_insert_with(|| BuildingSummary {
                min_emp: 1.0,
                min_qual: 1.0,
                min_input: 1.0,
                ..BuildingSummary::default()
            });
        entry.levels += building.level as u32;
        entry.count += 1;
        entry.low_prod_levels += if building.production_rate < 0.85 {
            building.level as u32
        } else {
            0
        };
        entry.production_weighted += building.production_rate * building.level as f32;
        entry.output_gbp += building.output_value_gbp;
        entry.profit_rm += building.profit_rm;
        entry.input_rm += building.input_cost_rm;
        entry.min_emp = entry.min_emp.min(employment);
        entry.min_qual = entry.min_qual.min(qualification);
        entry.min_input = entry.min_input.min(input);

        if building.production_rate < 0.85
            || employment < 0.85
            || qualification < 0.85
            || input < 0.85
        {
            low_rows.push((
                key,
                building.level,
                building.production_rate,
                employment,
                qualification,
                input,
            ));
        }
    }

    println!("\n[ger-prod] building summary");
    println!(
        "{:<26} {:>6} {:>6} {:>8} {:>8} {:>8} {:>9} {:>10} {:>10} {:>9}",
        "building",
        "count",
        "lvl",
        "prod%",
        "minEmp%",
        "minQual%",
        "minIn%",
        "outGBP",
        "profitRM",
        "lowLvl"
    );
    for (building, summary) in &summaries {
        let avg_prod = if summary.levels > 0 {
            summary.production_weighted / summary.levels as f32
        } else {
            0.0
        };
        println!(
            "{:<26} {:>6} {:>6} {:>7.1}% {:>7.1}% {:>8.1}% {:>8.1}% {:>10.1} {:>10.0} {:>9}",
            building,
            summary.count,
            summary.levels,
            avg_prod * 100.0,
            summary.min_emp * 100.0,
            summary.min_qual * 100.0,
            summary.min_input * 100.0,
            summary.output_gbp,
            summary.profit_rm,
            summary.low_prod_levels
        );
    }

    low_rows.sort_by(|a, b| {
        a.2.partial_cmp(&b.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.4.partial_cmp(&b.4).unwrap_or(std::cmp::Ordering::Equal))
    });
    println!("\n[ger-prod] lowest-rate buildings");
    println!(
        "{:<26} {:>5} {:>8} {:>8} {:>9} {:>8}",
        "building", "level", "prod%", "emp%", "qual%", "input%"
    );
    for (building, level, prod, emp, qual, input) in low_rows.into_iter().take(25) {
        println!(
            "{:<26} {:>5} {:>7.1}% {:>7.1}% {:>8.1}% {:>7.1}%",
            building,
            level,
            prod * 100.0,
            emp * 100.0,
            qual * 100.0,
            input * 100.0
        );
    }
}

fn employment_ratio(building: &Building, pms: &[&ProductionMethodDef]) -> f32 {
    if building.level == 0 {
        return 0.0;
    }
    let mut needed = 0.0;
    let mut filled = 0.0;
    for class_idx in 0..PopClass::COUNT {
        let class_needed = pms
            .iter()
            .map(|pm| pm.employment_demand[class_idx] as f32)
            .sum::<f32>()
            * building.level as f32;
        let class_filled = building.employment[class_idx] as f32;
        needed += class_needed;
        filled += class_filled.min(class_needed);
    }
    if needed <= 0.0 {
        1.0
    } else {
        (filled / needed).clamp(0.0, 1.0)
    }
}

fn qualification_ratio(world: &World, building_idx: usize, pms: &[&ProductionMethodDef]) -> f32 {
    let required_literacy = pms
        .iter()
        .map(|pm| pm.required_literacy)
        .fold(0.0_f32, f32::max);
    let required_skilled = pms
        .iter()
        .map(|pm| pm.required_skilled_ratio)
        .fold(0.0_f32, f32::max);
    if required_literacy <= 0.0 && required_skilled <= 0.0 {
        return 1.0;
    }

    let building_id = hoi4_state::BuildingId(building_idx as u32);
    let mut total = 0.0;
    let mut literacy = 0.0;
    let mut skilled = 0.0;
    for pop in &world.countries.pops.groups {
        if pop.employed_at != Some(building_id) {
            continue;
        }
        let size = pop.size as f32;
        total += size;
        literacy += pop.literacy * size;
        skilled += pop.skilled_ratio * size;
    }
    if total <= 0.0 {
        return 0.35;
    }

    let lit_ratio = if required_literacy > 0.0 {
        (literacy / total) / required_literacy
    } else {
        1.0
    };
    let skilled_ratio = if required_skilled > 0.0 {
        (skilled / total) / required_skilled
    } else {
        1.0
    };
    lit_ratio.min(skilled_ratio).clamp(0.35, 1.0)
}

fn input_fulfillment_from_sheet(world: &World, ci: usize, pms: &[&ProductionMethodDef]) -> f32 {
    let mut ratio = 1.0_f32;
    let sheet = &world.countries.market.markets[ci].clearing_sheet;
    for pm in pms {
        for good_id in &pm.input_good_ids {
            let Some(result) = sheet.results.get(good_id) else {
                continue;
            };
            let mut requested = 0.0;
            let mut fulfilled = 0.0;
            for bucket in &result.buckets {
                if matches!(
                    bucket.kind,
                    DemandBucketKind::BuildingInput | DemandBucketKind::MilitaryInput
                ) {
                    requested += bucket.requested;
                    fulfilled += bucket.fulfilled;
                }
            }
            if requested > 0.0 {
                ratio = ratio.min((fulfilled / requested).clamp(0.0, 1.0));
            }
        }
    }
    ratio
}

fn print_equipment_stockpile(econ: &hoi4_logic::economy::EconomyState, ci: usize) {
    let mut rows: Vec<_> = econ.stockpile[ci].iter().collect();
    rows.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
    println!("\n[ger-prod] equipment stockpile");
    for (equipment, amount) in rows.into_iter().take(20) {
        println!("  {equipment:<28} {:>10.1}", amount);
    }
}

fn print_population_table(world: &World, ci: usize) {
    let country = CountryId(ci as u16);
    let mut class_total = [0_u64; PopClass::COUNT];
    let mut employed = [0_u64; PopClass::COUNT];
    let mut satisfaction = [0.0_f64; PopClass::COUNT];
    for pop in &world.countries.pops.groups {
        let state_idx = pop.state.0 as usize;
        if state_idx >= world.states.count || world.states.owners[state_idx] != country {
            continue;
        }
        let idx = pop.class.index();
        class_total[idx] += pop.size as u64;
        if pop.employed_at.is_some() {
            employed[idx] += pop.size as u64;
        }
        satisfaction[idx] += pop.satisfaction as f64 * pop.size as f64;
    }

    println!("\n[ger-prod] population/employment");
    println!(
        "{:<12} {:>12} {:>12} {:>8} {:>8}",
        "class", "pop", "employed", "emp%", "sat"
    );
    for class in [
        PopClass::Peasant,
        PopClass::Worker,
        PopClass::Clerk,
        PopClass::Capitalist,
        PopClass::Aristocrat,
        PopClass::Soldier,
    ] {
        let idx = class.index();
        let total = class_total[idx];
        let emp_ratio = if total > 0 {
            employed[idx] as f64 / total as f64
        } else {
            0.0
        };
        let avg_sat = if total > 0 {
            satisfaction[idx] / total as f64
        } else {
            0.0
        };
        println!(
            "{:<12?} {:>12} {:>12} {:>7.1}% {:>8.3}",
            class,
            total,
            employed[idx],
            emp_ratio * 100.0,
            avg_sat
        );
    }
}
