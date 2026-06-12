use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{
    validate_focus_tree, DecisionDb, EventDb, EventValidationError, FocusTree,
    ScriptedSurrenderDef, SituationDb, SituationDef, SituationValidationError, SurrenderDb,
    SurrenderValidationError, ValidationError,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScenarioManifest {
    #[serde(default)]
    pub event_libraries: Vec<String>,
    #[serde(default)]
    pub focus_trees: Vec<String>,
    #[serde(default)]
    pub decision_libraries: Vec<String>,
    #[serde(default)]
    pub situation_libraries: Vec<String>,
    #[serde(default)]
    pub surrender_libraries: Vec<String>,
    #[serde(default)]
    pub history_profiles: Vec<String>,
    #[serde(default)]
    pub situation_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ScenarioContent {
    pub manifest: ScenarioManifest,
    pub events: EventDb,
    pub focus_trees: Vec<FocusTree>,
    pub decisions: DecisionDb,
    pub situations: Vec<SituationDef>,
    pub surrenders: Vec<ScriptedSurrenderDef>,
}

impl ScenarioContent {
    pub fn primary_focus_tree(&self) -> Option<&FocusTree> {
        self.focus_trees.first()
    }
}

#[derive(Debug)]
pub enum RegistryError {
    UnknownScenario(String),
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    ParseManifest {
        path: PathBuf,
        source: ron::error::SpannedError,
    },
    ParseEvents {
        path: PathBuf,
        source: ron::error::SpannedError,
    },
    ParseFocusTree {
        path: PathBuf,
        source: ron::error::SpannedError,
    },
    ParseDecisions {
        path: PathBuf,
        source: ron::error::SpannedError,
    },
    ParseSituations {
        path: PathBuf,
        source: ron::error::SpannedError,
    },
    EventValidation(EventValidationError),
    FocusValidation {
        country: String,
        error: ValidationError,
    },
    DecisionValidation(crate::DecisionValidationError),
    SituationValidation(SituationValidationError),
    SurrenderValidation(SurrenderValidationError),
    ParseSurrenders {
        path: PathBuf,
        source: ron::error::SpannedError,
    },
    EmptyFocusTrees,
    MissingFile(PathBuf),
    DuplicateSituationId(String),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownScenario(id) => write!(f, "unknown scenario '{id}'"),
            Self::Io { path, source } => write!(f, "failed to read {}: {source}", path.display()),
            Self::ParseManifest { path, source } => {
                write!(f, "failed to parse manifest {}: {source}", path.display())
            }
            Self::ParseEvents { path, source } => {
                write!(
                    f,
                    "failed to parse event library {}: {source}",
                    path.display()
                )
            }
            Self::ParseFocusTree { path, source } => {
                write!(f, "failed to parse focus tree {}: {source}", path.display())
            }
            Self::ParseDecisions { path, source } => {
                write!(
                    f,
                    "failed to parse decision library {}: {source}",
                    path.display()
                )
            }
            Self::ParseSituations { path, source } => {
                write!(
                    f,
                    "failed to parse situation library {}: {source}",
                    path.display()
                )
            }
            Self::EventValidation(error) => write!(f, "event validation failed: {error}"),
            Self::FocusValidation { country, error } => {
                write!(f, "focus validation failed for {country}: {error}")
            }
            Self::DecisionValidation(error) => write!(f, "decision validation failed: {error}"),
            Self::SituationValidation(error) => write!(f, "situation validation failed: {error}"),
            Self::SurrenderValidation(error) => write!(f, "surrender validation failed: {error}"),
            Self::ParseSurrenders { path, source } => {
                write!(
                    f,
                    "failed to parse surrender library {}: {source}",
                    path.display()
                )
            }
            Self::EmptyFocusTrees => write!(f, "scenario contains no focus trees"),
            Self::MissingFile(path) => {
                write!(f, "manifest references missing file {}", path.display())
            }
            Self::DuplicateSituationId(id) => write!(f, "duplicate situation id: {id}"),
        }
    }
}

impl std::error::Error for RegistryError {}

pub fn load_scenario_content(scenario: &str) -> Result<ScenarioContent, RegistryError> {
    let content_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("content");
    let manifest_path = match scenario {
        "1936" => content_root.join("scenarios").join("1936.ron"),
        other => return Err(RegistryError::UnknownScenario(other.to_owned())),
    };
    load_scenario_content_from_manifest(&content_root, &manifest_path)
}

pub fn load_scenario_content_from_manifest(
    content_root: &Path,
    manifest_path: &Path,
) -> Result<ScenarioContent, RegistryError> {
    let manifest_text = read_to_string(manifest_path)?;
    let manifest: ScenarioManifest =
        ron::from_str(&manifest_text).map_err(|source| RegistryError::ParseManifest {
            path: manifest_path.to_path_buf(),
            source,
        })?;

    for rel in manifest
        .event_libraries
        .iter()
        .chain(manifest.focus_trees.iter())
        .chain(manifest.decision_libraries.iter())
    {
        let path = content_root.join(rel);
        if !path.is_file() {
            return Err(RegistryError::MissingFile(path));
        }
    }

    for rel in &manifest.history_profiles {
        let path = content_root.join(rel);
        if !path.exists() {
            return Err(RegistryError::MissingFile(path));
        }
    }

    for rel in &manifest.situation_libraries {
        let path = content_root.join(rel);
        if !path.is_file() {
            return Err(RegistryError::MissingFile(path));
        }
    }

    for rel in &manifest.surrender_libraries {
        let path = content_root.join(rel);
        if !path.is_file() {
            return Err(RegistryError::MissingFile(path));
        }
    }

    let mut seen_situations = HashSet::new();
    for id in &manifest.situation_ids {
        if !seen_situations.insert(id.as_str()) {
            return Err(RegistryError::DuplicateSituationId(id.clone()));
        }
    }

    let mut events = Vec::new();
    for rel in &manifest.event_libraries {
        let path = content_root.join(rel);
        let text = read_to_string(&path)?;
        let db = EventDb::from_ron(&text).map_err(|source| RegistryError::ParseEvents {
            path: path.clone(),
            source,
        })?;
        if db.events.is_empty() {
            return Err(RegistryError::EventValidation(
                EventValidationError::EmptyDb,
            ));
        }
        events.extend(db.events);
    }
    let events = EventDb { events };
    for error in events.validate() {
        return Err(RegistryError::EventValidation(error));
    }

    let mut focus_trees = Vec::new();
    for rel in &manifest.focus_trees {
        let path = content_root.join(rel);
        let text = read_to_string(&path)?;
        let tree = FocusTree::from_ron(&text).map_err(|source| RegistryError::ParseFocusTree {
            path: path.clone(),
            source,
        })?;
        for error in validate_focus_tree(&tree) {
            return Err(RegistryError::FocusValidation {
                country: tree.country.clone(),
                error,
            });
        }
        focus_trees.push(tree);
    }
    if focus_trees.is_empty() {
        return Err(RegistryError::EmptyFocusTrees);
    }

    let mut decisions = Vec::new();
    for rel in &manifest.decision_libraries {
        let path = content_root.join(rel);
        let text = read_to_string(&path)?;
        let db = DecisionDb::from_ron(&text).map_err(|source| RegistryError::ParseDecisions {
            path: path.clone(),
            source,
        })?;
        if let Some(error) = db.validate().into_iter().next() {
            return Err(RegistryError::DecisionValidation(error));
        }
        decisions.extend(db.decisions);
    }
    let decisions = DecisionDb { decisions };
    if let Some(error) = decisions.validate().into_iter().next() {
        return Err(RegistryError::DecisionValidation(error));
    }

    let mut situations = Vec::new();
    for rel in &manifest.situation_libraries {
        let path = content_root.join(rel);
        let text = read_to_string(&path)?;
        let db = SituationDb::from_ron(&text).map_err(|source| RegistryError::ParseSituations {
            path: path.clone(),
            source,
        })?;
        if let Some(error) = db.validate().into_iter().next() {
            return Err(RegistryError::SituationValidation(error));
        }
        situations.extend(db.situations);
    }

    let mut seen_situation_defs = HashSet::new();
    for def in &situations {
        if !seen_situation_defs.insert(def.id.as_str()) {
            return Err(RegistryError::SituationValidation(
                SituationValidationError::DuplicateId(def.id.clone()),
            ));
        }
    }

    let mut surrenders = Vec::new();
    for rel in &manifest.surrender_libraries {
        let path = content_root.join(rel);
        let text = read_to_string(&path)?;
        let db = SurrenderDb::from_ron(&text).map_err(|source| RegistryError::ParseSurrenders {
            path: path.clone(),
            source,
        })?;
        if let Some(error) = db.validate().into_iter().next() {
            return Err(RegistryError::SurrenderValidation(error));
        }
        surrenders.extend(db.surrenders);
    }

    let mut seen_surrenders = HashSet::new();
    for def in &surrenders {
        if !seen_surrenders.insert(def.id.as_str()) {
            return Err(RegistryError::SurrenderValidation(
                SurrenderValidationError::DuplicateId(def.id.clone()),
            ));
        }
    }

    Ok(ScenarioContent {
        manifest,
        events,
        focus_trees,
        decisions,
        situations,
        surrenders,
    })
}

fn read_to_string(path: &Path) -> Result<String, RegistryError> {
    std::fs::read_to_string(path).map_err(|source| RegistryError::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_1936_scenario_content() {
        let content = load_scenario_content("1936").expect("1936 scenario loads");
        assert!(content.events.find("germany.rhineland").is_some());
        assert!(content.events.find("news.scw_outbreak").is_some());
        assert!(content.events.find("spain.scw_choose_side").is_some());
        assert_eq!(content.primary_focus_tree().unwrap().country, "GER");
        assert!(content.decisions.find("ger.volkswagen_werk").is_some());
        assert!(content
            .decisions
            .find("ger.establish_bohemia_moravia")
            .is_some());
        assert!(content.decisions.find("ger.establish_slovakia").is_some());
        assert!(content
            .situations
            .iter()
            .any(|s| s.id == "spanish_civil_war"));
        assert!(!content
            .situations
            .iter()
            .any(|s| s.id == "spanish_anarchist_uprising"));
        assert!(content
            .situations
            .iter()
            .any(|s| s.id == "italo_ethiopian_war"));
        let eth = content
            .situations
            .iter()
            .find(|s| s.id == "italo_ethiopian_war")
            .unwrap();
        assert_eq!(eth.start_date, (0, 0, 0));
        assert!(content
            .manifest
            .history_profiles
            .iter()
            .any(|p| p == "history_1936"));
        assert!(content.surrenders.iter().any(|s| s.id == "fra_surrender"));
    }

    #[test]
    fn unknown_scenario_is_rejected() {
        assert!(matches!(
            load_scenario_content("1941"),
            Err(RegistryError::UnknownScenario(id)) if id == "1941"
        ));
    }
}
