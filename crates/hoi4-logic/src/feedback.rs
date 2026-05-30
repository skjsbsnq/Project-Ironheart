//! J.7: Feedback Bus — domain event bus with 6 feedback loops.
//!
//! Single-threaded, non-Send. Events published during a tick are drained at tick end.
//! Ring buffer of 4096 entries for debug inspection.

use hoi4_state::{CountryId, ProvinceId, StateId, World};

use crate::economy::EconomyState;

const MAX_CASCADE_DEPTH: u8 = 8;
const RING_BUFFER_CAP: usize = 4096;

/// Domain events published by game systems.
#[derive(Debug, Clone)]
pub enum DomainEvent {
    BattleResolved {
        winner: CountryId,
        loser: CountryId,
        province: ProvinceId,
        winner_losses: f32,
        loser_losses: f32,
    },
    WarDeclared {
        aggressor: CountryId,
        target: CountryId,
    },
    FactionJoined {
        member: CountryId,
    },
    StateOccupied {
        state: StateId,
        owner: CountryId,
        new_controller: CountryId,
    },
    FocusCompleted {
        country: CountryId,
        focus_id: String,
        aggressive: bool,
    },
    TechResearched {
        country: CountryId,
        tech_id: String,
        is_industry: bool,
    },
}

/// The Feedback Bus: collects domain events and drains them through feedback loops.
pub struct FeedbackBus {
    queue: Vec<DomainEvent>,
    /// Ring buffer log for debug panel (F3).
    pub log: Vec<DomainEvent>,
    log_cursor: usize,
}

impl FeedbackBus {
    pub fn new() -> Self {
        Self {
            queue: Vec::new(),
            log: Vec::with_capacity(RING_BUFFER_CAP),
            log_cursor: 0,
        }
    }

    /// Publish a domain event (enqueued for processing at drain time).
    pub fn publish(&mut self, event: DomainEvent) {
        self.queue.push(event);
    }

    /// Drain all queued events through feedback loops. Call once per tick_hour end.
    /// Handles cascading up to MAX_CASCADE_DEPTH.
    pub fn drain(&mut self, world: &mut World, econ: &mut EconomyState) {
        let mut depth = 0u8;
        while !self.queue.is_empty() && depth < MAX_CASCADE_DEPTH {
            let batch: Vec<DomainEvent> = self.queue.drain(..).collect();
            for event in batch {
                self.log_event(&event);
                Self::dispatch(event, world, econ, &mut self.queue);
            }
            depth += 1;
        }
        self.queue.clear(); // safety: discard any remaining if max depth hit
    }

    /// Number of events in the log.
    pub fn log_len(&self) -> usize {
        self.log.len().min(RING_BUFFER_CAP)
    }

    fn log_event(&mut self, event: &DomainEvent) {
        if self.log.len() < RING_BUFFER_CAP {
            self.log.push(event.clone());
        } else {
            self.log[self.log_cursor] = event.clone();
        }
        self.log_cursor = (self.log_cursor + 1) % RING_BUFFER_CAP;
    }

    fn dispatch(
        event: DomainEvent,
        world: &mut World,
        econ: &mut EconomyState,
        _cascade: &mut Vec<DomainEvent>,
    ) {
        match event {
            DomainEvent::BattleResolved {
                winner,
                loser,
                winner_losses,
                loser_losses,
                ..
            } => {
                loop_battle_to_stability(world, winner, loser, winner_losses, loser_losses);
            }
            DomainEvent::WarDeclared { aggressor, target } => {
                loop_diplomacy_war_declared(world, aggressor, target);
            }
            DomainEvent::FactionJoined { member } => {
                loop_diplomacy_faction_joined(world, member);
            }
            DomainEvent::StateOccupied {
                state,
                owner,
                new_controller,
            } => {
                loop_occupation_to_economy(world, econ, state, owner, new_controller);
            }
            DomainEvent::FocusCompleted {
                country,
                aggressive,
                ..
            } => {
                loop_focus_to_diplomacy(world, country, aggressive);
            }
            DomainEvent::TechResearched {
                country,
                is_industry,
                ..
            } => {
                loop_research_to_production(world, country, is_industry);
            }
        }
    }
}

// ─── Loop implementations ───────────────────────────────────────────

/// Loop_Battle_To_Stability: winner stability+/war_support+, loser stability-/war_support-
fn loop_battle_to_stability(
    world: &mut World,
    winner: CountryId,
    loser: CountryId,
    winner_losses: f32,
    loser_losses: f32,
) {
    let total = (winner_losses + loser_losses).max(1.0);
    let kill_ratio = loser_losses / total;

    let wi = winner.0 as usize;
    if wi < world.countries.count {
        world.countries.stability[wi] =
            (world.countries.stability[wi] + 0.001 * kill_ratio).min(1.0);
        world.countries.war_support[wi] = (world.countries.war_support[wi] + 0.002).min(1.0);
    }

    let loss_ratio = winner_losses / total;
    let li = loser.0 as usize;
    if li < world.countries.count {
        world.countries.stability[li] =
            (world.countries.stability[li] - 0.0005 * loss_ratio).max(0.0);
        world.countries.war_support[li] = (world.countries.war_support[li] - 0.001).max(0.0);
    }
}

/// Loop_Diplomacy_To_AI_Mood: war declared → third-party threat_perception up
fn loop_diplomacy_war_declared(world: &mut World, aggressor: CountryId, target: CountryId) {
    // Set opinion between aggressor and target to min(current, -50)
    let cur = world.diplomacy.opinions.get(aggressor, target);
    if cur > -50 {
        world.diplomacy.opinions.set(aggressor, target, -50);
        world.diplomacy.opinions.set(target, aggressor, -50);
    }

    // Increase world tension
    world.diplomacy.world_tension = (world.diplomacy.world_tension + 5.0).min(100.0);
}

/// Loop_Diplomacy_To_AI_Mood: faction joined → opinion +25 among members
fn loop_diplomacy_faction_joined(world: &mut World, member: CountryId) {
    if let Some(fid) = world.diplomacy.faction_of(member) {
        let members: Vec<CountryId> = world
            .diplomacy
            .factions
            .get(fid.0 as usize)
            .map(|f| f.members.clone())
            .unwrap_or_default();
        for &a in &members {
            for &b in &members {
                if a != b {
                    world.diplomacy.opinions.modify(a, b, 25);
                }
            }
        }
    }
}

/// Loop_Occupation_To_Economy: state occupied → recalc factory caches
fn loop_occupation_to_economy(
    world: &mut World,
    _econ: &mut EconomyState,
    _state: StateId,
    _owner: CountryId,
    _new_controller: CountryId,
) {
    // Recalculate country factory caches (owner loses, controller gains)
    world.recalc_country_caches();
}

/// Loop_Focus_To_Diplomacy: aggressive focus → tension+0.02, neighbour opinion-15
fn loop_focus_to_diplomacy(world: &mut World, country: CountryId, aggressive: bool) {
    if aggressive {
        world.diplomacy.world_tension = (world.diplomacy.world_tension + 2.0).min(100.0);
        // Reduce opinion with all neighbours (simplified: all other countries)
        for ci in 0..world.countries.count {
            let other = CountryId(ci as u16);
            if other != country {
                world.diplomacy.opinions.modify(other, country, -15);
            }
        }
    } else {
        // Peaceful focus: +5 opinion with neighbours
        for ci in 0..world.countries.count {
            let other = CountryId(ci as u16);
            if other != country {
                world.diplomacy.opinions.modify(other, country, 5);
            }
        }
    }
}

/// Loop_Research_To_Production: industry tech → recalc factory_output_factor
fn loop_research_to_production(world: &mut World, _country: CountryId, is_industry: bool) {
    if is_industry {
        // Trigger a recalc of country caches (factory output factor is part of this)
        world.recalc_country_caches();
    }
    // Equipment tech: AI candidate pool update happens naturally at next production eval
}
