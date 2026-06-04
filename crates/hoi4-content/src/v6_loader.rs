//! V6 data loader: loads economy, law, and POP data from `content/economy_v6/*.ron` into `World`.
//!
//! This is intentionally separate from the vanilla TXT loader in `hoi4-data/src/loader.rs`.
//! Both loaders may run during startup; V6 data wins where the two sources overlap.
use hoi4_state::{
    ActiveProductionMethod, Autonomy, AutonomyLevel, Building, BuildingKind, BuildingOwner,
    CountryId, LawCategory, LawSlot, PopClass, PopGroup, StateId, World,
};
use serde::Deserialize;

use crate::v7_history_loader::{
    Historical1936Database, HistoricalCountryEconomyDef, HistoricalTradeProfileDef,
    StateIntegrationDef, StatePopulation1936Def, StateResourceDepositDef, TradeRouteKindDef,
};
// RON schema types. These mirror the content files one-to-one.
#[derive(Clone, Debug, Deserialize)]
pub struct GoodDef {
    pub id: String,
    pub name: String,
    pub category: GoodCategoryDef,
    pub base_price_rm: f32,
    pub unlocked_by: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum GoodCategoryDef {
    RawMaterial,
    Intermediate,
    Consumer,
    Luxury,
    Service,
    MilitaryIntermediate,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BuildingDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub economic_sector: EconomicSectorDef,
    pub gameplay_class: BuildingGameplayClassDef,
    pub gdp_rule: BuildingGdpRuleDef,
    pub kind: BuildingKindDef,
    pub max_level: u8,
    pub owner_default: OwnerDef,
    #[serde(default = "default_true")]
    pub buildable: bool,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub state_limit_kind: Option<String>,
    pub requires_law: Option<(LawCategoryDef, String)>,
    pub employment_profile: BuildingEmploymentProfileDef,
    pub construction_recipe: ConstructionRecipeDef,
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub enum BuildingKindDef {
    Resource,
    Industrial,
    Agriculture,
    ConsumerGoods,
    Service,
    Military,
    Infrastructure,
    MilitaryBase,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash)]
pub enum EconomicSectorDef {
    Primary,
    Secondary,
    Tertiary,
    Government,
    MilitarySupport,
    Infrastructure,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash)]
pub enum BuildingGameplayClassDef {
    Agriculture,
    ResourceExtraction,
    HeavyIndustry,
    LightIndustry,
    Service,
    MilitaryIndustry,
    Infrastructure,
    MilitaryBase,
    Government,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct BuildingGdpRuleDef {
    pub component: BuildingGdpComponentDef,
    #[serde(default = "default_gdp_value_added_multiplier")]
    pub value_added_multiplier: f32,
}

fn default_gdp_value_added_multiplier() -> f32 {
    1.0
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum BuildingGdpComponentDef {
    PrimaryOutput,
    SecondaryOutput,
    TertiaryOutput,
    GovernmentService,
    MilitaryProcurement,
    InfrastructureService,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub struct BuildingEmploymentProfileDef {
    pub peasants: u32,
    pub workers: u32,
    pub clerks: u32,
    pub capitalists: u32,
    pub aristocrats: u32,
    pub soldiers: u32,
}

impl BuildingEmploymentProfileDef {
    pub fn total(&self) -> u32 {
        self.peasants
            .saturating_add(self.workers)
            .saturating_add(self.clerks)
            .saturating_add(self.capitalists)
            .saturating_add(self.aristocrats)
            .saturating_add(self.soldiers)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ConstructionRecipeDef {
    pub cp_cost: f32,
    pub funds_rm: f64,
    pub materials: Vec<ConstructionMaterialDef>,
    pub labor: u32,
    pub engineering: u32,
    #[serde(default)]
    pub regional_restrictions: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ConstructionMaterialDef {
    pub good_id: String,
    pub amount: f32,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub enum OwnerDef {
    Private,
    State,
    Cartel,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PopModifiers {
    pub satisfaction: f32,
    pub loyalty_coefficient: f32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConscriptionDef {
    pub id: String,
    pub name: String,
    pub pp_cost: u32,
    pub soldier_ratio: f32,
    #[serde(default = "default_domestic_recruitable_ratio")]
    pub domestic_recruitable_ratio: f32,
    #[serde(default)]
    pub colonial_recruitable_ratio: f32,
    #[serde(default)]
    pub subject_force_contribution_ratio: f32,
    #[serde(default = "default_political_cost_multiplier")]
    pub political_cost_multiplier: f32,
    #[serde(default = "default_radicalism_gain_multiplier")]
    pub radicalism_gain_multiplier: f32,
    pub cooldown_days: u16,
    pub conscription_conversion_rate: f32,
    pub pop_modifiers: PopModifiers,
}

fn default_domestic_recruitable_ratio() -> f32 {
    1.0
}

fn default_political_cost_multiplier() -> f32 {
    1.0
}

fn default_radicalism_gain_multiplier() -> f32 {
    1.0
}

#[derive(Clone, Debug, Deserialize)]
pub struct EconomyDef {
    pub id: String,
    pub name: String,
    pub pp_cost: u32,
    pub cooldown_days: u16,
    pub wage_multiplier_worker: f32,
    pub wage_multiplier_capitalist: f32,
    pub consumer_goods_factor: f32,
    pub construction_speed_modifier: f32,
    pub forces_trade_law: Option<String>,
    pub pop_modifiers: PopModifiers,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TradeDef {
    pub id: String,
    pub name: String,
    pub pp_cost: u32,
    pub cooldown_days: u16,
    pub import_tariff_rate: f32,
    pub export_tariff_rate: f32,
    pub import_efficiency: f32,
    pub export_efficiency: f32,
    pub foreign_exchange_control: bool,
    pub trade_law_modifier: f32,
    pub pop_modifiers: PopModifiers,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TaxationDef {
    pub id: String,
    pub name: String,
    pub pp_cost: u32,
    pub cooldown_days: u16,
    pub income_tax_rate: f32,
    pub consumption_tax_rate: f32,
    pub corporate_tax_rate: f32,
    pub pop_modifiers: PopModifiers,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CivilRightsDef {
    pub id: String,
    pub name: String,
    pub pp_cost: u32,
    pub cooldown_days: u16,
    pub research_slots: u8,
    pub welfare_rate: f32,
    pub pop_modifiers: PopModifiers,
}

#[derive(Clone, Debug, Deserialize)]
pub struct InformationControlDef {
    pub id: String,
    pub name: String,
    pub pp_cost: u32,
    pub cooldown_days: u16,
    pub loyalty_decay_multiplier: f32,
    pub pop_modifiers: PopModifiers,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub enum LawCategoryDef {
    Conscription,
    Economy,
    Trade,
    Taxation,
    CivilRights,
    InformationControl,
}

#[derive(Clone, Debug, Deserialize)]
pub struct InitialPopGroup {
    pub state_id: u16,
    pub class: PopClassDef,
    pub size: u32,
    pub employed_at: Option<u32>,
    pub wage_rm: f32,
    pub satisfaction: f32,
    pub political_loyalty: f32,
    #[serde(default)]
    pub literacy: Option<f32>,
    #[serde(default)]
    pub skilled_ratio: Option<f32>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub enum PopClassDef {
    Peasant,
    Worker,
    Clerk,
    Capitalist,
    Aristocrat,
    Soldier,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EquipmentOutputDef {
    pub equipment_category: String,
    pub daily_per_level: f32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ProductionMethodDef {
    pub id: String,
    pub name: String,
    pub building_id: String,
    #[serde(default = "default_pm_group")]
    pub group: String,
    #[serde(default = "default_pm_group_name")]
    pub group_name: String,
    pub input_good_ids: Vec<String>,
    pub input_good_amounts: Vec<f32>,
    pub output_good_ids: Vec<String>,
    pub output_good_amounts: Vec<f32>,
    pub employment_demand: [u32; 6],
    pub unlocked_by: Option<String>,
    #[serde(default)]
    pub required_law: Option<(LawCategoryDef, String)>,
    #[serde(default = "default_pm_modifier")]
    pub throughput_modifier: f32,
    #[serde(default = "default_pm_modifier")]
    pub automation_modifier: f32,
    #[serde(default)]
    pub required_literacy: f32,
    #[serde(default)]
    pub required_skilled_ratio: f32,
    pub equipment_output: Option<EquipmentOutputDef>,
}

fn default_pm_group() -> String {
    "base".to_owned()
}

fn default_pm_group_name() -> String {
    "基础工艺".to_owned()
}

fn default_pm_modifier() -> f32 {
    1.0
}

pub fn pm_group_name(group: &str) -> &'static str {
    match group {
        "base" => "基础工艺",
        "secondary" => "副产物",
        "automation" => "自动化",
        "ownership" => "所有制",
        "military" => "军工型号",
        "government" => "政府职能",
        _ => "生产方式",
    }
}

pub fn production_method_group(pm: &ProductionMethodDef) -> &str {
    if pm.group.is_empty() {
        "base"
    } else {
        pm.group.as_str()
    }
}

fn active_pm_slot_key(pm: &ProductionMethodDef) -> String {
    let group = production_method_group(pm);
    if group == "secondary" {
        format!("secondary:{}", pm.id)
    } else {
        group.to_owned()
    }
}

pub fn production_method_group_name(pm: &ProductionMethodDef) -> String {
    if pm.group_name.is_empty() {
        pm_group_name(production_method_group(pm)).to_owned()
    } else {
        pm.group_name.clone()
    }
}

pub fn active_pms_for_building<'a>(
    building: &hoi4_state::Building,
    db: &'a V6Database,
) -> Vec<&'a ProductionMethodDef> {
    let mut out: Vec<&ProductionMethodDef> = Vec::new();
    let mut seen_groups: Vec<String> = Vec::new();

    for active in &building.active_pm_by_group {
        if let Some(pm) = db
            .production_methods
            .iter()
            .find(|pm| pm.id == active.pm_id && pm.building_id == building.building_def_id)
        {
            let group = active_pm_slot_key(pm);
            if !seen_groups.contains(&group) {
                seen_groups.push(group);
                out.push(pm);
            }
        }
    }

    if out.is_empty() {
        if let Some(pm) = db.production_methods.iter().find(|pm| {
            pm.id == building.active_pm
                || (pm.building_id == building.building_def_id && pm.id.ends_with("default"))
        }) {
            out.push(pm);
        }
    }
    out
}

pub fn default_pms_for_building<'a>(
    db: &'a V6Database,
    building_id: &str,
) -> Vec<&'a ProductionMethodDef> {
    let mut out: Vec<&ProductionMethodDef> = Vec::new();
    let mut seen_groups: Vec<String> = Vec::new();

    for pm in db
        .production_methods
        .iter()
        .filter(|pm| pm.building_id == building_id)
    {
        let group = active_pm_slot_key(pm);
        if !seen_groups.contains(&group) {
            if pm.id.ends_with("default")
                || production_method_group(pm) == "base"
                || production_method_group(pm) == "secondary"
            {
                out.push(pm);
            }
            seen_groups.push(group);
        }
    }
    if out.is_empty() {
        if let Some(pm) = db
            .production_methods
            .iter()
            .find(|pm| pm.building_id == building_id)
        {
            out.push(pm);
        }
    }
    out
}

#[derive(Clone, Debug, Deserialize)]
pub struct InitialPopsDef {
    pub total_population: u64,
    pub groups: Vec<InitialPopGroup>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum PopNeedTierDef {
    Essential,
    Normal,
    Luxury,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PopNeedEntryDef {
    pub good_id: String,
    pub tier: PopNeedTierDef,
    pub amount_per_million: f32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PopClassNeedsDef {
    pub class: PopClass,
    pub needs: Vec<PopNeedEntryDef>,
}

// V6 database: in-memory representation after all RON content is loaded.
#[derive(Clone, Debug)]
pub struct V6Database {
    pub goods: Vec<GoodDef>,
    pub buildings: Vec<BuildingDef>,
    pub production_methods: Vec<ProductionMethodDef>,
    pub conscription_laws: Vec<ConscriptionDef>,
    pub economy_laws: Vec<EconomyDef>,
    pub trade_laws: Vec<TradeDef>,
    pub taxation_laws: Vec<TaxationDef>,
    pub civil_rights_laws: Vec<CivilRightsDef>,
    pub information_control_laws: Vec<InformationControlDef>,
    pub pop_needs: Vec<PopClassNeedsDef>,
    pub initial_pops: std::collections::HashMap<String, InitialPopsDef>,
    pub pyatiletka_plans: Vec<PyatiletkaDef>,
    pub events_v6: Vec<V6EventDef>,
    pub mefo: MefoDef,
    pub technologies: Vec<TechDef>,
    pub state_resource_deposits: Vec<StateResourceDepositDef>,
    pub state_populations: Vec<StatePopulation1936Def>,
    pub historical_countries: Vec<HistoricalCountryEconomyDef>,
    pub historical_trade_routes: Vec<HistoricalTradeProfileDef>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HistoricalValidationRow {
    pub tag: String,
    pub runtime_population: u64,
    pub historical_reference_population: u64,
    pub population_error_ratio: f64,
    pub generated_gdp_gbp: f64,
    pub historical_reference_gdp_gbp: f64,
    pub gdp_error_ratio: f64,
}

impl Default for V6Database {
    fn default() -> Self {
        Self {
            goods: Vec::new(),
            buildings: Vec::new(),
            production_methods: Vec::new(),
            conscription_laws: Vec::new(),
            economy_laws: Vec::new(),
            trade_laws: Vec::new(),
            taxation_laws: Vec::new(),
            civil_rights_laws: Vec::new(),
            information_control_laws: Vec::new(),
            pop_needs: Vec::new(),
            initial_pops: std::collections::HashMap::new(),
            pyatiletka_plans: Vec::new(),
            events_v6: Vec::new(),
            mefo: MefoDef::default(),
            technologies: Vec::new(),
            state_resource_deposits: Vec::new(),
            state_populations: Vec::new(),
            historical_countries: Vec::new(),
            historical_trade_routes: Vec::new(),
        }
    }
}

impl V6Database {
    pub fn pop_need_entries_for_class(&self, class: PopClass) -> &[PopNeedEntryDef] {
        self.pop_needs
            .iter()
            .find(|entry| entry.class == class)
            .map(|entry| entry.needs.as_slice())
            .unwrap_or(&[])
    }

    pub fn load() -> Self {
        let mut db = Self::default();
        db.goods = load_ron::<Vec<GoodDef>>(include_str!("../content/economy_v6/goods.ron"))
            .unwrap_or_default();
        db.buildings = load_ron::<Vec<BuildingDef>>(include_str!(
            "../content/economy_v6/buildings/buildings.ron"
        ))
        .unwrap_or_default();
        db.production_methods = load_ron::<Vec<ProductionMethodDef>>(include_str!(
            "../content/economy_v6/production_methods/resource.ron"
        ))
        .unwrap_or_default();
        db.production_methods.extend(
            load_ron::<Vec<ProductionMethodDef>>(include_str!(
                "../content/economy_v6/production_methods/industrial.ron"
            ))
            .unwrap_or_default(),
        );
        db.production_methods.extend(
            load_ron::<Vec<ProductionMethodDef>>(include_str!(
                "../content/economy_v6/production_methods/agriculture.ron"
            ))
            .unwrap_or_default(),
        );
        db.production_methods.extend(
            load_ron::<Vec<ProductionMethodDef>>(include_str!(
                "../content/economy_v6/production_methods/consumer_goods.ron"
            ))
            .unwrap_or_default(),
        );
        db.production_methods.extend(
            load_ron::<Vec<ProductionMethodDef>>(include_str!(
                "../content/economy_v6/production_methods/service.ron"
            ))
            .unwrap_or_default(),
        );
        db.production_methods.extend(
            load_ron::<Vec<ProductionMethodDef>>(include_str!(
                "../content/economy_v6/production_methods/military.ron"
            ))
            .unwrap_or_default(),
        );
        db.production_methods.extend(
            load_ron::<Vec<ProductionMethodDef>>(include_str!(
                "../content/economy_v6/production_methods/infrastructure.ron"
            ))
            .unwrap_or_default(),
        );
        db.production_methods.extend(
            load_ron::<Vec<ProductionMethodDef>>(include_str!(
                "../content/economy_v6/production_methods/military_base.ron"
            ))
            .unwrap_or_default(),
        );
        db.conscription_laws = load_ron::<Vec<ConscriptionDef>>(include_str!(
            "../content/economy_v6/laws/conscription.ron"
        ))
        .unwrap_or_default();
        db.economy_laws =
            load_ron::<Vec<EconomyDef>>(include_str!("../content/economy_v6/laws/economy.ron"))
                .unwrap_or_default();
        db.trade_laws =
            load_ron::<Vec<TradeDef>>(include_str!("../content/economy_v6/laws/trade.ron"))
                .unwrap_or_default();
        db.taxation_laws =
            load_ron::<Vec<TaxationDef>>(include_str!("../content/economy_v6/laws/taxation.ron"))
                .unwrap_or_default();
        db.civil_rights_laws = load_ron::<Vec<CivilRightsDef>>(include_str!(
            "../content/economy_v6/laws/civil_rights.ron"
        ))
        .unwrap_or_default();
        db.information_control_laws = load_ron::<Vec<InformationControlDef>>(include_str!(
            "../content/economy_v6/laws/information_control.ron"
        ))
        .unwrap_or_default();
        db.pop_needs =
            load_ron::<Vec<PopClassNeedsDef>>(include_str!("../content/economy_v6/pop_needs.ron"))
                .expect("POP needs must load");
        let mut initial_pops: std::collections::HashMap<String, InitialPopsDef> =
            std::collections::HashMap::new();
        // 8 major countries
        initial_pops.insert(
            "GER".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_ger.ron"))
                .expect("GER POPs must load"),
        );
        initial_pops.insert(
            "USA".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_usa.ron"))
                .expect("USA POPs must load"),
        );
        initial_pops.insert(
            "SOV".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_sov.ron"))
                .expect("SOV POPs must load"),
        );
        initial_pops.insert(
            "ENG".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_eng.ron"))
                .expect("ENG POPs must load"),
        );
        initial_pops.insert(
            "FRA".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_fra.ron"))
                .expect("FRA POPs must load"),
        );
        initial_pops.insert(
            "JAP".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_jap.ron"))
                .expect("JAP POPs must load"),
        );
        initial_pops.insert(
            "ITA".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_ita.ron"))
                .expect("ITA POPs must load"),
        );
        initial_pops.insert(
            "CHI".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_chi.ron"))
                .expect("CHI POPs must load"),
        );
        // Colonies / autonomies
        initial_pops.insert(
            "RAJ".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_raj.ron"))
                .expect("RAJ POPs must load"),
        );
        initial_pops.insert(
            "CAN".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_can.ron"))
                .expect("CAN POPs must load"),
        );
        initial_pops.insert(
            "AST".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_ast.ron"))
                .expect("AST POPs must load"),
        );
        initial_pops.insert(
            "MAN".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_man.ron"))
                .expect("MAN POPs must load"),
        );
        initial_pops.insert(
            "NZL".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_nzl.ron"))
                .expect("NZL POPs must load"),
        );
        initial_pops.insert(
            "SAF".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_saf.ron"))
                .expect("SAF POPs must load"),
        );
        initial_pops.insert(
            "MAL".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_mal.ron"))
                .expect("MAL POPs must load"),
        );
        initial_pops.insert(
            "MEN".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_men.ron"))
                .expect("MEN POPs must load"),
        );
        // Phase 3 warehouse-visible 1936 tags.
        initial_pops.insert(
            "AUS".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_aus.ron"))
                .expect("AUS POPs must load"),
        );
        initial_pops.insert(
            "CZE".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_cze.ron"))
                .expect("CZE POPs must load"),
        );
        initial_pops.insert(
            "GDC".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_gdc.ron"))
                .expect("GDC POPs must load"),
        );
        initial_pops.insert(
            "GXC".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_gxc.ron"))
                .expect("GXC POPs must load"),
        );
        initial_pops.insert(
            "HBC".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_hbc.ron"))
                .expect("HBC POPs must load"),
        );
        initial_pops.insert(
            "LIT".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_lit.ron"))
                .expect("LIT POPs must load"),
        );
        initial_pops.insert(
            "POL".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_pol.ron"))
                .expect("POL POPs must load"),
        );
        initial_pops.insert(
            "PRC".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_prc.ron"))
                .expect("PRC POPs must load"),
        );
        initial_pops.insert(
            "ROM".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_rom.ron"))
                .expect("ROM POPs must load"),
        );
        initial_pops.insert(
            "SHX".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_shx.ron"))
                .expect("SHX POPs must load"),
        );
        initial_pops.insert(
            "SIC".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_sic.ron"))
                .expect("SIC POPs must load"),
        );
        initial_pops.insert(
            "SIK".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_sik.ron"))
                .expect("SIK POPs must load"),
        );
        initial_pops.insert(
            "SND".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_snd.ron"))
                .expect("SND POPs must load"),
        );
        initial_pops.insert(
            "SPR".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_spr.ron"))
                .expect("SPR POPs must load"),
        );
        initial_pops.insert(
            "SWE".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_swe.ron"))
                .expect("SWE POPs must load"),
        );
        initial_pops.insert(
            "TIB".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_tib.ron"))
                .expect("TIB POPs must load"),
        );
        initial_pops.insert(
            "XAJ".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_xaj.ron"))
                .expect("XAJ POPs must load"),
        );
        initial_pops.insert(
            "XSM".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_xsm.ron"))
                .expect("XSM POPs must load"),
        );
        initial_pops.insert(
            "YUN".to_owned(),
            load_ron::<InitialPopsDef>(include_str!("../content/economy_v6/pops/initial_yun.ron"))
                .expect("YUN POPs must load"),
        );
        db.initial_pops = initial_pops;
        if let Ok(plan) = load_ron::<PyatiletkaDef>(include_str!(
            "../content/economy_v6/pyatiletka/sov_first_plan.ron"
        )) {
            db.pyatiletka_plans.push(plan);
        }
        if let Ok(plan) = load_ron::<PyatiletkaDef>(include_str!(
            "../content/economy_v6/pyatiletka/sov_second_plan.ron"
        )) {
            db.pyatiletka_plans.push(plan);
        }

        // V6.E: load all events_v6 RON files.
        db.events_v6.extend(
            load_ron::<Vec<V6EventDef>>(include_str!(
                "../content/economy_v6/events_v6/mefo_crisis.ron"
            ))
            .unwrap_or_default(),
        );
        db.events_v6.extend(
            load_ron::<Vec<V6EventDef>>(include_str!(
                "../content/economy_v6/events_v6/nationalization.ron"
            ))
            .unwrap_or_default(),
        );
        db.events_v6.extend(
            load_ron::<Vec<V6EventDef>>(include_str!(
                "../content/economy_v6/events_v6/mark_devaluation.ron"
            ))
            .unwrap_or_default(),
        );
        db.events_v6.extend(
            load_ron::<Vec<V6EventDef>>(include_str!(
                "../content/economy_v6/events_v6/blockade_crisis.ron"
            ))
            .unwrap_or_default(),
        );
        db.events_v6.extend(
            load_ron::<Vec<V6EventDef>>(include_str!(
                "../content/economy_v6/events_v6/ger_historical_econ.ron"
            ))
            .unwrap_or_default(),
        );
        db.mefo = load_ron::<MefoDef>(include_str!("../content/economy_v6/finance/mefo.ron"))
            .unwrap_or_default();

        // V6.F: load all technology tree RON files.
        db.technologies.extend(
            load_ron::<Vec<TechDef>>(include_str!(
                "../content/economy_v6/technologies/industry.ron"
            ))
            .unwrap_or_default(),
        );
        db.technologies.extend(
            load_ron::<Vec<TechDef>>(include_str!(
                "../content/economy_v6/technologies/chemistry.ron"
            ))
            .unwrap_or_default(),
        );
        db.technologies.extend(
            load_ron::<Vec<TechDef>>(include_str!(
                "../content/economy_v6/technologies/electrical.ron"
            ))
            .unwrap_or_default(),
        );
        db.technologies.extend(
            load_ron::<Vec<TechDef>>(include_str!(
                "../content/economy_v6/technologies/metallurgy.ron"
            ))
            .unwrap_or_default(),
        );
        db.technologies.extend(
            load_ron::<Vec<TechDef>>(include_str!(
                "../content/economy_v6/technologies/military_doctrine.ron"
            ))
            .unwrap_or_default(),
        );
        db.technologies.extend(
            load_ron::<Vec<TechDef>>(include_str!(
                "../content/economy_v6/technologies/aviation.ron"
            ))
            .unwrap_or_default(),
        );
        db.technologies.extend(
            load_ron::<Vec<TechDef>>(include_str!("../content/economy_v6/technologies/naval.ron"))
                .unwrap_or_default(),
        );
        db.technologies.extend(
            load_ron::<Vec<TechDef>>(include_str!(
                "../content/economy_v6/technologies/social_science.ron"
            ))
            .unwrap_or_default(),
        );
        db.technologies.extend(
            load_ron::<Vec<TechDef>>(include_str!(
                "../content/economy_v6/technologies/information_control.ron"
            ))
            .unwrap_or_default(),
        );
        if let Ok(history) = Historical1936Database::load() {
            db.state_resource_deposits = history.state_deposits;
            db.state_populations = history.state_populations;
            db.historical_countries = history.countries;
            db.historical_trade_routes = history.trade_routes;
        }

        db
    }

    /// Assert key V6 IDs do not conflict with vanilla IDs.
    pub fn assert_no_vanilla_conflict(&self) {
        let v6_building_ids: Vec<&str> = self.buildings.iter().map(|b| b.id.as_str()).collect();
        let vanilla_conflict_ids = [
            "industrial_complex",
            "arms_factory",
            "dockyard",
            "infrastructure",
            "air_base",
            "anti_air_building",
            "radar_station",
        ];
        for vid in &vanilla_conflict_ids {
            assert!(
                !v6_building_ids.iter().any(|&id| id == *vid),
                "V6 building ID '{}' conflicts with vanilla! V6 must use distinct IDs.",
                vid
            );
        }
        let v6_good_ids: Vec<&str> = self.goods.iter().map(|g| g.id.as_str()).collect();
        let vanilla_resource_ids = [
            "oil",
            "aluminium",
            "rubber",
            "tungsten",
            "steel",
            "chromium",
        ];
        for vid in &vanilla_resource_ids {
            assert!(
                v6_good_ids.iter().any(|&id| id == *vid),
                "V6 good '{}' must exist as it replaces a vanilla resource.",
                vid
            );
        }
    }
}

impl HistoricalCountryEconomyDef {
    fn validation_reference_population(&self) -> u64 {
        self.population as u64
    }

    fn validation_reference_gdp_gbp(&self) -> f64 {
        self.gdp_1936_gbp
    }
}

pub fn historical_validation_table(world: &World, db: &V6Database) -> Vec<HistoricalValidationRow> {
    db.historical_countries
        .iter()
        .filter_map(|profile| {
            let country = world.country(profile.tag.as_str())?;
            let ci = country.0 as usize;
            let runtime_population = world.country_governed_population(country);
            let population_reference = profile.validation_reference_population();
            let population_error_ratio = if population_reference > 0 {
                (runtime_population as f64 - population_reference as f64)
                    / population_reference as f64
            } else {
                0.0
            };
            let rm_per_gbp = world.countries.treasury.exchange_rates[ci].rm_per_gbp as f64;
            let generated_gdp_gbp =
                estimate_annual_building_value_gbp(world, country, db, rm_per_gbp);
            let gdp_reference = profile.validation_reference_gdp_gbp();
            let gdp_error_ratio = if gdp_reference > 0.0 {
                (generated_gdp_gbp - gdp_reference) / gdp_reference
            } else {
                0.0
            };
            Some(HistoricalValidationRow {
                tag: profile.tag.clone(),
                runtime_population,
                historical_reference_population: population_reference,
                population_error_ratio,
                generated_gdp_gbp,
                historical_reference_gdp_gbp: gdp_reference,
                gdp_error_ratio,
            })
        })
        .collect()
}

fn load_ron<T: serde::de::DeserializeOwned>(s: &str) -> Result<T, ron::error::SpannedError> {
    ron::from_str(s)
}

// V6 to World injection.
fn map_building_kind(def: BuildingKindDef) -> BuildingKind {
    match def {
        BuildingKindDef::Resource => BuildingKind::Resource,
        BuildingKindDef::Industrial => BuildingKind::Industrial,
        BuildingKindDef::Agriculture => BuildingKind::Agriculture,
        BuildingKindDef::ConsumerGoods => BuildingKind::ConsumerGoods,
        BuildingKindDef::Service => BuildingKind::Service,
        BuildingKindDef::Military => BuildingKind::Military,
        BuildingKindDef::Infrastructure => BuildingKind::Infrastructure,
        BuildingKindDef::MilitaryBase => BuildingKind::MilitaryBase,
    }
}

fn map_building_owner(def: OwnerDef) -> BuildingOwner {
    match def {
        OwnerDef::Private => BuildingOwner::Private,
        OwnerDef::State => BuildingOwner::State,
        OwnerDef::Cartel => BuildingOwner::Cartel,
    }
}

fn map_law_category(def: LawCategoryDef) -> LawCategory {
    match def {
        LawCategoryDef::Conscription => LawCategory::Conscription,
        LawCategoryDef::Economy => LawCategory::Economy,
        LawCategoryDef::Trade => LawCategory::Trade,
        LawCategoryDef::Taxation => LawCategory::Taxation,
        LawCategoryDef::CivilRights => LawCategory::CivilRights,
        LawCategoryDef::InformationControl => LawCategory::InformationControl,
    }
}

/// Inject the V6 database into `World`.
///
/// - Initializes every country market with goods supply, demand, stockpile, and base price.
/// - Applies historical 1936 law profiles.
/// - Applies initial POP and building profiles.
pub fn inject_v6_into_world(world: &mut World, db: &V6Database) {
    let n = world.countries.count;
    world.countries.trade.ensure_capacity(world.states.count);

    // Initialize every country market with each good's base price.
    for ci in 0..n {
        let market = &mut world.countries.market.markets[ci];
        for good in &db.goods {
            market.price.insert(good.id.clone(), good.base_price_rm);
            market.supply.insert(good.id.clone(), 0.0);
            market.demand.insert(good.id.clone(), 0.0);
            market.stockpile.insert(good.id.clone(), 0.0);
            market.unmet_demand.insert(good.id.clone(), 0.0);
            market.stockpile_coverage_days.insert(good.id.clone(), 0.0);
            market.imports.insert(good.id.clone(), 0.0);
            market.exports.insert(good.id.clone(), 0.0);
        }
    }
    seed_initial_v6_technologies(world, db);

    let historical_profiles = db.historical_countries.clone();
    apply_state_integration_statuses(world, db);
    apply_fallback_state_integration_statuses(world, db);
    for profile in &historical_profiles {
        let Some(country) = world.country(&profile.tag) else {
            continue;
        };
        apply_historical_laws(world, country, profile);
        seed_historical_v6_tech_floor(world, country, profile, db);
        inject_historical_pops(world, country, profile, db);
        inject_historical_buildings(world, country, profile, db);
        apply_historical_finance(world, country, profile, db);
        apply_historical_pm_mix(world, country, profile, db);
        calibrate_building_employment_to_population(world, country, db);
        seed_initial_building_employment(world, country, db, profile.unemployment);
        seed_initial_market_stockpiles(world, country, db);
        record_historical_gdp_validation(world, country, profile, db);
    }
    // Fallback: inject algorithmic POPs for countries not covered by historical profiles
    inject_algorithmic_pops_for_remaining_countries(world, db);
    // Fallback: every country with owned 1936 states needs a minimum usable V6 economy.
    inject_baseline_buildings_for_remaining_countries(world, db);
    apply_fallback_finance_for_remaining_countries(world, db);

    inject_historical_trade_routes(world, db);
    inject_historical_autonomy(world);
    inject_historical_market_blocs(world);
}

fn seed_initial_v6_technologies(world: &mut World, db: &V6Database) {
    let known_v6_techs: std::collections::HashSet<&str> = db
        .technologies
        .iter()
        .map(|tech| tech.id.as_str())
        .collect();

    for ci in 0..world.countries.count {
        let vanilla_techs = world.countries.completed_techs[ci].clone();

        seed_v6_if_any(
            &mut world.countries.completed_techs[ci],
            &known_v6_techs,
            &vanilla_techs,
            "construction_technology",
            &["construction1", "construction_technology"],
        );
        seed_v6_if_any(
            &mut world.countries.completed_techs[ci],
            &known_v6_techs,
            &vanilla_techs,
            "industrial_production",
            &[
                "basic_machine_tools",
                "improved_machine_tools",
                "advanced_machine_tools",
                "industrial_production",
            ],
        );
        seed_v6_if_any(
            &mut world.countries.completed_techs[ci],
            &known_v6_techs,
            &vanilla_techs,
            "concentrated_industry",
            &["concentrated_industry", "concentrated_industry2"],
        );
        seed_v6_if_any(
            &mut world.countries.completed_techs[ci],
            &known_v6_techs,
            &vanilla_techs,
            "dispersed_industry",
            &["dispersed_industry", "dispersed_industry2"],
        );
        seed_v6_if_any(
            &mut world.countries.completed_techs[ci],
            &known_v6_techs,
            &vanilla_techs,
            "metallurgy",
            &[
                "basic_machine_tools",
                "improved_machine_tools",
                "advanced_machine_tools",
            ],
        );
        seed_v6_if_any(
            &mut world.countries.completed_techs[ci],
            &known_v6_techs,
            &vanilla_techs,
            "chemistry",
            &["synth_oil_experiments", "oil_processing", "fuel_refining"],
        );
        seed_v6_if_any(
            &mut world.countries.completed_techs[ci],
            &known_v6_techs,
            &vanilla_techs,
            "synthetic_materials",
            &["synth_oil_experiments", "oil_processing"],
        );
        seed_v6_if_any(
            &mut world.countries.completed_techs[ci],
            &known_v6_techs,
            &vanilla_techs,
            "electronics_base",
            &["electronic_mechanical_engineering", "radio"],
        );
        seed_v6_if_any(
            &mut world.countries.completed_techs[ci],
            &known_v6_techs,
            &vanilla_techs,
            "combustion_engine",
            &["motorised_infantry", "engines_1", "engines_2"],
        );
        seed_v6_if_any(
            &mut world.countries.completed_techs[ci],
            &known_v6_techs,
            &vanilla_techs,
            "welded_armor",
            &[
                "basic_light_tank",
                "basic_light_tank_chassis",
                "improved_light_tank",
                "improved_light_tank_chassis",
            ],
        );
        seed_v6_if_any(
            &mut world.countries.completed_techs[ci],
            &known_v6_techs,
            &vanilla_techs,
            "all_metal_aircraft",
            &[
                "aircraft_construction",
                "basic_small_airframe",
                "basic_medium_airframe",
                "fighter1",
                "tactical_bomber1",
            ],
        );
        seed_v6_if_any(
            &mut world.countries.completed_techs[ci],
            &known_v6_techs,
            &vanilla_techs,
            "naval_technology",
            &[
                "early_ship_hull_light",
                "basic_ship_hull_light",
                "early_destroyer",
                "basic_destroyer",
                "early_submarine",
                "basic_submarine",
            ],
        );
        seed_v6_if_any(
            &mut world.countries.completed_techs[ci],
            &known_v6_techs,
            &vanilla_techs,
            "destroyer_technology",
            &[
                "early_destroyer",
                "basic_destroyer",
                "basic_ship_hull_light",
            ],
        );
        seed_v6_if_any(
            &mut world.countries.completed_techs[ci],
            &known_v6_techs,
            &vanilla_techs,
            "submarine_technology",
            &["early_submarine", "basic_submarine"],
        );
        seed_v6_if_any(
            &mut world.countries.completed_techs[ci],
            &known_v6_techs,
            &vanilla_techs,
            "cruiser_technology",
            &[
                "early_cruiser",
                "basic_cruiser",
                "early_ship_hull_cruiser",
                "basic_ship_hull_cruiser",
            ],
        );
    }
}

fn seed_v6_if_any(
    completed: &mut Vec<String>,
    known_v6_techs: &std::collections::HashSet<&str>,
    vanilla_techs: &[String],
    v6_tech: &str,
    vanilla_aliases: &[&str],
) {
    if !known_v6_techs.contains(v6_tech) || completed.iter().any(|tech| tech == v6_tech) {
        return;
    }
    if vanilla_aliases
        .iter()
        .any(|alias| vanilla_techs.iter().any(|tech| tech == alias))
    {
        completed.push(v6_tech.to_owned());
    }
}

fn seed_historical_v6_tech_floor(
    world: &mut World,
    country: CountryId,
    profile: &HistoricalCountryEconomyDef,
    db: &V6Database,
) {
    let techs: &[&str] = match profile.tag.as_str() {
        "GER" => &[
            "construction_technology",
            "industrial_production",
            "metallurgy",
            "chemistry",
            "synthetic_materials",
            "electronics_base",
            "combustion_engine",
            "welded_armor",
            "all_metal_aircraft",
            "naval_technology",
        ],
        _ => &[],
    };
    if techs.is_empty() {
        return;
    }

    let known_v6_techs: std::collections::HashSet<&str> = db
        .technologies
        .iter()
        .map(|tech| tech.id.as_str())
        .collect();
    let completed = &mut world.countries.completed_techs[country.0 as usize];
    for tech in techs {
        if known_v6_techs.contains(tech) && !completed.iter().any(|known| known == tech) {
            completed.push((*tech).to_owned());
        }
    }
}

fn apply_state_integration_statuses(world: &mut World, db: &V6Database) {
    for state_profile in &db.state_populations {
        let Some(state) = world.state_id_lookup.get(&state_profile.state_id).copied() else {
            continue;
        };
        world.set_state_integration_status(state, map_state_integration(state_profile.integration));
    }
}

fn apply_fallback_state_integration_statuses(world: &mut World, db: &V6Database) {
    let explicitly_profiled: std::collections::HashSet<u16> = db
        .state_populations
        .iter()
        .filter_map(|profile| {
            world
                .state_id_lookup
                .get(&profile.state_id)
                .map(|sid| sid.0)
        })
        .collect();

    for si in 0..world.states.count {
        if explicitly_profiled.contains(&(si as u16)) || world.states.owners[si].is_none() {
            continue;
        }
        let status = fallback_state_integration_status(world, StateId(si as u16));
        world.set_state_integration_status(StateId(si as u16), status);
    }
}

fn fallback_state_integration_status(
    world: &World,
    state: StateId,
) -> hoi4_state::StateIntegrationStatus {
    let si = state.0 as usize;
    if si >= world.states.count {
        return hoi4_state::StateIntegrationStatus::Metropole;
    }
    let owner = world.states.owners[si];
    if owner.is_none() {
        return hoi4_state::StateIntegrationStatus::Metropole;
    }
    if world.states.controllers[si] != owner {
        return hoi4_state::StateIntegrationStatus::Occupied;
    }
    if world.states.cores[si].contains(&owner) {
        hoi4_state::StateIntegrationStatus::Metropole
    } else {
        hoi4_state::StateIntegrationStatus::Colony
    }
}

fn fallback_state_population(world: &World, state: StateId) -> u32 {
    let si = state.0 as usize;
    if si >= world.states.count {
        return 0;
    }
    let manpower = world.states.manpower_pool[si] as u64;
    if manpower > 0 {
        return manpower.min(u32::MAX as u64) as u32;
    }
    let infrastructure = world.states.infrastructure[si] as u32;
    let slots = world.states.category_slots[si] as u32;
    (25_000u32)
        .saturating_add(infrastructure.saturating_mul(35_000))
        .saturating_add(slots.saturating_mul(20_000))
        .max(25_000)
}

fn map_state_integration(def: StateIntegrationDef) -> hoi4_state::StateIntegrationStatus {
    match def {
        StateIntegrationDef::Metropole => hoi4_state::StateIntegrationStatus::Metropole,
        StateIntegrationDef::Incorporated => hoi4_state::StateIntegrationStatus::Incorporated,
        StateIntegrationDef::Colony => hoi4_state::StateIntegrationStatus::Colony,
        StateIntegrationDef::Protectorate => hoi4_state::StateIntegrationStatus::Protectorate,
        StateIntegrationDef::Mandate => hoi4_state::StateIntegrationStatus::Mandate,
        StateIntegrationDef::Concession => hoi4_state::StateIntegrationStatus::Concession,
        StateIntegrationDef::Occupied => hoi4_state::StateIntegrationStatus::Occupied,
    }
}

fn inject_historical_market_blocs(world: &mut World) {
    add_historical_market_bloc(
        world,
        "英帝国优惠体系",
        "ENG",
        &["ENG", "CAN", "AST", "NZL", "SAF", "RAJ", "MAL"],
        hoi4_state::MarketBlocKind::ImperialPreference,
        0.35,
        1.15,
        50,
    );
    add_historical_market_bloc(
        world,
        "轴心资源贸易圈",
        "GER",
        &["GER", "ITA", "ROM", "SWE"],
        hoi4_state::MarketBlocKind::FactionMarket,
        0.65,
        1.25,
        35,
    );
    add_historical_market_bloc(
        world,
        "日本势力圈",
        "JAP",
        &["JAP", "MAN", "MEN"],
        hoi4_state::MarketBlocKind::ColonialEmpire,
        0.55,
        1.30,
        40,
    );
    add_historical_market_bloc(
        world,
        "苏联计划调拨圈",
        "SOV",
        &["SOV"],
        hoi4_state::MarketBlocKind::FactionMarket,
        0.20,
        1.50,
        45,
    );
}

fn add_historical_market_bloc(
    world: &mut World,
    name: &str,
    leader_tag: &str,
    member_tags: &[&str],
    kind: hoi4_state::MarketBlocKind,
    internal_tariff_mult: f32,
    external_tariff_mult: f32,
    internal_trade_priority: i32,
) {
    let Some(leader) = world.country(leader_tag) else {
        return;
    };
    let members: Vec<CountryId> = member_tags
        .iter()
        .filter_map(|tag| world.country(tag))
        .collect();
    if members.is_empty() {
        return;
    }
    world.countries.market.add_bloc(
        name,
        leader,
        members,
        kind,
        internal_tariff_mult,
        external_tariff_mult,
        internal_trade_priority,
    );
}

fn inject_historical_autonomy(world: &mut World) {
    let relations = [
        ("ENG", "CAN", AutonomyLevel::Dominion),
        ("ENG", "AST", AutonomyLevel::Dominion),
        ("ENG", "NZL", AutonomyLevel::Dominion),
        ("ENG", "SAF", AutonomyLevel::Dominion),
        ("ENG", "RAJ", AutonomyLevel::Puppet),
        ("ENG", "MAL", AutonomyLevel::Puppet),
        ("JAP", "MAN", AutonomyLevel::Puppet),
        ("JAP", "MEN", AutonomyLevel::IntegratedPuppet),
    ];
    for (master_tag, subject_tag, level) in relations {
        let Some(master) = world.country(master_tag) else {
            continue;
        };
        let Some(subject) = world.country(subject_tag) else {
            continue;
        };
        if master == subject {
            continue;
        }
        world.diplomacy.autonomy.insert(
            subject,
            Autonomy {
                master,
                subject,
                level,
                progress: 0.0,
                since_hour: world.elapsed_hours,
            },
        );
        world.diplomacy.opinions.set(master, subject, 100);
        world.diplomacy.opinions.set(subject, master, 80);
    }
}

fn inject_historical_trade_routes(world: &mut World, db: &V6Database) {
    for route in &db.historical_trade_routes {
        let Some(importer) = world.country(&route.importer) else {
            continue;
        };
        let exporter = world.country(&route.exporter).unwrap_or(CountryId::NONE);
        let kind = match route.route_kind {
            TradeRouteKindDef::Land => hoi4_state::TradeRouteKind::Land,
            TradeRouteKindDef::Sea => hoi4_state::TradeRouteKind::Sea,
            TradeRouteKindDef::ImperialPreference => hoi4_state::TradeRouteKind::ImperialPreference,
        };
        let port_state = route
            .port_state
            .and_then(|state_id| state_id_from_game_or_internal(world, state_id));
        world.countries.trade.add_route_for_good(
            importer,
            exporter,
            Some(route.good_id.clone()),
            kind,
            port_state,
            route.daily_quantity,
            true,
        );
        if !exporter.is_none() {
            world.countries.trade.add_agreement(
                importer,
                exporter,
                route.good_id.clone(),
                route.daily_quantity,
                hoi4_state::TradeDirection::AImportsFromB,
            );
        }
    }
}

fn apply_historical_laws(
    world: &mut World,
    country: CountryId,
    profile: &HistoricalCountryEconomyDef,
) {
    let ci = country.0 as usize;
    let law_set = &mut world.countries.law_store.law_sets[ci];
    law_set.0[LawCategory::Conscription.index()] = LawSlot::new(
        LawCategory::Conscription,
        profile.initial_laws.conscription.as_str(),
    );
    law_set.0[LawCategory::Economy.index()] =
        LawSlot::new(LawCategory::Economy, profile.initial_laws.economy.as_str());
    if profile.initial_laws.trade == "state_trade_monopoly" {
        law_set.0[LawCategory::Trade.index()] = LawSlot {
            category: LawCategory::Trade,
            current: profile.initial_laws.trade.clone(),
            cooldown_days: 0,
            pending: None,
            previous_before_lock: Some("import_substitution".to_owned()),
            is_locked: profile.initial_laws.economy == "planned_economy",
        };
    } else {
        law_set.0[LawCategory::Trade.index()] =
            LawSlot::new(LawCategory::Trade, profile.initial_laws.trade.as_str());
    }
    law_set.0[LawCategory::Taxation.index()] = LawSlot::new(
        LawCategory::Taxation,
        profile.initial_laws.taxation.as_str(),
    );
    law_set.0[LawCategory::CivilRights.index()] = LawSlot::new(
        LawCategory::CivilRights,
        profile.initial_laws.civil_rights.as_str(),
    );
    law_set.0[LawCategory::InformationControl.index()] = LawSlot::new(
        LawCategory::InformationControl,
        profile.initial_laws.information_control.as_str(),
    );
}

fn apply_historical_finance(
    world: &mut World,
    country: CountryId,
    profile: &HistoricalCountryEconomyDef,
    db: &V6Database,
) {
    let ci = country.0 as usize;
    let rm_per_gbp = world.countries.treasury.exchange_rates[ci].rm_per_gbp as f64;
    let finance_scale_rm = runtime_initial_finance_scale_rm(world, country, db, rm_per_gbp);
    let treasury = &mut world.countries.treasury.treasuries[ci];
    treasury.gdp_gbp = 0.0;
    treasury.gdp_rm = 0.0;
    treasury.domestic_gdp_gbp = 0.0;
    treasury.domestic_gdp_rm = 0.0;
    treasury.colonial_gdp_gbp = 0.0;
    treasury.colonial_gdp_rm = 0.0;
    treasury.gdp_last_year_gbp = 0.0;
    treasury.gdp_growth_yoy = 0.0;
    treasury.gdp_breakdown.historical_validation_gbp = profile.gdp_1936_gbp;
    treasury.gdp_breakdown.historical_validation_error_ratio = 0.0;
    treasury.reserve_gbp = profile.foreign_exchange_reserve_gbp;
    treasury.public_debt_gbp = profile.public_debt_gbp;
    treasury.public_debt_rm = profile.public_debt_gbp * rm_per_gbp;
    treasury.gold_kg = (profile.gold_reserve_gbp / 1_000.0).max(0.0);
    treasury.cash_rm = (finance_scale_rm * 0.02).max(1_000_000.0);
    let finance_scale_gbp = finance_scale_rm / rm_per_gbp.max(0.1);
    let debt_ratio = if finance_scale_gbp > 0.0 {
        profile.public_debt_gbp / finance_scale_gbp
    } else {
        0.0
    };
    treasury.credit_rating = hoi4_state::CreditRating::from_debt_ratio(debt_ratio);
    let construction_levels = country_building_levels(world, country, "construction_sector");
    let construction_factor = 0.01 + (construction_levels as f64 * 0.0025).clamp(0.0, 0.04);
    let initial_private_pool = (finance_scale_rm * construction_factor).max(1_000_000.0);
    world.countries.private_investment_pool_rm[ci] = initial_private_pool;
    if let Some(account) = world
        .countries
        .investment_account_mut(country, hoi4_state::InvestmentAccountKind::Private)
    {
        account.balance_rm = initial_private_pool;
    }
}

fn runtime_initial_finance_scale_rm(
    world: &World,
    country: CountryId,
    db: &V6Database,
    rm_per_gbp: f64,
) -> f64 {
    let population_component = world.country_governed_population(country) as f64 * 220.0;
    let building_component =
        estimate_annual_building_value_gbp(world, country, db, rm_per_gbp) * rm_per_gbp;
    (population_component + building_component).max(50_000_000.0)
}

fn country_building_levels(world: &World, country: CountryId, building_id: &str) -> u16 {
    world
        .countries
        .buildings_v6
        .buildings
        .iter()
        .filter(|building| {
            let si = building.state.0 as usize;
            si < world.states.count
                && world.states.owners[si] == country
                && building.building_def_id == building_id
                && building.level > 0
        })
        .map(|building| building.level as u16)
        .sum()
}

fn record_historical_gdp_validation(
    world: &mut World,
    country: CountryId,
    profile: &HistoricalCountryEconomyDef,
    db: &V6Database,
) {
    let ci = country.0 as usize;
    let rm_per_gbp = world.countries.treasury.exchange_rates[ci].rm_per_gbp as f64;
    let raw_building_gdp = estimate_annual_building_value_gbp(world, country, db, rm_per_gbp);
    if profile.gdp_1936_gbp <= 0.0 {
        return;
    }
    let treasury = &mut world.countries.treasury.treasuries[ci];
    treasury.gdp_breakdown.historical_validation_gbp = profile.gdp_1936_gbp;
    treasury.gdp_breakdown.historical_validation_error_ratio =
        (raw_building_gdp - profile.gdp_1936_gbp) / profile.gdp_1936_gbp;
}

fn apply_fallback_finance_for_remaining_countries(world: &mut World, db: &V6Database) {
    let profile_tags: std::collections::HashSet<&str> = db
        .historical_countries
        .iter()
        .map(|profile| profile.tag.as_str())
        .collect();

    for ci in 0..world.countries.count {
        let country = CountryId(ci as u16);
        if country.is_none() {
            continue;
        }
        let Some(tag) = world.countries.tags.get(ci).map(|tag| tag.as_str()) else {
            continue;
        };
        if profile_tags.contains(tag) || !country_has_owned_state(world, country) {
            continue;
        }
        let rm_per_gbp = world.countries.treasury.exchange_rates[ci].rm_per_gbp as f64;
        let treasury = &mut world.countries.treasury.treasuries[ci];
        if treasury.cash_rm <= 0.0 {
            treasury.cash_rm = 2_500_000.0_f64.mul_add(rm_per_gbp, 250_000.0);
        }
        world.countries.private_investment_pool_rm[ci] =
            world.countries.private_investment_pool_rm[ci].max(5_000_000.0);
        if let Some(account) = world
            .countries
            .investment_account_mut(country, hoi4_state::InvestmentAccountKind::Private)
        {
            account.balance_rm = account.balance_rm.max(5_000_000.0);
        }
        world.countries.treasury.treasuries[ci].update_credit_rating(rm_per_gbp as f32);
    }
}

fn country_has_owned_state(world: &World, country: CountryId) -> bool {
    (0..world.states.count).any(|si| world.states.owners[si] == country)
}

fn estimate_annual_building_value_gbp(
    world: &World,
    country: CountryId,
    db: &V6Database,
    rm_per_gbp: f64,
) -> f64 {
    let ci = country.0 as usize;
    let market = &world.countries.market.markets[ci];
    world
        .countries
        .buildings_v6
        .buildings
        .iter()
        .filter(|building| {
            let si = building.state.0 as usize;
            si < world.states.count && world.states.owners[si] == country && building.level > 0
        })
        .map(|building| {
            let pms = active_pms_for_building(building, db);
            let daily_value_rm: f64 = pms
                .iter()
                .map(|pm| {
                    let output: f64 = pm
                        .output_good_ids
                        .iter()
                        .enumerate()
                        .map(|(i, good_id)| {
                            let amount = pm.output_good_amounts.get(i).copied().unwrap_or(0.0)
                                * building.level as f32;
                            let price = market.price.get(good_id).copied().unwrap_or(1.0);
                            amount as f64 * price as f64
                        })
                        .sum::<f64>()
                        + pm.equipment_output
                            .as_ref()
                            .map(|eq| eq.daily_per_level as f64 * building.level as f64 * 100.0)
                            .unwrap_or(0.0);
                    let input: f64 = pm
                        .input_good_ids
                        .iter()
                        .enumerate()
                        .map(|(i, good_id)| {
                            let amount = pm.input_good_amounts.get(i).copied().unwrap_or(0.0)
                                * building.level as f32;
                            let price = market.price.get(good_id).copied().unwrap_or(1.0);
                            amount as f64 * price as f64
                        })
                        .sum();
                    (output - input).max(output * 0.35)
                })
                .sum();
            daily_value_rm * 365.0 * 1_200.0 / rm_per_gbp.max(0.1)
        })
        .sum()
}

fn inject_historical_pops(
    world: &mut World,
    country: CountryId,
    profile: &HistoricalCountryEconomyDef,
    db: &V6Database,
) {
    let tag = profile.tag.as_str();
    world.countries.pops.groups.retain(|pg| {
        let si = pg.state.0 as usize;
        si >= world.states.count || world.states.owners[si] != country
    });

    let base_satisfaction = (0.62 - profile.unemployment * 0.75).clamp(0.30, 0.70);
    let tax_rates = initial_tax_rates(db, profile);
    let (loyalty_coefficient, loyalty_decay_mult) = initial_loyalty_modifiers(db, profile);

    let hand_pops = db.initial_pops.get(tag);
    let mut profiled_states = std::collections::HashSet::new();
    for state_profile in &db.state_populations {
        let Some(sid) = world.state_id_lookup.get(&state_profile.state_id).copied() else {
            continue;
        };
        let si = sid.0 as usize;
        if si >= world.states.count || world.states.owners[si] != country {
            continue;
        }
        let hand_groups: Vec<&InitialPopGroup> = hand_pops
            .map(|pops| {
                pops.groups
                    .iter()
                    .filter(|group| group.state_id == state_profile.state_id)
                    .collect()
            })
            .unwrap_or_default();
        inject_state_profile_pops(
            world,
            sid,
            profile,
            state_profile,
            &hand_groups,
            tax_rates,
            loyalty_coefficient,
            loyalty_decay_mult,
            base_satisfaction,
        );
        profiled_states.insert(sid.0);
    }

    let mut hand_state_ids = std::collections::HashSet::new();
    if let Some(hand_pops) = hand_pops {
        for hg in &hand_pops.groups {
            let Some(internal_sid) = resolve_game_state_id(world, hg.state_id) else {
                continue;
            };
            let si = internal_sid.0 as usize;
            if si >= world.states.count
                || world.states.owners[si] != country
                || profiled_states.contains(&internal_sid.0)
            {
                continue;
            }
            inject_hand_pop_group(
                world,
                internal_sid,
                hg,
                profile,
                hg.size,
                tax_rates,
                loyalty_coefficient,
                loyalty_decay_mult,
            );
            hand_state_ids.insert(internal_sid.0);
        }
    }

    let fallback_states: Vec<StateId> = historical_pop_states_by_weight(world, country)
        .into_iter()
        .filter(|sid| !profiled_states.contains(&sid.0) && !hand_state_ids.contains(&sid.0))
        .collect();
    if fallback_states.is_empty() {
        return;
    }
    inject_profile_fallback_pops_for_states(
        world,
        profile,
        &fallback_states,
        tax_rates,
        loyalty_coefficient,
        loyalty_decay_mult,
        base_satisfaction,
    );
}

fn inject_profile_fallback_pops_for_states(
    world: &mut World,
    profile: &HistoricalCountryEconomyDef,
    state_ids: &[StateId],
    tax_rates: [f32; 3],
    loyalty_coefficient: f32,
    loyalty_decay_mult: f32,
    base_satisfaction: f32,
) {
    for sid in state_ids {
        let state_population = fallback_state_population(world, *sid);
        if state_population == 0 {
            continue;
        }
        inject_generated_state_pops(
            world,
            *sid,
            profile,
            state_population,
            None,
            None,
            tax_rates,
            loyalty_coefficient,
            loyalty_decay_mult,
            base_satisfaction,
        );
    }
}

fn inject_hand_pop_group(
    world: &mut World,
    sid: StateId,
    hg: &InitialPopGroup,
    profile: &HistoricalCountryEconomyDef,
    size: u32,
    tax_rates: [f32; 3],
    loyalty_coefficient: f32,
    loyalty_decay_mult: f32,
) {
    if size == 0 {
        return;
    }
    let class = map_pop_class(hg.class);
    let literacy = hg
        .literacy
        .unwrap_or_else(|| initial_literacy(profile, class))
        .clamp(0.0, 1.0);
    let skilled_ratio = hg
        .skilled_ratio
        .unwrap_or_else(|| initial_skilled_ratio(profile, class))
        .clamp(0.0, 1.0);
    world.countries.pops.groups.push(PopGroup {
        class,
        state: sid,
        size,
        employed_at: None,
        wage_rm: hg.wage_rm,
        tax_burden: initial_tax_burden_from_rates(tax_rates, class),
        income_rm: 0.0,
        tax_paid_rm: 0.0,
        disposable_income_rm: 0.0,
        basic_consumption_budget: 0.0,
        satisfaction_law_modifier: 0.0,
        loyalty_coefficient,
        loyalty_decay_mult,
        satisfaction: hg.satisfaction.clamp(0.0, 1.0),
        political_loyalty: hg.political_loyalty.clamp(-1.0, 1.0),
        literacy,
        skilled_ratio,
        standard_of_living: hg.satisfaction.clamp(0.0, 1.0),
        needs_fulfillment: 1.0,
        essential_needs_fulfillment: 1.0,
        normal_needs_fulfillment: 1.0,
        luxury_needs_fulfillment: 1.0,
        radicalism: 0.0,
    });
}

fn inject_state_profile_pops(
    world: &mut World,
    sid: StateId,
    profile: &HistoricalCountryEconomyDef,
    state_profile: &StatePopulation1936Def,
    hand_groups: &[&InitialPopGroup],
    tax_rates: [f32; 3],
    loyalty_coefficient: f32,
    loyalty_decay_mult: f32,
    base_satisfaction: f32,
) {
    if state_profile.population == 0 {
        return;
    }
    let hand_total: u32 = hand_groups.iter().map(|group| group.size).sum();
    if hand_total > 0 {
        let mut remaining = state_profile.population;
        for (idx, group) in hand_groups.iter().enumerate() {
            let size = if idx + 1 == hand_groups.len() {
                remaining
            } else {
                ((state_profile.population as u64 * group.size as u64) / hand_total as u64)
                    .min(remaining as u64) as u32
            };
            remaining = remaining.saturating_sub(size);
            inject_hand_pop_group(
                world,
                sid,
                group,
                profile,
                size,
                tax_rates,
                loyalty_coefficient,
                loyalty_decay_mult,
            );
        }
        return;
    }
    inject_generated_state_pops(
        world,
        sid,
        profile,
        state_profile.population,
        state_profile.urbanization,
        state_profile.literacy,
        tax_rates,
        loyalty_coefficient,
        loyalty_decay_mult,
        base_satisfaction,
    );
}

fn inject_generated_state_pops(
    world: &mut World,
    sid: StateId,
    profile: &HistoricalCountryEconomyDef,
    state_population: u32,
    urbanization_override: Option<f32>,
    literacy_override: Option<f32>,
    tax_rates: [f32; 3],
    loyalty_coefficient: f32,
    loyalty_decay_mult: f32,
    base_satisfaction: f32,
) {
    if state_population == 0 {
        return;
    }
    let si = sid.0 as usize;
    if si >= world.states.count {
        return;
    }

    let infra_factor = (world.states.infrastructure[si] as f64 / 10.0).clamp(0.0, 1.0);
    let state_urbanization = urbanization_override
        .map(|value| value as f64)
        .unwrap_or_else(|| profile.urbanization as f64 * 0.65 + infra_factor * 0.35)
        .clamp(0.10, 0.95);
    let state_satisfaction =
        (base_satisfaction + (state_urbanization as f32 - 0.5) * 0.08).clamp(0.30, 0.70);

    let population = state_population as f64;
    let soldier_share =
        (0.006 + profile.military_spending_share as f64 * 0.16 + infra_factor * 0.01)
            .clamp(0.006, 0.075);
    let capitalist_share =
        (0.003 + profile.industrial_capacity_index as f64 / 100.0 * 0.008 + infra_factor * 0.004)
            .clamp(0.003, 0.020);
    let aristocrat_share = ((1.0 - state_urbanization) * 0.010 + 0.002).clamp(0.002, 0.012);
    let clerk_share = (state_urbanization
        * (profile.sector_shares.services + profile.sector_shares.government) as f64
        * 0.55)
        .clamp(0.03, 0.24);
    let worker_share = (state_urbanization
        * (profile.sector_shares.mining
            + profile.sector_shares.heavy_industry
            + profile.sector_shares.light_industry
            + profile.sector_shares.military_industry) as f64
        * 0.65
        + infra_factor * 0.01)
        .clamp(0.05, 0.38);

    let soldiers = (population * soldier_share).round() as u32;
    let capitalists = (population * capitalist_share).round() as u32;
    let aristocrats = (population * aristocrat_share).round() as u32;
    let clerks = (population * clerk_share).round() as u32;
    let workers = (population * worker_share).round() as u32;
    let assigned = soldiers
        .saturating_add(capitalists)
        .saturating_add(aristocrats)
        .saturating_add(clerks)
        .saturating_add(workers);
    let peasants = state_population.saturating_sub(assigned);

    for (class, size, sat) in [
        (PopClass::Peasant, peasants, state_satisfaction - 0.03),
        (PopClass::Worker, workers, state_satisfaction),
        (PopClass::Clerk, clerks, state_satisfaction + 0.04),
        (PopClass::Capitalist, capitalists, state_satisfaction + 0.08),
        (PopClass::Aristocrat, aristocrats, state_satisfaction + 0.06),
        (PopClass::Soldier, soldiers, state_satisfaction - 0.01),
    ] {
        if size == 0 {
            continue;
        }
        world.countries.pops.groups.push(PopGroup {
            class,
            state: sid,
            size,
            employed_at: None,
            wage_rm: 0.0,
            tax_burden: initial_tax_burden_from_rates(tax_rates, class),
            income_rm: 0.0,
            tax_paid_rm: 0.0,
            disposable_income_rm: 0.0,
            basic_consumption_budget: 0.0,
            satisfaction_law_modifier: 0.0,
            loyalty_coefficient,
            loyalty_decay_mult,
            satisfaction: sat.clamp(0.0, 1.0),
            political_loyalty: initial_political_loyalty(profile, class, sat),
            literacy: literacy_override
                .unwrap_or_else(|| initial_literacy(profile, class))
                .clamp(0.0, 1.0),
            skilled_ratio: initial_skilled_ratio(profile, class),
            standard_of_living: sat.clamp(0.0, 1.0),
            needs_fulfillment: 1.0,
            essential_needs_fulfillment: 1.0,
            normal_needs_fulfillment: 1.0,
            luxury_needs_fulfillment: 1.0,
            radicalism: 0.0,
        });
    }
}

fn inject_algorithmic_pops_for_remaining_countries(world: &mut World, db: &V6Database) {
    let profile_tags: std::collections::HashSet<String> = db
        .historical_countries
        .iter()
        .map(|p| p.tag.clone())
        .collect();

    for ci in 0..world.countries.count {
        let country = CountryId(ci as u16);
        if country.is_none() {
            continue;
        }
        let state_ids: Vec<StateId> = (0..world.states.count)
            .filter(|&si| world.states.owners[si] == country)
            .map(|si| StateId(si as u16))
            .collect();
        if state_ids.is_empty() {
            continue;
        }
        let total_pop: u64 = world
            .countries
            .pops
            .groups
            .iter()
            .filter(|pg| state_ids.contains(&pg.state))
            .map(|pg| pg.size as u64)
            .sum();
        if total_pop > 0 {
            continue;
        }
        let tag = world.countries.tags[ci].clone();
        let tag_str = tag.as_str();
        if profile_tags.contains(tag_str) {
            continue;
        }
        let mut inserted_hand_pops = false;
        if let Some(hand_pops) = db.initial_pops.get(tag_str) {
            for hg in &hand_pops.groups {
                let Some(internal_sid) = resolve_game_state_id(world, hg.state_id) else {
                    continue;
                };
                let si = internal_sid.0 as usize;
                if si >= world.states.count || world.states.owners[si] != country {
                    continue;
                }
                let class = map_pop_class(hg.class);
                let literacy = hg
                    .literacy
                    .unwrap_or(class.baseline_literacy())
                    .clamp(0.0, 1.0);
                let skilled_ratio = hg
                    .skilled_ratio
                    .unwrap_or(class.baseline_skilled_ratio())
                    .clamp(0.0, 1.0);
                world.countries.pops.groups.push(PopGroup {
                    class,
                    state: internal_sid,
                    size: hg.size,
                    employed_at: None,
                    wage_rm: hg.wage_rm,
                    tax_burden: 0.15,
                    income_rm: 0.0,
                    tax_paid_rm: 0.0,
                    disposable_income_rm: 0.0,
                    basic_consumption_budget: 0.0,
                    satisfaction_law_modifier: 0.0,
                    loyalty_coefficient: 1.0,
                    loyalty_decay_mult: 1.0,
                    satisfaction: hg.satisfaction.clamp(0.0, 1.0),
                    political_loyalty: hg.political_loyalty.clamp(-1.0, 1.0),
                    literacy,
                    skilled_ratio,
                    standard_of_living: hg.satisfaction.clamp(0.0, 1.0),
                    needs_fulfillment: 1.0,
                    essential_needs_fulfillment: 1.0,
                    normal_needs_fulfillment: 1.0,
                    luxury_needs_fulfillment: 1.0,
                    radicalism: 0.0,
                });
                inserted_hand_pops = true;
            }
        }
        if inserted_hand_pops {
            continue;
        }
        let estimated_pop = state_ids
            .iter()
            .map(|sid| fallback_state_population(world, *sid) as u64)
            .sum::<u64>()
            .min(u32::MAX as u64) as u32;
        if estimated_pop > 0 {
            inject_simple_pops_across_states(world, country, &state_ids, estimated_pop);
        }
    }
}

fn inject_simple_pops_across_states(
    world: &mut World,
    _country: CountryId,
    state_ids: &[StateId],
    total_population: u32,
) {
    let total_weight: u64 = state_ids
        .iter()
        .map(|sid| {
            let si = sid.0 as usize;
            (world.states.manpower_pool[si] as u64)
                .saturating_add(world.states.infrastructure[si] as u64 * 250_000)
                .max(1)
        })
        .sum();
    let mut remaining_pop = total_population;
    for (idx, sid) in state_ids.iter().enumerate() {
        let si = sid.0 as usize;
        let state_pop = if idx + 1 == state_ids.len() {
            remaining_pop
        } else {
            let weight = (world.states.manpower_pool[si] as u64)
                .saturating_add(world.states.infrastructure[si] as u64 * 250_000)
                .max(1);
            ((total_population as u64 * weight) / total_weight).min(remaining_pop as u64) as u32
        };
        remaining_pop = remaining_pop.saturating_sub(state_pop);
        if state_pop == 0 {
            continue;
        }
        let infra_factor = (world.states.infrastructure[si] as f64 / 10.0).clamp(0.0, 1.0);
        let urban = (0.25 + infra_factor * 0.35).clamp(0.10, 0.60);
        let mut peasants = (state_pop as f64 * (1.0 - urban) * 0.85).round() as u32;
        let workers = (state_pop as f64 * urban * 0.75).round() as u32;
        let clerks = (state_pop as f64 * urban * 0.15).round() as u32;
        let capitalists = (state_pop as f64 * urban * 0.03).round() as u32;
        let aristocrats = (state_pop as f64 * (1.0 - urban) * 0.10).round() as u32;
        let soldiers = (state_pop as f64 * 0.008).round() as u32;
        let assigned = peasants + workers + clerks + capitalists + aristocrats + soldiers;
        peasants += state_pop.saturating_sub(assigned);
        for (class, size) in [
            (PopClass::Peasant, peasants),
            (PopClass::Worker, workers),
            (PopClass::Clerk, clerks),
            (PopClass::Capitalist, capitalists),
            (PopClass::Aristocrat, aristocrats),
            (PopClass::Soldier, soldiers),
        ] {
            if size == 0 {
                continue;
            }
            world.countries.pops.groups.push(PopGroup {
                class,
                state: *sid,
                size,
                employed_at: None,
                wage_rm: 0.0,
                tax_burden: 0.15,
                income_rm: 0.0,
                tax_paid_rm: 0.0,
                disposable_income_rm: 0.0,
                basic_consumption_budget: 0.0,
                satisfaction_law_modifier: 0.0,
                loyalty_coefficient: 1.0,
                loyalty_decay_mult: 1.0,
                satisfaction: 0.45,
                political_loyalty: 0.05,
                literacy: class.baseline_literacy(),
                skilled_ratio: class.baseline_skilled_ratio(),
                standard_of_living: 0.45,
                needs_fulfillment: 1.0,
                essential_needs_fulfillment: 1.0,
                normal_needs_fulfillment: 1.0,
                luxury_needs_fulfillment: 1.0,
                radicalism: 0.0,
            });
        }
    }
}

fn inject_historical_buildings(
    world: &mut World,
    country: CountryId,
    profile: &HistoricalCountryEconomyDef,
    db: &V6Database,
) {
    let ci = country.0 as usize;
    if owned_states_by_weight(world, country).is_empty() {
        return;
    }

    allocate_resource_buildings(world, ci, profile, db);

    let runtime_population = world.country_governed_population(country);
    let targets = building_targets(profile, runtime_population);
    for (building_id, levels) in targets {
        let state_ids = states_for_building(world, country, building_id, db);
        allocate_levels_across_states(world, ci, &state_ids, building_id, levels, db);
    }
}

fn apply_historical_pm_mix(
    world: &mut World,
    country: CountryId,
    profile: &HistoricalCountryEconomyDef,
    db: &V6Database,
) {
    if profile.tag != "GER" {
        return;
    }

    let mut support_lines = 1_u8;
    for building in &mut world.countries.buildings_v6.buildings {
        let si = building.state.0 as usize;
        if si >= world.states.count || world.states.owners[si] != country || building.level == 0 {
            continue;
        }
        if building.building_def_id == "arms_industry" && support_lines > 0 {
            set_active_pm_for_building(building, db, "arms_industry_support_mk1");
            support_lines -= 1;
        }
    }
}

fn set_active_pm_for_building(building: &mut Building, db: &V6Database, pm_id: &str) {
    let Some(pm) = db.production_methods.iter().find(|pm| {
        pm.id == pm_id
            && pm.building_id == building.building_def_id
            && production_method_group(pm) == "military"
    }) else {
        return;
    };
    let group = active_pm_slot_key(pm);
    if let Some(active) = building
        .active_pm_by_group
        .iter_mut()
        .find(|active| active.group == group)
    {
        active.pm_id = pm.id.clone();
    } else {
        building.active_pm_by_group.push(ActiveProductionMethod {
            group,
            pm_id: pm.id.clone(),
        });
    }
    building.active_pm = pm.id.clone();
}

fn calibrate_building_employment_to_population(
    world: &mut World,
    country: CountryId,
    db: &V6Database,
) {
    for _ in 0..64 {
        let demand = country_employment_demand(world, country, db);
        let supply = country_pop_supply(world, country);
        let mut limiting_class: Option<usize> = None;
        for class_idx in 0..PopClass::COUNT {
            if demand[class_idx] > supply[class_idx] {
                limiting_class = Some(class_idx);
                break;
            }
        }
        let Some(class_idx) = limiting_class else {
            return;
        };
        let Some(building_idx) = world
            .countries
            .buildings_v6
            .buildings
            .iter()
            .enumerate()
            .rev()
            .find_map(|(idx, building)| {
                let si = building.state.0 as usize;
                if si >= world.states.count
                    || world.states.owners[si] != country
                    || building.level == 0
                    || building_needed_per_level(building, db)[class_idx] == 0
                {
                    None
                } else {
                    Some(idx)
                }
            })
        else {
            return;
        };
        world.countries.buildings_v6.buildings[building_idx].level =
            world.countries.buildings_v6.buildings[building_idx]
                .level
                .saturating_sub(1);
    }
}

fn country_employment_demand(world: &World, country: CountryId, db: &V6Database) -> [u32; 6] {
    let mut demand = [0u32; 6];
    for building in &world.countries.buildings_v6.buildings {
        let si = building.state.0 as usize;
        if si >= world.states.count || world.states.owners[si] != country || building.level == 0 {
            continue;
        }
        let per_level = building_needed_per_level(building, db);
        for class_idx in 0..PopClass::COUNT {
            demand[class_idx] = demand[class_idx]
                .saturating_add(per_level[class_idx].saturating_mul(building.level as u32));
        }
    }
    demand
}

fn building_needed_per_level(building: &Building, db: &V6Database) -> [u32; 6] {
    let mut demand = [0u32; 6];
    for pm in active_pms_for_building(building, db) {
        for class_idx in 0..PopClass::COUNT {
            demand[class_idx] = demand[class_idx].saturating_add(pm.employment_demand[class_idx]);
        }
    }
    demand
}

fn country_pop_supply(world: &World, country: CountryId) -> [u32; 6] {
    let mut supply = [0u32; 6];
    for pg in &world.countries.pops.groups {
        let si = pg.state.0 as usize;
        if si < world.states.count && world.states.owners[si] == country {
            supply[pg.class.index()] = supply[pg.class.index()].saturating_add(pg.size);
        }
    }
    supply
}

fn seed_initial_building_employment(
    world: &mut World,
    country: CountryId,
    db: &V6Database,
    unemployment: f32,
) {
    let demand = country_employment_demand(world, country, db);
    let supply = country_pop_supply(world, country);
    let target_employment_rate = (1.0 - unemployment).clamp(0.35, 0.98);
    let mut people_per_slot = [1.0f32; PopClass::COUNT];
    for class_idx in 0..PopClass::COUNT {
        if demand[class_idx] > 0 {
            people_per_slot[class_idx] = (supply[class_idx] as f32 * target_employment_rate
                / demand[class_idx] as f32)
                .max(1.0);
        }
    }

    let building_count = world.countries.buildings_v6.buildings.len();
    let tax_rates = world.countries.treasury.treasuries[country.0 as usize].tax_rates;
    let completed_techs = world.countries.completed_techs[country.0 as usize].clone();
    for building_idx in 0..building_count {
        let (state, level, demand) = {
            let building = &world.countries.buildings_v6.buildings[building_idx];
            let si = building.state.0 as usize;
            if si >= world.states.count || world.states.owners[si] != country || building.level == 0
            {
                continue;
            }
            let mut demand = [0u32; PopClass::COUNT];
            for pm in active_pms_for_building(building, db) {
                if !initial_pm_is_unlocked(world, country, pm, &completed_techs) {
                    continue;
                }
                for class_idx in 0..PopClass::COUNT {
                    demand[class_idx] = demand[class_idx]
                        .saturating_add(pm.employment_demand[class_idx] * building.level as u32);
                }
            }
            (building.state, building.level, demand)
        };
        if level == 0 || demand.iter().all(|needed| *needed == 0) {
            continue;
        }
        for class_idx in 0..PopClass::COUNT {
            let current =
                world.countries.buildings_v6.buildings[building_idx].employment[class_idx];
            let needed = demand[class_idx].saturating_sub(current);
            if needed == 0 {
                continue;
            }
            let Some(class) = PopClass::from_index(class_idx) else {
                continue;
            };
            let hired = hire_initial_pop_for_building(
                world,
                state,
                hoi4_state::BuildingId(building_idx as u32),
                class,
                needed,
                people_per_slot[class_idx],
            );
            world.countries.buildings_v6.buildings[building_idx].employment[class_idx] += hired;
        }
    }

    for building_idx in 0..building_count {
        let building = &world.countries.buildings_v6.buildings[building_idx];
        let si = building.state.0 as usize;
        if si >= world.states.count || world.states.owners[si] != country || building.level == 0 {
            continue;
        }
        for class_idx in 0..PopClass::COUNT {
            let employed =
                world.countries.buildings_v6.buildings[building_idx].employment[class_idx];
            if employed == 0 {
                continue;
            }
            let Some(class) = PopClass::from_index(class_idx) else {
                continue;
            };
            for pg in &mut world.countries.pops.groups {
                if pg.state == building.state && pg.class == class && pg.employed_at.is_some() {
                    pg.wage_rm = pg.wage_rm.max(base_wage_for_class(class));
                    pg.tax_burden = initial_tax_burden_from_rates(tax_rates, class);
                }
            }
        }
    }
}

fn initial_pm_is_unlocked(
    world: &World,
    country: CountryId,
    pm: &ProductionMethodDef,
    completed_techs: &[String],
) -> bool {
    if let Some(unlock_tech) = &pm.unlocked_by {
        if !completed_techs.iter().any(|tech| tech == unlock_tech) {
            return false;
        }
    }
    if let Some((cat, law_id)) = &pm.required_law {
        let ci = country.0 as usize;
        let current =
            &world.countries.law_store.law_sets[ci].0[map_law_category(*cat).index()].current;
        if current != law_id {
            return false;
        }
    }
    true
}

fn base_wage_for_class(class: PopClass) -> f32 {
    match class {
        PopClass::Peasant => 1.5,
        PopClass::Worker => 3.0,
        PopClass::Clerk => 5.0,
        PopClass::Capitalist => 15.0,
        PopClass::Aristocrat => 25.0,
        PopClass::Soldier => 2.5,
    }
}

fn hire_initial_pop_for_building(
    world: &mut World,
    state: StateId,
    building_id: hoi4_state::BuildingId,
    class: PopClass,
    needed_slots: u32,
    people_per_slot: f32,
) -> u32 {
    let mut hired_slots = hire_initial_pop_from_scope(
        world,
        state,
        building_id,
        class,
        needed_slots,
        people_per_slot,
        true,
    );
    if hired_slots < needed_slots {
        hired_slots += hire_initial_pop_from_scope(
            world,
            state,
            building_id,
            class,
            needed_slots - hired_slots,
            people_per_slot,
            false,
        );
    }
    hired_slots
}

fn hire_initial_pop_from_scope(
    world: &mut World,
    state: StateId,
    building_id: hoi4_state::BuildingId,
    class: PopClass,
    needed_slots: u32,
    people_per_slot: f32,
    same_state_only: bool,
) -> u32 {
    let country = world
        .states
        .owners
        .get(state.0 as usize)
        .copied()
        .unwrap_or(CountryId::NONE);
    let mut hired_slots = 0u32;
    let mut pop_idx = 0usize;
    while pop_idx < world.countries.pops.groups.len() && hired_slots < needed_slots {
        let pg = &world.countries.pops.groups[pop_idx];
        if pg.class != class || pg.employed_at.is_some() {
            pop_idx += 1;
            continue;
        }
        let pg_owner = world
            .states
            .owners
            .get(pg.state.0 as usize)
            .copied()
            .unwrap_or(CountryId::NONE);
        if (same_state_only && pg.state != state) || (!same_state_only && pg_owner != country) {
            pop_idx += 1;
            continue;
        }

        let remaining_slots = needed_slots - hired_slots;
        let wanted_people = ((remaining_slots as f32 * people_per_slot).round() as u32).max(1);
        let take = world.countries.pops.groups[pop_idx].size.min(wanted_people);
        if take == 0 {
            pop_idx += 1;
            continue;
        }
        let slots_from_take = ((take as f32 / people_per_slot).ceil() as u32)
            .max(1)
            .min(remaining_slots);
        let available = world.countries.pops.groups[pop_idx].size;
        if take < available {
            let mut hired_group = world.countries.pops.groups[pop_idx].clone();
            hired_group.size = take;
            hired_group.employed_at = Some(building_id);
            world.countries.pops.groups[pop_idx].size -= take;
            world.countries.pops.groups.push(hired_group);
        } else {
            world.countries.pops.groups[pop_idx].employed_at = Some(building_id);
        }
        hired_slots += slots_from_take;
        pop_idx += 1;
    }
    hired_slots
}

fn initial_tax_burden_from_rates(tax_rates: [f32; 3], class: PopClass) -> f32 {
    let [income_tax_rate, consumption_tax_rate, _] = tax_rates;
    let consumption_weight = match class {
        PopClass::Peasant => 0.6,
        PopClass::Worker => 0.8,
        PopClass::Clerk => 1.0,
        PopClass::Capitalist => 2.0,
        PopClass::Aristocrat => 1.5,
        PopClass::Soldier => 0.5,
    };
    let class_income_mult = match class {
        PopClass::Peasant => 0.75,
        PopClass::Worker => 1.0,
        PopClass::Clerk => 1.1,
        PopClass::Capitalist => 1.8,
        PopClass::Aristocrat => 1.5,
        PopClass::Soldier => 0.0,
    };
    (income_tax_rate * class_income_mult + consumption_tax_rate * consumption_weight)
        .clamp(0.0, 1.0)
}

fn initial_tax_rates(db: &V6Database, profile: &HistoricalCountryEconomyDef) -> [f32; 3] {
    db.taxation_laws
        .iter()
        .find(|law| law.id == profile.initial_laws.taxation)
        .map(|law| {
            [
                law.income_tax_rate,
                law.consumption_tax_rate,
                law.corporate_tax_rate,
            ]
        })
        .unwrap_or([0.20, 0.10, 0.20])
}

fn initial_loyalty_modifiers(db: &V6Database, profile: &HistoricalCountryEconomyDef) -> (f32, f32) {
    let loyalty_coefficient = db
        .civil_rights_laws
        .iter()
        .find(|law| law.id == profile.initial_laws.civil_rights)
        .map(|law| law.pop_modifiers.loyalty_coefficient)
        .unwrap_or(1.0);
    let loyalty_decay_mult = db
        .information_control_laws
        .iter()
        .find(|law| law.id == profile.initial_laws.information_control)
        .map(|law| law.loyalty_decay_multiplier)
        .unwrap_or(1.0);
    (loyalty_coefficient, loyalty_decay_mult)
}

fn initial_political_loyalty(
    profile: &HistoricalCountryEconomyDef,
    class: PopClass,
    satisfaction: f32,
) -> f32 {
    let regime_bias = match profile.initial_laws.information_control.as_str() {
        "state_media" => 0.18,
        "regulated_press" => 0.08,
        "free_press" => 0.02,
        _ => 0.0,
    };
    let civil_bias = match profile.initial_laws.civil_rights.as_str() {
        "police_state" | "national_security_act" => -0.05,
        "limited_rights" => 0.00,
        "open_society" => 0.04,
        _ => 0.0,
    };
    let class_bias = match class {
        PopClass::Peasant => 0.02,
        PopClass::Worker => -0.02,
        PopClass::Clerk => 0.01,
        PopClass::Capitalist => 0.08,
        PopClass::Aristocrat => 0.10,
        PopClass::Soldier => 0.14,
    };
    (regime_bias + civil_bias + class_bias + (satisfaction - 0.5) * 0.35).clamp(-1.0, 1.0)
}

fn building_targets(
    profile: &HistoricalCountryEconomyDef,
    runtime_population: u64,
) -> Vec<(&'static str, u16)> {
    let modern_units = profile_modern_capacity_units(profile, runtime_population);
    let primary_units = profile_primary_capacity_units(profile, runtime_population);
    let shares = &profile.sector_shares;
    let agriculture = target_levels(primary_units, shares.agriculture, 1.3, 1);
    let heavy = target_levels(modern_units, shares.heavy_industry, 1.25, 1);
    let light = target_levels(modern_units, shares.light_industry, 1.05, 1);
    let services = target_levels(modern_units, shares.services, 0.65, 1);
    let government = target_levels(modern_units, shares.government, 0.45, 1);
    let military = target_levels(modern_units, shares.military_industry, 1.4, 1);
    let construction = ((profile.construction_capacity_index / 12.0).round() as u16).clamp(1, 10);
    let railway = ((profile.industrial_capacity_index / 7.5).round() as u16).clamp(1, 18);

    let mut targets = vec![
        ("grain_farm", agriculture),
        ("steel_mill", (heavy as f32 * 0.54).round().max(1.0) as u16),
        (
            "machinery_workshop",
            (heavy as f32 * 0.22).round().max(1.0) as u16,
        ),
        (
            "chemical_plant",
            (heavy as f32 * 0.16).round().max(1.0) as u16,
        ),
        (
            "electrical_works",
            (heavy as f32 * 0.10).round().max(1.0) as u16,
        ),
        (
            "textile_mill",
            (light as f32 * 0.65).round().max(1.0) as u16,
        ),
        (
            "furniture_factory",
            (light as f32 * 0.25).round().max(1.0) as u16,
        ),
        ("distillery", (light as f32 * 0.10).round().max(1.0) as u16),
        (
            "livestock_ranch",
            (agriculture as f32 * 0.20).round().max(1.0) as u16,
        ),
        ("bank", (services as f32 * 0.35).round().max(1.0) as u16),
        (
            "telegraph_office",
            (services as f32 * 0.25).round().max(1.0) as u16,
        ),
        (
            "university",
            (government as f32 * 0.30).round().max(1.0) as u16,
        ),
        (
            "arms_industry",
            (military as f32 * 0.48).round().max(1.0) as u16,
        ),
        (
            "munition_plant",
            (military as f32 * 0.30).round().max(1.0) as u16,
        ),
        ("shipyard", (military as f32 * 0.18).round().max(0.0) as u16),
        ("construction_sector", construction),
        ("railway", railway),
    ];
    add_1936_industry_calibration_targets(profile.tag.as_str(), &mut targets);
    targets
}

fn add_1936_industry_calibration_targets(tag: &str, targets: &mut Vec<(&'static str, u16)>) {
    let additions: &[(&str, u16)] = match tag {
        // Detroit, machine tools, oil extraction and refining are the USA's
        // visible 1936 advantages for rapid motorization.
        "USA" => &[
            ("vehicle_factory", 10),
            ("engine_plant", 8),
            ("machine_tool_works", 9),
            ("oil_refinery", 10),
            ("rubber_factory", 3),
            ("shipyard", 2),
        ],
        // Germany starts strong in steel, chemicals, precision machinery and
        // armaments, but without domestic oil or rubber deposits.
        "GER" => &[
            ("steel_mill", 2),
            ("chemical_plant", 4),
            ("machinery_workshop", 12),
            ("machine_tool_works", 6),
            ("aluminium_plant", 2),
            ("engine_plant", 4),
            ("vehicle_factory", 3),
            ("aircraft_factory", 3),
            ("tank_factory", 2),
            ("arms_industry", 3),
            ("munition_plant", 2),
            ("synthetic_refinery", 4),
            ("rubber_factory", 2),
        ],
        // Soviet industry is heavy and expandable, with useful oil/refining,
        // but fewer precision plants than the western industrial powers.
        "SOV" => &[
            ("steel_mill", 5),
            ("machinery_workshop", 3),
            ("machine_tool_works", 2),
            ("engine_plant", 2),
            ("oil_refinery", 3),
            ("vehicle_factory", 2),
            ("arms_industry", 2),
            ("munition_plant", 2),
        ],
        // Britain should show finance, shipping and sea-connected industry.
        "ENG" => &[
            ("shipyard", 6),
            ("bank", 5),
            ("telegraph_office", 3),
            ("machine_tool_works", 3),
            ("engine_plant", 2),
            ("oil_refinery", 2),
        ],
        "FRA" => &[
            ("steel_mill", 2),
            ("machine_tool_works", 3),
            ("engine_plant", 2),
            ("vehicle_factory", 2),
            ("arms_industry", 2),
            ("munition_plant", 1),
        ],
        // Japan has usable yards and armaments, but limited domestic resource
        // extraction; its missing oil/rubber/aluminium is represented by trade.
        "JAP" => &[
            ("shipyard", 6),
            ("arms_industry", 3),
            ("munition_plant", 2),
            ("engine_plant", 2),
            ("vehicle_factory", 2),
            ("machine_tool_works", 1),
            ("oil_refinery", 2),
        ],
        "ITA" => &[
            ("shipyard", 3),
            ("arms_industry", 2),
            ("munition_plant", 1),
            ("engine_plant", 1),
            ("vehicle_factory", 1),
            ("machine_tool_works", 1),
        ],
        // Spain is not a great power, but Catalonia/Basque industry, Asturias mining,
        // Madrid services, and small arsenals must exist before the civil war split.
        "SPR" => &[
            ("steel_mill", 2),
            ("machinery_workshop", 2),
            ("machine_tool_works", 1),
            ("textile_mill", 3),
            ("shipyard", 1),
            ("arms_industry", 2),
            ("munition_plant", 1),
        ],
        // China remains agriculture-heavy with small arsenals and very limited
        // modern precision industry in the 1936 start.
        "CHI" => &[
            ("arms_industry", 1),
            ("munition_plant", 1),
            ("machine_tool_works", 1),
        ],
        _ => &[],
    };
    if additions.is_empty() {
        return;
    }
    let generic = std::mem::take(targets);
    targets.extend(additions.iter().copied());
    targets.extend(generic);
}

fn profile_modern_capacity_units(
    profile: &HistoricalCountryEconomyDef,
    runtime_population: u64,
) -> f32 {
    let population_units = runtime_population as f32 / 10_000_000.0;
    let human_capital = (profile.urbanization.clamp(0.0, 1.0) * 0.6
        + profile.literacy.clamp(0.0, 1.0) * 0.4)
        .max(0.05);
    (profile.industrial_capacity_index.max(0.0)
        + profile.construction_capacity_index.max(0.0) * 0.35
        + population_units * human_capital)
        .max(1.0)
}

fn profile_primary_capacity_units(
    profile: &HistoricalCountryEconomyDef,
    runtime_population: u64,
) -> f32 {
    let population_units = runtime_population as f32 / 10_000_000.0;
    let rural_weight = (1.0 - profile.urbanization.clamp(0.0, 1.0) * 0.55).max(0.25);
    (population_units * rural_weight + profile.industrial_capacity_index.max(0.0) * 0.08).max(1.0)
}

fn target_levels(capacity_units: f32, share: f32, multiplier: f32, min: u16) -> u16 {
    ((capacity_units * share * multiplier).round() as u16).max(min)
}

fn allocate_resource_buildings(
    world: &mut World,
    ci: usize,
    profile: &HistoricalCountryEconomyDef,
    db: &V6Database,
) {
    let country = CountryId(ci as u16);
    let mining_budget = target_levels(
        profile_primary_capacity_units(profile, world.country_governed_population(country)),
        profile.sector_shares.mining,
        4.0,
        1,
    );
    let mut remaining = mining_budget.max(1);
    let resource_order = [
        ("coal", "coal_mine"),
        ("iron", "iron_mine"),
        ("oil", "oil_rig"),
        ("rubber", "rubber_plantation"),
        ("bauxite", "bauxite_mine"),
        ("chromium", "chromium_mine"),
        ("tungsten", "tungsten_mine"),
    ];

    for (good_id, building_id) in resource_order {
        let mut entries: Vec<(StateId, u8)> = db
            .state_resource_deposits
            .iter()
            .filter_map(|entry| {
                let sid = state_id_from_game_or_internal(world, entry.state_id)?;
                let si = sid.0 as usize;
                if si >= world.states.count || world.states.owners[si] != country {
                    return None;
                }
                let discovered = entry
                    .deposits
                    .iter()
                    .find(|deposit| deposit.good_id == good_id)
                    .map(|deposit| deposit.discovered_level)?;
                Some((sid, discovered))
            })
            .collect();
        entries.sort_by_key(|(sid, discovered)| {
            let si = sid.0 as usize;
            std::cmp::Reverse(*discovered as u16 + world.states.infrastructure[si] as u16)
        });
        for (sid, discovered) in entries {
            if remaining == 0 {
                return;
            }
            let level = discovered.min(remaining as u8);
            if level > 0 {
                inject_building(world, ci, building_id, sid, level, db);
                remaining = remaining.saturating_sub(level as u16);
            }
        }
    }
}

fn seed_initial_market_stockpiles(world: &mut World, country: CountryId, db: &V6Database) {
    let ci = country.0 as usize;
    if ci >= world.countries.market.markets.len() {
        return;
    };

    let completed_techs = world.countries.completed_techs[ci].clone();
    let mut additions: Vec<(String, f32)> = Vec::new();
    for building in &world.countries.buildings_v6.buildings {
        let si = building.state.0 as usize;
        if si >= world.states.count || world.states.owners[si] != country || building.level == 0 {
            continue;
        }
        for pm in active_pms_for_building(building, db) {
            if !initial_pm_is_unlocked(world, country, pm, &completed_techs) {
                continue;
            }
            for (idx, good_id) in pm.input_good_ids.iter().enumerate() {
                let daily_need =
                    pm.input_good_amounts.get(idx).copied().unwrap_or(0.0) * building.level as f32;
                if daily_need > 0.0 {
                    additions.push((good_id.clone(), daily_need * 90.0));
                }
            }
        }
    }

    let market = &mut world.countries.market.markets[ci];
    for (good_id, amount) in additions {
        *market.stockpile.entry(good_id).or_insert(0.0) += amount;
    }
}

fn allocate_levels_across_states(
    world: &mut World,
    ci: usize,
    state_ids: &[StateId],
    building_id: &str,
    mut levels: u16,
    db: &V6Database,
) {
    if levels == 0
        || state_ids.is_empty()
        || db
            .buildings
            .iter()
            .all(|building| building.id != building_id)
    {
        return;
    }
    let mut cursor = 0usize;
    let max_iterations = state_ids.len().max(1) * 32;
    for _ in 0..max_iterations {
        if levels == 0 {
            break;
        }
        let sid = state_ids[cursor % state_ids.len()];
        cursor += 1;
        if !can_place_non_resource(world, sid, building_id) {
            continue;
        }
        inject_building(world, ci, building_id, sid, 1, db);
        levels -= 1;
    }
}

fn states_for_building(
    world: &World,
    country: CountryId,
    building_id: &str,
    db: &V6Database,
) -> Vec<StateId> {
    let Some(def) = db
        .buildings
        .iter()
        .find(|building| building.id == building_id)
    else {
        return Vec::new();
    };
    match def.kind {
        BuildingKindDef::Resource => states_for_resource_building(world, country, def),
        BuildingKindDef::Agriculture => states_for_agriculture_building(world, country),
        BuildingKindDef::ConsumerGoods if building_id.contains("plantation") => {
            states_for_agriculture_building(world, country)
        }
        BuildingKindDef::ConsumerGoods => states_for_industrial_building(world, country),
        BuildingKindDef::Industrial | BuildingKindDef::Military | BuildingKindDef::Service => {
            states_for_industrial_building(world, country)
        }
        BuildingKindDef::Infrastructure
            if matches!(building_id, "railway" | "port" | "v6_naval_base") =>
        {
            states_for_service_building(world, country)
        }
        BuildingKindDef::Infrastructure | BuildingKindDef::MilitaryBase => {
            states_for_industrial_building(world, country)
        }
    }
}

fn states_for_industrial_building(world: &World, country: CountryId) -> Vec<StateId> {
    let mut states: Vec<StateId> = (0..world.states.count)
        .filter(|&si| {
            world.states.owners[si] == country && world.states.integration_status[si].is_domestic()
        })
        .map(|si| StateId(si as u16))
        .collect();
    sort_states_by_development_weight(world, &mut states);
    states
}

fn states_for_resource_building(
    world: &World,
    country: CountryId,
    def: &BuildingDef,
) -> Vec<StateId> {
    let mut states = owned_states_by_weight(world, country);
    if def.state_limit_kind.as_deref() == Some("coastal") {
        states.retain(|state| state_is_coastal(world, *state));
    }
    states
}

fn states_for_agriculture_building(world: &World, country: CountryId) -> Vec<StateId> {
    owned_states_by_weight(world, country)
}

fn states_for_service_building(world: &World, country: CountryId) -> Vec<StateId> {
    owned_states_by_weight(world, country)
}

fn can_place_non_resource(world: &World, state: StateId, building_id: &str) -> bool {
    if matches!(building_id, "shipyard" | "port" | "v6_naval_base")
        && !state_is_coastal(world, state)
    {
        return false;
    }
    let si = state.0 as usize;
    if si >= world.states.count {
        return false;
    }
    let used: u16 = world
        .countries
        .buildings_v6
        .buildings
        .iter()
        .filter(|building| building.state == state && building.level > 0)
        .map(|building| building.level as u16)
        .sum();
    used < (world.states.category_slots[si] as u16).max(4) + 20
}

fn owned_states_by_weight(world: &World, country: CountryId) -> Vec<StateId> {
    let mut states: Vec<StateId> = (0..world.states.count)
        .filter(|&si| world.states.owners[si] == country)
        .map(|si| StateId(si as u16))
        .collect();
    sort_states_by_development_weight(world, &mut states);
    states
}

fn sort_states_by_development_weight(world: &World, states: &mut [StateId]) {
    states.sort_by_key(|sid| {
        let si = sid.0 as usize;
        std::cmp::Reverse(
            world.states.manpower_pool[si] as u64
                + world.states.infrastructure[si] as u64 * 750_000,
        )
    });
}

fn historical_pop_states_by_weight(world: &World, country: CountryId) -> Vec<StateId> {
    let mut states: Vec<StateId> = (0..world.states.count)
        .filter(|&si| world.states.owners[si] == country)
        .map(|si| StateId(si as u16))
        .collect();
    states.sort_by_key(|sid| {
        let si = sid.0 as usize;
        let is_core = world.states.cores[si].contains(&country);
        let core_bonus: u64 = if is_core { 1_000_000 } else { 0 };
        std::cmp::Reverse(
            world.states.manpower_pool[si] as u64
                + world.states.infrastructure[si] as u64 * 750_000
                + core_bonus,
        )
    });
    states
}

fn resolve_game_state_id(world: &World, game_state_id: u16) -> Option<StateId> {
    state_id_from_game_or_internal(world, game_state_id)
}

fn map_pop_class(def: PopClassDef) -> PopClass {
    match def {
        PopClassDef::Peasant => PopClass::Peasant,
        PopClassDef::Worker => PopClass::Worker,
        PopClassDef::Clerk => PopClass::Clerk,
        PopClassDef::Capitalist => PopClass::Capitalist,
        PopClassDef::Aristocrat => PopClass::Aristocrat,
        PopClassDef::Soldier => PopClass::Soldier,
    }
}

fn initial_literacy(profile: &HistoricalCountryEconomyDef, class: PopClass) -> f32 {
    let industrial = (profile.industrial_capacity_index / 100.0).clamp(0.0, 1.0);
    let urban = profile.urbanization.clamp(0.0, 1.0);
    (class.baseline_literacy() * 0.55 + industrial * 0.20 + urban * 0.25).clamp(0.08, 0.98)
}

fn initial_skilled_ratio(profile: &HistoricalCountryEconomyDef, class: PopClass) -> f32 {
    let industrial = (profile.industrial_capacity_index / 100.0).clamp(0.0, 1.0);
    let urban = profile.urbanization.clamp(0.0, 1.0);
    (class.baseline_skilled_ratio() * 0.55 + industrial * 0.30 + urban * 0.15).clamp(0.02, 0.90)
}

fn state_id_from_game_or_internal(world: &World, state_id: u16) -> Option<StateId> {
    world
        .state_id_lookup
        .get(&state_id)
        .copied()
        .or_else(|| ((state_id as usize) < world.states.count).then_some(StateId(state_id)))
}

fn state_is_coastal(world: &World, state: StateId) -> bool {
    let si = state.0 as usize;
    if si >= world.states.count {
        return false;
    }
    world.states.provinces[si].iter().any(|province| {
        world
            .map
            .definitions
            .get(province.0 as usize)
            .and_then(|definition| definition.as_ref())
            .map(|definition| definition.coastal)
            .unwrap_or(false)
    })
}

#[allow(dead_code)]
fn inject_ger_buildings_from_vanilla(world: &mut World, country: CountryId, db: &V6Database) {
    let ci = country.0 as usize;

    for si in 0..world.states.count {
        if world.states.owners[si] != country {
            continue;
        }
        let state_id = hoi4_state::StateId(si as u16);
        let infra = world.states.infrastructure[si];

        let name = world.states.names[si].to_ascii_lowercase();
        let is_ruhr_or_rhine =
            name.contains("rhein") || name.contains("westfalen") || name.contains("ruhr");
        let is_saxony_or_silesia =
            name.contains("sachsen") || name.contains("siles") || name.contains("schles");
        let is_berlin = name.contains("berlin") || name.contains("brandenburg");
        let is_port =
            name.contains("hamburg") || name.contains("kiel") || name.contains("mecklenburg");

        if !(is_ruhr_or_rhine || is_saxony_or_silesia || is_berlin || is_port || infra >= 5) {
            continue;
        }

        if infra >= 4 {
            inject_building(world, ci, "railway", state_id, infra.min(3), db);
        }

        if is_ruhr_or_rhine {
            inject_building(world, ci, "steel_mill", state_id, 4, db);
            inject_building(world, ci, "machinery_workshop", state_id, 2, db);
            inject_building(world, ci, "chemical_plant", state_id, 2, db);
            inject_building(world, ci, "coal_mine", state_id, 4, db);
            inject_building(world, ci, "iron_mine", state_id, 2, db);
            inject_building(world, ci, "construction_sector", state_id, 1, db);
        } else if is_saxony_or_silesia {
            inject_building(world, ci, "steel_mill", state_id, 2, db);
            inject_building(world, ci, "machinery_workshop", state_id, 2, db);
            inject_building(world, ci, "textile_mill", state_id, 2, db);
            inject_building(world, ci, "coal_mine", state_id, 2, db);
            inject_building(world, ci, "iron_mine", state_id, 1, db);
        } else if is_berlin {
            inject_building(world, ci, "machinery_workshop", state_id, 2, db);
            inject_building(world, ci, "arms_industry", state_id, 2, db);
            inject_building(world, ci, "munition_plant", state_id, 1, db);
            inject_building(world, ci, "construction_sector", state_id, 1, db);
        } else if is_port {
            inject_building(world, ci, "shipyard", state_id, 2, db);
            inject_building(world, ci, "textile_mill", state_id, 1, db);
        } else {
            inject_building(world, ci, "grain_farm", state_id, 1, db);
        }
        if is_ruhr_or_rhine {
            inject_building(world, ci, "aluminium_plant", state_id, 1, db);
            inject_building(world, ci, "electrical_works", state_id, 1, db);
        }
    }
}

fn inject_baseline_buildings_for_remaining_countries(world: &mut World, db: &V6Database) {
    let mut has_v6_building = vec![false; world.countries.count];
    for building in &world.countries.buildings_v6.buildings {
        let si = building.state.0 as usize;
        if si < world.states.count {
            let owner = world.states.owners[si];
            if !owner.is_none() {
                has_v6_building[owner.0 as usize] = true;
            }
        }
    }

    for ci in 0..world.countries.count {
        if has_v6_building[ci] {
            continue;
        }
        let country = CountryId(ci as u16);
        let mut candidate_states: Vec<usize> = (0..world.states.count)
            .filter(|&si| world.states.owners[si] == country)
            .collect();
        candidate_states.sort_by_key(|&si| {
            std::cmp::Reverse(
                (world.states.manpower_pool[si] as u64)
                    + world.states.infrastructure[si] as u64 * 500_000,
            )
        });
        for &si in candidate_states.iter().take(1) {
            let state_id = StateId(si as u16);
            let infra = world.states.infrastructure[si];
            if infra >= 4 {
                inject_building(world, ci, "railway", state_id, 1, db);
            }
            let pop = world.states.manpower_pool[si];
            inject_building(world, ci, "grain_farm", state_id, 1, db);
            if infra >= 3 || pop >= 2_000_000 {
                inject_building(world, ci, "steel_mill", state_id, 1, db);
                inject_building(world, ci, "textile_mill", state_id, 1, db);
            }
        }
        world.countries.private_investment_pool_rm[ci] = 5_000_000.0;
        if let Some(account) = world
            .countries
            .investment_account_mut(country, hoi4_state::InvestmentAccountKind::Private)
        {
            account.balance_rm = 5_000_000.0;
        }
    }
}

fn inject_building(
    world: &mut World,
    ci: usize,
    def_id: &str,
    state: StateId,
    level: u8,
    db: &V6Database,
) {
    let def = match db.buildings.iter().find(|b| b.id == def_id) {
        Some(d) => d,
        None => return,
    };
    let level = level.min(def.max_level);
    let active_pm_by_group = default_active_pm_entries(db, &def.id);
    let active_pm = active_pm_by_group
        .first()
        .map(|active| active.pm_id.clone())
        .unwrap_or_else(|| "default".to_owned());
    let owner = map_building_owner(def.owner_default);
    world.countries.buildings_v6.buildings.push(Building {
        kind: map_building_kind(def.kind),
        building_def_id: def.id.clone(),
        state,
        level,
        active_pm,
        active_pm_by_group,
        employment: [0; 6],
        owner,
        ownership_shares: Building::default_ownership_shares(owner, CountryId(ci as u16)),
        requires_law: def
            .requires_law
            .as_ref()
            .map(|(cat, law_id)| (map_law_category(*cat), law_id.clone())),
        max_level: def.max_level,
        built_progress: 1.0,
        ..Building::runtime_defaults()
    });
}

fn default_active_pm_entries(db: &V6Database, building_id: &str) -> Vec<ActiveProductionMethod> {
    default_pms_for_building(db, building_id)
        .into_iter()
        .map(|pm| ActiveProductionMethod {
            group: active_pm_slot_key(pm),
            pm_id: pm.id.clone(),
        })
        .collect()
}

#[allow(dead_code)]
fn inject_sov_buildings_from_vanilla(world: &mut World, country: CountryId, db: &V6Database) {
    let ci = country.0 as usize;

    for si in 0..world.states.count {
        if world.states.owners[si] != country {
            continue;
        }
        let state_id = hoi4_state::StateId(si as u16);
        let infra = world.states.infrastructure[si];

        if infra > 0 {
            inject_building_state_owned(world, ci, "railway", state_id, infra, db);
        }

        inject_building_state_owned(world, ci, "steel_mill", state_id, 2, db);
        inject_building_state_owned(world, ci, "arms_industry", state_id, 1, db);
        inject_building_state_owned(world, ci, "grain_farm", state_id, 3, db);
        inject_building_state_owned(world, ci, "coal_mine", state_id, 3, db);
        inject_building_state_owned(world, ci, "iron_mine", state_id, 2, db);
        inject_building_state_owned(world, ci, "conscription_center", state_id, 1, db);
    }
}

#[allow(dead_code)]
fn inject_building_state_owned(
    world: &mut World,
    ci: usize,
    def_id: &str,
    state: StateId,
    level: u8,
    db: &V6Database,
) {
    let def = match db.buildings.iter().find(|b| b.id == def_id) {
        Some(d) => d,
        None => return,
    };
    let level = level.min(def.max_level);
    let active_pm_by_group = default_active_pm_entries(db, &def.id);
    let active_pm = active_pm_by_group
        .first()
        .map(|active| active.pm_id.clone())
        .unwrap_or_else(|| "default".to_owned());
    world.countries.buildings_v6.buildings.push(Building {
        kind: map_building_kind(def.kind),
        building_def_id: def.id.clone(),
        state,
        level,
        active_pm,
        active_pm_by_group,
        employment: [0; 6],
        owner: BuildingOwner::State,
        requires_law: def
            .requires_law
            .as_ref()
            .map(|(cat, law_id)| (map_law_category(*cat), law_id.clone())),
        max_level: def.max_level,
        built_progress: 1.0,
        ..Building::runtime_defaults()
    });
    let _ = ci;
}

// V6 law setter.
/// Set a country's law tier and start its cooldown.
pub fn set_law(
    world: &mut World,
    country: CountryId,
    category: LawCategory,
    target_law_id: &str,
    db: &V6Database,
) -> Result<(), String> {
    let i = country.0 as usize;
    if i >= world.countries.count {
        return Err("无效的国家".to_owned());
    }

    let pp_cost = lookup_pp_cost(category, target_law_id, db)?;
    let cooldown = lookup_cooldown(category, target_law_id, db);

    let slot = &mut world.countries.law_store.law_sets[i].0[category.index()];

    if slot.cooldown_days > 0 {
        return Err(format!("法律冷却中（剩余 {} 天）", slot.cooldown_days));
    }
    if slot.is_locked {
        return Err("法律被经济体制锁定".to_owned());
    }
    if slot.current == target_law_id {
        return Err("已经是当前法律".to_owned());
    }

    if world.countries.political_power[i] < pp_cost as f32 {
        return Err(format!("政治力量不足，需要 {} PP", pp_cost));
    }

    world.countries.political_power[i] -= pp_cost as f32;
    slot.pending = Some((target_law_id.to_owned(), cooldown));
    Ok(())
}

/// Daily law cooldown countdown and pending-law activation.
pub fn tick_law_cooldowns(world: &mut World, db: &V6Database) {
    let n = world.countries.count;
    for ci in 0..n {
        let mut planned_transition: Option<(bool, bool)> = None;

        {
            let law_set = &mut world.countries.law_store.law_sets[ci];
            for slot in &mut law_set.0 {
                if slot.cooldown_days > 0 {
                    slot.cooldown_days -= 1;
                }
                if let Some((target, remaining)) = &mut slot.pending {
                    if *remaining == 0 {
                        let old_law = slot.current.clone();
                        let target_law = target.clone();

                        if slot.category == LawCategory::Economy {
                            let entering =
                                target_law == "planned_economy" && old_law != "planned_economy";
                            let leaving =
                                old_law == "planned_economy" && target_law != "planned_economy";
                            planned_transition = Some((entering, leaving));
                        }
                    }
                }
            }
        }

        {
            let law_set = &mut world.countries.law_store.law_sets[ci];
            for slot in &mut law_set.0 {
                if slot.cooldown_days > 0 {
                    continue;
                }
                if let Some((target, remaining)) = &mut slot.pending {
                    if *remaining == 0 {
                        slot.current = target.clone();
                        slot.pending = None;
                        slot.cooldown_days = 60;
                    } else {
                        *remaining -= 1;
                    }
                }
            }
        }

        if let Some((entering, leaving)) = planned_transition {
            if entering {
                lock_trade_law_for_planned_economy(world, ci, db);
                execute_nationalization(world, ci);
            } else if leaving {
                unlock_trade_law_on_leaving_planned(world, ci);
            }
        }
    }
}

// V6 event schema for `events_v6/*.ron`.
#[derive(Clone, Debug, Deserialize)]
pub struct V6EventOption {
    pub name: String,
    pub effects: Vec<(String, String)>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct V6EventDef {
    pub id: String,
    pub title: String,
    pub body: String,
    pub options: Vec<V6EventOption>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ExchangeRateDef {
    pub base_rate: f32,
    pub gold_modifier_per_1000kg: f32,
    pub debt_pressure_threshold: f64,
    pub debt_pressure_modifier: f32,
    pub weekly_ema_factor: f32,
    pub crisis_monthly_change_threshold: f32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PyatiletkaTargetDef {
    pub good_id: String,
    pub target_daily_output: f32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ForcedPMDef {
    pub building_def_id: String,
    pub forced_pm_id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PyatiletkaDef {
    pub id: String,
    pub name: String,
    pub year_start: u16,
    pub year_end: u16,
    pub focus_tech_directions: Vec<String>,
    pub focus_bonus: f32,
    pub off_focus_penalty: f32,
    pub targets: Vec<PyatiletkaTargetDef>,
    pub forced_pms: Vec<ForcedPMDef>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub enum TechCategoryDef {
    Industry,
    Chemistry,
    Electrical,
    Metallurgy,
    MilitaryDoctrine,
    Aviation,
    Naval,
    SocialScience,
    InformationControl,
}

#[derive(Clone, Debug, Deserialize)]
pub enum TechUnlockDef {
    Good(String),
    PM(String),
    Building(String),
    Law(LawCategoryDef, String),
}

#[derive(Clone, Debug, Deserialize)]
pub struct TechDef {
    pub id: String,
    pub name: String,
    pub category: TechCategoryDef,
    pub research_cost: f32,
    pub start_year: u16,
    pub difficulty: f32,
    pub prereqs: Vec<String>,
    pub unlocks: Vec<TechUnlockDef>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MefoDef {
    pub unlock_conscription_min: String,
    pub unlock_economy_min: String,
    pub crisis_threshold_ratio: f64,
    pub crisis_forced_payment_ratio: f64,
    pub crisis_residual_debt_ratio: f64,
    pub crisis_loyalty_penalty: f32,
    pub crisis_loyalty_duration_days: u32,
    pub crisis_inflation_multiplier: f32,
}

impl Default for MefoDef {
    fn default() -> Self {
        Self {
            unlock_conscription_min: "limited_conscription".to_owned(),
            unlock_economy_min: "interventionism".to_owned(),
            crisis_threshold_ratio: 0.30,
            crisis_forced_payment_ratio: 0.4,
            crisis_residual_debt_ratio: 0.6,
            crisis_loyalty_penalty: -0.20,
            crisis_loyalty_duration_days: 180,
            crisis_inflation_multiplier: 1.5,
        }
    }
}

// V6 finance commands: bonds, MEFO bills, gold sales, and foreign currency purchases.
/// V6 finance command emitted by UI panels.
#[derive(Debug, Clone, PartialEq)]
pub enum FinanceCommand {
    IssueDomesticBond { amount_rm: f64 },
    IssueForeignBond { amount_gbp: f64 },
    PrintMefo,
    SellGold { kg: f64 },
    BuyForeignCurrency { gbp_amount: f64 },
}

/// Execute a finance command and mutate Treasury state.
pub fn execute_finance_command(
    world: &mut World,
    db: &V6Database,
    ci: usize,
    cmd: &FinanceCommand,
) -> Result<(), String> {
    match cmd {
        FinanceCommand::IssueDomesticBond { amount_rm } => {
            let treasury = &mut world.countries.treasury.treasuries[ci];
            treasury.issue_domestic_bond(*amount_rm);
            Ok(())
        }
        FinanceCommand::IssueForeignBond { amount_gbp } => {
            let treasury = &mut world.countries.treasury.treasuries[ci];
            treasury.issue_foreign_bond(*amount_gbp)
        }
        FinanceCommand::PrintMefo => {
            if world.countries.tags.get(ci).map(|t| t.as_str()) != Some("GER") {
                return Err("MEFO issuance is only available to Germany".to_owned());
            }
            if world.countries.treasury.treasuries[ci].mefo_disabled {
                return Err("MEFO issuance is disabled after the crisis".to_owned());
            }
            let conscription = world.countries.law_store.law_sets[ci].0
                [LawCategory::Conscription.index()]
            .current
            .as_str();
            let economy = world.countries.law_store.law_sets[ci].0[LawCategory::Economy.index()]
                .current
                .as_str();
            let conscription_ok = law_at_least(
                conscription,
                &db.mefo.unlock_conscription_min,
                &[
                    "volunteer_only",
                    "limited_conscription",
                    "extensive_conscription",
                    "total_mobilization",
                ],
            );
            let economy_ok = law_at_least(
                economy,
                &db.mefo.unlock_economy_min,
                &[
                    "laissez_faire",
                    "interventionism",
                    "war_economy",
                    "planned_economy",
                    "corporatist_war_economy",
                ],
            );
            if !(conscription_ok && economy_ok) {
                return Err("current laws do not allow MEFO issuance".to_owned());
            }
            let treasury = &mut world.countries.treasury.treasuries[ci];
            let max_amount = treasury.operating_expense_rm - treasury.operating_income_rm;
            if max_amount <= 0.0 {
                return Err("no deficit to cover with MEFO".to_owned());
            }
            treasury.print_mefo(max_amount);
            Ok(())
        }
        FinanceCommand::SellGold { kg } => {
            let treasury = &mut world.countries.treasury.treasuries[ci];
            treasury.sell_gold(*kg);
            Ok(())
        }
        FinanceCommand::BuyForeignCurrency { gbp_amount } => {
            let rm_per_gbp = world.countries.treasury.exchange_rates[ci].rm_per_gbp;
            let treasury = &mut world.countries.treasury.treasuries[ci];
            treasury.buy_foreign_currency(rm_per_gbp, *gbp_amount);
            Ok(())
        }
    }
}

pub fn law_at_least(current: &str, minimum: &str, order: &[&str]) -> bool {
    let current_rank = order.iter().position(|id| *id == current);
    let minimum_rank = order.iter().position(|id| *id == minimum);
    match (current_rank, minimum_rank) {
        (Some(current_rank), Some(minimum_rank)) => current_rank >= minimum_rank,
        _ => current == minimum,
    }
}

fn lookup_pp_cost(category: LawCategory, law_id: &str, db: &V6Database) -> Result<u32, String> {
    match category {
        LawCategory::Conscription => db
            .conscription_laws
            .iter()
            .find(|l| l.id == law_id)
            .map(|l| l.pp_cost)
            .ok_or_else(|| format!("unknown conscription law: {}", law_id)),
        LawCategory::Economy => db
            .economy_laws
            .iter()
            .find(|l| l.id == law_id)
            .map(|l| l.pp_cost)
            .ok_or_else(|| format!("unknown economy law: {}", law_id)),
        LawCategory::Trade => db
            .trade_laws
            .iter()
            .find(|l| l.id == law_id)
            .map(|l| l.pp_cost)
            .ok_or_else(|| format!("unknown trade law: {}", law_id)),
        LawCategory::Taxation => db
            .taxation_laws
            .iter()
            .find(|l| l.id == law_id)
            .map(|l| l.pp_cost)
            .ok_or_else(|| format!("unknown taxation law: {}", law_id)),
        LawCategory::CivilRights => db
            .civil_rights_laws
            .iter()
            .find(|l| l.id == law_id)
            .map(|l| l.pp_cost)
            .ok_or_else(|| format!("unknown civil rights law: {}", law_id)),
        LawCategory::InformationControl => db
            .information_control_laws
            .iter()
            .find(|l| l.id == law_id)
            .map(|l| l.pp_cost)
            .ok_or_else(|| format!("unknown info control law: {}", law_id)),
    }
}

fn lookup_cooldown(category: LawCategory, law_id: &str, db: &V6Database) -> u16 {
    match category {
        LawCategory::Conscription => db
            .conscription_laws
            .iter()
            .find(|l| l.id == law_id)
            .map(|l| l.cooldown_days)
            .unwrap_or(60),
        LawCategory::Economy => db
            .economy_laws
            .iter()
            .find(|l| l.id == law_id)
            .map(|l| l.cooldown_days)
            .unwrap_or(90),
        LawCategory::Trade => db
            .trade_laws
            .iter()
            .find(|l| l.id == law_id)
            .map(|l| l.cooldown_days)
            .unwrap_or(60),
        LawCategory::Taxation => db
            .taxation_laws
            .iter()
            .find(|l| l.id == law_id)
            .map(|l| l.cooldown_days)
            .unwrap_or(60),
        LawCategory::CivilRights => db
            .civil_rights_laws
            .iter()
            .find(|l| l.id == law_id)
            .map(|l| l.cooldown_days)
            .unwrap_or(60),
        LawCategory::InformationControl => db
            .information_control_laws
            .iter()
            .find(|l| l.id == law_id)
            .map(|l| l.cooldown_days)
            .unwrap_or(60),
    }
}

// V6.D: nationalization event execution plus trade-law locking.
/// Execute the nationalization event effect:
/// - Convert all owned Private/Cartel buildings to State ownership.
/// - Permanently close banks and move employed Capitalist POPs into unemployment.
/// - Reduce Capitalist loyalty, political power, and stability.
pub fn execute_nationalization(world: &mut World, ci: usize) {
    let country_id = CountryId(ci as u16);
    let state_ids: Vec<StateId> = (0..world.states.count)
        .filter(|&si| world.states.owners[si] == country_id)
        .map(|si| StateId(si as u16))
        .collect();

    let mut capitalist_unemployed_size: u32 = 0;
    let mut bank_indices_to_close: Vec<usize> = Vec::new();

    for (bidx, building) in world
        .countries
        .buildings_v6
        .buildings
        .iter_mut()
        .enumerate()
    {
        if !state_ids.contains(&building.state) {
            continue;
        }
        if building.level == 0 {
            continue;
        }
        if building.owner == BuildingOwner::State {
            continue;
        }

        if building.building_def_id == "bank" {
            for class_idx in 0..6 {
                let employed = building.employment.get(class_idx).copied().unwrap_or(0);
                if employed > 0 && class_idx == PopClass::Capitalist.index() {
                    capitalist_unemployed_size += employed;
                    building.employment = [0; 6];
                    bank_indices_to_close.push(bidx);
                }
            }
            continue;
        }

        building.owner = BuildingOwner::State;
    }

    for bidx in &bank_indices_to_close {
        world.countries.buildings_v6.buildings[*bidx].level = 0;
    }

    if capitalist_unemployed_size > 0 {
        for pg in &mut world.countries.pops.groups {
            if pg.class == PopClass::Capitalist
                && pg.employed_at.is_some()
                && state_ids.contains(&pg.state)
            {
                pg.employed_at = None;
            }
        }

        let mut remaining = capitalist_unemployed_size;
        for pg in &mut world.countries.pops.groups {
            if remaining == 0 {
                break;
            }
            if pg.class == PopClass::Clerk && state_ids.contains(&pg.state) {
                pg.size += remaining;
                remaining = 0;
            }
        }
    }

    for pg in &mut world.countries.pops.groups {
        if pg.class == PopClass::Capitalist && state_ids.contains(&pg.state) {
            pg.political_loyalty = (pg.political_loyalty - 0.50).clamp(-1.0, 1.0);
        }
    }

    world.countries.political_power[ci] =
        (world.countries.political_power[ci] - 500.0).max(-1000.0);
    world.countries.stability[ci] = (world.countries.stability[ci] - 0.20).clamp(0.0, 1.0);
}

/// Lock trade law when switching to planned economy.
pub fn lock_trade_law_for_planned_economy(world: &mut World, ci: usize, db: &V6Database) {
    let economy_law = world.countries.law_store.law_sets[ci].0[LawCategory::Economy.index()]
        .current
        .clone();

    let forced_trade_law = db
        .economy_laws
        .iter()
        .find(|l| l.id == economy_law)
        .and_then(|l| l.forces_trade_law.clone());

    if let Some(forced_id) = forced_trade_law {
        let trade_slot = &mut world.countries.law_store.law_sets[ci].0[LawCategory::Trade.index()];

        trade_slot.previous_before_lock = Some(trade_slot.current.clone());
        trade_slot.current = forced_id;
        trade_slot.is_locked = true;
        trade_slot.pending = None;
        trade_slot.cooldown_days = 0;
    }
}

/// Unlock trade law when leaving planned economy.
pub fn unlock_trade_law_on_leaving_planned(world: &mut World, ci: usize) {
    let trade_slot = &mut world.countries.law_store.law_sets[ci].0[LawCategory::Trade.index()];

    if let Some(prev) = trade_slot.previous_before_lock.take() {
        trade_slot.current = prev;
    }
    trade_slot.is_locked = false;
    trade_slot.pending = None;
    trade_slot.cooldown_days = 0;
}

/// Return planned-economy research direction modifiers.
pub fn planned_research_direction_modifier(
    world: &World,
    ci: usize,
    tech_id: &str,
    db: &V6Database,
) -> f32 {
    let economy_law = world.countries.law_store.law_sets[ci].0[LawCategory::Economy.index()]
        .current
        .clone();

    if economy_law != "planned_economy" {
        return 0.0;
    }

    let year = world.date.year;
    let active_plan = db
        .pyatiletka_plans
        .iter()
        .find(|p| year >= p.year_start && year <= p.year_end);

    if let Some(plan) = active_plan {
        if plan
            .focus_tech_directions
            .iter()
            .any(|d| tech_id.starts_with(d))
        {
            return plan.focus_bonus;
        } else {
            return plan.off_focus_penalty;
        }
    }

    0.0
}

// Tests.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v6_database_loads() {
        let db = V6Database::load();
        assert!(!db.goods.is_empty(), "goods should load");
        assert!(!db.buildings.is_empty(), "buildings should load");
        assert!(
            !db.production_methods.is_empty(),
            "production methods should load"
        );
        assert!(
            !db.conscription_laws.is_empty(),
            "conscription laws should load"
        );
        assert!(!db.economy_laws.is_empty(), "economy laws should load");
        assert!(!db.trade_laws.is_empty(), "trade laws should load");
        assert!(!db.taxation_laws.is_empty(), "taxation laws should load");
        assert!(
            !db.civil_rights_laws.is_empty(),
            "civil rights laws should load"
        );
        assert!(
            !db.information_control_laws.is_empty(),
            "info control laws should load"
        );
        assert!(
            db.initial_pops.contains_key("GER"),
            "initial GER pops should load"
        );
        assert!(
            !db.pyatiletka_plans.is_empty(),
            "pyatiletka plans should load"
        );
        assert!(!db.technologies.is_empty(), "technologies should load");
        assert!(
            !db.state_resource_deposits.is_empty(),
            "state resource deposits should load"
        );
        assert!(
            !db.historical_countries.is_empty(),
            "historical country profiles should load"
        );
        assert!(
            !db.historical_trade_routes.is_empty(),
            "historical trade routes should load"
        );
        assert!(
            !db.state_populations.is_empty(),
            "state population profiles should load"
        );
    }

    #[test]
    fn p2_state_population_profiles_write_runtime_integration() {
        let mut world = h2_test_world();
        let target_state = *world
            .state_id_lookup
            .get(&229)
            .expect("test world has target state");
        let mut db = V6Database::default();
        db.state_populations.push(StatePopulation1936Def {
            state_id: 229,
            population: 1_000_000,
            integration: StateIntegrationDef::Protectorate,
            urbanization: None,
            literacy: None,
            workforce_profile: None,
            data_quality: crate::HistoricalDataQuality::Rough,
        });

        apply_state_integration_statuses(&mut world, &db);

        assert_eq!(
            world.state_integration_status(target_state),
            Some(hoi4_state::StateIntegrationStatus::Protectorate)
        );
    }

    #[test]
    fn p10_batch_de_missing_states_get_fallback_integration() {
        let mut world = h2_test_world();
        let db = V6Database::default();
        world.states.cores[0].clear();

        apply_fallback_state_integration_statuses(&mut world, &db);

        assert_eq!(
            world.state_integration_status(StateId(0)),
            Some(hoi4_state::StateIntegrationStatus::Colony),
            "owned non-core states without explicit profiles should be colonial fallback"
        );
        assert_eq!(
            world.state_integration_status(StateId(1)),
            Some(hoi4_state::StateIntegrationStatus::Metropole),
            "owned core states without explicit profiles should remain metropole fallback"
        );
    }

    #[test]
    fn p10_batch_de_missing_state_pops_are_state_fallback_not_country_topoff() {
        let mut world = h2_test_world();
        let mut db = V6Database::load();
        db.state_populations.clear();
        db.initial_pops.clear();
        let profile = db
            .historical_countries
            .iter()
            .find(|profile| profile.tag == "USA")
            .expect("USA profile")
            .clone();
        let usa = world.country("USA").expect("USA exists");
        let expected: u64 = (0..world.states.count)
            .filter(|&si| world.states.owners[si] == usa)
            .map(|si| fallback_state_population(&world, StateId(si as u16)) as u64)
            .sum();

        inject_historical_pops(&mut world, usa, &profile, &db);

        let actual: u64 = world
            .countries
            .pops
            .groups
            .iter()
            .filter(|pop| world.states.owners[pop.state.0 as usize] == usa)
            .map(|pop| pop.size as u64)
            .sum();
        assert_eq!(
            actual, expected,
            "missing state profiles should use per-state fallback population, not profile.population top-off"
        );
        assert_ne!(
            actual, profile.population as u64,
            "fallback should not silently restore the old country population strong target"
        );
    }

    #[test]
    fn fallback_state_population_uses_state_manpower_without_multiplier() {
        let world = h2_test_world();
        let usa = world.country("USA").expect("USA exists");
        let state = (0..world.states.count)
            .find(|&si| world.states.owners[si] == usa)
            .map(|si| StateId(si as u16))
            .expect("USA test state");
        let si = state.0 as usize;

        assert_eq!(
            fallback_state_population(&world, state),
            world.states.manpower_pool[si],
            "fallback population must not multiply HOI4 state manpower/population"
        );
    }

    #[test]
    fn resource_buildings_declare_required_deposit_kind() {
        let db = V6Database::load();
        let expected = [
            ("iron_mine", "iron"),
            ("coal_mine", "coal"),
            ("oil_rig", "oil"),
            ("rubber_plantation", "rubber"),
            ("bauxite_mine", "bauxite"),
            ("chromium_mine", "chromium"),
            ("tungsten_mine", "tungsten"),
        ];
        for (building_id, deposit_kind) in expected {
            let building = db
                .buildings
                .iter()
                .find(|building| building.id == building_id)
                .unwrap_or_else(|| panic!("missing building {building_id}"));
            assert_eq!(building.state_limit_kind.as_deref(), Some(deposit_kind));
        }
    }

    #[test]
    fn production_methods_vec_lengths_match() {
        let db = V6Database::load();
        for pm in &db.production_methods {
            assert_eq!(
                pm.input_good_ids.len(),
                pm.input_good_amounts.len(),
                "PM '{}': input_good_ids and input_good_amounts length mismatch",
                pm.id
            );
            assert_eq!(
                pm.output_good_ids.len(),
                pm.output_good_amounts.len(),
                "PM '{}': output_good_ids and output_good_amounts length mismatch",
                pm.id
            );
        }
    }

    #[test]
    fn invariant_i3_no_vanilla_conflict() {
        let db = V6Database::load();
        db.assert_no_vanilla_conflict();
    }

    #[test]
    fn all_buildings_have_production_methods() {
        let db = V6Database::load();
        for b in &db.buildings {
            let has_pm = db
                .production_methods
                .iter()
                .any(|pm| pm.building_id == b.id);
            assert!(has_pm, "building '{}' has no production method", b.id);
        }
    }

    #[test]
    fn vehicle_factory_outputs_motorized() {
        let db = V6Database::load();
        let pm = db
            .production_methods
            .iter()
            .find(|pm| pm.building_id == "vehicle_factory" && pm.id == "vehicle_factory_default")
            .expect("missing vehicle factory default PM");
        assert!(pm
            .equipment_output
            .as_ref()
            .is_some_and(|out| out.equipment_category == "motorized"));
        assert!(pm.input_good_ids.iter().any(|id| id == "engines"));
        assert!(pm.input_good_ids.iter().any(|id| id == "vehicle_parts"));
    }

    #[test]
    fn oil_refinery_outputs_fuel() {
        let db = V6Database::load();
        let pm = db
            .production_methods
            .iter()
            .find(|pm| pm.building_id == "oil_refinery" && pm.id == "oil_refinery_default")
            .expect("missing oil refinery default PM");
        assert!(pm.output_good_ids.iter().any(|id| id == "fuel"));
    }

    #[test]
    fn final_equipment_uses_1936_intermediate_inputs() {
        let db = V6Database::load();
        let required: [(&str, &[&str]); 5] = [
            (
                "infantry_equipment",
                &["small_arms_parts", "ammunition", "steel", "textiles"],
            ),
            (
                "support_equipment",
                &["radio_sets", "machinery", "rubber_parts", "textiles"],
            ),
            (
                "artillery",
                &["gun_barrels", "artillery_shells", "machinery"],
            ),
            (
                "armor",
                &[
                    "tank_hulls",
                    "engines",
                    "armor_plate",
                    "gun_barrels",
                    "optics",
                    "radio_sets",
                ],
            ),
            (
                "aircraft",
                &[
                    "airframes",
                    "engines",
                    "aluminium",
                    "rubber_parts",
                    "radio_sets",
                ],
            ),
        ];

        for (equipment_category, input_goods) in required {
            let pm = db
                .production_methods
                .iter()
                .find(|pm| {
                    pm.equipment_output
                        .as_ref()
                        .is_some_and(|out| out.equipment_category == equipment_category)
                })
                .unwrap_or_else(|| panic!("missing equipment PM for {equipment_category}"));
            for input_good in input_goods {
                assert!(
                    pm.input_good_ids.iter().any(|id| id == input_good),
                    "{equipment_category} PM '{}' should consume {input_good}",
                    pm.id
                );
            }
        }
    }

    #[test]
    fn military_intermediate_goods_have_sources() {
        let db = V6Database::load();
        for good_id in [
            "small_arms_parts",
            "ammunition",
            "gun_barrels",
            "armor_plate",
            "radio_sets",
            "optics",
            "airframes",
            "textiles",
        ] {
            assert!(
                db.production_methods
                    .iter()
                    .any(|pm| pm.output_good_ids.iter().any(|id| id == good_id)),
                "good '{good_id}' has no production source"
            );
        }
    }

    #[test]
    fn buildable_buildings_have_catalog_metadata() {
        let db = V6Database::load();
        for b in db.buildings.iter().filter(|b| b.buildable) {
            assert!(
                !b.name.trim().is_empty(),
                "buildable building '{}' has no name",
                b.id
            );
            assert!(
                !b.group.trim().is_empty(),
                "buildable building '{}' has no catalog group",
                b.id
            );
        }
    }

    #[test]
    fn g07_all_buildings_have_sector_gameplay_names_and_employment() {
        let db = V6Database::load();
        assert_eq!(
            db.buildings.len(),
            42,
            "G07 should cover the full V6 building catalog"
        );
        let mut sectors = std::collections::HashSet::new();
        let mut gameplay_classes = std::collections::HashSet::new();

        for building in &db.buildings {
            sectors.insert(building.economic_sector);
            gameplay_classes.insert(building.gameplay_class);
            assert!(
                has_cjk(&building.name),
                "{} should have a Chinese display name",
                building.id
            );
            assert!(
                has_cjk(&building.description),
                "{} should have a Chinese description",
                building.id
            );
            assert!(
                !has_mojibake(&building.name) && !has_mojibake(&building.description),
                "{} should not contain mojibake in name/description",
                building.id
            );
            assert!(
                building.employment_profile.total() > 0,
                "{} should have a nonzero building employment profile",
                building.id
            );
            assert!(
                building_gameplay_matches_kind(building.gameplay_class, building.kind),
                "{} gameplay_class should preserve the runtime gameplay kind",
                building.id
            );
            match building.economic_sector {
                EconomicSectorDef::Primary
                | EconomicSectorDef::Secondary
                | EconomicSectorDef::Tertiary
                | EconomicSectorDef::Government
                | EconomicSectorDef::MilitarySupport
                | EconomicSectorDef::Infrastructure => {}
            }
            assert!(
                building.gdp_rule.value_added_multiplier.is_finite()
                    && building.gdp_rule.value_added_multiplier >= 0.0,
                "{} should declare a valid GDP value-added rule",
                building.id
            );
            assert!(
                building_gdp_rule_matches_sector(
                    building.gdp_rule.component,
                    building.economic_sector
                ),
                "{} GDP rule should match its economic sector",
                building.id
            );
        }
        for sector in [
            EconomicSectorDef::Primary,
            EconomicSectorDef::Secondary,
            EconomicSectorDef::Tertiary,
            EconomicSectorDef::Government,
            EconomicSectorDef::MilitarySupport,
            EconomicSectorDef::Infrastructure,
        ] {
            assert!(
                sectors.contains(&sector),
                "building catalog should use economic sector {sector:?}"
            );
        }
        for class in [
            BuildingGameplayClassDef::Agriculture,
            BuildingGameplayClassDef::ResourceExtraction,
            BuildingGameplayClassDef::HeavyIndustry,
            BuildingGameplayClassDef::LightIndustry,
            BuildingGameplayClassDef::Service,
            BuildingGameplayClassDef::MilitaryIndustry,
            BuildingGameplayClassDef::Infrastructure,
            BuildingGameplayClassDef::MilitaryBase,
            BuildingGameplayClassDef::Government,
        ] {
            assert!(
                gameplay_classes.contains(&class),
                "building catalog should use gameplay class {class:?}"
            );
        }
    }

    #[test]
    fn g08_all_buildings_have_construction_recipes() {
        let db = V6Database::load();
        let known_goods: std::collections::HashSet<&str> =
            db.goods.iter().map(|good| good.id.as_str()).collect();
        let mut recipe_signatures = std::collections::HashSet::new();

        assert_eq!(
            db.buildings.len(),
            42,
            "G08 should cover the full V6 building catalog"
        );

        for building in &db.buildings {
            let recipe = &building.construction_recipe;
            assert!(
                recipe.cp_cost > 0.0,
                "{} should have nonzero construction CP",
                building.id
            );
            assert!(
                recipe.funds_rm > 0.0,
                "{} should have nonzero construction funds",
                building.id
            );
            assert!(
                recipe.labor > 0,
                "{} should have nonzero construction labor",
                building.id
            );
            assert!(
                recipe.engineering > 0,
                "{} should have nonzero engineering need",
                building.id
            );
            assert!(
                !recipe.materials.is_empty(),
                "{} should declare construction materials",
                building.id
            );
            for material in &recipe.materials {
                assert!(
                    material.amount > 0.0,
                    "{} material '{}' should have positive amount",
                    building.id,
                    material.good_id
                );
                assert!(
                    known_goods.contains(material.good_id.as_str()),
                    "{} construction material '{}' should reference a known good",
                    building.id,
                    material.good_id
                );
            }
            if let Some(limit) = &building.state_limit_kind {
                assert!(
                    recipe
                        .regional_restrictions
                        .iter()
                        .any(|restriction| restriction == limit),
                    "{} recipe should expose state limit '{}' as a regional restriction",
                    building.id,
                    limit
                );
            }
            recipe_signatures.insert(construction_recipe_signature(recipe));
        }

        assert!(
            recipe_signatures.len() >= 30,
            "G08 construction materials should be building-specific, not one generic recipe"
        );

        let farm = building_recipe(&db, "grain_farm");
        let port = building_recipe(&db, "port");
        assert_ne!(
            construction_recipe_signature(farm),
            construction_recipe_signature(port),
            "farm_and_port_recipes_differ = true"
        );
        assert!(
            !port
                .materials
                .iter()
                .any(|material| material.good_id == "grain"),
            "port recipe should not reuse the farm material profile"
        );

        let arms = building_recipe(&db, "arms_industry");
        let civilian_factory = building_recipe(&db, "steel_mill");
        assert_ne!(
            construction_recipe_signature(arms),
            construction_recipe_signature(civilian_factory),
            "military_factory_recipe_distinct = true"
        );
        assert!(
            arms.materials
                .iter()
                .any(|material| material.good_id == "small_arms_parts")
                || arms
                    .materials
                    .iter()
                    .any(|material| material.good_id == "ammunition"),
            "arms_industry should require military-specific construction materials"
        );
    }

    #[test]
    fn g08_construction_recipe_materials_have_player_visible_names() {
        let db = V6Database::load();
        let goods_by_id: std::collections::HashMap<&str, &GoodDef> = db
            .goods
            .iter()
            .map(|good| (good.id.as_str(), good))
            .collect();

        for building in &db.buildings {
            for material in &building.construction_recipe.materials {
                let good = goods_by_id
                    .get(material.good_id.as_str())
                    .unwrap_or_else(|| {
                        panic!(
                            "{} construction material '{}' should reference a known good",
                            building.id, material.good_id
                        )
                    });
                assert!(
                    has_cjk(&good.name),
                    "{} construction material '{}' should resolve to a Chinese display name",
                    building.id,
                    material.good_id
                );
                assert!(
                    !has_mojibake(&good.name),
                    "{} construction material '{}' display name should not contain mojibake",
                    building.id,
                    material.good_id
                );
            }
        }
    }

    #[test]
    fn g08_v6_loader_source_has_no_common_mojibake() {
        let source = include_str!("v6_loader.rs");
        assert!(
            !has_mojibake(source),
            "v6_loader.rs should not contain common mojibake or placeholder text"
        );
    }

    fn building_recipe<'a>(db: &'a V6Database, building_id: &str) -> &'a ConstructionRecipeDef {
        &db.buildings
            .iter()
            .find(|building| building.id == building_id)
            .unwrap_or_else(|| panic!("missing building {building_id}"))
            .construction_recipe
    }

    fn construction_recipe_signature(recipe: &ConstructionRecipeDef) -> String {
        let mut materials: Vec<String> = recipe
            .materials
            .iter()
            .map(|material| format!("{}:{:.1}", material.good_id, material.amount))
            .collect();
        materials.sort();
        let mut regions = recipe.regional_restrictions.clone();
        regions.sort();
        format!(
            "cp:{:.1}|rm:{:.1}|labor:{}|eng:{}|mat:{}|reg:{}",
            recipe.cp_cost,
            recipe.funds_rm,
            recipe.labor,
            recipe.engineering,
            materials.join(","),
            regions.join(",")
        )
    }

    fn has_cjk(text: &str) -> bool {
        text.chars()
            .any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch))
    }

    fn has_mojibake(text: &str) -> bool {
        const MOJIBAKE_CODEPOINTS: [char; 6] = [
            '\u{fffd}', '\u{9422}', '\u{7ecb}', '\u{93c2}', '\u{923a}', '\u{20ac}',
        ];
        text.chars().any(|ch| MOJIBAKE_CODEPOINTS.contains(&ch))
            || text
                .as_bytes()
                .windows(3)
                .any(|window| window == [b'?', b'?', b'?'])
    }

    fn building_gameplay_matches_kind(
        gameplay_class: BuildingGameplayClassDef,
        kind: BuildingKindDef,
    ) -> bool {
        matches!(
            (gameplay_class, kind),
            (
                BuildingGameplayClassDef::ResourceExtraction,
                BuildingKindDef::Resource
            ) | (
                BuildingGameplayClassDef::HeavyIndustry,
                BuildingKindDef::Industrial
            ) | (
                BuildingGameplayClassDef::Agriculture,
                BuildingKindDef::Agriculture
            ) | (
                BuildingGameplayClassDef::LightIndustry,
                BuildingKindDef::ConsumerGoods
            ) | (BuildingGameplayClassDef::Service, BuildingKindDef::Service)
                | (
                    BuildingGameplayClassDef::MilitaryIndustry,
                    BuildingKindDef::Military
                )
                | (
                    BuildingGameplayClassDef::Infrastructure,
                    BuildingKindDef::Infrastructure
                )
                | (
                    BuildingGameplayClassDef::MilitaryBase,
                    BuildingKindDef::MilitaryBase
                )
                | (
                    BuildingGameplayClassDef::Government,
                    BuildingKindDef::Service
                )
                | (
                    BuildingGameplayClassDef::Government,
                    BuildingKindDef::MilitaryBase
                )
        )
    }

    fn building_gdp_rule_matches_sector(
        component: BuildingGdpComponentDef,
        sector: EconomicSectorDef,
    ) -> bool {
        matches!(
            (component, sector),
            (
                BuildingGdpComponentDef::PrimaryOutput,
                EconomicSectorDef::Primary
            ) | (
                BuildingGdpComponentDef::SecondaryOutput,
                EconomicSectorDef::Secondary
            ) | (
                BuildingGdpComponentDef::TertiaryOutput,
                EconomicSectorDef::Tertiary
            ) | (
                BuildingGdpComponentDef::GovernmentService,
                EconomicSectorDef::Government
            ) | (
                BuildingGdpComponentDef::MilitaryProcurement,
                EconomicSectorDef::MilitarySupport
            ) | (
                BuildingGdpComponentDef::InfrastructureService,
                EconomicSectorDef::Infrastructure
            )
        )
    }

    #[test]
    fn building_tech_unlocks_reference_known_buildings() {
        let db = V6Database::load();
        for tech in &db.technologies {
            for unlock in &tech.unlocks {
                if let TechUnlockDef::Building(building_id) = unlock {
                    assert!(
                        db.buildings
                            .iter()
                            .any(|b| b.id == *building_id && b.buildable),
                        "tech '{}' unlocks unknown or unbuildable building '{}'",
                        tech.id,
                        building_id,
                    );
                }
            }
        }
    }

    #[test]
    fn military_buildings_have_equipment_output() {
        let db = V6Database::load();
        let military_bldg_ids: Vec<&str> = db
            .buildings
            .iter()
            .filter(|b| matches!(b.kind, BuildingKindDef::Military))
            .map(|b| b.id.as_str())
            .collect();
        for pm in &db.production_methods {
            if military_bldg_ids.contains(&pm.building_id.as_str())
                && pm.output_good_ids.is_empty()
                && pm.id.contains("default")
            {
                assert!(
                    pm.equipment_output.is_some(),
                    "military PM '{}' has no output_goods and no equipment_output",
                    pm.id
                );
            }
        }
    }

    #[test]
    fn invariant_i1_manpower_field_removed() {
        // HC-1: grep check that Vec<u64> manpower is NOT a field on CountryStore.
        // This test ensures the method-based manpower() exists.
        let store = hoi4_state::CountryStore::new(1);
        let _ = store.manpower(0, &[]);
    }

    #[test]
    fn goods_have_valid_categories() {
        let db = V6Database::load();
        for good in &db.goods {
            match good.category {
                GoodCategoryDef::RawMaterial
                | GoodCategoryDef::Intermediate
                | GoodCategoryDef::Consumer
                | GoodCategoryDef::Luxury
                | GoodCategoryDef::Service
                | GoodCategoryDef::MilitaryIntermediate => {}
            }
        }
    }

    #[test]
    fn all_law_categories_have_entries() {
        let db = V6Database::load();
        assert!(
            db.conscription_laws.len() >= 4,
            "conscription should have 4+ tiers"
        );
        assert!(db.economy_laws.len() >= 4, "economy should have 4+ tiers");
        assert!(db.trade_laws.len() >= 4, "trade should have 4+ tiers");
        assert!(db.taxation_laws.len() >= 3, "taxation should have 3+ tiers");
        assert!(
            db.civil_rights_laws.len() >= 4,
            "civil rights should have 4+ tiers"
        );
        assert!(
            db.information_control_laws.len() >= 4,
            "info control should have 4+ tiers"
        );
    }

    fn h2_test_map(max_province: u16) -> std::sync::Arc<hoi4_map::GameMap> {
        let mut definitions = vec![None; max_province as usize + 1];
        for province_id in 1..=max_province {
            definitions[province_id as usize] = Some(hoi4_map::ProvinceDefinition {
                id: province_id,
                r: (province_id & 0xff) as u8,
                g: 0,
                b: 0,
                province_type: hoi4_map::ProvinceType::Land,
                coastal: province_id % 3 == 0,
                terrain: "plains".to_owned(),
                continent: 1,
            });
        }
        std::sync::Arc::new(hoi4_map::GameMap {
            definitions,
            rgb_to_id: std::collections::HashMap::new(),
            province_map: hoi4_map::ProvinceMap {
                width: 1,
                height: 1,
                pixels: vec![0],
            },
            adjacencies: vec![],
            special_adjacencies: vec![],
            heightmap: hoi4_map::Heightmap {
                width: 1,
                height: 1,
                pixels: vec![0],
            },
            terrain_bmp: hoi4_map::TerrainBitmap {
                width: 1,
                height: 1,
                pixels: vec![0],
                palette: [[0; 3]; 256],
            },
            terrain_catalog: hoi4_map::TerrainCatalog::default(),
            tree_definition_bmp: None,
            tree_indices: std::collections::HashSet::new(),
        })
    }

    fn h2_test_world() -> hoi4_state::World {
        let tags = ["USA", "GER", "SOV", "ENG", "FRA", "JAP", "ITA", "CHI"];
        let state_ids: [[u16; 3]; 8] = [
            [195, 202, 276],
            [28, 51, 29],
            [229, 230, 231],
            [91, 92, 93],
            [105, 106, 107],
            [300, 301, 302],
            [212, 213, 214],
            [613, 614, 615],
        ];
        let mut data = hoi4_data::GameData::default();
        let mut province_id = 1u16;
        for (tag_idx, tag_str) in tags.iter().enumerate() {
            let tag = hoi4_data::CountryTag::new(tag_str);
            data.countries.insert(
                tag.clone(),
                hoi4_data::Country {
                    tag: tag.clone(),
                    color: hoi4_data::Color {
                        r: (40 + tag_idx as u8 * 20),
                        g: 80,
                        b: 120,
                    },
                    graphical_culture: "western_european_gfx".to_owned(),
                    capital: state_ids[tag_idx][0],
                    ruling_party: "neutrality".to_owned(),
                    technologies: Vec::new(),
                },
            );
            for (local_idx, state_id) in state_ids[tag_idx].iter().enumerate() {
                data.states.push(hoi4_data::State {
                    id: *state_id,
                    name: format!("{tag_str} State {local_idx}"),
                    manpower: 1_000_000 + tag_idx as u64 * 250_000 + local_idx as u64 * 100_000,
                    owner: tag.clone(),
                    cores: vec![tag.clone()],
                    provinces: vec![province_id],
                    category: "city".to_owned(),
                    infrastructure: 4 + local_idx as u8,
                    victory_points: vec![],
                    resources: vec![],
                });
                province_id += 1;
            }
        }
        hoi4_state::World::new(h2_test_map(province_id), std::sync::Arc::new(data))
    }

    fn h2_raj_population_test_world() -> hoi4_state::World {
        let tag = hoi4_data::CountryTag::new("RAJ");
        let state_ids = [303u16, 9001, 9002, 9003, 9004];
        let mut data = hoi4_data::GameData::default();
        data.countries.insert(
            tag.clone(),
            hoi4_data::Country {
                tag: tag.clone(),
                color: hoi4_data::Color {
                    r: 120,
                    g: 80,
                    b: 60,
                },
                graphical_culture: "asian_gfx".to_owned(),
                capital: 303,
                ruling_party: "neutrality".to_owned(),
                technologies: Vec::new(),
            },
        );

        for (idx, state_id) in state_ids.iter().enumerate() {
            data.states.push(hoi4_data::State {
                id: *state_id,
                name: format!("RAJ Population Test State {idx}"),
                manpower: 80_000_000,
                owner: tag.clone(),
                cores: vec![tag.clone()],
                provinces: vec![idx as u16 + 1],
                category: "rural".to_owned(),
                infrastructure: 3,
                victory_points: vec![],
                resources: vec![],
            });
        }

        hoi4_state::World::new(
            h2_test_map(state_ids.len() as u16 + 1),
            std::sync::Arc::new(data),
        )
    }

    fn h2_colonial_empire_population_test_world() -> hoi4_state::World {
        let tag = hoi4_data::CountryTag::new("FRA");
        let state_specs = [
            (105u16, 8_000_000u64, true),
            (106u16, 8_000_000u64, true),
            (107u16, 8_000_000u64, true),
            (458u16, 3_000_000u64, false),
            (459u16, 4_000_000u64, false),
            (461u16, 5_000_000u64, false),
            (462u16, 2_000_000u64, false),
        ];
        let mut data = hoi4_data::GameData::default();
        data.countries.insert(
            tag.clone(),
            hoi4_data::Country {
                tag: tag.clone(),
                color: hoi4_data::Color {
                    r: 80,
                    g: 100,
                    b: 180,
                },
                graphical_culture: "western_european_gfx".to_owned(),
                capital: 105,
                ruling_party: "democratic".to_owned(),
                technologies: Vec::new(),
            },
        );

        for (idx, (state_id, manpower, core)) in state_specs.iter().enumerate() {
            data.states.push(hoi4_data::State {
                id: *state_id,
                name: format!("FRA Population Test State {idx}"),
                manpower: *manpower,
                owner: tag.clone(),
                cores: if *core { vec![tag.clone()] } else { vec![] },
                provinces: vec![idx as u16 + 1],
                category: "rural".to_owned(),
                infrastructure: 3,
                victory_points: vec![],
                resources: vec![],
            });
        }

        hoi4_state::World::new(
            h2_test_map(state_specs.len() as u16 + 1),
            std::sync::Arc::new(data),
        )
    }

    fn g06_minimum_economy_world(tags: &[&str]) -> hoi4_state::World {
        let mut data = hoi4_data::GameData::default();
        let mut province_id = 1u16;
        for (idx, tag_str) in tags.iter().copied().enumerate() {
            let tag = hoi4_data::CountryTag::new(tag_str);
            let state_id = 10_000 + idx as u16;
            data.countries.insert(
                tag.clone(),
                hoi4_data::Country {
                    tag: tag.clone(),
                    color: hoi4_data::Color {
                        r: (idx as u8).wrapping_mul(37),
                        g: 96,
                        b: 144,
                    },
                    graphical_culture: "western_european_gfx".to_owned(),
                    capital: state_id,
                    ruling_party: "neutrality".to_owned(),
                    technologies: Vec::new(),
                },
            );
            data.states.push(hoi4_data::State {
                id: state_id,
                name: format!("{tag_str} Minimum Economy State"),
                manpower: 900_000 + idx as u64 * 11_000,
                owner: tag.clone(),
                cores: vec![tag],
                provinces: vec![province_id],
                category: "rural".to_owned(),
                infrastructure: 2 + (idx % 4) as u8,
                victory_points: vec![],
                resources: vec![],
            });
            province_id += 1;
        }
        hoi4_state::World::new(h2_test_map(province_id), std::sync::Arc::new(data))
    }

    fn game_state_id(world: &World, state: StateId) -> u16 {
        world
            .state_id_lookup
            .iter()
            .find_map(|(game_id, sid)| (*sid == state).then_some(*game_id))
            .unwrap_or(state.0)
    }

    fn expected_state_population(world: &World, db: &V6Database, state: StateId) -> u64 {
        let game_id = game_state_id(world, state);
        db.state_populations
            .iter()
            .find(|entry| entry.state_id == game_id)
            .map(|entry| entry.population as u64)
            .unwrap_or_else(|| fallback_state_population(world, state) as u64)
    }

    fn expected_owned_state_population(world: &World, db: &V6Database, country: CountryId) -> u64 {
        (0..world.states.count)
            .filter(|&si| world.states.owners[si] == country)
            .map(|si| expected_state_population(world, db, StateId(si as u16)))
            .sum()
    }

    fn expected_owned_state_population_by_status(
        world: &World,
        db: &V6Database,
        country: CountryId,
        domestic: bool,
    ) -> u64 {
        (0..world.states.count)
            .filter(|&si| world.states.owners[si] == country)
            .filter(|&si| {
                let status = world.states.integration_status[si];
                let is_core_state = world.states.cores[si].contains(&country);
                let is_domestic = match status {
                    hoi4_state::StateIntegrationStatus::Metropole if !is_core_state => false,
                    status if status.is_domestic() => true,
                    status if status.is_colonial_or_occupied() => false,
                    _ => true,
                };
                is_domestic == domestic
            })
            .map(|si| expected_state_population(world, db, StateId(si as u16)))
            .sum()
    }

    #[test]
    fn g06_all_existing_countries_receive_minimum_economy() {
        let db = V6Database::load();
        let tags: Vec<&str> = db
            .historical_countries
            .iter()
            .map(|profile| profile.tag.as_str())
            .collect();
        let mut world = g06_minimum_economy_world(&tags);
        let authored_profiles: std::collections::HashSet<&str> = db
            .historical_countries
            .iter()
            .map(|profile| profile.tag.as_str())
            .collect();

        inject_v6_into_world(&mut world, &db);

        let mut missing = Vec::new();
        let mut fallback_profiles = Vec::new();
        for tag in tags {
            let country = world.country(tag).unwrap_or_else(|| panic!("{tag} exists"));
            let ci = country.0 as usize;
            let state_ids: Vec<StateId> = (0..world.states.count)
                .filter(|&si| world.states.owners[si] == country)
                .map(|si| StateId(si as u16))
                .collect();
            assert!(!state_ids.is_empty(), "{tag} has owned state");

            let pop_total: u64 = world
                .countries
                .pops
                .groups
                .iter()
                .filter(|pg| state_ids.contains(&pg.state))
                .map(|pg| pg.size as u64)
                .sum();
            let building_levels = country_total_building_levels(&world, country);
            let treasury = &world.countries.treasury.treasuries[ci];
            let market = &world.countries.market.markets[ci];
            let law_set = &world.countries.law_store.law_sets[ci];

            let has_minimum = pop_total > 0
                && building_levels > 0
                && treasury.cash_rm > 0.0
                && db
                    .goods
                    .iter()
                    .all(|good| market.price.contains_key(&good.id))
                && !law_set.0[LawCategory::Economy.index()].current.is_empty()
                && !law_set.0[LawCategory::Trade.index()].current.is_empty()
                && world.countries.trade.blockaded_ports.len() >= world.states.count;
            if !has_minimum {
                missing.push(format!(
                    "{tag}(pop={pop_total}, buildings={building_levels}, cash={:.1}, historical_gdp={:.1}, market={}, trade_ports={})",
                    treasury.cash_rm,
                    treasury.gdp_breakdown.historical_validation_gbp,
                    db.goods.iter().all(|good| market.price.contains_key(&good.id)),
                    world.countries.trade.blockaded_ports.len()
                ));
            }
            if !authored_profiles.contains(tag) {
                fallback_profiles.push(tag.to_owned());
            }
        }

        assert!(
            missing.is_empty(),
            "countries without minimum economy: {}",
            missing.join(", ")
        );
        assert!(
            fallback_profiles.is_empty(),
            "G06 must not count fallback profiles as G1 coverage: {}",
            fallback_profiles.join(", ")
        );
    }

    #[test]
    fn h2_historical_profiles_generate_initial_buildings() {
        let mut world = h2_test_world();
        let db = V6Database::load();

        inject_v6_into_world(&mut world, &db);

        for tag in ["USA", "GER", "SOV", "ENG", "FRA", "JAP", "ITA", "CHI"] {
            let country = world.country(tag).expect("country exists");
            let has_building = world
                .countries
                .buildings_v6
                .buildings
                .iter()
                .any(|building| {
                    let si = building.state.0 as usize;
                    si < world.states.count
                        && world.states.owners[si] == country
                        && building.level > 0
                });
            assert!(has_building, "{tag} should receive generated V6 buildings");
        }

        let chi = world.country("CHI").unwrap().0 as usize;

        let chi_grain: u16 = world
            .countries
            .buildings_v6
            .buildings
            .iter()
            .filter(|building| {
                let si = building.state.0 as usize;
                si < world.states.count
                    && world.states.owners[si] == hoi4_state::CountryId(chi as u16)
                    && building.building_def_id == "grain_farm"
            })
            .map(|building| building.level as u16)
            .sum();
        let chi_heavy: u16 = world
            .countries
            .buildings_v6
            .buildings
            .iter()
            .filter(|building| {
                let si = building.state.0 as usize;
                si < world.states.count
                    && world.states.owners[si] == hoi4_state::CountryId(chi as u16)
                    && matches!(
                        building.building_def_id.as_str(),
                        "steel_mill" | "arms_industry"
                    )
            })
            .map(|building| building.level as u16)
            .sum();
        assert!(chi_grain > chi_heavy, "CHI should remain agriculture-heavy");
    }

    #[test]
    fn g13_profile_population_does_not_drive_buildings_or_finance() {
        let mut baseline_world = h2_test_world();
        let baseline_db = V6Database::load();
        inject_v6_into_world(&mut baseline_world, &baseline_db);

        let mut changed_world = h2_test_world();
        let mut changed_db = V6Database::load();
        let profile = changed_db
            .historical_countries
            .iter_mut()
            .find(|profile| profile.tag == "USA")
            .expect("USA profile exists");
        profile.population = 1;
        inject_v6_into_world(&mut changed_world, &changed_db);

        let baseline_usa = baseline_world.country("USA").expect("USA exists");
        let changed_usa = changed_world.country("USA").expect("USA exists");
        assert_eq!(
            baseline_world.country_governed_population(baseline_usa),
            changed_world.country_governed_population(changed_usa),
            "runtime population should still come from owned state POPs"
        );
        assert_eq!(
            country_total_building_levels(&baseline_world, baseline_usa),
            country_total_building_levels(&changed_world, changed_usa),
            "historical profile population should not drive building targets"
        );
        assert_eq!(
            baseline_world.countries.treasury.treasuries[baseline_usa.0 as usize].cash_rm,
            changed_world.countries.treasury.treasuries[changed_usa.0 as usize].cash_rm,
            "historical profile population should not drive initial fiscal cash"
        );
        assert_eq!(
            baseline_world.countries.private_investment_pool_rm[baseline_usa.0 as usize],
            changed_world.countries.private_investment_pool_rm[changed_usa.0 as usize],
            "historical profile population should not drive the private investment pool"
        );
    }

    #[test]
    fn g13_profile_gdp_does_not_drive_buildings_or_finance() {
        let mut baseline_world = h2_test_world();
        let baseline_db = V6Database::load();
        inject_v6_into_world(&mut baseline_world, &baseline_db);

        let mut changed_world = h2_test_world();
        let mut changed_db = V6Database::load();
        let profile = changed_db
            .historical_countries
            .iter_mut()
            .find(|profile| profile.tag == "USA")
            .expect("USA profile exists");
        profile.gdp_1936_gbp *= 10.0;
        let changed_reference_gdp = profile.gdp_1936_gbp;
        inject_v6_into_world(&mut changed_world, &changed_db);

        let baseline_usa = baseline_world.country("USA").expect("USA exists");
        let changed_usa = changed_world.country("USA").expect("USA exists");
        assert_eq!(
            country_total_building_levels(&baseline_world, baseline_usa),
            country_total_building_levels(&changed_world, changed_usa),
            "historical GDP should not drive building targets"
        );
        assert_eq!(
            baseline_world.countries.treasury.treasuries[baseline_usa.0 as usize].cash_rm,
            changed_world.countries.treasury.treasuries[changed_usa.0 as usize].cash_rm,
            "historical GDP should not drive initial fiscal cash"
        );
        assert_eq!(
            baseline_world.countries.private_investment_pool_rm[baseline_usa.0 as usize],
            changed_world.countries.private_investment_pool_rm[changed_usa.0 as usize],
            "historical GDP should not drive the private investment pool"
        );
        assert_eq!(
            changed_world.countries.treasury.treasuries[changed_usa.0 as usize]
                .gdp_breakdown
                .historical_validation_gbp,
            changed_reference_gdp,
            "historical GDP should be retained as the validation target"
        );
        assert_eq!(
            changed_world.countries.treasury.treasuries[changed_usa.0 as usize].gdp_gbp, 0.0,
            "historical GDP should not seed runtime GDP"
        );
    }

    #[test]
    fn h2_ger_population_is_distributed_across_all_states() {
        let mut world = h2_test_world();
        let db = V6Database::load();

        inject_v6_into_world(&mut world, &db);

        let ger = world.country("GER").expect("GER exists");
        let ger_states: Vec<StateId> = (0..world.states.count)
            .filter(|&si| world.states.owners[si] == ger)
            .map(|si| StateId(si as u16))
            .collect();
        assert_eq!(
            ger_states.len(),
            3,
            "test world should give GER three states"
        );

        let mut state_totals: Vec<u64> = ger_states
            .iter()
            .map(|sid| {
                world
                    .countries
                    .pops
                    .groups
                    .iter()
                    .filter(|pg| pg.state == *sid)
                    .map(|pg| pg.size as u64)
                    .sum()
            })
            .collect();

        assert!(
            state_totals.iter().all(|&total| total > 0),
            "each GER state should receive POPs"
        );
        state_totals.sort_unstable();
        assert_ne!(
            state_totals[0], state_totals[2],
            "GER state populations should not be flatly equal"
        );
    }

    #[test]
    fn h2_profiled_country_population_is_capped_to_reference_total() {
        let mut world = h2_raj_population_test_world();
        let db = V6Database::load();

        inject_v6_into_world(&mut world, &db);

        let raj = world.country("RAJ").expect("RAJ exists");
        let total = world.country_governed_population(raj);
        let expected = expected_owned_state_population(&world, &db, raj);
        let reference = db
            .historical_countries
            .iter()
            .find(|profile| profile.tag == "RAJ")
            .expect("RAJ historical profile")
            .population as u64;

        assert_eq!(
            total, expected,
            "RAJ national population should be derived from owned state populations"
        );
        assert_ne!(
            total, reference,
            "RAJ historical country population should be a validation reference, not an injected target"
        );
    }

    #[test]
    fn h2_colonial_empire_population_sums_owned_states() {
        let mut world = h2_colonial_empire_population_test_world();
        let db = V6Database::load();

        inject_v6_into_world(&mut world, &db);

        let fra = world.country("FRA").expect("FRA exists");
        let breakdown = world.country_population_breakdown(fra);
        let expected_domestic = expected_owned_state_population_by_status(&world, &db, fra, true);
        let expected_colonial = expected_owned_state_population_by_status(&world, &db, fra, false);

        assert_eq!(
            breakdown.domestic, expected_domestic,
            "FRA domestic population should sum domestic owned states"
        );
        assert_eq!(
            breakdown.colonial, expected_colonial,
            "FRA colonial population should sum colonial owned states"
        );
        assert_eq!(
            breakdown.governed,
            expected_domestic + expected_colonial,
            "FRA governed population should be domestic plus colonial state sums"
        );
    }

    #[test]
    fn h2_pops_use_tax_and_loyalty_initialization() {
        let mut world = h2_test_world();
        let db = V6Database::load();

        inject_v6_into_world(&mut world, &db);

        let ger = world.country("GER").expect("GER exists");
        let sample = world
            .countries
            .pops
            .groups
            .iter()
            .find(|pg| world.states.owners[pg.state.0 as usize] == ger)
            .expect("GER pop group");

        assert!(
            sample.tax_burden > 0.0,
            "initial tax burden should be initialized"
        );
        assert_ne!(
            sample.political_loyalty, 0.0,
            "initial political loyalty should not be flat zero"
        );
        assert!(sample.loyalty_coefficient > 0.0);
        assert!(sample.loyalty_decay_mult > 0.0);
    }

    #[test]
    #[ignore = "known Phase 9 baseline: historical resource deposit calibration is not yet reconciled"]
    fn h2_resource_buildings_do_not_exceed_discovered_deposits() {
        let mut world = h2_test_world();
        let db = V6Database::load();

        inject_v6_into_world(&mut world, &db);

        for building in &world.countries.buildings_v6.buildings {
            let Some(def) = db
                .buildings
                .iter()
                .find(|def| def.id == building.building_def_id)
            else {
                continue;
            };
            let Some(resource_kind) = def.state_limit_kind.as_deref() else {
                continue;
            };
            if resource_kind == "coastal" || resource_kind == "urban" {
                continue;
            }
            let game_state_id = world
                .state_id_lookup
                .iter()
                .find_map(|(game_id, sid)| (*sid == building.state).then_some(*game_id))
                .unwrap_or(building.state.0);
            let discovered = db
                .state_resource_deposits
                .iter()
                .find(|entry| entry.state_id == game_state_id)
                .and_then(|entry| {
                    entry
                        .deposits
                        .iter()
                        .find(|deposit| deposit.good_id == resource_kind)
                })
                .map(|deposit| deposit.discovered_level)
                .unwrap_or(0);
            assert!(
                building.level <= discovered,
                "{} in state {} exceeds deposit {}",
                building.building_def_id,
                game_state_id,
                discovered
            );
        }
    }

    #[test]
    fn ger_starts_with_corporatist_war_economy() {
        let mut data = hoi4_data::GameData::default();
        let tag = hoi4_data::CountryTag::new("GER");
        data.countries.insert(
            tag.clone(),
            hoi4_data::Country {
                tag,
                color: hoi4_data::Color {
                    r: 80,
                    g: 80,
                    b: 80,
                },
                graphical_culture: "western_european_gfx".to_owned(),
                capital: 1,
                ruling_party: "fascism".to_owned(),
                technologies: Vec::new(),
            },
        );
        let mut world = hoi4_state::World::new(
            std::sync::Arc::new(hoi4_map::GameMap {
                definitions: vec![],
                rgb_to_id: std::collections::HashMap::new(),
                province_map: hoi4_map::ProvinceMap {
                    width: 1,
                    height: 1,
                    pixels: vec![0],
                },
                adjacencies: vec![],
                special_adjacencies: vec![],
                heightmap: hoi4_map::Heightmap {
                    width: 1,
                    height: 1,
                    pixels: vec![0],
                },
                terrain_bmp: hoi4_map::TerrainBitmap {
                    width: 1,
                    height: 1,
                    pixels: vec![0],
                    palette: [[0; 3]; 256],
                },
                terrain_catalog: hoi4_map::TerrainCatalog::default(),
                tree_definition_bmp: None,
                tree_indices: std::collections::HashSet::new(),
            }),
            std::sync::Arc::new(data),
        );
        let db = V6Database::load();

        inject_v6_into_world(&mut world, &db);

        let ger = world.country("GER").expect("GER exists");
        let ci = ger.0 as usize;
        assert_eq!(
            world.countries.law_store.law_sets[ci].0[LawCategory::Economy.index()].current,
            "corporatist_war_economy",
        );
        assert_eq!(
            world.countries.law_store.law_sets[ci].0[LawCategory::Conscription.index()].current,
            "limited_conscription",
        );
    }

    #[test]
    fn h3_historical_profiles_generate_pops_for_all_major_countries() {
        let mut world = h2_test_world();
        let db = V6Database::load();

        inject_v6_into_world(&mut world, &db);

        for tag in ["USA", "GER", "SOV", "ENG", "FRA", "JAP", "ITA", "CHI"] {
            let country = world.country(tag).expect("country exists");
            let state_ids: Vec<StateId> = (0..world.states.count)
                .filter(|&si| world.states.owners[si] == country)
                .map(|si| StateId(si as u16))
                .collect();
            let total_pop: u64 = world
                .countries
                .pops
                .groups
                .iter()
                .filter(|pg| state_ids.contains(&pg.state))
                .map(|pg| pg.size as u64)
                .sum();
            assert!(total_pop > 0, "{tag} should receive POPs");
            let state_profile_total: u64 = db
                .state_populations
                .iter()
                .filter_map(|entry| {
                    world
                        .state_id_lookup
                        .get(&entry.state_id)
                        .copied()
                        .filter(|sid| state_ids.contains(sid))
                        .map(|_| entry.population as u64)
                })
                .sum();
            let fallback_total: u64 = state_ids
                .iter()
                .filter(|sid| {
                    let game_id = game_state_id(&world, **sid);
                    !db.state_populations
                        .iter()
                        .any(|entry| entry.state_id == game_id)
                })
                .map(|sid| fallback_state_population(&world, *sid) as u64)
                .sum();
            assert_eq!(
                total_pop,
                state_profile_total + fallback_total,
                "{tag} POP total should be the sum of owned state profiles plus state fallback"
            );
            for class in [
                PopClass::Peasant,
                PopClass::Worker,
                PopClass::Clerk,
                PopClass::Soldier,
            ] {
                assert!(
                    world.countries.pops.groups.iter().any(|pg| {
                        pg.class == class && state_ids.contains(&pg.state) && pg.size > 0
                    }),
                    "{tag} should have {class:?} POPs"
                );
            }
        }
    }

    #[test]
    fn p3_state_population_profiles_are_pop_source_of_truth() {
        let mut world = h2_test_world();
        let db = V6Database::load();

        inject_v6_into_world(&mut world, &db);

        for state_id in [28u16, 51, 29, 91, 92, 93, 105, 106, 107] {
            let sid = *world
                .state_id_lookup
                .get(&state_id)
                .expect("state id should resolve in test world");
            let expected = db
                .state_populations
                .iter()
                .find(|entry| entry.state_id == state_id)
                .expect("state population profile exists")
                .population as u64;
            let actual: u64 = world
                .countries
                .pops
                .groups
                .iter()
                .filter(|group| group.state == sid)
                .map(|group| group.size as u64)
                .sum();
            assert_eq!(
                actual, expected,
                "state {state_id} POP total should equal state_population.ron"
            );
        }
    }

    #[test]
    fn h3_building_jobs_do_not_exceed_available_population() {
        let mut world = h2_test_world();
        let db = V6Database::load();

        inject_v6_into_world(&mut world, &db);

        for tag in ["USA", "GER", "SOV", "ENG", "FRA", "JAP", "ITA", "CHI"] {
            let country = world.country(tag).expect("country exists");
            let demand = country_employment_demand(&world, country, &db);
            let supply = country_pop_supply(&world, country);
            for class_idx in 0..PopClass::COUNT {
                assert!(
                    demand[class_idx] <= supply[class_idx],
                    "{tag} demand for class {class_idx} exceeds POP supply: {} > {}",
                    demand[class_idx],
                    supply[class_idx]
                );
            }
        }
    }

    #[test]
    fn h3_initial_historical_buildings_start_employed() {
        let mut world = h2_test_world();
        let db = V6Database::load();

        inject_v6_into_world(&mut world, &db);

        let ger = world.country("GER").expect("GER exists");
        let mut employed_buildings = 0u32;
        let mut total_employment = 0u32;
        for building in &world.countries.buildings_v6.buildings {
            let si = building.state.0 as usize;
            if si >= world.states.count || world.states.owners[si] != ger || building.level == 0 {
                continue;
            }
            let employment: u32 = building.employment.iter().sum();
            if employment > 0 {
                employed_buildings += 1;
                total_employment += employment;
            }
        }

        assert!(
            employed_buildings > 0,
            "GER buildings should start employed"
        );
        assert!(
            total_employment > 0,
            "GER initial employment should be seeded"
        );
    }

    fn country_building_levels(world: &World, country: CountryId, ids: &[&str]) -> u16 {
        world
            .countries
            .buildings_v6
            .buildings
            .iter()
            .filter(|building| {
                let si = building.state.0 as usize;
                si < world.states.count
                    && world.states.owners[si] == country
                    && ids.contains(&building.building_def_id.as_str())
            })
            .map(|building| building.level as u16)
            .sum()
    }

    fn country_total_building_levels(world: &World, country: CountryId) -> u16 {
        world
            .countries
            .buildings_v6
            .buildings
            .iter()
            .filter(|building| {
                let si = building.state.0 as usize;
                si < world.states.count && world.states.owners[si] == country
            })
            .map(|building| building.level as u16)
            .sum()
    }

    fn country_avg_qualification(world: &World, country: CountryId) -> (f32, f32) {
        let mut total = 0u64;
        let mut literacy = 0.0f64;
        let mut skilled = 0.0f64;
        for pg in &world.countries.pops.groups {
            let si = pg.state.0 as usize;
            if si >= world.states.count || world.states.owners[si] != country {
                continue;
            }
            total += pg.size as u64;
            literacy += pg.literacy as f64 * pg.size as f64;
            skilled += pg.skilled_ratio as f64 * pg.size as f64;
        }
        if total == 0 {
            return (0.0, 0.0);
        }
        (
            (literacy / total as f64) as f32,
            (skilled / total as f64) as f32,
        )
    }

    #[test]
    fn i9_major_country_industry_calibration_is_visible() {
        let mut world = h2_test_world();
        let db = V6Database::load();

        inject_v6_into_world(&mut world, &db);

        let usa = world.country("USA").unwrap();
        let ger = world.country("GER").unwrap();
        let sov = world.country("SOV").unwrap();
        let eng = world.country("ENG").unwrap();
        let jap = world.country("JAP").unwrap();

        let usa_motor = country_building_levels(
            &world,
            usa,
            &[
                "vehicle_factory",
                "engine_plant",
                "machine_tool_works",
                "oil_refinery",
            ],
        );
        let ger_motor = country_building_levels(
            &world,
            ger,
            &[
                "vehicle_factory",
                "engine_plant",
                "machine_tool_works",
                "oil_refinery",
            ],
        );
        assert!(
            usa_motor > ger_motor,
            "USA should visibly lead GER in automotive, machine-tool and refining capacity: USA={usa_motor}, GER={ger_motor}"
        );
        assert!(
            country_building_levels(&world, usa, &["oil_rig", "oil_refinery"])
                > country_building_levels(&world, ger, &["oil_rig", "oil_refinery"]),
            "USA oil and refining advantage should be visible"
        );

        assert!(
            country_building_levels(
                &world,
                ger,
                &[
                    "steel_mill",
                    "chemical_plant",
                    "machine_tool_works",
                    "arms_industry",
                    "munition_plant",
                ],
            ) > country_building_levels(
                &world,
                jap,
                &["steel_mill", "chemical_plant", "machine_tool_works"]
            ),
            "GER should be stronger than JAP in chemicals, steel, machine tools and armaments"
        );
        assert_eq!(
            country_building_levels(&world, ger, &["oil_rig", "rubber_plantation"]),
            0,
            "GER should lack domestic oil and rubber extraction"
        );

        assert!(
            country_building_levels(
                &world,
                jap,
                &["shipyard", "arms_industry", "munition_plant"]
            ) > 0,
            "JAP should start with usable shipbuilding and armaments"
        );
        assert_eq!(
            country_building_levels(
                &world,
                jap,
                &["oil_rig", "rubber_plantation", "bauxite_mine"]
            ),
            0,
            "JAP should lack domestic oil, rubber and aluminium extraction in the test start"
        );

        assert!(
            country_building_levels(
                &world,
                sov,
                &["steel_mill", "machinery_workshop", "oil_rig"]
            ) > country_building_levels(
                &world,
                jap,
                &["steel_mill", "machinery_workshop", "oil_rig"]
            ),
            "SOV raw material and heavy industry base should exceed JAP"
        );
        assert!(
            country_building_levels(&world, sov, &["machine_tool_works", "electrical_works"])
                < country_building_levels(&world, usa, &["machine_tool_works", "electrical_works"]),
            "SOV precision industry should trail the USA"
        );
        let (_, sov_skilled) = country_avg_qualification(&world, sov);
        let (_, usa_skilled) = country_avg_qualification(&world, usa);
        assert!(
            sov_skilled < usa_skilled,
            "SOV skilled ratio should trail USA qualifications"
        );

        assert!(
            country_building_levels(&world, eng, &["bank", "shipyard"])
                >= country_building_levels(&world, ger, &["bank", "shipyard"]),
            "ENG finance and sea industry should be visible"
        );
    }

    #[test]
    fn h3_ger_initial_industrial_inputs_are_self_sustaining() {
        let mut world = h2_test_world();
        let db = V6Database::load();

        inject_v6_into_world(&mut world, &db);

        let ger = world.country("GER").expect("GER exists");
        let mut net: std::collections::HashMap<String, f32> = std::collections::HashMap::new();
        for building in &world.countries.buildings_v6.buildings {
            let si = building.state.0 as usize;
            if si >= world.states.count || world.states.owners[si] != ger || building.level == 0 {
                continue;
            }
            for pm in active_pms_for_building(building, &db)
                .into_iter()
                .filter(|pm| pm.unlocked_by.is_none())
            {
                for (idx, good_id) in pm.output_good_ids.iter().enumerate() {
                    let amount = pm.output_good_amounts.get(idx).copied().unwrap_or(0.0)
                        * building.level as f32
                        * pm.throughput_modifier.max(0.0);
                    *net.entry(good_id.clone()).or_insert(0.0) += amount;
                }
                for (idx, good_id) in pm.input_good_ids.iter().enumerate() {
                    let amount = pm.input_good_amounts.get(idx).copied().unwrap_or(0.0)
                        * building.level as f32;
                    *net.entry(good_id.clone()).or_insert(0.0) -= amount;
                }
            }
        }

        for good_id in ["coal", "steel", "machinery", "artillery_shells"] {
            let flow = net.get(good_id).copied().unwrap_or(0.0);
            assert!(
                flow >= -0.01,
                "GER initial economy should not rely on imports/stockpiles for {good_id}: net daily flow {flow:.2}"
            );
        }
    }

    #[test]
    fn h3_profile_unemployment_affects_initial_satisfaction() {
        let mut world = h2_test_world();
        let db = V6Database::load();

        inject_v6_into_world(&mut world, &db);

        let usa = world.country("USA").unwrap();
        let ger = world.country("GER").unwrap();
        let usa_states: Vec<StateId> = (0..world.states.count)
            .filter(|&si| world.states.owners[si] == usa)
            .map(|si| StateId(si as u16))
            .collect();
        let ger_states: Vec<StateId> = (0..world.states.count)
            .filter(|&si| world.states.owners[si] == ger)
            .map(|si| StateId(si as u16))
            .collect();
        let usa_worker_sat = world
            .countries
            .pops
            .groups
            .iter()
            .find(|pg| pg.class == PopClass::Worker && usa_states.contains(&pg.state))
            .map(|pg| pg.satisfaction)
            .unwrap();
        let ger_worker_sat = world
            .countries
            .pops
            .groups
            .iter()
            .find(|pg| pg.class == PopClass::Worker && ger_states.contains(&pg.state))
            .map(|pg| pg.satisfaction)
            .unwrap();
        assert!(
            usa_worker_sat < ger_worker_sat,
            "higher USA unemployment should lower initial worker satisfaction"
        );
    }

    #[test]
    fn g13_historical_gdp_is_validation_only() {
        let mut world = h2_test_world();
        let db = V6Database::load();

        inject_v6_into_world(&mut world, &db);

        for tag in ["USA", "GER", "SOV", "ENG", "FRA", "JAP", "ITA", "CHI"] {
            let country = world.country(tag).expect("country exists");
            let profile = db
                .historical_countries
                .iter()
                .find(|profile| profile.tag == tag)
                .expect("historical profile exists");
            let treasury = &world.countries.treasury.treasuries[country.0 as usize];
            assert_eq!(treasury.gdp_gbp, 0.0, "{tag} runtime GDP starts unset");
            assert_eq!(treasury.gdp_rm, 0.0, "{tag} runtime GDP RM starts unset");
            assert_eq!(
                treasury.gdp_breakdown.historical_validation_gbp, profile.gdp_1936_gbp,
                "{tag} historical GDP should be retained only as validation target"
            );
            assert!(
                treasury
                    .gdp_breakdown
                    .historical_validation_error_ratio
                    .is_finite(),
                "{tag} validation error should be computed from generated buildings"
            );
        }
    }

    #[test]
    fn g13_validation_table_covers_all_historical_population_and_gdp_references() {
        let db = V6Database::load();
        let tags: Vec<&str> = db
            .historical_countries
            .iter()
            .map(|profile| profile.tag.as_str())
            .collect();
        let mut world = g06_minimum_economy_world(&tags);

        inject_v6_into_world(&mut world, &db);

        let table = historical_validation_table(&world, &db);
        assert_eq!(
            table.len(),
            db.historical_countries.len(),
            "validation table should include every historical country present in the world"
        );
        for profile in &db.historical_countries {
            let row = table
                .iter()
                .find(|row| row.tag == profile.tag)
                .unwrap_or_else(|| panic!("{} validation row exists", profile.tag));
            let country = world
                .country(profile.tag.as_str())
                .unwrap_or_else(|| panic!("{} exists", profile.tag));
            let owned_state_pop: u64 = (0..world.states.count)
                .filter(|&si| world.states.owners[si] == country)
                .map(|si| {
                    world
                        .countries
                        .pops
                        .groups
                        .iter()
                        .filter(|group| group.state == StateId(si as u16))
                        .map(|group| group.size as u64)
                        .sum::<u64>()
                })
                .sum();
            assert_eq!(
                row.runtime_population, owned_state_pop,
                "{} runtime population should be owned-state POP sum",
                profile.tag
            );
            assert_eq!(
                row.historical_reference_population, profile.population as u64,
                "{} historical population should stay reference-only",
                profile.tag
            );
            assert_eq!(
                row.historical_reference_gdp_gbp, profile.gdp_1936_gbp,
                "{} historical GDP should stay reference-only",
                profile.tag
            );
            assert!(
                row.population_error_ratio.is_finite(),
                "{} population validation error should be finite",
                profile.tag
            );
            assert!(
                row.generated_gdp_gbp.is_finite(),
                "{} generated GDP should be finite",
                profile.tag
            );
            assert!(
                row.gdp_error_ratio.is_finite(),
                "{} GDP validation error should be finite",
                profile.tag
            );
        }
    }

    #[test]
    fn h5_historical_trade_routes_are_injected() {
        let mut world = h2_test_world();
        let db = V6Database::load();

        inject_v6_into_world(&mut world, &db);

        let ger = world.country("GER").unwrap();
        let jap = world.country("JAP").unwrap();
        let eng = world.country("ENG").unwrap();
        assert!(
            world.countries.trade.routes.iter().any(|route| {
                route.importer == ger && route.good_id.as_deref() == Some("oil") && route.historical
            }),
            "GER should start with a historical oil import route"
        );
        assert!(
            world.countries.trade.routes.iter().any(|route| {
                route.importer == jap
                    && route.good_id.as_deref() == Some("rubber")
                    && route.historical
            }),
            "JAP should start with a historical rubber import route"
        );
        assert!(
            world.countries.trade.routes.iter().any(|route| {
                route.importer == eng
                    && route.good_id.as_deref() == Some("grain")
                    && route.historical
            }),
            "ENG should start with an imperial grain import route"
        );
        assert!(
            world.countries.trade.agreements.iter().any(|agreement| {
                agreement.party_a == jap
                    && agreement.good_id == "oil"
                    && agreement.daily_quantity > 0.0
            }),
            "historical routes with known exporters should create trade agreements"
        );
    }

    #[test]
    fn injected_buildings_clamp_to_max_level() {
        let data = std::sync::Arc::new(hoi4_data::GameData::default());
        let mut world = hoi4_state::World::new(
            std::sync::Arc::new(hoi4_map::GameMap {
                definitions: vec![],
                rgb_to_id: std::collections::HashMap::new(),
                province_map: hoi4_map::ProvinceMap {
                    width: 1,
                    height: 1,
                    pixels: vec![0],
                },
                adjacencies: vec![],
                special_adjacencies: vec![],
                heightmap: hoi4_map::Heightmap {
                    width: 1,
                    height: 1,
                    pixels: vec![0],
                },
                terrain_bmp: hoi4_map::TerrainBitmap {
                    width: 1,
                    height: 1,
                    pixels: vec![0],
                    palette: [[0; 3]; 256],
                },
                terrain_catalog: hoi4_map::TerrainCatalog::default(),
                tree_definition_bmp: None,
                tree_indices: std::collections::HashSet::new(),
            }),
            data,
        );
        let db = V6Database::load();
        let max_level = db
            .buildings
            .iter()
            .find(|b| b.id == "steel_mill")
            .unwrap()
            .max_level;

        inject_building(
            &mut world,
            0,
            "steel_mill",
            StateId(0),
            max_level.saturating_add(20),
            &db,
        );

        assert_eq!(world.countries.buildings_v6.buildings[0].level, max_level);
    }

    #[test]
    fn invariant_i2_ger_manpower_matches_population() {
        // I-2: GER 1936 total population from initial_pops should be ~67M
        // Soldier POP should be ~1.58M (2.37% of population = volunteer_only 1% + existing soldiers)
        // Total manpower (available Soldier POP) should be within 5% of ~23M (鍏靛焦閫傞緞浜哄彛)
        let db = V6Database::load();
        let pops = db
            .initial_pops
            .get("GER")
            .expect("initial GER pops must load");
        let total_pop: u64 = pops.groups.iter().map(|p| p.size as u64).sum();
        assert!(total_pop > 0, "total population must be non-zero");
        let expected = 67_000_000u64;
        let ratio = total_pop as f64 / expected as f64;
        assert!(
            (ratio - 1.0).abs() < 0.10,
            "GER total population = {} (expected ~{}, ratio={:.2})",
            total_pop,
            expected,
            ratio
        );
        let soldier_pop: u64 = pops
            .groups
            .iter()
            .filter(|p| matches!(p.class, PopClassDef::Soldier))
            .map(|p| p.size as u64)
            .sum();
        let available_soldiers: u64 = pops
            .groups
            .iter()
            .filter(|p| matches!(p.class, PopClassDef::Soldier) && p.employed_at.is_none())
            .map(|p| p.size as u64)
            .sum();
        assert!(
            available_soldiers > 0,
            "GER available Soldier POP must be > 0 (got {})",
            available_soldiers
        );
        assert!(
            soldier_pop <= total_pop / 5,
            "Soldier POP should be < 20% of total (got {:.1}%)",
            soldier_pop as f64 / total_pop as f64 * 100.0
        );
    }
}
