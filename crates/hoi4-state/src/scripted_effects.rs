use std::collections::{HashMap, HashSet};

use crate::{Autonomy, AutonomyLevel, CountryId, Faction, StateId, War, WarJoinPolicy, World};

const SCRIPTED_WAR_TENSION: f32 = 5.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptedEffectError {
    BadCountry(String),
    BadState(u16),
    BadAutonomyLevel(String),
    Faction(String),
    War(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScriptedDiplomaticEffect {
    CreateFaction {
        leader: String,
        name: String,
    },
    AddToFaction {
        faction_leader: String,
        member: String,
    },
    DeclareWar {
        attacker: String,
        defender: String,
    },
    AddWarParticipant {
        war_leader: String,
        participant: String,
    },
    GrantMilitaryAccess {
        grantor: String,
        grantee: String,
    },
    SetAutonomy {
        master: String,
        subject: String,
        level: String,
    },
    AnnexCountry {
        annexer: String,
        target: String,
    },
    TransferState {
        state: u16,
        owner: String,
    },
    AddCore {
        state: u16,
        country: String,
    },
    SetWarJoinPolicy {
        war_leader: String,
        country: String,
        policy: String,
    },
    AddDelayedWarParticipant {
        war_leader: String,
        participant: String,
    },
}

pub fn apply_diplomatic_effect(
    world: &mut World,
    effect: &ScriptedDiplomaticEffect,
) -> Result<(), ScriptedEffectError> {
    match effect {
        ScriptedDiplomaticEffect::CreateFaction { leader, name } => {
            let leader = country(world, leader)?;
            create_faction(world, leader, name.clone())?;
            Ok(())
        }
        ScriptedDiplomaticEffect::AddToFaction {
            faction_leader,
            member,
        } => {
            let leader = country(world, faction_leader)?;
            let member = country(world, member)?;
            let fid = world
                .diplomacy
                .faction_of(leader)
                .ok_or_else(|| ScriptedEffectError::Faction("leader has no faction".into()))?;
            if world.diplomacy.faction_of(member) == Some(fid) {
                return Ok(());
            }
            if world.diplomacy.faction_of(member).is_some() {
                return Err(ScriptedEffectError::Faction(
                    "member already in another faction".into(),
                ));
            }
            let faction = world
                .diplomacy
                .faction_mut(fid)
                .ok_or_else(|| ScriptedEffectError::Faction("faction id is invalid".into()))?;
            if !faction.members.contains(&member) {
                faction.members.push(member);
            }
            Ok(())
        }
        ScriptedDiplomaticEffect::DeclareWar { attacker, defender } => {
            let attacker = country(world, attacker)?;
            let defender = country(world, defender)?;
            force_declare_war(world, attacker, defender)?;
            Ok(())
        }
        ScriptedDiplomaticEffect::AddWarParticipant {
            war_leader,
            participant,
        } => {
            let leader = country(world, war_leader)?;
            let participant = country(world, participant)?;
            add_war_participant(world, leader, participant)
        }
        ScriptedDiplomaticEffect::GrantMilitaryAccess { grantor, grantee } => {
            let grantor = country(world, grantor)?;
            let grantee = country(world, grantee)?;
            world.diplomacy.grant_military_access(grantor, grantee);
            Ok(())
        }
        ScriptedDiplomaticEffect::SetAutonomy {
            master,
            subject,
            level,
        } => {
            let master = country(world, master)?;
            let subject = country(world, subject)?;
            let level = autonomy_level(level)?;
            if master == subject {
                return Err(ScriptedEffectError::Faction(
                    "self autonomy relation".into(),
                ));
            }
            world.diplomacy.autonomy.insert(
                subject,
                Autonomy {
                    master,
                    subject,
                    level,
                    progress: 0.0,
                    since_hour: world.elapsed_hours,
                },
            );
            Ok(())
        }
        ScriptedDiplomaticEffect::AnnexCountry { annexer, target } => {
            let annexer = country(world, annexer)?;
            let target = country(world, target)?;
            annex_country(world, annexer, target);
            Ok(())
        }
        ScriptedDiplomaticEffect::TransferState { state, owner } => {
            let owner = country(world, owner)?;
            let state = state_id(world, *state)?;
            transfer_state(world, state, owner, false);
            Ok(())
        }
        ScriptedDiplomaticEffect::AddCore {
            state,
            country: tag,
        } => {
            let country = country(world, tag)?;
            let state = state_id(world, *state)?;
            add_core(world, state, country);
            Ok(())
        }
        ScriptedDiplomaticEffect::SetWarJoinPolicy {
            war_leader,
            country: target,
            policy,
        } => {
            let leader = country(world, war_leader)?;
            let target = country(world, target)?;
            let policy = war_join_policy(policy)?;
            set_war_join_policy_inner(world, leader, target, policy)
        }
        ScriptedDiplomaticEffect::AddDelayedWarParticipant {
            war_leader,
            participant,
        } => {
            let leader = country(world, war_leader)?;
            let participant = country(world, participant)?;
            add_delayed_war_participant_inner(world, leader, participant)
        }
    }
}

pub fn force_declare_war(
    world: &mut World,
    attacker: CountryId,
    defender: CountryId,
) -> Result<u32, ScriptedEffectError> {
    if attacker == defender || attacker.is_none() || defender.is_none() {
        return Err(ScriptedEffectError::War("invalid war participants".into()));
    }
    if world.diplomacy.at_war_with(attacker, defender) {
        return Err(ScriptedEffectError::War("countries already at war".into()));
    }

    let mut attackers = HashSet::new();
    let mut defenders = HashSet::new();
    attackers.insert(attacker);
    defenders.insert(defender);
    if let Some(fid) = world.diplomacy.faction_of(attacker) {
        if let Some(faction) = world.diplomacy.faction(fid) {
            attackers.extend(faction.members.iter().copied());
        }
    }
    if let Some(fid) = world.diplomacy.faction_of(defender) {
        if let Some(faction) = world.diplomacy.faction(fid) {
            defenders.extend(faction.members.iter().copied());
        }
    }

    let war_id = world.diplomacy.next_war_id;
    world.diplomacy.next_war_id += 1;
    world.diplomacy.wars.insert(
        war_id,
        War {
            id: war_id,
            primary_attacker: attacker,
            primary_defender: defender,
            attackers: attackers.clone(),
            defenders: defenders.clone(),
            started_at_hour: world.elapsed_hours,
            attacker_war_score: 0.0,
            defender_war_score: 0.0,
            attacker_wargoals: vec![],
            defender_wargoals: vec![],
            war_join_policies: HashMap::new(),
        },
    );
    for country in attackers.iter().chain(defenders.iter()) {
        let idx = country.0 as usize;
        if idx < world.countries.count {
            world.countries.at_war[idx] = true;
        }
    }
    world.diplomacy.world_tension =
        (world.diplomacy.world_tension + SCRIPTED_WAR_TENSION).clamp(0.0, 100.0);
    Ok(war_id)
}

pub fn annex_country(world: &mut World, annexer: CountryId, target: CountryId) -> u32 {
    let mut transferred = 0;
    world.diplomacy.annexed_countries.insert(target);
    for si in 0..world.states.count {
        if world.states.owners[si] == target {
            transfer_state(world, StateId(si as u16), annexer, true);
            transferred += 1;
        }
    }
    remove_country_control(world, target);
    world.diplomacy.wars.retain(|_, war| !war.contains(target));
    let ti = target.0 as usize;
    if ti < world.countries.count {
        world.countries.at_war[ti] = false;
    }
    if let Some(tag) = world.countries.tags.get(ti).cloned() {
        world.tag_to_country.remove(&tag);
    }
    world.recalc_country_caches();
    transferred
}

pub fn remove_country_control(world: &mut World, country: CountryId) -> u32 {
    if country.is_none() {
        return 0;
    }

    let mut changed = 0;
    for si in 0..world.states.count {
        if world.states.controllers[si] == country {
            world.states.controllers[si] = world.states.owners[si];
            changed += 1;
        }
    }
    for pi in 0..world.provinces.count {
        if world.provinces.controllers[pi] == country {
            world.provinces.controllers[pi] = world.provinces.owners[pi];
            changed += 1;
        }
    }
    if changed > 0 {
        world.path_cache.clear();
    }
    changed
}

pub fn transfer_state(world: &mut World, state: StateId, owner: CountryId, add_owner_core: bool) {
    let si = state.0 as usize;
    if si >= world.states.count {
        return;
    }
    world.states.owners[si] = owner;
    world.states.controllers[si] = owner;
    if add_owner_core {
        add_core(world, state, owner);
    }
    for &province in &world.states.provinces[si] {
        let pi = province.0 as usize;
        if pi < world.provinces.count {
            world.provinces.owners[pi] = owner;
            world.provinces.controllers[pi] = owner;
        }
    }
    world.recalc_country_caches();
}

pub fn add_core(world: &mut World, state: StateId, country: CountryId) {
    let si = state.0 as usize;
    if si < world.states.count && !world.states.cores[si].contains(&country) {
        world.states.cores[si].push(country);
    }
}

fn create_faction(
    world: &mut World,
    leader: CountryId,
    name: String,
) -> Result<(), ScriptedEffectError> {
    if world.diplomacy.faction_of(leader).is_some() {
        return Err(ScriptedEffectError::Faction(
            "leader already in faction".into(),
        ));
    }
    if world.diplomacy.factions.iter().any(|f| f.name == name) {
        return Err(ScriptedEffectError::Faction(
            "duplicate faction name".into(),
        ));
    }
    let id = world.diplomacy.allocate_faction_id();
    world.diplomacy.factions.push(Faction {
        id,
        name,
        leader,
        members: vec![leader],
        created_at_hour: world.elapsed_hours,
    });
    Ok(())
}

fn add_war_participant(
    world: &mut World,
    war_leader: CountryId,
    participant: CountryId,
) -> Result<(), ScriptedEffectError> {
    let Some((_war_id, war)) =
        world.diplomacy.wars.iter_mut().find(|(_, war)| {
            war.primary_attacker == war_leader || war.primary_defender == war_leader
        })
    else {
        return Err(ScriptedEffectError::War(
            "war leader is not in a war".into(),
        ));
    };
    if war.attackers.contains(&war_leader) {
        war.attackers.insert(participant);
    } else {
        war.defenders.insert(participant);
    }
    let idx = participant.0 as usize;
    if idx < world.countries.count {
        world.countries.at_war[idx] = true;
    }
    Ok(())
}

fn country(world: &World, tag: &str) -> Result<CountryId, ScriptedEffectError> {
    world
        .country(tag)
        .ok_or_else(|| ScriptedEffectError::BadCountry(tag.to_owned()))
}

fn state_id(world: &World, state: u16) -> Result<StateId, ScriptedEffectError> {
    world
        .state_id_lookup
        .get(&state)
        .copied()
        .ok_or(ScriptedEffectError::BadState(state))
}

fn autonomy_level(level: &str) -> Result<AutonomyLevel, ScriptedEffectError> {
    match level {
        "integrated" => Ok(AutonomyLevel::Integrated),
        "integrated_puppet" => Ok(AutonomyLevel::IntegratedPuppet),
        "puppet" => Ok(AutonomyLevel::Puppet),
        "dominion" => Ok(AutonomyLevel::Dominion),
        "satellite" => Ok(AutonomyLevel::Satellite),
        "freedom_association" => Ok(AutonomyLevel::FreedomAssociation),
        other => Err(ScriptedEffectError::BadAutonomyLevel(other.to_owned())),
    }
}

fn war_join_policy(policy: &str) -> Result<WarJoinPolicy, ScriptedEffectError> {
    match policy {
        "AutoJoin" | "auto_join" => Ok(WarJoinPolicy::AutoJoin),
        "Delayed" | "delayed" => Ok(WarJoinPolicy::Delayed),
        "Forbidden" | "forbidden" => Ok(WarJoinPolicy::Forbidden),
        other => Err(ScriptedEffectError::War(format!(
            "invalid war join policy: {other}"
        ))),
    }
}

fn set_war_join_policy_inner(
    world: &mut World,
    war_leader: CountryId,
    target: CountryId,
    policy: WarJoinPolicy,
) -> Result<(), ScriptedEffectError> {
    let Some((_war_id, war)) =
        world.diplomacy.wars.iter_mut().find(|(_, war)| {
            war.primary_attacker == war_leader || war.primary_defender == war_leader
        })
    else {
        return Err(ScriptedEffectError::War(
            "war leader is not in a war".into(),
        ));
    };
    war.set_join_policy(target, policy);
    Ok(())
}

fn add_delayed_war_participant_inner(
    world: &mut World,
    war_leader: CountryId,
    participant: CountryId,
) -> Result<(), ScriptedEffectError> {
    let Some((_war_id, war)) =
        world.diplomacy.wars.iter_mut().find(|(_, war)| {
            war.primary_attacker == war_leader || war.primary_defender == war_leader
        })
    else {
        return Err(ScriptedEffectError::War(
            "war leader is not in a war".into(),
        ));
    };
    if war.attackers.contains(&participant) || war.defenders.contains(&participant) {
        return Err(ScriptedEffectError::War(
            "participant already in war".into(),
        ));
    }
    if war.attackers.contains(&war_leader) {
        war.attackers.insert(participant);
    } else {
        war.defenders.insert(participant);
    }
    war.war_join_policies.remove(&participant);
    let idx = participant.0 as usize;
    if idx < world.countries.count {
        world.countries.at_war[idx] = world.diplomacy.is_at_war(participant);
    }
    Ok(())
}
