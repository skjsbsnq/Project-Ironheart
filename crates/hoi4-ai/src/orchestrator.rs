use hoi4_content::V6Database;
use hoi4_logic::economy::EconomyState;
use hoi4_logic::research::ResearchState;
use hoi4_state::{CountryId, World};
use rayon::prelude::*;

use crate::air;
use crate::constants::*;
use crate::diagnostics::{build_country_summary, AiLogRingbuf};
use crate::diplomacy;
use crate::focus;
use crate::frontline;
use crate::ground;
use crate::ground_orders;
use crate::intent::{self, NationalIntent};
use crate::naval;
use crate::production;
use crate::profile::AiProfile;
use crate::recruit;
use crate::research;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationState {
    Preparing,
    Attacking,
    Exploiting,
    Recovering,
    Paused,
}

#[derive(Debug, Clone, Copy)]
pub struct OperationPlanState {
    pub state: OperationState,
    pub since_day: i64,
    pub last_ratio: f32,
    pub last_progress: i32,
    pub failure_count: u8,
}

struct CountryAiState {
    last_focus_eval: i64,
    last_research_eval: i64,
    last_production_eval: i64,
    last_diplomacy_eval: i64,
    last_tactical_eval: i64,
    last_recruit_eval: i64,
    last_intent_eval: i64,
    profile: AiProfile,
    intent: NationalIntent,
    last_postures: std::collections::HashMap<u16, (crate::ground::GroundPosture, i64)>,
    operations: std::collections::HashMap<u16, OperationPlanState>,
    invasion_plans: Vec<crate::china_theater::NavalInvasionPlan>,
    next_invasion_plan_id: u32,
}

pub struct StrategicAi {
    states: Vec<CountryAiState>,
    pub log: AiLogRingbuf,
}

pub struct AiCadence {
    pub focus_days: u32,
    pub research_days: u32,
    pub production_days: u32,
    pub diplomacy_days: u32,
    pub tactical_days: u32,
    pub intent_days: u32,
}

impl Default for AiCadence {
    fn default() -> Self {
        Self {
            focus_days: FOCUS_EVAL_CADENCE,
            research_days: RESEARCH_EVAL_CADENCE,
            production_days: PRODUCTION_EVAL_CADENCE,
            diplomacy_days: DIPLOMACY_EVAL_CADENCE,
            tactical_days: ground_orders::TACTICAL_EVAL_CADENCE_DAYS,
            intent_days: 7,
        }
    }
}

#[derive(Debug, Default)]
pub struct StrategicTickReport {
    pub focus_decisions: Vec<(CountryId, String)>,
    pub research_decisions: Vec<(CountryId, String)>,
    pub production_decisions: Vec<(CountryId, String)>,
    pub diplomacy_decisions: Vec<(CountryId, String)>,
    pub tactical_decisions: Vec<(CountryId, String)>,
}

impl StrategicAi {
    pub fn new(world: &World) -> Self {
        let n = world.countries.count;
        let mut states = Vec::with_capacity(n);
        for i in 0..n {
            let tag = &world.countries.tags[i];
            let profile = AiProfile::for_country(tag);
            let intent = NationalIntent::default_at_peace(&profile);
            states.push(CountryAiState {
                last_focus_eval: 0,
                last_research_eval: 0,
                last_production_eval: 0,
                last_diplomacy_eval: 0,
                last_tactical_eval: 0,
                last_recruit_eval: 0,
                last_intent_eval: 0,
                profile,
                intent,
                last_postures: std::collections::HashMap::new(),
                operations: std::collections::HashMap::new(),
                invasion_plans: Vec::new(),
                next_invasion_plan_id: 0,
            });
        }
        Self {
            states,
            log: AiLogRingbuf::new(),
        }
    }

    pub fn ensure_capacity(&mut self, world: &World) {
        while self.states.len() < world.countries.count {
            let i = self.states.len();
            let tag = world
                .countries
                .tags
                .get(i)
                .map(|s| s.as_str())
                .unwrap_or("");
            let profile = AiProfile::for_country(tag);
            let intent = NationalIntent::default_at_peace(&profile);
            self.states.push(CountryAiState {
                last_focus_eval: 0,
                last_research_eval: 0,
                last_production_eval: 0,
                last_diplomacy_eval: 0,
                last_tactical_eval: 0,
                last_recruit_eval: 0,
                last_intent_eval: 0,
                profile,
                intent,
                last_postures: std::collections::HashMap::new(),
                operations: std::collections::HashMap::new(),
                invasion_plans: Vec::new(),
                next_invasion_plan_id: 0,
            });
        }
    }

    pub fn tick(
        &mut self,
        world: &mut World,
        econ: &mut EconomyState,
        research_state: &mut ResearchState,
        v6_db: &V6Database,
    ) -> StrategicTickReport {
        self.tick_filtered(world, econ, research_state, v6_db, |_| true, true)
    }

    pub fn tick_bucket(
        &mut self,
        world: &mut World,
        econ: &mut EconomyState,
        research_state: &mut ResearchState,
        v6_db: &V6Database,
        bucket: usize,
        buckets: usize,
    ) -> StrategicTickReport {
        let buckets = buckets.max(1);
        let bucket = bucket.min(buckets - 1);
        self.tick_filtered(
            world,
            econ,
            research_state,
            v6_db,
            |country_index| country_index % buckets == bucket,
            bucket + 1 == buckets,
        )
    }

    fn tick_filtered<F>(
        &mut self,
        world: &mut World,
        econ: &mut EconomyState,
        research_state: &mut ResearchState,
        v6_db: &V6Database,
        mut include_country: F,
        run_weekly_summary: bool,
    ) -> StrategicTickReport
    where
        F: FnMut(usize) -> bool,
    {
        let mut report = StrategicTickReport::default();
        let cadence = AiCadence::default();
        let day = world.date.days_since_epoch();

        if self.states.len() < world.countries.count {
            self.ensure_capacity(world);
        }

        for ci in 0..world.countries.count {
            if !include_country(ci) {
                continue;
            }
            if ci >= self.states.len() {
                break;
            }
            let country = CountryId(ci as u16);

            if country == world.player {
                continue;
            }

            if world.diplomacy.annexed_countries.contains(&country) {
                continue;
            }

            if !country_has_ai_work(world, country) {
                continue;
            }

            let state = &mut self.states[ci];
            let profile = state.profile.clone();

            if cadence_due(day, state.last_intent_eval, cadence.intent_days, ci) {
                state.last_intent_eval = day;
                let postures_snapshot = state.last_postures.clone();
                state.intent =
                    intent::evaluate_intent(world, country, &profile, &postures_snapshot, day);
            }

            let current_intent = state.intent.clone();

            if cadence_due(day, state.last_focus_eval, cadence.focus_days, ci) {
                state.last_focus_eval = day;
                let candidates = focus::evaluate_focuses(world, country);
                if let Some(best) = candidates.first() {
                    focus::apply_focus_decision(world, country, &best.value, &world.data.clone());
                    report
                        .focus_decisions
                        .push((country, best.value.focus_id.clone()));
                }
            }

            if cadence_due(day, state.last_research_eval, cadence.research_days, ci) {
                state.last_research_eval = day;
                let candidates =
                    research::evaluate_research(world, country, research_state, &profile, v6_db);
                if let Some(best) = candidates.first() {
                    research::apply_research_decision(
                        research_state,
                        world,
                        country,
                        &best.value,
                        v6_db,
                    );
                    report
                        .research_decisions
                        .push((country, best.value.tech_key.clone()));
                }
            }

            let prod_due =
                cadence_due(day, state.last_production_eval, cadence.production_days, ci);
            if prod_due {
                state.last_production_eval = day;
                let decision = production::evaluate_production(
                    world,
                    country,
                    econ,
                    &profile,
                    &current_intent,
                    v6_db,
                );
                if !decision.construction.is_empty() || !decision.production_lines.is_empty() {
                    let desc = format!(
                        "build={} prod={}",
                        decision.construction.len(),
                        decision.production_lines.len()
                    );
                    production::apply_production_decision(econ, world, country, &decision);
                    report.production_decisions.push((country, desc));
                }
            }

            let recruit_cadence = if world.countries.at_war[ci] {
                recruit::RECRUIT_CADENCE_DAYS_WAR
            } else {
                recruit::RECRUIT_CADENCE_DAYS
            };
            if cadence_due(day, state.last_recruit_eval, recruit_cadence, ci) {
                state.last_recruit_eval = day;
                if let Some(template_idx) =
                    recruit::try_recruit(world, econ, country, &profile, &current_intent)
                {
                    report
                        .production_decisions
                        .push((country, format!("recruit tpl={template_idx}")));
                }
            }

            if cadence_due(day, state.last_diplomacy_eval, cadence.diplomacy_days, ci) {
                state.last_diplomacy_eval = day;
                let candidates = diplomacy::evaluate_diplomacy(world, country, &profile);
                if let Some(best) = candidates.first() {
                    let desc = format!("{:?}", best.value);
                    diplomacy::apply_diplomacy_decision(world, country, &best.value);
                    report.diplomacy_decisions.push((country, desc));
                }
            }

            if cadence_due(day, state.last_tactical_eval, cadence.tactical_days, ci) {
                state.last_tactical_eval = day;
                let mut tactical_summary: Vec<String> = Vec::new();

                if let Some(summary) =
                    crate::china_theater::tick_prewar_china_deployment(world, country)
                {
                    tactical_summary.push(summary);
                }

                if world.diplomacy.is_at_war(country) {
                    let front = frontline::compute_front(world, country);
                    if front.has_front() {
                        let mut eval = ground::evaluate_ground(world, country, &front, &profile);
                        let mut op_postures = std::collections::HashMap::new();
                        let progress_scores = frontline_progress_scores(
                            world,
                            country,
                            eval.decisions.iter().map(|d| d.enemy),
                        );
                        for d in eval.decisions.iter_mut() {
                            let key = d.enemy.0;
                            let progress = progress_scores.get(&key).copied().unwrap_or(0);
                            let japanese_china_front =
                                is_japan_china_incident_front(world, country, d.enemy);
                            let op = update_operation_state(
                                state.operations.get(&key).copied(),
                                d.posture,
                                d.ratio,
                                progress,
                                day,
                                japanese_china_front,
                            );
                            state.operations.insert(key, op);
                            d.posture = posture_for_operation(d.posture, op.state);
                            let prev = state.last_postures.get(&key).copied();
                            let new_posture = match prev {
                                Some((prev_p, prev_day)) => {
                                    if d.posture == prev_p {
                                        d.posture
                                    } else if day - prev_day < POSTURE_STICKY_DAYS as i64 {
                                        prev_p
                                    } else {
                                        match (prev_p, d.posture) {
                                            (
                                                ground::GroundPosture::Attack,
                                                ground::GroundPosture::Defend,
                                            )
                                            | (
                                                ground::GroundPosture::Attack,
                                                ground::GroundPosture::Retreat,
                                            ) => {
                                                if d.ratio < 0.85 {
                                                    d.posture
                                                } else {
                                                    prev_p
                                                }
                                            }
                                            (
                                                ground::GroundPosture::Defend,
                                                ground::GroundPosture::Attack,
                                            )
                                            | (
                                                ground::GroundPosture::Retreat,
                                                ground::GroundPosture::Attack,
                                            )
                                            | (
                                                ground::GroundPosture::Hold,
                                                ground::GroundPosture::Attack,
                                            ) => {
                                                if d.ratio > 1.0 {
                                                    d.posture
                                                } else {
                                                    prev_p
                                                }
                                            }
                                            _ => d.posture,
                                        }
                                    }
                                }
                                None => d.posture,
                            };
                            state.last_postures.insert(key, (new_posture, day));
                            d.posture = new_posture;
                            op_postures.insert(key, d.posture);
                        }

                        let (atk, def, enc) = ground::decision_summary(&eval);
                        let sector_log = eval
                            .decisions
                            .iter()
                            .filter_map(|d| d.sectors.first().map(|s| s.reason.clone()))
                            .take(2)
                            .collect::<Vec<_>>()
                            .join(" ; ");
                        if sector_log.is_empty() {
                            tactical_summary.push(format!(
                                "ground atk={} def={} enc={} ops={}",
                                atk,
                                def,
                                enc,
                                operation_summary(&state.operations)
                            ));
                        } else {
                            tactical_summary.push(format!(
                                "ground atk={} def={} enc={} ops={} | {}",
                                atk,
                                def,
                                enc,
                                operation_summary(&state.operations),
                                sector_log
                            ));
                        }
                        crate::frontline_ai::tick_ai_frontlines(
                            world,
                            country,
                            &eval.decisions,
                            &front,
                        );
                        for d in eval.decisions.iter_mut() {
                            if let Some(&p) = op_postures.get(&d.enemy.0) {
                                d.posture = p;
                            }
                        }
                        ground_orders::execute_ground_orders(
                            world,
                            econ,
                            country,
                            &eval,
                            &current_intent,
                            &front,
                        );
                    } else {
                        ground_orders::execute_mop_up_only(world, econ, country);
                        tactical_summary.push("ground mop-up".to_string());
                    }
                }

                if country_has_fleets(world, country) {
                    let naval_decisions = naval::evaluate_naval(world, country, &profile);
                    if !naval_decisions.is_empty() {
                        naval::apply_naval_decisions(world, &naval_decisions);
                        tactical_summary.push(format!("naval×{}", naval_decisions.len()));
                    }
                }

                if crate::china_theater::is_japan_china_war_active(world, country) {
                    let state = &mut self.states[ci];
                    let new_plans = crate::china_theater::generate_invasion_plans(
                        world,
                        country,
                        &state.invasion_plans,
                        &mut state.next_invasion_plan_id,
                    );
                    state.invasion_plans.extend(new_plans);
                    crate::china_theater::tick_invasion_plans(
                        world,
                        econ,
                        country,
                        &mut state.invasion_plans,
                    )
                    .into_iter()
                    .take(3)
                    .for_each(|diag| tactical_summary.push(format!("invasion {diag}")));
                    let active_count = state
                        .invasion_plans
                        .iter()
                        .filter(|p| p.is_active())
                        .count();
                    if active_count > 0 {
                        tactical_summary.push(format!("invasion_plans×{}", active_count));
                    }
                }

                if country_has_air_wings(world, country) {
                    let air_decisions = air::evaluate_air(world, country, &profile);
                    if !air_decisions.is_empty() {
                        air::apply_air_decisions(world, &air_decisions);
                        tactical_summary.push(format!("air×{}", air_decisions.len()));
                    }
                }

                if !tactical_summary.is_empty() {
                    report
                        .tactical_decisions
                        .push((country, tactical_summary.join(" | ")));
                }
            }
        }

        let is_weekly = world.date.is_week_anchor();
        if run_weekly_summary && is_weekly {
            let summary_count = world.countries.count.min(self.states.len());
            let active_countries: Vec<usize> = (0..summary_count)
                .filter(|&ci| {
                    let country = CountryId(ci as u16);
                    country != world.player
                        && !world.diplomacy.annexed_countries.contains(&country)
                        && (world.diplomacy.is_at_war(country)
                            || !self.states[ci].last_postures.is_empty())
                })
                .collect();
            let summaries: Vec<String> = active_countries
                .into_par_iter()
                .filter_map(|ci| {
                    let country = CountryId(ci as u16);
                    let summary =
                        build_country_summary(world, econ, country, &self.states[ci].last_postures);
                    Some(format!("{}", summary))
                })
                .collect();
            for summary in summaries {
                self.log.push(summary);
            }
        }

        report
    }
}

fn country_has_ai_work(world: &World, country: CountryId) -> bool {
    let ci = country.0 as usize;
    if ci >= world.countries.count {
        return false;
    }
    country_has_states(world, country)
        || country_has_divisions(world, country)
        || country_has_fleets(world, country)
        || country_has_air_wings(world, country)
        || world.diplomacy.is_at_war(country)
}

fn country_has_states(world: &World, country: CountryId) -> bool {
    let ci = country.0 as usize;
    world
        .country_state_index
        .get(ci)
        .is_some_and(|states| !states.is_empty())
}

fn country_has_divisions(world: &World, country: CountryId) -> bool {
    let ci = country.0 as usize;
    world
        .country_division_index
        .get(ci)
        .is_some_and(|divisions| !divisions.is_empty())
}

fn country_has_fleets(world: &World, country: CountryId) -> bool {
    let ci = country.0 as usize;
    world
        .country_fleet_index
        .get(ci)
        .is_some_and(|fleets| !fleets.is_empty())
}

fn country_has_air_wings(world: &World, country: CountryId) -> bool {
    let ci = country.0 as usize;
    world
        .country_air_wing_index
        .get(ci)
        .is_some_and(|air_wings| !air_wings.is_empty())
}

fn cadence_due(day: i64, last_eval: i64, cadence_days: u32, country_index: usize) -> bool {
    let cadence = cadence_days.max(1) as i64;
    if cadence == 1 {
        return day >= last_eval + 1;
    }
    let offset = country_index as i64 % cadence;
    if day < offset || (day - offset) % cadence != 0 {
        return false;
    }
    last_eval == 0 || day >= last_eval + cadence
}

fn update_operation_state(
    prev: Option<OperationPlanState>,
    posture: ground::GroundPosture,
    ratio: f32,
    progress: i32,
    day: i64,
    japanese_china_front: bool,
) -> OperationPlanState {
    let Some(mut op) = prev else {
        let state = match posture {
            ground::GroundPosture::Attack => OperationState::Preparing,
            ground::GroundPosture::Retreat => OperationState::Recovering,
            ground::GroundPosture::Defend | ground::GroundPosture::Hold => OperationState::Paused,
        };
        return OperationPlanState {
            state,
            since_day: day,
            last_ratio: ratio,
            last_progress: progress,
            failure_count: 0,
        };
    };

    let old_state = op.state;
    let days_in_state = day - op.since_day;
    let lost_ground = progress < op.last_progress && days_in_state >= 7;
    let improved = progress > op.last_progress;

    op.state = match (op.state, posture) {
        (_, ground::GroundPosture::Retreat) => OperationState::Recovering,
        (OperationState::Paused, ground::GroundPosture::Attack) if days_in_state >= 10 => {
            OperationState::Preparing
        }
        (OperationState::Recovering, ground::GroundPosture::Attack)
            if ratio >= 0.95 && days_in_state >= 7 =>
        {
            OperationState::Preparing
        }
        (OperationState::Preparing, ground::GroundPosture::Attack)
            if ratio >= 1.05 || days_in_state >= 7 =>
        {
            OperationState::Attacking
        }
        (OperationState::Attacking, ground::GroundPosture::Attack) if improved => {
            OperationState::Exploiting
        }
        (OperationState::Attacking, ground::GroundPosture::Attack)
            if lost_ground || ratio < if japanese_china_front { 0.65 } else { 0.75 } =>
        {
            op.failure_count = op.failure_count.saturating_add(1);
            OperationState::Recovering
        }
        (OperationState::Exploiting, ground::GroundPosture::Attack)
            if ratio < if japanese_china_front { 0.70 } else { 0.8 } =>
        {
            OperationState::Recovering
        }
        (OperationState::Exploiting, ground::GroundPosture::Attack) if lost_ground => {
            OperationState::Preparing
        }
        (_, ground::GroundPosture::Defend | ground::GroundPosture::Hold) => OperationState::Paused,
        (state, _) => state,
    };

    if op.state != old_state {
        op.since_day = day;
    }
    op.last_ratio = ratio;
    op.last_progress = progress;
    op
}

fn is_japan_china_incident_front(world: &World, country: CountryId, enemy: CountryId) -> bool {
    if !matches!(world.country_tag(country), Some("JAP")) {
        return false;
    }
    let ci = country.0 as usize;
    if ci >= world.countries.ideas.len()
        || !world.countries.ideas[ci]
            .iter()
            .any(|idea| idea == "FLAG:china_incident_escalated")
    {
        return false;
    }
    matches!(world.country_tag(enemy), Some("SND" | "CHI" | "SHX"))
}

fn posture_for_operation(
    posture: ground::GroundPosture,
    op_state: OperationState,
) -> ground::GroundPosture {
    match op_state {
        // There is no implemented planning-bonus mechanic yet. Suppressing Attack
        // during Preparing/Paused only makes fronts go quiet for weeks.
        OperationState::Preparing
        | OperationState::Attacking
        | OperationState::Exploiting
        | OperationState::Paused => posture,
        OperationState::Recovering => ground::GroundPosture::Defend,
    }
}

fn frontline_progress_scores<I>(
    world: &World,
    country: CountryId,
    enemies: I,
) -> std::collections::HashMap<u16, i32>
where
    I: IntoIterator<Item = CountryId>,
{
    let enemy_set: std::collections::HashSet<CountryId> = enemies.into_iter().collect();
    let mut scores: std::collections::HashMap<u16, i32> =
        enemy_set.iter().map(|enemy| (enemy.0, 0i32)).collect();

    if enemy_set.is_empty() {
        return scores;
    }

    // Progress happens province-by-province. Waiting for an entire state to flip
    // makes active offensives look stalled for weeks and pushes the AI into pause.
    for pi in 0..world.provinces.count {
        let owner = world.provinces.owners[pi];
        let controller = world.provinces.controllers[pi];
        if enemy_set.contains(&owner) && controller == country {
            if let Some(score) = scores.get_mut(&owner.0) {
                *score += 1;
            }
        }
        if owner == country && enemy_set.contains(&controller) {
            if let Some(score) = scores.get_mut(&controller.0) {
                *score -= 1;
            }
        }
    }

    for si in 0..world.states.count {
        let owner = world.states.owners[si];
        let controller = world.states.controllers[si];
        if enemy_set.contains(&owner) && controller == country {
            if let Some(score) = scores.get_mut(&owner.0) {
                *score += 10;
            }
        }
        if owner == country && enemy_set.contains(&controller) {
            if let Some(score) = scores.get_mut(&controller.0) {
                *score -= 12;
            }
        }
    }

    scores
}

fn operation_summary(operations: &std::collections::HashMap<u16, OperationPlanState>) -> String {
    if operations.is_empty() {
        return "none".to_string();
    }
    let mut counts: std::collections::BTreeMap<&'static str, usize> =
        std::collections::BTreeMap::new();
    for op in operations.values() {
        let key = match op.state {
            OperationState::Preparing => "prep",
            OperationState::Attacking => "atk",
            OperationState::Exploiting => "exploit",
            OperationState::Recovering => "recover",
            OperationState::Paused => "pause",
        };
        *counts.entry(key).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .map(|(k, v)| format!("{k}:{v}"))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_state_prepares_before_attacking() {
        let op = update_operation_state(None, ground::GroundPosture::Attack, 1.2, 0, 10, false);
        assert_eq!(op.state, OperationState::Preparing);

        let op = update_operation_state(Some(op), ground::GroundPosture::Attack, 1.2, 0, 17, false);
        assert_eq!(op.state, OperationState::Attacking);
    }

    #[test]
    fn operation_state_recovers_after_failed_attack() {
        let op = OperationPlanState {
            state: OperationState::Attacking,
            since_day: 10,
            last_ratio: 1.0,
            last_progress: 5,
            failure_count: 0,
        };

        let op = update_operation_state(Some(op), ground::GroundPosture::Attack, 0.7, 5, 18, false);
        assert_eq!(op.state, OperationState::Recovering);
    }
}
