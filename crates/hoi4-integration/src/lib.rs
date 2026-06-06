//! Phase 0.4：集成测试基础设施。
//!
//! 提供：
//! 1. [`Metrics`] —— 一个国家在某时刻的关键数值（PP / 工厂 / 人力 / 已研 tech 数 /
//!    已完成 focus 数）。集成测试在不同时间点采样并比对。
//! 2. [`load_world`] —— 从默认路径解析 + 加载完整 World，返回结果或错误诊断。
//! 3. [`tick_days`] —— 用 [`hoi4_runtime::SystemSchedule`] 推进 N 天并收集执行时间。
//!
//! ## 使用模式（集成测试）
//! ```ignore
//! let mut world = hoi4_integration::load_world().expect("HOI4 install required");
//! let baseline = hoi4_integration::Metrics::sample(&world, "GER");
//! hoi4_integration::tick_days(&mut world, 30);
//! let after = hoi4_integration::Metrics::sample(&world, "GER");
//! assert!(after.industry_buildings >= baseline.industry_buildings);
//! ```

use std::time::{Duration, Instant};

use hoi4_assets::AssetDb;
use hoi4_data::{CharacterDef, CountryTag, GameData};
use hoi4_logic::economy::EconomyState;
use hoi4_logic::politics::PoliticsCache;
use hoi4_logic::research::ResearchState;
use hoi4_map::GameMap;
use hoi4_paths::{PathConfig, ResolveOverrides};
use hoi4_runtime::{AiState, ContentRuntimeState, HourlyRuntime, ScriptState, SystemSchedule};
use hoi4_state::World;

/// Phase 3.12.14: Load `common/defines.lua` from vanilla and diff against
/// the project's mirrored constants in `hoi4_render::defines_lua`.
/// Returns a banner string for CI smoke test output.
pub fn defines_lua_banner() -> String {
    let cfg = match PathConfig::resolve(ResolveOverrides::default()) {
        Ok(c) => c,
        Err(_) => return "[defines_lua] HOI4 install not found — skipping".to_string(),
    };
    let db = hoi4_assets::FsAssetDb::new(cfg);
    let lua_src = match db.open("common/defines.lua") {
        Ok(bytes) => String::from_utf8_lossy(&*bytes).into_owned(),
        Err(_) => return "[defines_lua] common/defines.lua not found — skipping".to_string(),
    };
    let diff = hoi4_render::defines_lua::diff_against_vanilla(&lua_src);
    let banner = format!(
        "[defines_lua] mirrored={}/{} missing={:?} stale={:?}",
        diff.shared_count,
        diff.shared_count + diff.missing_in_project.len(),
        diff.missing_in_project,
        diff.stale_in_project,
    );
    banner
}

pub use hoi4_logic::economy::BuildOrder;
pub use hoi4_runtime::init_simulation;
/// Re-exports so tests don't have to depend on hoi4-state directly.
pub use hoi4_runtime::AiState as ReexportAiState;
pub use hoi4_runtime::ScriptState as ReexportScriptState;
pub use hoi4_state::{CountryId, DivisionStore, ProvinceId, World as ReexportWorld};

/// 关键回归 metrics。Phase 1 起所有数值会随系统运行变化；Phase 0 系统是 no-op
/// 所以采样后的值只反映"World::new 初始化结果"。
#[derive(Debug, Clone, PartialEq)]
pub struct Metrics {
    pub tag: String,
    /// 政治力（PP）
    pub political_power: f32,
    /// 稳定度
    pub stability: f32,
    /// 战争支持
    pub war_support: f32,
    /// V6 建筑工业统计。
    pub industry_buildings: u32,
    pub military_buildings: u32,
    pub shipyards: u32,
    /// 累计人力池
    pub manpower: u64,
    /// 已研发科技数（completed_techs.len()）
    pub completed_techs: usize,
    /// 已完成 focus 数
    pub completed_focuses: usize,
    /// 当前正在执行的 focus（None 表示未选）
    pub current_focus: Option<String>,
}

impl Metrics {
    /// 在某国家上采样一次。country tag 不存在则 panic（集成测试必备）。
    pub fn sample(world: &World, tag: &str) -> Self {
        let cid = world
            .country(tag)
            .unwrap_or_else(|| panic!("country tag {tag} not present in world"));
        let i = cid.0 as usize;
        let c = &world.countries;
        Self {
            tag: tag.to_owned(),
            political_power: c.political_power[i],
            stability: c.stability[i],
            war_support: c.war_support[i],
            industry_buildings: world.country_industry(cid).0,
            military_buildings: world.country_industry(cid).1,
            shipyards: world.country_industry(cid).2,
            manpower: world.manpower(cid),
            completed_techs: c.completed_techs[i].len(),
            completed_focuses: c.completed_focuses[i].len(),
            current_focus: c.current_focus[i].clone(),
        }
    }
}

/// 加载真实 World。失败返回 `Err(message)`；CI / 本机均可用。
pub fn load_world() -> Result<World, String> {
    let cfg = PathConfig::resolve(ResolveOverrides::default()).map_err(|e| e.to_string())?;
    let game_path = cfg.game_path();
    let map =
        std::sync::Arc::new(GameMap::load(game_path).map_err(|e| format!("GameMap::load: {e}"))?);
    let mut data = GameData::load(game_path).map_err(|e| format!("GameData::load: {e}"))?;
    inject_project_head_of_state_characters(&mut data);
    let data = std::sync::Arc::new(data);
    let mut world = World::new(map, data);
    let _ = world.populate_from_history();
    Ok(world)
}

fn inject_project_head_of_state_characters(data: &mut GameData) {
    let Ok(history) = hoi4_content::Historical1936Database::load() else {
        return;
    };
    for head in history.head_of_states {
        if head.character_key.is_empty() || head.portrait_gfx.is_empty() {
            continue;
        }
        if data
            .characters
            .iter()
            .any(|character| character.key == head.character_key)
        {
            continue;
        }
        let tag = CountryTag::new(&head.tag);
        let idx = data.characters.len();
        data.characters.push(CharacterDef {
            key: head.character_key.clone(),
            tag: tag.clone(),
            name_loc_key: if head.name.is_empty() {
                head.character_key
            } else {
                head.name
            },
            portrait_large: Some(head.portrait_gfx),
            country_leader_ideology: Some("despotism".to_owned()),
            source_order: u32::MAX,
        });
        data.characters_by_tag.entry(tag).or_default().push(idx);
    }
}

/// 推进 `days` 天，每天 24 hour ticks，路由到 [`SystemSchedule`]（Phase 1 真实系统）。
/// 内部调用 [`init_simulation`] 创建 companion states。
/// 返回总执行耗时（用于 ms/day 报表）。
pub fn tick_days(world: &mut World, days: u32) -> TickReport {
    let (mut econ, mut research, mut politics_cache, mut script, mut ai) = init_simulation(world);
    tick_days_with(
        world,
        &mut econ,
        &mut research,
        &mut politics_cache,
        &mut script,
        &mut ai,
        days,
    )
}

/// 推进 `days` 天，使用调用方提供的 companion states。
/// 适用于需要在 tick 前预置状态（如建造队列）的测试。
pub fn tick_days_with(
    world: &mut World,
    econ: &mut EconomyState,
    research: &mut ResearchState,
    politics_cache: &mut PoliticsCache,
    script: &mut ScriptState,
    ai: &mut AiState,
    days: u32,
) -> TickReport {
    let mut schedule = SystemSchedule::with_phase1_systems();
    let mut feedback_bus = hoi4_logic::feedback::FeedbackBus::new();
    let v6_db = hoi4_content::V6Database::load();
    // P0.1：构造空 content state（集成测试不依赖 ScenarioContent）
    let mut content = ContentRuntimeState::empty_for_test(
        hoi4_state::CountryId::NONE,
        world.date.days_since_epoch(),
    );
    let t0 = Instant::now();
    for _ in 0..days {
        for _ in 0..24 {
            let _ = hoi4_runtime::tick_one_hour(&mut HourlyRuntime {
                world,
                econ,
                research,
                politics_cache,
                script,
                ai,
                v6_db: &v6_db,
                schedule: &mut schedule,
                feedback_bus: &mut feedback_bus,
                content: &mut content,
            });
        }
    }
    let elapsed = t0.elapsed();
    TickReport {
        days,
        total: elapsed,
        ms_per_day: elapsed.as_secs_f64() * 1000.0 / days.max(1) as f64,
        report_line: schedule.report_systems(),
    }
}

/// `tick_days` 的回报数据。
#[derive(Debug, Clone)]
pub struct TickReport {
    pub days: u32,
    pub total: Duration,
    pub ms_per_day: f64,
    /// `SystemSchedule::report_systems()` 末态字符串
    pub report_line: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 Metrics::sample 不需要真实 HOI4 安装就能编译；
    /// 真正的"采样某国"在 tests/ 下运行。
    #[test]
    fn metrics_struct_layout() {
        let m = Metrics {
            tag: "TST".into(),
            political_power: 0.0,
            stability: 0.0,
            war_support: 0.0,
            industry_buildings: 0,
            military_buildings: 0,
            shipyards: 0,
            manpower: 0,
            completed_techs: 0,
            completed_focuses: 0,
            current_focus: None,
        };
        assert_eq!(m.tag, "TST");
    }
}
