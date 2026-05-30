//! J.5b Part 2+3: Situation system — persistent world-level events with progress bars,
//! map effects, and end-of-situation callbacks.

use hoi4_state::{CountryId, PopClass, World};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn normalize_equipment_id(id: &str) -> String {
    let key = id.to_ascii_lowercase();
    match key.as_str() {
        "infantry_equipment"
        | "infantry_equipment_0"
        | "infantry_equipment_1"
        | "infantry_equipment_2"
        | "infantry_equipment_3" => "infantry_equipment".to_owned(),
        "support_equipment" | "support_equipment_1" => "support_equipment".to_owned(),
        "artillery"
        | "artillery_equipment"
        | "artillery_equipment_1"
        | "artillery_equipment_2"
        | "artillery_equipment_3" => "artillery".to_owned(),
        "anti_tank"
        | "anti_tank_equipment"
        | "anti_tank_equipment_1"
        | "anti_tank_equipment_2"
        | "anti_tank_equipment_3" => "anti_tank".to_owned(),
        "anti_air"
        | "anti_air_equipment"
        | "anti_air_equipment_1"
        | "anti_air_equipment_2"
        | "anti_air_equipment_3" => "anti_air".to_owned(),
        "motorized" | "motorized_equipment" | "motorized_equipment_1" => "motorized".to_owned(),
        "mechanized"
        | "mechanized_equipment"
        | "mechanized_equipment_1"
        | "mechanized_equipment_2"
        | "mechanized_equipment_3" => "mechanized".to_owned(),
        "convoy" | "convoy_1" => "convoy".to_owned(),
        "train" | "train_equipment" | "train_equipment_1" => "train".to_owned(),
        _ if key.contains("tank")
            || key.contains("armor")
            || key.contains("armour")
            || key.contains("chassis") =>
        {
            "armor".to_owned()
        }
        _ if key.contains("fighter")
            || key.contains("bomber")
            || key.contains("cas")
            || key.contains("aircraft")
            || key.contains("plane") =>
        {
            "aircraft".to_owned()
        }
        _ if key.contains("ship") || key.contains("naval") => "naval_vessel".to_owned(),
        _ => id.to_owned(),
    }
}

/// Effects that a situation can trigger on the world.
///
/// 设计原则：局势效果**只**在事件点触发（开战、分裂、停战），不每天偷偷改地图。
/// 真实战争结果由 `hoi4-ai` / `hoi4-logic::military` 驱动；本枚举不再包含
/// 「每天累加进度然后转地」式的伪驱动效果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SituationEffect {
    /// Split a country: create rebel_tag, give it ~half of source_tag's states, create war.
    /// Used for civil wars (e.g. Spanish Civil War).
    SplitCountry {
        source_tag: String,
        rebel_tag: String,
        rebel_color: [u8; 3],
        rebel_party: String,
        fraction: f32,
    },
    /// Split a country using explicit game state ids for the rebel side.
    /// This is for civil wars with fixed historical opening fronts, where array-order splits are wrong.
    SplitCountryByStates {
        source_tag: String,
        rebel_tag: String,
        rebel_color: [u8; 3],
        rebel_party: String,
        rebel_states: Vec<u16>,
        manpower_fraction: f32,
    },
    /// Phase 1: 把 source 在 rebel 已得到的州里的师团 / 空军 / 舰队转给 rebel；
    /// 同时把 source 的装备储备按 fraction 切给 rebel。
    /// SCW 必须在 SplitCountry **之后**调用，确保 rebel 拥有自己境内的部队。
    SplitDivisions {
        source_tag: String,
        rebel_tag: String,
        fraction: f32,
    },
    /// Create a war between two tags.
    CreateWar {
        attacker: String,
        defender: String,
    },
    /// Annex a country (all states transfer to annexer). 仅在剧本明确"无血吞并"时使用
    /// （如波罗的海三国）。普通战争结果走 `hoi4-logic::diplomacy::peace` 而非此效果。
    AnnexCountry {
        annexer: String,
        target: String,
    },
    /// Fire a global news event (trigger event by id).
    TriggerEvent(String),
    /// Add an idea to a country.
    AddIdea {
        country: String,
        idea: String,
    },
    /// Add stability to a country.
    AddStability {
        country: String,
        amount: f32,
    },
    /// Add war support to a country.
    AddWarSupport {
        country: String,
        amount: f32,
    },
    /// Add opinion between two countries.
    AddOpinion {
        country_a: String,
        country_b: String,
        amount: i16,
    },
    CreateFaction {
        leader: String,
        name: String,
    },
    AddToFaction {
        faction_leader: String,
        member: String,
    },
    AddWarParticipant {
        war_leader: String,
        participant: String,
    },
    GrantMilitaryAccess {
        grantor: String,
        grantee: String,
    },
    SetAutonomy {
        master: String,
        subject: String,
        level: String,
    },
    TransferState {
        state: u16,
        owner: String,
    },
    AddCore {
        state: u16,
        country: String,
    },
    /// Set a country flag.
    SetCountryFlag {
        country: String,
        flag: String,
    },

    /// P0.11：设置某国在某场战争中的参战策略。
    /// Delayed = 阵营成员但不自动参战，需要事件或脚本显式触发。
    /// Forbidden = 阵营成员但禁止参战。
    SetWarJoinPolicy {
        war_leader: String,
        country: String,
        policy: String,
    },

    /// P0.11：将延迟参战的国家正式加入战争（与 war_leader 同侧）。
    AddDelayedWarParticipant {
        war_leader: String,
        participant: String,
    },

    /// Phase 4: 把 `from` 国的 N 个师按 `template_priority` 优先级"借"给 `to` 国。
    /// 借出的师立即改 owner，并被 teleport 到 `to` 国首都所在省（如有）。
    /// MVP 不实现归还逻辑（战争结束师就归 `to` 永久持有）。
    LendDivisions {
        from: String,
        to: String,
        count: u32,
        template_priority: Vec<String>,
    },

    /// Phase 4: 从 `from` 国的 stockpile 划拨指定 equipment 到 `to` 国 stockpile。
    /// 数量不足则按现有量发送（不阻塞）。
    SendEquipment {
        from: String,
        to: String,
        equipment: String,
        amount: f32,
    },

    /// Phase 4: 把 `army_xp` / `air_xp` 直接加到 `to` 国（来源不计成本）。
    SendXpBuff {
        to: String,
        army_xp: f32,
        air_xp: f32,
    },

    /// Phase 5: 为 `tag` 在其所有与敌对国接壤的前线省份按密度生成师团，
    /// 不从 source 分流，而是独立生成新师团。
    /// `density` = 每多少个前线省生成 1 个师（1 = 每省 1 师，3 = 每 3 省 1 师）。
    /// `enemy_tag` 为敌对国 tag，用于判断前线省份。
    SpawnFrontlineDivisions {
        tag: String,
        enemy_tag: String,
        density: u32,
    },
    SpawnDivisionsInStates {
        tag: String,
        states: Vec<u16>,
        count_per_state: u32,
        template_priority: Vec<String>,
    },
}

/// Progress source: how `ActiveSituation::progress` is computed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProgressSource {
    /// 旧行为：进度只在 `intervene` / 外部 setter 中变动（不每日自增）。
    /// 用于无法从战场判断的事件（外交压力、政治剧情等）。
    Manual,
    /// Phase 2: 进度从战场反推。每天遍历战区州，计算每方控制的州数比例。
    /// `side_country_tags[i]` = 第 i 方涉及的国家 tag 列表（多 tag 用于阵营战）。
    /// 战区州集合 = 局势开始时所有相关国家拥有 + 控制的州的并集。
    TerritorialControl { side_country_tags: Vec<Vec<String>> },
}

impl Default for ProgressSource {
    fn default() -> Self {
        ProgressSource::Manual
    }
}

/// A situation definition (static data, loaded at startup).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SituationDef {
    pub id: String,
    pub title: String,
    pub description: String,
    /// Two sides competing
    pub sides: Vec<SituationSide>,
    /// Available interventions
    pub interventions: Vec<Intervention>,
    /// 【已废弃 — Phase 0 重构】per-day base progress increment.
    /// 进度现在由战场状态反推（Phase 2 引入），不再每天自增。
    /// 字段保留是为了向后兼容已有 def 实例化代码；新代码不应依赖。
    #[deprecated(note = "progress is now derived from battlefield state, not auto-incremented")]
    pub base_progress: Vec<f32>,
    /// Phase 2: 进度来源。默认 `Manual`（保持旧行为）。
    /// 战争类局势（SCW、意-埃）应配置为 `TerritorialControl`。
    pub progress_source: ProgressSource,
    /// Milestones: (side_index, progress_threshold, event_id_to_fire)
    pub milestones: Vec<(usize, f32, String)>,
    /// Effects to run when a milestone is reached (parallel to milestones vec)
    pub milestone_effects: Vec<Vec<SituationEffect>>,
    /// Start date (year, month, day). (0,0,0) = always active from game start.
    pub start_date: (u16, u8, u8),
    /// End date (auto-end if reached) — 用于"超时仲裁"，不再用进度满 100 提前结束
    pub end_date: (u16, u8, u8),
    /// Effects to run when the situation starts.
    pub on_start: Vec<SituationEffect>,
    /// Effects to run when the situation ends, indexed by winner side.
    /// on_end[0] = effects if side 0 wins, on_end[1] = effects if side 1 wins.
    pub on_end: Vec<Vec<SituationEffect>>,
}

/// One side in a situation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SituationSide {
    pub id: String,
    pub name: String,
    pub color: [u8; 3],
    pub initial_progress: f32,
    pub default_supporters: Vec<String>,
}

/// An intervention a country can perform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intervention {
    pub id: String,
    pub name: String,
    pub side_index: usize,
    /// Cost: political_power
    pub cost_pp: f32,
    /// Cost: manpower
    pub cost_manpower: u64,
    /// Cost: equipment (key, amount)
    pub cost_equipment: Vec<(String, f32)>,
    /// Cooldown in days
    pub cooldown_days: u32,
    /// Progress boost to the supported side
    pub progress_boost: f32,
    /// XP rewards
    pub army_xp: f32,
    pub air_xp: f32,
    /// Phase 4: 实际军事 / 经济效果。在 `intervene` 成功扣费后追加到 pending_effects，
    /// 由 app 层应用到 World（spawn 借出师团 / 划装备 / +XP 等）。
    /// 注意：`{caller}` 占位符在效果被 enqueue 时会被替换为调用国 tag。
    pub effects: Vec<SituationEffect>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SituationDb {
    pub situations: Vec<SituationDef>,
}

impl SituationDb {
    pub fn from_ron(s: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(s)
    }

    pub fn validate(&self) -> Vec<SituationValidationError> {
        let mut errors = Vec::new();
        if self.situations.is_empty() {
            errors.push(SituationValidationError::EmptyDb);
            return errors;
        }

        let mut seen = std::collections::HashSet::new();
        for def in &self.situations {
            if !seen.insert(def.id.as_str()) {
                errors.push(SituationValidationError::DuplicateId(def.id.clone()));
            }
        }
        errors
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SituationValidationError {
    EmptyDb,
    DuplicateId(String),
}

impl std::fmt::Display for SituationValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyDb => write!(f, "situation db is empty"),
            Self::DuplicateId(id) => write!(f, "duplicate situation id: {id}"),
        }
    }
}

/// A record of an intervention performed by a country.
#[derive(Debug, Clone)]
pub struct InterventionLog {
    pub country_tag: String,
    pub intervention_name: String,
    pub side_name: String,
    pub progress_boost: f32,
    pub army_xp: f32,
    pub air_xp: f32,
}

/// Runtime state for an active situation.
#[derive(Debug, Clone)]
pub struct ActiveSituation {
    pub def_id: String,
    /// Progress per side (index matches def.sides)
    pub progress: Vec<f32>,
    /// Supporters per side: side_index -> list of country tags
    pub supporters: Vec<Vec<CountryId>>,
    /// Cooldowns: (country_id, intervention_id) -> days remaining
    pub cooldowns: HashMap<(u16, String), u32>,
    /// Milestones already triggered
    pub triggered_milestones: Vec<usize>,
    /// Whether this situation has ended
    pub ended: bool,
    /// Winner side index (set when ended)
    pub winner: Option<usize>,
    /// Whether on_start effects have been applied
    pub started: bool,
    /// Log of all interventions performed (for situation panel display)
    pub intervention_log: Vec<InterventionLog>,
    /// Phase 2: 战区州集合（开战时一次性计算，后续 territorial 进度按这个范围算）。
    /// 为空表示尚未初始化或不是 territorial 类型。
    pub theater_states: Vec<u16>,
}

/// The situation manager — holds all situation defs and active instances.
#[derive(Debug, Clone, Default)]
pub struct SituationState {
    pub defs: Vec<SituationDef>,
    pub active: Vec<ActiveSituation>,
    /// Pending effects to be applied by the app layer (since we can't mutate World here).
    pub pending_effects: Vec<SituationEffect>,
}

impl SituationState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a situation definition.
    pub fn add_def(&mut self, def: SituationDef) {
        self.defs.push(def);
    }

    /// Check if any situation should start based on current date.
    pub fn check_start(&mut self, world: &World) {
        let (y, m, d) = (world.date.year, world.date.month, world.date.day);
        for def in &self.defs {
            // Already active?
            if self.active.iter().any(|a| a.def_id == def.id) {
                continue;
            }
            let (sy, sm, sd) = def.start_date;
            let should_start = if sy == 0 && sm == 0 && sd == 0 {
                true // always-true start
            } else {
                y > sy || (y == sy && m > sm) || (y == sy && m == sm && d >= sd)
            };
            if should_start {
                println!("[situation] STARTING '{}' on {y}.{m}.{d}", def.id);
                let mut progress = Vec::new();
                let mut supporters = Vec::new();
                for side in &def.sides {
                    progress.push(side.initial_progress);
                    let sups: Vec<CountryId> = side
                        .default_supporters
                        .iter()
                        .filter_map(|tag| world.country(tag))
                        .collect();
                    supporters.push(sups);
                }
                // Phase 2: 计算战区州集合（仅 TerritorialControl）
                let theater_states = match &def.progress_source {
                    ProgressSource::TerritorialControl { side_country_tags } => {
                        let mut set: std::collections::HashSet<u16> =
                            std::collections::HashSet::new();
                        for side_tags in side_country_tags {
                            for tag in side_tags {
                                if let Some(cid) = world.country(tag) {
                                    for si in 0..world.states.count {
                                        if world.states.owners[si] == cid
                                            || world.states.controllers[si] == cid
                                        {
                                            set.insert(si as u16);
                                        }
                                    }
                                }
                            }
                        }
                        let mut v: Vec<u16> = set.into_iter().collect();
                        v.sort_unstable();
                        v
                    }
                    ProgressSource::Manual => Vec::new(),
                };
                // Queue on_start effects
                self.pending_effects.extend(def.on_start.clone());
                self.active.push(ActiveSituation {
                    def_id: def.id.clone(),
                    progress,
                    supporters,
                    cooldowns: HashMap::new(),
                    triggered_milestones: Vec::new(),
                    ended: false,
                    winner: None,
                    started: true,
                    intervention_log: Vec::new(),
                    theater_states,
                });
            }
        }
    }

    /// Daily tick: 检查 milestone（基于外部写入的 progress）+ end_date 仲裁
    /// + Phase 2 territorial 进度反推 + Phase 3 军事胜利判定。
    ///
    /// Phase 0 重构后，progress **不再**在此函数自增；它由 Phase 2
    /// `recompute_territorial_progress` 从战场状态反推后写入。
    /// 100% 自动结束也已删除——真正的胜利由军事系统判定（Phase 3）。
    pub fn daily_tick(&mut self, world: &World) {
        // Phase 2: 先按战场状态反推 territorial 进度
        self.recompute_territorial_progress(world);

        for active in self.active.iter_mut() {
            if active.ended {
                continue;
            }

            let Some(def) = self.defs.iter().find(|d| d.id == active.def_id) else {
                continue;
            };

            // Phase 3: 军事胜利判定 — 优先于超时仲裁
            if let ProgressSource::TerritorialControl { side_country_tags } = &def.progress_source {
                if let Some(winner) = check_military_victory(world, side_country_tags) {
                    active.ended = true;
                    active.winner = Some(winner);
                    println!(
                        "[situation] '{}' ENDED by military victory, winner={}, progress={:?}",
                        active.def_id, winner, active.progress
                    );
                    if winner < def.on_end.len() {
                        self.pending_effects.extend(def.on_end[winner].clone());
                    }
                    continue;
                }
            }

            // end_date 仲裁：超时按当前进度判赢家（兜底，避免局势永久挂着）
            let (ey, em, ed) = def.end_date;
            if world.date.year > ey
                || (world.date.year == ey && world.date.month > em)
                || (world.date.year == ey && world.date.month == em && world.date.day >= ed)
            {
                let winner = active
                    .progress
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(i, _)| i);
                active.ended = true;
                active.winner = winner;
                println!(
                    "[situation] '{}' ENDED by date, winner={:?}, progress={:?}",
                    active.def_id, winner, active.progress
                );
                if let Some(wi) = winner {
                    if wi < def.on_end.len() {
                        self.pending_effects.extend(def.on_end[wi].clone());
                    }
                }
                continue;
            }

            // 检查里程碑（仅触发新闻 / AddIdea 等"软效果"，不再做地图变更）
            for (mi, (side_idx, threshold, _event_id)) in def.milestones.iter().enumerate() {
                if active.triggered_milestones.contains(&mi) {
                    continue;
                }
                if let Some(&prog) = active.progress.get(*side_idx) {
                    if prog >= *threshold {
                        active.triggered_milestones.push(mi);
                        if mi < def.milestone_effects.len() {
                            self.pending_effects
                                .extend(def.milestone_effects[mi].clone());
                        }
                    }
                }
            }

            // 冷却递减（按天）
            active.cooldowns.retain(|_, days| {
                *days = days.saturating_sub(1);
                *days > 0
            });
        }
    }

    /// Phase 2: 对所有 TerritorialControl 类型的局势，基于战区州的当前 controller
    /// 反推每方的 progress（占总州数百分比）。
    pub fn recompute_territorial_progress(&mut self, world: &World) {
        for active in self.active.iter_mut() {
            if active.ended {
                continue;
            }
            let Some(def) = self.defs.iter().find(|d| d.id == active.def_id) else {
                continue;
            };
            let ProgressSource::TerritorialControl { side_country_tags } = &def.progress_source
            else {
                continue;
            };
            if active.theater_states.is_empty() {
                continue;
            }

            // 把 tag 转 CountryId
            let side_cids: Vec<Vec<CountryId>> = side_country_tags
                .iter()
                .map(|tags| tags.iter().filter_map(|t| world.country(t)).collect())
                .collect();

            let total = active.theater_states.len() as f32;
            let mut counts = vec![0u32; side_cids.len()];
            for &si_raw in &active.theater_states {
                let si = si_raw as usize;
                if si >= world.states.count {
                    continue;
                }
                let ctrl = world.states.controllers[si];
                if ctrl.is_none() {
                    continue;
                }
                for (idx, members) in side_cids.iter().enumerate() {
                    if members.iter().any(|&cid| cid == ctrl) {
                        counts[idx] += 1;
                        break;
                    }
                }
            }
            // 写回 progress（每方 = 控制州数 / 总州数 × 100）
            for (idx, &cnt) in counts.iter().enumerate() {
                if idx < active.progress.len() {
                    active.progress[idx] = (cnt as f32 / total) * 100.0;
                }
            }
        }
    }

    /// 兼容旧路径：weekly_tick 现在转发到 daily_tick × 7。
    /// 新代码应直接调用 `daily_tick`。
    #[deprecated(note = "use daily_tick instead — progress is no longer auto-advanced")]
    pub fn weekly_tick(&mut self, world: &World) {
        self.daily_tick(world);
    }

    /// AI auto-intervention: each AI country checks its default_supporter sides
    /// and performs the highest-priority affordable intervention if not on cooldown.
    /// Called weekly after weekly_tick by the app layer.
    pub fn ai_auto_intervene(&mut self, world: &mut World) {
        let def_ids: Vec<String> = self.defs.iter().map(|d| d.id.clone()).collect();
        for def_id in &def_ids {
            let player = world.player;
            // Collect interventions to perform (side_index, country, intervention_id)
            let mut to_perform: Vec<(usize, CountryId, String)> = Vec::new();

            for side_idx in 0..self
                .defs
                .iter()
                .find(|d| d.id == *def_id)
                .map(|d| d.sides.len())
                .unwrap_or(0)
            {
                let supporters_tags: Vec<String> = self
                    .defs
                    .iter()
                    .find(|d| d.id == *def_id)
                    .and_then(|d| d.sides.get(side_idx))
                    .map(|s| s.default_supporters.clone())
                    .unwrap_or_default();

                for tag in &supporters_tags {
                    let Some(cid) = world.country(tag) else {
                        continue;
                    };
                    if cid == player {
                        continue;
                    } // Skip player country

                    let Some(def) = self.defs.iter().find(|d| d.id == *def_id) else {
                        continue;
                    };
                    let Some(active) = self.active.iter().find(|a| a.def_id == *def_id && !a.ended)
                    else {
                        continue;
                    };

                    // Find the first affordable intervention for this side that's not on cooldown
                    for interv in &def.interventions {
                        if interv.side_index != side_idx {
                            continue;
                        }
                        let cd_key = (cid.0, interv.id.clone());
                        if active.cooldowns.contains_key(&cd_key) {
                            continue;
                        }

                        // Check affordability
                        let ci = cid.0 as usize;
                        if ci >= world.countries.count {
                            continue;
                        }
                        if interv.cost_pp > 0.0
                            && world
                                .countries
                                .political_power
                                .get(ci)
                                .map_or(true, |&pp| pp < interv.cost_pp)
                        {
                            continue;
                        }
                        if interv.cost_manpower > 0 && world.manpower(cid) < interv.cost_manpower {
                            continue;
                        }

                        to_perform.push((side_idx, cid, interv.id.clone()));
                        break; // One intervention per side per AI country per tick
                    }
                }
            }

            // Perform interventions
            for (_side_idx, cid, interv_id) in to_perform {
                self.intervene(def_id, &interv_id, cid, world, None);
            }
        }
    }

    /// Drain pending effects (called by app layer to apply them to World).
    pub fn drain_effects(&mut self) -> Vec<SituationEffect> {
        std::mem::take(&mut self.pending_effects)
    }

    /// Player/AI performs an intervention.
    pub fn intervene(
        &mut self,
        situation_id: &str,
        intervention_id: &str,
        country: CountryId,
        world: &mut World,
        stockpile: Option<&mut HashMap<String, f32>>,
    ) -> bool {
        let Some(def) = self.defs.iter().find(|d| d.id == situation_id) else {
            return false;
        };
        let Some(interv) = def.interventions.iter().find(|i| i.id == intervention_id) else {
            return false;
        };
        let Some(active) = self
            .active
            .iter_mut()
            .find(|a| a.def_id == situation_id && !a.ended)
        else {
            return false;
        };

        // Check cooldown
        let cd_key = (country.0, intervention_id.to_string());
        if active.cooldowns.contains_key(&cd_key) {
            return false;
        }

        let ci = country.0 as usize;

        // Check & deduct PP
        if interv.cost_pp > 0.0 {
            if world.countries.political_power[ci] < interv.cost_pp {
                return false;
            }
            world.countries.political_power[ci] -= interv.cost_pp;
        }

        // Check & deduct manpower
        if interv.cost_manpower > 0 {
            if world.manpower(country) < interv.cost_manpower {
                return false;
            }
            subtract_manpower_from_pops(world, country, interv.cost_manpower);
        }

        // Check & deduct equipment
        if let Some(stockpile) = stockpile {
            for (equip, amount) in &interv.cost_equipment {
                let key = normalize_equipment_id(equip);
                let have = stockpile.get(&key).copied().unwrap_or(0.0);
                if have < *amount {
                    return false;
                }
            }
            for (equip, amount) in &interv.cost_equipment {
                let key = normalize_equipment_id(equip);
                let entry = stockpile.entry(key).or_insert(0.0);
                *entry -= amount;
            }
        }

        // Apply progress boost
        if interv.side_index < active.progress.len() {
            active.progress[interv.side_index] += interv.progress_boost;
        }

        // Apply XP rewards
        world.countries.army_xp[ci] += interv.army_xp;
        world.countries.air_xp[ci] += interv.air_xp;

        // Add to supporters if not already
        if interv.side_index < active.supporters.len() {
            if !active.supporters[interv.side_index].contains(&country) {
                active.supporters[interv.side_index].push(country);
            }
        }

        // Log the intervention
        let side_name = def
            .sides
            .get(interv.side_index)
            .map(|s| s.name.clone())
            .unwrap_or_default();
        let country_tag = world
            .country_tag(country)
            .map(|t| t.to_string())
            .unwrap_or_else(|| format!("C{}", country.0));
        active.intervention_log.push(InterventionLog {
            country_tag,
            intervention_name: interv.name.clone(),
            side_name,
            progress_boost: interv.progress_boost,
            army_xp: interv.army_xp,
            air_xp: interv.air_xp,
        });

        // Set cooldown
        active.cooldowns.insert(cd_key, interv.cooldown_days);

        // Phase 4: 推送 concrete effects（替换 {caller} 占位符为调用国 tag）
        let caller_tag = world
            .country_tag(country)
            .map(|t| t.to_string())
            .unwrap_or_default();
        let interv_effects: Vec<SituationEffect> = interv
            .effects
            .iter()
            .map(|e| substitute_caller_tag(e, &caller_tag))
            .collect();
        self.pending_effects.extend(interv_effects);

        true
    }
}

/// Phase 4: 把 SituationEffect 中的字符串字段做 `{caller}` 占位符替换。
fn substitute_caller_tag(effect: &SituationEffect, caller: &str) -> SituationEffect {
    let sub = |s: &String| -> String {
        if s == "{caller}" {
            caller.to_string()
        } else {
            s.clone()
        }
    };
    match effect {
        SituationEffect::LendDivisions {
            from,
            to,
            count,
            template_priority,
        } => SituationEffect::LendDivisions {
            from: sub(from),
            to: sub(to),
            count: *count,
            template_priority: template_priority.clone(),
        },
        SituationEffect::SendEquipment {
            from,
            to,
            equipment,
            amount,
        } => SituationEffect::SendEquipment {
            from: sub(from),
            to: sub(to),
            equipment: equipment.clone(),
            amount: *amount,
        },
        SituationEffect::SendXpBuff {
            to,
            army_xp,
            air_xp,
        } => SituationEffect::SendXpBuff {
            to: sub(to),
            army_xp: *army_xp,
            air_xp: *air_xp,
        },
        SituationEffect::SpawnFrontlineDivisions {
            tag,
            enemy_tag,
            density,
        } => SituationEffect::SpawnFrontlineDivisions {
            tag: sub(tag),
            enemy_tag: sub(enemy_tag),
            density: *density,
        },
        other => other.clone(),
    }
}

/// Phase 3: 军事胜利判定。
///
/// 规则：当某一方对所有"对手国"满足以下条件，则该方获胜：
/// - 对手首都州的 controller 已变为该方阵营成员
/// - 对手原属州里 ≥ 90% 的 land 省的 controller 已变为该方阵营成员
///   （不要求 100% — 否则一个 ETH 师团残留在边角省会让战争永远结束不了）
///
/// 返回获胜方索引；如无明确赢家则 None（继续打）。
fn check_military_victory(world: &World, side_country_tags: &[Vec<String>]) -> Option<usize> {
    if side_country_tags.len() != 2 {
        return None;
    }
    let side_cids: Vec<Vec<CountryId>> = side_country_tags
        .iter()
        .map(|tags| tags.iter().filter_map(|t| world.country(t)).collect())
        .collect();

    for atk_idx in 0..2 {
        let def_idx = 1 - atk_idx;
        let attackers = &side_cids[atk_idx];
        let defenders = &side_cids[def_idx];
        if defenders.is_empty() {
            continue;
        }

        let mut all_conquered = true;
        for &def_cid in defenders {
            if world.diplomacy.annexed_countries.contains(&def_cid) {
                continue;
            }

            // 1) 首都所在 state 的 land 省份大多数（≥80%）已被 attacker 控制
            //    （而不是要求 state.controller == attacker —— 那个 flag 只在 state 全翻
            //     时才更新；任何残留敌方单位都会让它永远不翻）
            let ci = def_cid.0 as usize;
            if ci >= world.countries.count {
                all_conquered = false;
                break;
            }
            let cap_state = world.countries.capitals[ci];
            if cap_state.is_none() {
                continue;
            }
            let cap_si = cap_state.0 as usize;
            if cap_si >= world.states.count {
                all_conquered = false;
                break;
            }
            {
                let cap_provs = &world.states.provinces[cap_si];
                let mut cap_total = 0u32;
                let mut cap_held = 0u32;
                for &p in cap_provs {
                    let pi = p.0 as usize;
                    if pi >= world.provinces.count {
                        continue;
                    }
                    cap_total += 1;
                    let pc = world.provinces.controllers[pi];
                    if attackers.iter().any(|&a| a == pc) {
                        cap_held += 1;
                    }
                }
                if cap_total > 0 {
                    let cap_ratio = cap_held as f32 / cap_total as f32;
                    if cap_ratio < 0.80 {
                        all_conquered = false;
                        break;
                    }
                }
            }

            // 2) 该国所有原属州的省 controller 比例 ≥ 80%
            //    （不强求 100% — 否则一个 ETH 师团残留在边角会让战争永远结束不了）
            let mut total_provs = 0u32;
            let mut atk_held = 0u32;
            for si in 0..world.states.count {
                if world.states.owners[si] != def_cid {
                    continue;
                }
                for &p in &world.states.provinces[si] {
                    let pi = p.0 as usize;
                    if pi >= world.provinces.count {
                        continue;
                    }
                    total_provs += 1;
                    let pc = world.provinces.controllers[pi];
                    if attackers.iter().any(|&a| a == pc) {
                        atk_held += 1;
                    }
                }
            }
            if total_provs == 0 {
                continue;
            }
            let ratio = atk_held as f32 / total_provs as f32;
            if ratio < 0.80 {
                all_conquered = false;
                break;
            }
        }
        if all_conquered {
            return Some(atk_idx);
        }
    }
    None
}

fn subtract_manpower_from_pops(world: &mut World, country: CountryId, amount: u64) {
    let state_ids = world.country_state_ids(country);
    let indices = world
        .countries
        .pops
        .pops_by_class_in_country_mut(PopClass::Soldier, &state_ids);
    let mut remaining = amount;
    for &idx in &indices {
        if remaining == 0 {
            break;
        }
        let pg = &mut world.countries.pops.groups[idx];
        if pg.employed_at.is_none() {
            let sub = remaining.min(pg.size as u64);
            pg.size = pg.size.saturating_sub(sub as u32);
            remaining = remaining.saturating_sub(sub);
        }
    }
}
