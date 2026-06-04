use serde::Deserialize;
use std::collections::HashMap;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Historical1936Database {
    pub countries: Vec<HistoricalCountryEconomyDef>,
    pub state_populations: Vec<StatePopulation1936Def>,
    pub state_deposits: Vec<StateResourceDepositDef>,
    pub trade_routes: Vec<HistoricalTradeProfileDef>,
    pub military_profiles: Vec<HistoricalMilitaryProfileDef>,
    pub head_of_states: Vec<HeadOfState1936Def>,
}

impl Historical1936Database {
    pub fn load() -> Result<Self, ron::error::SpannedError> {
        let countries = vec![
            load_ron(include_str!("../content/history_1936/countries/USA.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/GER.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/SOV.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/ENG.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/FRA.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/CZE.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/JAP.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/ITA.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/SPR.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/CHI.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/HBC.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/SND.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/PRC.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/SHX.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/GXC.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/GDC.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/YUN.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/XAJ.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/SIC.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/XSM.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/SIK.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/TIB.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/CAN.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/AST.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/NZL.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/SAF.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/RAJ.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/MAL.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/ROM.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/SWE.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/MAN.ron"))?,
            load_ron(include_str!("../content/history_1936/countries/MEN.ron"))?,
        ];

        Ok(Self {
            countries,
            state_populations: load_ron(include_str!(
                "../content/history_1936/states/state_population.ron"
            ))?,
            state_deposits: load_ron(include_str!(
                "../content/history_1936/resources/state_deposits.ron"
            ))?,
            trade_routes: load_ron(include_str!(
                "../content/history_1936/trade/initial_trade_1936.ron"
            ))?,
            military_profiles: load_ron(include_str!(
                "../content/history_1936/military/force_profiles_1936.ron"
            ))?,
            head_of_states: load_ron(include_str!(
                "../content/history_1936/politics/head_of_state_1936.ron"
            ))?,
        })
    }

    pub fn country(&self, tag: &str) -> Option<&HistoricalCountryEconomyDef> {
        self.countries.iter().find(|country| country.tag == tag)
    }

    pub fn state_population(&self, state_id: u16) -> Option<&StatePopulation1936Def> {
        self.state_populations
            .iter()
            .find(|state| state.state_id == state_id)
    }

    pub fn head_of_state(&self, tag: &str) -> Option<&HeadOfState1936Def> {
        self.head_of_states.iter().find(|entry| entry.tag == tag)
    }

    pub fn initial_equipment_stockpiles(&self) -> HashMap<String, HashMap<String, f32>> {
        let countries_by_tag: HashMap<&str, &HistoricalCountryEconomyDef> = self
            .countries
            .iter()
            .map(|country| (country.tag.as_str(), country))
            .collect();
        let mut by_tag = HashMap::new();

        for profile in &self.military_profiles {
            let country = countries_by_tag.get(profile.tag.as_str()).copied();
            let readiness = initial_equipment_readiness(profile.tag.as_str(), country);
            let mut stockpile = HashMap::new();

            let infantry_equipment = (profile.army_divisions as f32 * 1_000.0 * readiness)
                .max(profile.active_personnel as f32 * 0.12 * readiness);
            stockpile.insert("infantry_equipment".to_owned(), infantry_equipment.round());

            let artillery = (profile.army_divisions as f32 * 36.0 * readiness * 0.65).round();
            if artillery > 0.0 {
                stockpile.insert("artillery".to_owned(), artillery);
            }

            if matches!(
                profile.tag.as_str(),
                "GER" | "SOV" | "ENG" | "FRA" | "USA" | "JAP" | "ITA"
            ) {
                let support = (profile.army_divisions as f32 * 60.0 * readiness).round();
                if support > 0.0 {
                    stockpile.insert("support_equipment".to_owned(), support);
                }

                let motorized = (profile.army_divisions as f32 * 18.0 * readiness).round();
                if motorized > 0.0 {
                    stockpile.insert("motorized".to_owned(), motorized);
                }

                let armor = (profile.army_divisions as f32 * 8.0 * readiness).round();
                if armor > 0.0 {
                    stockpile.insert("armor".to_owned(), armor);
                }
            }

            let aircraft = (profile.air_force_index * 20.0 * readiness).round();
            if aircraft > 0.0 {
                stockpile.insert("aircraft".to_owned(), aircraft);
            }

            let convoys = (profile.naval_tonnage_index * 3.0 * readiness).round();
            if convoys > 0.0 {
                stockpile.insert("convoy".to_owned(), convoys);
            }

            by_tag.insert(profile.tag.clone(), stockpile);
        }

        by_tag
    }
}

fn initial_equipment_readiness(tag: &str, country: Option<&HistoricalCountryEconomyDef>) -> f32 {
    match tag {
        "PRC" => 0.12,
        "TIB" => 0.22,
        "XSM" | "SIK" => 0.14,
        "HBC" => 0.30,
        "SHX" | "GXC" | "GDC" | "YUN" | "XAJ" | "SIC" | "SND" => 0.16,
        "CHI" => 0.18,
        "MEN" => 0.45,
        "MAN" => 0.70,
        "JAP" => 1.80,
        _ => {
            let industrial = country
                .map(|profile| profile.industrial_capacity_index)
                .unwrap_or(0.0);
            if industrial >= 5.0 {
                1.30
            } else if industrial >= 2.0 {
                1.10
            } else {
                0.85
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct HeadOfState1936Def {
    pub tag: String,
    #[serde(default)]
    pub character_key: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub portrait_gfx: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct HistoricalCountryEconomyDef {
    pub tag: String,
    pub population: u32,
    pub gdp_1936_gbp: f64,
    pub gdp_quality: HistoricalDataQuality,
    pub sector_shares: SectorShares,
    pub urbanization: f32,
    pub literacy: f32,
    pub unemployment: f32,
    pub industrial_capacity_index: f32,
    pub military_spending_share: f32,
    pub construction_capacity_index: f32,
    pub gold_reserve_gbp: f64,
    pub foreign_exchange_reserve_gbp: f64,
    pub public_debt_gbp: f64,
    pub initial_laws: InitialLawSet,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum HistoricalDataQuality {
    Primary,
    Estimated,
    Rough,
    Fallback,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct StatePopulation1936Def {
    pub state_id: u16,
    pub population: u32,
    pub integration: StateIntegrationDef,
    pub urbanization: Option<f32>,
    pub literacy: Option<f32>,
    pub workforce_profile: Option<WorkforceProfileDef>,
    pub data_quality: HistoricalDataQuality,
}

pub type StatePopulationProfile1936 = StatePopulation1936Def;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum StateIntegrationDef {
    Metropole,
    Incorporated,
    Colony,
    Protectorate,
    Mandate,
    Concession,
    Occupied,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum WorkforceProfileDef {
    Agrarian,
    Industrial,
    Mining,
    Plantation,
    UrbanServices,
    MilitaryFrontier,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SectorShares {
    pub agriculture: f32,
    pub mining: f32,
    pub heavy_industry: f32,
    pub light_industry: f32,
    pub services: f32,
    pub government: f32,
    pub military_industry: f32,
}

impl SectorShares {
    pub fn total(&self) -> f32 {
        self.agriculture
            + self.mining
            + self.heavy_industry
            + self.light_industry
            + self.services
            + self.government
            + self.military_industry
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct InitialLawSet {
    pub conscription: String,
    pub economy: String,
    pub trade: String,
    pub taxation: String,
    pub civil_rights: String,
    pub information_control: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct StateResourceDepositDef {
    pub state_id: u16,
    pub deposits: Vec<ResourceDepositDef>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ResourceDepositDef {
    pub good_id: String,
    pub discovered_level: u8,
    pub potential_level: u8,
    pub extraction_difficulty: f32,
    pub requires_tech: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct HistoricalTradeProfileDef {
    pub importer: String,
    pub exporter: String,
    pub good_id: String,
    pub daily_quantity: f32,
    pub route_kind: TradeRouteKindDef,
    pub port_state: Option<u16>,
    pub strategic_importance: f32,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum TradeRouteKindDef {
    Land,
    Sea,
    ImperialPreference,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct HistoricalMilitaryProfileDef {
    pub tag: String,
    pub active_personnel: u32,
    pub reserve_personnel: u32,
    pub army_divisions: u16,
    pub naval_tonnage_index: f32,
    pub air_force_index: f32,
    pub military_spending_gbp: f64,
    pub equipment_demands: Vec<MilitaryEquipmentDemandDef>,
    pub fuel_need_daily: f32,
    pub maintenance_gbp_daily: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct MilitaryEquipmentDemandDef {
    pub equipment_category: String,
    pub daily_quantity: f32,
}

fn load_ron<T: serde::de::DeserializeOwned>(s: &str) -> Result<T, ron::error::SpannedError> {
    ron::from_str(s)
}
