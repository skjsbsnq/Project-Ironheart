//! Commander bonuses for player/AI frontline armies.

use hoi4_state::frontline::General;
use hoi4_state::World;

/// Effective commander modifiers for one division.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeneralModifier {
    pub skill: u8,
    pub attack: u8,
    pub defense: u8,
    pub planning: u8,
    pub logistics: u8,
    pub command_limit: u16,
    pub army_size: usize,
    pub command_efficiency: f32,
    pub attack_mult: f32,
    pub defense_mult: f32,
    pub plan_efficiency_mult: f32,
    pub org_recovery_mult: f32,
    pub supply_mult: f32,
}

impl GeneralModifier {
    pub fn from_general(general: &General, army_size: usize) -> Self {
        let command_limit = general.command_limit.max(1);
        let command_efficiency = if army_size > command_limit as usize {
            (command_limit as f32 / army_size as f32).clamp(0.25, 1.0)
        } else {
            1.0
        };
        let scaled =
            |value: u8, per_level: f32| 1.0 + value as f32 * per_level * command_efficiency;
        Self {
            skill: general.skill,
            attack: general.attack,
            defense: general.defense,
            planning: general.planning,
            logistics: general.logistics,
            command_limit,
            army_size,
            command_efficiency,
            attack_mult: scaled(general.attack, 0.03),
            defense_mult: scaled(general.defense, 0.03),
            plan_efficiency_mult: scaled(general.planning, 0.04),
            org_recovery_mult: scaled(general.logistics, 0.025),
            supply_mult: (1.0 - general.logistics as f32 * 0.02 * command_efficiency)
                .clamp(0.75, 1.0),
        }
    }

    pub fn over_command_limit(self) -> bool {
        self.army_size > self.command_limit as usize
    }
}

pub fn modifier_for_division(world: &World, div_idx: usize) -> Option<GeneralModifier> {
    let army = world
        .player_armies
        .iter()
        .find(|army| army.members.contains(&div_idx))?;
    let commander = army.commander?;
    let general = world
        .generals
        .iter()
        .find(|general| general.id == commander)?;
    Some(GeneralModifier::from_general(general, army.members.len()))
}

pub fn plan_efficiency_for_army(world: &World, army_idx: usize) -> f32 {
    world
        .player_armies
        .get(army_idx)
        .and_then(|army| {
            let commander = army.commander?;
            let general = world
                .generals
                .iter()
                .find(|general| general.id == commander)?;
            Some(GeneralModifier::from_general(general, army.members.len()).plan_efficiency_mult)
        })
        .unwrap_or(1.0)
}
