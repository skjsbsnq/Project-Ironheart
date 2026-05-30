//! V6 贸易路线与封锁状态（§4.9）。

use serde::{Deserialize, Serialize};

use std::collections::HashMap;

use crate::ids::{CountryId, StateId};
use crate::ProvinceId;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum TradeRouteKind {
    Sea,
    Land,
    Transit,
    ImperialPreference,
}

impl TradeRouteKind {
    pub fn uses_sea_lanes(self) -> bool {
        matches!(self, Self::Sea | Self::ImperialPreference)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TradeRoute {
    pub id: u32,
    pub importer: CountryId,
    pub exporter: CountryId,
    #[serde(default)]
    pub good_id: Option<String>,
    pub kind: TradeRouteKind,
    pub port_state: Option<StateId>,
    pub throughput: f32,
    pub is_blockaded: bool,
    #[serde(default)]
    pub historical: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TradeAgreement {
    pub id: u32,
    pub party_a: CountryId,
    pub party_b: CountryId,
    pub good_id: String,
    pub daily_quantity: f32,
    pub direction: TradeDirection,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum TradeDirection {
    AImportsFromB,
    BImportsFromA,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SeaRegionMapSnapshot {
    pub province_region: HashMap<u16, u32>,
}

impl Default for SeaRegionMapSnapshot {
    fn default() -> Self {
        Self {
            province_region: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TradeStore {
    pub routes: Vec<TradeRoute>,
    pub agreements: Vec<TradeAgreement>,
    pub next_route_id: u32,
    pub next_agreement_id: u32,
    pub blockaded_ports: Vec<bool>,
    #[serde(default)]
    pub import_failures: HashMap<String, String>,
    #[serde(default)]
    pub port_to_sea_region: HashMap<u16, u32>,
    #[serde(default)]
    pub sea_region_map: Option<SeaRegionMapSnapshot>,
    #[serde(default)]
    pub world_spot_daily_consumed: HashMap<String, f32>,
}

impl TradeStore {
    pub fn new() -> Self {
        Self {
            routes: Vec::new(),
            agreements: Vec::new(),
            next_route_id: 0,
            next_agreement_id: 0,
            blockaded_ports: Vec::new(),
            import_failures: HashMap::new(),
            port_to_sea_region: HashMap::new(),
            sea_region_map: None,
            world_spot_daily_consumed: HashMap::new(),
        }
    }

    pub fn ensure_capacity(&mut self, state_count: usize) {
        while self.blockaded_ports.len() < state_count {
            self.blockaded_ports.push(false);
        }
    }

    pub fn add_route(
        &mut self,
        importer: CountryId,
        exporter: CountryId,
        kind: TradeRouteKind,
        port_state: Option<StateId>,
        base_throughput: f32,
    ) -> u32 {
        self.add_route_for_good(
            importer,
            exporter,
            None,
            kind,
            port_state,
            base_throughput,
            false,
        )
    }

    pub fn add_route_for_good(
        &mut self,
        importer: CountryId,
        exporter: CountryId,
        good_id: Option<String>,
        kind: TradeRouteKind,
        port_state: Option<StateId>,
        base_throughput: f32,
        historical: bool,
    ) -> u32 {
        let id = self.next_route_id;
        self.next_route_id += 1;
        self.routes.push(TradeRoute {
            id,
            importer,
            exporter,
            good_id,
            kind,
            port_state,
            throughput: base_throughput,
            is_blockaded: false,
            historical,
        });
        id
    }

    pub fn add_agreement(
        &mut self,
        party_a: CountryId,
        party_b: CountryId,
        good_id: String,
        daily_quantity: f32,
        direction: TradeDirection,
    ) -> u32 {
        let id = self.next_agreement_id;
        self.next_agreement_id += 1;
        self.agreements.push(TradeAgreement {
            id,
            party_a,
            party_b,
            good_id,
            daily_quantity,
            direction,
        });
        id
    }

    pub fn trade_throughput_for_state(&self, state_idx: usize) -> f32 {
        if state_idx >= self.blockaded_ports.len() {
            return 1.0;
        }
        if self.blockaded_ports[state_idx] {
            0.0
        } else {
            1.0
        }
    }
}
