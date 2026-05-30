//! HOI4 兼容的文本存档格式。
//!
//! 顶层结构：
//! ```text
//! HOI4txt
//! version="Ironheart 0.1.0"
//! date=1936.1.1.12
//! player="GER"
//! ironman=no
//! random_seed=12345
//! game_unique_id=0
//! elapsed_hours=0
//! speed=0
//! states={
//!     1={
//!         owner="GER"
//!         controller="GER"
//!         cores={ "GER" "AUS" }
//!         provinces={ 266 3351 6375 }
//!         infrastructure=3
//!         ...
//!     }
//! }
//! countries={
//!     GER={
//!         political_power=50.0
//!         ...
//!     }
//! }
//! province_overrides={
//!     # 只列出 controller != owner 或 supply 异常的省份
//!     1234={ controller="SOV" supply=42.5 }
//! }
//! ```

use std::fmt::Write as _;

use clausewitz_parser::parser::{parse, Block, Value};

use crate::diplomacy::{
    Autonomy, AutonomyLevel, DiplomaticRequest, DiplomaticRequestId, DiplomaticRequestKind,
    DiplomaticRequestStatus, Faction, OpinionMatrix, Treaty, TreatyId, TreatyKind, War, Wargoal,
    WargoalType,
};
use crate::frontline::{ArmyId, FrontlineOrder, General, GeneralId, OffensiveArrow, PlayerArmy};
use crate::ids::{CountryId, FactionId, ProvinceId, StateId};
use crate::save::{SaveError, SaveKind, SaveMeta, SaveResult, ENGINE_VERSION, TEXT_MAGIC};
use crate::store::StateIntegrationStatus;
use crate::time::GameDate;
use crate::World;

// ─── 写入 ──────────────────────────────────────────────────────────

/// 序列化整个 World 为字符串
pub fn write_string(world: &World) -> String {
    // 估算容量：~50 字节/state + ~150 字节/country + 头部
    let cap =
        1024 + world.states.count * 256 + world.countries.count * 256 + world.provinces.count * 8;
    let mut s = String::with_capacity(cap);

    // 头部 magic
    s.push_str(TEXT_MAGIC);
    s.push('\n');

    // 元数据
    let _ = writeln!(s, "version=\"{}\"", ENGINE_VERSION);
    let _ = writeln!(
        s,
        "date={}.{}.{}.{}",
        world.date.year, world.date.month, world.date.day, world.date.hour
    );
    let player_tag = if world.player.is_none() {
        ""
    } else {
        world.country_tag(world.player).unwrap_or("")
    };
    let _ = writeln!(s, "player=\"{}\"", player_tag);
    let _ = writeln!(s, "ironman=no");
    let _ = writeln!(s, "random_seed={}", world.random_seed);
    let _ = writeln!(s, "game_unique_id={}", world.game_unique_id);
    let _ = writeln!(s, "elapsed_hours={}", world.elapsed_hours);
    let _ = writeln!(s, "speed={}", speed_to_int(world.speed));

    // ─── states ───
    s.push_str("states={\n");
    // 排序保证确定性
    let mut state_order: Vec<(u16, StateId)> = world
        .state_id_lookup
        .iter()
        .map(|(&game_id, &sid)| (game_id, sid))
        .collect();
    state_order.sort_by_key(|x| x.0);

    for (game_id, sid) in state_order {
        write_state(&mut s, world, game_id, sid);
    }
    s.push_str("}\n");

    // ─── countries ───
    s.push_str("countries={\n");
    let mut tag_order: Vec<(String, CountryId)> = world
        .tag_to_country
        .iter()
        .map(|(t, &id)| (t.clone(), id))
        .collect();
    tag_order.sort_by(|a, b| a.0.cmp(&b.0));

    for (tag, cid) in tag_order {
        write_country(&mut s, world, &tag, cid);
    }
    s.push_str("}\n");

    write_diplomacy(&mut s, world);

    write_generals(&mut s, world);

    // ─── province overrides ───
    // 仅写入 controller != owner 或 supply != 默认 50.0 的省份
    s.push_str("province_overrides={\n");
    for pidx in 0..world.provinces.count {
        // 只看属于某 state 的省份；其余省份没有运行时状态需要保存
        if world.provinces.state_of[pidx].is_none() {
            continue;
        }
        let owner = world.provinces.owners[pidx];
        let controller = world.provinces.controllers[pidx];
        let supply = world.provinces.supply[pidx];
        let needs = controller != owner || (supply - 50.0).abs() > 0.001;
        if !needs {
            continue;
        }
        let _ = writeln!(s, "\t{}={{", pidx);
        if controller != owner {
            let ctag = if controller.is_none() {
                ""
            } else {
                world.country_tag(controller).unwrap_or("")
            };
            let _ = writeln!(s, "\t\tcontroller=\"{}\"", ctag);
        }
        let _ = writeln!(s, "\t\tsupply={}", format_float(supply));
        s.push_str("\t}\n");
    }
    s.push_str("}\n");

    s
}

fn write_state(s: &mut String, world: &World, game_id: u16, sid: StateId) {
    let i = sid.0 as usize;
    if i >= world.states.count {
        return;
    }
    let _ = writeln!(s, "\t{}={{", game_id);

    let owner_tag = tag_or_none(world, world.states.owners[i]);
    let ctrl_tag = tag_or_none(world, world.states.controllers[i]);
    let _ = writeln!(s, "\t\towner=\"{}\"", owner_tag);
    let _ = writeln!(s, "\t\tcontroller=\"{}\"", ctrl_tag);

    // cores
    s.push_str("\t\tcores={");
    for (idx, &core_cid) in world.states.cores[i].iter().enumerate() {
        if idx > 0 {
            s.push(' ');
        } else {
            s.push(' ');
        }
        let t = tag_or_none(world, core_cid);
        let _ = write!(s, "\"{}\"", t);
    }
    s.push_str(" }\n");

    // provinces
    s.push_str("\t\tprovinces={");
    for (idx, p) in world.states.provinces[i].iter().enumerate() {
        if idx > 0 {
            s.push(' ');
        } else {
            s.push(' ');
        }
        let _ = write!(s, "{}", p.0);
    }
    s.push_str(" }\n");

    let _ = writeln!(s, "\t\tinfrastructure={}", world.states.infrastructure[i]);
    let _ = writeln!(s, "\t\tmanpower={}", world.states.manpower_pool[i]);
    let _ = writeln!(
        s,
        "\t\tintegration=\"{}\"",
        state_integration_to_str(world.states.integration_status[i])
    );
    let _ = writeln!(
        s,
        "\t\tresistance={}",
        format_float(world.states.resistance[i])
    );
    let _ = writeln!(
        s,
        "\t\tcompliance={}",
        format_float(world.states.compliance[i])
    );
    let _ = writeln!(s, "\t\tname=\"{}\"", world.states.names[i]);
    s.push_str("\t}\n");
}

fn write_country(s: &mut String, world: &World, tag: &str, cid: CountryId) {
    let i = cid.0 as usize;
    if i >= world.countries.count {
        return;
    }
    let _ = writeln!(s, "\t{}={{", tag);
    let _ = writeln!(
        s,
        "\t\tpolitical_power={}",
        format_float(world.countries.political_power[i])
    );
    let _ = writeln!(
        s,
        "\t\tstability={}",
        format_float(world.countries.stability[i])
    );
    let _ = writeln!(
        s,
        "\t\twar_support={}",
        format_float(world.countries.war_support[i])
    );
    let _ = writeln!(
        s,
        "\t\truling_party=\"{}\"",
        world.countries.ruling_party[i]
    );
    let _ = writeln!(s, "\t\tmanpower={}", world.manpower(CountryId(i as u16)));
    let _ = writeln!(s, "\t\tfuel={}", format_float(world.countries.fuel[i]));
    let _ = writeln!(
        s,
        "\t\tfuel_capacity={}",
        format_float(world.countries.fuel_capacity[i])
    );
    let _ = writeln!(
        s,
        "\t\tarmy_xp={}",
        format_float(world.countries.army_xp[i])
    );
    let _ = writeln!(
        s,
        "\t\tnavy_xp={}",
        format_float(world.countries.navy_xp[i])
    );
    let _ = writeln!(s, "\t\tair_xp={}", format_float(world.countries.air_xp[i]));
    let _ = writeln!(
        s,
        "\t\tat_war={}",
        if world.countries.at_war[i] {
            "yes"
        } else {
            "no"
        }
    );

    // completed_techs
    s.push_str("\t\tcompleted_techs={");
    for (idx, t) in world.countries.completed_techs[i].iter().enumerate() {
        if idx > 0 {
            s.push(' ');
        } else {
            s.push(' ');
        }
        let _ = write!(s, "\"{}\"", t);
    }
    s.push_str(" }\n");

    // ideas
    s.push_str("\t\tideas={");
    for (idx, t) in world.countries.ideas[i].iter().enumerate() {
        if idx > 0 {
            s.push(' ');
        } else {
            s.push(' ');
        }
        let _ = write!(s, "\"{}\"", t);
    }
    s.push_str(" }\n");

    // ─── armies (frontline-orders R10/R12) ───
    write_armies(s, world, cid);

    s.push_str("\t}\n");
}

/// 把 owner==cid 的所有 [`PlayerArmy`] 序列化进 country 块尾部（_R10.1, R12.2_）。
///
/// - 顶层键为 `ArmyId.0` 升序，确保确定性输出（_R12.2_）。
/// - 字段顺序固定：`name`、`members`、`path`、`arrow`、`anchor`、`active`。
/// - `order` 为 `None` 的集团军只输出 `name` 与 `members`；不写 `path/arrow/anchor/active`。
fn write_armies(s: &mut String, world: &World, cid: CountryId) {
    // 收集 owner==cid 的索引并按 ArmyId.0 升序排序
    let mut order: Vec<usize> = world
        .player_armies
        .iter()
        .enumerate()
        .filter(|(_, a)| a.owner == cid)
        .map(|(idx, _)| idx)
        .collect();
    if order.is_empty() {
        return;
    }
    order.sort_by_key(|&idx| world.player_armies[idx].id.0);

    s.push_str("\t\tarmies={\n");
    for ai in order {
        let army = &world.player_armies[ai];
        let _ = writeln!(s, "\t\t\t{}={{", army.id.0);
        // name
        let _ = writeln!(s, "\t\t\t\tname=\"{}\"", escape_name(&army.name));
        if let Some(commander) = army.commander {
            let _ = writeln!(s, "\t\t\t\tcommander={}", commander.0);
        }
        // members
        s.push_str("\t\t\t\tmembers={");
        for &m in &army.members {
            s.push(' ');
            let _ = write!(s, "{}", m);
        }
        s.push_str(" }\n");

        // order：None 时只写 name+members（design.md "存档扩展"）
        if let Some(o) = &army.order {
            // path
            s.push_str("\t\t\t\tpath={");
            for p in &o.path {
                s.push(' ');
                let _ = write!(s, "{}", p.0);
            }
            s.push_str(" }\n");
            // arrow（可选）
            if let Some(arrow) = &o.arrow {
                s.push_str("\t\t\t\tarrow={");
                for p in &arrow.provinces {
                    s.push(' ');
                    let _ = write!(s, "{}", p.0);
                }
                s.push_str(" }\n");
            }
            // anchor（可选）
            if let Some(a) = o.anchor {
                let _ = writeln!(s, "\t\t\t\tanchor={}", a.0);
            }
            // active
            let _ = writeln!(s, "\t\t\t\tactive={}", if o.active { "yes" } else { "no" });
            let _ = writeln!(
                s,
                "\t\t\t\texecuting={}",
                if o.executing { "yes" } else { "no" }
            );
        }

        s.push_str("\t\t\t}\n");
    }
    s.push_str("\t\t}\n");
}

fn write_generals(s: &mut String, world: &World) {
    if world.generals.is_empty() {
        return;
    }
    s.push_str("generals={\n");
    let mut generals: Vec<&General> = world.generals.iter().collect();
    generals.sort_by_key(|g| g.id.0);
    for general in generals {
        let _ = writeln!(s, "\t{}={{", general.id.0);
        let _ = writeln!(s, "\t\towner=\"{}\"", tag_or_none(world, general.owner));
        let _ = writeln!(s, "\t\tname=\"{}\"", escape_name(&general.name));
        let _ = writeln!(s, "\t\tskill={}", general.skill);
        let _ = writeln!(s, "\t\tattack={}", general.attack);
        let _ = writeln!(s, "\t\tdefense={}", general.defense);
        let _ = writeln!(s, "\t\tplanning={}", general.planning);
        let _ = writeln!(s, "\t\tlogistics={}", general.logistics);
        let _ = writeln!(s, "\t\tcommand_limit={}", general.command_limit);
        s.push_str("\t}\n");
    }
    s.push_str("}\n");
}

fn write_diplomacy(s: &mut String, world: &World) {
    s.push_str("diplomacy={\n");
    let _ = writeln!(
        s,
        "\tworld_tension={}",
        format_float(world.diplomacy.world_tension)
    );
    let _ = writeln!(s, "\tnext_war_id={}", world.diplomacy.next_war_id);
    let _ = writeln!(s, "\tnext_faction_id={}", world.diplomacy.next_faction_id);

    s.push_str("\tfactions={\n");
    for f in &world.diplomacy.factions {
        let _ = writeln!(s, "\t\t{}={{", f.id.0);
        let _ = writeln!(s, "\t\t\tname=\"{}\"", escape_name(&f.name));
        let _ = writeln!(s, "\t\t\tleader=\"{}\"", tag_or_none(world, f.leader));
        let _ = writeln!(s, "\t\t\tcreated_at_hour={}", f.created_at_hour);
        s.push_str("\t\t\tmembers={");
        for &member in &f.members {
            let _ = write!(s, " \"{}\"", tag_or_none(world, member));
        }
        s.push_str(" }\n\t\t}\n");
    }
    s.push_str("\t}\n");

    s.push_str("\twars={\n");
    let mut wars: Vec<_> = world.diplomacy.wars.iter().collect();
    wars.sort_by_key(|(id, _)| **id);
    for (id, war) in wars {
        let _ = writeln!(s, "\t\t{}={{", id);
        let _ = writeln!(
            s,
            "\t\t\tprimary_attacker=\"{}\"",
            tag_or_none(world, war.primary_attacker)
        );
        let _ = writeln!(
            s,
            "\t\t\tprimary_defender=\"{}\"",
            tag_or_none(world, war.primary_defender)
        );
        let _ = writeln!(s, "\t\t\tstarted_at_hour={}", war.started_at_hour);
        let _ = writeln!(
            s,
            "\t\t\tattacker_war_score={}",
            format_float(war.attacker_war_score)
        );
        let _ = writeln!(
            s,
            "\t\t\tdefender_war_score={}",
            format_float(war.defender_war_score)
        );
        write_country_set(s, world, "attackers", &war.attackers, "\t\t\t");
        write_country_set(s, world, "defenders", &war.defenders, "\t\t\t");
        write_wargoals(
            s,
            world,
            "attacker_wargoals",
            &war.attacker_wargoals,
            "\t\t\t",
        );
        write_wargoals(
            s,
            world,
            "defender_wargoals",
            &war.defender_wargoals,
            "\t\t\t",
        );
        s.push_str("\t\t}\n");
    }
    s.push_str("\t}\n");

    s.push_str("\tautonomy={\n");
    let mut autonomy: Vec<_> = world.diplomacy.autonomy.iter().collect();
    autonomy.sort_by_key(|(subject, _)| subject.0);
    for (subject, a) in autonomy {
        let _ = writeln!(s, "\t\t\"{}\"={{", tag_or_none(world, *subject));
        let _ = writeln!(s, "\t\t\tmaster=\"{}\"", tag_or_none(world, a.master));
        let _ = writeln!(s, "\t\t\tlevel={}", autonomy_level_to_u8(a.level));
        let _ = writeln!(s, "\t\t\tprogress={}", format_float(a.progress));
        let _ = writeln!(s, "\t\t\tsince_hour={}", a.since_hour);
        s.push_str("\t\t}\n");
    }
    s.push_str("\t}\n");

    s.push_str("\tpending_wargoals={\n");
    let mut pending: Vec<_> = world.diplomacy.pending_wargoals.iter().collect();
    pending.sort_by_key(|(claimant, _)| claimant.0);
    for (claimant, goals) in pending {
        let _ = writeln!(s, "\t\t\"{}\"={{", tag_or_none(world, *claimant));
        write_wargoals(s, world, "goals", goals, "\t\t\t");
        s.push_str("\t\t}\n");
    }
    s.push_str("\t}\n");

    s.push_str("\topinions={\n");
    let mut opinions: Vec<_> = world.diplomacy.opinions.opinions.iter().collect();
    opinions.sort_by_key(|((from, to), _)| (from.0, to.0));
    for ((from, to), value) in opinions {
        let _ = writeln!(
            s,
            "\t\topinion={{ from=\"{}\" to=\"{}\" value={} }}",
            tag_or_none(world, *from),
            tag_or_none(world, *to),
            value
        );
    }
    s.push_str("\t}\n");

    write_country_pairs(
        s,
        world,
        "annexed_countries",
        &world.diplomacy.annexed_countries,
        "\t",
    );

    s.push_str("\tmilitary_access={\n");
    let mut access: Vec<_> = world.diplomacy.military_access.iter().copied().collect();
    access.sort_unstable();
    for (grantor, grantee) in access {
        let _ = writeln!(
            s,
            "\t\taccess={{ grantor=\"{}\" grantee=\"{}\" }}",
            tag_or_none(world, CountryId(grantor)),
            tag_or_none(world, CountryId(grantee))
        );
    }
    s.push_str("\t}\n");

    let _ = writeln!(s, "\tnext_treaty_id={}", world.diplomacy.next_treaty_id);
    s.push_str("\ttreaties={\n");
    for treaty in &world.diplomacy.treaties {
        let _ = writeln!(s, "\t\t{}={{", treaty.id.0);
        let _ = writeln!(s, "\t\t\tkind={}", treaty_kind_to_u8(treaty.kind));
        let _ = writeln!(s, "\t\t\tsince_hour={}", treaty.since_hour);
        let _ = writeln!(
            s,
            "\t\t\texpires_at_hour={}",
            treaty.expires_at_hour.map(|v| v as i64).unwrap_or(-1)
        );
        s.push_str("\t\t\tparties={");
        for &party in &treaty.parties {
            let _ = write!(s, " \"{}\"", tag_or_none(world, party));
        }
        s.push_str(" }\n\t\t}\n");
    }
    s.push_str("\t}\n");

    let _ = writeln!(
        s,
        "\tnext_diplomatic_request_id={}",
        world.diplomacy.next_diplomatic_request_id
    );
    s.push_str("\tdiplomatic_requests={\n");
    for request in &world.diplomacy.diplomatic_requests {
        let _ = writeln!(s, "\t\t{}={{", request.id.0);
        let _ = writeln!(s, "\t\t\tfrom=\"{}\"", tag_or_none(world, request.from));
        let _ = writeln!(s, "\t\t\tto=\"{}\"", tag_or_none(world, request.to));
        write_request_kind(s, &request.kind, "\t\t\t");
        let _ = writeln!(s, "\t\t\tstatus={}", request_status_to_u8(request.status));
        let _ = writeln!(s, "\t\t\tcreated_at_hour={}", request.created_at_hour);
        let _ = writeln!(
            s,
            "\t\t\texpires_at_hour={}",
            request.expires_at_hour.map(|v| v as i64).unwrap_or(-1)
        );
        let _ = writeln!(
            s,
            "\t\t\tresolved_at_hour={}",
            request.resolved_at_hour.map(|v| v as i64).unwrap_or(-1)
        );
        s.push_str("\t\t}\n");
    }
    s.push_str("\t}\n}");
    s.push('\n');
}

fn write_country_set(
    s: &mut String,
    world: &World,
    key: &str,
    countries: &std::collections::HashSet<CountryId>,
    indent: &str,
) {
    let mut values: Vec<_> = countries.iter().copied().collect();
    values.sort_by_key(|c| c.0);
    let _ = write!(s, "{}{}={{", indent, key);
    for cid in values {
        let _ = write!(s, " \"{}\"", tag_or_none(world, cid));
    }
    s.push_str(" }\n");
}

fn write_country_pairs(
    s: &mut String,
    world: &World,
    key: &str,
    countries: &std::collections::HashSet<CountryId>,
    indent: &str,
) {
    let mut values: Vec<_> = countries.iter().copied().collect();
    values.sort_by_key(|c| c.0);
    let _ = write!(s, "{}{}={{", indent, key);
    for cid in values {
        let _ = write!(s, " \"{}\"", tag_or_none(world, cid));
    }
    s.push_str(" }\n");
}

fn write_wargoals(s: &mut String, world: &World, key: &str, goals: &[Wargoal], indent: &str) {
    let _ = writeln!(s, "{}{}={{", indent, key);
    for goal in goals {
        let _ = writeln!(s, "{}\tgoal={{", indent);
        let _ = writeln!(
            s,
            "{}\t\tclaimant=\"{}\"",
            indent,
            tag_or_none(world, goal.claimant)
        );
        let _ = writeln!(
            s,
            "{}\t\ttarget=\"{}\"",
            indent,
            tag_or_none(world, goal.target)
        );
        let _ = writeln!(s, "{}\t\tkind={}", indent, wargoal_type_to_u8(goal.kind));
        let _ = writeln!(
            s,
            "{}\t\ttarget_state={}",
            indent,
            goal.target_state.map(|s| s.0 as i64).unwrap_or(-1)
        );
        let _ = writeln!(
            s,
            "{}\t\tjustified={}",
            indent,
            if goal.justified { "yes" } else { "no" }
        );
        let _ = writeln!(
            s,
            "{}\t\tjustify_progress={}",
            indent,
            format_float(goal.justify_progress)
        );
        let _ = writeln!(
            s,
            "{}\t\tjustify_total_days={}",
            indent,
            format_float(goal.justify_total_days)
        );
        let _ = writeln!(s, "{}\t}}", indent);
    }
    let _ = writeln!(s, "{}}}", indent);
}

/// 转义军名中的双引号与反斜杠（保持与现有字符串字段同形）。
fn escape_name(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            _ => out.push(ch),
        }
    }
    out
}

fn tag_or_none(world: &World, cid: CountryId) -> &str {
    if cid.is_none() {
        ""
    } else {
        world.country_tag(cid).unwrap_or("")
    }
}

fn speed_to_int(s: crate::time::GameSpeed) -> u8 {
    use crate::time::GameSpeed::*;
    match s {
        Paused => 0,
        Speed1 => 1,
        Speed2 => 2,
        Speed3 => 3,
        Speed4 => 4,
        Speed5 => 5,
    }
}

fn int_to_speed(v: i64) -> crate::time::GameSpeed {
    use crate::time::GameSpeed::*;
    match v {
        1 => Speed1,
        2 => Speed2,
        3 => Speed3,
        4 => Speed4,
        5 => Speed5,
        _ => Paused,
    }
}

/// 格式化浮点数，避免科学计数和尾随零
fn format_float(v: f32) -> String {
    if v.fract() == 0.0 {
        format!("{:.1}", v)
    } else {
        // 6 位精度
        let s = format!("{:.6}", v);
        // 去掉尾随零（保留至少一位小数）
        let trimmed = s.trim_end_matches('0');
        let trimmed = trimmed.trim_end_matches('.');
        if trimmed.contains('.') {
            trimmed.to_owned()
        } else {
            format!("{}.0", trimmed)
        }
    }
}

// ─── 读取 ──────────────────────────────────────────────────────────

/// 跳过开头的 magic 行，返回剩余内容
fn strip_magic(input: &str) -> SaveResult<&str> {
    let trimmed = input.trim_start_matches('\u{FEFF}');
    let trimmed = trimmed.trim_start();
    if let Some(rest) = trimmed.strip_prefix(TEXT_MAGIC) {
        Ok(rest)
    } else {
        Err(SaveError::BadMagic(format!(
            "expected `{}` magic at start of text save",
            TEXT_MAGIC
        )))
    }
}

/// 仅解析头部元数据
pub fn read_meta_str(input: &str) -> SaveResult<SaveMeta> {
    let body = strip_magic(input)?;
    let block = parse(body);
    extract_meta(&block)
}

fn extract_meta(block: &Block) -> SaveResult<SaveMeta> {
    let version = block
        .get_string("version")
        .unwrap_or(ENGINE_VERSION)
        .to_owned();

    let date = parse_date_token(block)
        .ok_or_else(|| SaveError::Parse("missing or malformed `date` field".to_owned()))?;

    let player_tag = block.get_string("player").unwrap_or("").to_owned();
    let random_seed = get_u64(block, "random_seed").unwrap_or(0);
    let game_unique_id = get_u64(block, "game_unique_id").unwrap_or(0);
    let elapsed_hours = get_u64(block, "elapsed_hours").unwrap_or(0);
    let ironman = block.get_bool("ironman").unwrap_or(false);

    Ok(SaveMeta {
        kind: SaveKind::Text,
        version,
        date,
        player_tag,
        random_seed,
        game_unique_id,
        elapsed_hours,
        ironman,
    })
}

/// `date=1936.1.1.12` 在我们的 lexer 里是 Ident (因为有多个 `.`)
fn parse_date_token(block: &Block) -> Option<GameDate> {
    let raw = block.get("date")?;
    match raw {
        Value::String(s) => GameDate::parse(s),
        _ => None,
    }
}

/// 读 u64：值可能是 Integer（适合时）或 String（数字超出 i64 范围时被 lexer 当作 ident）
fn get_u64(block: &Block, key: &str) -> Option<u64> {
    match block.get(key)? {
        Value::Integer(v) => {
            if *v >= 0 {
                Some(*v as u64)
            } else {
                Some(*v as u64) // 让用户决定如何处理负数（按位重解释）
            }
        }
        Value::String(s) => s.parse::<u64>().ok(),
        _ => None,
    }
}

fn get_i64(block: &Block, key: &str) -> Option<i64> {
    match block.get(key)? {
        Value::Integer(v) => Some(*v),
        Value::String(s) => s.parse::<i64>().ok(),
        _ => None,
    }
}

/// 把文本存档解析回 World（覆盖 world 的运行时状态）
pub fn read_str(input: &str, world: &mut World) -> SaveResult<()> {
    let body = strip_magic(input)?;
    let block = parse(body);
    let meta = extract_meta(&block)?;

    world.date = meta.date;
    world.elapsed_hours = meta.elapsed_hours;
    world.random_seed = meta.random_seed;
    world.game_unique_id = meta.game_unique_id;
    world.speed = int_to_speed(block.get_int("speed").unwrap_or(0));

    world.player = if meta.player_tag.is_empty() {
        CountryId::NONE
    } else {
        world
            .tag_to_country
            .get(&meta.player_tag)
            .copied()
            .unwrap_or(CountryId::NONE)
    };

    // ─── frontline-orders: 清空旧的 player_armies，让 apply_country 中的
    // ─── apply_armies push 新条目（_R10.3_：旧档无 armies 块时保持 empty）。
    world.player_armies.clear();
    world.next_army_id = 0;
    world.generals.clear();
    world.next_general_id = 0;

    // ─── states ───
    if let Some(states_block) = block.get_block("states") {
        for entry in &states_block.entries {
            let game_id: u16 = match entry.key.parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let Some(&sid) = world.state_id_lookup.get(&game_id) else {
                continue;
            };
            let Value::Block(state_block) = &entry.value else {
                continue;
            };
            apply_state(world, sid, state_block)?;
        }
    }

    // ─── countries ───
    if let Some(countries_block) = block.get_block("countries") {
        for entry in &countries_block.entries {
            let tag = &entry.key;
            let Some(&cid) = world.tag_to_country.get(tag) else {
                continue;
            };
            let Value::Block(c_block) = &entry.value else {
                continue;
            };
            apply_country(world, cid, c_block);
        }
    }

    if let Some(diplomacy_block) = block.get_block("diplomacy") {
        apply_diplomacy(world, diplomacy_block);
    }

    if let Some(generals_block) = block.get_block("generals") {
        apply_generals(world, generals_block);
    } else {
        world.seed_default_generals();
    }
    sanitize_army_commanders(world);

    // ─── province overrides ───
    if let Some(po) = block.get_block("province_overrides") {
        for entry in &po.entries {
            let pid: usize = match entry.key.parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            if pid >= world.provinces.count {
                continue;
            }
            let Value::Block(b) = &entry.value else {
                continue;
            };
            if let Some(ctrl_tag) = b.get_string("controller") {
                let cid = if ctrl_tag.is_empty() {
                    CountryId::NONE
                } else {
                    world
                        .tag_to_country
                        .get(ctrl_tag)
                        .copied()
                        .unwrap_or(CountryId::NONE)
                };
                world.provinces.controllers[pid] = cid;
            }
            if let Some(s) = b.get_float("supply") {
                world.provinces.supply[pid] = s as f32;
            }
        }
    }

    // 重新计算工厂/人力缓存
    world.recalc_country_caches();

    // ─── frontline-orders: 排序 + next_army_id 推导（_R10.1, R10.2_）。
    world.player_armies.sort_by_key(|a| a.id.0);
    world.next_army_id = world
        .player_armies
        .iter()
        .map(|a| a.id.0)
        .max()
        .map(|m| m.saturating_add(1))
        .unwrap_or(0);
    world.next_general_id = world
        .generals
        .iter()
        .map(|g| g.id.0)
        .max()
        .map(|m| m.saturating_add(1))
        .unwrap_or(0);

    Ok(())
}

fn apply_generals(world: &mut World, generals: &Block) {
    for entry in &generals.entries {
        let raw_id: u32 = match entry.key.parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let Value::Block(b) = &entry.value else {
            continue;
        };
        let owner = b
            .get_string("owner")
            .map(|tag| lookup_tag(world, tag))
            .unwrap_or(CountryId::NONE);
        if owner.is_none() {
            continue;
        }
        world.generals.push(General {
            id: GeneralId(raw_id),
            owner,
            name: b.get_string("name").unwrap_or("General").to_owned(),
            skill: b.get_int("skill").unwrap_or(1).clamp(1, 9) as u8,
            attack: b.get_int("attack").unwrap_or(1).clamp(0, 9) as u8,
            defense: b.get_int("defense").unwrap_or(1).clamp(0, 9) as u8,
            planning: b.get_int("planning").unwrap_or(1).clamp(0, 9) as u8,
            logistics: b.get_int("logistics").unwrap_or(1).clamp(0, 9) as u8,
            command_limit: b.get_int("command_limit").unwrap_or(24).clamp(1, 999) as u16,
        });
    }
}

fn sanitize_army_commanders(world: &mut World) {
    let mut assigned = std::collections::HashSet::new();
    let generals = &world.generals;
    for army in &mut world.player_armies {
        let Some(gid) = army.commander else {
            continue;
        };
        let valid = generals
            .iter()
            .any(|g| g.id == gid && g.owner == army.owner);
        if !valid || !assigned.insert(gid) {
            army.commander = None;
        }
    }
}

fn apply_state(world: &mut World, sid: StateId, b: &Block) -> SaveResult<()> {
    let i = sid.0 as usize;

    if let Some(owner_tag) = b.get_string("owner") {
        let cid = lookup_tag(world, owner_tag);
        world.states.owners[i] = cid;
        // 同步省份 owner
        for &p in &world.states.provinces[i].clone() {
            let pi = p.0 as usize;
            if pi < world.provinces.count {
                world.provinces.owners[pi] = cid;
            }
        }
    }
    if let Some(ctrl_tag) = b.get_string("controller") {
        let cid = lookup_tag(world, ctrl_tag);
        world.states.controllers[i] = cid;
        for &p in &world.states.provinces[i].clone() {
            let pi = p.0 as usize;
            if pi < world.provinces.count {
                world.provinces.controllers[pi] = cid;
            }
        }
    }

    // cores: 数组形式 { "GER" "AUS" }
    if let Some(cores_block) = b.get_block("cores") {
        let mut cores = Vec::with_capacity(cores_block.values.len());
        for v in &cores_block.values {
            if let Value::String(t) = v {
                let cid = lookup_tag(world, t);
                if !cid.is_none() {
                    cores.push(cid);
                }
            }
        }
        world.states.cores[i] = cores;
    }

    if let Some(provs_block) = b.get_block("provinces") {
        let mut provs = Vec::with_capacity(provs_block.values.len());
        for v in &provs_block.values {
            if let Value::Integer(p) = v {
                provs.push(ProvinceId(*p as u16));
            }
        }
        world.states.provinces[i] = provs;
    }

    if let Some(v) = b.get_int("infrastructure") {
        world.states.infrastructure[i] = v.clamp(0, 255) as u8;
    }
    if let Some(v) = b.get_int("manpower") {
        world.states.manpower_pool[i] = v.max(0) as u32;
    }
    if let Some(v) = b.get_string("integration") {
        world.states.integration_status[i] = str_to_state_integration(v);
    }
    if let Some(v) = b.get_float("resistance") {
        world.states.resistance[i] = v as f32;
    }
    if let Some(v) = b.get_float("compliance") {
        world.states.compliance[i] = v as f32;
    }
    if let Some(name) = b.get_string("name") {
        world.states.names[i] = name.to_owned();
    }
    Ok(())
}

fn apply_country(world: &mut World, cid: CountryId, b: &Block) {
    let i = cid.0 as usize;

    if let Some(v) = b.get_float("political_power") {
        world.countries.political_power[i] = v as f32;
    }
    if let Some(v) = b.get_float("stability") {
        world.countries.stability[i] = v as f32;
    }
    if let Some(v) = b.get_float("war_support") {
        world.countries.war_support[i] = v as f32;
    }
    if let Some(v) = b.get_string("ruling_party") {
        world.countries.ruling_party[i] = v.to_owned();
    }
    if let Some(_v) = b.get_int("manpower") {
        // manpower is now derived from PopGroups; loaded value is ignored
    }
    if let Some(v) = b.get_float("fuel") {
        world.countries.fuel[i] = v as f32;
    }
    if let Some(v) = b.get_float("fuel_capacity") {
        world.countries.fuel_capacity[i] = v as f32;
    }
    if let Some(v) = b.get_float("army_xp") {
        world.countries.army_xp[i] = v as f32;
    }
    if let Some(v) = b.get_float("navy_xp") {
        world.countries.navy_xp[i] = v as f32;
    }
    if let Some(v) = b.get_float("air_xp") {
        world.countries.air_xp[i] = v as f32;
    }
    if let Some(v) = b.get_bool("at_war") {
        world.countries.at_war[i] = v;
    }
    if let Some(techs_block) = b.get_block("completed_techs") {
        let techs: Vec<String> = techs_block
            .values
            .iter()
            .filter_map(|v| match v {
                Value::String(s) => Some(s.clone()),
                _ => None,
            })
            .collect();
        world.countries.completed_techs[i] = techs;
    }
    if let Some(ideas_block) = b.get_block("ideas") {
        let ideas: Vec<String> = ideas_block
            .values
            .iter()
            .filter_map(|v| match v {
                Value::String(s) => Some(s.clone()),
                _ => None,
            })
            .collect();
        world.countries.ideas[i] = ideas;
    }

    // ─── armies (frontline-orders R10/R12) ───
    if let Some(armies_block) = b.get_block("armies") {
        apply_armies(world, cid, armies_block);
    }
}

/// 反序列化 `armies={…}` 块，把每条 entry 还原为一个 [`PlayerArmy`] 并 push 到
/// `world.player_armies`。`world` 中是否已存在同 owner 的集团军不影响；调用方负责
/// 在加载前清空。
///
/// 验证规则（_R10.4, R12.3_）：
/// - 越界成员（`i >= world.divisions.count`）丢弃并 emit `tracing::warn!("过期成员", ...)`。
/// - 路径/箭头省 id 越界、相邻不 land-adjacent、或路径内重复 → 丢弃整个 `order`，
///   保留 `PlayerArmy` 外壳（id/name/owner/members），并 emit
///   `tracing::warn!("存档战线已损坏，已重置")`。
fn apply_armies(world: &mut World, owner: CountryId, armies: &Block) {
    for entry in &armies.entries {
        let raw_id: u32 = match entry.key.parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let Value::Block(army_block) = &entry.value else {
            continue;
        };

        let name = army_block.get_string("name").unwrap_or("").to_owned();
        let commander = army_block
            .get_int("commander")
            .and_then(|id| (id >= 0).then_some(GeneralId(id as u32)));

        // members
        let mut members: Vec<usize> = Vec::new();
        if let Some(mb) = army_block.get_block("members") {
            for v in &mb.values {
                if let Value::Integer(i) = v {
                    let idx = *i as usize;
                    if idx >= world.divisions.count {
                        tracing::warn!(
                            target: "frontline_save",
                            army_id = raw_id,
                            idx = idx,
                            "过期成员"
                        );
                        continue;
                    }
                    members.push(idx);
                }
            }
        }

        // order：path / arrow / anchor / active
        let order = parse_order(world, raw_id, army_block);

        world.player_armies.push(PlayerArmy {
            id: ArmyId(raw_id),
            name,
            owner,
            commander,
            members,
            order,
        });
    }
}

/// 解析单条 `armies={ … }` 内的 order 字段。返回 `None` 表示该集团军无 order
/// （可能因为存档里没写，或者校验失败被丢弃）。
fn parse_order(world: &World, army_id: u32, b: &Block) -> Option<FrontlineOrder> {
    // path 不存在 → 无 order
    let path_block = b.get_block("path")?;

    // 解析 path
    let mut path: Vec<ProvinceId> = Vec::with_capacity(path_block.values.len());
    for v in &path_block.values {
        if let Value::Integer(i) = v {
            path.push(ProvinceId(*i as u16));
        }
    }
    if path.is_empty() {
        // 视为无 order
        return None;
    }

    // 解析 arrow（可选）
    let mut arrow_provs: Option<Vec<ProvinceId>> = None;
    if let Some(arr_block) = b.get_block("arrow") {
        let mut v: Vec<ProvinceId> = Vec::with_capacity(arr_block.values.len());
        for value in &arr_block.values {
            if let Value::Integer(i) = value {
                v.push(ProvinceId(*i as u16));
            }
        }
        if !v.is_empty() {
            arrow_provs = Some(v);
        }
    }

    // 解析 anchor / active（可选）
    let anchor = b.get_int("anchor").map(|n| ProvinceId(n as u16));
    let active = b.get_bool("active").unwrap_or(true);
    let executing = b.get_bool("executing").unwrap_or(false);

    // 校验 path 与 arrow 的合法性。失败 → 丢整个 order（_R12.3_）。
    if !validate_save_path(world, &path) || !no_dups(&path) {
        tracing::warn!(
            target: "frontline_save",
            army_id = army_id,
            "存档战线已损坏，已重置"
        );
        return None;
    }
    if let Some(arr) = &arrow_provs {
        if !validate_save_path(world, arr) {
            tracing::warn!(
                target: "frontline_save",
                army_id = army_id,
                "存档战线已损坏，已重置"
            );
            return None;
        }
    }

    Some(FrontlineOrder {
        path,
        arrow: arrow_provs.map(|provinces| OffensiveArrow { provinces }),
        anchor,
        active,
        executing,
    })
}

/// 校验一条省 id 序列：每个 id 在 `world.provinces` 范围内，且每对相邻在
/// `world.map.adjacencies` 中陆地相邻。空列表视为非法（调用方已处理）。
///
/// 暴露为 `pub(super)` 以便 `save::binary` 在反序列化二进制 `FRARM` chunk
/// 时复用同一套 R10.4 / R12.3 校验规则。
pub(super) fn validate_save_path(world: &World, path: &[ProvinceId]) -> bool {
    let pcount = world.provinces.count;
    for p in path {
        if (p.0 as usize) >= pcount {
            return false;
        }
    }
    for win in path.windows(2) {
        let a = win[0].0;
        let b = win[1].0;
        let neigh = world
            .map
            .adjacencies
            .get(a as usize)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        if !neigh.contains(&b) {
            return false;
        }
    }
    true
}

pub(super) fn no_dups(path: &[ProvinceId]) -> bool {
    let mut seen: std::collections::HashSet<u16> =
        std::collections::HashSet::with_capacity(path.len());
    for p in path {
        if !seen.insert(p.0) {
            return false;
        }
    }
    true
}

fn lookup_tag(world: &World, tag: &str) -> CountryId {
    if tag.is_empty() {
        return CountryId::NONE;
    }
    world
        .tag_to_country
        .get(tag)
        .copied()
        .unwrap_or(CountryId::NONE)
}

fn apply_diplomacy(world: &mut World, b: &Block) {
    world.diplomacy.world_tension = b.get_float("world_tension").unwrap_or(0.0) as f32;
    world.diplomacy.next_war_id = b.get_int("next_war_id").unwrap_or(0).max(0) as u32;
    world.diplomacy.next_faction_id = b.get_int("next_faction_id").unwrap_or(0).max(0) as u32;
    world.diplomacy.factions.clear();
    world.diplomacy.wars.clear();
    world.diplomacy.autonomy.clear();
    world.diplomacy.pending_wargoals.clear();
    world.diplomacy.opinions = OpinionMatrix::new();
    world.diplomacy.annexed_countries.clear();
    world.diplomacy.military_access.clear();
    world.diplomacy.treaties.clear();
    world.diplomacy.diplomatic_requests.clear();

    if let Some(factions) = b.get_block("factions") {
        for entry in &factions.entries {
            let Value::Block(fb) = &entry.value else {
                continue;
            };
            let id = FactionId(
                entry
                    .key
                    .parse::<u32>()
                    .unwrap_or(world.diplomacy.factions.len() as u32),
            );
            let leader = fb
                .get_string("leader")
                .map(|t| lookup_tag(world, t))
                .unwrap_or(CountryId::NONE);
            let members = parse_country_values(world, fb.get_block("members"));
            world.diplomacy.factions.push(Faction {
                id,
                name: fb.get_string("name").unwrap_or("").to_owned(),
                leader,
                members,
                created_at_hour: get_u64(fb, "created_at_hour").unwrap_or(0),
            });
        }
    }

    if let Some(wars) = b.get_block("wars") {
        for entry in &wars.entries {
            let Ok(id) = entry.key.parse::<u32>() else {
                continue;
            };
            let Value::Block(wb) = &entry.value else {
                continue;
            };
            let attackers = parse_country_values(world, wb.get_block("attackers"))
                .into_iter()
                .collect();
            let defenders = parse_country_values(world, wb.get_block("defenders"))
                .into_iter()
                .collect();
            world.diplomacy.wars.insert(
                id,
                War {
                    id,
                    primary_attacker: wb
                        .get_string("primary_attacker")
                        .map(|t| lookup_tag(world, t))
                        .unwrap_or(CountryId::NONE),
                    primary_defender: wb
                        .get_string("primary_defender")
                        .map(|t| lookup_tag(world, t))
                        .unwrap_or(CountryId::NONE),
                    attackers,
                    defenders,
                    started_at_hour: get_u64(wb, "started_at_hour").unwrap_or(0),
                    attacker_war_score: wb.get_float("attacker_war_score").unwrap_or(0.0) as f32,
                    defender_war_score: wb.get_float("defender_war_score").unwrap_or(0.0) as f32,
                    attacker_wargoals: parse_wargoals(world, wb.get_block("attacker_wargoals")),
                    defender_wargoals: parse_wargoals(world, wb.get_block("defender_wargoals")),
                    war_join_policies: std::collections::HashMap::new(),
                },
            );
        }
    }

    if let Some(autonomy) = b.get_block("autonomy") {
        for entry in &autonomy.entries {
            let subject = lookup_tag(world, &entry.key);
            let Value::Block(ab) = &entry.value else {
                continue;
            };
            if subject.is_none() {
                continue;
            }
            world.diplomacy.autonomy.insert(
                subject,
                Autonomy {
                    master: ab
                        .get_string("master")
                        .map(|t| lookup_tag(world, t))
                        .unwrap_or(CountryId::NONE),
                    subject,
                    level: u8_to_autonomy_level(ab.get_int("level").unwrap_or(2) as u8),
                    progress: ab.get_float("progress").unwrap_or(0.0) as f32,
                    since_hour: get_u64(ab, "since_hour").unwrap_or(0),
                },
            );
        }
    }

    if let Some(pending) = b.get_block("pending_wargoals") {
        for entry in &pending.entries {
            let claimant = lookup_tag(world, &entry.key);
            let Value::Block(pb) = &entry.value else {
                continue;
            };
            if claimant.is_none() {
                continue;
            }
            world
                .diplomacy
                .pending_wargoals
                .insert(claimant, parse_wargoals(world, pb.get_block("goals")));
        }
    }

    if let Some(opinions) = b.get_block("opinions") {
        for value in opinions.get_all("opinion") {
            let Value::Block(ob) = value else {
                continue;
            };
            let from = ob
                .get_string("from")
                .map(|t| lookup_tag(world, t))
                .unwrap_or(CountryId::NONE);
            let to = ob
                .get_string("to")
                .map(|t| lookup_tag(world, t))
                .unwrap_or(CountryId::NONE);
            if !from.is_none() && !to.is_none() {
                world
                    .diplomacy
                    .opinions
                    .set(from, to, ob.get_int("value").unwrap_or(0) as i16);
            }
        }
    }

    world.diplomacy.annexed_countries =
        parse_country_values(world, b.get_block("annexed_countries"))
            .into_iter()
            .collect();

    if let Some(access) = b.get_block("military_access") {
        for value in access.get_all("access") {
            let Value::Block(ab) = value else {
                continue;
            };
            let grantor = ab
                .get_string("grantor")
                .map(|t| lookup_tag(world, t))
                .unwrap_or(CountryId::NONE);
            let grantee = ab
                .get_string("grantee")
                .map(|t| lookup_tag(world, t))
                .unwrap_or(CountryId::NONE);
            if !grantor.is_none() && !grantee.is_none() {
                world
                    .diplomacy
                    .military_access
                    .insert((grantor.0, grantee.0));
            }
        }
    }

    world.diplomacy.next_treaty_id = b.get_int("next_treaty_id").unwrap_or(0).max(0) as u32;
    if let Some(treaties) = b.get_block("treaties") {
        for entry in &treaties.entries {
            let Value::Block(tb) = &entry.value else {
                continue;
            };
            let id = TreatyId(
                entry
                    .key
                    .parse::<u32>()
                    .unwrap_or(world.diplomacy.treaties.len() as u32),
            );
            let expires = get_i64(tb, "expires_at_hour").unwrap_or(-1);
            world.diplomacy.treaties.push(Treaty {
                id,
                kind: u8_to_treaty_kind(tb.get_int("kind").unwrap_or(0) as u8),
                parties: parse_country_values(world, tb.get_block("parties")),
                since_hour: get_u64(tb, "since_hour").unwrap_or(0),
                expires_at_hour: (expires >= 0).then_some(expires as u64),
            });
        }
    }

    world.diplomacy.next_diplomatic_request_id =
        b.get_int("next_diplomatic_request_id").unwrap_or(0).max(0) as u32;
    if let Some(requests) = b.get_block("diplomatic_requests") {
        for entry in &requests.entries {
            let Value::Block(rb) = &entry.value else {
                continue;
            };
            let expires = get_i64(rb, "expires_at_hour").unwrap_or(-1);
            let resolved = get_i64(rb, "resolved_at_hour").unwrap_or(-1);
            world.diplomacy.diplomatic_requests.push(DiplomaticRequest {
                id: DiplomaticRequestId(
                    entry
                        .key
                        .parse::<u32>()
                        .unwrap_or(world.diplomacy.diplomatic_requests.len() as u32),
                ),
                from: rb
                    .get_string("from")
                    .map(|t| lookup_tag(world, t))
                    .unwrap_or(CountryId::NONE),
                to: rb
                    .get_string("to")
                    .map(|t| lookup_tag(world, t))
                    .unwrap_or(CountryId::NONE),
                kind: parse_request_kind(rb),
                status: u8_to_request_status(rb.get_int("status").unwrap_or(0) as u8),
                created_at_hour: get_u64(rb, "created_at_hour").unwrap_or(0),
                expires_at_hour: (expires >= 0).then_some(expires as u64),
                resolved_at_hour: (resolved >= 0).then_some(resolved as u64),
            });
        }
    }

    world.diplomacy.next_faction_id = world.diplomacy.next_faction_id.max(
        world
            .diplomacy
            .factions
            .iter()
            .map(|f| f.id.0)
            .max()
            .unwrap_or(0)
            .saturating_add(1),
    );
}

fn parse_country_values(world: &World, block: Option<&Block>) -> Vec<CountryId> {
    block
        .map(|b| {
            b.values
                .iter()
                .filter_map(|v| match v {
                    Value::String(tag) => Some(lookup_tag(world, tag)),
                    _ => None,
                })
                .filter(|c| !c.is_none())
                .collect()
        })
        .unwrap_or_default()
}

fn parse_wargoals(world: &World, block: Option<&Block>) -> Vec<Wargoal> {
    let Some(block) = block else {
        return Vec::new();
    };
    block
        .get_all("goal")
        .into_iter()
        .filter_map(|value| {
            let Value::Block(gb) = value else {
                return None;
            };
            let target_state = gb
                .get_int("target_state")
                .and_then(|v| (v >= 0).then_some(StateId(v as u16)));
            Some(Wargoal {
                claimant: gb
                    .get_string("claimant")
                    .map(|t| lookup_tag(world, t))
                    .unwrap_or(CountryId::NONE),
                target: gb
                    .get_string("target")
                    .map(|t| lookup_tag(world, t))
                    .unwrap_or(CountryId::NONE),
                kind: u8_to_wargoal_type(gb.get_int("kind").unwrap_or(0) as u8),
                target_state,
                justified: gb.get_bool("justified").unwrap_or(false),
                justify_progress: gb.get_float("justify_progress").unwrap_or(0.0) as f32,
                justify_total_days: gb.get_float("justify_total_days").unwrap_or(0.0) as f32,
            })
        })
        .collect()
}

fn wargoal_type_to_u8(kind: WargoalType) -> u8 {
    match kind {
        WargoalType::Annex => 0,
        WargoalType::TakeState => 1,
        WargoalType::Liberate => 2,
        WargoalType::Puppet => 3,
        WargoalType::ToppleGovernment => 4,
        WargoalType::NavalAccess => 5,
    }
}

fn u8_to_wargoal_type(v: u8) -> WargoalType {
    match v {
        1 => WargoalType::TakeState,
        2 => WargoalType::Liberate,
        3 => WargoalType::Puppet,
        4 => WargoalType::ToppleGovernment,
        5 => WargoalType::NavalAccess,
        _ => WargoalType::Annex,
    }
}

fn autonomy_level_to_u8(level: AutonomyLevel) -> u8 {
    match level {
        AutonomyLevel::Integrated => 0,
        AutonomyLevel::IntegratedPuppet => 1,
        AutonomyLevel::Puppet => 2,
        AutonomyLevel::Dominion => 3,
        AutonomyLevel::Satellite => 4,
        AutonomyLevel::FreedomAssociation => 5,
    }
}

fn u8_to_autonomy_level(v: u8) -> AutonomyLevel {
    match v {
        0 => AutonomyLevel::Integrated,
        1 => AutonomyLevel::IntegratedPuppet,
        3 => AutonomyLevel::Dominion,
        4 => AutonomyLevel::Satellite,
        5 => AutonomyLevel::FreedomAssociation,
        _ => AutonomyLevel::Puppet,
    }
}

fn treaty_kind_to_u8(kind: TreatyKind) -> u8 {
    match kind {
        TreatyKind::MilitaryAccess => 0,
        TreatyKind::NonAggressionPact => 1,
        TreatyKind::GuaranteeIndependence => 2,
        TreatyKind::FactionMembership => 3,
        TreatyKind::SubjectRelation => 4,
        TreatyKind::Truce => 5,
    }
}

fn u8_to_treaty_kind(v: u8) -> TreatyKind {
    match v {
        1 => TreatyKind::NonAggressionPact,
        2 => TreatyKind::GuaranteeIndependence,
        3 => TreatyKind::FactionMembership,
        4 => TreatyKind::SubjectRelation,
        5 => TreatyKind::Truce,
        _ => TreatyKind::MilitaryAccess,
    }
}

fn request_status_to_u8(status: DiplomaticRequestStatus) -> u8 {
    match status {
        DiplomaticRequestStatus::Pending => 0,
        DiplomaticRequestStatus::Accepted => 1,
        DiplomaticRequestStatus::Rejected => 2,
        DiplomaticRequestStatus::Expired => 3,
        DiplomaticRequestStatus::Withdrawn => 4,
    }
}

fn u8_to_request_status(v: u8) -> DiplomaticRequestStatus {
    match v {
        1 => DiplomaticRequestStatus::Accepted,
        2 => DiplomaticRequestStatus::Rejected,
        3 => DiplomaticRequestStatus::Expired,
        4 => DiplomaticRequestStatus::Withdrawn,
        _ => DiplomaticRequestStatus::Pending,
    }
}

fn state_integration_to_str(status: StateIntegrationStatus) -> &'static str {
    match status {
        StateIntegrationStatus::Metropole => "metropole",
        StateIntegrationStatus::Incorporated => "incorporated",
        StateIntegrationStatus::Colony => "colony",
        StateIntegrationStatus::Protectorate => "protectorate",
        StateIntegrationStatus::Mandate => "mandate",
        StateIntegrationStatus::Concession => "concession",
        StateIntegrationStatus::Occupied => "occupied",
    }
}

fn str_to_state_integration(raw: &str) -> StateIntegrationStatus {
    match raw {
        "metropole" | "Metropole" => StateIntegrationStatus::Metropole,
        "incorporated" | "Incorporated" => StateIntegrationStatus::Incorporated,
        "colony" | "Colony" => StateIntegrationStatus::Colony,
        "protectorate" | "Protectorate" => StateIntegrationStatus::Protectorate,
        "mandate" | "Mandate" => StateIntegrationStatus::Mandate,
        "concession" | "Concession" => StateIntegrationStatus::Concession,
        "occupied" | "Occupied" => StateIntegrationStatus::Occupied,
        _ => StateIntegrationStatus::Metropole,
    }
}

fn write_request_kind(s: &mut String, kind: &DiplomaticRequestKind, indent: &str) {
    match kind {
        DiplomaticRequestKind::InviteToFaction { faction_id } => {
            let _ = writeln!(s, "{}kind=0", indent);
            let _ = writeln!(s, "{}faction_id={}", indent, faction_id.0);
        }
        DiplomaticRequestKind::RequestMilitaryAccess => {
            let _ = writeln!(s, "{}kind=1", indent);
        }
        DiplomaticRequestKind::OfferNonAggressionPact => {
            let _ = writeln!(s, "{}kind=2", indent);
        }
        DiplomaticRequestKind::OfferPeace => {
            let _ = writeln!(s, "{}kind=3", indent);
        }
    }
}

fn parse_request_kind(block: &Block) -> DiplomaticRequestKind {
    match block.get_int("kind").unwrap_or(1) {
        0 => DiplomaticRequestKind::InviteToFaction {
            faction_id: FactionId(block.get_int("faction_id").unwrap_or(0).max(0) as u32),
        },
        2 => DiplomaticRequestKind::OfferNonAggressionPact,
        3 => DiplomaticRequestKind::OfferPeace,
        _ => DiplomaticRequestKind::RequestMilitaryAccess,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_float() {
        assert_eq!(format_float(0.0), "0.0");
        assert_eq!(format_float(1.0), "1.0");
        assert_eq!(format_float(1.5), "1.5");
        assert_eq!(format_float(0.5), "0.5");
        assert_eq!(format_float(-1.25), "-1.25");
    }

    #[test]
    fn test_strip_magic_ok() {
        let input = "HOI4txt\nfoo=1";
        let body = strip_magic(input).unwrap();
        assert!(body.starts_with("\nfoo=1"));
    }

    #[test]
    fn test_strip_magic_bad() {
        assert!(strip_magic("nope\nfoo=1").is_err());
    }

    #[test]
    fn test_meta_extract() {
        let s = "HOI4txt\n\
                 version=\"Ironheart 0.1.0\"\n\
                 date=1937.6.15.8\n\
                 player=\"GER\"\n\
                 ironman=no\n\
                 random_seed=42\n\
                 game_unique_id=999\n\
                 elapsed_hours=12500\n\
                 speed=3\n";
        let meta = read_meta_str(s).unwrap();
        assert_eq!(meta.kind, SaveKind::Text);
        assert_eq!(meta.version, "Ironheart 0.1.0");
        assert_eq!(meta.date.year, 1937);
        assert_eq!(meta.date.month, 6);
        assert_eq!(meta.date.day, 15);
        assert_eq!(meta.date.hour, 8);
        assert_eq!(meta.player_tag, "GER");
        assert_eq!(meta.random_seed, 42);
        assert_eq!(meta.game_unique_id, 999);
        assert_eq!(meta.elapsed_hours, 12500);
        assert!(!meta.ironman);
    }

    #[test]
    fn test_speed_roundtrip() {
        use crate::time::GameSpeed::*;
        for s in [Paused, Speed1, Speed2, Speed3, Speed4, Speed5] {
            let v = speed_to_int(s);
            assert_eq!(int_to_speed(v as i64), s);
        }
    }
}
