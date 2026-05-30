//! 傀儡 / 自治度。
//!
//! 模型：
//! - master 把某 subject 设为 [`AutonomyLevel`]，存入 `world.diplomacy.autonomy`
//! - 每日：subject 累积 `AUTONOMY_DAILY_BASE` × (subject 工业力 / master 工业力) 的进度
//! - 进度达到 [`AutonomyLevel::upgrade_threshold`] 即升级（更独立）
//! - 反向：master 可手工"压低"自治度（非战争 effect 暂不实现）
//! - 整合（subject 进度低于 [`INTEGRATION_BELOW`]）：可被 master 吞并

use hoi4_state::{Autonomy, AutonomyLevel, BuildingKind, CountryId, World};

use super::constants::{AUTONOMY_DAILY_BASE, INTEGRATION_BELOW};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutonomyError {
    BadCountry,
    /// subject 已经是某主国的傀儡
    AlreadySubject,
    /// master == subject
    SelfPuppet,
    /// 关系不存在
    NoRelation,
}

/// 设定一国为另一国的傀儡。
///
/// 如果 subject 已是其它 master 的傀儡，返回 [`AutonomyError::AlreadySubject`]。
pub fn set_puppet(
    world: &mut World,
    master: CountryId,
    subject: CountryId,
    initial_level: AutonomyLevel,
) -> Result<(), AutonomyError> {
    if master == subject {
        return Err(AutonomyError::SelfPuppet);
    }
    if master.is_none() || master.0 as usize >= world.countries.count {
        return Err(AutonomyError::BadCountry);
    }
    if subject.is_none() || subject.0 as usize >= world.countries.count {
        return Err(AutonomyError::BadCountry);
    }
    if let Some(existing) = world.diplomacy.autonomy.get(&subject) {
        if existing.master != master {
            return Err(AutonomyError::AlreadySubject);
        }
    }
    world.diplomacy.autonomy.insert(
        subject,
        Autonomy {
            master,
            subject,
            level: initial_level,
            progress: 0.0,
            since_hour: world.elapsed_hours,
        },
    );
    Ok(())
}

/// 解除傀儡（让 subject 完全独立）。
pub fn release_puppet(
    world: &mut World,
    master: CountryId,
    subject: CountryId,
) -> Result<(), AutonomyError> {
    let a = world
        .diplomacy
        .autonomy
        .get(&subject)
        .ok_or(AutonomyError::NoRelation)?;
    if a.master != master {
        return Err(AutonomyError::NoRelation);
    }
    world.diplomacy.autonomy.remove(&subject);
    Ok(())
}

/// 整合（master 直接吞并 subject）。
///
/// 要求 subject 当前 progress 已降到 [`INTEGRATION_BELOW`] 以下，
/// 否则返回 [`AutonomyError::NoRelation`]（语义上：尚不允许）。
pub fn integrate_subject(
    world: &mut World,
    master: CountryId,
    subject: CountryId,
) -> Result<(), AutonomyError> {
    let a = world
        .diplomacy
        .autonomy
        .get(&subject)
        .ok_or(AutonomyError::NoRelation)?;
    if a.master != master {
        return Err(AutonomyError::NoRelation);
    }
    if a.progress > INTEGRATION_BELOW {
        return Err(AutonomyError::NoRelation);
    }
    // 把 subject 拥有的所有 state 转给 master，并标记 annexed
    let subject_id = subject;
    for i in 0..world.states.count {
        if world.states.owners[i] == subject_id {
            world.states.owners[i] = master;
            world.states.controllers[i] = master;
            for &p in world.states.provinces[i].clone().iter() {
                let pi = p.0 as usize;
                if pi < world.provinces.count {
                    world.provinces.owners[pi] = master;
                    world.provinces.controllers[pi] = master;
                }
            }
        }
    }
    world.diplomacy.annexed_countries.insert(subject_id);
    world.diplomacy.autonomy.remove(&subject_id);
    world.recalc_country_caches();
    Ok(())
}

/// 每日推进所有傀儡的自治度进度。
pub fn tick_autonomy_daily(world: &mut World) {
    // 先把需要的工业值快照下来，避免借用冲突
    let n = world.countries.count;
    let mut industry: Vec<f32> = vec![0.0; n];
    for i in 0..n {
        industry[i] = country_industry_score(world, CountryId(i as u16));
    }

    let mut upgrades: Vec<(CountryId, AutonomyLevel)> = Vec::new();
    for (subject, a) in world.diplomacy.autonomy.iter_mut() {
        let mi = a.master.0 as usize;
        let si = subject.0 as usize;
        if mi >= n || si >= n {
            continue;
        }
        let master_ind = industry[mi].max(1.0);
        let subj_ind = industry[si];
        let extraction_penalty = a.level.master_resource_share() * 0.15;
        // 自治进度 = base × (subject_ind / master_ind) - 主国压榨惩罚。
        let delta = (AUTONOMY_DAILY_BASE * (subj_ind / master_ind).clamp(0.0, 5.0)
            - extraction_penalty)
            .max(0.01);
        a.progress += delta;

        // 升级判定
        let threshold = a.level.upgrade_threshold();
        if a.progress >= threshold && a.level != AutonomyLevel::FreedomAssociation {
            let next = next_level_up(a.level);
            upgrades.push((*subject, next));
        }
    }

    for (subject, new_level) in upgrades {
        if let Some(a) = world.diplomacy.autonomy.get_mut(&subject) {
            a.level = new_level;
            // 升级后进度归 0
            a.progress = 0.0;
        }
    }
}

pub fn country_industry_score(world: &World, country: CountryId) -> f32 {
    if country.is_none() || country.0 as usize >= world.countries.count {
        return 0.0;
    }
    world
        .countries
        .buildings_v6
        .buildings
        .iter()
        .filter(|building| {
            let si = building.state.0 as usize;
            si < world.states.count && world.states.owners[si] == country && building.level > 0
        })
        .map(|building| {
            let weight = match building.kind {
                BuildingKind::Military => 2.0,
                BuildingKind::Industrial => 1.5,
                BuildingKind::Infrastructure => 1.0,
                BuildingKind::Resource => 1.0,
                BuildingKind::MilitaryBase => 0.8,
                BuildingKind::ConsumerGoods | BuildingKind::Service => 0.7,
                BuildingKind::Agriculture => 0.35,
            };
            building.level as f32 * weight
        })
        .sum()
}

fn next_level_up(level: AutonomyLevel) -> AutonomyLevel {
    match level {
        AutonomyLevel::Integrated => AutonomyLevel::IntegratedPuppet,
        AutonomyLevel::IntegratedPuppet => AutonomyLevel::Puppet,
        AutonomyLevel::Puppet => AutonomyLevel::Dominion,
        AutonomyLevel::Dominion => AutonomyLevel::Satellite,
        AutonomyLevel::Satellite => AutonomyLevel::FreedomAssociation,
        AutonomyLevel::FreedomAssociation => AutonomyLevel::FreedomAssociation,
    }
}
