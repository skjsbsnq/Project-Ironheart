//! Unified diplomacy action evaluation and execution.

use hoi4_state::Wargoal;
use hoi4_state::{
    CountryId, DiplomaticRequestKind, DiplomaticRequestStatus, StateId, TreatyKind, WarSide,
    WargoalType, World,
};

use super::{factions, peace, war, wargoal};

#[derive(Debug, Clone, PartialEq)]
pub enum DiplomaticAction {
    StartJustification {
        target: CountryId,
        kind: WargoalType,
        target_state: Option<StateId>,
    },
    DeclareWar {
        target: CountryId,
    },
    CreateFaction {
        name: String,
    },
    InviteToFaction {
        target: CountryId,
    },
    LeaveFaction,
    RequestMilitaryAccess {
        target: CountryId,
    },
    GrantMilitaryAccess {
        target: CountryId,
    },
    RevokeMilitaryAccess {
        target: CountryId,
    },
    ResolvePeace {
        war_id: u32,
        winning_side: WarSide,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnavailableReason {
    BadActor,
    BadTarget,
    SelfTarget,
    AlreadyAtWar,
    MissingJustifiedWargoal,
    AlreadyInFaction,
    NotInFaction,
    TargetAlreadyInFaction,
    NoFactionToInviteFrom,
    OpinionTooLow { current: i16, required: i16 },
    DuplicateWargoal,
    InsufficientPoliticalPower,
    MissingTargetState,
    ExtraneousTargetState,
    BadFaction,
    WarNotFound,
    NotWarParticipant,
    DuplicatePendingRequest,
    PeaceNotReady,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionAvailability {
    pub available: bool,
    pub reason: Option<UnavailableReason>,
}

impl ActionAvailability {
    pub fn available() -> Self {
        Self {
            available: true,
            reason: None,
        }
    }

    pub fn unavailable(reason: UnavailableReason) -> Self {
        Self {
            available: false,
            reason: Some(reason),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActionPreview {
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActionOutcome {
    pub summary: String,
    pub war_id: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiplomacyError {
    Unavailable(UnavailableReason),
    Faction(factions::FactionError),
    War(war::WarError),
    Wargoal(wargoal::WargoalError),
    PeaceFailed,
}

pub fn preview_action(world: &World, actor: CountryId, action: &DiplomaticAction) -> ActionPreview {
    let actor_tag = tag_or_id(world, actor);
    let summary = match action {
        DiplomaticAction::StartJustification { target, .. } => {
            format!(
                "{actor_tag} starts justifying against {}",
                tag_or_id(world, *target)
            )
        }
        DiplomaticAction::DeclareWar { target } => {
            format!("{actor_tag} declares war on {}", tag_or_id(world, *target))
        }
        DiplomaticAction::CreateFaction { name } => {
            format!("{actor_tag} creates faction {name}")
        }
        DiplomaticAction::InviteToFaction { target } => {
            format!(
                "{actor_tag} invites {} to faction",
                tag_or_id(world, *target)
            )
        }
        DiplomaticAction::LeaveFaction => format!("{actor_tag} leaves faction"),
        DiplomaticAction::RequestMilitaryAccess { target } => format!(
            "{actor_tag} requests military access from {}",
            tag_or_id(world, *target)
        ),
        DiplomaticAction::GrantMilitaryAccess { target } => format!(
            "{actor_tag} grants military access to {}",
            tag_or_id(world, *target)
        ),
        DiplomaticAction::RevokeMilitaryAccess { target } => format!(
            "{actor_tag} revokes military access from {}",
            tag_or_id(world, *target)
        ),
        DiplomaticAction::ResolvePeace { war_id, .. } => format!("Resolve war #{war_id}"),
    };
    ActionPreview { summary }
}

pub fn evaluate_action(
    world: &World,
    actor: CountryId,
    action: &DiplomaticAction,
) -> ActionAvailability {
    if !valid_country(world, actor) {
        return ActionAvailability::unavailable(UnavailableReason::BadActor);
    }

    match action {
        DiplomaticAction::StartJustification {
            target,
            kind,
            target_state,
        } => evaluate_start_justification(world, actor, *target, *kind, *target_state),
        DiplomaticAction::DeclareWar { target } => evaluate_declare_war(world, actor, *target),
        DiplomaticAction::CreateFaction { .. } => {
            if world.diplomacy.faction_of(actor).is_some() {
                ActionAvailability::unavailable(UnavailableReason::AlreadyInFaction)
            } else {
                ActionAvailability::available()
            }
        }
        DiplomaticAction::InviteToFaction { target } => {
            evaluate_invite_to_faction(world, actor, *target)
        }
        DiplomaticAction::LeaveFaction => {
            if world.diplomacy.faction_of(actor).is_none() {
                ActionAvailability::unavailable(UnavailableReason::NotInFaction)
            } else {
                ActionAvailability::available()
            }
        }
        DiplomaticAction::RequestMilitaryAccess { target } => {
            evaluate_request_military_access(world, actor, *target)
        }
        DiplomaticAction::GrantMilitaryAccess { target }
        | DiplomaticAction::RevokeMilitaryAccess { target } => {
            evaluate_target(world, actor, *target)
        }
        DiplomaticAction::ResolvePeace {
            war_id,
            winning_side,
        } => evaluate_resolve_peace(world, actor, *war_id, *winning_side),
    }
}

pub fn execute_action(
    world: &mut World,
    actor: CountryId,
    action: DiplomaticAction,
) -> Result<ActionOutcome, DiplomacyError> {
    let availability = evaluate_action(world, actor, &action);
    if let Some(reason) = availability.reason {
        return Err(DiplomacyError::Unavailable(reason));
    }

    match action {
        DiplomaticAction::StartJustification {
            target,
            kind,
            target_state,
        } => {
            wargoal::start_justification(world, actor, target, kind, target_state)
                .map_err(DiplomacyError::Wargoal)?;
            Ok(ActionOutcome {
                summary: "justification started".to_owned(),
                war_id: None,
            })
        }
        DiplomaticAction::DeclareWar { target } => {
            let war_id = war::declare_war(world, actor, target).map_err(DiplomacyError::War)?;
            Ok(ActionOutcome {
                summary: "war declared".to_owned(),
                war_id: Some(war_id),
            })
        }
        DiplomaticAction::CreateFaction { name } => {
            factions::create_faction(world, actor, name).map_err(DiplomacyError::Faction)?;
            Ok(ActionOutcome {
                summary: "faction created".to_owned(),
                war_id: None,
            })
        }
        DiplomaticAction::InviteToFaction { target } => {
            let faction = world
                .diplomacy
                .faction_of(actor)
                .ok_or(DiplomacyError::Unavailable(
                    UnavailableReason::NoFactionToInviteFrom,
                ))?;
            world.diplomacy.create_request(
                actor,
                target,
                DiplomaticRequestKind::InviteToFaction {
                    faction_id: faction,
                },
                world.elapsed_hours,
                Some(world.elapsed_hours + 24 * 30),
            );
            Ok(ActionOutcome {
                summary: "faction invitation sent".to_owned(),
                war_id: None,
            })
        }
        DiplomaticAction::LeaveFaction => {
            let faction = world
                .diplomacy
                .faction_of(actor)
                .ok_or(DiplomacyError::Unavailable(UnavailableReason::NotInFaction))?;
            factions::leave_faction(world, faction, actor).map_err(DiplomacyError::Faction)?;
            Ok(ActionOutcome {
                summary: "left faction".to_owned(),
                war_id: None,
            })
        }
        DiplomaticAction::RequestMilitaryAccess { target } => {
            world.diplomacy.create_request(
                actor,
                target,
                DiplomaticRequestKind::RequestMilitaryAccess,
                world.elapsed_hours,
                Some(world.elapsed_hours + 24 * 30),
            );
            Ok(ActionOutcome {
                summary: "military access requested".to_owned(),
                war_id: None,
            })
        }
        DiplomaticAction::GrantMilitaryAccess { target } => {
            world.diplomacy.grant_military_access(actor, target);
            Ok(ActionOutcome {
                summary: "military access granted".to_owned(),
                war_id: None,
            })
        }
        DiplomaticAction::RevokeMilitaryAccess { target } => {
            world.diplomacy.revoke_military_access(actor, target);
            Ok(ActionOutcome {
                summary: "military access revoked".to_owned(),
                war_id: None,
            })
        }
        DiplomaticAction::ResolvePeace {
            war_id,
            winning_side,
        } => {
            peace::peace_conference(world, war_id, winning_side)
                .ok_or(DiplomacyError::PeaceFailed)?;
            Ok(ActionOutcome {
                summary: "peace resolved".to_owned(),
                war_id: Some(war_id),
            })
        }
    }
}

/// Privileged debug effect for settings such as `instant_war`.
///
/// This deliberately bypasses normal action availability and should not be used by AI,
/// scripts, or regular UI flows.
pub fn debug_grant_justified_wargoal(
    world: &mut World,
    claimant: CountryId,
    target: CountryId,
    kind: WargoalType,
    target_state: Option<StateId>,
) {
    let already_justified = world
        .diplomacy
        .pending_wargoals
        .get(&claimant)
        .map(|goals| {
            goals.iter().any(|goal| {
                goal.target == target
                    && goal.kind == kind
                    && goal.target_state == target_state
                    && goal.justified
            })
        })
        .unwrap_or(false);
    if already_justified {
        return;
    }
    world
        .diplomacy
        .pending_wargoals
        .entry(claimant)
        .or_default()
        .push(Wargoal {
            claimant,
            target,
            kind,
            target_state,
            justified: true,
            justify_progress: 0.0,
            justify_total_days: 0.0,
        });
}

fn evaluate_target(world: &World, actor: CountryId, target: CountryId) -> ActionAvailability {
    if !valid_country(world, target) {
        return ActionAvailability::unavailable(UnavailableReason::BadTarget);
    }
    if actor == target {
        return ActionAvailability::unavailable(UnavailableReason::SelfTarget);
    }
    ActionAvailability::available()
}

fn evaluate_start_justification(
    world: &World,
    actor: CountryId,
    target: CountryId,
    kind: WargoalType,
    target_state: Option<StateId>,
) -> ActionAvailability {
    let target_ok = evaluate_target(world, actor, target);
    if !target_ok.available {
        return target_ok;
    }
    if kind.needs_state() && target_state.is_none() {
        return ActionAvailability::unavailable(UnavailableReason::MissingTargetState);
    }
    if !kind.needs_state() && target_state.is_some() {
        return ActionAvailability::unavailable(UnavailableReason::ExtraneousTargetState);
    }
    if world
        .diplomacy
        .pending_wargoals
        .get(&actor)
        .map(|goals| {
            goals.iter().any(|goal| {
                goal.target == target && goal.kind == kind && goal.target_state == target_state
            })
        })
        .unwrap_or(false)
    {
        return ActionAvailability::unavailable(UnavailableReason::DuplicateWargoal);
    }
    let ci = actor.0 as usize;
    if world.countries.political_power[ci] < super::constants::BASE_JUSTIFY_PP_COST {
        return ActionAvailability::unavailable(UnavailableReason::InsufficientPoliticalPower);
    }
    ActionAvailability::available()
}

fn evaluate_declare_war(world: &World, actor: CountryId, target: CountryId) -> ActionAvailability {
    let target_ok = evaluate_target(world, actor, target);
    if !target_ok.available {
        return target_ok;
    }
    if world.diplomacy.at_war_with(actor, target) {
        return ActionAvailability::unavailable(UnavailableReason::AlreadyAtWar);
    }
    let has_ready_goal = world
        .diplomacy
        .pending_wargoals
        .get(&actor)
        .map(|goals| {
            goals
                .iter()
                .any(|goal| goal.target == target && goal.justified)
        })
        .unwrap_or(false);
    if !has_ready_goal {
        return ActionAvailability::unavailable(UnavailableReason::MissingJustifiedWargoal);
    }
    ActionAvailability::available()
}

fn evaluate_invite_to_faction(
    world: &World,
    actor: CountryId,
    target: CountryId,
) -> ActionAvailability {
    let target_ok = evaluate_target(world, actor, target);
    if !target_ok.available {
        return target_ok;
    }
    if world.diplomacy.faction_of(actor).is_none() {
        return ActionAvailability::unavailable(UnavailableReason::NoFactionToInviteFrom);
    }
    if world.diplomacy.faction_of(target).is_some() {
        return ActionAvailability::unavailable(UnavailableReason::TargetAlreadyInFaction);
    }
    if world.diplomacy.at_war_with(actor, target) {
        return ActionAvailability::unavailable(UnavailableReason::AlreadyAtWar);
    }
    let Some(faction_id) = world.diplomacy.faction_of(actor) else {
        return ActionAvailability::unavailable(UnavailableReason::NoFactionToInviteFrom);
    };
    if world.diplomacy.diplomatic_requests.iter().any(|request| {
        request.from == actor
            && request.to == target
            && request.status == DiplomaticRequestStatus::Pending
            && request.kind == DiplomaticRequestKind::InviteToFaction { faction_id }
    }) {
        return ActionAvailability::unavailable(UnavailableReason::DuplicatePendingRequest);
    }
    ActionAvailability::available()
}

fn evaluate_request_military_access(
    world: &World,
    actor: CountryId,
    target: CountryId,
) -> ActionAvailability {
    let target_ok = evaluate_target(world, actor, target);
    if !target_ok.available {
        return target_ok;
    }
    if world.diplomacy.at_war_with(actor, target) {
        return ActionAvailability::unavailable(UnavailableReason::AlreadyAtWar);
    }
    if world.diplomacy.has_military_access(actor, target) {
        return ActionAvailability::unavailable(UnavailableReason::DuplicatePendingRequest);
    }
    if world.diplomacy.diplomatic_requests.iter().any(|request| {
        request.from == actor
            && request.to == target
            && request.status == DiplomaticRequestStatus::Pending
            && request.kind == DiplomaticRequestKind::RequestMilitaryAccess
    }) {
        return ActionAvailability::unavailable(UnavailableReason::DuplicatePendingRequest);
    }
    let opinion = world.diplomacy.opinions.get(target, actor);
    if opinion <= 50 {
        return ActionAvailability::unavailable(UnavailableReason::OpinionTooLow {
            current: opinion,
            required: 51,
        });
    }
    ActionAvailability::available()
}

pub fn tick_diplomatic_requests(world: &mut World) -> usize {
    let now = world.elapsed_hours;
    let mut accepted = Vec::new();
    let mut decisions = Vec::new();

    for (idx, request) in world.diplomacy.diplomatic_requests.iter().enumerate() {
        if request.status != DiplomaticRequestStatus::Pending {
            continue;
        }
        if request
            .expires_at_hour
            .is_some_and(|expires| now >= expires)
        {
            decisions.push((idx, DiplomaticRequestStatus::Expired));
            continue;
        }
        let should_accept = match &request.kind {
            DiplomaticRequestKind::InviteToFaction { faction_id } => {
                world.diplomacy.faction(*faction_id).is_some()
                    && world.diplomacy.faction_of(request.to).is_none()
                    && !world.diplomacy.at_war_with(request.from, request.to)
            }
            DiplomaticRequestKind::RequestMilitaryAccess => {
                world.diplomacy.opinions.get(request.to, request.from) > 50
                    && !world.diplomacy.at_war_with(request.from, request.to)
            }
            DiplomaticRequestKind::OfferNonAggressionPact | DiplomaticRequestKind::OfferPeace => {
                false
            }
        };
        if should_accept {
            decisions.push((idx, DiplomaticRequestStatus::Accepted));
            accepted.push((request.from, request.to, request.kind.clone()));
        } else {
            decisions.push((idx, DiplomaticRequestStatus::Rejected));
        }
    }

    for (idx, status) in &decisions {
        if let Some(request) = world.diplomacy.diplomatic_requests.get_mut(*idx) {
            request.status = *status;
            request.resolved_at_hour = Some(now);
        }
    }

    for (from, to, kind) in accepted {
        match kind {
            DiplomaticRequestKind::InviteToFaction { faction_id } => {
                let _ = factions::join_faction(world, faction_id, to);
            }
            DiplomaticRequestKind::RequestMilitaryAccess => {
                world
                    .diplomacy
                    .add_treaty(TreatyKind::MilitaryAccess, vec![to, from], now, None);
            }
            DiplomaticRequestKind::OfferNonAggressionPact | DiplomaticRequestKind::OfferPeace => {}
        }
    }

    world.diplomacy.treaties.retain(|treaty| {
        treaty
            .expires_at_hour
            .map(|expires| now < expires)
            .unwrap_or(true)
    });

    decisions.len()
}

fn evaluate_resolve_peace(
    world: &World,
    actor: CountryId,
    war_id: u32,
    winning_side: WarSide,
) -> ActionAvailability {
    let Some(war) = world.diplomacy.wars.get(&war_id) else {
        return ActionAvailability::unavailable(UnavailableReason::WarNotFound);
    };
    if war.side_of(actor) != Some(winning_side) {
        return ActionAvailability::unavailable(UnavailableReason::NotWarParticipant);
    }
    let score = match winning_side {
        WarSide::Attacker => war.attacker_war_score,
        WarSide::Defender => war.defender_war_score,
    };
    let enemy_capitulated =
        peace_opponents_for_side(war, winning_side)
            .into_iter()
            .any(|country| {
                super::war::is_capitulated(world, country)
                    || world.diplomacy.annexed_countries.contains(&country)
            });
    if score < 50.0 && !enemy_capitulated {
        return ActionAvailability::unavailable(UnavailableReason::PeaceNotReady);
    }
    ActionAvailability::available()
}

fn peace_opponents_for_side(war: &hoi4_state::War, winning_side: WarSide) -> Vec<CountryId> {
    let mut out: Vec<CountryId> = match winning_side {
        WarSide::Attacker => war.defenders.iter().copied().collect(),
        WarSide::Defender => war.attackers.iter().copied().collect(),
    };
    match winning_side {
        WarSide::Attacker => out.push(war.primary_defender),
        WarSide::Defender => out.push(war.primary_attacker),
    }
    let goals = match winning_side {
        WarSide::Attacker => &war.attacker_wargoals,
        WarSide::Defender => &war.defender_wargoals,
    };
    for goal in goals {
        out.push(goal.target);
    }
    out.sort_by_key(|country| country.0);
    out.dedup();
    out
}

fn valid_country(world: &World, country: CountryId) -> bool {
    !country.is_none() && (country.0 as usize) < world.countries.count
}

fn tag_or_id(world: &World, country: CountryId) -> String {
    world
        .country_tag(country)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("#{:?}", country))
}
