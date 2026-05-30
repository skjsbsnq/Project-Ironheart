use serde::{Deserialize, Serialize};

use crate::SituationEffect;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SurrenderDb {
    pub surrenders: Vec<ScriptedSurrenderDef>,
}

impl SurrenderDb {
    pub fn from_ron(s: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(s)
    }

    pub fn validate(&self) -> Vec<SurrenderValidationError> {
        let mut errors = Vec::new();
        if self.surrenders.is_empty() {
            errors.push(SurrenderValidationError::EmptyDb);
            return errors;
        }

        let mut seen = std::collections::HashSet::new();
        for def in &self.surrenders {
            if !seen.insert(def.id.as_str()) {
                errors.push(SurrenderValidationError::DuplicateId(def.id.clone()));
            }
            if def.target.trim().is_empty() {
                errors.push(SurrenderValidationError::MissingTarget(def.id.clone()));
            }
            if def.enemy_side.is_empty() {
                errors.push(SurrenderValidationError::MissingEnemySide(def.id.clone()));
            }
        }
        errors
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptedSurrenderDef {
    pub id: String,
    pub target: String,
    #[serde(default)]
    pub enemy_side: Vec<String>,
    #[serde(default = "default_capital_required")]
    pub require_capital_lost: bool,
    #[serde(default = "default_core_control_ratio")]
    pub max_target_core_control_ratio: f32,
    #[serde(default)]
    pub remove_from_wars: bool,
    #[serde(default)]
    pub effects: Vec<SituationEffect>,
}

fn default_capital_required() -> bool {
    true
}

fn default_core_control_ratio() -> f32 {
    0.35
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurrenderValidationError {
    EmptyDb,
    DuplicateId(String),
    MissingTarget(String),
    MissingEnemySide(String),
}

impl std::fmt::Display for SurrenderValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyDb => write!(f, "surrender db is empty"),
            Self::DuplicateId(id) => write!(f, "duplicate surrender id: {id}"),
            Self::MissingTarget(id) => write!(f, "surrender '{id}' has empty target"),
            Self::MissingEnemySide(id) => write!(f, "surrender '{id}' has no enemy_side"),
        }
    }
}
