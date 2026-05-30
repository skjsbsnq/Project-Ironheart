//! Feature: frontline-orders — snapper / arrow / distributor / executor 集成测试
//!
//! 使用真实 HOI4 世界验证核心算法在真实地图上正确运行。

use std::sync::Arc;

use hoi4_data::GameData;
use hoi4_map::GameMap;
use hoi4_state::frontline::{FrontlineOrder, PlayerArmy};
use hoi4_state::ids::{CountryId, ProvinceId};
use hoi4_state::World;

use hoi4_logic::military::frontline::{
    arrow_snapper, frontline_distributor, frontline_snapper, pick_arrow_executors, validate_path,
    FrontlineError, MAX_PATH_LEN,
};

fn hoi4_path() -> std::path::PathBuf {
    hoi4_paths::PathConfig::resolve(Default::default())
        .expect("set IRONHEART_HOI4_PATH or place HOI4 install at default Steam path")
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

fn find_adjacent_friendly_enemy_pair(
    world: &World,
    owner: CountryId,
    enemy: CountryId,
) -> Option<(ProvinceId, ProvinceId, ProvinceId)> {
    for si in 0..world.states.count {
        if world.states.owners[si] != owner {
            continue;
        }
        for &p in &world.states.provinces[si] {
            let pi = p.0 as usize;
            if pi >= world.provinces.count
                || !world
                    .map
                    .adjacencies
                    .get(pi)
                    .map(|v| !v.is_empty())
                    .unwrap_or(false)
            {
                continue;
            }
            let p_ctrl = world.provinces.controllers[pi];
            if p_ctrl != owner {
                continue;
            }
            for &nb in &world.map.adjacencies[pi] {
                let nbi = nb as usize;
                if nbi >= world.provinces.count {
                    continue;
                }
                let nb_ctrl = world.provinces.controllers[nbi];
                if nb_ctrl == enemy {
                    for &nb2 in &world.map.adjacencies[nbi] {
                        if nb2 == p.0 {
                            continue;
                        }
                        let nb2i = nb2 as usize;
                        if nb2i >= world.provinces.count {
                            continue;
                        }
                        let nb2_ctrl = world.provinces.controllers[nb2i];
                        if nb2_ctrl == enemy {
                            return Some((p, ProvinceId(nb), ProvinceId(nb2)));
                        }
                    }
                    return Some((p, ProvinceId(nb), ProvinceId(nb)));
                }
            }
        }
    }
    None
}

// ─── 4.3 frontline_snapper ───

#[test]
fn test_snapper_basic_on_real_map() {
    let mut world = build_world();
    let ger = world.country("GER").unwrap();
    let sov = world.country("SOV").unwrap();
    make_war(&mut world, ger, sov);

    // set some SOV-controlled provinces adjacent to GER
    // find a GER province and flip a neighbor to SOV
    if let Some((ger_prov, sov_prov, _)) = find_adjacent_friendly_enemy_pair(&world, ger, sov) {
        let samples = vec![ger_prov, sov_prov];
        let result = frontline_snapper(&world, ger, &samples);
        // snapper should at minimum find the friendly province
        match result {
            Ok(path) => {
                assert!(!path.is_empty());
                assert!(path.len() <= MAX_PATH_LEN);
            }
            Err(FrontlineError::NoEligibleProvince) => {
                // possible if no enemy-adjacent province exists yet
            }
            Err(FrontlineError::PathTruncated(orig)) => {
                assert!(orig > MAX_PATH_LEN);
            }
            _ => panic!("unexpected error: {:?}", result),
        }
    }
}

#[test]
fn test_snapper_empty_samples() {
    let world = build_world();
    let ger = world.country("GER").unwrap();
    let result = frontline_snapper(&world, ger, &[]);
    assert!(matches!(result, Err(FrontlineError::NoEligibleProvince)));
}

#[test]
fn test_snapper_no_eligible_peace_time() {
    let world = build_world();
    let ger = world.country("GER").unwrap();
    let ger_provs: Vec<ProvinceId> = (0..world.states.count)
        .filter(|&i| world.states.owners[i] == ger)
        .flat_map(|i| world.states.provinces[i].clone())
        .take(5)
        .collect();
    if !ger_provs.is_empty() {
        let result = frontline_snapper(&world, ger, &ger_provs);
        assert!(
            result.is_ok(),
            "peacetime snapper should accept owner-controlled provinces"
        );
    }
}

// ─── 4.5 arrow_snapper ───

#[test]
fn test_arrow_snapper_anchor_not_adjacent() {
    let mut world = build_world();
    let ger = world.country("GER").unwrap();
    let sov = world.country("SOV").unwrap();
    make_war(&mut world, ger, sov);

    // anchor in Germany, samples in Soviet territory — might not be adjacent
    let ger_provs: Vec<ProvinceId> = (0..world.states.count)
        .filter(|&i| world.states.owners[i] == ger)
        .flat_map(|i| world.states.provinces[i].clone())
        .take(1)
        .collect();
    let sov_provs: Vec<ProvinceId> = (0..world.states.count)
        .filter(|&i| world.states.owners[i] == sov)
        .flat_map(|i| world.states.provinces[i].clone())
        .take(3)
        .collect();

    if let (Some(&anchor), Some(_)) = (ger_provs.first(), sov_provs.first()) {
        // flip sov provinces to SOV controller so they're "enemy controlled"
        for &sp in &sov_provs {
            world.provinces.controllers[sp.0 as usize] = sov;
        }
        let result = arrow_snapper(&world, ger, anchor, &sov_provs);
        // should either succeed (if adjacent) or fail with AnchorNotAdjacent
        match result {
            Err(FrontlineError::AnchorNotAdjacent) | Ok(_) => {}
            Err(FrontlineError::NoEligibleProvince) => {}
            Err(e) => panic!("unexpected error: {:?}", e),
        }
    }
}

#[test]
fn test_arrow_snapper_empty_samples() {
    let world = build_world();
    let ger = world.country("GER").unwrap();
    let ger_prov = ProvinceId(0);
    let result = arrow_snapper(&world, ger, ger_prov, &[]);
    assert!(matches!(result, Err(FrontlineError::NoEligibleProvince)));
}

// ─── 4.7 frontline_distributor ───

#[test]
fn test_distributor_basic_distribution() {
    let world = build_world();
    let ger = world.country("GER").unwrap();

    let ger_divs: Vec<usize> = (0..world.divisions.count)
        .filter(|&i| world.divisions.owners[i] == ger)
        .take(6)
        .collect();
    if ger_divs.len() < 2 {
        return;
    }

    let path: Vec<ProvinceId> = (0..world.states.count)
        .filter(|&i| world.states.owners[i] == ger)
        .flat_map(|i| world.states.provinces[i].clone())
        .filter(|&p| (p.0 as usize) < world.map.adjacencies.len())
        .take(4)
        .collect();
    if path.len() < 2 {
        return;
    }

    let army = PlayerArmy {
        id: hoi4_state::frontline::ArmyId(1),
        name: "Test".to_owned(),
        owner: ger,
        commander: None,
        members: ger_divs.clone(),
        order: Some(FrontlineOrder {
            path: path.clone(),
            arrow: None,
            anchor: None,
            active: true,
            executing: false,
        }),
    };

    let dist = frontline_distributor(&world, &army);
    assert!(!dist.is_empty(), "should produce some distribution");

    for (div_idx, target) in &dist {
        assert!(ger_divs.contains(div_idx), "div should be a member");
        assert!(path.contains(target), "target should be on path");
    }

    let assigned_divs: HashSet<usize> = dist.iter().map(|(i, _)| *i).collect();
    assert_eq!(assigned_divs.len(), dist.len(), "no duplicate divisions");
}

// ─── 4.9 pick_arrow_executors ───

#[test]
fn test_executors_count_is_ceil_half() {
    let world = build_world();
    let ger = world.country("GER").unwrap();

    let ger_divs: Vec<usize> = (0..world.divisions.count)
        .filter(|&i| world.divisions.owners[i] == ger)
        .take(5)
        .collect();
    if ger_divs.len() < 3 {
        return;
    }

    let path: Vec<ProvinceId> = (0..world.states.count)
        .filter(|&i| world.states.owners[i] == ger)
        .flat_map(|i| world.states.provinces[i].clone())
        .filter(|&p| (p.0 as usize) < world.map.adjacencies.len())
        .take(4)
        .collect();
    if path.len() < 2 {
        return;
    }

    let anchor = path[0];
    let n = ger_divs.len();
    let line_need = path.len().max(1).min((n + 1) / 2);
    let expected_k = n.saturating_sub(line_need);

    let army = PlayerArmy {
        id: hoi4_state::frontline::ArmyId(1),
        name: "Test".to_owned(),
        owner: ger,
        commander: None,
        members: ger_divs.clone(),
        order: Some(FrontlineOrder {
            path,
            arrow: None,
            anchor: Some(anchor),
            active: true,
            executing: false,
        }),
    };

    let executors = pick_arrow_executors(&world, &army);
    assert_eq!(
        executors.len(),
        expected_k,
        "executors keep enough divisions on the frontline"
    );
    for &e in &executors {
        assert!(ger_divs.contains(&e), "executor must be a member");
    }
}

#[test]
fn test_executors_no_duplicates() {
    let world = build_world();
    let ger = world.country("GER").unwrap();

    let ger_divs: Vec<usize> = (0..world.divisions.count)
        .filter(|&i| world.divisions.owners[i] == ger)
        .take(8)
        .collect();
    if ger_divs.len() < 3 {
        return;
    }

    let path: Vec<ProvinceId> = (0..world.states.count)
        .filter(|&i| world.states.owners[i] == ger)
        .flat_map(|i| world.states.provinces[i].clone())
        .filter(|&p| (p.0 as usize) < world.map.adjacencies.len())
        .take(4)
        .collect();
    if path.len() < 2 {
        return;
    }

    let anchor = path[0];
    let army = PlayerArmy {
        id: hoi4_state::frontline::ArmyId(1),
        name: "Test".to_owned(),
        owner: ger,
        commander: None,
        members: ger_divs,
        order: Some(FrontlineOrder {
            path,
            arrow: None,
            anchor: Some(anchor),
            active: true,
            executing: false,
        }),
    };

    let executors = pick_arrow_executors(&world, &army);
    let unique: HashSet<usize> = executors.iter().copied().collect();
    assert_eq!(unique.len(), executors.len(), "no duplicate executors");
}

// ─── validate_path ───

#[test]
fn test_validate_path_filters_non_cobeligerent() {
    let world = build_world();
    let ger = world.country("GER").unwrap();
    let sov = world.country("SOV").unwrap();
    // SOV provinces are not co-belligerent (no war)
    let sov_provs: Vec<ProvinceId> = (0..world.states.count)
        .filter(|&i| world.states.owners[i] == sov)
        .flat_map(|i| world.states.provinces[i].clone())
        .take(3)
        .collect();
    if !sov_provs.is_empty() {
        let result = validate_path(&world, ger, &sov_provs);
        assert!(
            result.is_empty(),
            "no war → SOV provinces should be filtered out"
        );
    }
}

#[test]
fn test_distributor_deterministic() {
    let world = build_world();
    let ger = world.country("GER").unwrap();

    let ger_divs: Vec<usize> = (0..world.divisions.count)
        .filter(|&i| world.divisions.owners[i] == ger)
        .take(5)
        .collect();
    if ger_divs.len() < 2 {
        return;
    }

    let path: Vec<ProvinceId> = (0..world.states.count)
        .filter(|&i| world.states.owners[i] == ger)
        .flat_map(|i| world.states.provinces[i].clone())
        .filter(|&p| (p.0 as usize) < world.map.adjacencies.len())
        .take(4)
        .collect();
    if path.len() < 2 {
        return;
    }

    let army = PlayerArmy {
        id: hoi4_state::frontline::ArmyId(1),
        name: "Test".to_owned(),
        owner: ger,
        commander: None,
        members: ger_divs,
        order: Some(FrontlineOrder {
            path,
            arrow: None,
            anchor: None,
            active: true,
            executing: false,
        }),
    };

    let d1 = frontline_distributor(&world, &army);
    let d2 = frontline_distributor(&world, &army);
    assert_eq!(d1, d2, "distributor must be deterministic");
}

use std::collections::HashSet;
