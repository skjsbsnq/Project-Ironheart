use hoi4_logic::economy::EconomyState;
use hoi4_logic::military::command_executor;
use hoi4_logic::military::movement::{can_enter_province, order_naval_invasion};
use hoi4_state::{
    CountryId, DivisionAssignment, DivisionIntent, DivisionRole, ProvinceId, StateId, World,
};

pub const JAPAN_CHINA_INCIDENT_FLAG: &str = "china_incident_escalated";
pub const CHINA_WAR_ENEMY_TAGS: &[&str] = &[
    "SND", "CHI", "SHX", "PRC", "GXC", "GDC", "YUN", "XAJ", "SIC", "XSM", "SIK",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChinaWarPhase {
    MarcoPolo,
    NorthChina,
    Shandong,
    ShanghaiNanjing,
    Wuhan,
    Stalemate,
}

#[derive(Debug, Clone)]
pub struct ChinaTheaterStrategy {
    pub active: bool,
    pub phase: ChinaWarPhase,
    pub priority_enemy_tags: Vec<&'static str>,
    pub force_attack_enemy_tags: Vec<&'static str>,
    pub preferred_state_ids: Vec<u16>,
    pub attack_bias: f32,
    pub min_front_ratio: f32,
}

impl ChinaTheaterStrategy {
    pub fn inactive() -> Self {
        Self {
            active: false,
            phase: ChinaWarPhase::MarcoPolo,
            priority_enemy_tags: Vec::new(),
            force_attack_enemy_tags: Vec::new(),
            preferred_state_ids: Vec::new(),
            attack_bias: 0.0,
            min_front_ratio: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvasionPlanPhase {
    Preparing,
    WaitingForSeaControl,
    Embarking,
    Landing,
    Exploiting,
    FailedRecovery,
}

#[derive(Debug, Clone)]
pub struct NavalInvasionPlan {
    pub id: u32,
    pub target_province: ProvinceId,
    pub target_state: Option<u16>,
    pub embark_port: Option<ProvinceId>,
    pub route_regions: Vec<u32>,
    pub phase: InvasionPlanPhase,
    pub assigned_divisions: Vec<usize>,
    pub assigned_fleets: Vec<u32>,
    pub required_divisions: usize,
    pub required_support: f32,
    pub actual_support: f32,
    pub created_at_hour: u64,
    pub phase_since_hour: u64,
    pub failure_count: u8,
}

impl NavalInvasionPlan {
    pub fn new(
        id: u32,
        target_province: ProvinceId,
        target_state: Option<u16>,
        embark_port: Option<ProvinceId>,
        route_regions: Vec<u32>,
        required_divisions: usize,
        now_hour: u64,
    ) -> Self {
        Self {
            id,
            target_province,
            target_state,
            embark_port,
            route_regions,
            phase: InvasionPlanPhase::Preparing,
            assigned_divisions: Vec::new(),
            assigned_fleets: Vec::new(),
            required_divisions,
            required_support: 0.5,
            actual_support: 0.0,
            created_at_hour: now_hour,
            phase_since_hour: now_hour,
            failure_count: 0,
        }
    }

    pub fn is_ready_to_execute(&self) -> bool {
        matches!(self.phase, InvasionPlanPhase::WaitingForSeaControl)
            && self.assigned_divisions.len() >= self.required_divisions.min(1)
            && self.actual_support >= self.required_support
    }

    pub fn is_active(&self) -> bool {
        !matches!(self.phase, InvasionPlanPhase::FailedRecovery)
    }

    pub fn transition(&mut self, new_phase: InvasionPlanPhase, now_hour: u64) {
        if self.phase != new_phase {
            if matches!(new_phase, InvasionPlanPhase::FailedRecovery) {
                self.failure_count = self.failure_count.saturating_add(1);
            }
            self.phase = new_phase;
            self.phase_since_hour = now_hour;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DivisionTheaterLocation {
    Homeland,
    KoreaManchuria,
    MainlandFront,
    InTransit,
    InvasionPreparing,
}

#[derive(Debug, Clone)]
pub struct ReinforcementRoute {
    pub destination_name: &'static str,
    pub preferred_states: Vec<u16>,
    pub min_phase: ChinaWarPhase,
    pub priority: u32,
}

const INVASION_PLAN_PREP_HOURS: u64 = 7 * 24;
const INVASION_PLAN_RECOVERY_HOURS: u64 = 14 * 24;
const INVASION_PLAN_MAX_FAILURES: u8 = 3;
const INVASIONS_PER_PLAN: usize = 3;
const PREWAR_FRONT_DEPLOYMENT_RATIO: f32 = 0.55;
const PREWAR_INVASION_RESERVE_RATIO: f32 = 0.25;

pub fn reinforcement_routes_for_phase(phase: ChinaWarPhase) -> Vec<ReinforcementRoute> {
    match phase {
        ChinaWarPhase::MarcoPolo | ChinaWarPhase::NorthChina => vec![
            ReinforcementRoute {
                destination_name: "tianjin",
                preferred_states: vec![608, 1039, 615, 621],
                min_phase: ChinaWarPhase::MarcoPolo,
                priority: 3,
            },
            ReinforcementRoute {
                destination_name: "qingdao",
                preferred_states: vec![597, 743, 1038],
                min_phase: ChinaWarPhase::NorthChina,
                priority: 2,
            },
        ],
        ChinaWarPhase::Shandong => vec![
            ReinforcementRoute {
                destination_name: "qingdao",
                preferred_states: vec![597, 743, 1038],
                min_phase: ChinaWarPhase::Shandong,
                priority: 3,
            },
            ReinforcementRoute {
                destination_name: "tianjin",
                preferred_states: vec![608, 1039],
                min_phase: ChinaWarPhase::MarcoPolo,
                priority: 2,
            },
        ],
        ChinaWarPhase::ShanghaiNanjing | ChinaWarPhase::Wuhan => vec![
            ReinforcementRoute {
                destination_name: "shanghai",
                preferred_states: vec![613, 598, 606],
                min_phase: ChinaWarPhase::ShanghaiNanjing,
                priority: 3,
            },
            ReinforcementRoute {
                destination_name: "qingdao",
                preferred_states: vec![597, 743],
                min_phase: ChinaWarPhase::Shandong,
                priority: 2,
            },
        ],
        ChinaWarPhase::Stalemate => vec![
            ReinforcementRoute {
                destination_name: "shanghai",
                preferred_states: vec![613, 598, 606, 597],
                min_phase: ChinaWarPhase::Stalemate,
                priority: 2,
            },
            ReinforcementRoute {
                destination_name: "qingdao",
                preferred_states: vec![597, 743],
                min_phase: ChinaWarPhase::Shandong,
                priority: 1,
            },
        ],
    }
}

pub fn resolve_reinforcement_ports(
    world: &World,
    country: CountryId,
    phase: ChinaWarPhase,
) -> Vec<ProvinceId> {
    let usable = hoi4_logic::military::movement::usable_ports(world, country);
    let mut routes = reinforcement_routes_for_phase(phase);
    routes.sort_by(|a, b| b.priority.cmp(&a.priority));

    let mut resolved = Vec::new();
    for route in &routes {
        if let Some(&port) = usable
            .iter()
            .find(|p| is_port_near_named_area(world, **p, route.destination_name))
        {
            if !resolved.contains(&port) {
                resolved.push(port);
            }
        }
    }

    if resolved.is_empty() {
        for &p in &usable {
            if let Some(def) = world.map.get_province(p.0) {
                if def.coastal && matches!(def.province_type, hoi4_map::ProvinceType::Land) {
                    resolved.push(p);
                }
            }
        }
    }
    resolved
}

fn is_port_near_named_area(world: &World, port: ProvinceId, name: &str) -> bool {
    let pi = port.0 as usize;
    if pi >= world.provinces.count {
        return false;
    }
    let state = world.provinces.state_of[pi];
    if state.is_none() {
        return false;
    }
    let si = state.0 as usize;
    if si < world.data.states.len() {
        let state_name = &world.data.states[si].name;
        if state_name.to_ascii_lowercase().contains(name) {
            return true;
        }
    }
    state_game_id(world, state).is_some_and(|id| match name {
        "tianjin" | "beiping" | "beijing" => matches!(id, 608 | 1039),
        "qingdao" | "shandong" => matches!(id, 597 | 743 | 1038),
        "shanghai" => matches!(id, 613),
        _ => false,
    })
}

pub fn invasion_targets_for_phase(
    world: &World,
    country: CountryId,
    phase: ChinaWarPhase,
) -> Vec<(ProvinceId, i32, Option<u16>)> {
    let mut targets = Vec::new();
    let phase_targets: Vec<(&str, i32)> = match phase {
        ChinaWarPhase::MarcoPolo | ChinaWarPhase::NorthChina => {
            vec![("tianjin", 80), ("qingdao", 60)]
        }
        ChinaWarPhase::Shandong => {
            vec![("qingdao", 90), ("shanghai", 40)]
        }
        ChinaWarPhase::ShanghaiNanjing => {
            vec![("shanghai", 100), ("qingdao", 30)]
        }
        ChinaWarPhase::Wuhan => {
            vec![("shanghai", 50)]
        }
        ChinaWarPhase::Stalemate => {
            vec![]
        }
    };

    for (area_name, base_score) in &phase_targets {
        for si in 0..world.states.count {
            let controller = world.states.controllers[si];
            if controller == country || !world.diplomacy.at_war_with(country, controller) {
                continue;
            }
            let state = StateId(si as u16);
            let game_id = state_game_id(world, state);
            let state_name = world
                .data
                .states
                .get(si)
                .map(|s| s.name.to_ascii_lowercase())
                .unwrap_or_default();

            let matches_area = match *area_name {
                "tianjin" | "beiping" | "beijing" => {
                    game_id.is_some_and(|id| matches!(id, 608 | 1039))
                        || state_name.contains("hebei")
                        || state_name.contains("beiping")
                        || state_name.contains("beijing")
                        || state_name.contains("tianjin")
                }
                "qingdao" | "shandong" => {
                    game_id.is_some_and(|id| matches!(id, 597 | 743 | 1038))
                        || state_name.contains("shandong")
                        || state_name.contains("jinan")
                        || state_name.contains("qingdao")
                }
                "shanghai" => {
                    game_id.is_some_and(|id| matches!(id, 613))
                        || state_name.contains("shanghai")
                        || state_name.contains("nanjing")
                }
                _ => false,
            };

            if !matches_area {
                continue;
            }

            for &p in &world.states.provinces[si] {
                let pi = p.0 as usize;
                if pi >= world.provinces.count || world.provinces.controllers[pi] == country {
                    continue;
                }
                if !world.map.get_province(p.0).is_some_and(|def| {
                    matches!(def.province_type, hoi4_map::ProvinceType::Land) && def.coastal
                }) {
                    continue;
                }
                if !world
                    .diplomacy
                    .at_war_with(country, world.provinces.controllers[pi])
                {
                    continue;
                }

                let vp_score = world
                    .data
                    .states
                    .get(si)
                    .map(|s| s.victory_points.iter().map(|(_, v)| *v as i32).sum())
                    .unwrap_or(0);
                let score = *base_score + vp_score * 3 + world.state_building_levels(state) as i32;
                targets.push((p, score, game_id));
            }
        }
    }

    targets.sort_by(|a, b| b.1.cmp(&a.1));
    targets
}

pub fn classify_division_theater(
    world: &World,
    _country: CountryId,
    div_idx: usize,
) -> DivisionTheaterLocation {
    if world.divisions.transport[div_idx].is_some() {
        return DivisionTheaterLocation::InTransit;
    }

    if let Some(asgn) = world
        .divisions
        .assignments
        .get(div_idx)
        .and_then(|a| a.as_ref())
    {
        if asgn.role == hoi4_state::DivisionRole::Reserve {
            let Some(target) = asgn.target else {
                return DivisionTheaterLocation::Homeland;
            };
            if is_province_in_china_war_zone(world, target) {
                return DivisionTheaterLocation::InvasionPreparing;
            }
        }
    }

    let loc = world.divisions.locations[div_idx];
    if is_province_in_china_war_zone(world, loc) {
        return DivisionTheaterLocation::MainlandFront;
    }

    if is_province_in_korea_manchuria(world, loc) {
        return DivisionTheaterLocation::KoreaManchuria;
    }

    DivisionTheaterLocation::Homeland
}

fn is_province_in_china_war_zone(world: &World, p: ProvinceId) -> bool {
    let pi = p.0 as usize;
    if pi >= world.provinces.count {
        return false;
    }
    let state = world.provinces.state_of[pi];
    if state.is_none() {
        return false;
    }
    is_chinese_core_state(world, state)
}

fn is_province_in_korea_manchuria(world: &World, p: ProvinceId) -> bool {
    let pi = p.0 as usize;
    if pi >= world.provinces.count {
        return false;
    }
    let state = world.provinces.state_of[pi];
    if state.is_none() {
        return false;
    }
    let si = state.0 as usize;
    let state_name = world
        .data
        .states
        .get(si)
        .map(|s| s.name.to_ascii_lowercase())
        .unwrap_or_default();
    let game_id = state_game_id(world, state).unwrap_or_default();
    state_name.contains("korea")
        || state_name.contains("chosen")
        || state_name.contains("manchukuo")
        || state_name.contains("manchuria")
        || matches!(game_id, 739 | 740 | 741 | 742)
}

pub fn count_divisions_by_theater(
    world: &World,
    country: CountryId,
) -> std::collections::HashMap<DivisionTheaterLocation, usize> {
    let mut counts = std::collections::HashMap::new();
    for di in 0..world.divisions.count {
        if world.divisions.owners[di] != country {
            continue;
        }
        let loc = classify_division_theater(world, country, di);
        *counts.entry(loc).or_insert(0) += 1;
    }
    counts
}

pub fn tick_invasion_plans(
    world: &mut World,
    econ: &EconomyState,
    country: CountryId,
    plans: &mut Vec<NavalInvasionPlan>,
) -> Vec<String> {
    let now = world.elapsed_hours;
    let mut diagnostics = Vec::new();

    assign_divisions_to_invasion_plans(world, country, plans);

    for plan in plans.iter_mut() {
        match plan.phase {
            InvasionPlanPhase::Preparing => {
                if plan.assigned_divisions.len() >= plan.required_divisions.min(1) {
                    let support = hoi4_logic::naval::missions::naval_invasion_support_for_route(
                        world,
                        world.data.as_ref(),
                        country,
                        &plan.route_regions,
                    );
                    plan.actual_support = support;
                    if support >= plan.required_support {
                        plan.transition(InvasionPlanPhase::WaitingForSeaControl, now);
                    } else if now - plan.phase_since_hour > INVASION_PLAN_PREP_HOURS * 2 {
                        diagnostics.push(format!(
                            "plan {} failed: sea support {:.2} < {:.2}",
                            plan.id, support, plan.required_support
                        ));
                        plan.transition(InvasionPlanPhase::FailedRecovery, now);
                    }
                } else if now - plan.phase_since_hour > INVASION_PLAN_PREP_HOURS * 3 {
                    diagnostics.push(format!(
                        "plan {} failed: assigned divisions {}/{}",
                        plan.id,
                        plan.assigned_divisions.len(),
                        plan.required_divisions
                    ));
                    plan.transition(InvasionPlanPhase::FailedRecovery, now);
                }
            }
            InvasionPlanPhase::WaitingForSeaControl => {
                let support = hoi4_logic::naval::missions::naval_invasion_support_for_route(
                    world,
                    world.data.as_ref(),
                    country,
                    &plan.route_regions,
                );
                plan.actual_support = support;
                if plan.is_ready_to_execute() {
                    let issued =
                        issue_plan_invasion_orders(world, econ, country, plan, &mut diagnostics);
                    if issued > 0 {
                        plan.transition(InvasionPlanPhase::Embarking, now);
                    }
                } else if now - plan.phase_since_hour > INVASION_PLAN_PREP_HOURS * 4 {
                    diagnostics.push(format!(
                        "plan {} failed: waiting support {:.2}/{:.2}, divisions {}/{}",
                        plan.id,
                        support,
                        plan.required_support,
                        plan.assigned_divisions.len(),
                        plan.required_divisions
                    ));
                    plan.transition(InvasionPlanPhase::FailedRecovery, now);
                }
            }
            InvasionPlanPhase::Embarking => {
                let all_embarked = plan.assigned_divisions.iter().all(|&di| {
                    di < world.divisions.count
                        && world.divisions.transport[di].is_some()
                        && matches!(
                            world.divisions.transport[di].as_ref().map(|t| t.phase),
                            Some(hoi4_state::ArmyTransportPhase::AtSea)
                                | Some(hoi4_state::ArmyTransportPhase::Disembarking)
                        )
                });
                if all_embarked {
                    plan.transition(InvasionPlanPhase::Landing, now);
                } else if now - plan.phase_since_hour > INVASION_PLAN_PREP_HOURS {
                    diagnostics.push(format!("plan {} failed: divisions did not embark", plan.id));
                    plan.transition(InvasionPlanPhase::FailedRecovery, now);
                }
            }
            InvasionPlanPhase::Landing => {
                let any_landed = plan.assigned_divisions.iter().any(|&di| {
                    di < world.divisions.count
                        && world.divisions.transport[di].is_none()
                        && world.divisions.locations[di] == plan.target_province
                });
                if any_landed {
                    plan.transition(InvasionPlanPhase::Exploiting, now);
                } else if now - plan.phase_since_hour > INVASION_PLAN_PREP_HOURS {
                    diagnostics.push(format!("plan {} failed: no division landed", plan.id));
                    plan.transition(InvasionPlanPhase::FailedRecovery, now);
                }
            }
            InvasionPlanPhase::Exploiting => {
                let any_landed = plan.assigned_divisions.iter().any(|&di| {
                    di < world.divisions.count
                        && world.divisions.transport[di].is_none()
                        && world.divisions.locations[di] == plan.target_province
                });
                if !any_landed && now - plan.phase_since_hour > INVASION_PLAN_PREP_HOURS * 2 {
                    plan.transition(InvasionPlanPhase::FailedRecovery, now);
                }
            }
            InvasionPlanPhase::FailedRecovery => {
                if plan.failure_count < INVASION_PLAN_MAX_FAILURES
                    && now - plan.phase_since_hour > INVASION_PLAN_RECOVERY_HOURS
                {
                    plan.assigned_divisions.clear();
                    plan.assigned_fleets.clear();
                    plan.transition(InvasionPlanPhase::Preparing, now);
                }
            }
        }
    }

    plans.retain(|p| p.is_active() || p.failure_count < INVASION_PLAN_MAX_FAILURES);
    diagnostics
}

pub fn tick_prewar_china_deployment(world: &mut World, country: CountryId) -> Option<String> {
    if !matches!(world.country_tag(country), Some("JAP")) || world.diplomacy.is_at_war(country) {
        return None;
    }
    if world.date.year > 1937 || (world.date.year == 1937 && world.date.month > 7) {
        return None;
    }

    grant_japan_continental_access(world, country);

    let total = count_country_divisions(world, country);
    if total == 0 {
        return None;
    }
    let target_front = ((total as f32) * PREWAR_FRONT_DEPLOYMENT_RATIO).ceil() as usize;
    let target_front = target_front.max(total / 2 + 1).min(total);
    let reserve_target = ((total as f32) * PREWAR_INVASION_RESERVE_RATIO).ceil() as usize;

    let theater_counts = count_divisions_by_theater(world, country);
    let front_now = theater_counts
        .get(&DivisionTheaterLocation::KoreaManchuria)
        .copied()
        .unwrap_or(0)
        + theater_counts
            .get(&DivisionTheaterLocation::MainlandFront)
            .copied()
            .unwrap_or(0)
        + theater_counts
            .get(&DivisionTheaterLocation::InTransit)
            .copied()
            .unwrap_or(0);

    let staging = prewar_border_staging_provinces(world, country);
    let reserve_ports = homeland_invasion_reserve_ports(world, country);
    let mut issued_front = 0usize;
    let mut issued_reserve = 0usize;

    if front_now < target_front && !staging.is_empty() {
        let need = target_front - front_now;
        for di in candidate_home_divisions(world, country) {
            if issued_front >= need {
                break;
            }
            let target = nearest_candidate(world, world.divisions.locations[di], &staging);
            if issue_reserve_move(world, country, di, target) {
                issued_front += 1;
            }
        }
    }

    if !reserve_ports.is_empty() {
        for di in candidate_home_divisions(world, country) {
            if issued_reserve >= reserve_target {
                break;
            }
            if world.divisions.commands[di].is_some() || world.divisions.destinations[di].is_some()
            {
                continue;
            }
            let target = nearest_candidate(world, world.divisions.locations[di], &reserve_ports);
            if issue_reserve_move(world, country, di, target) {
                issued_reserve += 1;
            }
        }
    }

    if issued_front == 0 && issued_reserve == 0 {
        None
    } else {
        Some(format!(
            "china prewar deploy front+{} reserve+{} target_front={}/{}",
            issued_front, issued_reserve, target_front, total
        ))
    }
}

pub fn generate_invasion_plans(
    world: &World,
    country: CountryId,
    existing_plans: &[NavalInvasionPlan],
    next_id: &mut u32,
) -> Vec<NavalInvasionPlan> {
    let strategy = strategy_for(world, country);
    if !strategy.active {
        return Vec::new();
    }

    let mut new_plans = Vec::new();
    let targets = invasion_targets_for_phase(world, country, strategy.phase);
    let staging_ports = invasion_staging_ports(world, country);
    if staging_ports.is_empty() {
        return Vec::new();
    }

    let existing_targets: std::collections::HashSet<u16> =
        existing_plans.iter().map(|p| p.target_province.0).collect();

    let now = world.elapsed_hours;

    for (target, _score, game_id) in targets {
        if existing_targets.contains(&target.0) {
            continue;
        }

        let Some(embark_port) = staging_ports.iter().copied().min_by_key(|p| {
            hoi4_logic::military::movement::land_distance_approx(world, *p, target)
        }) else {
            continue;
        };

        let route_regions =
            hoi4_logic::military::movement::port_route_regions_pub(world, embark_port, target);

        let plan = NavalInvasionPlan::new(
            *next_id,
            target,
            game_id,
            Some(embark_port),
            route_regions,
            INVASIONS_PER_PLAN,
            now,
        );
        *next_id += 1;
        new_plans.push(plan);

        if new_plans.len() >= 2 {
            break;
        }
    }

    new_plans
}

fn assign_divisions_to_invasion_plans(
    world: &mut World,
    country: CountryId,
    plans: &mut [NavalInvasionPlan],
) {
    let mut used: std::collections::HashSet<usize> = plans
        .iter()
        .flat_map(|p| p.assigned_divisions.iter().copied())
        .collect();
    let candidates = candidate_invasion_divisions(world, country);

    for plan in plans.iter_mut() {
        if !matches!(
            plan.phase,
            InvasionPlanPhase::Preparing | InvasionPlanPhase::WaitingForSeaControl
        ) {
            continue;
        }

        plan.assigned_divisions.retain(|&di| {
            di < world.divisions.count
                && world.divisions.owners[di] == country
                && !world.divisions.in_combat[di]
                && !is_broken_division(world, di)
        });

        while plan.assigned_divisions.len() < plan.required_divisions {
            let Some(di) = candidates
                .iter()
                .copied()
                .filter(|di| !used.contains(di))
                .min_by_key(|&di| {
                    plan.embark_port
                        .map(|port| {
                            hoi4_logic::military::movement::land_distance_approx(
                                world,
                                world.divisions.locations[di],
                                port,
                            )
                        })
                        .unwrap_or(0)
                })
            else {
                break;
            };
            used.insert(di);
            plan.assigned_divisions.push(di);

            if let Some(port) = plan.embark_port {
                let _ = issue_reserve_move(world, country, di, port);
            }
        }
    }
}

fn issue_plan_invasion_orders(
    world: &mut World,
    econ: &EconomyState,
    country: CountryId,
    plan: &mut NavalInvasionPlan,
    diagnostics: &mut Vec<String>,
) -> usize {
    let mut issued = 0usize;
    for &di in &plan.assigned_divisions {
        if di >= world.divisions.count || world.divisions.owners[di] != country {
            continue;
        }
        if world.divisions.transport[di].is_some() {
            issued += 1;
            continue;
        }

        match order_naval_invasion(
            world,
            econ,
            di,
            plan.target_province,
            INVASION_PLAN_PREP_HOURS,
        ) {
            Ok(()) => {
                let asgn = DivisionAssignment {
                    front: Some(world.provinces.controllers[plan.target_province.0 as usize]),
                    role: DivisionRole::Assault,
                    target: Some(plan.target_province),
                    assigned_at: world.elapsed_hours,
                };
                command_executor::issue_invasion_command(
                    world,
                    di,
                    DivisionIntent::NavalInvasion,
                    plan.target_province,
                    asgn,
                );
                issued += 1;
            }
            Err(err) => diagnostics.push(format!(
                "plan {} division {} invasion order failed: {:?}",
                plan.id, di, err
            )),
        }
    }
    issued
}

fn grant_japan_continental_access(world: &mut World, japan: CountryId) {
    for tag in ["MAN", "MEN", "HBC"] {
        if let Some(host) = world.country(tag) {
            world.diplomacy.grant_military_access(host, japan);
        }
    }
}

fn count_country_divisions(world: &World, country: CountryId) -> usize {
    world
        .divisions
        .owners
        .iter()
        .take(world.divisions.count)
        .filter(|&&owner| owner == country)
        .count()
}

fn candidate_home_divisions(world: &World, country: CountryId) -> Vec<usize> {
    (0..world.divisions.count)
        .filter(|&di| {
            world.divisions.owners[di] == country
                && world.divisions.transport[di].is_none()
                && !world.divisions.in_combat[di]
                && !is_broken_division(world, di)
                && classify_division_theater(world, country, di)
                    == DivisionTheaterLocation::Homeland
        })
        .collect()
}

fn candidate_invasion_divisions(world: &World, country: CountryId) -> Vec<usize> {
    (0..world.divisions.count)
        .filter(|&di| {
            if world.divisions.owners[di] != country
                || world.divisions.transport[di].is_some()
                || world.divisions.in_combat[di]
                || is_broken_division(world, di)
            {
                return false;
            }
            matches!(
                classify_division_theater(world, country, di),
                DivisionTheaterLocation::Homeland | DivisionTheaterLocation::InvasionPreparing
            )
        })
        .collect()
}

fn prewar_border_staging_provinces(world: &World, country: CountryId) -> Vec<ProvinceId> {
    let mut out = Vec::new();
    for si in 0..world.states.count {
        let state = StateId(si as u16);
        let game_id = state_game_id(world, state).unwrap_or_default();
        let name = world
            .data
            .states
            .get(si)
            .map(|s| s.name.to_ascii_lowercase())
            .unwrap_or_default();
        let wanted = matches!(game_id, 739 | 740 | 741 | 742)
            || name.contains("korea")
            || name.contains("chosen")
            || name.contains("manchuria")
            || name.contains("manchukuo");
        if !wanted {
            continue;
        }
        for &p in &world.states.provinces[si] {
            if can_enter_province(world, country, p) && is_land(world, p) {
                out.push(p);
                break;
            }
        }
    }
    out.sort_by_key(|p| p.0);
    out.dedup();
    out
}

fn homeland_invasion_reserve_ports(world: &World, country: CountryId) -> Vec<ProvinceId> {
    invasion_staging_ports(world, country)
}

fn nearest_candidate(world: &World, from: ProvinceId, candidates: &[ProvinceId]) -> ProvinceId {
    candidates
        .iter()
        .copied()
        .min_by_key(|&p| hoi4_logic::military::movement::land_distance_approx(world, from, p))
        .unwrap_or(from)
}

fn issue_reserve_move(
    world: &mut World,
    country: CountryId,
    div_idx: usize,
    target: ProvinceId,
) -> bool {
    if div_idx >= world.divisions.count || !can_enter_province(world, country, target) {
        return false;
    }
    command_executor::issue_ai_ground_command(
        world,
        div_idx,
        DivisionIntent::Reserve,
        target,
        DivisionAssignment {
            front: None,
            role: DivisionRole::Reserve,
            target: Some(target),
            assigned_at: world.elapsed_hours,
        },
    )
}

fn is_broken_division(world: &World, di: usize) -> bool {
    let max_org = world.divisions.max_organisation[di].max(1e-6);
    world.divisions.organisation[di] / max_org < 0.05 || world.divisions.strength[di] < 0.05
}

fn is_land(world: &World, province: ProvinceId) -> bool {
    world
        .map
        .get_province(province.0)
        .is_some_and(|def| matches!(def.province_type, hoi4_map::ProvinceType::Land))
}

pub fn invasion_staging_ports(world: &World, country: CountryId) -> Vec<ProvinceId> {
    let usable = hoi4_logic::military::movement::usable_ports(world, country);
    let mut preferred: Vec<ProvinceId> = usable
        .iter()
        .copied()
        .filter(|p| is_port_near_homeland(world, *p))
        .collect();
    if preferred.is_empty() {
        preferred = usable;
    }
    preferred.sort_by_key(|p| p.0);
    preferred.dedup();
    preferred
}

fn is_port_near_homeland(world: &World, port: ProvinceId) -> bool {
    let pi = port.0 as usize;
    if pi >= world.provinces.count {
        return false;
    }
    let state = world.provinces.state_of[pi];
    if state.is_none() {
        return false;
    }
    let si = state.0 as usize;
    let state_name = world
        .data
        .states
        .get(si)
        .map(|s| s.name.to_ascii_lowercase())
        .unwrap_or_default();
    is_japanese_home_or_korea_state(world, state, &state_name)
}

fn is_japanese_home_or_korea_state(world: &World, state: StateId, state_name: &str) -> bool {
    let game_id = state_game_id(world, state).unwrap_or_default();
    state_name.contains("japan")
        || state_name.contains("kanto")
        || state_name.contains("tokyo")
        || state_name.contains("kansai")
        || state_name.contains("osaka")
        || state_name.contains("kyushu")
        || state_name.contains("honshu")
        || state_name.contains("hokkaido")
        || state_name.contains("shikoku")
        || state_name.contains("chugoku")
        || state_name.contains("tohoku")
        || state_name.contains("korea")
        || state_name.contains("chosen")
        || matches!(game_id, 300..=302 | 525 | 527 | 739..=742)
}

pub fn strategy_for(world: &World, country: CountryId) -> ChinaTheaterStrategy {
    if !is_japan_china_war_active(world, country) {
        return ChinaTheaterStrategy::inactive();
    }

    let occupied = japan_controlled_chinese_core_states(world, country);
    let shandong_ready = world.date.year > 1937
        || (world.date.year == 1937 && world.date.month >= 8)
        || japan_controls_any_state_ids(world, country, &[608, 1039, 615, 621, 597, 743, 1038]);
    let phase = if world.date.year >= 1939 || occupied >= 18 {
        ChinaWarPhase::Stalemate
    } else if country_has_flag(world, country, "jap_advance_on_nanjing")
        || country_has_flag(world, country, "jap_priority_shanghai_nanjing")
    {
        ChinaWarPhase::ShanghaiNanjing
    } else if country_has_flag(world, country, "jap_priority_shandong") && shandong_ready {
        ChinaWarPhase::Shandong
    } else if country_has_flag(world, country, "jap_priority_north_china") {
        ChinaWarPhase::NorthChina
    } else {
        ChinaWarPhase::MarcoPolo
    };

    match phase {
        ChinaWarPhase::MarcoPolo | ChinaWarPhase::NorthChina => ChinaTheaterStrategy {
            active: true,
            phase,
            priority_enemy_tags: vec!["SND", "CHI", "SHX"],
            force_attack_enemy_tags: vec!["SND", "CHI", "SHX"],
            preferred_state_ids: vec![608, 1039, 615, 621, 746, 597, 743, 1038],
            attack_bias: 0.16,
            min_front_ratio: 0.84,
        },
        ChinaWarPhase::Shandong => ChinaTheaterStrategy {
            active: true,
            phase,
            priority_enemy_tags: vec!["SND", "CHI", "SHX"],
            force_attack_enemy_tags: vec!["SND", "CHI", "SHX"],
            preferred_state_ids: vec![597, 743, 1038, 608, 1039, 615, 621, 746],
            attack_bias: 0.18,
            min_front_ratio: 0.82,
        },
        ChinaWarPhase::ShanghaiNanjing => ChinaTheaterStrategy {
            active: true,
            phase,
            priority_enemy_tags: vec!["CHI", "SND", "SHX", "GDC", "GXC"],
            force_attack_enemy_tags: vec!["CHI", "SND", "SHX"],
            preferred_state_ids: vec![613, 598, 606, 597, 743, 1038],
            attack_bias: 0.14,
            min_front_ratio: 0.88,
        },
        ChinaWarPhase::Wuhan => ChinaTheaterStrategy {
            active: true,
            phase,
            priority_enemy_tags: vec!["CHI", "SND", "SHX", "GDC", "GXC"],
            force_attack_enemy_tags: vec!["CHI", "SND"],
            preferred_state_ids: vec![598, 606, 613],
            attack_bias: 0.08,
            min_front_ratio: 0.94,
        },
        ChinaWarPhase::Stalemate => ChinaTheaterStrategy {
            active: true,
            phase,
            priority_enemy_tags: vec!["CHI", "SND", "SHX", "PRC"],
            force_attack_enemy_tags: vec!["CHI", "SND"],
            preferred_state_ids: vec![598, 606, 613, 597, 743],
            attack_bias: -0.08,
            min_front_ratio: 1.05,
        },
    }
}

pub fn country_has_flag(world: &World, country: CountryId, flag: &str) -> bool {
    let i = country.0 as usize;
    i < world.countries.ideas.len()
        && world.countries.ideas[i]
            .iter()
            .any(|idea| idea == &format!("FLAG:{flag}"))
}

fn japan_controls_any_state_ids(world: &World, country: CountryId, game_ids: &[u16]) -> bool {
    (0..world.states.count).any(|si| {
        world.states.controllers[si] == country
            && world
                .data
                .states
                .get(si)
                .is_some_and(|s| game_ids.contains(&s.id))
    })
}

pub fn is_japan_china_war_active(world: &World, country: CountryId) -> bool {
    matches!(world.country_tag(country), Some("JAP"))
        && country_has_flag(world, country, JAPAN_CHINA_INCIDENT_FLAG)
        && CHINA_WAR_ENEMY_TAGS.iter().any(|tag| {
            world
                .country(tag)
                .is_some_and(|enemy| world.diplomacy.at_war_with(country, enemy))
        })
}

pub fn is_china_war_enemy(world: &World, enemy: CountryId) -> bool {
    world
        .country_tag(enemy)
        .is_some_and(|tag| CHINA_WAR_ENEMY_TAGS.contains(&tag))
}

pub fn is_chinese_core_state(world: &World, state: StateId) -> bool {
    let si = state.0 as usize;
    if si >= world.states.cores.len() {
        return false;
    }
    world.states.cores[si].iter().any(|&core| {
        world
            .country_tag(core)
            .is_some_and(|tag| CHINA_WAR_ENEMY_TAGS.contains(&tag))
    })
}

pub fn japan_controlled_chinese_core_states(world: &World, country: CountryId) -> usize {
    if !matches!(world.country_tag(country), Some("JAP")) {
        return 0;
    }
    (0..world.states.count)
        .filter(|&si| {
            world.states.controllers[si] == country
                && world.states.owners[si] != country
                && is_chinese_core_state(world, StateId(si as u16))
        })
        .count()
}

pub fn state_game_id(world: &World, state: StateId) -> Option<u16> {
    world.data.states.get(state.0 as usize).map(|s| s.id)
}

pub fn state_name_contains(world: &World, state: StateId, needle: &str) -> bool {
    world
        .data
        .states
        .get(state.0 as usize)
        .is_some_and(|s| s.name.to_ascii_lowercase().contains(needle))
}

pub fn state_priority_bonus(world: &World, country: CountryId, state: StateId) -> f32 {
    let strategy = strategy_for(world, country);
    if !strategy.active {
        return 0.0;
    }

    let owner_tag = world.country_tag(world.state_owner(state));
    let game_id = state_game_id(world, state).unwrap_or_default();
    let preferred = strategy.preferred_state_ids.contains(&game_id);

    let mut bonus = if matches!(owner_tag, Some("SND"))
        || matches!(game_id, 597 | 743 | 1038)
        || state_name_contains(world, state, "shandong")
        || state_name_contains(world, state, "jinan")
        || state_name_contains(world, state, "qingdao")
    {
        if matches!(strategy.phase, ChinaWarPhase::Shandong) || preferred {
            70.0
        } else {
            55.0
        }
    } else if matches!(owner_tag, Some("SHX"))
        || matches!(game_id, 615 | 621 | 746)
        || state_name_contains(world, state, "shanxi")
        || state_name_contains(world, state, "taiyuan")
    {
        if matches!(strategy.phase, ChinaWarPhase::NorthChina) || preferred {
            55.0
        } else {
            42.0
        }
    } else if matches!(game_id, 608 | 1039)
        || state_name_contains(world, state, "hebei")
        || state_name_contains(world, state, "beiping")
        || state_name_contains(world, state, "beijing")
        || state_name_contains(world, state, "tianjin")
    {
        if matches!(strategy.phase, ChinaWarPhase::NorthChina) || preferred {
            52.0
        } else {
            40.0
        }
    } else if matches!(game_id, 613 | 598 | 606)
        || state_name_contains(world, state, "shanghai")
        || state_name_contains(world, state, "nanjing")
    {
        if matches!(strategy.phase, ChinaWarPhase::ShanghaiNanjing) || preferred {
            60.0
        } else {
            42.0
        }
    } else if state_name_contains(world, state, "xuzhou")
        || state_name_contains(world, state, "wuhan")
    {
        28.0
    } else if preferred {
        24.0
    } else {
        0.0
    };

    if country_has_flag(world, country, "jap_continental_offensive_momentum") {
        bonus += 8.0;
    }
    if country_has_flag(world, country, "jap_north_china_expeditionary_expansion")
        && (matches!(owner_tag, Some("SND") | Some("SHX")) || bonus >= 40.0)
    {
        bonus += 6.0;
    }
    if world.date.year >= 1939 || japan_controlled_chinese_core_states(world, country) >= 10 {
        bonus *= 0.85;
    }
    if japan_controlled_chinese_core_states(world, country) >= 18 {
        bonus *= 0.75;
    }
    bonus
}

pub fn attack_threshold_multiplier(world: &World, country: CountryId, enemy: CountryId) -> f32 {
    let strategy = strategy_for(world, country);
    if !strategy.active || !is_china_war_enemy(world, enemy) {
        return 1.0;
    }

    let enemy_tag = world.country_tag(enemy);
    let mut multiplier = strategy.min_front_ratio;
    if enemy_tag.is_some_and(|tag| strategy.force_attack_enemy_tags.contains(&tag)) {
        multiplier *= 0.94;
    }
    if country_has_flag(world, country, "jap_continental_offensive_momentum") {
        multiplier *= 0.92;
    }
    if world.date.year >= 1939 || japan_controlled_chinese_core_states(world, country) >= 10 {
        multiplier *= 1.12;
    }
    if japan_controlled_chinese_core_states(world, country) >= 18 {
        multiplier *= 1.18;
    }
    multiplier.clamp(0.65, 1.35)
}

pub fn segment_weight(
    world: &World,
    country: CountryId,
    enemy: CountryId,
    enemy_states: &[StateId],
) -> f32 {
    let strategy = strategy_for(world, country);
    if !strategy.active || !is_china_war_enemy(world, enemy) {
        return 1.0;
    }

    let enemy_tag = world.country_tag(enemy);
    let mut weight = 1.0 + strategy.attack_bias.max(0.0);
    if enemy_tag.is_some_and(|tag| strategy.priority_enemy_tags.contains(&tag)) {
        weight += 0.16;
    }
    if enemy_states.iter().any(|&state| {
        state_game_id(world, state).is_some_and(|id| strategy.preferred_state_ids.contains(&id))
    }) {
        weight += 0.18;
    }
    if matches!(strategy.phase, ChinaWarPhase::Stalemate) {
        weight *= 0.90;
    }
    weight.clamp(0.75, 1.55)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invasion_plan_transitions_on_support() {
        let mut plan = NavalInvasionPlan::new(
            0,
            ProvinceId(100),
            Some(613),
            Some(ProvinceId(50)),
            vec![1, 2],
            3,
            0,
        );
        assert_eq!(plan.phase, InvasionPlanPhase::Preparing);

        plan.assigned_divisions.push(0);
        plan.actual_support = 0.6;
        plan.required_support = 0.5;
        plan.transition(InvasionPlanPhase::WaitingForSeaControl, 100);
        assert_eq!(plan.phase, InvasionPlanPhase::WaitingForSeaControl);

        plan.assigned_divisions.push(1);
        plan.assigned_divisions.push(2);
        assert!(plan.is_ready_to_execute());
    }

    #[test]
    fn invasion_plan_failure_recovery_cycle() {
        let mut plan = NavalInvasionPlan::new(
            0,
            ProvinceId(100),
            Some(613),
            Some(ProvinceId(50)),
            vec![1, 2],
            3,
            0,
        );
        plan.transition(InvasionPlanPhase::FailedRecovery, 0);
        assert_eq!(plan.failure_count, 1);
        assert!(!plan.is_active());
    }

    #[test]
    fn division_theater_location_enum_coverage() {
        let homeland = DivisionTheaterLocation::Homeland;
        let korea = DivisionTheaterLocation::KoreaManchuria;
        let front = DivisionTheaterLocation::MainlandFront;
        let transit = DivisionTheaterLocation::InTransit;
        let prep = DivisionTheaterLocation::InvasionPreparing;
        assert_ne!(homeland, korea);
        assert_ne!(front, transit);
        assert_ne!(prep, homeland);
    }

    #[test]
    fn reinforcement_routes_match_phase() {
        let marco = reinforcement_routes_for_phase(ChinaWarPhase::MarcoPolo);
        assert!(!marco.is_empty());
        assert_eq!(marco[0].destination_name, "tianjin");

        let sh = reinforcement_routes_for_phase(ChinaWarPhase::ShanghaiNanjing);
        assert!(!sh.is_empty());
        assert_eq!(sh[0].destination_name, "shanghai");

        let stale = reinforcement_routes_for_phase(ChinaWarPhase::Stalemate);
        assert!(!stale.is_empty());
    }
}
