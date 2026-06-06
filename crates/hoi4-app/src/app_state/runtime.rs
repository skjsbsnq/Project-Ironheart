use hoi4_logic::economy::construction_planner::ConstructionCandidateScore;
use hoi4_logic::economy::EconomyState;
use hoi4_logic::politics::PoliticsCache;
use hoi4_logic::research::ResearchState;
use hoi4_runtime::{AiState, ScriptState, SystemSchedule};

pub(crate) struct RuntimeState {
    pub(crate) econ: EconomyState,
    pub(crate) research: ResearchState,
    pub(crate) politics_cache: PoliticsCache,
    pub(crate) script: ScriptState,
    pub(crate) ai: AiState,
    pub(crate) feedback_bus: hoi4_logic::FeedbackBus,
    pub(crate) schedule: SystemSchedule,
    pub(crate) music_player: hoi4_audio::MusicPlayer,
    pub(crate) auto_build_enabled: bool,
    pub(crate) last_auto_build_month: Option<(u16, u8)>,
    pub(crate) last_auto_build_explanations: Vec<ConstructionCandidateScore>,
    pub(crate) content: hoi4_runtime::ContentRuntimeState,
    pub(crate) last_war_count: usize,
}
