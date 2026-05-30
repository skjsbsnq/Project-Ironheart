pub mod air;
pub mod building;
pub mod characters;
pub mod country;
pub mod equipment;
pub mod history;
pub mod loader;
pub mod military;
pub mod naval;
pub mod politics;
pub mod resource;
pub mod state;
pub mod technology;

pub use air::{AircraftDef, AircraftKind};
pub use building::{BuildingDef, BuildingScope};
pub use characters::{
    load_characters, select_country_leader, select_country_leader_with_recruits, CharacterDef,
};
pub use country::{Color, Country, CountryTag};
pub use equipment::{EquipmentCategory, EquipmentDef};
pub use history::{
    AirWingEntry, CountryHistory, DivisionInstance, FleetSpec, OobAir, OobLand, OobNaval, ShipSpec,
    TaskForceSpec,
};
pub use loader::{GameData, GameDataLoadReport, LoadWarning, LoadedCounts, StateCategoryDef};
pub use military::{
    CombatTactic, DivisionTemplate, EditableDivisionTemplate, ReinforcementPriority, SubunitDef,
    TemplateEditorCommand, TemplatePreview,
};
pub use naval::{ShipClassDef, ShipKind};
pub use politics::{DecisionCategoryDef, DecisionDef, FocusTree, IdeaDef, Ideology, NationalFocus};
pub use resource::{ResourceDef, ResourceKind};
pub use state::State;
pub use technology::{TechPath, Technology};
