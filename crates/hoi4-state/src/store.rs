//! 运行时实体存储。使用 SoA（Structure of Arrays）布局以提高缓存命中率。

use crate::buildings_v6::BuildingStore;
use crate::finance::TreasuryStore;
use crate::ids::{AirWingId, CountryId, FleetId, ProvinceId, ShipId, StateId};
use crate::laws::LawStore;
use crate::market::MarketStore;
use crate::pops::{PopClass, PopStore};
use crate::trade::TradeStore;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InvestmentAccountKind {
    Private,
    Cartel,
    StateDevelopmentBank,
    ColonialExtraction,
    ForeignCapital,
}

/// P2.6：第一轮人物运行时状态。
///
/// RON 事件可通过 `Effect::SetCharacterAvailable` / `KillCharacter` /
/// `ExileCharacter` / `RecruitCharacter` 改变状态；后续事件可读取
/// `Trigger::CharacterDead` / `CharacterAvailable` / `CharacterIsLeader`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CharacterRuntimeStatus {
    /// 未被任何事件标记，但已被 `RecruitCharacter` 入册可用。
    Available,
    /// 当前在任领导人/掌权者；最多一国一人物（由 RON 事件保证）。
    InOffice,
    /// 缺席（如何塞·安东尼奥被关在阿利坎特），事件可读取此状态。
    Absent,
    /// 被捕。
    Imprisoned,
    /// 流亡海外。
    Exiled,
    /// 死亡（如桑胡尔霍坠机）。
    Dead,
}

impl Default for CharacterRuntimeStatus {
    fn default() -> Self {
        Self::Available
    }
}

#[derive(Debug, Clone)]
pub struct InvestmentAccount {
    pub country: CountryId,
    pub account_kind: InvestmentAccountKind,
    pub balance_rm: f64,
    pub last_income_rm: f64,
    pub last_spent_rm: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OccupationPolicy {
    LenientOccupation,
    CivilianOversight,
    MilitaryGovernor,
    HarshRepression,
    LootingEconomy,
}

impl Default for OccupationPolicy {
    fn default() -> Self {
        Self::CivilianOversight
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StateIntegrationStatus {
    Metropole,
    Incorporated,
    Colony,
    Protectorate,
    Mandate,
    Concession,
    Occupied,
}

impl Default for StateIntegrationStatus {
    fn default() -> Self {
        Self::Metropole
    }
}

impl StateIntegrationStatus {
    pub fn is_domestic(self) -> bool {
        matches!(self, Self::Metropole | Self::Incorporated)
    }

    pub fn is_colonial_or_occupied(self) -> bool {
        matches!(
            self,
            Self::Colony | Self::Protectorate | Self::Mandate | Self::Concession | Self::Occupied
        )
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PopulationBreakdown {
    pub domestic: u64,
    pub colonial: u64,
    pub governed: u64,
    pub subject: u64,
    pub imperial: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DivisionRole {
    Garrison,
    Assault,
    Reserve,
    MopUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DivisionIntent {
    Garrison,
    Assault,
    Reserve,
    MopUp,
    NavalInvasion,
    OverseasTransport,
    Retreat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandSource {
    PlayerManual,
    PlayerFrontline,
    AiGroundOrders,
    AiFrontline,
    AiInvasionPlan,
    AiOverseasReinforce,
    SystemRetreat,
}

impl CommandSource {
    pub fn priority(self) -> u8 {
        match self {
            CommandSource::PlayerManual => 200,
            CommandSource::PlayerFrontline => 150,
            CommandSource::AiInvasionPlan => 100,
            CommandSource::AiOverseasReinforce => 90,
            CommandSource::AiGroundOrders => 50,
            CommandSource::AiFrontline => 40,
            CommandSource::SystemRetreat => 30,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DivisionCommand {
    pub intent: DivisionIntent,
    pub source: CommandSource,
    pub target: ProvinceId,
    pub expires_at_hour: u64,
    pub assignment: Option<DivisionAssignment>,
}

#[derive(Debug, Clone)]
pub struct DivisionAssignment {
    pub front: Option<CountryId>,
    pub role: DivisionRole,
    pub target: Option<ProvinceId>,
    pub assigned_at: u64,
}

/// Army overseas transport lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArmyTransportPhase {
    Embarking,
    AtSea,
    Disembarking,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArmyTransportState {
    pub phase: ArmyTransportPhase,
    pub origin_port: ProvinceId,
    pub destination_port: ProvinceId,
    pub route_regions: Vec<u32>,
    pub phase_ends_at_hour: u64,
}

/// 省份运行时状态。SoA 布局：每个字段一个数组。
pub struct ProvinceStore {
    pub count: usize,
    /// 法理拥有者（来自 state.owner）
    pub owners: Vec<CountryId>,
    /// 当前控制者（战时可能 != owner）
    pub controllers: Vec<CountryId>,
    /// 该省份所属的 state
    pub state_of: Vec<StateId>,
    /// 当前补给等级（0-100）
    pub supply: Vec<f32>,
}

impl ProvinceStore {
    pub fn new(count: usize) -> Self {
        Self {
            count,
            owners: vec![CountryId::NONE; count],
            controllers: vec![CountryId::NONE; count],
            state_of: vec![StateId::NONE; count],
            supply: vec![0.0; count],
        }
    }
}

/// 州运行时状态。
pub struct StateStore {
    pub count: usize,
    pub owners: Vec<CountryId>,
    pub controllers: Vec<CountryId>,
    pub cores: Vec<Vec<CountryId>>,
    /// 该州的省份列表
    pub provinces: Vec<Vec<ProvinceId>>,

    // 建筑
    pub infrastructure: Vec<u8>,

    /// J.3: building slot cap from state category
    pub category_slots: Vec<u8>,

    // 资源
    pub manpower_pool: Vec<u32>,
    pub integration_status: Vec<StateIntegrationStatus>,

    // 占领状态
    pub resistance: Vec<f32>,
    pub compliance: Vec<f32>,
    /// 对该州执行占领政策的国家；非占领州为 NONE。
    pub occupiers: Vec<CountryId>,
    pub occupation_policies: Vec<OccupationPolicy>,
    pub garrison_template_indices: Vec<Option<u32>>,
    pub required_suppression: Vec<f32>,
    pub provided_suppression: Vec<f32>,
    pub last_sabotage_day: Vec<i64>,

    /// 名称（loc key 用 String，预算 1KB/state）
    pub names: Vec<String>,
}

impl StateStore {
    pub fn new(count: usize) -> Self {
        Self {
            count,
            owners: vec![CountryId::NONE; count],
            controllers: vec![CountryId::NONE; count],
            cores: vec![Vec::new(); count],
            provinces: vec![Vec::new(); count],
            infrastructure: vec![0; count],
            category_slots: vec![4; count],
            manpower_pool: vec![0; count],
            integration_status: vec![StateIntegrationStatus::default(); count],
            resistance: vec![0.0; count],
            compliance: vec![1.0; count],
            occupiers: vec![CountryId::NONE; count],
            occupation_policies: vec![OccupationPolicy::default(); count],
            garrison_template_indices: vec![None; count],
            required_suppression: vec![0.0; count],
            provided_suppression: vec![0.0; count],
            last_sabotage_day: vec![-1; count],
            names: vec![String::new(); count],
        }
    }
}

/// 国家运行时状态。
pub struct CountryStore {
    pub count: usize,
    /// 标签字符串 (e.g. "GER")
    pub tags: Vec<String>,
    pub colors: Vec<[u8; 3]>,
    pub capitals: Vec<StateId>,

    // 政治
    pub political_power: Vec<f32>,
    pub stability: Vec<f32>,
    pub war_support: Vec<f32>,
    pub ruling_party: Vec<String>,

    // 资源
    pub fuel: Vec<f32>,
    pub fuel_capacity: Vec<f32>,

    // 经验
    pub army_xp: Vec<f32>,
    pub navy_xp: Vec<f32>,
    pub air_xp: Vec<f32>,

    // 外交
    pub at_war: Vec<bool>,

    // 国家精神/科技（少量，可以用 Vec<String>）
    pub completed_techs: Vec<Vec<String>>,
    pub ideas: Vec<Vec<String>>,

    // 科技 / 研发
    /// 研发槽位数量（默认 BASE_RESEARCH_SLOTS = 2）
    pub research_slots: Vec<u8>,
    pub tech_bonus: Vec<std::collections::HashMap<String, f32>>,
    /// Effective conscription cap written by V6 law modifiers.
    pub conscription_max_ratio: Vec<f32>,
    /// Effective daily recruit conversion multiplier written by V6 law modifiers.
    pub conscription_recruit_speed_mult: Vec<f32>,
    /// 已解锁的装备（来自完成科技的 `enable_equipments`）。生产线只能选择被解锁的装备
    pub unlocked_equipments: Vec<std::collections::HashSet<String>>,
    /// 已解锁的子单位（兵种）类型
    pub unlocked_subunits: Vec<std::collections::HashSet<String>>,
    /// 已解锁的建筑类型（部分建筑由科技解锁，如 radar_station）
    pub unlocked_buildings: Vec<std::collections::HashSet<String>>,

    // 政治（Phase 3.3）
    /// 各意识形态支持率（key=democratic/communism/fascism/neutrality,value=0..1）
    pub party_popularity: Vec<std::collections::HashMap<String, f32>>,
    /// 当前正在执行的国策 id（Idle = None）
    pub current_focus: Vec<Option<String>>,
    /// 当前国策已累积进度（天）
    pub focus_progress: Vec<f32>,
    /// 已完成国策集合
    pub completed_focuses: Vec<std::collections::HashSet<String>>,

    /// J.1.2：每国 country_leader 在 `data.characters` 中的索引；`None` = 未匹配。
    /// 由 [`crate::World::refresh_country_leader`] 维护：每次 `ruling_party` 变化后
    /// 应调用以保持字段一致。
    pub country_leader_idx: Vec<Option<u32>>,

    /// P2.5：第一轮事件变量系统。每国一张 `name -> f32` 表，由 RON 事件
    /// `Effect::SetVariable` / `AddToVariable` / `ClampVariable` 写入，
    /// `Trigger::VariableGreaterThan` / `VariableLessThan` 等读取。
    /// 国家级变量按 country index 隔离，避免 `SPR`/`SPA`/`CNT` 互相污染。
    pub variables: Vec<std::collections::HashMap<String, f32>>,

    /// P2.6：第一轮人物效果系统。每国一张 `character_key -> CharacterRuntimeStatus`
    /// 表，记录人物是否死亡、被捕、流亡、可用或在任。
    /// 由 RON 事件 `Effect::KillCharacter` / `ExileCharacter` / `RecruitCharacter`
    /// / `SetCharacterAvailable` / `SetLeader` 写入，
    /// `Trigger::CharacterDead` / `CharacterAvailable` / `CharacterIsLeader` 读取。
    /// 跨国家共享同一张表使用 country.0 == 0 的全局表。
    pub characters: Vec<std::collections::HashMap<String, CharacterRuntimeStatus>>,

    // ─── V6 经济子系统 ───
    pub buildings_v6: BuildingStore,
    pub pops: PopStore,
    /// Private retained profits available for autonomous market-economy construction.
    pub private_investment_pool_rm: Vec<f64>,
    pub investment_accounts: Vec<InvestmentAccount>,
    pub market: MarketStore,
    pub treasury: TreasuryStore,
    pub law_store: LawStore,
    pub trade: TradeStore,
    pub v6_events_fired: Vec<HashSet<String>>,
}

impl CountryStore {
    pub fn new(count: usize) -> Self {
        Self {
            count,
            tags: vec![String::new(); count],
            colors: vec![[128, 128, 128]; count],
            capitals: vec![StateId::NONE; count],
            political_power: vec![0.0; count],
            stability: vec![0.5; count],
            war_support: vec![0.0; count],
            ruling_party: vec![String::new(); count],
            fuel: vec![0.0; count],
            fuel_capacity: vec![1000.0; count],
            army_xp: vec![0.0; count],
            navy_xp: vec![0.0; count],
            air_xp: vec![0.0; count],
            at_war: vec![false; count],
            completed_techs: vec![Vec::new(); count],
            ideas: vec![Vec::new(); count],
            research_slots: vec![2; count],
            tech_bonus: vec![std::collections::HashMap::new(); count],
            conscription_max_ratio: vec![0.0; count],
            conscription_recruit_speed_mult: vec![0.0; count],
            unlocked_equipments: vec![std::collections::HashSet::new(); count],
            unlocked_subunits: vec![std::collections::HashSet::new(); count],
            unlocked_buildings: vec![std::collections::HashSet::new(); count],
            party_popularity: vec![std::collections::HashMap::new(); count],
            current_focus: vec![None; count],
            focus_progress: vec![0.0; count],
            completed_focuses: vec![std::collections::HashSet::new(); count],
            country_leader_idx: vec![None; count],
            variables: vec![std::collections::HashMap::new(); count],
            characters: vec![std::collections::HashMap::new(); count],
            buildings_v6: BuildingStore::new(),
            pops: PopStore::new(),
            private_investment_pool_rm: vec![0.0; count],
            investment_accounts: default_investment_accounts(count),
            market: MarketStore::new(count),
            treasury: TreasuryStore::new(count),
            law_store: LawStore::new(count),
            trade: TradeStore::new(),
            v6_events_fired: vec![HashSet::new(); count],
        }
    }

    /// P5（Soldier↔manpower 统一）：从 PopGroup 派生 manpower。
    /// 仅统计 class=Soldier 且 employed_at=None（未编入师）的 PopGroup size 之和。
    pub fn manpower(&self, _ci: usize, state_ids: &[StateId]) -> u64 {
        self.pops
            .pops_by_class_in_country(PopClass::Soldier, state_ids)
            .iter()
            .filter(|p| p.employed_at.is_none())
            .map(|p| p.size as u64)
            .sum()
    }

    pub fn investment_account_mut(
        &mut self,
        country: CountryId,
        account_kind: InvestmentAccountKind,
    ) -> Option<&mut InvestmentAccount> {
        self.investment_accounts
            .iter_mut()
            .find(|account| account.country == country && account.account_kind == account_kind)
    }

    pub fn investment_balance_rm(
        &self,
        country: CountryId,
        account_kind: InvestmentAccountKind,
    ) -> f64 {
        self.investment_accounts
            .iter()
            .find(|account| account.country == country && account.account_kind == account_kind)
            .map(|account| account.balance_rm)
            .unwrap_or(0.0)
    }
}

fn default_investment_accounts(count: usize) -> Vec<InvestmentAccount> {
    let kinds = [
        InvestmentAccountKind::Private,
        InvestmentAccountKind::Cartel,
        InvestmentAccountKind::StateDevelopmentBank,
        InvestmentAccountKind::ColonialExtraction,
        InvestmentAccountKind::ForeignCapital,
    ];
    let mut accounts = Vec::with_capacity(count * kinds.len());
    for ci in 0..count {
        for kind in kinds {
            accounts.push(InvestmentAccount {
                country: CountryId(ci as u16),
                account_kind: kind,
                balance_rm: 0.0,
                last_income_rm: 0.0,
                last_spent_rm: 0.0,
            });
        }
    }
    accounts
}

/// 师（Division）运行时状态。SoA 布局。
///
/// 每个师对应一个模板（template_index）+ 持有方（owner）+ 当前位置（location）。
/// 战斗参数（strength / org / xp）在每日 tick 与战斗中变动；
/// `combat_state` 用于标记是否在战斗中（影响 org 恢复）
pub struct DivisionStore {
    pub count: usize,
    /// 持有方
    pub owners: Vec<CountryId>,
    /// 当前位置（省份）
    pub locations: Vec<ProvinceId>,
    /// 模板索引（持有方在 GameData.division_templates[tag] 中的下标）
    pub template_indices: Vec<u16>,
    /// 战力（0..1，1 = 满员；按装备 / 人力的最小满足度）
    pub strength: Vec<f32>,
    /// 当前组织度（0..max_org）
    pub organisation: Vec<f32>,
    /// 该师的 max_organisation（汇总自模板）
    pub max_organisation: Vec<f32>,
    /// 该师的 max_strength（汇总自模板，HOI4 单位）
    pub max_strength: Vec<f32>,
    /// 经验（0..900）
    pub experience: Vec<f32>,
    /// 是否在战斗中
    pub in_combat: Vec<bool>,
    /// 该师的将领 id（u16，none = u16::MAX）
    pub general_id: Vec<u16>,
    /// 名称（"1. Infanterie-Division" 等）
    pub names: Vec<String>,
    /// 移动目标省份（None = 静止）。
    pub destinations: Vec<Option<ProvinceId>>,
    /// 当前移动进度（0..1，到 1 时翻入 destination）。
    pub move_progress: Vec<f32>,
    /// 师团战略任务标签（P2：解决"打着打着不打了"）
    pub assignments: Vec<Option<DivisionAssignment>>,
    /// P0.13：统一命令队列。AI / 玩家前线 / 系统撤退只写命令，不直接写 destination。
    /// 统一执行层 tick_division_commands_daily 按优先级裁决后写最终 destination。
    pub commands: Vec<Option<DivisionCommand>>,
    /// Bug #7：连续多少个 daily tick 满足"濒死"条件（strength<0.01 或 strength<0.05+org<5%）。
    /// 满足时累加，不满足时归零。daily_division_cleanup 用 ≥1 触发销毁，
    /// 让师团在残血状态至少存活一个 tick 才被永久清除（避免误杀刚被打到 0 的师）。
    pub low_strength_days: Vec<u8>,
    /// Bug #13：该师最后一次参与战斗的小时数（u64::MAX = 从未战斗）。
    /// org 恢复时检查：如果 `elapsed_hours - last_combat_hour < ORG_REGEN_DELAY_HOURS`，
    /// 则该师不恢复 org，避免"打完立刻恢复"导致的无休止拉锯。
    pub last_combat_hour: Vec<u64>,
    /// 陆军海运状态。Some 时师团处于装船/海上/卸船阶段。
    pub transport: Vec<Option<ArmyTransportState>>,
}

impl DivisionStore {
    pub fn new() -> Self {
        Self {
            count: 0,
            owners: Vec::new(),
            locations: Vec::new(),
            template_indices: Vec::new(),
            strength: Vec::new(),
            organisation: Vec::new(),
            max_organisation: Vec::new(),
            max_strength: Vec::new(),
            experience: Vec::new(),
            in_combat: Vec::new(),
            general_id: Vec::new(),
            names: Vec::new(),
            destinations: Vec::new(),
            move_progress: Vec::new(),
            assignments: Vec::new(),
            commands: Vec::new(),
            low_strength_days: Vec::new(),
            last_combat_hour: Vec::new(),
            transport: Vec::new(),
        }
    }

    /// 添加一个新师，返回内部索引（即 DivisionId.0）
    pub fn push(
        &mut self,
        owner: CountryId,
        location: ProvinceId,
        template_index: u16,
        max_org: f32,
        max_str: f32,
        name: String,
    ) -> u32 {
        let idx = self.count as u32;
        self.owners.push(owner);
        self.locations.push(location);
        self.template_indices.push(template_index);
        self.strength.push(1.0);
        self.organisation.push(max_org);
        self.max_organisation.push(max_org);
        self.max_strength.push(max_str);
        self.experience.push(0.0);
        self.in_combat.push(false);
        self.general_id.push(u16::MAX);
        self.names.push(name);
        self.destinations.push(None);
        self.move_progress.push(0.0);
        self.assignments.push(None);
        self.commands.push(None);
        self.low_strength_days.push(0);
        self.last_combat_hour.push(u64::MAX);
        self.transport.push(None);
        self.count += 1;
        idx
    }

    /// Bug #7：从 SoA 存储中移除指定下标的师（O(1) swap_remove 语义）。
    ///
    /// 返回值：
    /// - `None` —— `index` 越界或被移除的恰好是末尾元素，调用方无需 remap；
    /// - `Some(swap_from)` —— 末尾元素的旧下标 `swap_from`（== `count` 删除前的值），
    ///   它的数据已经搬到 `index` 处。调用方必须把所有引用 `swap_from` 的下标
    ///   重新映射到 `index`。
    ///
    /// **注意**：本方法只动 `DivisionStore` 自身的 SoA 字段；外部容器
    /// （`World::command.div_to_corps` / `World::player_armies` 等）由调用方
    /// （通常是 `World::remove_division`）一起修复。
    pub fn remove(&mut self, index: usize) -> Option<usize> {
        if index >= self.count {
            return None;
        }
        let last = self.count - 1;
        // swap_remove 所有 SoA 字段。Vec::swap_remove 自身处理 index == last 的退化情形。
        self.owners.swap_remove(index);
        self.locations.swap_remove(index);
        self.template_indices.swap_remove(index);
        self.strength.swap_remove(index);
        self.organisation.swap_remove(index);
        self.max_organisation.swap_remove(index);
        self.max_strength.swap_remove(index);
        self.experience.swap_remove(index);
        self.in_combat.swap_remove(index);
        self.general_id.swap_remove(index);
        self.names.swap_remove(index);
        self.destinations.swap_remove(index);
        self.move_progress.swap_remove(index);
        self.assignments.swap_remove(index);
        self.commands.swap_remove(index);
        self.low_strength_days.swap_remove(index);
        self.last_combat_hour.swap_remove(index);
        self.transport.swap_remove(index);
        self.count -= 1;
        if index == last {
            None
        } else {
            Some(last)
        }
    }
}

impl Default for DivisionStore {
    fn default() -> Self {
        Self::new()
    }
}

// ─── 海军 ──────────────────────────────────────────────────────────

/// 舰只运行时数据。SoA。
pub struct ShipStore {
    pub count: usize,
    pub owners: Vec<CountryId>,
    /// 该舰所属的舰队
    pub fleet_id: Vec<FleetId>,
    /// 舰类索引（GameData.ship_classes 中按 sorted order 的下标；具体由调用方决定）
    /// 这里直接保存 class_key 字符串以避免 indirection
    pub class_keys: Vec<String>,
    /// 当前 HP（0..max_hp）
    pub hp: Vec<f32>,
    /// max_hp（汇总自舰类）
    pub max_hp: Vec<f32>,
    /// 当前 organisation（士气等价物）
    pub organisation: Vec<f32>,
    pub max_organisation: Vec<f32>,
    /// 是否在战斗中
    pub in_combat: Vec<bool>,
    /// 名称（"Bismarck" 等）
    pub names: Vec<String>,
}

impl ShipStore {
    pub fn new() -> Self {
        Self {
            count: 0,
            owners: Vec::new(),
            fleet_id: Vec::new(),
            class_keys: Vec::new(),
            hp: Vec::new(),
            max_hp: Vec::new(),
            organisation: Vec::new(),
            max_organisation: Vec::new(),
            in_combat: Vec::new(),
            names: Vec::new(),
        }
    }

    pub fn push(
        &mut self,
        owner: CountryId,
        fleet: FleetId,
        class_key: String,
        max_hp: f32,
        max_org: f32,
        name: String,
    ) -> u32 {
        let idx = self.count as u32;
        self.owners.push(owner);
        self.fleet_id.push(fleet);
        self.class_keys.push(class_key);
        self.hp.push(max_hp);
        self.max_hp.push(max_hp);
        self.organisation.push(max_org);
        self.max_organisation.push(max_org);
        self.in_combat.push(false);
        self.names.push(name);
        self.count += 1;
        idx
    }
}

impl Default for ShipStore {
    fn default() -> Self {
        Self::new()
    }
}

/// 海军任务（与某 fleet 绑定）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NavalMission {
    /// 在港 / 闲置
    Idle,
    /// 巡逻（被动迎击）
    Patrol,
    /// 护航（保护己方 convoy）
    ConvoyEscort,
    /// 攻击舰队（主动出击）
    StrikeForce,
    /// 通商破坏（攻击敌 convoy）
    ConvoyRaiding,
    /// 布雷
    MineLaying,
    /// 扫雷
    MineSweeping,
    /// 登陆支援
    NavalInvasionSupport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NavalRepairState {
    AtSea,
    InPort,
    Repairing,
}

/// 舰队（task force）。SoA。每个舰队由若干 ship 组成
pub struct FleetStore {
    pub count: usize,
    pub owners: Vec<CountryId>,
    /// 当前所在战略海区（map.strategic_regions 索引；none = u32::MAX）
    pub region_id: Vec<u32>,
    /// 当前停靠港口省份（none = 在海上）。港口通过逻辑层映射到战略海区。
    pub home_port: Vec<u16>,
    /// 正在移动时的目标海区；none = 未移动。
    pub target_region_id: Vec<u32>,
    /// 抵达目标所需的游戏小时戳；0 = 无移动。
    pub arrival_hour: Vec<u64>,
    /// 舰队移动速度（海区/日的抽象速度，越高越快）。
    pub speed_knots: Vec<f32>,
    pub repair_state: Vec<NavalRepairState>,
    /// 当前任务
    pub mission: Vec<NavalMission>,
    /// 名称（"Hochseeflotte" / "Home Fleet" 等）
    pub names: Vec<String>,
    /// 该舰队包含的 ship_id 列表
    pub ships: Vec<Vec<ShipId>>,
}

impl FleetStore {
    pub fn new() -> Self {
        Self {
            count: 0,
            owners: Vec::new(),
            region_id: Vec::new(),
            home_port: Vec::new(),
            target_region_id: Vec::new(),
            arrival_hour: Vec::new(),
            speed_knots: Vec::new(),
            repair_state: Vec::new(),
            mission: Vec::new(),
            names: Vec::new(),
            ships: Vec::new(),
        }
    }

    pub fn push(&mut self, owner: CountryId, region: u32, name: String) -> u32 {
        let idx = self.count as u32;
        self.owners.push(owner);
        self.region_id.push(region);
        self.home_port.push(u16::MAX);
        self.target_region_id.push(u32::MAX);
        self.arrival_hour.push(0);
        self.speed_knots.push(20.0);
        self.repair_state.push(NavalRepairState::AtSea);
        self.mission.push(NavalMission::Idle);
        self.names.push(name);
        self.ships.push(Vec::new());
        self.count += 1;
        idx
    }
}

impl Default for FleetStore {
    fn default() -> Self {
        Self::new()
    }
}

// ─── 空军 ──────────────────────────────────────────────────────────

/// 空军任务
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AirMission {
    /// 在基地待命
    Idle,
    /// 制空战（驱逐敌机）
    AirSuperiority,
    /// 拦截（被动）
    Interception,
    /// 近距空中支援（CAS：地面战斗加成）
    CloseAirSupport,
    /// 战略轰炸（敌方工业 / 基建）
    StrategicBombing,
    /// 港口打击
    PortStrike,
    /// 海军打击（反舰）
    NavalStrike,
    /// 海上巡逻（侦察 + 反潜）
    NavalPatrol,
    /// 后勤打击（敌补给）
    LogisticalStrike,
    /// 空投
    Drop,
}

/// 空军联队（air wing）— 一个由 N 架同型飞机组成的编队，
/// 部署在一个空区（air region），执行单一任务。
///
/// SoA 布局，count = 联队数。
pub struct AirWingStore {
    pub count: usize,
    /// 持有方
    pub owners: Vec<CountryId>,
    /// 飞机类型 key（如 "fighter" / "cas" / "strategic_bomber"）— 在 GameData.aircraft 中查找
    pub aircraft_keys: Vec<String>,
    /// 部署的空区（u32，可由调用方映射到 strategic_region 或 air_zone）
    pub region_id: Vec<u32>,
    /// 所在机场/基地州（内部 StateId raw；none = u16::MAX）。
    pub base_state: Vec<u16>,
    /// 任务目标空区（与 region_id 不同时表示需要调动；MAX = 同区作业）
    pub target_region: Vec<u32>,
    /// 调动抵达游戏小时戳；0 = 未调动。
    pub transfer_arrival_hour: Vec<u64>,
    /// 作战航程，来自飞机定义 baseline。
    pub range_km: Vec<f32>,
    /// 是否从 aircraft 库存自动补员。
    pub reinforce_enabled: Vec<bool>,
    /// 任务
    pub mission: Vec<AirMission>,
    /// 当前飞机数量（≤ max_count；战损减少）
    pub count_planes: Vec<u32>,
    /// 满员飞机数量
    pub max_planes: Vec<u32>,
    /// 当前组织度（0..max_org）
    pub organisation: Vec<f32>,
    /// 满组织度（来自飞机定义）
    pub max_organisation: Vec<f32>,
    /// 是否在战斗中
    pub in_combat: Vec<bool>,
    /// 名称（"3. Jagdgeschwader" 等）
    pub names: Vec<String>,
    /// 经验（0..900）
    pub experience: Vec<f32>,
}

impl AirWingStore {
    pub fn new() -> Self {
        Self {
            count: 0,
            owners: Vec::new(),
            aircraft_keys: Vec::new(),
            region_id: Vec::new(),
            base_state: Vec::new(),
            target_region: Vec::new(),
            transfer_arrival_hour: Vec::new(),
            range_km: Vec::new(),
            reinforce_enabled: Vec::new(),
            mission: Vec::new(),
            count_planes: Vec::new(),
            max_planes: Vec::new(),
            organisation: Vec::new(),
            max_organisation: Vec::new(),
            in_combat: Vec::new(),
            names: Vec::new(),
            experience: Vec::new(),
        }
    }

    /// 创建一个新联队，返回 [`AirWingId`] 内部索引值。
    pub fn push(
        &mut self,
        owner: CountryId,
        aircraft_key: String,
        region: u32,
        max_planes: u32,
        max_org: f32,
        name: String,
    ) -> u32 {
        let idx = self.count as u32;
        self.owners.push(owner);
        self.aircraft_keys.push(aircraft_key);
        self.region_id.push(region);
        self.base_state.push(region.min(u16::MAX as u32) as u16);
        self.target_region.push(u32::MAX);
        self.transfer_arrival_hour.push(0);
        self.range_km.push(0.0);
        self.reinforce_enabled.push(true);
        self.mission.push(AirMission::Idle);
        self.count_planes.push(max_planes);
        self.max_planes.push(max_planes);
        self.organisation.push(max_org);
        self.max_organisation.push(max_org);
        self.in_combat.push(false);
        self.names.push(name);
        self.experience.push(0.0);
        self.count += 1;
        idx
    }

    /// 取联队的 AirWingId
    pub fn id(idx: u32) -> AirWingId {
        AirWingId(idx)
    }
}

impl Default for AirWingStore {
    fn default() -> Self {
        Self::new()
    }
}
