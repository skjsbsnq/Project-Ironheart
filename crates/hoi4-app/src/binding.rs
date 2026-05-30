//! V5 收口（2026-05-18）：玩家国家 + 全局状态的 UI 数据快照。
//!
//! V3 时代实现了 `hoi4_assets::DataBinding` trait 把 `[Key]` 占位符路由到 World
//! 字段，喂给 GuiRt-removed 渲染 vanilla `.gui` 文本框。V5 放弃 vanilla GUI 路线
//! 后，`DataBinding` trait 已被删除；本快照仍然保留为只读 `pub` 字段，下一阶段
//! 由 egui UI 层直接消费。
//!
//! 该 binding 是只读快照——构造时拷贝数值/字符串，避免与 World mutate 冲突。

use hoi4_state::{CountryId, GameSpeed, World};

/// 玩家国家 + 全局状态的 UI 数据绑定快照。
pub struct WorldBinding {
    pub player_idx: usize,
    pub player_tag: String,
    pub player_name: String,
    pub political_power: f32,
    pub stability: f32,
    pub war_support: f32,
    pub manpower: u64,
    pub civ_factories: u32,
    pub mil_factories: u32,
    pub shipyards: u32,
    pub army_xp: f32,
    pub navy_xp: f32,
    pub air_xp: f32,
    pub fuel: f32,
    pub fuel_capacity: f32,
    pub date: String,
    pub speed: String,
    pub ideology: String,
    pub country_count: usize,
    /// 4.3 Step B：玩家执政党的内部 key（"democratic" / "communism" / "fascism" / "neutrality"）。
    /// 来源 `World.countries.ruling_party`。空字符串 = 未设置。
    pub ruling_party: String,
    /// 4.3 Step B：玩家当前执行的国策内部 id（如 `"GER_rhineland"`）。
    /// 来源 `World.countries.current_focus`。`None` = idle。
    pub current_focus: Option<String>,
    /// 4.3 Step B：当前国策已累积进度（天）。
    pub focus_progress: f32,
    /// 国家选择阶段被高亮的国家（与 player_idx 不一定相同 —
    /// player 切换前 = 当前光标）。
    pub selected_idx: usize,
    pub selected_tag: String,
    pub selected_name: String,
    pub selected_manpower: u64,
    pub selected_civ: u32,
    pub selected_mil: u32,
    pub selected_ideology: String,
    /// 当前加载阶段（loading 屏使用）。
    pub load_stage: String,
}

impl WorldBinding {
    /// 从 World 构造（player_idx = playing 阶段绑定的国家；selected_idx = 国选阶段光标）。
    pub fn snapshot(world: &World, player_idx: usize, selected_idx: usize) -> Self {
        let speed = match world.speed {
            GameSpeed::Paused => "Paused",
            GameSpeed::Speed1 => "Speed 1",
            GameSpeed::Speed2 => "Speed 2",
            GameSpeed::Speed3 => "Speed 3",
            GameSpeed::Speed4 => "Speed 4",
            GameSpeed::Speed5 => "Speed 5",
        }
        .to_string();

        let player_tag = world
            .countries
            .tags
            .get(player_idx)
            .cloned()
            .unwrap_or_default();
        let player_name = country_display_name(world, player_idx);

        let selected_tag = world
            .countries
            .tags
            .get(selected_idx)
            .cloned()
            .unwrap_or_default();
        let selected_name = country_display_name(world, selected_idx);

        let ruling_party = world
            .countries
            .ruling_party
            .get(player_idx)
            .cloned()
            .unwrap_or_default();
        let current_focus = world
            .countries
            .current_focus
            .get(player_idx)
            .cloned()
            .unwrap_or(None);
        let focus_progress = world
            .countries
            .focus_progress
            .get(player_idx)
            .copied()
            .unwrap_or(0.0);

        Self {
            player_idx,
            player_tag,
            player_name,
            political_power: world
                .countries
                .political_power
                .get(player_idx)
                .copied()
                .unwrap_or(0.0),
            stability: world
                .countries
                .stability
                .get(player_idx)
                .copied()
                .unwrap_or(0.5),
            war_support: world
                .countries
                .war_support
                .get(player_idx)
                .copied()
                .unwrap_or(0.5),
            manpower: world.manpower(CountryId(player_idx as u16)),
            civ_factories: 0,
            mil_factories: 0,
            shipyards: world.country_industry(CountryId(player_idx as u16)).2,
            army_xp: world
                .countries
                .army_xp
                .get(player_idx)
                .copied()
                .unwrap_or(0.0),
            navy_xp: world
                .countries
                .navy_xp
                .get(player_idx)
                .copied()
                .unwrap_or(0.0),
            air_xp: world
                .countries
                .air_xp
                .get(player_idx)
                .copied()
                .unwrap_or(0.0),
            fuel: world.countries.fuel.get(player_idx).copied().unwrap_or(0.0),
            fuel_capacity: world
                .countries
                .fuel_capacity
                .get(player_idx)
                .copied()
                .unwrap_or(1000.0),
            date: format!("{}", world.date),
            speed,
            ideology: country_ideology(world, player_idx),
            country_count: world.countries.count,
            ruling_party,
            current_focus,
            focus_progress,
            selected_idx,
            selected_manpower: world.manpower(CountryId(selected_idx as u16)),
            selected_civ: 0,
            selected_mil: 0,
            selected_ideology: country_ideology(world, selected_idx),
            selected_tag,
            selected_name,
            load_stage: String::new(),
        }
    }

    pub fn with_load_stage(mut self, stage: impl Into<String>) -> Self {
        self.load_stage = stage.into();
        self
    }
}

fn country_display_name(world: &World, idx: usize) -> String {
    // 优先用 tag（country.localised_name 在 hoi4-data 里没有暴露，走 tag）
    world.countries.tags.get(idx).cloned().unwrap_or_default()
}

fn country_ideology(world: &World, idx: usize) -> String {
    // 4.3 Step B：vanilla `countrypoliticsview.gui::ideology` 显示执政党名。
    // 优先用 World 的 `ruling_party` 字段（"democratic" / "communism" / ...），
    // 翻译为玩家可读的英文名（4.9 本地化拼包前的占位）。
    let key = world
        .countries
        .ruling_party
        .get(idx)
        .cloned()
        .unwrap_or_default();
    ideology_display_name(&key)
}

/// 4.3 Step B：把 vanilla 内部意识形态 key 翻译为可读文本。
/// 返回空字符串当 key 未设置 — 不会让 vanilla 占位文字 "ideology" 漏出来。
fn ideology_display_name(key: &str) -> String {
    match key {
        "democratic" => "Democratic".to_string(),
        "communism" => "Communist".to_string(),
        "fascism" => "Fascist".to_string(),
        "neutrality" => "Non-Aligned".to_string(),
        "" => String::new(),
        other => other.to_string(),
    }
}

impl WorldBinding {
    /// V3 4.2 时代是 `DataBinding::query_value`，V5 改为 inherent 方法供新 UI
    /// 层（egui 等）直接调用。键名沿用以便 stage B 平滑接入。
    pub fn query_value(&self, key: &str) -> Option<f64> {
        match key {
            "PoliticalPower" => Some(self.political_power as f64),
            "Stability" => Some(self.stability as f64 * 100.0),
            "WarSupport" => Some(self.war_support as f64 * 100.0),
            "Manpower" => Some(self.manpower as f64),
            "ManpowerMillions" => Some(self.manpower as f64 / 1_000_000.0),
            "CivFactories" => Some(self.civ_factories as f64),
            "MilFactories" => Some(self.mil_factories as f64),
            "Shipyards" => Some(self.shipyards as f64),
            "TotalIndustry" => {
                Some((self.civ_factories + self.mil_factories + self.shipyards) as f64)
            }
            "ArmyXp" => Some(self.army_xp as f64),
            "NavyXp" => Some(self.navy_xp as f64),
            "AirXp" => Some(self.air_xp as f64),
            "Fuel" => Some(self.fuel as f64),
            "FuelCapacity" => Some(self.fuel_capacity as f64),
            "CountryCount" => Some(self.country_count as f64),
            "SelectedManpower" => Some(self.selected_manpower as f64),
            _ => None,
        }
    }

    pub fn query_string(&self, key: &str) -> Option<String> {
        match key {
            // Generic placeholders (still supported via `[Key]` syntax).
            "PlayerCountryTag" | "CountryTag" => Some(self.player_tag.clone()),
            "PlayerCountryName" | "CountryName" => Some(self.player_name.clone()),
            "Date" => Some(self.date.clone()),
            "Speed" => Some(self.speed.clone()),
            "Ideology" => Some(self.ideology.clone()),
            "SelectedCountryTag" => Some(self.selected_tag.clone()),
            "SelectedCountryName" => Some(self.selected_name.clone()),
            "SelectedIdeology" => Some(self.selected_ideology.clone()),
            "SelectedManpower" => Some(format!(
                "{:.2}M",
                self.selected_manpower as f64 / 1_000_000.0
            )),
            "SelectedFactories" => Some(format!("{}/{}", self.selected_civ, self.selected_mil)),
            "LoadStage" => Some(self.load_stage.clone()),

            // 4.1.bis.3: vanilla `topbar.gui` widget names — engine overrides
            // placeholder text by name. Map each widget name to the formatted value.
            "pol_power" => Some(format!("{:.0}", self.political_power)),
            "stability_value" | "stability" => Some(format!("{:.0}%", self.stability * 100.0)),
            "war_support_value" | "war_support" => {
                Some(format!("{:.0}%", self.war_support * 100.0))
            }
            "manpower" | "manpower_value" => {
                Some(format!("{:.2}M", self.manpower as f64 / 1_000_000.0))
            }
            // 4.1.bis.10 (2026-05-16): user request — show the **total**
            // factory count in the topbar's industrial_capacity widget rather
            // than the vanilla `civ/mil/dock` triple. Each branch is still
            // available individually via the per-branch keys below.
            "industrial_capacity" => Some(format!(
                "{}",
                self.civ_factories + self.mil_factories + self.shipyards
            )),
            "civilian_industry" | "civ_factories_value" => Some(format!("{}", self.civ_factories)),
            "military_industry" | "mil_factories_value" => Some(format!("{}", self.mil_factories)),
            "naval_industry" | "shipyards_value" => Some(format!("{}", self.shipyards)),
            // 4.1.bis.10 (2026-05-16): vanilla shows fuel abbreviated as
            // `12.3k` once the value exceeds 999 (the widget is 48 px wide
            // and `hoi_18mbs` would clip a raw 5-digit number). Match that
            // convention so the visible string fits.
            "fuel_value" | "fuel" => Some(format_short(self.fuel as f64)),
            // Supply (equipment stockpile) — not modelled yet; show empty so
            // the placeholder "999d" doesn't leak through. When Phase 4.x
            // wires equipment storage we'll return the real total.
            "supply_value" => Some(String::new()),
            // Convoys are not modelled yet (Phase 4.x); show 0 abbreviated
            // (so the formatting code path matches vanilla even at zero).
            "convoys_count" | "convoys" => Some(format_short(0.0)),
            // Threat / world tension widget — placeholder until 4.7 wires
            // the real value. Empty string keeps the topbar uncluttered.
            "threat_value" => Some(String::new()),
            // Per-branch experience widgets (within `land`/`navy`/`air` containers).
            "experience_value" => Some(format!("{:.0}", self.army_xp)),
            "navy_experience_value" => Some(format!("{:.0}", self.navy_xp)),
            "air_experience_value" => Some(format!("{:.0}", self.air_xp)),
            "DateText" => Some(self.date.clone()),
            "date" | "date_text" => Some(self.date.clone()),
            // Total factories — vanilla shows civ+mil+dock as separate widgets,
            // but a few mods aggregate; provide both forms.
            "total_industry" => Some(format!(
                "{}",
                self.civ_factories + self.mil_factories + self.shipyards
            )),
            // World Tension widget — placeholder until 4.7 wires real value.
            "world_tension_value" => Some(format!("{:.0}%", 0.0)),
            // LOBBY button label (vanilla label, kept untranslated for now).
            "reopen_ingame_lobby_label" => Some("LOBBY".to_string()),

            // 4.1.bis.6 fix (2026-05-16): every other vanilla `text="999"`
            // placeholder. Without an explicit binding the literal "999"
            // bleeds through; clamp to empty until the relevant feature
            // (Phase 4.x) wires real data.
            "command_power_value" => Some(format!("{:.0}", self.army_xp.max(0.0).min(150.0))),
            "initiative_text"
            | "decisionview_amount_timeout_items"
            | "decisionview_amount_to_take_items"
            | "intelview_ops_prepared_or_completed_items"
            | "intelview_ops_available_items"
            | "prototype_rewards_aquired"
            | "non_delivering_contract_amount"
            | "ongoing_contract_amount"
            | "nukes_value" => Some(String::new()),

            // ─────────────────────────────────────────────────────────────────
            // 4.3 Step B (2026-05-18): vanilla `countrypoliticsview.gui` widget
            // names. Each `text="..."` placeholder（POLITICAL_POLITICAL /
            // ideology / elections / autonomy / 3 / faction_name / ...）must be
            // overridden by widget name, otherwise the literal vanilla
            // placeholder leaks through.
            //
            // 4.9 之前不查 localisation/*.yml — political_title 直接返回 raw
            // loc key（或可读占位），其它字段在功能未建模时返回空。
            // ─────────────────────────────────────────────────────────────────

            // 政治面板 header（4.9 接本地化前先返回 raw key 占位）。
            "political_title" => Some("POLITICAL_POLITICAL".to_string()),

            // 执政党 ideology 名（country_ideology 已映射 ruling_party → 可读）。
            "ideology" => Some(self.ideology.clone()),

            // 选举倒计时：未建模 — 返回空，避免 vanilla 占位 "elections" 漏出。
            "elections" => Some(String::new()),

            // 阵营 / 宗主国 / 自治：项目尚未实现这些 DLC 子系统，统一返回空。
            "faction_name" => Some(String::new()),
            "no_faction" => Some(String::new()),
            "view_factions_text" => Some(String::new()),
            "overlord_name" => Some(String::new()),
            "current_autonomy_name" => Some(String::new()),

            // power balance（CWtP DLC）：未建模 — 返回空。
            "power_balance_percentage" | "power_balance_levels" => Some(String::new()),

            // 当前 focus 的 PP 消耗：vanilla 占位 "3"。focus 树定义未加载 →
            // 没法查 cost。current_focus 有值时显示固定 "70"（vanilla 默认 7
            // 天 × 10 PP/day），idle 时空。4.5 国策树章节会替换为真实 cost。
            "focus_cost" => Some(if self.current_focus.is_some() {
                "70".to_string()
            } else {
                String::new()
            }),

            _ => None,
        }
    }

    pub fn query_bool(&self, _key: &str) -> Option<bool> {
        None
    }
}

/// 4.1.bis.10 (2026-05-16): 把数字格式化成 vanilla topbar 的"短"形式：
/// - `< 10000` → 整数
/// - `< 1_000_000` → `12.3k`
/// - `< 1_000_000_000` → `1.23M`
/// 用于 `fuel_value` / `convoys_count` 等狭窄文本框（48-50 px）。
fn format_short(v: f64) -> String {
    let abs = v.abs();
    if abs < 10_000.0 {
        format!("{:.0}", v)
    } else if abs < 1_000_000.0 {
        format!("{:.1}k", v / 1_000.0)
    } else if abs < 1_000_000_000.0 {
        format!("{:.2}M", v / 1_000_000.0)
    } else {
        format!("{:.2}B", v / 1_000_000_000.0)
    }
}
