//! P0.13：统一师命令执行层。
//!
//! 核心原则：
//! - AI 只输出意图（DivisionCommand），不直接写 `destinations`。
//! - 玩家前线 tick 也输出命令，不直接写 `destinations`。
//! - 系统撤退输出命令。
//! - 只有本模块的 `tick_division_commands_daily` 才写最终 `destinations`。
//!
//! 裁决规则：
//! - 同一师同一天只由最高优先级命令写最终目的地。
//! - 过期命令被清除。
//! - 手动命令 > 登陆计划 > 海外增援 > 地面命令 > 前线命令 > 撤退。

use hoi4_state::{CommandSource, DivisionCommand, DivisionIntent, ProvinceId, World};

use crate::military::movement;

/// 向指定师提交命令。若已有更高或同优先级命令，则不覆盖。
/// 返回 true 表示命令已写入。
pub fn issue_division_command(world: &mut World, div_idx: usize, command: DivisionCommand) -> bool {
    if div_idx >= world.divisions.count {
        return false;
    }
    let owner = world.divisions.owners[div_idx];
    if owner.is_none() {
        return false;
    }
    if !movement::can_enter_province(world, owner, command.target) {
        return false;
    }
    let new_priority = command.source.priority();
    let should_replace = match world.divisions.commands[div_idx].as_ref() {
        None => true,
        Some(existing) => {
            let existing_priority = existing.source.priority();
            if new_priority > existing_priority {
                true
            } else if new_priority == existing_priority {
                true
            } else {
                false
            }
        }
    };
    if should_replace {
        world.divisions.commands[div_idx] = Some(command);
        true
    } else {
        false
    }
}

/// 清除指定师的命令。
pub fn clear_division_command(world: &mut World, div_idx: usize) {
    if div_idx < world.divisions.count {
        world.divisions.commands[div_idx] = None;
    }
}

/// 统一命令执行层：在 military daily 开头、movement tick 之前调用。
///
/// 对每个师：
/// 1. 清除过期命令。
/// 2. 若有有效命令，按优先级裁决写入 `destinations`。
/// 3. 若无命令且无 transport，保持现有 destination 不变（movement tick 自己处理）。
pub fn tick_division_commands_daily(world: &mut World) {
    let now = world.elapsed_hours;

    for di in 0..world.divisions.count {
        let owner = world.divisions.owners[di];
        if owner.is_none() {
            continue;
        }

        // 清除过期命令
        let expired = match world.divisions.commands[di].as_ref() {
            Some(cmd) => cmd.expires_at_hour <= now,
            None => false,
        };
        if expired {
            world.divisions.commands[di] = None;
        }

        let Some(cmd) = world.divisions.commands[di].take() else {
            continue;
        };

        // 运输中的师不受命令影响
        if world.divisions.transport[di].is_some() {
            world.divisions.commands[di] = None;
            continue;
        }

        // 濒死师不执行进攻类命令
        let max_org = world.divisions.max_organisation[di].max(1e-6);
        let org_ratio = world.divisions.organisation[di] / max_org;
        let str_now = world.divisions.strength[di];
        let is_broken = org_ratio < 0.05 || str_now < 0.05;
        if is_broken
            && !matches!(
                cmd.intent,
                DivisionIntent::Retreat | DivisionIntent::Reserve
            )
        {
            world.divisions.commands[di] = None;
            continue;
        }

        let target = cmd.target;
        let can_enter = movement::can_enter_province(world, owner, target);
        if !can_enter {
            world.divisions.commands[di] = None;
            continue;
        }

        // 写入 assignment；玩家手动命令传 None，用于清除旧前线分配。
        world.divisions.assignments[di] = cmd.assignment.clone();
        // 写入最终 destination
        let cur = world.divisions.locations[di];
        let keep_command = matches!(cmd.source, CommandSource::PlayerManual) && cur != target;
        if cur == target {
            world.divisions.destinations[di] = None;
        } else {
            world.divisions.destinations[di] = Some(target);
        }

        // 普通命令在这里消费；玩家手动命令到达前持续占用最高优先级。
        world.divisions.commands[di] = keep_command.then_some(cmd);
    }
}

/// 为 AI ground_orders 提供的便捷函数：提交一个守备/进攻/清剿/后备命令。
pub fn issue_ai_ground_command(
    world: &mut World,
    div_idx: usize,
    intent: DivisionIntent,
    target: ProvinceId,
    assignment: hoi4_state::DivisionAssignment,
) -> bool {
    let expires_at = world.elapsed_hours + 48;
    issue_division_command(
        world,
        div_idx,
        DivisionCommand {
            intent,
            source: CommandSource::AiGroundOrders,
            target,
            expires_at_hour: expires_at,
            assignment: Some(assignment),
        },
    )
}

/// 为 AI 前线提供：提交前线命令。
pub fn issue_ai_frontline_command(
    world: &mut World,
    div_idx: usize,
    intent: DivisionIntent,
    target: ProvinceId,
    assignment: hoi4_state::DivisionAssignment,
) -> bool {
    let expires_at = world.elapsed_hours + 48;
    issue_division_command(
        world,
        div_idx,
        DivisionCommand {
            intent,
            source: CommandSource::AiFrontline,
            target,
            expires_at_hour: expires_at,
            assignment: Some(assignment),
        },
    )
}

/// 为玩家前线 tick 提供：提交前线命令。
pub fn issue_player_frontline_command(
    world: &mut World,
    div_idx: usize,
    intent: DivisionIntent,
    target: ProvinceId,
    assignment: hoi4_state::DivisionAssignment,
) -> bool {
    let expires_at = world.elapsed_hours + 72;
    issue_division_command(
        world,
        div_idx,
        DivisionCommand {
            intent,
            source: CommandSource::PlayerFrontline,
            target,
            expires_at_hour: expires_at,
            assignment: Some(assignment),
        },
    )
}

/// 为系统撤退提供：提交撤退命令。
pub fn issue_retreat_command(world: &mut World, div_idx: usize, target: ProvinceId) -> bool {
    let expires_at = world.elapsed_hours + 72;
    issue_division_command(
        world,
        div_idx,
        DivisionCommand {
            intent: DivisionIntent::Retreat,
            source: CommandSource::SystemRetreat,
            target,
            expires_at_hour: expires_at,
            assignment: None,
        },
    )
}

/// 为登陆计划提供：提交登陆命令（高优先级）。
pub fn issue_invasion_command(
    world: &mut World,
    div_idx: usize,
    intent: DivisionIntent,
    target: ProvinceId,
    assignment: hoi4_state::DivisionAssignment,
) -> bool {
    let expires_at = world.elapsed_hours + 240;
    issue_division_command(
        world,
        div_idx,
        DivisionCommand {
            intent,
            source: CommandSource::AiInvasionPlan,
            target,
            expires_at_hour: expires_at,
            assignment: Some(assignment),
        },
    )
}

/// 为海外增援提供：提交运输命令（高优先级）。
pub fn issue_overseas_reinforce_command(
    world: &mut World,
    div_idx: usize,
    target: ProvinceId,
    assignment: hoi4_state::DivisionAssignment,
) -> bool {
    let expires_at = world.elapsed_hours + 168;
    issue_division_command(
        world,
        div_idx,
        DivisionCommand {
            intent: DivisionIntent::OverseasTransport,
            source: CommandSource::AiOverseasReinforce,
            target,
            expires_at_hour: expires_at,
            assignment: Some(assignment),
        },
    )
}

/// 为玩家手动移动提供：最高优先级命令。
pub fn issue_player_manual_command(world: &mut World, div_idx: usize, target: ProvinceId) -> bool {
    let expires_at = u64::MAX;
    issue_division_command(
        world,
        div_idx,
        DivisionCommand {
            intent: DivisionIntent::Garrison,
            source: CommandSource::PlayerManual,
            target,
            expires_at_hour: expires_at,
            assignment: None,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use hoi4_data::{Color, Country, CountryTag, GameData, State};
    use hoi4_map::{
        GameMap, Heightmap, ProvinceDefinition, ProvinceMap, ProvinceType, TerrainBitmap,
        TerrainCatalog,
    };
    use hoi4_state::diplomacy::War;
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;

    fn synthetic_world() -> World {
        let definitions = vec![
            None,
            Some(ProvinceDefinition {
                id: 1,
                r: 1,
                g: 0,
                b: 0,
                province_type: ProvinceType::Land,
                coastal: false,
                terrain: "plains".to_owned(),
                continent: 1,
            }),
            Some(ProvinceDefinition {
                id: 2,
                r: 2,
                g: 0,
                b: 0,
                province_type: ProvinceType::Land,
                coastal: false,
                terrain: "plains".to_owned(),
                continent: 1,
            }),
            Some(ProvinceDefinition {
                id: 3,
                r: 3,
                g: 0,
                b: 0,
                province_type: ProvinceType::Land,
                coastal: false,
                terrain: "plains".to_owned(),
                continent: 1,
            }),
        ];
        let map = Arc::new(GameMap {
            definitions,
            rgb_to_id: HashMap::new(),
            province_map: ProvinceMap {
                width: 1,
                height: 1,
                pixels: vec![1],
            },
            adjacencies: vec![Vec::new(), vec![2], vec![1, 3], vec![2]],
            special_adjacencies: Vec::new(),
            heightmap: Heightmap {
                width: 1,
                height: 1,
                pixels: vec![0],
            },
            terrain_bmp: TerrainBitmap {
                width: 1,
                height: 1,
                pixels: vec![0],
                palette: [[0; 3]; 256],
            },
            terrain_catalog: TerrainCatalog::default(),
            tree_definition_bmp: None,
            tree_indices: Default::default(),
        });

        let mut data = GameData::default();
        let ger = CountryTag::new("GER");
        let aus = CountryTag::new("AUS");
        for tag in [ger.clone(), aus.clone()] {
            data.countries.insert(
                tag.clone(),
                Country {
                    tag,
                    color: Color { r: 1, g: 1, b: 1 },
                    graphical_culture: "western_european_gfx".to_owned(),
                    capital: 1,
                    ruling_party: "neutrality".to_owned(),
                    technologies: Vec::new(),
                },
            );
        }
        data.states.push(State {
            id: 1,
            name: "GER State".to_owned(),
            manpower: 1000,
            owner: ger.clone(),
            cores: vec![ger],
            provinces: vec![1],
            category: "city".to_owned(),
            infrastructure: 0,
            victory_points: Vec::new(),
            resources: Vec::new(),
        });
        data.states.push(State {
            id: 2,
            name: "AUS State".to_owned(),
            manpower: 1000,
            owner: aus.clone(),
            cores: vec![aus],
            provinces: vec![2, 3],
            category: "city".to_owned(),
            infrastructure: 0,
            victory_points: Vec::new(),
            resources: Vec::new(),
        });

        World::new(map, Arc::new(data))
    }

    fn make_war(world: &mut World, a: hoi4_state::CountryId, b: hoi4_state::CountryId) {
        let war = War {
            id: world.diplomacy.next_war_id,
            primary_attacker: a,
            primary_defender: b,
            attackers: HashSet::from([a]),
            defenders: HashSet::from([b]),
            started_at_hour: world.elapsed_hours,
            attacker_war_score: 0.0,
            defender_war_score: 0.0,
            attacker_wargoals: vec![],
            defender_wargoals: vec![],
            war_join_policies: HashMap::new(),
        };
        world
            .diplomacy
            .wars
            .insert(world.diplomacy.next_war_id, war);
        world.diplomacy.next_war_id += 1;
        world.countries.at_war[a.0 as usize] = true;
        world.countries.at_war[b.0 as usize] = true;
    }

    #[test]
    fn player_manual_command_persists_until_arrival_and_blocks_ai_reorder() {
        let mut world = synthetic_world();
        let ger = world.country("GER").unwrap();
        let aus = world.country("AUS").unwrap();
        make_war(&mut world, ger, aus);
        world.provinces.owners[2] = ger;
        world.provinces.controllers[2] = ger;
        world.provinces.owners[3] = ger;
        world.provinces.controllers[3] = ger;

        let div = world
            .divisions
            .push(ger, ProvinceId(1), 0, 10.0, 1.0, "test".to_owned()) as usize;

        assert!(issue_player_manual_command(&mut world, div, ProvinceId(3)));
        assert_eq!(
            world.divisions.commands[div]
                .as_ref()
                .map(|cmd| cmd.expires_at_hour),
            Some(u64::MAX)
        );

        tick_division_commands_daily(&mut world);
        assert_eq!(world.divisions.destinations[div], Some(ProvinceId(3)));
        assert!(matches!(
            world.divisions.commands[div].as_ref().map(|cmd| cmd.source),
            Some(CommandSource::PlayerManual)
        ));

        let ai_assignment = hoi4_state::DivisionAssignment {
            front: Some(aus),
            role: hoi4_state::DivisionRole::Garrison,
            target: Some(ProvinceId(1)),
            assigned_at: world.elapsed_hours,
        };
        assert!(!issue_ai_ground_command(
            &mut world,
            div,
            DivisionIntent::Garrison,
            ProvinceId(1),
            ai_assignment,
        ));

        crate::military::movement::daily_movement_tick(&mut world);
        assert_eq!(world.divisions.locations[div], ProvinceId(2));
        assert_eq!(world.divisions.destinations[div], Some(ProvinceId(3)));

        world.elapsed_hours += 24;
        tick_division_commands_daily(&mut world);
        crate::military::movement::daily_movement_tick(&mut world);
        assert_eq!(world.divisions.locations[div], ProvinceId(3));
        assert_eq!(world.divisions.destinations[div], None);
        assert!(matches!(
            world.divisions.commands[div].as_ref().map(|cmd| cmd.source),
            Some(CommandSource::PlayerManual)
        ));

        world.elapsed_hours += 24;
        tick_division_commands_daily(&mut world);
        assert!(world.divisions.commands[div].is_none());
    }

    #[test]
    fn 登陆命令不被普通前线推进覆盖() {
        let mut world = synthetic_world();
        let ger = world.country("GER").unwrap();
        let aus = world.country("AUS").unwrap();
        make_war(&mut world, ger, aus);

        let div = world
            .divisions
            .push(ger, ProvinceId(1), 0, 10.0, 1.0, "test".to_owned()) as usize;

        // 登陆命令
        let now = world.elapsed_hours;
        let ok = issue_invasion_command(
            &mut world,
            div,
            DivisionIntent::NavalInvasion,
            ProvinceId(2),
            hoi4_state::DivisionAssignment {
                front: Some(aus),
                role: hoi4_state::DivisionRole::Assault,
                target: Some(ProvinceId(2)),
                assigned_at: now,
            },
        );
        assert!(ok, "登陆命令应成功");

        // 前线命令尝试覆盖
        let ok = issue_ai_frontline_command(
            &mut world,
            div,
            DivisionIntent::Garrison,
            ProvinceId(1),
            hoi4_state::DivisionAssignment {
                front: Some(aus),
                role: hoi4_state::DivisionRole::Garrison,
                target: Some(ProvinceId(1)),
                assigned_at: now,
            },
        );
        assert!(!ok, "前线命令优先级低于登陆，不应覆盖");

        // 执行裁决后，destination 应为登陆目标
        tick_division_commands_daily(&mut world);
        assert_eq!(
            world.divisions.destinations[div],
            Some(ProvinceId(2)),
            "登陆命令不被前线覆盖"
        );
    }

    #[test]
    fn 手动命令优先级高于_ai_命令() {
        let mut world = synthetic_world();
        let ger = world.country("GER").unwrap();
        let aus = world.country("AUS").unwrap();
        make_war(&mut world, ger, aus);

        let div = world
            .divisions
            .push(ger, ProvinceId(1), 0, 10.0, 1.0, "test".to_owned()) as usize;

        // AI 命令
        let now = world.elapsed_hours;
        issue_ai_ground_command(
            &mut world,
            div,
            DivisionIntent::Assault,
            ProvinceId(2),
            hoi4_state::DivisionAssignment {
                front: Some(aus),
                role: hoi4_state::DivisionRole::Assault,
                target: Some(ProvinceId(2)),
                assigned_at: now,
            },
        );

        // 玩家手动命令
        issue_player_manual_command(&mut world, div, ProvinceId(1));

        tick_division_commands_daily(&mut world);
        // destination 应为玩家目标（原地不动 = None）
        assert_eq!(
            world.divisions.destinations[div], None,
            "手动命令优先，师在原地"
        );
    }

    #[test]
    fn 同一师同一天只由一个执行层写最终目的地() {
        let mut world = synthetic_world();
        let ger = world.country("GER").unwrap();
        let aus = world.country("AUS").unwrap();
        make_war(&mut world, ger, aus);

        let div = world
            .divisions
            .push(ger, ProvinceId(1), 0, 10.0, 1.0, "test".to_owned()) as usize;

        // 同时提交多个同优先级命令，后提交覆盖前
        let now = world.elapsed_hours;
        issue_ai_ground_command(
            &mut world,
            div,
            DivisionIntent::Garrison,
            ProvinceId(1),
            hoi4_state::DivisionAssignment {
                front: Some(aus),
                role: hoi4_state::DivisionRole::Garrison,
                target: Some(ProvinceId(1)),
                assigned_at: now,
            },
        );
        issue_ai_ground_command(
            &mut world,
            div,
            DivisionIntent::Assault,
            ProvinceId(2),
            hoi4_state::DivisionAssignment {
                front: Some(aus),
                role: hoi4_state::DivisionRole::Assault,
                target: Some(ProvinceId(2)),
                assigned_at: now,
            },
        );

        tick_division_commands_daily(&mut world);
        assert_eq!(
            world.divisions.destinations[div],
            Some(ProvinceId(2)),
            "后提交的同优先级命令覆盖前命令"
        );
        assert_eq!(
            world.divisions.assignments[div].as_ref().map(|a| a.role),
            Some(hoi4_state::DivisionRole::Assault),
            "assignment 应为最后提交的角色"
        );
    }
}
