//! V6 建筑模型：建筑等级 × 雇佣 POP × 生产方法（PM）。

use serde::{Deserialize, Serialize};

use crate::ids::{CountryId, StateId};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveProductionMethod {
    pub group: String,
    pub pm_id: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum BuildingKind {
    Resource,
    Industrial,
    Agriculture,
    ConsumerGoods,
    Service,
    Military,
    Infrastructure,
    MilitaryBase,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum BuildingOwner {
    Private,
    State,
    Cartel,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum OwnershipAccount {
    State {
        country: CountryId,
    },
    DomesticPrivate {
        country: CountryId,
    },
    Cartel {
        country: CountryId,
    },
    Overlord {
        master: CountryId,
        subject: CountryId,
    },
    ForeignPrivate {
        country: CountryId,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OwnershipShare {
    pub account: OwnershipAccount,
    pub share: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct BuildingId(pub u32);

impl BuildingId {
    pub const NONE: Self = Self(u32::MAX);
    pub fn is_none(self) -> bool {
        self.0 == u32::MAX
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Building {
    pub kind: BuildingKind,
    pub building_def_id: String,
    pub state: StateId,
    pub level: u8,
    pub active_pm: String,
    #[serde(default)]
    pub active_pm_by_group: Vec<ActiveProductionMethod>,
    pub employment: [u32; 6],
    pub owner: BuildingOwner,
    #[serde(default)]
    pub ownership_shares: Vec<OwnershipShare>,
    pub requires_law: Option<(LawCategory, String)>,
    #[serde(default)]
    pub production_rate: f32,
    #[serde(default)]
    pub output_value_gbp: f64,
    #[serde(default)]
    pub wage_rm: f32,
    #[serde(default)]
    pub profit_rm: f64,
    #[serde(default)]
    pub input_cost_rm: f64,
    #[serde(default)]
    pub estimated_profit_rm: f64,
    #[serde(default)]
    pub value_added_rm: f64,
    #[serde(default)]
    pub cp_cost: f32,
    #[serde(default)]
    pub max_level: u8,
    #[serde(default)]
    pub built_progress: f32,
}

impl Building {
    pub fn runtime_defaults() -> Self {
        Self {
            kind: BuildingKind::Resource,
            building_def_id: String::new(),
            state: StateId::NONE,
            level: 0,
            active_pm: String::new(),
            active_pm_by_group: Vec::new(),
            employment: [0; 6],
            owner: BuildingOwner::Private,
            ownership_shares: Vec::new(),
            requires_law: None,
            production_rate: 0.0,
            output_value_gbp: 0.0,
            wage_rm: 0.0,
            profit_rm: 0.0,
            input_cost_rm: 0.0,
            estimated_profit_rm: 0.0,
            value_added_rm: 0.0,
            cp_cost: 0.0,
            max_level: 0,
            built_progress: 0.0,
        }
    }

    pub fn default_ownership_shares(
        owner: BuildingOwner,
        country: CountryId,
    ) -> Vec<OwnershipShare> {
        match owner {
            BuildingOwner::State => vec![OwnershipShare {
                account: OwnershipAccount::State { country },
                share: 1.0,
            }],
            BuildingOwner::Private => vec![OwnershipShare {
                account: OwnershipAccount::DomesticPrivate { country },
                share: 1.0,
            }],
            BuildingOwner::Cartel => vec![
                OwnershipShare {
                    account: OwnershipAccount::Cartel { country },
                    share: 0.70,
                },
                OwnershipShare {
                    account: OwnershipAccount::State { country },
                    share: 0.30,
                },
            ],
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BuildingStore {
    pub buildings: Vec<Building>,
}

impl BuildingStore {
    pub fn new() -> Self {
        Self {
            buildings: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum LawCategory {
    Conscription,
    Economy,
    Trade,
    Taxation,
    CivilRights,
    InformationControl,
}

impl LawCategory {
    pub const COUNT: usize = 6;

    pub fn index(self) -> usize {
        match self {
            Self::Conscription => 0,
            Self::Economy => 1,
            Self::Trade => 2,
            Self::Taxation => 3,
            Self::CivilRights => 4,
            Self::InformationControl => 5,
        }
    }
}
