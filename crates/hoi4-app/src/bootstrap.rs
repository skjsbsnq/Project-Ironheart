use std::sync::Arc;
use std::time::Instant;

use hoi4_data::GameData;
use hoi4_map::GameMap;
use hoi4_paths::{PathConfig, ResolveOverrides};
use hoi4_state::World;

use hoi4_runtime::{init_simulation, HourlyRuntime, SystemSchedule};

pub struct Cli {
    pub game_path: Option<std::path::PathBuf>,
    pub mods: Vec<std::path::PathBuf>,
    pub headless: bool,
    pub headless_days: u32,
    pub map_phase0: bool,
    pub map_phase0_report_only: bool,
    pub map_phase0_output: std::path::PathBuf,
    pub map_audit: bool,
    pub map_audit_output: std::path::PathBuf,
    pub help_requested: bool,
}

pub fn parse_cli() -> Cli {
    let mut out = Cli {
        game_path: None,
        mods: Vec::new(),
        headless: false,
        headless_days: 1,
        map_phase0: false,
        map_phase0_report_only: false,
        map_phase0_output: std::path::PathBuf::from("target/map_baseline"),
        map_audit: false,
        map_audit_output: std::path::PathBuf::from("target/map_audit"),
        help_requested: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--game-path" => {
                if let Some(v) = args.next() {
                    out.game_path = Some(std::path::PathBuf::from(v));
                }
            }
            "--mod" => {
                if let Some(v) = args.next() {
                    out.mods.push(std::path::PathBuf::from(v));
                }
            }
            "--headless" => out.headless = true,
            "--headless-days" => {
                if let Some(v) = args.next() {
                    out.headless_days = v.parse().unwrap_or(1);
                }
            }
            "--map-phase0" | "--map-baseline-phase0" => out.map_phase0 = true,
            "--map-phase0-report-only" | "--map-baseline-report-only" => {
                out.map_phase0 = true;
                out.map_phase0_report_only = true;
            }
            "--map-phase0-output" | "--map-baseline-output" => {
                if let Some(v) = args.next() {
                    out.map_phase0_output = std::path::PathBuf::from(v);
                }
            }
            "--map-audit" => out.map_audit = true,
            "--map-audit-output" => {
                if let Some(v) = args.next() {
                    out.map_audit_output = std::path::PathBuf::from(v);
                }
            }
            "-h" | "--help" => out.help_requested = true,
            other => eprintln!("[hoi4-app] ignoring unknown argument: {other}"),
        }
    }
    out
}

pub fn print_usage() {
    println!(
        r#"hoi4-app - Project Ironheart V3

USAGE:
    hoi4-app [--game-path <PATH>] [--mod <PATH>]... [--headless [--headless-days N]]
    hoi4-app --map-audit [--map-audit-output <DIR>]
    hoi4-app --map-phase0 [--map-phase0-output <DIR>]
    hoi4-app --map-phase0-report-only [--map-phase0-output <DIR>]

OPTIONS:
    --game-path <PATH>      HOI4 install directory
    --mod <PATH>            mod root directory (repeatable, first listed = highest priority)
    --headless              no window; load world, tick N days, exit (CI smoke)
    --headless-days <N>     days to tick in headless mode (default 1)
    --map-audit             write map resource audit to target/map_audit/latest.json, then exit
    --map-audit-output DIR  output directory for map audit (default target/map_audit)
    --map-phase0            capture Map Renderer V2 Phase 0 screenshots, reports, audit, then exit
    --map-phase0-report-only write Phase 0 manifest/audit without launching the renderer
    --map-phase0-output DIR output directory for Phase 0 reports (default target/map_baseline)
    -h, --help              show this help"#
    );
}

pub fn resolve_path_config(cli: &Cli) -> PathConfig {
    match PathConfig::resolve(ResolveOverrides {
        cli_game_path: cli.game_path.clone(),
        cli_mods: cli.mods.clone(),
    }) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[hoi4-app] {e}");
            std::process::exit(2);
        }
    }
}

pub fn load_world(path_cfg: &PathConfig) -> World {
    println!(
        "[hoi4-app] HOI4 path = {} (source: {:?})",
        path_cfg.game_path().display(),
        path_cfg.source(),
    );

    println!("Loading game data...");
    let t0 = Instant::now();
    let map = Arc::new(GameMap::load_from_paths(path_cfg).expect("Failed to load map"));
    let (game_data, load_report) =
        GameData::load_from_paths(path_cfg).expect("Failed to load game data");
    let data = Arc::new(game_data);
    let mut world = World::new(map, data);
    let pop = world.populate_from_history();
    world.command =
        hoi4_state::CommandHierarchy::auto_group(world.divisions.count, &world.divisions.owners);
    println!(
        "Loaded in {:.2}s  ?{} provinces, {} countries, {} states, heightmap {}x{}",
        t0.elapsed().as_secs_f32(),
        world.provinces.count,
        world.countries.count,
        world.states.count,
        world.map.heightmap.width,
        world.map.heightmap.height,
    );
    println!(
        "Data loaded: {} countries, {} states, {} buildings, {} resources, {} equipment, {} techs, {} ideas, {} characters",
        load_report.loaded_counts.countries,
        load_report.loaded_counts.states,
        load_report.loaded_counts.buildings,
        load_report.loaded_counts.resources,
        load_report.loaded_counts.equipment,
        load_report.loaded_counts.technologies,
        load_report.loaded_counts.ideas,
        load_report.loaded_counts.characters,
    );
    for warning in &load_report.warnings {
        eprintln!(
            "[hoi4-app] data load warning: {}: {}",
            warning.path, warning.message
        );
    }
    println!(
        "OOB applied: {} divisions, {} fleets ({} ships), {} air wings; {} ideas, {} focuses completed",
        pop.divisions_spawned,
        pop.fleets_spawned,
        pop.ships_spawned,
        pop.air_wings_spawned,
        pop.ideas_applied,
        pop.focuses_completed,
    );
    if pop.warn_oob_missing
        + pop.warn_template_missing
        + pop.warn_ship_class_missing
        + pop.warn_aircraft_missing
        > 0
    {
        eprintln!(
            "[hoi4-app] OOB warnings: {} oob missing, {} template missing, {} ship class missing, {} aircraft missing",
            pop.warn_oob_missing,
            pop.warn_template_missing,
            pop.warn_ship_class_missing,
            pop.warn_aircraft_missing,
        );
    }
    print_controls();
    world
}

pub fn print_controls() {
    println!("\nControls:");
    println!("  arrows / screen edge - pan camera");
    println!("  Drag        - pan with mouse");
    println!("  Wheel       - zoom");
    println!("  SPACE       - pause/resume");
    println!("  1-5         - game speed");
    println!("  M           - cycle map mode");
    println!("  V           - cycle border debug view");
    println!("  F1          - toggle GUI debug overlay");
    println!("  F6          - cycle terrain debug view");
    println!("  F10         - cycle water debug view");
    println!("  F11         - toggle borderless fullscreen");
    println!("  ESC         - quit\n");
}

/// CI smoke / headless: tick N days, print metrics, exit.
/// P0.1：headless 与 GUI 使用同一套 ContentRuntimeState，走同一条日更路径。
pub fn run_headless(mut world: World, days: u32) {
    let default_player = world
        .countries
        .tags
        .iter()
        .position(|t| t == "GER")
        .unwrap_or(0);
    world.player = hoi4_state::CountryId(default_player as u16);

    let (mut econ, mut research, mut politics_cache, mut script, mut ai) =
        init_simulation(&mut world);
    let v6_db = hoi4_content::V6Database::load();
    let mut schedule = SystemSchedule::with_phase1_systems();
    let mut feedback_bus = hoi4_logic::feedback::FeedbackBus::new();

    // P0.1：headless 构造同一套 content state
    let scenario_content = crate::content_bootstrap::load_scenario_content("1936");
    let mut content = hoi4_runtime::ContentRuntimeState::new(
        &scenario_content,
        hoi4_state::CountryId(default_player as u16),
        world.date.days_since_epoch(),
    );

    let t0 = Instant::now();
    for _ in 0..days {
        for _ in 0..24 {
            let _ = hoi4_runtime::tick_one_hour(&mut HourlyRuntime {
                world: &mut world,
                econ: &mut econ,
                research: &mut research,
                politics_cache: &mut politics_cache,
                script: &mut script,
                ai: &mut ai,
                v6_db: &v6_db,
                schedule: &mut schedule,
                feedback_bus: &mut feedback_bus,
                content: &mut content,
            });
        }
    }
    let elapsed = t0.elapsed().as_secs_f64();
    println!(
        "[headless] {} days ticked in {:.3}s ({:.1} ms/day)  ?date now {}",
        days,
        elapsed,
        elapsed * 1000.0 / days.max(1) as f64,
        world.date,
    );
    println!("[headless] {}", schedule.report_systems());
    println!("[headless] timing {}", schedule.timing_summary_report());
}
