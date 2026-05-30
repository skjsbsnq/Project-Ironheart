//! V6 法律 schema（D7：6 大类）。

use serde::{Deserialize, Serialize};

use crate::buildings_v6::LawCategory;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LawSlot {
    pub category: LawCategory,
    pub current: String,
    pub cooldown_days: u16,
    pub pending: Option<(String, u16)>,
    pub previous_before_lock: Option<String>,
    pub is_locked: bool,
}

impl LawSlot {
    pub fn new(category: LawCategory, default_law: &str) -> Self {
        Self {
            category,
            current: default_law.to_owned(),
            cooldown_days: 0,
            pending: None,
            previous_before_lock: None,
            is_locked: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LawSet(pub [LawSlot; LawCategory::COUNT]);

impl Default for LawSet {
    fn default() -> Self {
        Self([
            LawSlot::new(LawCategory::Conscription, "volunteer_only"),
            LawSlot::new(LawCategory::Economy, "laissez_faire"),
            LawSlot::new(LawCategory::Trade, "free_trade"),
            LawSlot::new(LawCategory::Taxation, "medium_taxation"),
            LawSlot::new(LawCategory::CivilRights, "limited_rights"),
            LawSlot::new(LawCategory::InformationControl, "regulated_press"),
        ])
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LawStore {
    pub law_sets: Vec<LawSet>,
}

impl LawStore {
    pub fn new(count: usize) -> Self {
        Self {
            law_sets: (0..count).map(|_| LawSet::default()).collect(),
        }
    }

    pub fn ensure_capacity(&mut self, n: usize) {
        while self.law_sets.len() < n {
            self.law_sets.push(LawSet::default());
        }
    }
}
