use hoi4_logic::military::frontline::{self, MAX_FRONTLINES_PER_COUNTRY};
use hoi4_state::frontline::ArmyId;
use hoi4_state::{CountryId, ProvinceId, World};

use crate::frontline::{FrontLine, FrontSegment};
use crate::ground::{evaluate_front_sectors, FrontDecision, GroundPosture};
use crate::ground_orders;

pub fn tick_ai_frontlines(
    world: &mut World,
    country: CountryId,
    decisions: &[FrontDecision],
    front: &FrontLine,
) {
    let segments: Vec<(FrontSegment, GroundPosture)> = decisions
        .iter()
        .filter_map(|decision| {
            // 复用已计算的 FrontLine 段，避免同轮 compute_segment 冗余。
            let seg = front.segment_against(decision.enemy)?;
            if !seg.is_active() {
                return None;
            }
            Some((seg.clone(), decision.posture))
        })
        .collect();

    for (seg, posture) in &segments {
        let friendly_state_set: std::collections::HashSet<hoi4_state::ids::StateId> =
            seg.friendly_states.iter().copied().collect();

        let existing_idx = world.player_armies.iter().position(|a| {
            a.owner == country
                && a.order.as_ref().is_some_and(|o| {
                    o.path.iter().any(|p| {
                        let pi = p.0 as usize;
                        pi < world.provinces.count
                            && friendly_state_set.contains(&world.provinces.state_of[pi])
                    })
                })
        });

        let active_count = world
            .player_armies
            .iter()
            .filter(|a| a.owner == country && a.order.as_ref().is_some_and(|o| o.active))
            .count();

        let army_id = if let Some(idx) = existing_idx {
            let aid = world.player_armies[idx].id;
            let segment_members = collect_segment_divisions(world, country, seg);
            let missing_members: Vec<usize> = segment_members
                .into_iter()
                .filter(|di| !world.player_armies[idx].members.contains(di))
                .collect();
            if !missing_members.is_empty() {
                let _ = frontline::add_members(world, aid, &missing_members);
            }

            let has_arrow = world.player_armies[idx]
                .order
                .as_ref()
                .is_some_and(|o| o.arrow.is_some());
            let current_path_len = world.player_armies[idx]
                .order
                .as_ref()
                .map(|o| o.path.len())
                .unwrap_or(0);

            let path_state_coverage = world.player_armies[idx]
                .order
                .as_ref()
                .map(|o| {
                    o.path
                        .iter()
                        .filter(|p| {
                            let pi = p.0 as usize;
                            pi < world.provinces.count
                                && friendly_state_set.contains(&world.provinces.state_of[pi])
                        })
                        .count()
                })
                .unwrap_or(0);
            let needs_path_update = path_state_coverage == 0;

            if needs_path_update {
                let provs = friendly_states_to_provinces(world, seg);
                if !provs.is_empty() {
                    let _ = frontline::set_frontline_path(world, aid, &provs);
                }
            }

            match posture {
                GroundPosture::Attack => {
                    let provs = if needs_path_update {
                        friendly_states_to_provinces(world, seg)
                    } else if current_path_len > 0 {
                        world
                            .player_armies
                            .iter()
                            .find(|a| a.id == aid)
                            .and_then(|a| a.order.as_ref().map(|o| o.path.clone()))
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };
                    let targets = pick_sector_attack_targets(world, country, seg);
                    if !targets.is_empty() {
                        let first_target = targets[0];
                        let anchor = provs
                            .iter()
                            .filter(|&&p| {
                                let pi = p.0 as usize;
                                if pi >= world.map.adjacencies.len() {
                                    return false;
                                }
                                world.map.adjacencies[pi].contains(&first_target.0)
                            })
                            .copied()
                            .next()
                            .or_else(|| provs.first().copied())
                            .unwrap_or(ProvinceId(0));
                        let _ = frontline::set_arrow(world, aid, anchor, &targets);
                        let _ = frontline::execute_plan(world, aid);
                    } else if has_arrow {
                        let _ = frontline::halt_plan(world, aid);
                    }
                }
                GroundPosture::Defend | GroundPosture::Hold => {
                    if has_arrow {
                        let _ = frontline::halt_plan(world, aid);
                    }
                }
                GroundPosture::Retreat => {
                    let _ = frontline::clear_arrow(world, aid);
                }
            }
            aid
        } else if active_count < MAX_FRONTLINES_PER_COUNTRY {
            let members = collect_segment_divisions(world, country, seg);
            let name = format!("AI Army {}", world.next_army_id + 1);
            let army_id = match frontline::create_army(world, country, members, name) {
                Ok(id) => id,
                Err(_) => continue,
            };

            let provs = friendly_states_to_provinces(world, seg);
            if !provs.is_empty() {
                let _ = frontline::set_frontline_path(world, army_id, &provs);
            }

            if let GroundPosture::Attack = posture {
                let targets = pick_sector_attack_targets(world, country, seg);
                if !targets.is_empty() {
                    let first_target = targets[0];
                    let anchor = provs
                        .iter()
                        .filter(|&&p| {
                            let pi = p.0 as usize;
                            if pi >= world.map.adjacencies.len() {
                                return false;
                            }
                            world.map.adjacencies[pi].contains(&first_target.0)
                        })
                        .copied()
                        .next()
                        .or_else(|| provs.first().copied())
                        .unwrap_or(ProvinceId(0));
                    let _ = frontline::set_arrow(world, army_id, anchor, &targets);
                    let _ = frontline::execute_plan(world, army_id);
                }
            }
            army_id
        } else {
            continue;
        };

        let _ = army_id;
    }

    let friendly_state_sets: Vec<std::collections::HashSet<hoi4_state::ids::StateId>> = segments
        .iter()
        .map(|(seg, _)| seg.friendly_states.iter().copied().collect())
        .collect();

    let to_dissolve: Vec<ArmyId> = world
        .player_armies
        .iter()
        .filter(|a| a.owner == country)
        .filter(|a| {
            if a.members.is_empty() {
                return true;
            }
            let Some(o) = a.order.as_ref() else {
                return true;
            };
            if o.path.is_empty() {
                return true;
            }
            if !world.diplomacy.is_at_war(country) {
                return false;
            }
            !friendly_state_sets.iter().any(|fset| {
                o.path.iter().any(|p| {
                    let pi = p.0 as usize;
                    pi < world.provinces.count && fset.contains(&world.provinces.state_of[pi])
                })
            })
        })
        .map(|a| a.id)
        .collect();

    for id in to_dissolve {
        let _ = frontline::dissolve_army(world, id);
    }
}

fn pick_sector_attack_targets(
    world: &World,
    country: CountryId,
    seg: &FrontSegment,
) -> Vec<ProvinceId> {
    let mut targets = ground_orders::pick_attack_targets_export(world, country, seg);
    let sectors = evaluate_front_sectors(world, country, seg);
    if let Some(best) = sectors.first() {
        for &p in best.target_provinces.iter().rev() {
            if let Some(pos) = targets.iter().position(|&t| t == p) {
                let t = targets.remove(pos);
                targets.insert(0, t);
            } else {
                targets.insert(0, p);
            }
        }
    }
    targets.truncate(4);
    targets
}

fn collect_segment_divisions(world: &World, country: CountryId, seg: &FrontSegment) -> Vec<usize> {
    let mut members = Vec::new();
    for &sid in &seg.friendly_states {
        let si = sid.0 as usize;
        if si >= world.states.count {
            continue;
        }
        for &p in &world.states.provinces[si] {
            for &di in world.divisions_in_province(p) {
                if di < world.divisions.count
                    && world.divisions.owners[di] == country
                    && !members.contains(&di)
                {
                    members.push(di);
                }
            }
        }
    }
    members
}

fn friendly_states_to_provinces(world: &World, seg: &FrontSegment) -> Vec<ProvinceId> {
    let mut provs = Vec::new();
    for &sid in &seg.friendly_states {
        let si = sid.0 as usize;
        if si >= world.states.count {
            continue;
        }
        for &p in &world.states.provinces[si] {
            let pi = p.0 as usize;
            if pi >= world.provinces.count {
                continue;
            }
            if can_stage_from_province(world, seg.owner, p) {
                provs.push(p);
            }
        }
    }
    provs.dedup();
    provs
}

fn can_stage_from_province(world: &World, country: CountryId, p: ProvinceId) -> bool {
    let pi = p.0 as usize;
    if pi >= world.provinces.count {
        return false;
    }
    let controller = world.provinces.controllers[pi];
    if controller == country {
        return true;
    }
    if controller.is_none() {
        return false;
    }
    if world.diplomacy.has_military_access(country, controller) {
        return true;
    }
    world
        .diplomacy
        .faction_of(country)
        .is_some_and(|f| world.diplomacy.faction_of(controller) == Some(f))
}
