//! 科技定义（来自 `common/technologies/`）。
//!
//! HOI4 中每条 tech 大致结构：
//! ```text
//! infantry_weapons1 = {
//!     research_cost = 1.5
//!     start_year = 1936
//!     enable_equipments = { infantry_equipment_1 }
//!     enable_subunits = { infantry }
//!     path = { leads_to_tech = infantry_weapons2 research_cost_coeff = 1 }
//!     categories = { infantry_weapons }
//!     folder = { name = infantry_folder position = { x = 0 y = 2 } }
//! }
//! ```
//!
//! 我们抽出经济/解锁有用的字段，丢弃 ai_will_do、modifier 等纯 AI/UI 信息。

#[derive(Debug, Clone)]
pub struct TechPath {
    /// 后续科技 key（必须先研发本科技才能研发它）
    pub leads_to: String,
    /// 该路径附带的研发成本系数（默认 1）
    pub research_cost_coeff: f32,
}

#[derive(Debug, Clone)]
pub struct Technology {
    /// 唯一 key，如 `infantry_weapons1` / `concentrated_industry` / `land_doctrine`
    pub key: String,
    /// 基础研发成本（HOI4 单位，1.0 ≈ 100 天）
    pub research_cost: f32,
    /// 历史"按时"年份（提前研发会受惩罚）
    pub start_year: u16,
    /// 后续路径（可能多条，分支用）
    pub paths: Vec<TechPath>,
    /// 类别标签，用于 doctrine 判断、加成累计
    pub categories: Vec<String>,
    /// folder 名（如 `industry_folder` / `land_doctrine_folder`）
    pub folder: Option<String>,
    /// 解锁的装备 key 列表
    pub enable_equipments: Vec<String>,
    /// 解锁的子单位（兵种）类型
    pub enable_subunits: Vec<String>,
    /// 解锁的装备模块
    pub enable_equipment_modules: Vec<String>,
    /// 解锁的建筑
    pub enable_building: Vec<String>,
    /// 是否为学说（doctrine）— 通过 folder 名包含 "doctrine" 判断
    pub is_doctrine: bool,
}

impl Technology {
    /// 计算"前置依赖"：所有指向本 tech 的其它 tech key
    /// （注意：HOI4 是从 parent.path.leads_to → child，所以 child 的依赖不在自身上声明，
    ///  需要在加载层做反向索引。`Technology` 本身只持有出边路径）
    pub fn outgoing_paths(&self) -> &[TechPath] {
        &self.paths
    }

    /// 是否在某年之前为"提前研发"
    pub fn is_ahead_of_time(&self, year: u16) -> bool {
        year < self.start_year
    }

    /// 提前研发的年数（可能为负）
    pub fn years_ahead(&self, year: u16) -> i32 {
        self.start_year as i32 - year as i32
    }
}
