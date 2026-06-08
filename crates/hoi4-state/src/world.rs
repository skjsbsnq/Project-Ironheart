//! 顶层 World 容器，包含所有运行时状态。

use std::collections::HashMap;
use std::sync::Arc;

use hoi4_data::GameData;
use hoi4_map::GameMap;

use crate::diplomacy::DiplomacyState;
use crate::ids::{CountryId, ProvinceId, StateId};
use crate::store::{
    AirWingStore, CountryStore, DivisionStore, FleetStore, ProvinceStore, ShipStore, StateStore,
};
use crate::time::{GameDate, GameSpeed};
use crate::{
    InvestmentAccount, InvestmentAccountKind, PopGroup, PopulationBreakdown, StateIntegrationStatus,
};

/// Phase 1.1：[`World::populate_from_history`] 的诊断报告。
#[derive(Debug, Default, Clone)]
pub struct PopulateReport {
    pub divisions_spawned: usize,
    pub fleets_spawned: usize,
    pub ships_spawned: usize,
    pub air_wings_spawned: usize,
    pub ideas_applied: usize,
    pub focuses_completed: usize,
    /// tag → 该国 spawn 的师数（集成测试用）
    pub divisions_per_country: HashMap<String, usize>,
    /// 警告：模板名在 OOB 中引用但 division_templates 中找不到
    pub warn_template_missing: usize,
    /// 警告：set_oob 引用了不存在的 OOB 名（vanilla 可能因为 DLC 缺失而触发；
    /// Phase 1.1 不读 has_dlc，会优先非 DLC 分支）
    pub warn_oob_missing: usize,
    /// 警告：舰类未注册
    pub warn_ship_class_missing: usize,
    /// 警告：飞机类未注册
    pub warn_aircraft_missing: usize,
}

/// 从 vanilla 装备 key 还原飞机类 key。
///
/// vanilla 的 equipment_key 形如 `fighter_equipment_0` / `tac_bomber_equipment_0` /
/// `CAS_equipment_1` / `nav_bomber_equipment_1`。我们去掉数字后缀和
/// `_equipment` 中缀，再映射到 `data.aircraft` 中的实际键名。
fn aircraft_key_from_equipment(eq: &str) -> String {
    let lower = eq.to_ascii_lowercase();
    // 切掉 _equipment_<n>
    let stem = match lower.find("_equipment") {
        Some(idx) => &lower[..idx],
        None => &lower,
    };
    // vanilla → hoi4_data::aircraft 键名映射
    match stem {
        "fighter" => "fighter".to_owned(),
        "heavy_fighter" => "heavy_fighter".to_owned(),
        "cas" => "cas".to_owned(),
        "tac_bomber" => "tac_bomber".to_owned(),
        "strat_bomber" | "strategic_bomber" => "strategic_bomber".to_owned(),
        "nav_bomber" | "naval_bomber" => "naval_bomber".to_owned(),
        "transport_plane" => "transport_plane".to_owned(),
        // 其它前缀（jet_*, ms_* 等）保持原样
        _ => stem.to_owned(),
    }
}

/// 完整游戏世界状态
pub struct World {
    // 时间
    pub date: GameDate,
    pub speed: GameSpeed,
    /// 自开局以来的小时数
    pub elapsed_hours: u64,

    // 实体存储
    pub provinces: ProvinceStore,
    pub states: StateStore,
    pub countries: CountryStore,
    pub divisions: DivisionStore,
    pub ships: ShipStore,
    pub fleets: FleetStore,
    pub air_wings: AirWingStore,

    /// 外交状态：阵营 / 战争 / wargoal / 傀儡 / opinion / 世界紧张度
    pub diplomacy: DiplomacyState,

    /// CR-3: OOB command hierarchy
    pub command: crate::command::CommandHierarchy,

    // 加载时数据（不可变共享引用）
    pub map: Arc<GameMap>,
    pub data: Arc<GameData>,

    // 索引
    /// 国家 tag → CountryId
    pub tag_to_country: HashMap<String, CountryId>,
    /// State id (game ID) → StateId (内部索引)
    pub state_id_lookup: HashMap<u16, StateId>,

    // ─── 存档 / 元数据 ───
    /// 玩家控制的国家（None = 观察者模式）
    pub player: CountryId,
    /// RNG 种子，决定每次开局的伪随机序列。存档读写时保留以保证一致性
    pub random_seed: u64,
    /// 全局存档唯一标识符（每次新开局生成一次）
    pub game_unique_id: u64,

    /// P4: BFS next_step 缓存。key = (owner.0, from.0, dest.0) → value = next province raw id。
    /// TTL = 一天，每天开始时清空。
    pub path_cache: HashMap<(u16, u16, u16), u16>,
    /// P4: path_cache 创建时的 game day（用于 TTL 判定）
    pub path_cache_day: i64,
    /// 性能索引：province raw id → 师团下标列表。
    /// 每天在 movement tick 之后调用 `rebuild_province_div_index()` 重建。
    pub prov_div_index: HashMap<u16, Vec<usize>>,
    /// P2.1：国家 → 州索引，避免每国 tick 反复扫描全部州。
    pub country_state_index: Vec<Vec<StateId>>,
    /// P2.1：国家 → POP group 下标索引。
    pub country_pop_index: Vec<Vec<usize>>,
    /// P2.1：国家 → 建筑下标索引。
    pub country_building_index: Vec<Vec<usize>>,
    /// P2.1：国家 → 师下标索引。
    pub country_division_index: Vec<Vec<usize>>,
    /// P2.1：国家 → 舰队下标索引。
    pub country_fleet_index: Vec<Vec<usize>>,
    /// P2.1：国家 → 空军联队下标索引。
    pub country_air_wing_index: Vec<Vec<usize>>,
    pub runtime_country_indexes_valid: bool,
    /// P2.1：商品 → 有可出口 surplus 的国家候选。
    pub trade_export_surplus_index: HashMap<String, Vec<(CountryId, f32)>>,

    // ─── 玩家划线战线（frontline-orders, design.md "数据模型"）───
    /// 玩家与 AI 共享的划线集团军容器。每条由 `PlayerArmy.owner` 区分所有者。
    /// 容器名沿用 `player_armies` 仅为兼容历史命名（_R15.1_）。
    pub player_armies: Vec<crate::frontline::PlayerArmy>,
    /// 被战线指令托管的师下标集合：`execute_ground_orders` 必须跳过这些
    /// 下标。由 `tick_frontlines` 在每个每日 tick 末尾从所有活跃集团军重建
    /// （_R8.4_）。
    pub player_locked_divisions: std::collections::HashSet<usize>,
    /// 单调递增的 `ArmyId` 分配器；解散不回收，保证 `ArmyId` 在整局中唯一
    /// （_R1.1_）。
    pub next_army_id: u32,
    pub generals: Vec<crate::frontline::General>,
    pub next_general_id: u32,
}

impl World {
    /// 从加载的静态数据初始化游戏世界（1936.1.1）
    pub fn new(map: Arc<GameMap>, data: Arc<GameData>) -> Self {
        // 1. 创建国家存储
        let country_count = data.countries.len();
        let mut country_store = CountryStore::new(country_count);
        let mut tag_to_country = HashMap::with_capacity(country_count);

        // 排序国家 tag 保证确定性
        let mut tags: Vec<&String> = data.countries.keys().map(|t| &t.0).collect();
        tags.sort();

        for (idx, tag) in tags.iter().enumerate() {
            let country = &data.countries[&hoi4_data::CountryTag::new(tag)];
            country_store.tags[idx] = (*tag).clone();
            country_store.colors[idx] = [country.color.r, country.color.g, country.color.b];
            // capital is a state ID (game ID), translate later after state_id_lookup is built
            country_store.completed_techs[idx] = country.technologies.clone();
            country_store.ruling_party[idx] = country.ruling_party.clone();
            tag_to_country.insert((*tag).clone(), CountryId(idx as u16));
        }

        // 2. 创建州存储
        let state_count = data.states.len();
        let mut state_store = StateStore::new(state_count);
        let mut state_id_lookup = HashMap::with_capacity(state_count);

        let max_prov_id = map.definitions.len();
        let mut province_store = ProvinceStore::new(max_prov_id);

        for (idx, state) in data.states.iter().enumerate() {
            let owner_id = tag_to_country
                .get(state.owner.as_str())
                .copied()
                .unwrap_or(CountryId::NONE);
            state_store.owners[idx] = owner_id;
            state_store.controllers[idx] = owner_id;
            state_store.cores[idx] = state
                .cores
                .iter()
                .filter_map(|c| tag_to_country.get(c.as_str()).copied())
                .collect();
            state_store.provinces[idx] = state.provinces.iter().map(|&p| ProvinceId(p)).collect();
            state_store.infrastructure[idx] = state.infrastructure;
            state_store.manpower_pool[idx] = state.manpower as u32;
            state_store.names[idx] = state.name.clone();

            state_id_lookup.insert(state.id, StateId(idx as u16));

            // 设置省份归属
            for &prov_id in &state.provinces {
                let pidx = prov_id as usize;
                if pidx < province_store.count {
                    province_store.owners[pidx] = owner_id;
                    province_store.controllers[pidx] = owner_id;
                    province_store.state_of[pidx] = StateId(idx as u16);
                    province_store.supply[pidx] = 50.0;
                }
            }
        }

        // 3. 回填 capitals（HOI4 的 capital 字段是 state 游戏 ID，需要转换为内部 StateId）
        for (idx, tag) in tags.iter().enumerate() {
            let country = &data.countries[&hoi4_data::CountryTag::new(tag)];
            if let Some(&sid) = state_id_lookup.get(&country.capital) {
                country_store.capitals[idx] = sid;
            }
        }

        // J.3: populate category_slots from state_categories
        for (idx, state) in data.states.iter().enumerate() {
            if let Some(cat) = data.state_categories.get(&state.category) {
                state_store.category_slots[idx] = cat.local_building_slots;
            }
        }

        // 4. (V6: factory cache removed — building-based calculations replace civ/mil/dock counts)

        // 5. 由 completed_techs 推导初始解锁的装备/子单位/建筑
        for cidx in 0..country_store.count {
            let techs = country_store.completed_techs[cidx].clone();
            for tech_key in &techs {
                if let Some(tech) = data.technologies.get(tech_key) {
                    for eq in &tech.enable_equipments {
                        country_store.unlocked_equipments[cidx].insert(eq.clone());
                    }
                    for su in &tech.enable_subunits {
                        country_store.unlocked_subunits[cidx].insert(su.clone());
                    }
                    for b in &tech.enable_building {
                        country_store.unlocked_buildings[cidx].insert(b.clone());
                    }
                }
            }
        }

        let mut world = Self {
            date: GameDate::START,
            speed: GameSpeed::Paused,
            elapsed_hours: 0,
            provinces: province_store,
            states: state_store,
            countries: country_store,
            divisions: DivisionStore::new(),
            ships: ShipStore::new(),
            fleets: FleetStore::new(),
            air_wings: AirWingStore::new(),
            diplomacy: DiplomacyState::new(),
            command: crate::command::CommandHierarchy::default(),
            map,
            data,
            tag_to_country,
            state_id_lookup,
            player: CountryId::NONE,
            random_seed: 0,
            game_unique_id: 0,
            path_cache: HashMap::new(),
            path_cache_day: 0,
            prov_div_index: HashMap::new(),
            country_state_index: vec![Vec::new(); country_count],
            country_pop_index: vec![Vec::new(); country_count],
            country_building_index: vec![Vec::new(); country_count],
            country_division_index: vec![Vec::new(); country_count],
            country_fleet_index: vec![Vec::new(); country_count],
            country_air_wing_index: vec![Vec::new(); country_count],
            runtime_country_indexes_valid: false,
            trade_export_surplus_index: HashMap::new(),
            player_armies: Vec::new(),
            player_locked_divisions: std::collections::HashSet::new(),
            next_army_id: 0,
            generals: Vec::new(),
            next_general_id: 0,
        };

        world.seed_default_generals();
        // J.1.2：从 `data.countries[tag].ruling_party` 推出的初始 leader（在
        // populate_from_history 重新设 ruling_party 之前的"裸"状态）。提供尽早的
        // 字段非空保证，便于无 history 跑测试时也能查询。
        world.refresh_country_leader_all();
        world
    }

    pub fn seed_default_generals(&mut self) {
        if !self.generals.is_empty() {
            return;
        }
        let mut tags: Vec<(String, CountryId)> = self
            .tag_to_country
            .iter()
            .map(|(tag, &cid)| (tag.clone(), cid))
            .collect();
        tags.sort_by(|a, b| a.0.cmp(&b.0));
        for (tag, cid) in tags {
            let names: [&str; 3] = match tag.as_str() {
                "GER" => ["Heinz Guderian", "Erwin Rommel", "Fedor von Bock"],
                "SOV" => ["Georgy Zhukov", "Konstantin Rokossovsky", "Ivan Konev"],
                "ENG" => ["Bernard Montgomery", "Harold Alexander", "Alan Brooke"],
                "USA" => ["Dwight Eisenhower", "George Patton", "Omar Bradley"],
                "FRA" => ["Charles de Gaulle", "Alphonse Juin", "Jean de Lattre"],
                "ITA" => ["Giovanni Messe", "Italo Gariboldi", "Ugo Cavallero"],
                "JAP" => ["Tomoyuki Yamashita", "Hisaichi Terauchi", "Shunroku Hata"],
                _ => ["Field Commander", "Army Commander", "Staff Commander"],
            };
            for (idx, name) in names.iter().enumerate() {
                let id = crate::frontline::GeneralId(self.next_general_id);
                self.next_general_id = self.next_general_id.saturating_add(1);
                self.generals.push(crate::frontline::General {
                    id,
                    owner: cid,
                    name: if *name == "Field Commander" {
                        format!("{} Field Commander", tag)
                    } else if *name == "Army Commander" {
                        format!("{} Army Commander", tag)
                    } else if *name == "Staff Commander" {
                        format!("{} Staff Commander", tag)
                    } else {
                        (*name).to_owned()
                    },
                    skill: 2 + idx as u8,
                    attack: 1 + (idx == 1) as u8,
                    defense: 1 + (idx == 2) as u8,
                    planning: 1 + (idx == 0) as u8,
                    logistics: 1,
                    command_limit: 24,
                });
            }
        }
    }

    pub fn state_integration_status(&self, state: StateId) -> Option<StateIntegrationStatus> {
        self.states
            .integration_status
            .get(state.0 as usize)
            .copied()
    }

    pub fn set_state_integration_status(
        &mut self,
        state: StateId,
        status: StateIntegrationStatus,
    ) -> bool {
        let Some(slot) = self.states.integration_status.get_mut(state.0 as usize) else {
            return false;
        };
        *slot = status;
        true
    }

    /// Phase 1.1：把 `data.country_histories` 与 `data.oob_*` 应用到 World。
    ///
    /// 必须在 [`World::new`] 之后、调度器开始 tick 之前调用一次。
    /// 安全：所有引用错误（模板不存在 / 省份越界 / OOB 缺失）都只 warn 不 panic，
    /// 让 vanilla 逐步演进的数据完整性不会卡住启动。
    ///
    /// 返回 `PopulateReport`：每国应用结果汇总（用于诊断 / 集成测试）。
    pub fn populate_from_history(&mut self) -> PopulateReport {
        let mut report = PopulateReport::default();
        // 把 Arc<GameData> clone 一份给读路径，避免 self 借用冲突
        let data = self.data.clone();

        // 排序的 tag 集合保证确定性（同 World::new）
        let tags: Vec<String> = self.tag_to_country.keys().cloned().collect();
        let mut tags_sorted = tags;
        tags_sorted.sort();

        for tag in &tags_sorted {
            let cid = match self.tag_to_country.get(tag) {
                Some(&c) => c,
                None => continue,
            };
            let i = cid.0 as usize;
            let Some(hist) = data.country_histories.get(tag) else {
                continue;
            };

            // 1. ideas
            for idea in &hist.initial_ideas {
                if !self.countries.ideas[i].iter().any(|x| x == idea) {
                    self.countries.ideas[i].push(idea.clone());
                }
            }
            report.ideas_applied += hist.initial_ideas.len();

            // 2. 已完成 / 已解锁 focus
            for f in &hist.completed_focuses {
                self.countries.completed_focuses[i].insert(f.clone());
            }
            report.focuses_completed += hist.completed_focuses.len();

            // 3. research_slots / stability / war_support / ruling_party / popularities
            if let Some(n) = hist.research_slots {
                self.countries.research_slots[i] = n;
            }
            if let Some(v) = hist.stability {
                self.countries.stability[i] = v.clamp(0.0, 1.0);
            }
            if let Some(v) = hist.war_support {
                self.countries.war_support[i] = v.clamp(0.0, 1.0);
            }
            if let Some(rp) = &hist.set_ruling_party {
                self.countries.ruling_party[i] = rp.clone();
            }
            for (k, v) in &hist.party_popularities {
                self.countries.party_popularity[i].insert(k.clone(), *v);
            }

            // 4. 陆军 OOB → 师 spawn
            if let Some(oob_name) = &hist.set_oob {
                if let Some(oob) = data.oob_land.get(oob_name) {
                    let mut spawned = 0usize;
                    let templates = data.division_templates.get(tag);
                    for div in &oob.divisions {
                        let Some(tpls) = templates else {
                            report.warn_template_missing += 1;
                            break;
                        };
                        let Some(tpl_idx) = tpls.iter().position(|t| t.name == div.template_name)
                        else {
                            report.warn_template_missing += 1;
                            continue;
                        };
                        let regiments = tpls[tpl_idx].regiments.len() as f32;
                        let max_org = 30.0 + 10.0 * regiments.sqrt();
                        let max_str = 100.0 + 50.0 * regiments;
                        let location = ProvinceId(div.location);
                        let name = match div.name_order {
                            Some(n) => format!("{}. {}", n, div.template_name),
                            None => div.template_name.clone(),
                        };
                        self.divisions
                            .push(cid, location, tpl_idx as u16, max_org, max_str, name);
                        spawned += 1;
                    }
                    *report.divisions_per_country.entry(tag.clone()).or_insert(0) += spawned;
                    report.divisions_spawned += spawned;
                } else {
                    report.warn_oob_missing += 1;
                }
            }

            // 5. 海军 OOB → 舰队 + 舰只
            if let Some(naval_name) = &hist.set_naval_oob {
                if let Some(oob) = data.oob_naval.get(naval_name) {
                    let mut spawned_fleets = 0usize;
                    let mut spawned_ships = 0usize;
                    for fleet_spec in &oob.fleets {
                        let region = fleet_spec.naval_base.unwrap_or(0) as u32;
                        let fleet_idx = self.fleets.push(cid, region, fleet_spec.name.clone());
                        if let Some(port) = fleet_spec.naval_base {
                            let fi = fleet_idx as usize;
                            self.fleets.home_port[fi] = port;
                            self.fleets.repair_state[fi] = crate::NavalRepairState::InPort;
                        }
                        spawned_fleets += 1;
                        for tf in &fleet_spec.task_forces {
                            for ship in &tf.ships {
                                let class = match data.ship_classes.get(&ship.definition) {
                                    Some(c) => c,
                                    None => {
                                        report.warn_ship_class_missing += 1;
                                        continue;
                                    }
                                };
                                let max_hp = class.max_hp;
                                let max_org = class.max_organisation;
                                let ship_idx = self.ships.push(
                                    cid,
                                    crate::FleetId(fleet_idx),
                                    ship.definition.clone(),
                                    max_hp,
                                    max_org,
                                    ship.name.clone(),
                                );
                                self.fleets.ships[fleet_idx as usize].push(crate::ShipId(ship_idx));
                                spawned_ships += 1;
                                let _ = tf.location; // 当前简化，统一用 fleet.naval_base 作为 region
                            }
                        }
                    }
                    report.fleets_spawned += spawned_fleets;
                    report.ships_spawned += spawned_ships;
                } else {
                    report.warn_oob_missing += 1;
                }
            }

            // 6. 空军 OOB → 联队
            if let Some(air_name) = &hist.set_air_oob {
                if let Some(oob) = data.oob_air.get(air_name) {
                    let mut spawned = 0usize;
                    for w in &oob.wings {
                        // equipment_key 形如 "fighter_equipment_0"；映射回 aircraft 类
                        let aircraft_key = aircraft_key_from_equipment(&w.equipment_key);
                        let def = match data.aircraft.get(&aircraft_key) {
                            Some(d) => d,
                            None => {
                                report.warn_aircraft_missing += 1;
                                continue;
                            }
                        };
                        // air_wings 的 key 是 state-id；用作 region 占位（Phase 6 接战略空区）
                        let region = w.state_id as u32;
                        let wing_idx = self.air_wings.push(
                            cid,
                            aircraft_key,
                            region,
                            w.amount,
                            def.max_organisation,
                            w.name.clone().unwrap_or_else(|| "Air Wing".to_owned()),
                        );
                        let wi = wing_idx as usize;
                        self.air_wings.base_state[wi] = w.state_id;
                        self.air_wings.range_km[wi] = def.range;
                        spawned += 1;
                    }
                    report.air_wings_spawned += spawned;
                } else {
                    report.warn_oob_missing += 1;
                }
            }
        }

        // J.1.2：所有国家的 ruling_party 现已就位，统一刷新 country_leader_idx。
        self.refresh_country_leader_all();

        report
    }

    /// 重建 province→divisions 索引。应在 daily_movement_tick 之后、
    /// arbiter / occupation_tick 之前调用（一天一次）。
    pub fn rebuild_province_div_index(&mut self) {
        self.prov_div_index.clear();
        for di in 0..self.divisions.count {
            let prov = self.divisions.locations[di].0;
            self.prov_div_index
                .entry(prov)
                .or_insert_with(Vec::new)
                .push(di);
        }
    }

    /// P2.1：重建国家级运行时索引。经济、贸易、AI 和 UI 高频路径应优先使用这些索引，
    /// 避免按国家重复扫描全局州、POP、建筑和部队。
    pub fn rebuild_runtime_country_indexes(&mut self) {
        let country_count = self.countries.count;
        self.country_state_index = vec![Vec::new(); country_count];
        self.country_pop_index = vec![Vec::new(); country_count];
        self.country_building_index = vec![Vec::new(); country_count];
        self.country_division_index = vec![Vec::new(); country_count];
        self.country_fleet_index = vec![Vec::new(); country_count];
        self.country_air_wing_index = vec![Vec::new(); country_count];

        for si in 0..self.states.count {
            let owner = self.states.owners[si];
            if !owner.is_none() {
                let ci = owner.0 as usize;
                if ci < country_count {
                    self.country_state_index[ci].push(StateId(si as u16));
                }
            }
        }

        for (pi, pg) in self.countries.pops.groups.iter().enumerate() {
            let state_idx = pg.state.0 as usize;
            if state_idx >= self.states.count {
                continue;
            }
            let owner = self.states.owners[state_idx];
            if !owner.is_none() {
                let ci = owner.0 as usize;
                if ci < country_count {
                    self.country_pop_index[ci].push(pi);
                }
            }
        }

        for (bi, building) in self.countries.buildings_v6.buildings.iter().enumerate() {
            let state_idx = building.state.0 as usize;
            if state_idx >= self.states.count {
                continue;
            }
            let owner = self.states.owners[state_idx];
            if !owner.is_none() {
                let ci = owner.0 as usize;
                if ci < country_count {
                    self.country_building_index[ci].push(bi);
                }
            }
        }

        for di in 0..self.divisions.count {
            let owner = self.divisions.owners[di];
            if !owner.is_none() {
                let ci = owner.0 as usize;
                if ci < country_count {
                    self.country_division_index[ci].push(di);
                }
            }
        }

        for fi in 0..self.fleets.count {
            let owner = self.fleets.owners[fi];
            if !owner.is_none() {
                let ci = owner.0 as usize;
                if ci < country_count {
                    self.country_fleet_index[ci].push(fi);
                }
            }
        }

        for ai in 0..self.air_wings.count {
            let owner = self.air_wings.owners[ai];
            if !owner.is_none() {
                let ci = owner.0 as usize;
                if ci < country_count {
                    self.country_air_wing_index[ci].push(ai);
                }
            }
        }
        self.runtime_country_indexes_valid = true;
    }

    /// Push a POP group while keeping the country POP index current for same-tick consumers.
    pub fn push_pop_group(&mut self, pop_group: PopGroup) -> usize {
        let pop_idx = self.countries.pops.groups.len();
        let state_idx = pop_group.state.0 as usize;
        let owner = self
            .states
            .owners
            .get(state_idx)
            .copied()
            .unwrap_or(CountryId::NONE);
        self.countries.pops.groups.push(pop_group);
        if !owner.is_none() {
            let ci = owner.0 as usize;
            if ci < self.country_pop_index.len() {
                self.country_pop_index[ci].push(pop_idx);
            }
        }
        pop_idx
    }

    /// P2.1：重建商品可出口 surplus 索引，供贸易撮合按商品直接查候选出口国。
    pub fn rebuild_trade_export_surplus_index(&mut self) {
        self.trade_export_surplus_index.clear();
        for ci in 0..self.countries.market.markets.len() {
            let country = CountryId(ci as u16);
            let market = &self.countries.market.markets[ci];
            for (good_id, supply) in &market.supply {
                let demand = market.demand.get(good_id).copied().unwrap_or(0.0);
                let surplus = (*supply - demand).max(0.0);
                if surplus > 0.0 {
                    self.trade_export_surplus_index
                        .entry(good_id.clone())
                        .or_default()
                        .push((country, surplus));
                }
            }
        }
    }

    /// P2.1：合并同州、阶级、雇主和近似属性相同的 POP group，防止就业拆分长期碎片化。
    pub fn compact_similar_pop_groups(&mut self) -> usize {
        use crate::pops::PopGroup;

        let before = self.countries.pops.groups.len();
        if before <= 1 {
            return 0;
        }

        let mut buckets: HashMap<(StateId, crate::PopClass, Option<u32>, i32, i32, i32), PopGroup> =
            HashMap::with_capacity(before);
        for pg in self.countries.pops.groups.drain(..) {
            if pg.size == 0 {
                continue;
            }
            let key = (
                pg.state,
                pg.class,
                pg.employed_at.map(|id| id.0),
                (pg.wage_rm * 100.0).round() as i32,
                (pg.literacy * 1000.0).round() as i32,
                (pg.skilled_ratio * 1000.0).round() as i32,
            );
            if let Some(existing) = buckets.get_mut(&key) {
                merge_pop_group(existing, &pg);
            } else {
                buckets.insert(key, pg);
            }
        }
        self.countries.pops.groups = buckets.into_values().collect();
        self.runtime_country_indexes_valid = false;
        before.saturating_sub(self.countries.pops.groups.len())
    }

    /// 查询某省份内的所有师团下标（O(1) 查表）。
    pub fn divisions_in_province(&self, prov: ProvinceId) -> &[usize] {
        self.prov_div_index
            .get(&prov.0)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Bug #7：永久销毁一批师，并用一次性 old->new mapping 修复所有外部索引容器。
    pub fn remove_divisions(&mut self, indices: &[usize]) -> usize {
        if indices.is_empty() {
            return 0;
        }
        let old_count = self.divisions.count;
        let mut remove = vec![false; old_count];
        let mut removed = 0usize;
        for &idx in indices {
            if idx < old_count && !remove[idx] {
                remove[idx] = true;
                removed += 1;
            }
        }
        if removed == 0 {
            return 0;
        }

        let mut old_to_new = vec![None; old_count];
        let mut keep = Vec::with_capacity(old_count - removed);
        for old_idx in 0..old_count {
            if !remove[old_idx] {
                old_to_new[old_idx] = Some(keep.len());
                keep.push(old_idx);
            }
        }

        self.remap_division_references(&old_to_new);
        self.compact_divisions(&keep);
        self.rebuild_province_div_index();
        self.runtime_country_indexes_valid = false;
        removed
    }

    fn remap_division_references(&mut self, old_to_new: &[Option<usize>]) {
        self.command.div_to_corps = old_to_new
            .iter()
            .enumerate()
            .filter_map(|(old_idx, new_idx)| new_idx.map(|_| old_idx))
            .map(|old_idx| self.command.div_to_corps.get(old_idx).copied().flatten())
            .collect();

        for corps in &mut self.command.corps {
            corps.divisions = corps
                .divisions
                .iter()
                .filter_map(|&old_idx| old_to_new.get(old_idx).copied().flatten())
                .collect();
        }
        for army in &mut self.player_armies {
            army.members = army
                .members
                .iter()
                .filter_map(|&old_idx| old_to_new.get(old_idx).copied().flatten())
                .collect();
        }
        self.player_locked_divisions = self
            .player_locked_divisions
            .iter()
            .filter_map(|&old_idx| old_to_new.get(old_idx).copied().flatten())
            .collect();
    }

    fn compact_divisions(&mut self, keep: &[usize]) {
        let old = &self.divisions;
        self.divisions = DivisionStore {
            count: keep.len(),
            owners: keep.iter().map(|&i| old.owners[i]).collect(),
            locations: keep.iter().map(|&i| old.locations[i]).collect(),
            template_indices: keep.iter().map(|&i| old.template_indices[i]).collect(),
            strength: keep.iter().map(|&i| old.strength[i]).collect(),
            organisation: keep.iter().map(|&i| old.organisation[i]).collect(),
            max_organisation: keep.iter().map(|&i| old.max_organisation[i]).collect(),
            max_strength: keep.iter().map(|&i| old.max_strength[i]).collect(),
            experience: keep.iter().map(|&i| old.experience[i]).collect(),
            in_combat: keep.iter().map(|&i| old.in_combat[i]).collect(),
            general_id: keep.iter().map(|&i| old.general_id[i]).collect(),
            names: keep.iter().map(|&i| old.names[i].clone()).collect(),
            destinations: keep.iter().map(|&i| old.destinations[i]).collect(),
            move_progress: keep.iter().map(|&i| old.move_progress[i]).collect(),
            assignments: keep.iter().map(|&i| old.assignments[i].clone()).collect(),
            commands: keep.iter().map(|&i| old.commands[i].clone()).collect(),
            low_strength_days: keep.iter().map(|&i| old.low_strength_days[i]).collect(),
            last_combat_hour: keep.iter().map(|&i| old.last_combat_hour[i]).collect(),
            transport: keep.iter().map(|&i| old.transport[i].clone()).collect(),
        };
    }

    pub fn transfer_state_to_country(&mut self, state: StateId, country: CountryId) -> bool {
        let si = state.0 as usize;
        if si >= self.states.count {
            return false;
        }
        self.states.owners[si] = country;
        self.states.controllers[si] = country;
        for &prov in &self.states.provinces[si] {
            let pi = prov.0 as usize;
            if pi < self.provinces.count {
                self.provinces.owners[pi] = country;
                self.provinces.controllers[pi] = country;
            }
        }
        self.path_cache.clear();
        true
    }

    /// 推进一个游戏小时
    pub fn tick_hour(&mut self) {
        let prev_day = self.date.days_since_epoch();
        self.date.advance_hour();
        self.elapsed_hours += 1;
        let cur_day = self.date.days_since_epoch();
        if cur_day != prev_day {
            self.path_cache_day = cur_day;
        }
    }

    /// 推进一天（24 hour ticks）
    pub fn tick_day(&mut self) {
        for _ in 0..24 {
            self.tick_hour();
        }
    }

    // ─── 查询辅助 ───

    pub fn country(&self, tag: &str) -> Option<CountryId> {
        self.tag_to_country.get(tag).copied()
    }

    pub fn country_tag(&self, id: CountryId) -> Option<&str> {
        if id.is_none() {
            return None;
        }
        self.countries.tags.get(id.0 as usize).map(|s| s.as_str())
    }

    pub fn state_owner(&self, state_id: StateId) -> CountryId {
        if state_id.is_none() {
            return CountryId::NONE;
        }
        self.states
            .owners
            .get(state_id.0 as usize)
            .copied()
            .unwrap_or(CountryId::NONE)
    }

    pub fn province_owner(&self, prov: ProvinceId) -> CountryId {
        if prov.is_none() {
            return CountryId::NONE;
        }
        self.provinces
            .owners
            .get(prov.0 as usize)
            .copied()
            .unwrap_or(CountryId::NONE)
    }

    /// V6 building-based industry rollup: (civilian-like, military, shipyard levels).
    pub fn country_industry(&self, id: CountryId) -> (u32, u32, u32) {
        if id.is_none() {
            return (0, 0, 0);
        }
        let mut civilian = 0u32;
        let mut military = 0u32;
        let mut shipyards = 0u32;
        let indexed_buildings = self
            .country_building_index
            .get(id.0 as usize)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if !indexed_buildings.is_empty() {
            for &building_idx in indexed_buildings {
                let Some(building) = self.countries.buildings_v6.buildings.get(building_idx) else {
                    continue;
                };
                if building.level == 0 {
                    continue;
                }
                match building.building_def_id.as_str() {
                    "shipyard" => shipyards += building.level as u32,
                    _ if building.kind == crate::buildings_v6::BuildingKind::Military => {
                        military += building.level as u32
                    }
                    _ if building.kind == crate::buildings_v6::BuildingKind::MilitaryBase => {}
                    _ => civilian += building.level as u32,
                }
            }
            return (civilian, military, shipyards);
        }
        for building in &self.countries.buildings_v6.buildings {
            let state_idx = building.state.0 as usize;
            if state_idx >= self.states.count
                || self.states.owners[state_idx] != id
                || building.level == 0
            {
                continue;
            }
            match building.building_def_id.as_str() {
                "shipyard" => shipyards += building.level as u32,
                _ if building.kind == crate::buildings_v6::BuildingKind::Military => {
                    military += building.level as u32
                }
                _ if building.kind == crate::buildings_v6::BuildingKind::MilitaryBase => {}
                _ => civilian += building.level as u32,
            }
        }
        (civilian, military, shipyards)
    }

    pub fn state_building_levels(&self, state: StateId) -> u16 {
        if state.is_none() {
            return 0;
        }
        self.countries
            .buildings_v6
            .buildings
            .iter()
            .filter(|building| building.state == state && building.level > 0)
            .map(|building| building.level as u16)
            .sum()
    }

    /// P5：获取某国拥有的所有 state ID 列表。
    pub fn country_state_ids(&self, country: CountryId) -> Vec<StateId> {
        if let Some(states) = self.country_state_index.get(country.0 as usize) {
            if !states.is_empty() {
                return states.clone();
            }
        }
        let mut ids = Vec::new();
        for si in 0..self.states.count {
            if self.states.owners[si] == country {
                ids.push(StateId(si as u16));
            }
        }
        ids
    }

    pub fn state_population(&self, state: StateId) -> u64 {
        let state_idx = state.0 as usize;
        if state_idx < self.states.count {
            let owner = self.states.owners[state_idx];
            if let Some(pop_indices) = self.country_pop_index.get(owner.0 as usize) {
                if !pop_indices.is_empty() {
                    return pop_indices
                        .iter()
                        .filter_map(|&idx| self.countries.pops.groups.get(idx))
                        .filter(|pg| pg.state == state)
                        .map(|pg| pg.size as u64)
                        .sum();
                }
            }
        }
        self.countries
            .pops
            .groups
            .iter()
            .filter(|pg| pg.state == state)
            .map(|pg| pg.size as u64)
            .sum()
    }

    pub fn country_governed_population(&self, country: CountryId) -> u64 {
        self.country_state_ids(country)
            .iter()
            .map(|state| self.state_population(*state))
            .sum()
    }

    pub fn country_population_breakdown(&self, country: CountryId) -> PopulationBreakdown {
        if country.is_none() {
            return PopulationBreakdown::default();
        }
        let mut out = PopulationBreakdown::default();
        for state in self.country_state_ids(country) {
            let si = state.0 as usize;
            let population = self.state_population(state);
            let is_core_state = self.states.cores[si].contains(&country);
            match self.states.integration_status[si] {
                StateIntegrationStatus::Metropole if !is_core_state => out.colonial += population,
                status if status.is_domestic() => out.domestic += population,
                status if status.is_colonial_or_occupied() => out.colonial += population,
                _ => out.domestic += population,
            }
        }
        out.governed = out.domestic + out.colonial;
        out.subject = self
            .diplomacy
            .autonomy
            .values()
            .filter(|autonomy| autonomy.master == country)
            .map(|autonomy| self.country_governed_population(autonomy.subject))
            .sum();
        out.imperial = out.governed + out.subject;
        out
    }

    /// P5：从 PopGroup 派生 manpower（替代已删除的 Vec<u64> 字段）。
    /// 如果没有 PopGroup 数据（旧路径并行模式 A.6），回退到州 manpower_pool 累加。
    pub fn manpower(&self, country: CountryId) -> u64 {
        if country.is_none() {
            return 0;
        }
        let state_ids = self.country_state_ids(country);
        // 如果有 PopGroup 数据，从 Soldier 阶级派生
        let soldier_pop: u64 = self
            .countries
            .pops
            .groups
            .iter()
            .filter(|p| {
                p.class == crate::pops::PopClass::Soldier
                    && state_ids.contains(&p.state)
                    && p.employed_at.is_none()
            })
            .map(|p| p.size as u64)
            .sum();
        let has_country_pops = self
            .countries
            .pops
            .groups
            .iter()
            .any(|p| state_ids.contains(&p.state));
        if has_country_pops {
            soldier_pop
        } else {
            // A.6 fallback：该国尚未注入 PopGroup 时回退到州 manpower_pool 累加。
            // V6 目前只给少数国家注入 POP；不能因为别国有 POP 就让本国人力恒为 0。
            let mut total: u64 = 0;
            for si in 0..self.states.count {
                if self.states.owners[si] == country {
                    total += self.states.manpower_pool[si] as u64;
                }
            }
            total
        }
    }

    /// V6: no-op. Building-based calculations replace the old cached factory counts.
    /// Kept as a no-op because many callers still invoke it after state ownership changes.
    pub fn recalc_country_caches(&mut self) {}

    /// 标记某国家完成某科技：写入 completed_techs，并把 enable_equipments/subunits/buildings
    /// 加入解锁集合。
    ///
    /// 返回 `true` 表示成功加入；`false` 表示该 tech 已在该国 completed_techs 中（重复完成）。
    pub fn mark_tech_completed(&mut self, country: CountryId, tech_key: &str) -> bool {
        if country.is_none() {
            return false;
        }
        let i = country.0 as usize;
        if i >= self.countries.count {
            return false;
        }
        if self.countries.completed_techs[i]
            .iter()
            .any(|t| t == tech_key)
        {
            return false;
        }
        self.countries.completed_techs[i].push(tech_key.to_owned());

        if let Some(tech) = self.data.technologies.get(tech_key) {
            for eq in &tech.enable_equipments {
                self.countries.unlocked_equipments[i].insert(eq.clone());
            }
            for su in &tech.enable_subunits {
                self.countries.unlocked_subunits[i].insert(su.clone());
            }
            for b in &tech.enable_building {
                self.countries.unlocked_buildings[i].insert(b.clone());
            }
        }
        true
    }

    /// J.1.2 + J.1.8：根据当前 `ruling_party` 重新挑选某国 country_leader。
    ///
    /// - 在 `ruling_party` 变化（`set_politics` effect / `swap_ruling_party`
    ///   focus / 和会推翻政府）后必须调用，否则面板顶部肖像会延迟一帧。
    /// - 多人时取 `data.characters_by_tag` 中第一个意识形态匹配者。
    /// - 没有匹配的角色时设 `None`（UI 走 fallback）。
    pub fn refresh_country_leader(&mut self, country: CountryId) {
        if country.is_none() {
            return;
        }
        let i = country.0 as usize;
        if i >= self.countries.count {
            return;
        }
        let tag_str = self.countries.tags[i].clone();
        if tag_str.is_empty() {
            self.countries.country_leader_idx[i] = None;
            return;
        }
        let ruling = self.countries.ruling_party[i].clone();
        let tag = hoi4_data::CountryTag::new(&tag_str);
        // 顶层意识形态 → 子类型集合
        let subtypes: Vec<String> = if ruling.is_empty() {
            Vec::new()
        } else {
            self.data
                .ideologies
                .get(&ruling)
                .map(|ide| ide.types.clone())
                // 兜底：若 `ruling_party` 字面就是子类型本身（罕见但 vanilla 老存档存在），直接用它
                .unwrap_or_else(|| vec![ruling.clone()])
        };
        let recruited = self
            .data
            .country_histories
            .get(&tag_str)
            .map(|hist| hist.recruited_characters.as_slice())
            .unwrap_or(&[]);
        let idx = hoi4_data::select_country_leader_with_recruits(
            &self.data.characters,
            &self.data.characters_by_tag,
            &tag,
            &subtypes,
            recruited,
        );
        self.countries.country_leader_idx[i] = idx.map(|v| v as u32);
    }

    /// J.1.2：对所有国家批量调用 [`Self::refresh_country_leader`]。
    /// 在 `World::new` + `populate_from_history` 链路尾部调用一次。
    pub fn refresh_country_leader_all(&mut self) {
        for i in 0..self.countries.count {
            self.refresh_country_leader(CountryId(i as u16));
        }
    }

    /// J.1.5：取某国 country_leader 角色定义（如果有）。
    pub fn country_leader(&self, country: CountryId) -> Option<&hoi4_data::CharacterDef> {
        if country.is_none() {
            return None;
        }
        let i = country.0 as usize;
        let idx = self
            .countries
            .country_leader_idx
            .get(i)
            .copied()
            .flatten()?;
        self.data.characters.get(idx as usize)
    }

    /// J.5b Part 3: Dynamically spawn a new country at runtime (e.g. Nationalist Spain).
    /// Returns the new CountryId, or existing one if tag already exists.
    pub fn spawn_country(&mut self, tag: &str, color: [u8; 3], ruling_party: &str) -> CountryId {
        if let Some(&existing) = self.tag_to_country.get(tag) {
            return existing;
        }
        if let Some(idx) = self
            .countries
            .tags
            .iter()
            .position(|existing| existing == tag)
        {
            let cid = CountryId(idx as u16);
            self.countries.colors[idx] = color;
            self.countries.ruling_party[idx] = ruling_party.to_owned();
            self.countries.at_war[idx] = false;
            self.diplomacy.annexed_countries.remove(&cid);
            self.tag_to_country.insert(tag.to_owned(), cid);
            return cid;
        }
        let idx = self.countries.count;
        self.countries.count += 1;
        self.countries.tags.push(tag.to_owned());
        self.countries.colors.push(color);
        self.countries.capitals.push(StateId::NONE);
        self.countries.political_power.push(0.0);
        self.countries.stability.push(0.5);
        self.countries.war_support.push(0.5);
        self.countries.ruling_party.push(ruling_party.to_owned());
        self.countries.fuel.push(0.0);
        self.countries.fuel_capacity.push(1000.0);
        self.countries.army_xp.push(0.0);
        self.countries.navy_xp.push(0.0);
        self.countries.air_xp.push(0.0);
        self.countries.at_war.push(false);
        self.countries.completed_techs.push(Vec::new());
        self.countries.ideas.push(Vec::new());
        self.countries.research_slots.push(2);
        self.countries
            .tech_bonus
            .push(std::collections::HashMap::new());
        self.countries.conscription_max_ratio.push(0.0);
        self.countries.conscription_recruit_speed_mult.push(0.0);
        self.countries
            .unlocked_equipments
            .push(std::collections::HashSet::new());
        self.countries
            .unlocked_subunits
            .push(std::collections::HashSet::new());
        self.countries
            .unlocked_buildings
            .push(std::collections::HashSet::new());
        self.countries
            .party_popularity
            .push(std::collections::HashMap::new());
        self.countries.current_focus.push(None);
        self.countries.focus_progress.push(0.0);
        self.countries
            .completed_focuses
            .push(std::collections::HashSet::new());
        self.countries.country_leader_idx.push(None);
        self.countries
            .variables
            .push(std::collections::HashMap::new());
        self.countries
            .characters
            .push(std::collections::HashMap::new());
        self.countries.private_investment_pool_rm.push(0.0);
        for account_kind in [
            InvestmentAccountKind::Private,
            InvestmentAccountKind::Cartel,
            InvestmentAccountKind::StateDevelopmentBank,
            InvestmentAccountKind::ColonialExtraction,
            InvestmentAccountKind::ForeignCapital,
        ] {
            self.countries.investment_accounts.push(InvestmentAccount {
                country: CountryId(idx as u16),
                account_kind,
                balance_rm: 0.0,
                last_income_rm: 0.0,
                last_spent_rm: 0.0,
            });
        }
        self.countries
            .v6_events_fired
            .push(std::collections::HashSet::new());
        self.countries.treasury.ensure_capacity(idx + 1);
        self.countries.market.ensure_capacity(idx + 1);
        self.countries.law_store.ensure_capacity(idx + 1);
        let cid = CountryId(idx as u16);
        self.tag_to_country.insert(tag.to_owned(), cid);
        cid
    }
}

fn merge_pop_group(target: &mut crate::pops::PopGroup, incoming: &crate::pops::PopGroup) {
    let old_size = target.size as f32;
    let add_size = incoming.size as f32;
    let total = old_size + add_size;
    if total <= 0.0 {
        return;
    }
    let weight = |a: f32, b: f32| (a * old_size + b * add_size) / total;
    target.size = target.size.saturating_add(incoming.size);
    target.wage_rm = weight(target.wage_rm, incoming.wage_rm);
    target.tax_burden = weight(target.tax_burden, incoming.tax_burden);
    target.income_rm = weight(target.income_rm, incoming.income_rm);
    target.tax_paid_rm = weight(target.tax_paid_rm, incoming.tax_paid_rm);
    target.disposable_income_rm =
        weight(target.disposable_income_rm, incoming.disposable_income_rm);
    target.basic_consumption_budget = weight(
        target.basic_consumption_budget,
        incoming.basic_consumption_budget,
    );
    target.satisfaction_law_modifier = weight(
        target.satisfaction_law_modifier,
        incoming.satisfaction_law_modifier,
    );
    target.loyalty_coefficient = weight(target.loyalty_coefficient, incoming.loyalty_coefficient);
    target.loyalty_decay_mult = weight(target.loyalty_decay_mult, incoming.loyalty_decay_mult);
    target.satisfaction = weight(target.satisfaction, incoming.satisfaction);
    target.political_loyalty = weight(target.political_loyalty, incoming.political_loyalty);
    target.literacy = weight(target.literacy, incoming.literacy);
    target.skilled_ratio = weight(target.skilled_ratio, incoming.skilled_ratio);
    target.standard_of_living = weight(target.standard_of_living, incoming.standard_of_living);
    target.needs_fulfillment = weight(target.needs_fulfillment, incoming.needs_fulfillment);
    target.essential_needs_fulfillment = weight(
        target.essential_needs_fulfillment,
        incoming.essential_needs_fulfillment,
    );
    target.normal_needs_fulfillment = weight(
        target.normal_needs_fulfillment,
        incoming.normal_needs_fulfillment,
    );
    target.luxury_needs_fulfillment = weight(
        target.luxury_needs_fulfillment,
        incoming.luxury_needs_fulfillment,
    );
    target.radicalism = weight(target.radicalism, incoming.radicalism);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    use hoi4_data::{Color, Country, CountryTag, State};
    use hoi4_map::{GameMap, Heightmap, ProvinceMap, TerrainBitmap, TerrainCatalog};

    fn test_map(province_count: usize) -> Arc<GameMap> {
        Arc::new(GameMap {
            definitions: vec![None; province_count],
            rgb_to_id: HashMap::new(),
            province_map: ProvinceMap {
                width: 1,
                height: 1,
                pixels: vec![0],
            },
            adjacencies: vec![Vec::new(); province_count],
            special_adjacencies: Vec::new(),
            heightmap: Heightmap {
                width: 1,
                height: 1,
                pixels: vec![0],
            },
            terrain_bmp: TerrainBitmap {
                width: 1,
                height: 1,
                pixels: vec![0],
                palette: [[0; 3]; 256],
            },
            terrain_catalog: TerrainCatalog::default(),
            tree_definition_bmp: None,
            tree_indices: HashSet::new(),
        })
    }

    fn test_data() -> Arc<GameData> {
        let mut data = GameData::default();
        let ger = CountryTag::new("GER");
        let sov = CountryTag::new("SOV");
        for (tag, capital, color) in [
            (
                ger.clone(),
                1,
                Color {
                    r: 80,
                    g: 80,
                    b: 80,
                },
            ),
            (
                sov.clone(),
                2,
                Color {
                    r: 160,
                    g: 20,
                    b: 20,
                },
            ),
        ] {
            data.countries.insert(
                tag.clone(),
                Country {
                    tag,
                    color,
                    graphical_culture: "western_european_gfx".to_owned(),
                    capital,
                    ruling_party: "neutrality".to_owned(),
                    technologies: Vec::new(),
                },
            );
        }
        data.states.push(State {
            id: 1,
            name: "State One".to_owned(),
            manpower: 1000,
            owner: ger.clone(),
            cores: vec![ger],
            provinces: vec![1, 2],
            category: "city".to_owned(),
            infrastructure: 0,
            victory_points: Vec::new(),
            resources: Vec::new(),
        });
        data.states.push(State {
            id: 2,
            name: "State Two".to_owned(),
            manpower: 1000,
            owner: sov.clone(),
            cores: vec![sov],
            provinces: vec![3],
            category: "city".to_owned(),
            infrastructure: 0,
            victory_points: Vec::new(),
            resources: Vec::new(),
        });
        Arc::new(data)
    }

    #[test]
    fn populate_from_history_applies_german_initial_ideas() {
        let mut data = (*test_data()).clone();
        data.country_histories.insert(
            "GER".to_owned(),
            hoi4_data::CountryHistory {
                tag: "GER".to_owned(),
                initial_ideas: vec!["german_starting_spirit".to_owned()],
                ..Default::default()
            },
        );
        data.country_histories.insert(
            "SOV".to_owned(),
            hoi4_data::CountryHistory {
                tag: "SOV".to_owned(),
                initial_ideas: vec!["soviet_starting_spirit".to_owned()],
                ..Default::default()
            },
        );

        let mut world = World::new(test_map(5), Arc::new(data));
        let report = world.populate_from_history();
        let ger = world.country("GER").unwrap();
        let sov = world.country("SOV").unwrap();

        assert_eq!(
            world.countries.ideas[ger.0 as usize],
            vec!["german_starting_spirit".to_owned()]
        );
        assert_eq!(
            world.countries.ideas[sov.0 as usize],
            vec!["soviet_starting_spirit".to_owned()]
        );
        assert_eq!(report.ideas_applied, 2);
    }

    #[test]
    fn remove_divisions_remaps_all_external_indices() {
        let mut world = World::new(test_map(5), test_data());
        let ger = world.country("GER").unwrap();
        for idx in 0..6 {
            world
                .divisions
                .push(ger, ProvinceId(1), 0, 10.0, 1.0, format!("div {idx}"));
        }
        world.command = crate::command::CommandHierarchy::auto_group(
            world.divisions.count,
            &world.divisions.owners,
        );
        world.player_armies.push(crate::frontline::PlayerArmy {
            id: crate::frontline::ArmyId(1),
            name: "army".to_owned(),
            owner: ger,
            commander: None,
            members: vec![1, 2, 4, 5],
            order: None,
        });
        world.player_locked_divisions.insert(2);
        world.player_locked_divisions.insert(5);

        assert_eq!(world.remove_divisions(&[1, 4]), 2);

        assert_eq!(world.divisions.count, 4);
        assert_eq!(
            world.divisions.names,
            vec!["div 0", "div 2", "div 3", "div 5"]
        );
        assert_eq!(world.command.div_to_corps.len(), world.divisions.count);
        for (div_idx, corps_idx) in world.command.div_to_corps.iter().enumerate() {
            if let Some(corps_idx) = corps_idx {
                assert!(world.command.corps[*corps_idx].divisions.contains(&div_idx));
            }
        }
        for (corps_idx, corps) in world.command.corps.iter().enumerate() {
            for &div_idx in &corps.divisions {
                assert_eq!(world.command.div_to_corps[div_idx], Some(corps_idx));
            }
        }
        assert_eq!(world.player_armies[0].members, vec![1, 3]);
        assert_eq!(world.player_locked_divisions, HashSet::from([1, 3]));
    }

    #[test]
    fn transfer_state_updates_province_owner_and_controller() {
        let mut world = World::new(test_map(5), test_data());
        let sov = world.country("SOV").unwrap();

        assert!(world.transfer_state_to_country(StateId(0), sov));

        assert_eq!(world.states.owners[0], sov);
        assert_eq!(world.states.controllers[0], sov);
        for prov in [ProvinceId(1), ProvinceId(2)] {
            let pi = prov.0 as usize;
            assert_eq!(world.provinces.owners[pi], sov);
            assert_eq!(world.provinces.controllers[pi], sov);
        }
    }

    #[test]
    fn spawn_country_extends_per_country_runtime_stores() {
        let mut world = World::new(test_map(5), test_data());
        let cid = world.spawn_country("MOL", [120, 80, 40], "neutrality");
        let count = world.countries.count;

        assert_eq!(world.countries.tags[cid.0 as usize], "MOL");
        assert_eq!(world.countries.private_investment_pool_rm.len(), count);
        assert_eq!(world.countries.v6_events_fired.len(), count);
        assert_eq!(world.countries.treasury.treasuries.len(), count);
        assert_eq!(world.countries.market.markets.len(), count);
        assert_eq!(world.countries.law_store.law_sets.len(), count);

        let account_count = world
            .countries
            .investment_accounts
            .iter()
            .filter(|account| account.country == cid)
            .count();
        assert_eq!(account_count, 5);
    }

    #[test]
    fn spawn_country_revives_removed_tag_without_duplicate_country() {
        let mut world = World::new(test_map(5), test_data());
        let cze = world.spawn_country("CZE", [54, 167, 156], "democratic");
        let count = world.countries.count;
        world.tag_to_country.remove("CZE");
        world.diplomacy.annexed_countries.insert(cze);

        let revived = world.spawn_country("CZE", [54, 167, 156], "fascism");

        assert_eq!(revived, cze);
        assert_eq!(world.countries.count, count);
        assert_eq!(world.country("CZE"), Some(cze));
        assert_eq!(world.countries.ruling_party[cze.0 as usize], "fascism");
        assert!(!world.diplomacy.annexed_countries.contains(&cze));
    }
}
