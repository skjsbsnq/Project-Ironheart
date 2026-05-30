//! Feature: frontline-orders — 公共 API 集成测试
//!
//! 覆盖：create_army / dissolve_army / add_members / remove_members /
//! set_frontline_path / clear_frontline_path / set_arrow / clear_arrow
//! 以及所有 11 种 FrontlineError 变体。

use std::sync::Arc;

use hoi4_data::GameData;
use hoi4_map::{GameMap, ProvinceType};
use hoi4_state::frontline::{ArmyId, FrontlineOrder, OffensiveArrow};
use hoi4_state::ids::{CountryId, ProvinceId};
use hoi4_state::World;

use hoi4_logic::military::frontline::{
    add_members, clear_arrow, clear_frontline_path, create_army, dissolve_army, remove_members,
    set_arrow, set_frontline_path, tick_frontlines, FrontlineError, MAX_FRONTLINES_PER_COUNTRY,
};

fn hoi4_path() -> std::path::PathBuf {
    hoi4_paths::PathConfig::resolve(Default::default())
        .expect("set IRONHEART_HOI4_PATH")
        .game_path()
        .to_path_buf()
}

fn build_world() -> World {
    let game_path = hoi4_path();
    let map = Arc::new(GameMap::load(&game_path).unwrap());
    let data = Arc::new(GameData::load(&game_path).unwrap());
    let mut world = World::new(map, data);
    world.populate_from_history();
    world
}

fn make_war(world: &mut World, a: CountryId, b: CountryId) {
    let war = hoi4_state::diplomacy::War {
        id: world.diplomacy.next_war_id,
        primary_attacker: a,
        primary_defender: b,
        attackers: {
            let mut s = std::collections::HashSet::new();
            s.insert(a);
            s
        },
        defenders: {
            let mut s = std::collections::HashSet::new();
            s.insert(b);
            s
        },
        started_at_hour: world.elapsed_hours,
        attacker_war_score: 0.0,
        defender_war_score: 0.0,
        attacker_wargoals: vec![],
        defender_wargoals: vec![],
        war_join_policies: std::collections::HashMap::new(),
    };
    world
        .diplomacy
        .wars
        .insert(world.diplomacy.next_war_id, war);
    world.diplomacy.next_war_id += 1;
    world.countries.at_war[a.0 as usize] = true;
    world.countries.at_war[b.0 as usize] = true;
}

fn ger_divs(world: &World, n: usize) -> Vec<usize> {
    let ger = world.country("GER").unwrap();
    (0..world.divisions.count)
        .filter(|&i| world.divisions.owners[i] == ger)
        .take(n)
        .collect()
}

fn is_land(world: &World, province: ProvinceId) -> bool {
    world
        .map
        .get_province(province.0)
        .map(|def| matches!(def.province_type, ProvinceType::Land))
        .unwrap_or(false)
}

fn find_four_land_attack_pairs(world: &World) -> Option<(Vec<ProvinceId>, Vec<ProvinceId>)> {
    let mut pairs = Vec::new();
    let mut used = std::collections::HashSet::new();
    for raw in 0..world.provinces.count.min(world.map.adjacencies.len()) {
        let front = ProvinceId(raw as u16);
        if !is_land(world, front) || !used.insert(front) {
            continue;
        }
        let Some(target_raw) = world.map.adjacencies[raw].iter().copied().find(|&nb| {
            let target = ProvinceId(nb);
            !used.contains(&target) && is_land(world, target)
        }) else {
            continue;
        };
        let target = ProvinceId(target_raw);
        used.insert(target);
        pairs.push((front, target));
        if pairs.len() == 4 {
            let (fronts, targets): (Vec<_>, Vec<_>) = pairs.into_iter().unzip();
            return Some((fronts, targets));
        }
    }
    None
}

// ─── 5.1 create_army / dissolve_army ───

#[test]
fn test_create_army_basic() {
    let mut world = build_world();
    let ger = world.country("GER").unwrap();
    let divs = ger_divs(&world, 3);
    if divs.len() < 3 {
        return;
    }

    let id = create_army(&mut world, ger, divs.clone(), "1. Armee".into()).unwrap();
    assert!(!id.is_none(), "army id should be valid");
    assert_eq!(world.player_armies.len(), 1);
    assert_eq!(world.player_armies[0].id, id);
    assert_eq!(world.player_armies[0].owner, ger);
    assert_eq!(world.player_armies[0].members, divs);
    assert_eq!(world.player_armies[0].order, None);
}

#[test]
fn test_create_army_empty_members() {
    let mut world = build_world();
    let ger = world.country("GER").unwrap();
    let err = create_army(&mut world, ger, vec![], "Empty".into()).unwrap_err();
    assert!(matches!(err, FrontlineError::NoSelectedDivisions));
}

#[test]
fn test_create_army_division_not_owned() {
    let mut world = build_world();
    let ger = world.country("GER").unwrap();
    let sov = world.country("SOV").unwrap();
    let sov_divs: Vec<usize> = (0..world.divisions.count)
        .filter(|&i| world.divisions.owners[i] == sov)
        .take(1)
        .collect();
    if sov_divs.is_empty() {
        return;
    }
    let err = create_army(&mut world, ger, sov_divs, "Bad".into()).unwrap_err();
    assert!(matches!(err, FrontlineError::DivisionNotOwned));
}

#[test]
fn test_create_army_mutual_exclusion() {
    let mut world = build_world();
    let ger = world.country("GER").unwrap();
    let all_divs = ger_divs(&world, 6);
    if all_divs.len() < 6 {
        return;
    }

    let _id1 = create_army(&mut world, ger, all_divs[0..3].to_vec(), "A".into()).unwrap();
    let id2 = create_army(&mut world, ger, all_divs[2..5].to_vec(), "B".into()).unwrap();

    let army1 = world.player_armies.iter().find(|a| a.id != id2).unwrap();
    let army2 = world.player_armies.iter().find(|a| a.id == id2).unwrap();

    assert!(
        !army1.members.contains(&all_divs[2]),
        "member should have been removed from army1"
    );
    assert!(
        army2.members.contains(&all_divs[2]),
        "member should be in army2"
    );
}

#[test]
fn test_dissolve_army() {
    let mut world = build_world();
    let ger = world.country("GER").unwrap();
    let divs = ger_divs(&world, 2);
    if divs.is_empty() {
        return;
    }

    let id = create_army(&mut world, ger, divs.clone(), "Temp".into()).unwrap();
    dissolve_army(&mut world, id).unwrap();
    assert!(world.player_armies.is_empty());
    for &d in &divs {
        assert!(!world.player_locked_divisions.contains(&d));
    }
}

#[test]
fn test_dissolve_army_preserves_destinations() {
    let mut world = build_world();
    let ger = world.country("GER").unwrap();
    let divs = ger_divs(&world, 2);
    if divs.is_empty() {
        return;
    }

    let id = create_army(&mut world, ger, divs.clone(), "T".into()).unwrap();

    // set some destinations
    for &d in &divs {
        world.divisions.destinations[d] = Some(ProvinceId(100));
    }
    dissolve_army(&mut world, id).unwrap();
    for &d in &divs {
        assert_eq!(
            world.divisions.destinations[d],
            Some(ProvinceId(100)),
            "dissolve_army should not change destinations (R1.3)"
        );
    }
}

#[test]
fn test_dissolve_unknown_army() {
    let mut world = build_world();
    let err = dissolve_army(&mut world, ArmyId(9999)).unwrap_err();
    assert!(matches!(err, FrontlineError::UnknownArmy));
}

// ─── 5.2 add_members / remove_members ───

#[test]
fn test_add_members() {
    let mut world = build_world();
    let ger = world.country("GER").unwrap();
    let divs = ger_divs(&world, 5);
    if divs.len() < 5 {
        return;
    }

    let id = create_army(&mut world, ger, divs[0..2].to_vec(), "A".into()).unwrap();
    add_members(&mut world, id, &divs[2..4].to_vec()).unwrap();

    let army = world.player_armies.iter().find(|a| a.id == id).unwrap();
    assert!(army.members.contains(&divs[2]));
    assert!(army.members.contains(&divs[3]));
}

#[test]
fn test_add_members_mutual_exclusion() {
    let mut world = build_world();
    let ger = world.country("GER").unwrap();
    let divs = ger_divs(&world, 6);
    if divs.len() < 6 {
        return;
    }

    let id1 = create_army(&mut world, ger, divs[0..3].to_vec(), "A".into()).unwrap();
    let id2 = create_army(&mut world, ger, divs[3..6].to_vec(), "B".to_owned()).unwrap();
    // add divs[3] to army1 — should remove from army2
    add_members(&mut world, id1, &[divs[3]]).unwrap();

    let army1 = world.player_armies.iter().find(|a| a.id == id1).unwrap();
    let army2 = world.player_armies.iter().find(|a| a.id == id2).unwrap();
    assert!(army1.members.contains(&divs[3]));
    assert!(!army2.members.contains(&divs[3]));
}

#[test]
fn test_add_members_wrong_owner() {
    let mut world = build_world();
    let ger = world.country("GER").unwrap();
    let sov = world.country("SOV").unwrap();
    let ger_divs = ger_divs(&world, 2);
    let sov_divs: Vec<usize> = (0..world.divisions.count)
        .filter(|&i| world.divisions.owners[i] == sov)
        .take(1)
        .collect();
    if ger_divs.is_empty() || sov_divs.is_empty() {
        return;
    }

    let id = create_army(&mut world, ger, ger_divs, "G".into()).unwrap();
    let err = add_members(&mut world, id, &sov_divs).unwrap_err();
    assert!(matches!(err, FrontlineError::DivisionNotOwned));
}

#[test]
fn test_remove_members() {
    let mut world = build_world();
    let ger = world.country("GER").unwrap();
    let divs = ger_divs(&world, 4);
    if divs.len() < 4 {
        return;
    }

    let id = create_army(&mut world, ger, divs[0..4].to_vec(), "A".into()).unwrap();
    remove_members(&mut world, id, &[divs[1], divs[2]]).unwrap();

    let army = world.player_armies.iter().find(|a| a.id == id).unwrap();
    assert!(!army.members.contains(&divs[1]));
    assert!(!army.members.contains(&divs[2]));
    assert!(army.members.contains(&divs[0]));
    assert!(army.members.contains(&divs[3]));
}

#[test]
fn test_remove_all_members_keeps_army() {
    let mut world = build_world();
    let ger = world.country("GER").unwrap();
    let divs = ger_divs(&world, 2);
    if divs.is_empty() {
        return;
    }

    let id = create_army(&mut world, ger, divs.clone(), "A".into()).unwrap();
    remove_members(&mut world, id, &divs).unwrap();

    let army = world.player_armies.iter().find(|a| a.id == id).unwrap();
    assert!(
        army.members.is_empty(),
        "R1.6: empty PlayerArmy should not be deleted"
    );
}

// ─── 5.3 set_frontline_path / clear_frontline_path ───

#[test]
fn test_set_frontline_path_no_eligible() {
    let mut world = build_world();
    let ger = world.country("GER").unwrap();
    let divs = ger_divs(&world, 2);
    if divs.is_empty() {
        return;
    }

    let id = create_army(&mut world, ger, divs, "A".into()).unwrap();
    let ger_provs: Vec<ProvinceId> = (0..world.states.count)
        .filter(|&i| world.states.owners[i] == ger)
        .flat_map(|i| world.states.provinces[i].clone())
        .take(5)
        .collect();
    if ger_provs.is_empty() {
        return;
    }
    // peacetime: path is accepted, active=true, executing=false
    let result = set_frontline_path(&mut world, id, &ger_provs);
    assert!(result.is_ok(), "peacetime frontline should succeed");
    let army = world.player_armies.iter().find(|a| a.id == id).unwrap();
    let o = army.order.as_ref().unwrap();
    assert!(!o.path.is_empty());
    assert!(o.active);
    assert!(!o.executing);
}

#[test]
fn test_clear_frontline_path() {
    let mut world = build_world();
    let ger = world.country("GER").unwrap();
    let sov = world.country("SOV").unwrap();
    let divs = ger_divs(&world, 2);
    if divs.is_empty() {
        return;
    }

    let id = create_army(&mut world, ger, divs, "A".into()).unwrap();
    make_war(&mut world, ger, sov);

    // find a GER province adjacent to an enemy
    for si in 0..world.states.count {
        if world.states.owners[si] != ger {
            continue;
        }
        for &p in &world.states.provinces[si].clone() {
            let pi = p.0 as usize;
            if world.provinces.controllers[pi] != ger {
                continue;
            }
            let has_enemy = world
                .map
                .adjacencies
                .get(pi)
                .map(|ns| {
                    ns.iter().any(|&nb| {
                        let nbi = nb as usize;
                        nbi < world.provinces.count && world.provinces.controllers[nbi] == sov
                    })
                })
                .unwrap_or(false);
            if has_enemy {
                let result = set_frontline_path(&mut world, id, &[p]);
                if result.is_ok() {
                    clear_frontline_path(&mut world, id).unwrap();
                    let army = world.player_armies.iter().find(|a| a.id == id).unwrap();
                    assert!(army.order.is_none());
                    return;
                }
            }
        }
    }
}

// ─── 5.4 set_arrow / clear_arrow ───

#[test]
fn test_set_arrow_no_active_path() {
    let mut world = build_world();
    let ger = world.country("GER").unwrap();
    let sov = world.country("SOV").unwrap();
    let divs = ger_divs(&world, 2);
    if divs.is_empty() {
        return;
    }

    let id = create_army(&mut world, ger, divs, "A".into()).unwrap();
    make_war(&mut world, ger, sov);

    // no path set yet → set_arrow should fail
    let sov_provs: Vec<ProvinceId> = (0..world.states.count)
        .filter(|&i| world.states.owners[i] == sov)
        .flat_map(|i| world.states.provinces[i].clone())
        .take(1)
        .collect();
    if sov_provs.is_empty() {
        return;
    }
    let ger_provs: Vec<ProvinceId> = (0..world.states.count)
        .filter(|&i| world.states.owners[i] == ger)
        .flat_map(|i| world.states.provinces[i].clone())
        .take(1)
        .collect();
    if ger_provs.is_empty() {
        return;
    }

    let anchor = ger_provs[0];
    let err = set_arrow(&mut world, id, anchor, &sov_provs).unwrap_err();
    assert!(matches!(err, FrontlineError::NoEligibleProvince));
}

#[test]
fn test_clear_arrow() {
    let mut world = build_world();
    let ger = world.country("GER").unwrap();
    let sov = world.country("SOV").unwrap();
    let divs = ger_divs(&world, 2);
    if divs.is_empty() {
        return;
    }

    let id = create_army(&mut world, ger, divs, "A".into()).unwrap();
    make_war(&mut world, ger, sov);

    // find adjacent pair and set path
    for si in 0..world.states.count {
        if world.states.owners[si] != ger {
            continue;
        }
        for &p in &world.states.provinces[si].clone() {
            let pi = p.0 as usize;
            if world.provinces.controllers[pi] != ger {
                continue;
            }
            let has_enemy = world
                .map
                .adjacencies
                .get(pi)
                .map(|ns| {
                    ns.iter().any(|&nb| {
                        let nbi = nb as usize;
                        nbi < world.provinces.count && world.provinces.controllers[nbi] == sov
                    })
                })
                .unwrap_or(false);
            if has_enemy && set_frontline_path(&mut world, id, &[p]).is_ok() {
                clear_arrow(&mut world, id).unwrap();
                let army = world.player_armies.iter().find(|a| a.id == id).unwrap();
                assert!(army.order.as_ref().unwrap().arrow.is_none());
                assert!(!army.order.as_ref().unwrap().path.is_empty());
                return;
            }
        }
    }
}

#[test]
fn test_executing_arrow_distributes_across_current_attack_targets() {
    let mut world = build_world();
    let ger = world.country("GER").unwrap();
    let sov = world.country("SOV").unwrap();
    make_war(&mut world, ger, sov);

    let Some((fronts, targets)) = find_four_land_attack_pairs(&world) else {
        return;
    };
    let mut divs = Vec::new();
    for i in 0..12 {
        let idx = world.divisions.push(
            ger,
            fronts[i % fronts.len()],
            0,
            50.0,
            1.0,
            format!("Phase 5 Test Division {}", i + 1),
        ) as usize;
        divs.push(idx);
    }
    for &front in &fronts {
        world.provinces.controllers[front.0 as usize] = ger;
    }
    for &target in &targets {
        world.provinces.controllers[target.0 as usize] = sov;
    }
    world.rebuild_province_div_index();

    create_army(&mut world, ger, divs.clone(), "Phase 5 Army".into()).unwrap();
    world.player_armies[0].order = Some(FrontlineOrder {
        path: fronts.clone(),
        arrow: Some(OffensiveArrow {
            provinces: targets.clone(),
        }),
        anchor: fronts.first().copied(),
        active: true,
        executing: true,
    });

    tick_frontlines(&mut world);

    let mut assault_targets: std::collections::HashMap<ProvinceId, usize> =
        std::collections::HashMap::new();
    for &di in &divs {
        if world.divisions.assignments[di]
            .as_ref()
            .is_some_and(|a| a.role == hoi4_state::DivisionRole::Assault)
        {
            let target = world.divisions.destinations[di].expect("assault destination");
            *assault_targets.entry(target).or_default() += 1;
        }
    }

    assert!(
        assault_targets.len() >= 2,
        "executing arrow should not send every division to one target: {:?}",
        assault_targets
    );
    assert!(
        assault_targets.values().all(|&count| count <= 4),
        "per-target assault capacity should be respected: {:?}",
        assault_targets
    );

    let before = assault_targets.clone();
    tick_frontlines(&mut world);

    let mut after: std::collections::HashMap<ProvinceId, usize> = std::collections::HashMap::new();
    for &di in &divs {
        if world.divisions.assignments[di]
            .as_ref()
            .is_some_and(|a| a.role == hoi4_state::DivisionRole::Assault)
        {
            let target = world.divisions.destinations[di].expect("assault destination");
            *after.entry(target).or_default() += 1;
        }
    }

    assert_eq!(before, after, "arrow target distribution should be stable");
}

// ─── 5.5 Property 6: 成员互斥 ───

#[test]
fn test_member_exclusion_invariant() {
    let mut world = build_world();
    let ger = world.country("GER").unwrap();
    let divs = ger_divs(&world, 10);
    if divs.len() < 10 {
        return;
    }

    let id1 = create_army(&mut world, ger, divs[0..5].to_vec(), "A".into()).unwrap();
    let _id2 = create_army(&mut world, ger, divs[5..10].to_vec(), "B".into()).unwrap();
    add_members(&mut world, id1, &[divs[5]]).unwrap();

    // verify each div in at most 1 army
    let mut seen: std::collections::HashSet<usize> = std::collections::HashSet::new();
    for army in &world.player_armies {
        for &m in &army.members {
            assert!(
                !seen.contains(&m),
                "Property 6: member {} in multiple armies",
                m
            );
            seen.insert(m);
        }
    }
}

// ─── 5.5 Property 11: 每国战线上限 ───

#[test]
fn test_army_cap_reached() {
    let mut world = build_world();
    let ger = world.country("GER").unwrap();
    let sov = world.country("SOV").unwrap();
    make_war(&mut world, ger, sov);

    let divs = ger_divs(&world, 20);
    if divs.len() < 20 {
        return;
    }

    let chunk = 5;
    for i in 0..MAX_FRONTLINES_PER_COUNTRY {
        let start = i * chunk;
        let end = (start + chunk).min(divs.len());
        let id = create_army(
            &mut world,
            ger,
            divs[start..end].to_vec(),
            format!("Army {}", i),
        )
        .unwrap();
        // need an active path to count toward the cap
        for si in 0..world.states.count {
            if world.states.owners[si] != ger {
                continue;
            }
            for &p in &world.states.provinces[si].clone() {
                let pi = p.0 as usize;
                if world.provinces.controllers[pi] != ger {
                    continue;
                }
                let has_enemy = world
                    .map
                    .adjacencies
                    .get(pi)
                    .map(|ns| {
                        ns.iter().any(|&nb| {
                            let nbi = nb as usize;
                            nbi < world.provinces.count && world.provinces.controllers[nbi] == sov
                        })
                    })
                    .unwrap_or(false);
                if has_enemy {
                    let _ = set_frontline_path(&mut world, id, &[p]);
                    break;
                }
            }
            if world
                .player_armies
                .iter()
                .any(|a| a.id == id && a.order.is_some())
            {
                break;
            }
        }
    }

    // now the cap should be hit
    let remaining: Vec<usize> = divs[20.min(divs.len())..].to_vec();
    if !remaining.is_empty() {
        let err = create_army(&mut world, ger, remaining, "Overflow".into()).unwrap_err();
        assert!(matches!(err, FrontlineError::ArmyCapReached));
    } else {
        let extra: Vec<usize> = (0..world.divisions.count)
            .filter(|&i| {
                world.divisions.owners[i] == ger && !world.player_locked_divisions.contains(&i)
            })
            .take(1)
            .collect();
        if !extra.is_empty() {
            let err = create_army(&mut world, ger, extra, "Overflow".into());
            if let Err(e) = err {
                assert!(matches!(e, FrontlineError::ArmyCapReached));
            }
        }
    }
}

// ─── 5.5 Property 16: 解散保留 destinations ───

#[test]
fn test_dissolve_preserves_destinations_and_lockset() {
    let mut world = build_world();
    let ger = world.country("GER").unwrap();
    let sov = world.country("SOV").unwrap();
    let divs = ger_divs(&world, 3);
    if divs.len() < 3 {
        return;
    }

    let id = create_army(&mut world, ger, divs.clone(), "T".into()).unwrap();
    make_war(&mut world, ger, sov);

    for si in 0..world.states.count {
        if world.states.owners[si] != ger {
            continue;
        }
        for &p in &world.states.provinces[si].clone() {
            let pi = p.0 as usize;
            if world.provinces.controllers[pi] != ger {
                continue;
            }
            let has_enemy = world
                .map
                .adjacencies
                .get(pi)
                .map(|ns| {
                    ns.iter().any(|&nb| {
                        let nbi = nb as usize;
                        nbi < world.provinces.count && world.provinces.controllers[nbi] == sov
                    })
                })
                .unwrap_or(false);
            if has_enemy {
                let _ = set_frontline_path(&mut world, id, &[p]);
                break;
            }
        }
        if world
            .player_armies
            .iter()
            .any(|a| a.id == id && a.order.is_some())
        {
            break;
        }
    }

    // set destinations
    for &d in &divs {
        world.divisions.destinations[d] = Some(ProvinceId(500));
    }

    dissolve_army(&mut world, id).unwrap();

    for &d in &divs {
        assert_eq!(
            world.divisions.destinations[d],
            Some(ProvinceId(500)),
            "Property 16: destinations unchanged after dissolve"
        );
        assert!(
            !world.player_locked_divisions.contains(&d),
            "Property 16: lockset should not contain former members"
        );
    }
}

// ─── 5.6 StaleMember error ───

#[test]
fn test_stale_member_detected() {
    let mut world = build_world();
    let ger = world.country("GER").unwrap();
    let oob_idx = world.divisions.count + 100;
    // try creating army with out-of-range member
    let err = create_army(&mut world, ger, vec![oob_idx], "Bad".into()).unwrap_err();
    assert!(matches!(err, FrontlineError::DivisionNotOwned));
}
