//! 外交状态 — 阵营 / 战争 / 战争目标 / 傀儡自治度 / 国家关系 / 世界紧张度。
//!
//! Phase 3.7。数据结构使用 AoS（小集合） + HashMap，因为外交实体数量远小于
//! 师 / 省份等热点数据，缓存命中不是瓶颈。

use std::collections::{HashMap, HashSet};

use crate::ids::{CountryId, FactionId, StateId};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct TreatyId(pub u32);

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct DiplomaticRequestId(pub u32);

// ─── 阵营 ──────────────────────────────────────────────────────────

/// 阵营（faction，盟约）。
#[derive(Debug, Clone)]
pub struct Faction {
    pub id: FactionId,
    /// 阵营名（如 "Allies" / "Axis" / "Comintern"）
    pub name: String,
    /// 阵营领袖
    pub leader: CountryId,
    /// 全部成员（含 leader）
    pub members: Vec<CountryId>,
    /// 创建时间（hours since start）
    pub created_at_hour: u64,
}

impl Faction {
    pub fn contains(&self, c: CountryId) -> bool {
        self.members.iter().any(|&m| m == c)
    }
}

#[derive(Debug, Clone)]
pub struct DiplomaticRelation {
    pub a: CountryId,
    pub b: CountryId,
    pub opinion_ab: i16,
    pub opinion_ba: i16,
    pub threat_ab: f32,
    pub threat_ba: f32,
    pub trust: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TreatyKind {
    MilitaryAccess,
    NonAggressionPact,
    GuaranteeIndependence,
    FactionMembership,
    SubjectRelation,
    Truce,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Treaty {
    pub id: TreatyId,
    pub kind: TreatyKind,
    pub parties: Vec<CountryId>,
    pub since_hour: u64,
    pub expires_at_hour: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiplomaticRequestKind {
    InviteToFaction { faction_id: FactionId },
    RequestMilitaryAccess,
    OfferNonAggressionPact,
    OfferPeace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiplomaticRequestStatus {
    Pending,
    Accepted,
    Rejected,
    Expired,
    Withdrawn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiplomaticRequest {
    pub id: DiplomaticRequestId,
    pub from: CountryId,
    pub to: CountryId,
    pub kind: DiplomaticRequestKind,
    pub status: DiplomaticRequestStatus,
    pub created_at_hour: u64,
    pub expires_at_hour: Option<u64>,
    pub resolved_at_hour: Option<u64>,
}

// ─── 战争目标 ──────────────────────────────────────────────────────

/// 战争目标的类型。HOI4 有 ~15 种 wargoal generation_rules，
/// 我们简化为最常用的 6 种。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WargoalType {
    /// 吞并（整个国家）
    Annex,
    /// 占领指定州
    TakeState,
    /// 解放（建立新国家）
    Liberate,
    /// 傀儡化
    Puppet,
    /// 推翻政府（同意识形态）
    ToppleGovernment,
    /// 占用海军基地（HOI4: naval_access）
    NavalAccess,
}

impl WargoalType {
    /// 该 wargoal 是否需要指定一个 target_state
    pub fn needs_state(self) -> bool {
        matches!(self, Self::TakeState | Self::Liberate)
    }
}

/// 单个战争目标。
#[derive(Debug, Clone)]
pub struct Wargoal {
    /// 索取方
    pub claimant: CountryId,
    /// 受害方（被索取的国家）
    pub target: CountryId,
    pub kind: WargoalType,
    /// 目标 state（仅当 kind.needs_state() 为 true）
    pub target_state: Option<StateId>,
    /// 是否已完成正当化
    pub justified: bool,
    /// 正当化进度（天，0..duration）
    pub justify_progress: f32,
    /// 该目标耗时多少天才能完成（按 PP cost / 1.0 每日 = 天数）
    pub justify_total_days: f32,
}

impl Wargoal {
    /// 当前是否可作为开战理由
    pub fn is_ready(&self) -> bool {
        self.justified
    }
}

// ─── 战争 ──────────────────────────────────────────────────────────

/// 阵营成员自动参战策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum WarJoinPolicy {
    /// 阵营成员无条件自动参战（默认行为，历史兼容）
    AutoJoin,
    /// 阵营成员不自动参战，需要事件或脚本显式加入
    Delayed,
    /// 阵营成员禁止参战（用于严格限制参战范围）
    Forbidden,
}

impl Default for WarJoinPolicy {
    fn default() -> Self {
        Self::AutoJoin
    }
}

/// 一场战争（attacker side vs defender side）。
#[derive(Debug, Clone)]
pub struct War {
    /// 全局唯一战争 id（HashMap key）
    pub id: u32,
    /// 主进攻方（宣战发起者）
    pub primary_attacker: CountryId,
    /// 主防守方（被宣战国）
    pub primary_defender: CountryId,
    /// 全部攻方（含 primary_attacker；阵营成员入战时入此集合）
    pub attackers: HashSet<CountryId>,
    /// 全部守方
    pub defenders: HashSet<CountryId>,
    /// 战争开始小时
    pub started_at_hour: u64,
    /// 攻方累计 war score（0..100）
    pub attacker_war_score: f32,
    /// 守方累计 war score
    pub defender_war_score: f32,
    /// 战争目标：主攻方持有的若干已正当化目标（在宣战瞬间转移到这里）
    pub attacker_wargoals: Vec<Wargoal>,
    /// 守方反向 wargoal（白和后归还领土等）
    pub defender_wargoals: Vec<Wargoal>,
    /// P0.11：阵营成员自动参战策略表。
    /// key = 国家 id，value = 该国在此战争中的参战策略。
    /// 未出现在此表中的国家默认 AutoJoin。
    pub war_join_policies: HashMap<CountryId, WarJoinPolicy>,
}

impl War {
    pub fn contains(&self, c: CountryId) -> bool {
        self.attackers.contains(&c) || self.defenders.contains(&c)
    }

    /// 该国处于哪一方（None = 不参战）
    pub fn side_of(&self, c: CountryId) -> Option<WarSide> {
        if self.attackers.contains(&c) {
            Some(WarSide::Attacker)
        } else if self.defenders.contains(&c) {
            Some(WarSide::Defender)
        } else {
            None
        }
    }

    /// P0.11：获取某国在此战争中的参战策略。未配置则默认 AutoJoin。
    pub fn join_policy(&self, c: CountryId) -> WarJoinPolicy {
        self.war_join_policies
            .get(&c)
            .copied()
            .unwrap_or(WarJoinPolicy::AutoJoin)
    }

    /// P0.11：设置某国在此战争中的参战策略。
    pub fn set_join_policy(&mut self, c: CountryId, policy: WarJoinPolicy) {
        self.war_join_policies.insert(c, policy);
    }

    /// P0.11：判断某国是否被禁止自动参战（Delayed 或 Forbidden）。
    pub fn is_auto_join_blocked(&self, c: CountryId) -> bool {
        matches!(
            self.join_policy(c),
            WarJoinPolicy::Delayed | WarJoinPolicy::Forbidden
        )
    }
}

/// 战争中的一方
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarSide {
    Attacker,
    Defender,
}

// ─── 傀儡 / 自治度 ────────────────────────────────────────────────

/// 自治等级（HOI4 vanilla autonomy DLC 的简化版本）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AutonomyLevel {
    /// 完全吞并（已不再独立 — 只保留作为转换中间态）
    Integrated,
    /// 全方位傀儡（最深度控制；通常是 Reichskommissariat / Soviet Republic）
    IntegratedPuppet,
    /// 标准傀儡
    Puppet,
    /// 自治领（更高自治度）
    Dominion,
    /// 卫星国
    Satellite,
    /// 自由协约（最少控制）
    FreedomAssociation,
}

impl AutonomyLevel {
    /// 数值表示（越大越独立；0 = Integrated, 5 = FreedomAssociation）
    pub fn rank(self) -> u8 {
        match self {
            Self::Integrated => 0,
            Self::IntegratedPuppet => 1,
            Self::Puppet => 2,
            Self::Dominion => 3,
            Self::Satellite => 4,
            Self::FreedomAssociation => 5,
        }
    }

    /// 自治进度阈值（达到此值可升级到下一档）
    pub fn upgrade_threshold(self) -> f32 {
        match self {
            Self::Integrated => f32::MAX, // already max integration
            Self::IntegratedPuppet => 100.0,
            Self::Puppet => 200.0,
            Self::Dominion => 400.0,
            Self::Satellite => 600.0,
            Self::FreedomAssociation => f32::MAX, // already most free
        }
    }

    /// 主国对受国的资源 / 工厂"租用"比例（HOI4 简化值）
    pub fn master_resource_share(self) -> f32 {
        match self {
            Self::Integrated => 1.0,
            Self::IntegratedPuppet => 0.85,
            Self::Puppet => 0.50,
            Self::Dominion => 0.25,
            Self::Satellite => 0.10,
            Self::FreedomAssociation => 0.0,
        }
    }

    pub fn master_manpower_share(self) -> f32 {
        match self {
            Self::Integrated => 0.25,
            Self::IntegratedPuppet => 0.15,
            Self::Puppet => 0.05,
            Self::Dominion | Self::Satellite | Self::FreedomAssociation => 0.0,
        }
    }

    pub fn autonomy_pressure_from_extraction(self) -> f32 {
        match self {
            Self::Integrated => 0.05,
            Self::IntegratedPuppet => 0.10,
            Self::Puppet => 0.15,
            Self::Dominion => 0.20,
            Self::Satellite => 0.10,
            Self::FreedomAssociation => 0.0,
        }
    }
}

/// 一对 master → subject 的傀儡关系。
#[derive(Debug, Clone)]
pub struct Autonomy {
    pub master: CountryId,
    pub subject: CountryId,
    pub level: AutonomyLevel,
    /// 当前自治度进度（每日累加 / 减扣）。达到阈值可升级。
    pub progress: f32,
    /// 自治创建时间
    pub since_hour: u64,
}

// ─── 国家间关系 / 世界紧张度 ──────────────────────────────────────

/// 国家关系修饰类型。HOI4 中 opinion modifier 数十种；我们简化为标量。
#[derive(Debug, Clone)]
pub struct OpinionMatrix {
    /// (from, to) → opinion ∈ [-200, 200]
    pub opinions: HashMap<(CountryId, CountryId), i16>,
}

impl OpinionMatrix {
    pub fn new() -> Self {
        Self {
            opinions: HashMap::new(),
        }
    }

    pub fn get(&self, from: CountryId, to: CountryId) -> i16 {
        self.opinions.get(&(from, to)).copied().unwrap_or(0)
    }

    /// 增减 opinion，clamp 到 [-200, 200]
    pub fn modify(&mut self, from: CountryId, to: CountryId, delta: i16) {
        let v = self.opinions.entry((from, to)).or_insert(0);
        *v = (*v as i32 + delta as i32).clamp(-200, 200) as i16;
    }

    /// 设置为绝对值
    pub fn set(&mut self, from: CountryId, to: CountryId, value: i16) {
        self.opinions.insert((from, to), value.clamp(-200, 200));
    }
}

impl Default for OpinionMatrix {
    fn default() -> Self {
        Self::new()
    }
}

// ─── 顶层外交状态 ────────────────────────────────────────────────

/// 全局外交状态容器。
pub struct DiplomacyState {
    /// 所有阵营。FactionId 是稳定 id，不等于 Vec 下标。
    pub factions: Vec<Faction>,
    pub next_faction_id: u32,
    /// 进行中的战争，war_id → War
    pub wars: HashMap<u32, War>,
    /// 下一个分配的 war_id
    pub next_war_id: u32,
    /// 主权傀儡列表（按 master → subject 一对一索引；HOI4 中受国只能有一个 master）
    pub autonomy: HashMap<CountryId, Autonomy>,
    /// 国家间未完成的战争目标（按 claimant → 列表存放；正当化中的也放这里）
    pub pending_wargoals: HashMap<CountryId, Vec<Wargoal>>,
    /// 国家关系
    pub opinions: OpinionMatrix,
    /// 全局世界紧张度（0..100）
    pub world_tension: f32,
    /// 已被吞并的国家（用于紧张度持续累加；HashSet 防止重复计入）
    pub annexed_countries: HashSet<CountryId>,
    pub treaties: Vec<Treaty>,
    pub next_treaty_id: u32,
    pub diplomatic_requests: Vec<DiplomaticRequest>,
    pub next_diplomatic_request_id: u32,
    /// 旧存档兼容军事通行权：(grantor, grantee)。新流程通过 Treaty 派生。
    pub military_access: HashSet<(u16, u16)>,
}

impl DiplomacyState {
    pub fn new() -> Self {
        Self {
            factions: Vec::new(),
            next_faction_id: 0,
            wars: HashMap::new(),
            next_war_id: 0,
            autonomy: HashMap::new(),
            pending_wargoals: HashMap::new(),
            opinions: OpinionMatrix::new(),
            world_tension: 0.0,
            annexed_countries: HashSet::new(),
            treaties: Vec::new(),
            next_treaty_id: 0,
            diplomatic_requests: Vec::new(),
            next_diplomatic_request_id: 0,
            military_access: HashSet::new(),
        }
    }

    pub fn allocate_faction_id(&mut self) -> FactionId {
        let id = FactionId(self.next_faction_id);
        self.next_faction_id = self.next_faction_id.saturating_add(1);
        id
    }

    pub fn faction(&self, id: FactionId) -> Option<&Faction> {
        self.factions.iter().find(|f| f.id == id)
    }

    pub fn faction_mut(&mut self, id: FactionId) -> Option<&mut Faction> {
        self.factions.iter_mut().find(|f| f.id == id)
    }

    /// 找到 country 所在阵营（None = 无阵营）
    pub fn faction_of(&self, country: CountryId) -> Option<FactionId> {
        for f in &self.factions {
            if f.contains(country) {
                return Some(f.id);
            }
        }
        None
    }

    pub fn faction_members(&self, faction: FactionId) -> Vec<CountryId> {
        self.faction(faction)
            .map(|f| f.members.clone())
            .unwrap_or_default()
    }

    pub fn pending_requests_for(
        &self,
        recipient: CountryId,
    ) -> impl Iterator<Item = &DiplomaticRequest> {
        self.diplomatic_requests
            .iter()
            .filter(move |r| r.to == recipient && r.status == DiplomaticRequestStatus::Pending)
    }

    pub fn create_request(
        &mut self,
        from: CountryId,
        to: CountryId,
        kind: DiplomaticRequestKind,
        created_at_hour: u64,
        expires_at_hour: Option<u64>,
    ) -> DiplomaticRequestId {
        let id = DiplomaticRequestId(self.next_diplomatic_request_id);
        self.next_diplomatic_request_id = self.next_diplomatic_request_id.saturating_add(1);
        self.diplomatic_requests.push(DiplomaticRequest {
            id,
            from,
            to,
            kind,
            status: DiplomaticRequestStatus::Pending,
            created_at_hour,
            expires_at_hour,
            resolved_at_hour: None,
        });
        id
    }

    pub fn add_treaty(
        &mut self,
        kind: TreatyKind,
        parties: Vec<CountryId>,
        since_hour: u64,
        expires_at_hour: Option<u64>,
    ) -> TreatyId {
        let id = TreatyId(self.next_treaty_id);
        self.next_treaty_id = self.next_treaty_id.saturating_add(1);
        self.treaties.push(Treaty {
            id,
            kind,
            parties,
            since_hour,
            expires_at_hour,
        });
        id
    }

    /// 该国与另一国是否在交战
    pub fn at_war_with(&self, a: CountryId, b: CountryId) -> bool {
        for war in self.wars.values() {
            let sa = war.side_of(a);
            let sb = war.side_of(b);
            match (sa, sb) {
                (Some(WarSide::Attacker), Some(WarSide::Defender)) => return true,
                (Some(WarSide::Defender), Some(WarSide::Attacker)) => return true,
                _ => {}
            }
        }
        false
    }

    /// 该国是否在战争中（任意方）
    pub fn is_at_war(&self, c: CountryId) -> bool {
        self.wars.values().any(|w| w.contains(c))
    }

    /// 该国是否为某主国的傀儡
    pub fn is_subject_of(&self, subject: CountryId, master: CountryId) -> bool {
        self.autonomy
            .get(&subject)
            .map(|a| a.master == master)
            .unwrap_or(false)
    }

    /// 授予军事通行权：grantor 允许 grantee 通过其领土
    pub fn grant_military_access(&mut self, grantor: CountryId, grantee: CountryId) {
        if !self.has_military_access(grantee, grantor) {
            self.add_treaty(TreatyKind::MilitaryAccess, vec![grantor, grantee], 0, None);
        }
    }

    /// 撤销军事通行权
    pub fn revoke_military_access(&mut self, grantor: CountryId, grantee: CountryId) {
        self.military_access.remove(&(grantor.0, grantee.0));
        self.treaties.retain(|t| {
            !(t.kind == TreatyKind::MilitaryAccess
                && t.parties.first() == Some(&grantor)
                && t.parties.get(1) == Some(&grantee))
        });
    }

    /// grantee (from) 是否拥有 grantor (to_controller) 授予的军事通行权
    pub fn has_military_access(&self, from: CountryId, to_controller: CountryId) -> bool {
        self.military_access.contains(&(to_controller.0, from.0))
            || self.treaties.iter().any(|t| {
                t.kind == TreatyKind::MilitaryAccess
                    && t.parties.first() == Some(&to_controller)
                    && t.parties.get(1) == Some(&from)
            })
    }
}

impl Default for DiplomacyState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faction_contains() {
        let f = Faction {
            id: FactionId(0),
            name: "Test".into(),
            leader: CountryId(1),
            members: vec![CountryId(1), CountryId(2)],
            created_at_hour: 0,
        };
        assert!(f.contains(CountryId(1)));
        assert!(f.contains(CountryId(2)));
        assert!(!f.contains(CountryId(3)));
    }

    #[test]
    fn wargoal_type_needs_state() {
        assert!(WargoalType::TakeState.needs_state());
        assert!(WargoalType::Liberate.needs_state());
        assert!(!WargoalType::Annex.needs_state());
        assert!(!WargoalType::Puppet.needs_state());
    }

    #[test]
    fn autonomy_rank_order() {
        assert!(AutonomyLevel::Integrated.rank() < AutonomyLevel::Puppet.rank());
        assert!(AutonomyLevel::Puppet.rank() < AutonomyLevel::Satellite.rank());
        assert!(AutonomyLevel::Satellite.rank() < AutonomyLevel::FreedomAssociation.rank());
    }

    #[test]
    fn opinion_clamp() {
        let mut o = OpinionMatrix::new();
        let a = CountryId(1);
        let b = CountryId(2);
        o.modify(a, b, 250);
        assert_eq!(o.get(a, b), 200);
        o.modify(a, b, -500);
        assert_eq!(o.get(a, b), -200);
    }

    #[test]
    fn diplomacy_state_default() {
        let d = DiplomacyState::new();
        assert!(d.factions.is_empty());
        assert_eq!(d.next_faction_id, 0);
        assert!(d.wars.is_empty());
        assert_eq!(d.world_tension, 0.0);
        assert!(!d.is_at_war(CountryId(1)));
    }

    #[test]
    fn at_war_with_detection() {
        let mut d = DiplomacyState::new();
        let a = CountryId(1);
        let b = CountryId(2);
        let mut war = War {
            id: 0,
            primary_attacker: a,
            primary_defender: b,
            attackers: HashSet::new(),
            defenders: HashSet::new(),
            started_at_hour: 0,
            attacker_war_score: 0.0,
            defender_war_score: 0.0,
            attacker_wargoals: vec![],
            defender_wargoals: vec![],
            war_join_policies: HashMap::new(),
        };
        war.attackers.insert(a);
        war.defenders.insert(b);
        d.wars.insert(0, war);
        assert!(d.at_war_with(a, b));
        assert!(d.at_war_with(b, a));
        assert!(!d.at_war_with(a, CountryId(3)));
    }

    #[test]
    fn faction_id_is_stable_after_removal() {
        let mut d = DiplomacyState::new();
        let a = d.allocate_faction_id();
        let b = d.allocate_faction_id();
        d.factions.push(Faction {
            id: a,
            name: "A".into(),
            leader: CountryId(1),
            members: vec![CountryId(1)],
            created_at_hour: 0,
        });
        d.factions.push(Faction {
            id: b,
            name: "B".into(),
            leader: CountryId(2),
            members: vec![CountryId(2)],
            created_at_hour: 0,
        });
        d.factions.retain(|f| f.id != a);
        assert_eq!(d.faction_of(CountryId(2)), Some(b));
        assert_eq!(d.faction(b).unwrap().name, "B");
    }

    #[test]
    fn war_join_policy_default_is_auto_join() {
        let war = War {
            id: 0,
            primary_attacker: CountryId(1),
            primary_defender: CountryId(2),
            attackers: HashSet::new(),
            defenders: HashSet::new(),
            started_at_hour: 0,
            attacker_war_score: 0.0,
            defender_war_score: 0.0,
            attacker_wargoals: vec![],
            defender_wargoals: vec![],
            war_join_policies: HashMap::new(),
        };
        assert_eq!(war.join_policy(CountryId(1)), WarJoinPolicy::AutoJoin);
        assert!(!war.is_auto_join_blocked(CountryId(1)));
    }

    #[test]
    fn war_join_policy_delayed_blocks_auto_join() {
        let mut war = War {
            id: 0,
            primary_attacker: CountryId(1),
            primary_defender: CountryId(2),
            attackers: HashSet::new(),
            defenders: HashSet::new(),
            started_at_hour: 0,
            attacker_war_score: 0.0,
            defender_war_score: 0.0,
            attacker_wargoals: vec![],
            defender_wargoals: vec![],
            war_join_policies: HashMap::new(),
        };
        war.set_join_policy(CountryId(3), WarJoinPolicy::Delayed);
        assert_eq!(war.join_policy(CountryId(3)), WarJoinPolicy::Delayed);
        assert!(war.is_auto_join_blocked(CountryId(3)));
        assert!(!war.is_auto_join_blocked(CountryId(1)));
    }

    #[test]
    fn war_join_policy_forbidden_blocks_auto_join() {
        let mut war = War {
            id: 0,
            primary_attacker: CountryId(1),
            primary_defender: CountryId(2),
            attackers: HashSet::new(),
            defenders: HashSet::new(),
            started_at_hour: 0,
            attacker_war_score: 0.0,
            defender_war_score: 0.0,
            attacker_wargoals: vec![],
            defender_wargoals: vec![],
            war_join_policies: HashMap::new(),
        };
        war.set_join_policy(CountryId(3), WarJoinPolicy::Forbidden);
        assert!(war.is_auto_join_blocked(CountryId(3)));
    }
}
