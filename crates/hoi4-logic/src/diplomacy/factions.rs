//! 阵营操作 — 创建 / 加入 / 退出 / 踢出 / 解散 / 转让领导。

use hoi4_state::{CountryId, Faction, FactionId, World};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FactionError {
    /// 国家无效
    BadCountry,
    /// 国家已经在另一阵营
    AlreadyInFaction,
    /// 阵营索引无效
    BadFaction,
    /// 不是该阵营领袖
    NotLeader,
    /// 国家不在该阵营
    NotMember,
    /// 阵营名重复
    DuplicateName,
}

/// 用于 dry-run 之类的 op 标识
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactionOp {
    Create,
    Join,
    Leave,
    Kick,
    Dissolve,
    TransferLeadership,
}

/// 创建一个新阵营，由 leader 国领导。
///
/// 要求：leader 是有效国家，且当前不在任何阵营。
pub fn create_faction(
    world: &mut World,
    leader: CountryId,
    name: impl Into<String>,
) -> Result<FactionId, FactionError> {
    if leader.is_none() || leader.0 as usize >= world.countries.count {
        return Err(FactionError::BadCountry);
    }
    if world.diplomacy.faction_of(leader).is_some() {
        return Err(FactionError::AlreadyInFaction);
    }
    let name = name.into();
    if world.diplomacy.factions.iter().any(|f| f.name == name) {
        return Err(FactionError::DuplicateName);
    }
    let id = world.diplomacy.allocate_faction_id();
    world.diplomacy.factions.push(Faction {
        id,
        name,
        leader,
        members: vec![leader],
        created_at_hour: world.elapsed_hours,
    });
    Ok(id)
}

/// 让一国加入指定阵营（被邀请且接受邀请的简化版本）。
pub fn join_faction(
    world: &mut World,
    faction: FactionId,
    member: CountryId,
) -> Result<(), FactionError> {
    if member.is_none() || member.0 as usize >= world.countries.count {
        return Err(FactionError::BadCountry);
    }
    if world.diplomacy.faction_of(member).is_some() {
        return Err(FactionError::AlreadyInFaction);
    }
    let f = world
        .diplomacy
        .faction_mut(faction)
        .ok_or(FactionError::BadFaction)?;
    f.members.push(member);
    Ok(())
}

/// 一国主动退出阵营。
pub fn leave_faction(
    world: &mut World,
    faction: FactionId,
    member: CountryId,
) -> Result<(), FactionError> {
    let f = world
        .diplomacy
        .faction_mut(faction)
        .ok_or(FactionError::BadFaction)?;
    let pos = f
        .members
        .iter()
        .position(|&m| m == member)
        .ok_or(FactionError::NotMember)?;
    // 领袖退出 → 整个阵营解散
    if f.leader == member {
        world.diplomacy.factions.retain(|f| f.id != faction);
        return Ok(());
    }
    f.members.remove(pos);
    Ok(())
}

/// 阵营领袖踢出某成员。
pub fn kick_member(
    world: &mut World,
    faction: FactionId,
    leader: CountryId,
    target: CountryId,
) -> Result<(), FactionError> {
    let f = world
        .diplomacy
        .faction_mut(faction)
        .ok_or(FactionError::BadFaction)?;
    if f.leader != leader {
        return Err(FactionError::NotLeader);
    }
    if target == leader {
        // 不能踢自己
        return Err(FactionError::NotMember);
    }
    let pos = f
        .members
        .iter()
        .position(|&m| m == target)
        .ok_or(FactionError::NotMember)?;
    f.members.remove(pos);
    Ok(())
}

/// 解散阵营（仅领袖可以）。
pub fn dissolve_faction(
    world: &mut World,
    faction: FactionId,
    leader: CountryId,
) -> Result<(), FactionError> {
    let f = world
        .diplomacy
        .faction(faction)
        .ok_or(FactionError::BadFaction)?;
    if f.leader != leader {
        return Err(FactionError::NotLeader);
    }
    world.diplomacy.factions.retain(|f| f.id != faction);
    Ok(())
}

/// 转让阵营领导权。
pub fn transfer_leadership(
    world: &mut World,
    faction: FactionId,
    current_leader: CountryId,
    new_leader: CountryId,
) -> Result<(), FactionError> {
    let f = world
        .diplomacy
        .faction_mut(faction)
        .ok_or(FactionError::BadFaction)?;
    if f.leader != current_leader {
        return Err(FactionError::NotLeader);
    }
    if !f.contains(new_leader) {
        return Err(FactionError::NotMember);
    }
    f.leader = new_leader;
    Ok(())
}
