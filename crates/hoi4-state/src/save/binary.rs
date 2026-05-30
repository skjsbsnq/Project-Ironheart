//! 自定义快速二进制存档格式。
//!
//! 设计目标：
//! - 比文本格式快 1-2 个数量级（无文本解析）
//! - 体积更小（不写键名、紧凑布局）
//! - 仅供本引擎自用 —— 故格式可随版本演进
//!
//! 二进制布局（little-endian）：
//! ```text
//!   magic [8]  = "IRHRT001"
//!   format_version u32
//!   country_count u32
//!   state_count u32
//!   province_count u32
//!   year u16
//!   month u8, day u8, hour u8, speed u8
//!   elapsed_hours u64
//!   random_seed u64
//!   game_unique_id u64
//!   player u16  (CountryId raw, u16::MAX 表示 none)
//!
//!   countries × country_count:
//!     str(tag)
//!     color [u8;3]
//!     u16 capital
//!     f32 political_power
//!     f32 stability
//!     f32 war_support
//!     str(ruling_party)
//!     u64 manpower
//!     f32 fuel, fuel_capacity
//!     f32 army_xp, navy_xp, air_xp
//!     f32 army_xp, navy_xp, air_xp
//!     u8 at_war
//!     u32 n_techs, str × n_techs
//!     u32 n_ideas, str × n_ideas
//!
//!   states × state_count:
//!     u16 game_id
//!     u16 owner, controller
//!     u32 n_cores, u16 × n_cores
//!     u32 n_provs, u16 × n_provs
//!     u8 infra, civ, mil, dock
//!     u32 manpower_pool
//!     f32 resistance, compliance
//!     str(name)
//!
//!   provinces × province_count:
//!     u16 owner, controller, state_of
//!     f32 supply
//!
//!   str = u16 byte_len + bytes (UTF-8)
//! ```

use crate::diplomacy::{
    Autonomy, AutonomyLevel, DiplomaticRequest, DiplomaticRequestId, DiplomaticRequestKind,
    DiplomaticRequestStatus, Faction, OpinionMatrix, Treaty, TreatyId, TreatyKind, War, Wargoal,
    WargoalType,
};
use crate::frontline::{ArmyId, FrontlineOrder, General, GeneralId, OffensiveArrow, PlayerArmy};
use crate::ids::{CountryId, FactionId, ProvinceId, StateId};
use crate::save::{SaveError, SaveKind, SaveMeta, SaveResult, BINARY_MAGIC, ENGINE_VERSION};
use crate::store::StateIntegrationStatus;
use crate::time::{GameDate, GameSpeed};
use crate::World;

const FORMAT_VERSION: u32 = 2;

/// frontline-orders（_R10, R12_）每国可选 chunk 的魔术字与版本。
const FRARM_MAGIC: &[u8; 5] = b"FRARM";
const FRARM_VERSION: u8 = 0x02;
const GENRL_MAGIC: &[u8; 5] = b"GENRL";
const GENRL_VERSION: u8 = 0x01;
const DIPLO_MAGIC: &[u8; 5] = b"DIPLO";
const DIPLO_VERSION: u8 = 0x02;

// ─── 写入器 ─────────────────────────────────────────────────────────

struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    fn new(cap: usize) -> Self {
        Self {
            buf: Vec::with_capacity(cap),
        }
    }
    fn push_u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    fn push_u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn push_u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn push_u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn push_f32(&mut self, v: f32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn push_bytes(&mut self, b: &[u8]) {
        self.buf.extend_from_slice(b);
    }
    fn push_str(&mut self, s: &str) {
        let b = s.as_bytes();
        let n = b.len().min(u16::MAX as usize) as u16;
        self.push_u16(n);
        self.buf.extend_from_slice(&b[..n as usize]);
    }
}

// ─── 读取器 ─────────────────────────────────────────────────────────

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }
    fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }
    fn need(&self, n: usize) -> SaveResult<()> {
        if self.remaining() < n {
            Err(SaveError::Corrupt(format!(
                "unexpected EOF at offset {} (need {} bytes, remaining {})",
                self.pos,
                n,
                self.remaining()
            )))
        } else {
            Ok(())
        }
    }
    fn read_u8(&mut self) -> SaveResult<u8> {
        self.need(1)?;
        let v = self.buf[self.pos];
        self.pos += 1;
        Ok(v)
    }
    fn read_u16(&mut self) -> SaveResult<u16> {
        self.need(2)?;
        let arr: [u8; 2] = self.buf[self.pos..self.pos + 2].try_into().unwrap();
        self.pos += 2;
        Ok(u16::from_le_bytes(arr))
    }
    fn read_u32(&mut self) -> SaveResult<u32> {
        self.need(4)?;
        let arr: [u8; 4] = self.buf[self.pos..self.pos + 4].try_into().unwrap();
        self.pos += 4;
        Ok(u32::from_le_bytes(arr))
    }
    fn read_u64(&mut self) -> SaveResult<u64> {
        self.need(8)?;
        let arr: [u8; 8] = self.buf[self.pos..self.pos + 8].try_into().unwrap();
        self.pos += 8;
        Ok(u64::from_le_bytes(arr))
    }
    fn read_f32(&mut self) -> SaveResult<f32> {
        self.need(4)?;
        let arr: [u8; 4] = self.buf[self.pos..self.pos + 4].try_into().unwrap();
        self.pos += 4;
        Ok(f32::from_le_bytes(arr))
    }
    fn read_bytes(&mut self, n: usize) -> SaveResult<&'a [u8]> {
        self.need(n)?;
        let slice = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }
    fn read_str(&mut self) -> SaveResult<String> {
        let n = self.read_u16()? as usize;
        let bytes = self.read_bytes(n)?;
        String::from_utf8(bytes.to_owned())
            .map_err(|e| SaveError::Corrupt(format!("invalid UTF-8 string in save: {}", e)))
    }

    fn try_magic(&mut self, magic: &[u8]) -> SaveResult<bool> {
        if self.remaining() < magic.len() {
            return Ok(false);
        }
        if &self.buf[self.pos..self.pos + magic.len()] == magic {
            self.pos += magic.len();
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

// ─── 写入 World → bytes ─────────────────────────────────────────────

pub fn write_bytes(world: &World) -> Vec<u8> {
    // 容量估计：~80B/state + ~120B/country + 10B/province + 头部
    let cap =
        256 + world.states.count * 96 + world.countries.count * 256 + world.provinces.count * 10;
    let mut w = Writer::new(cap);

    // 头部
    w.push_bytes(BINARY_MAGIC);
    w.push_u32(FORMAT_VERSION);
    w.push_u32(world.countries.count as u32);
    w.push_u32(world.states.count as u32);
    w.push_u32(world.provinces.count as u32);

    w.push_u16(world.date.year);
    w.push_u8(world.date.month);
    w.push_u8(world.date.day);
    w.push_u8(world.date.hour);
    w.push_u8(speed_to_byte(world.speed));
    w.push_u64(world.elapsed_hours);
    w.push_u64(world.random_seed);
    w.push_u64(world.game_unique_id);
    w.push_u16(world.player.0);

    // 国家
    let n = world.countries.count;
    for i in 0..n {
        w.push_str(&world.countries.tags[i]);
        let c = world.countries.colors[i];
        w.push_u8(c[0]);
        w.push_u8(c[1]);
        w.push_u8(c[2]);
        w.push_u16(world.countries.capitals[i].0);
        w.push_f32(world.countries.political_power[i]);
        w.push_f32(world.countries.stability[i]);
        w.push_f32(world.countries.war_support[i]);
        w.push_str(&world.countries.ruling_party[i]);
        w.push_u64(world.manpower(CountryId(i as u16)));
        w.push_f32(world.countries.fuel[i]);
        w.push_f32(world.countries.fuel_capacity[i]);
        w.push_f32(world.countries.army_xp[i]);
        w.push_f32(world.countries.navy_xp[i]);
        w.push_f32(world.countries.air_xp[i]);
        w.push_u8(if world.countries.at_war[i] { 1 } else { 0 });

        let techs = &world.countries.completed_techs[i];
        w.push_u32(techs.len() as u32);
        for t in techs {
            w.push_str(t);
        }
        let ideas = &world.countries.ideas[i];
        w.push_u32(ideas.len() as u32);
        for t in ideas {
            w.push_str(t);
        }

        // ─── frontline-orders 可选 chunk（_R10.1, R12.1, R12.2_）───
        // 仅当此国家拥有至少一支 PlayerArmy 时写入；缺省即"该国无集团军"。
        write_armies_chunk(&mut w, world, CountryId(i as u16));
    }

    write_generals_chunk(&mut w, world);

    // 州（按 game_id 排序保证确定性）
    let mut state_order: Vec<(u16, StateId)> = world
        .state_id_lookup
        .iter()
        .map(|(&game_id, &sid)| (game_id, sid))
        .collect();
    state_order.sort_by_key(|x| x.0);

    for (game_id, sid) in state_order {
        let i = sid.0 as usize;
        if i >= world.states.count {
            continue;
        }
        w.push_u16(game_id);
        w.push_u16(world.states.owners[i].0);
        w.push_u16(world.states.controllers[i].0);

        let cores = &world.states.cores[i];
        w.push_u32(cores.len() as u32);
        for c in cores {
            w.push_u16(c.0);
        }

        let provs = &world.states.provinces[i];
        w.push_u32(provs.len() as u32);
        for p in provs {
            w.push_u16(p.0);
        }

        w.push_u8(world.states.infrastructure[i]);
        w.push_u32(world.states.manpower_pool[i]);
        w.push_u8(state_integration_to_u8(world.states.integration_status[i]));
        w.push_f32(world.states.resistance[i]);
        w.push_f32(world.states.compliance[i]);
        w.push_str(&world.states.names[i]);
    }

    // 省份
    for i in 0..world.provinces.count {
        w.push_u16(world.provinces.owners[i].0);
        w.push_u16(world.provinces.controllers[i].0);
        w.push_u16(world.provinces.state_of[i].0);
        w.push_f32(world.provinces.supply[i]);
    }

    write_diplomacy_chunk(&mut w, world);

    w.buf
}

// ─── 读取 bytes → 仅元数据 ─────────────────────────────────────────

pub fn read_meta_bytes(bytes: &[u8]) -> SaveResult<SaveMeta> {
    let mut r = Reader::new(bytes);
    check_magic(&mut r)?;
    let _format_version = r.read_u32()?;
    let _country_count = r.read_u32()?;
    let _state_count = r.read_u32()?;
    let _province_count = r.read_u32()?;
    let date = read_date(&mut r)?;
    let _speed = r.read_u8()?;
    let elapsed_hours = r.read_u64()?;
    let random_seed = r.read_u64()?;
    let game_unique_id = r.read_u64()?;
    let player_id = r.read_u16()?;

    // 玩家 tag 需要从 country 表第一个匹配 id 的位置取，这里只能给原始数字
    // 完整加载会在 read_bytes 里再设置。这里简单返回空字符串。
    let _ = player_id;

    Ok(SaveMeta {
        kind: SaveKind::Binary,
        version: ENGINE_VERSION.to_owned(),
        date,
        player_tag: String::new(),
        random_seed,
        game_unique_id,
        elapsed_hours,
        ironman: false,
    })
}

// ─── 读取 bytes → World ─────────────────────────────────────────────

pub fn read_bytes(bytes: &[u8], world: &mut World) -> SaveResult<()> {
    let mut r = Reader::new(bytes);
    check_magic(&mut r)?;
    let format_version = r.read_u32()?;
    if format_version > FORMAT_VERSION {
        return Err(SaveError::UnsupportedVersion(format!(
            "binary save format_version {} > supported {}",
            format_version, FORMAT_VERSION
        )));
    }

    let country_count = r.read_u32()? as usize;
    let state_count = r.read_u32()? as usize;
    let province_count = r.read_u32()? as usize;

    if country_count != world.countries.count {
        return Err(SaveError::Parse(format!(
            "country_count mismatch: save has {}, world has {}",
            country_count, world.countries.count
        )));
    }
    if state_count != world.states.count {
        return Err(SaveError::Parse(format!(
            "state_count mismatch: save has {}, world has {}",
            state_count, world.states.count
        )));
    }
    if province_count != world.provinces.count {
        return Err(SaveError::Parse(format!(
            "province_count mismatch: save has {}, world has {}",
            province_count, world.provinces.count
        )));
    }

    world.date = read_date(&mut r)?;
    world.speed = byte_to_speed(r.read_u8()?);
    world.elapsed_hours = r.read_u64()?;
    world.random_seed = r.read_u64()?;
    world.game_unique_id = r.read_u64()?;
    let player_raw = r.read_u16()?;
    world.player = CountryId(player_raw);

    // ─── frontline-orders（_R10.3_）───
    // 清空旧的 player_armies，由后续 country chunk 重新填充；
    // 旧档不含 FRARM chunk -> world.player_armies 留空。
    world.player_armies.clear();
    world.next_army_id = 0;
    world.generals.clear();
    world.next_general_id = 0;

    // ─── 国家 ───
    // 二进制存档假定国家顺序与 World 内部顺序一致（World 创建时按 tag 字典序排）
    // 但为安全起见，我们用读到的 tag 反查 CountryId
    for _ in 0..country_count {
        let tag = r.read_str()?;
        let r0 = r.read_u8()?;
        let g0 = r.read_u8()?;
        let b0 = r.read_u8()?;
        let capital = r.read_u16()?;
        let pp = r.read_f32()?;
        let stab = r.read_f32()?;
        let ws = r.read_f32()?;
        let rp = r.read_str()?;
        let _mp = r.read_u64()?;
        let fuel = r.read_f32()?;
        let fuel_cap = r.read_f32()?;
        let axp = r.read_f32()?;
        let nxp = r.read_f32()?;
        let aixp = r.read_f32()?;
        let aw = r.read_u8()? != 0;

        let n_techs = r.read_u32()? as usize;
        let mut techs = Vec::with_capacity(n_techs);
        for _ in 0..n_techs {
            techs.push(r.read_str()?);
        }
        let n_ideas = r.read_u32()? as usize;
        let mut ideas = Vec::with_capacity(n_ideas);
        for _ in 0..n_ideas {
            ideas.push(r.read_str()?);
        }

        // ─── frontline-orders 可选 chunk（_R10.3_）───
        // 旧档不含此 chunk -> 跳过；新档读出 PlayerArmy 列表，稍后归属到该国家。
        let armies_for_country = read_armies_chunk(&mut r)?;

        let cid = match world.tag_to_country.get(&tag) {
            Some(&id) => id,
            None => {
                // 未知 tag — 跳过此国（包括它的 armies chunk，已在上面消费完）。
                continue;
            }
        };
        let i = cid.0 as usize;
        world.countries.colors[i] = [r0, g0, b0];
        world.countries.capitals[i] = StateId(capital);
        world.countries.political_power[i] = pp;
        world.countries.stability[i] = stab;
        world.countries.war_support[i] = ws;
        world.countries.ruling_party[i] = rp;
        // manpower loaded from save but now derived from PopGroups; ignore the loaded value
        world.countries.fuel[i] = fuel;
        world.countries.fuel_capacity[i] = fuel_cap;
        world.countries.army_xp[i] = axp;
        world.countries.navy_xp[i] = nxp;
        world.countries.air_xp[i] = aixp;
        world.countries.at_war[i] = aw;
        world.countries.completed_techs[i] = techs;
        world.countries.ideas[i] = ideas;

        // 把 owner=cid 标记到刚刚读到的 armies 上，并 push 进 world。
        // 越界成员、非法 path 等在 `sanitize_army` 中清洗（_R10.4, R12.3_）。
        for mut a in armies_for_country {
            a.owner = cid;
            sanitize_army(world, &mut a);
            world.player_armies.push(a);
        }
    }

    if !read_generals_chunk(&mut r, world)? {
        world.seed_default_generals();
    }
    sanitize_army_commanders(world);

    // ─── 州 ───
    for _ in 0..state_count {
        let game_id = r.read_u16()?;
        let owner = r.read_u16()?;
        let controller = r.read_u16()?;

        let n_cores = r.read_u32()? as usize;
        let mut cores = Vec::with_capacity(n_cores);
        for _ in 0..n_cores {
            cores.push(CountryId(r.read_u16()?));
        }
        let n_provs = r.read_u32()? as usize;
        let mut provs = Vec::with_capacity(n_provs);
        for _ in 0..n_provs {
            provs.push(ProvinceId(r.read_u16()?));
        }
        let infra = r.read_u8()?;
        let mp = r.read_u32()?;
        let integration = if format_version >= 2 {
            u8_to_state_integration(r.read_u8()?)
        } else {
            StateIntegrationStatus::Metropole
        };
        let resistance = r.read_f32()?;
        let compliance = r.read_f32()?;
        let name = r.read_str()?;

        let sid = match world.state_id_lookup.get(&game_id) {
            Some(&id) => id,
            None => continue,
        };
        let i = sid.0 as usize;
        world.states.owners[i] = CountryId(owner);
        world.states.controllers[i] = CountryId(controller);
        world.states.cores[i] = cores;
        world.states.provinces[i] = provs;
        world.states.infrastructure[i] = infra;
        world.states.manpower_pool[i] = mp;
        world.states.integration_status[i] = integration;
        world.states.resistance[i] = resistance;
        world.states.compliance[i] = compliance;
        world.states.names[i] = name;
    }

    // ─── 省份 ───
    for i in 0..province_count {
        world.provinces.owners[i] = CountryId(r.read_u16()?);
        world.provinces.controllers[i] = CountryId(r.read_u16()?);
        world.provinces.state_of[i] = StateId(r.read_u16()?);
        world.provinces.supply[i] = r.read_f32()?;
    }

    read_diplomacy_chunk(&mut r, world)?;

    // ─── frontline-orders（_R10.1, R10.2_）───
    // 按 ArmyId.0 升序排序，并把 next_army_id 推到 max+1（与文本格式一致）。
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

fn write_diplomacy_chunk(w: &mut Writer, world: &World) {
    w.push_bytes(DIPLO_MAGIC);
    w.push_u8(DIPLO_VERSION);
    w.push_f32(world.diplomacy.world_tension);
    w.push_u32(world.diplomacy.next_war_id);
    w.push_u32(world.diplomacy.next_faction_id);

    w.push_u32(world.diplomacy.factions.len() as u32);
    for f in &world.diplomacy.factions {
        w.push_u32(f.id.0);
        w.push_str(&f.name);
        w.push_u16(f.leader.0);
        w.push_u64(f.created_at_hour);
        w.push_u32(f.members.len() as u32);
        for member in &f.members {
            w.push_u16(member.0);
        }
    }

    let mut wars: Vec<_> = world.diplomacy.wars.iter().collect();
    wars.sort_by_key(|(id, _)| **id);
    w.push_u32(wars.len() as u32);
    for (id, war) in wars {
        w.push_u32(*id);
        w.push_u16(war.primary_attacker.0);
        w.push_u16(war.primary_defender.0);
        w.push_u64(war.started_at_hour);
        w.push_f32(war.attacker_war_score);
        w.push_f32(war.defender_war_score);
        write_country_set(w, &war.attackers);
        write_country_set(w, &war.defenders);
        write_wargoals(w, &war.attacker_wargoals);
        write_wargoals(w, &war.defender_wargoals);
    }

    let mut autonomy: Vec<_> = world.diplomacy.autonomy.iter().collect();
    autonomy.sort_by_key(|(subject, _)| subject.0);
    w.push_u32(autonomy.len() as u32);
    for (subject, a) in autonomy {
        w.push_u16(subject.0);
        w.push_u16(a.master.0);
        w.push_u8(autonomy_level_to_u8(a.level));
        w.push_f32(a.progress);
        w.push_u64(a.since_hour);
    }

    let mut pending: Vec<_> = world.diplomacy.pending_wargoals.iter().collect();
    pending.sort_by_key(|(claimant, _)| claimant.0);
    w.push_u32(pending.len() as u32);
    for (claimant, goals) in pending {
        w.push_u16(claimant.0);
        write_wargoals(w, goals);
    }

    let mut opinions: Vec<_> = world.diplomacy.opinions.opinions.iter().collect();
    opinions.sort_by_key(|((from, to), _)| (from.0, to.0));
    w.push_u32(opinions.len() as u32);
    for ((from, to), value) in opinions {
        w.push_u16(from.0);
        w.push_u16(to.0);
        w.push_u16(*value as u16);
    }

    write_country_set(w, &world.diplomacy.annexed_countries);

    let mut access: Vec<_> = world.diplomacy.military_access.iter().copied().collect();
    access.sort_unstable();
    w.push_u32(access.len() as u32);
    for (grantor, grantee) in access {
        w.push_u16(grantor);
        w.push_u16(grantee);
    }

    w.push_u32(world.diplomacy.next_treaty_id);
    w.push_u32(world.diplomacy.treaties.len() as u32);
    for treaty in &world.diplomacy.treaties {
        w.push_u32(treaty.id.0);
        w.push_u8(treaty_kind_to_u8(treaty.kind));
        w.push_u64(treaty.since_hour);
        w.push_u64(treaty.expires_at_hour.unwrap_or(u64::MAX));
        w.push_u32(treaty.parties.len() as u32);
        for party in &treaty.parties {
            w.push_u16(party.0);
        }
    }

    w.push_u32(world.diplomacy.next_diplomatic_request_id);
    w.push_u32(world.diplomacy.diplomatic_requests.len() as u32);
    for request in &world.diplomacy.diplomatic_requests {
        w.push_u32(request.id.0);
        w.push_u16(request.from.0);
        w.push_u16(request.to.0);
        write_request_kind(w, &request.kind);
        w.push_u8(request_status_to_u8(request.status));
        w.push_u64(request.created_at_hour);
        w.push_u64(request.expires_at_hour.unwrap_or(u64::MAX));
        w.push_u64(request.resolved_at_hour.unwrap_or(u64::MAX));
    }
}

fn read_diplomacy_chunk(r: &mut Reader<'_>, world: &mut World) -> SaveResult<()> {
    if !r.try_magic(DIPLO_MAGIC)? {
        return Ok(());
    }
    let version = r.read_u8()?;
    if version != DIPLO_VERSION {
        return Err(SaveError::UnsupportedVersion(format!(
            "diplomacy chunk version {} != expected {}",
            version, DIPLO_VERSION
        )));
    }

    world.diplomacy.world_tension = r.read_f32()?;
    world.diplomacy.next_war_id = r.read_u32()?;
    world.diplomacy.next_faction_id = r.read_u32()?;
    world.diplomacy.factions.clear();
    world.diplomacy.wars.clear();
    world.diplomacy.autonomy.clear();
    world.diplomacy.pending_wargoals.clear();
    world.diplomacy.opinions = OpinionMatrix::new();
    world.diplomacy.annexed_countries.clear();
    world.diplomacy.military_access.clear();
    world.diplomacy.treaties.clear();
    world.diplomacy.diplomatic_requests.clear();

    let factions = r.read_u32()? as usize;
    for _ in 0..factions {
        let id = FactionId(r.read_u32()?);
        let name = r.read_str()?;
        let leader = CountryId(r.read_u16()?);
        let created_at_hour = r.read_u64()?;
        let members = read_country_vec(r)?;
        world.diplomacy.factions.push(Faction {
            id,
            name,
            leader,
            members,
            created_at_hour,
        });
    }

    let wars = r.read_u32()? as usize;
    for _ in 0..wars {
        let id = r.read_u32()?;
        let primary_attacker = CountryId(r.read_u16()?);
        let primary_defender = CountryId(r.read_u16()?);
        let started_at_hour = r.read_u64()?;
        let attacker_war_score = r.read_f32()?;
        let defender_war_score = r.read_f32()?;
        let attackers = read_country_vec(r)?.into_iter().collect();
        let defenders = read_country_vec(r)?.into_iter().collect();
        let attacker_wargoals = read_wargoals(r)?;
        let defender_wargoals = read_wargoals(r)?;
        world.diplomacy.wars.insert(
            id,
            War {
                id,
                primary_attacker,
                primary_defender,
                attackers,
                defenders,
                started_at_hour,
                attacker_war_score,
                defender_war_score,
                attacker_wargoals,
                defender_wargoals,
                war_join_policies: std::collections::HashMap::new(),
            },
        );
    }

    let autonomy = r.read_u32()? as usize;
    for _ in 0..autonomy {
        let subject = CountryId(r.read_u16()?);
        let master = CountryId(r.read_u16()?);
        let level = u8_to_autonomy_level(r.read_u8()?);
        let progress = r.read_f32()?;
        let since_hour = r.read_u64()?;
        world.diplomacy.autonomy.insert(
            subject,
            Autonomy {
                master,
                subject,
                level,
                progress,
                since_hour,
            },
        );
    }

    let pending = r.read_u32()? as usize;
    for _ in 0..pending {
        let claimant = CountryId(r.read_u16()?);
        let goals = read_wargoals(r)?;
        world.diplomacy.pending_wargoals.insert(claimant, goals);
    }

    let opinions = r.read_u32()? as usize;
    for _ in 0..opinions {
        let from = CountryId(r.read_u16()?);
        let to = CountryId(r.read_u16()?);
        let value = r.read_u16()? as i16;
        world.diplomacy.opinions.set(from, to, value);
    }

    world.diplomacy.annexed_countries = read_country_vec(r)?.into_iter().collect();

    let access = r.read_u32()? as usize;
    for _ in 0..access {
        let grantor = r.read_u16()?;
        let grantee = r.read_u16()?;
        world.diplomacy.military_access.insert((grantor, grantee));
    }

    world.diplomacy.next_treaty_id = r.read_u32()?;
    let treaties = r.read_u32()? as usize;
    for _ in 0..treaties {
        let id = TreatyId(r.read_u32()?);
        let kind = u8_to_treaty_kind(r.read_u8()?);
        let since_hour = r.read_u64()?;
        let expires_raw = r.read_u64()?;
        let parties = read_country_vec(r)?;
        world.diplomacy.treaties.push(Treaty {
            id,
            kind,
            parties,
            since_hour,
            expires_at_hour: (expires_raw != u64::MAX).then_some(expires_raw),
        });
    }

    world.diplomacy.next_diplomatic_request_id = r.read_u32()?;
    let requests = r.read_u32()? as usize;
    for _ in 0..requests {
        let id = DiplomaticRequestId(r.read_u32()?);
        let from = CountryId(r.read_u16()?);
        let to = CountryId(r.read_u16()?);
        let kind = read_request_kind(r)?;
        let status = u8_to_request_status(r.read_u8()?);
        let created_at_hour = r.read_u64()?;
        let expires_raw = r.read_u64()?;
        let resolved_raw = r.read_u64()?;
        world.diplomacy.diplomatic_requests.push(DiplomaticRequest {
            id,
            from,
            to,
            kind,
            status,
            created_at_hour,
            expires_at_hour: (expires_raw != u64::MAX).then_some(expires_raw),
            resolved_at_hour: (resolved_raw != u64::MAX).then_some(resolved_raw),
        });
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

    Ok(())
}

fn write_country_set(w: &mut Writer, countries: &std::collections::HashSet<CountryId>) {
    let mut values: Vec<_> = countries.iter().copied().collect();
    values.sort_by_key(|c| c.0);
    w.push_u32(values.len() as u32);
    for c in values {
        w.push_u16(c.0);
    }
}

fn read_country_vec(r: &mut Reader<'_>) -> SaveResult<Vec<CountryId>> {
    let n = r.read_u32()? as usize;
    let mut values = Vec::with_capacity(n);
    for _ in 0..n {
        values.push(CountryId(r.read_u16()?));
    }
    Ok(values)
}

fn write_wargoals(w: &mut Writer, goals: &[Wargoal]) {
    w.push_u32(goals.len() as u32);
    for goal in goals {
        w.push_u16(goal.claimant.0);
        w.push_u16(goal.target.0);
        w.push_u8(wargoal_type_to_u8(goal.kind));
        w.push_u16(goal.target_state.map(|s| s.0).unwrap_or(u16::MAX));
        w.push_u8(if goal.justified { 1 } else { 0 });
        w.push_f32(goal.justify_progress);
        w.push_f32(goal.justify_total_days);
    }
}

fn read_wargoals(r: &mut Reader<'_>) -> SaveResult<Vec<Wargoal>> {
    let n = r.read_u32()? as usize;
    let mut goals = Vec::with_capacity(n);
    for _ in 0..n {
        goals.push(Wargoal {
            claimant: CountryId(r.read_u16()?),
            target: CountryId(r.read_u16()?),
            kind: u8_to_wargoal_type(r.read_u8()?),
            target_state: match r.read_u16()? {
                u16::MAX => None,
                raw => Some(StateId(raw)),
            },
            justified: r.read_u8()? != 0,
            justify_progress: r.read_f32()?,
            justify_total_days: r.read_f32()?,
        });
    }
    Ok(goals)
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

fn state_integration_to_u8(status: StateIntegrationStatus) -> u8 {
    match status {
        StateIntegrationStatus::Metropole => 0,
        StateIntegrationStatus::Incorporated => 1,
        StateIntegrationStatus::Colony => 2,
        StateIntegrationStatus::Protectorate => 3,
        StateIntegrationStatus::Mandate => 4,
        StateIntegrationStatus::Concession => 5,
        StateIntegrationStatus::Occupied => 6,
    }
}

fn u8_to_state_integration(raw: u8) -> StateIntegrationStatus {
    match raw {
        0 => StateIntegrationStatus::Metropole,
        1 => StateIntegrationStatus::Incorporated,
        2 => StateIntegrationStatus::Colony,
        3 => StateIntegrationStatus::Protectorate,
        4 => StateIntegrationStatus::Mandate,
        5 => StateIntegrationStatus::Concession,
        6 => StateIntegrationStatus::Occupied,
        _ => StateIntegrationStatus::Metropole,
    }
}

fn write_request_kind(w: &mut Writer, kind: &DiplomaticRequestKind) {
    match kind {
        DiplomaticRequestKind::InviteToFaction { faction_id } => {
            w.push_u8(0);
            w.push_u32(faction_id.0);
        }
        DiplomaticRequestKind::RequestMilitaryAccess => w.push_u8(1),
        DiplomaticRequestKind::OfferNonAggressionPact => w.push_u8(2),
        DiplomaticRequestKind::OfferPeace => w.push_u8(3),
    }
}

fn read_request_kind(r: &mut Reader<'_>) -> SaveResult<DiplomaticRequestKind> {
    Ok(match r.read_u8()? {
        0 => DiplomaticRequestKind::InviteToFaction {
            faction_id: FactionId(r.read_u32()?),
        },
        2 => DiplomaticRequestKind::OfferNonAggressionPact,
        3 => DiplomaticRequestKind::OfferPeace,
        _ => DiplomaticRequestKind::RequestMilitaryAccess,
    })
}

// ─── helpers ────────────────────────────────────────────────────────

fn check_magic(r: &mut Reader<'_>) -> SaveResult<()> {
    let m = r.read_bytes(BINARY_MAGIC.len())?;
    if m != BINARY_MAGIC {
        return Err(SaveError::BadMagic(format!(
            "expected binary magic {:?}, got {:?}",
            BINARY_MAGIC, m
        )));
    }
    Ok(())
}

fn read_date(r: &mut Reader<'_>) -> SaveResult<GameDate> {
    let year = r.read_u16()?;
    let month = r.read_u8()?;
    let day = r.read_u8()?;
    let hour = r.read_u8()?;
    Ok(GameDate {
        year,
        month,
        day,
        hour,
    })
}

fn speed_to_byte(s: GameSpeed) -> u8 {
    match s {
        GameSpeed::Paused => 0,
        GameSpeed::Speed1 => 1,
        GameSpeed::Speed2 => 2,
        GameSpeed::Speed3 => 3,
        GameSpeed::Speed4 => 4,
        GameSpeed::Speed5 => 5,
    }
}

fn byte_to_speed(v: u8) -> GameSpeed {
    match v {
        1 => GameSpeed::Speed1,
        2 => GameSpeed::Speed2,
        3 => GameSpeed::Speed3,
        4 => GameSpeed::Speed4,
        5 => GameSpeed::Speed5,
        _ => GameSpeed::Paused,
    }
}

// ─── frontline-orders chunk（_R10, R12_）─────────────────────────────
//
// 在每个 country 块的尾部，可选地追加一个版本化 chunk：
//
// ```text
//   magic       [u8; 5]   = b"FRARM"
//   version     u8        = 0x01
//   army_count  u32 (LE)
//   for each army:
//     army_id        u32 (LE)
//     name_len       u32 (LE)
//     name_bytes     [u8; name_len]   (UTF-8)
//     members_count  u32 (LE)
//     members        [u32; members_count] (LE; DivisionStore 索引)
//     has_order      u8 (0 or 1)
//     if has_order == 1:
//       path_count   u16 (LE)
//       path         [u16; path_count]    (ProvinceId)
//       has_arrow    u8
//       if has_arrow == 1:
//         arrow_count u16 (LE)
//         arrow       [u16; arrow_count]
//       has_anchor   u8
//       if has_anchor == 1:
//         anchor     u16 (LE)
//       active       u8 (0 or 1)
// ```
//
// 不含此 chunk 的旧档（在 `ideas` 之后直接进入下一个 country/state）→
// reader 看到 magic 不匹配，把读位置回退；该 country 的 armies 留空。
//
// 编码风格：完全沿用本文件既有的 little-endian 定宽（u32/u16/u8），
// 不引入 LEB128 与新依赖（与 design.md "二进制格式扩展"一致）。

/// 写一个 country 的 FRARM chunk。若该 country 没有任何 PlayerArmy，
/// 完全不写（让"缺省 = 该国无集团军"成为格式约定）。
fn write_armies_chunk(w: &mut Writer, world: &World, owner: CountryId) {
    // 收集 owner==cid 的索引并按 ArmyId.0 升序排序，保证确定性输出（_R12.2_）。
    let mut order: Vec<usize> = world
        .player_armies
        .iter()
        .enumerate()
        .filter(|(_, a)| a.owner == owner)
        .map(|(idx, _)| idx)
        .collect();
    if order.is_empty() {
        return;
    }
    order.sort_by_key(|&idx| world.player_armies[idx].id.0);

    w.push_bytes(FRARM_MAGIC);
    w.push_u8(FRARM_VERSION);
    w.push_u32(order.len() as u32);

    for ai in order {
        let army = &world.player_armies[ai];
        w.push_u32(army.id.0);
        w.push_str(&army.name);
        w.push_u32(army.commander.map(|id| id.0).unwrap_or(u32::MAX));
        w.push_u32(army.members.len() as u32);
        for &m in &army.members {
            w.push_u32(m as u32);
        }

        if let Some(o) = &army.order {
            w.push_u8(1);
            w.push_u16(o.path.len() as u16);
            for p in &o.path {
                w.push_u16(p.0);
            }
            if let Some(arr) = &o.arrow {
                w.push_u8(1);
                w.push_u16(arr.provinces.len() as u16);
                for p in &arr.provinces {
                    w.push_u16(p.0);
                }
            } else {
                w.push_u8(0);
            }
            if let Some(a) = o.anchor {
                w.push_u8(1);
                w.push_u16(a.0);
            } else {
                w.push_u8(0);
            }
            w.push_u8(if o.active { 1 } else { 0 });
            w.push_u8(if o.executing { 1 } else { 0 });
        } else {
            w.push_u8(0);
        }
    }
}

/// 尝试在当前位置读一个 FRARM chunk。如果 magic 不匹配，把读位置回退
/// 并返回空向量（表示"该国无 armies chunk"，对应旧档兼容路径，_R10.3_）。
///
/// 返回的 `PlayerArmy` 列表中 `owner` 字段是占位 [`CountryId::NONE`]；
/// 由调用方根据上下文（当前 country）写回真实 owner。
///
/// 越界成员（_R10.4_）与非法 path/arrow（_R12.3_）的清洗在 [`sanitize_army`]
/// 内做（需要 `&World`），不在 chunk 解码阶段做。
fn read_armies_chunk(r: &mut Reader<'_>) -> SaveResult<Vec<PlayerArmy>> {
    // peek magic + version (5 + 1 = 6 bytes)
    if r.remaining() < FRARM_MAGIC.len() + 1 {
        return Ok(Vec::new());
    }
    let head = &r.buf[r.pos..r.pos + FRARM_MAGIC.len()];
    if head != FRARM_MAGIC {
        return Ok(Vec::new());
    }
    let version = r.buf[r.pos + FRARM_MAGIC.len()];
    if version != 0x01 && version != FRARM_VERSION {
        // 未来其它版本：保守起见不消费、不解析；当前识别 v1/v2。
        return Ok(Vec::new());
    }
    // commit 消费 magic+version
    r.pos += FRARM_MAGIC.len() + 1;

    let army_count = r.read_u32()? as usize;
    let mut out = Vec::with_capacity(army_count);

    for _ in 0..army_count {
        let raw_id = r.read_u32()?;
        let name = r.read_str()?;
        let commander = if version >= 0x02 {
            let commander_raw = r.read_u32()?;
            (commander_raw != u32::MAX).then_some(GeneralId(commander_raw))
        } else {
            None
        };

        let n_members = r.read_u32()? as usize;
        let mut members: Vec<usize> = Vec::with_capacity(n_members);
        for _ in 0..n_members {
            members.push(r.read_u32()? as usize);
        }

        let has_order = r.read_u8()? != 0;
        let order = if has_order {
            let path_count = r.read_u16()? as usize;
            let mut path: Vec<ProvinceId> = Vec::with_capacity(path_count);
            for _ in 0..path_count {
                path.push(ProvinceId(r.read_u16()?));
            }

            let has_arrow = r.read_u8()? != 0;
            let arrow = if has_arrow {
                let arrow_count = r.read_u16()? as usize;
                let mut provinces: Vec<ProvinceId> = Vec::with_capacity(arrow_count);
                for _ in 0..arrow_count {
                    provinces.push(ProvinceId(r.read_u16()?));
                }
                Some(OffensiveArrow { provinces })
            } else {
                None
            };

            let has_anchor = r.read_u8()? != 0;
            let anchor = if has_anchor {
                Some(ProvinceId(r.read_u16()?))
            } else {
                None
            };

            let active = r.read_u8()? != 0;
            let executing = r.read_u8()? != 0;

            Some(FrontlineOrder {
                path,
                arrow,
                anchor,
                active,
                executing,
            })
        } else {
            None
        };

        out.push(PlayerArmy {
            id: ArmyId(raw_id),
            name,
            owner: CountryId::NONE, // 由调用方写回真实 owner
            commander,
            members,
            order,
        });
    }

    Ok(out)
}

fn write_generals_chunk(w: &mut Writer, world: &World) {
    w.push_bytes(GENRL_MAGIC);
    w.push_u8(GENRL_VERSION);
    w.push_u32(world.generals.len() as u32);
    let mut generals: Vec<&General> = world.generals.iter().collect();
    generals.sort_by_key(|g| g.id.0);
    for general in generals {
        w.push_u32(general.id.0);
        w.push_u16(general.owner.0);
        w.push_str(&general.name);
        w.push_u8(general.skill);
        w.push_u8(general.attack);
        w.push_u8(general.defense);
        w.push_u8(general.planning);
        w.push_u8(general.logistics);
        w.push_u16(general.command_limit);
    }
}

fn read_generals_chunk(r: &mut Reader<'_>, world: &mut World) -> SaveResult<bool> {
    if r.remaining() < GENRL_MAGIC.len() + 1 {
        return Ok(false);
    }
    let head = &r.buf[r.pos..r.pos + GENRL_MAGIC.len()];
    if head != GENRL_MAGIC {
        return Ok(false);
    }
    let version = r.buf[r.pos + GENRL_MAGIC.len()];
    if version != GENRL_VERSION {
        return Ok(false);
    }
    r.pos += GENRL_MAGIC.len() + 1;
    let count = r.read_u32()? as usize;
    for _ in 0..count {
        let id = GeneralId(r.read_u32()?);
        let owner = CountryId(r.read_u16()?);
        let name = r.read_str()?;
        let skill = r.read_u8()?;
        let attack = r.read_u8()?;
        let defense = r.read_u8()?;
        let planning = r.read_u8()?;
        let logistics = r.read_u8()?;
        let command_limit = r.read_u16()?;
        if !owner.is_none() && (owner.0 as usize) < world.countries.count {
            world.generals.push(General {
                id,
                owner,
                name,
                skill,
                attack,
                defense,
                planning,
                logistics,
                command_limit,
            });
        }
    }
    Ok(true)
}

/// 应用文本读路径的 R10.4 / R12.3 校验：
/// - 越界成员（`i >= world.divisions.count`）丢弃并 emit `tracing::warn!("过期成员")`。
/// - 路径/箭头 id 越界 / 不相邻 / 重复 → 丢整个 `order`，保留 `PlayerArmy` 外壳，
///   emit `tracing::warn!("存档战线已损坏，已重置")`。
fn sanitize_army(world: &World, army: &mut PlayerArmy) {
    // 成员越界丢弃（_R10.4_）
    let div_count = world.divisions.count;
    let raw_id = army.id.0;
    army.members.retain(|&idx| {
        if idx >= div_count {
            tracing::warn!(
                target: "frontline_save",
                army_id = raw_id,
                idx = idx,
                "过期成员"
            );
            false
        } else {
            true
        }
    });

    // path / arrow 校验（_R12.3_）
    if let Some(o) = &army.order {
        let path_ok = !o.path.is_empty()
            && crate::save::text::validate_save_path(world, &o.path)
            && crate::save::text::no_dups(&o.path);
        let arrow_ok = o
            .arrow
            .as_ref()
            .map(|arr| crate::save::text::validate_save_path(world, &arr.provinces))
            .unwrap_or(true);
        if !path_ok || !arrow_ok {
            tracing::warn!(
                target: "frontline_save",
                army_id = raw_id,
                "存档战线已损坏，已重置"
            );
            army.order = None;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writer_basic() {
        let mut w = Writer::new(0);
        w.push_u8(0x12);
        w.push_u16(0xABCD);
        w.push_u32(0xDEAD_BEEF);
        w.push_u64(0x0102_0304_0506_0708);
        w.push_f32(1.5);
        w.push_str("hi");
        let mut r = Reader::new(&w.buf);
        assert_eq!(r.read_u8().unwrap(), 0x12);
        assert_eq!(r.read_u16().unwrap(), 0xABCD);
        assert_eq!(r.read_u32().unwrap(), 0xDEAD_BEEF);
        assert_eq!(r.read_u64().unwrap(), 0x0102_0304_0506_0708);
        assert_eq!(r.read_f32().unwrap(), 1.5);
        assert_eq!(r.read_str().unwrap(), "hi");
    }

    #[test]
    fn reader_eof() {
        let buf = vec![1u8, 2, 3];
        let mut r = Reader::new(&buf);
        let _ = r.read_u8().unwrap();
        let _ = r.read_u16().unwrap();
        assert!(r.read_u8().is_err());
    }

    #[test]
    fn speed_roundtrip() {
        for s in [
            GameSpeed::Paused,
            GameSpeed::Speed1,
            GameSpeed::Speed2,
            GameSpeed::Speed3,
            GameSpeed::Speed4,
            GameSpeed::Speed5,
        ] {
            assert_eq!(byte_to_speed(speed_to_byte(s)), s);
        }
    }

    #[test]
    fn bad_magic_rejected() {
        let bytes = b"NOTAGOODMAGIC";
        let mut r = Reader::new(bytes);
        assert!(check_magic(&mut r).is_err());
    }
}
