use hoi4_state::{AutonomyLevel, CountryId, StateId, World};

use crate::diplomacy::{
    add_war_participant, force_declare_war, set_puppet, FactionError, WarError,
};

pub use hoi4_state::scripted_effects::{ScriptedDiplomaticEffect, ScriptedEffectError};

pub fn apply_diplomatic_effect(
    world: &mut World,
    effect: &ScriptedDiplomaticEffect,
) -> Result<(), ScriptedEffectError> {
    match effect {
        ScriptedDiplomaticEffect::CreateFaction { leader, name } => {
            let leader = country(world, leader)?;
            match crate::diplomacy::factions::create_faction(world, leader, name.clone()) {
                Ok(_) | Err(FactionError::AlreadyInFaction) => Ok(()),
                Err(FactionError::DuplicateName) => Err(ScriptedEffectError::Faction(
                    "duplicate faction name".to_owned(),
                )),
                Err(err) => Err(ScriptedEffectError::Faction(format!("{err:?}"))),
            }
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
            crate::diplomacy::factions::join_faction(world, fid, member)
                .map_err(|err| ScriptedEffectError::Faction(format!("{err:?}")))
        }
        ScriptedDiplomaticEffect::DeclareWar { attacker, defender } => {
            let attacker = country(world, attacker)?;
            let defender = country(world, defender)?;
            match force_declare_war(world, attacker, defender) {
                Ok(_) | Err(WarError::AlreadyAtWar) => Ok(()),
                Err(err) => Err(ScriptedEffectError::War(format!("{err:?}"))),
            }
        }
        ScriptedDiplomaticEffect::AddWarParticipant {
            war_leader,
            participant,
        } => {
            let leader = country(world, war_leader)?;
            let participant = country(world, participant)?;
            add_war_participant(world, leader, participant)
                .map_err(|err| ScriptedEffectError::War(format!("{err:?}")))
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
            set_puppet(world, master, subject, level)
                .map_err(|err| ScriptedEffectError::Faction(format!("{err:?}")))
        }
        ScriptedDiplomaticEffect::AnnexCountry { annexer, target } => {
            let annexer = country(world, annexer)?;
            let target = country(world, target)?;
            hoi4_state::scripted_effects::annex_country(world, annexer, target);
            Ok(())
        }
        ScriptedDiplomaticEffect::TransferState { state, owner } => {
            let owner = country(world, owner)?;
            let state = state_id(world, *state)?;
            hoi4_state::scripted_effects::transfer_state(world, state, owner, false);
            Ok(())
        }
        ScriptedDiplomaticEffect::AddCore {
            state,
            country: tag,
        } => {
            let country = country(world, tag)?;
            let state = state_id(world, *state)?;
            hoi4_state::scripted_effects::add_core(world, state, country);
            Ok(())
        }
        ScriptedDiplomaticEffect::SetWarJoinPolicy {
            war_leader,
            country: target,
            policy,
        } => {
            let leader = country(world, war_leader)?;
            let target = country(world, target)?;
            let war_id = find_war_by_leader(world, leader)?;
            crate::diplomacy::set_war_join_policy(world, war_id, target, war_join_policy(policy)?)
                .map_err(|err| ScriptedEffectError::War(format!("{err:?}")))
        }
        ScriptedDiplomaticEffect::AddDelayedWarParticipant {
            war_leader,
            participant,
        } => {
            let leader = country(world, war_leader)?;
            let participant = country(world, participant)?;
            let war_id = find_war_by_leader(world, leader)?;
            crate::diplomacy::add_delayed_war_participant(world, war_id, leader, participant)
                .map_err(|err| ScriptedEffectError::War(format!("{err:?}")))
        }
    }
}

pub fn apply_situation_diplomatic_effect(
    world: &mut World,
    effect: &hoi4_content::SituationEffect,
) -> Option<Result<(), ScriptedEffectError>> {
    let scripted = match effect {
        hoi4_content::SituationEffect::CreateWar { attacker, defender } => {
            ScriptedDiplomaticEffect::DeclareWar {
                attacker: attacker.clone(),
                defender: defender.clone(),
            }
        }
        hoi4_content::SituationEffect::AnnexCountry { annexer, target } => {
            ScriptedDiplomaticEffect::AnnexCountry {
                annexer: annexer.clone(),
                target: target.clone(),
            }
        }
        hoi4_content::SituationEffect::CreateFaction { leader, name } => {
            ScriptedDiplomaticEffect::CreateFaction {
                leader: leader.clone(),
                name: name.clone(),
            }
        }
        hoi4_content::SituationEffect::AddToFaction {
            faction_leader,
            member,
        } => ScriptedDiplomaticEffect::AddToFaction {
            faction_leader: faction_leader.clone(),
            member: member.clone(),
        },
        hoi4_content::SituationEffect::AddWarParticipant {
            war_leader,
            participant,
        } => ScriptedDiplomaticEffect::AddWarParticipant {
            war_leader: war_leader.clone(),
            participant: participant.clone(),
        },
        hoi4_content::SituationEffect::GrantMilitaryAccess { grantor, grantee } => {
            ScriptedDiplomaticEffect::GrantMilitaryAccess {
                grantor: grantor.clone(),
                grantee: grantee.clone(),
            }
        }
        hoi4_content::SituationEffect::SetAutonomy {
            master,
            subject,
            level,
        } => ScriptedDiplomaticEffect::SetAutonomy {
            master: master.clone(),
            subject: subject.clone(),
            level: level.clone(),
        },
        hoi4_content::SituationEffect::TransferState { state, owner } => {
            ScriptedDiplomaticEffect::TransferState {
                state: *state,
                owner: owner.clone(),
            }
        }
        hoi4_content::SituationEffect::AddCore { state, country } => {
            ScriptedDiplomaticEffect::AddCore {
                state: *state,
                country: country.clone(),
            }
        }
        hoi4_content::SituationEffect::SetWarJoinPolicy {
            war_leader,
            country,
            policy,
        } => ScriptedDiplomaticEffect::SetWarJoinPolicy {
            war_leader: war_leader.clone(),
            country: country.clone(),
            policy: policy.clone(),
        },
        hoi4_content::SituationEffect::AddDelayedWarParticipant {
            war_leader,
            participant,
        } => ScriptedDiplomaticEffect::AddDelayedWarParticipant {
            war_leader: war_leader.clone(),
            participant: participant.clone(),
        },
        _ => return None,
    };
    Some(apply_diplomatic_effect(world, &scripted))
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

fn war_join_policy(policy: &str) -> Result<hoi4_state::WarJoinPolicy, ScriptedEffectError> {
    match policy {
        "AutoJoin" | "auto_join" => Ok(hoi4_state::WarJoinPolicy::AutoJoin),
        "Delayed" | "delayed" => Ok(hoi4_state::WarJoinPolicy::Delayed),
        "Forbidden" | "forbidden" => Ok(hoi4_state::WarJoinPolicy::Forbidden),
        other => Err(ScriptedEffectError::War(format!(
            "invalid war join policy: {other}"
        ))),
    }
}

fn find_war_by_leader(world: &World, leader: CountryId) -> Result<u32, ScriptedEffectError> {
    world
        .diplomacy
        .wars
        .iter()
        .find(|(_, war)| war.primary_attacker == leader || war.primary_defender == leader)
        .map(|(id, _)| *id)
        .ok_or_else(|| ScriptedEffectError::War("war leader is not in a war".into()))
}
