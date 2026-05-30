//! 玩家划线战线（frontline）数据类型。
//!
//! # 与 `command::Army` 的区别（design.md "设计决策与权衡" §1）
//!
//! 本模块的 [`PlayerArmy`] 是 **玩家划线 frontline 指令容器**：玩家（或 AI）
//! 在地图上画一条战线后，引擎用一支命名集团军把若干师沿线展开，可叠加进攻
//! 箭头沿线推进。`PlayerArmy` 由战线 API 显式创建/解散，存档随 `World`
//! round-trip。
//!
//! 而 [`crate::command::Army`] 是 **OOB（Order of Battle）自动分组结构**：
//! 由 `CommandHierarchy::auto_group` 按 5/3/3 规则把 `DivisionStore` 中的
//! 师团聚成 Corps→Army→ArmyGroup 树，仅用于建制展示。
//!
//! 两者**语义不同、互不替代**：
//! - `PlayerArmy.members` 是直接索引到 `DivisionStore` 的师下标；
//!   `command::Army.corps` 是索引到 `CommandHierarchy.corps`。
//! - 同一个师可以同时属于一个 OOB `command::Army` 和一个划线 `PlayerArmy`。

use crate::ids::{CountryId, ProvinceId};

/// 划线集团军的强类型 id；由 `World::next_army_id` 单调分配，解散不回收。
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ArmyId(pub u32);

impl ArmyId {
    /// 哨兵值，等价于"未指定 / 已解散"。
    pub const NONE: Self = Self(u32::MAX);

    /// 返回内部 `u32`。
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// 是否为 [`ArmyId::NONE`] 哨兵。
    pub const fn is_none(self) -> bool {
        self.0 == u32::MAX
    }
}

/// 将领的强类型 id；由 `World::next_general_id` 单调分配。
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct GeneralId(pub u32);

impl GeneralId {
    pub const NONE: Self = Self(u32::MAX);

    pub const fn raw(self) -> u32 {
        self.0
    }

    pub const fn is_none(self) -> bool {
        self.0 == u32::MAX
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct General {
    pub id: GeneralId,
    pub owner: CountryId,
    pub name: String,
    pub skill: u8,
    pub attack: u8,
    pub defense: u8,
    pub planning: u8,
    pub logistics: u8,
    pub command_limit: u16,
}

/// 进攻箭头：沿战线推进的有序敌占省序列。
///
/// `provinces` 长度运行时维持 ∈ [1, 32]，相邻省陆地相邻。
#[derive(Clone, Debug, PartialEq)]
pub struct OffensiveArrow {
    pub provinces: Vec<ProvinceId>,
}

/// 战线指令：一支 `PlayerArmy` 当前的路径 + 可选进攻箭头。
///
/// `path` 长度运行时维持 ∈ [1, 64]，相邻省陆地相邻、无重复、控制者
/// ∈ {owner, co-belligerent}。运行时由 `tick_frontlines` 维持合法性。
#[derive(Clone, Debug, PartialEq)]
pub struct FrontlineOrder {
    /// 有序、无重复，长度 ∈ [1, 64]，每对相邻省陆地相邻。
    pub path: Vec<ProvinceId>,
    /// 可选叠加的进攻箭头。
    pub arrow: Option<OffensiveArrow>,
    /// 锚点省：箭头第一省的相邻战线省。R6.6 易手时滚动选取。
    pub anchor: Option<ProvinceId>,
    /// 有 path 时为 true：师自动分布到战线上。
    pub active: bool,
    /// 玩家点击"执行计划"后为 true：师沿箭头推进。
    pub executing: bool,
}

/// 玩家划线集团军：战线指令容器。
///
/// `members` 是 `DivisionStore` 索引；属性 F 不变量要求每个下标至多在 1
/// 个 `PlayerArmy` 中。
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerArmy {
    pub id: ArmyId,
    pub name: String,
    pub owner: CountryId,
    pub commander: Option<GeneralId>,
    /// `DivisionStore` 索引；不变量：每个下标至多在 1 个集团军中（属性 F）。
    pub members: Vec<usize>,
    pub order: Option<FrontlineOrder>,
}
