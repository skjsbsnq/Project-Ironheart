//! V6 POP 模型：6 阶级 × 职业槽（D1 简化）。

use serde::{Deserialize, Serialize};

use crate::buildings_v6::BuildingId;
use crate::ids::StateId;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum PopClass {
    Peasant,
    Worker,
    Clerk,
    Capitalist,
    Aristocrat,
    Soldier,
}

impl PopClass {
    pub const COUNT: usize = 6;

    pub fn index(self) -> usize {
        match self {
            Self::Peasant => 0,
            Self::Worker => 1,
            Self::Clerk => 2,
            Self::Capitalist => 3,
            Self::Aristocrat => 4,
            Self::Soldier => 5,
        }
    }

    pub fn from_index(i: usize) -> Option<Self> {
        match i {
            0 => Some(Self::Peasant),
            1 => Some(Self::Worker),
            2 => Some(Self::Clerk),
            3 => Some(Self::Capitalist),
            4 => Some(Self::Aristocrat),
            5 => Some(Self::Soldier),
            _ => None,
        }
    }

    pub fn baseline_literacy(self) -> f32 {
        match self {
            Self::Peasant => 0.35,
            Self::Worker => 0.55,
            Self::Clerk => 0.80,
            Self::Capitalist => 0.85,
            Self::Aristocrat => 0.75,
            Self::Soldier => 0.55,
        }
    }

    pub fn baseline_skilled_ratio(self) -> f32 {
        match self {
            Self::Peasant => 0.05,
            Self::Worker => 0.18,
            Self::Clerk => 0.35,
            Self::Capitalist => 0.40,
            Self::Aristocrat => 0.25,
            Self::Soldier => 0.15,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PopGroup {
    pub class: PopClass,
    pub state: StateId,
    pub size: u32,
    pub employed_at: Option<BuildingId>,
    pub wage_rm: f32,
    /// Effective daily tax pressure written by finance tick and read by satisfaction formulas.
    pub tax_burden: f32,
    /// 每人每日总收入 (RM)，包括工资和资本家分红
    #[serde(default)]
    pub income_rm: f32,
    /// 每人每日实际缴税额 (RM)
    #[serde(default)]
    pub tax_paid_rm: f32,
    /// 每人每日可支配收入 = income_rm - tax_paid_rm
    #[serde(default)]
    pub disposable_income_rm: f32,
    /// 每人每日基础消费预算上限 (RM)，由可支配收入决定
    #[serde(default)]
    pub basic_consumption_budget: f32,
    pub satisfaction_law_modifier: f32,
    pub loyalty_coefficient: f32,
    pub loyalty_decay_mult: f32,
    pub satisfaction: f32,
    pub political_loyalty: f32,
    pub literacy: f32,
    pub skilled_ratio: f32,
    pub standard_of_living: f32,
    pub needs_fulfillment: f32,
    pub essential_needs_fulfillment: f32,
    pub normal_needs_fulfillment: f32,
    pub luxury_needs_fulfillment: f32,
    pub radicalism: f32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PopStore {
    pub groups: Vec<PopGroup>,
}

impl PopStore {
    pub fn new() -> Self {
        Self { groups: Vec::new() }
    }

    pub fn pops_by_class_in_country(
        &self,
        class: PopClass,
        state_ids: &[StateId],
    ) -> Vec<&PopGroup> {
        self.groups
            .iter()
            .filter(|p| p.class == class && state_ids.contains(&p.state))
            .collect()
    }

    pub fn pops_by_class_in_country_mut(
        &mut self,
        class: PopClass,
        state_ids: &[StateId],
    ) -> Vec<usize> {
        self.groups
            .iter()
            .enumerate()
            .filter(|(_, p)| p.class == class && state_ids.contains(&p.state))
            .map(|(i, _)| i)
            .collect()
    }
}
