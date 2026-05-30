//! per-country AI 配置（AiProfile）。
//!
//! 取代 `personality.rs` + `strategic_profile.rs` 的硬编码：
//! 所有字段有默认值（= default 配置），国家专属配置覆盖任意子集。
//! 仿 vanilla HOI4 的 `default.txt` + `<TAG>.txt`。
//!
//! 配置在 `PROFILES` 静态数组里定义，启动时构建 `HashMap<String, AiProfile>`，
//! spawn_country 时按 tag 查找。没有配置的国家用 default 副本。
//!
//! 后续可搬到 `data/ai_profiles/<TAG>.toml`（需引入 toml crate）。

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::scoring::Scorer;

#[derive(Debug, Clone, PartialEq)]
pub enum FactionRole {
    LeadFaction {
        name: String,
        by_year: u16,
        by_month: u8,
        by_day: u8,
    },
    JoinFaction {
        faction_name: String,
        by_year: u16,
        by_month: u8,
        by_day: u8,
    },
    AwaitInvite,
    Solo,
}

impl Default for FactionRole {
    fn default() -> Self {
        FactionRole::Solo
    }
}

#[derive(Debug, Clone)]
pub struct AiProfile {
    pub aggression: f32,
    pub civilian_focus: f32,
    pub doctrine_priority: f32,
    pub air_focus: f32,
    pub naval_focus: f32,
    pub production_focus_weights: [f32; 5],

    pub target_civ_factories: u32,
    pub target_mil_factories: u32,
    pub civ_to_mil_ratio: f32,
    pub mil_focus_year: u16,

    pub target_div_count_peace: u32,
    pub target_div_count_war: u32,

    pub priority_targets: Vec<String>,
    pub force_attack_against: Vec<String>,

    pub faction_role: FactionRole,
    pub faction_target_name: Option<String>,
    pub invite_targets: Vec<String>,
    pub refuse_cession: bool,
    pub defensive_only: bool,
    pub defensive_weekly_boost: (i64, f32, f32),
    pub peacetime_weekly_pp: f32,
}

impl Default for AiProfile {
    fn default() -> Self {
        Self {
            aggression: 0.5,
            civilian_focus: 0.5,
            doctrine_priority: 0.5,
            air_focus: 0.3,
            naval_focus: 0.3,
            production_focus_weights: [0.3, 0.4, 0.1, 0.1, 0.1],

            target_civ_factories: 20,
            target_mil_factories: 30,
            civ_to_mil_ratio: 1.5,
            mil_focus_year: 1939,

            target_div_count_peace: 24,
            target_div_count_war: 60,

            priority_targets: Vec::new(),
            force_attack_against: Vec::new(),

            faction_role: FactionRole::Solo,
            faction_target_name: None,
            invite_targets: Vec::new(),
            refuse_cession: false,
            defensive_only: false,
            defensive_weekly_boost: (0, 0.0, 0.0),
            peacetime_weekly_pp: 0.0,
        }
    }
}

struct ProfileEntry {
    tag: &'static str,
    profile: AiProfile,
}

fn hardcoded_profiles() -> Vec<ProfileEntry> {
    vec![
        ProfileEntry {
            tag: "GER",
            profile: AiProfile {
                aggression: 0.9,
                civilian_focus: 0.4,
                doctrine_priority: 0.8,
                air_focus: 0.7,
                naval_focus: 0.4,
                production_focus_weights: [0.2, 0.5, 0.1, 0.1, 0.1],
                target_civ_factories: 30,
                target_mil_factories: 60,
                civ_to_mil_ratio: 1.0,
                mil_focus_year: 1937,
                target_div_count_peace: 50,
                target_div_count_war: 200,
                priority_targets: vec!["POL".into(), "CZE".into(), "FRA".into()],
                force_attack_against: vec!["POL".into()],
                faction_role: FactionRole::LeadFaction {
                    name: "Axis".into(),
                    by_year: 1936,
                    by_month: 8,
                    by_day: 1,
                },
                faction_target_name: Some("Axis".into()),
                invite_targets: vec!["ITA".into(), "HUN".into(), "ROM".into(), "BUL".into()],
                refuse_cession: false,
                defensive_only: false,
                defensive_weekly_boost: (12000, 0.015, 10.0),
                peacetime_weekly_pp: 2.0,
            },
        },
        ProfileEntry {
            tag: "ITA",
            profile: AiProfile {
                aggression: 0.75,
                civilian_focus: 0.4,
                doctrine_priority: 0.6,
                air_focus: 0.5,
                naval_focus: 0.6,
                production_focus_weights: [0.2, 0.3, 0.3, 0.1, 0.1],
                target_civ_factories: 35,
                target_mil_factories: 60,
                civ_to_mil_ratio: 1.2,
                mil_focus_year: 1937,
                target_div_count_peace: 30,
                target_div_count_war: 80,
                priority_targets: vec!["ETH".into(), "GRE".into(), "YUG".into()],
                force_attack_against: vec!["ETH".into()],
                faction_role: FactionRole::AwaitInvite,
                faction_target_name: Some("Axis".into()),
                invite_targets: Vec::new(),
                refuse_cession: false,
                defensive_only: false,
                defensive_weekly_boost: (10000, 0.015, 8.0),
                peacetime_weekly_pp: 1.5,
            },
        },
        ProfileEntry {
            tag: "JAP",
            profile: AiProfile {
                aggression: 0.8,
                civilian_focus: 0.3,
                doctrine_priority: 0.7,
                air_focus: 0.7,
                naval_focus: 0.8,
                production_focus_weights: [0.1, 0.3, 0.4, 0.1, 0.1],
                target_civ_factories: 25,
                target_mil_factories: 40,
                civ_to_mil_ratio: 1.0,
                mil_focus_year: 1938,
                target_div_count_peace: 40,
                target_div_count_war: 120,
                priority_targets: vec!["SND".into(), "CHI".into(), "SHX".into(), "PRC".into()],
                force_attack_against: vec!["SND".into(), "CHI".into(), "SHX".into()],
                faction_role: FactionRole::Solo,
                faction_target_name: None,
                invite_targets: Vec::new(),
                refuse_cession: false,
                defensive_only: false,
                defensive_weekly_boost: (12000, 0.015, 8.0),
                peacetime_weekly_pp: 1.5,
            },
        },
        ProfileEntry {
            tag: "USA",
            profile: AiProfile {
                aggression: 0.3,
                civilian_focus: 0.8,
                doctrine_priority: 0.6,
                air_focus: 0.7,
                naval_focus: 0.7,
                production_focus_weights: [0.4, 0.2, 0.2, 0.1, 0.1],
                target_civ_factories: 60,
                target_mil_factories: 80,
                civ_to_mil_ratio: 1.8,
                mil_focus_year: 1941,
                target_div_count_peace: 30,
                target_div_count_war: 200,
                priority_targets: Vec::new(),
                force_attack_against: Vec::new(),
                faction_role: FactionRole::Solo,
                faction_target_name: None,
                invite_targets: Vec::new(),
                refuse_cession: false,
                defensive_only: true,
                defensive_weekly_boost: (20000, 0.020, 15.0),
                peacetime_weekly_pp: 2.0,
            },
        },
        ProfileEntry {
            tag: "ENG",
            profile: AiProfile {
                aggression: 0.4,
                civilian_focus: 0.6,
                doctrine_priority: 0.6,
                air_focus: 0.6,
                naval_focus: 0.8,
                production_focus_weights: [0.2, 0.2, 0.4, 0.1, 0.1],
                target_civ_factories: 40,
                target_mil_factories: 50,
                civ_to_mil_ratio: 1.5,
                mil_focus_year: 1939,
                target_div_count_peace: 30,
                target_div_count_war: 100,
                priority_targets: Vec::new(),
                force_attack_against: Vec::new(),
                faction_role: FactionRole::LeadFaction {
                    name: "Allies".into(),
                    by_year: 1937,
                    by_month: 1,
                    by_day: 1,
                },
                faction_target_name: Some("Allies".into()),
                invite_targets: vec![
                    "FRA".into(),
                    "CAN".into(),
                    "AST".into(),
                    "NZL".into(),
                    "SAF".into(),
                    "RAJ".into(),
                ],
                refuse_cession: true,
                defensive_only: true,
                defensive_weekly_boost: (8000, 0.012, 8.0),
                peacetime_weekly_pp: 1.5,
            },
        },
        ProfileEntry {
            tag: "FRA",
            profile: AiProfile {
                aggression: 0.3,
                civilian_focus: 0.6,
                doctrine_priority: 0.5,
                air_focus: 0.4,
                naval_focus: 0.4,
                production_focus_weights: [0.3, 0.3, 0.1, 0.2, 0.1],
                target_civ_factories: 30,
                target_mil_factories: 45,
                civ_to_mil_ratio: 1.3,
                mil_focus_year: 1939,
                target_div_count_peace: 30,
                target_div_count_war: 80,
                priority_targets: Vec::new(),
                force_attack_against: Vec::new(),
                faction_role: FactionRole::JoinFaction {
                    faction_name: "Allies".into(),
                    by_year: 1937,
                    by_month: 1,
                    by_day: 14,
                },
                faction_target_name: Some("Allies".into()),
                invite_targets: Vec::new(),
                refuse_cession: true,
                defensive_only: true,
                defensive_weekly_boost: (10000, 0.015, 10.0),
                peacetime_weekly_pp: 1.5,
            },
        },
        ProfileEntry {
            tag: "SOV",
            profile: AiProfile {
                aggression: 0.5,
                civilian_focus: 0.8,
                doctrine_priority: 0.9,
                air_focus: 0.6,
                naval_focus: 0.3,
                production_focus_weights: [0.4, 0.3, 0.1, 0.1, 0.1],
                target_civ_factories: 50,
                target_mil_factories: 80,
                civ_to_mil_ratio: 1.5,
                mil_focus_year: 1939,
                target_div_count_peace: 60,
                target_div_count_war: 300,
                priority_targets: Vec::new(),
                force_attack_against: Vec::new(),
                faction_role: FactionRole::LeadFaction {
                    name: "Comintern".into(),
                    by_year: 1936,
                    by_month: 6,
                    by_day: 1,
                },
                faction_target_name: Some("Comintern".into()),
                invite_targets: vec!["MON".into(), "TAN".into()],
                refuse_cession: true,
                defensive_only: false,
                defensive_weekly_boost: (15000, 0.020, 12.0),
                peacetime_weekly_pp: 2.0,
            },
        },
        ProfileEntry {
            tag: "POL",
            profile: AiProfile {
                aggression: 0.3,
                civilian_focus: 0.5,
                doctrine_priority: 0.4,
                air_focus: 0.3,
                naval_focus: 0.2,
                production_focus_weights: [0.2, 0.4, 0.1, 0.2, 0.1],
                target_civ_factories: 15,
                target_mil_factories: 20,
                civ_to_mil_ratio: 1.2,
                mil_focus_year: 1939,
                target_div_count_peace: 24,
                target_div_count_war: 60,
                priority_targets: Vec::new(),
                force_attack_against: Vec::new(),
                faction_role: FactionRole::Solo,
                faction_target_name: None,
                invite_targets: Vec::new(),
                refuse_cession: true,
                defensive_only: true,
                defensive_weekly_boost: (12000, 0.025, 6.0),
                peacetime_weekly_pp: 1.0,
            },
        },
        ProfileEntry {
            tag: "CZE",
            profile: AiProfile {
                aggression: 0.2,
                civilian_focus: 0.6,
                doctrine_priority: 0.4,
                air_focus: 0.3,
                naval_focus: 0.2,
                production_focus_weights: [0.3, 0.3, 0.1, 0.2, 0.1],
                target_civ_factories: 12,
                target_mil_factories: 15,
                civ_to_mil_ratio: 1.3,
                mil_focus_year: 1939,
                target_div_count_peace: 18,
                target_div_count_war: 40,
                priority_targets: Vec::new(),
                force_attack_against: Vec::new(),
                faction_role: FactionRole::JoinFaction {
                    faction_name: "Allies".into(),
                    by_year: 1938,
                    by_month: 3,
                    by_day: 1,
                },
                faction_target_name: Some("Allies".into()),
                invite_targets: Vec::new(),
                refuse_cession: true,
                defensive_only: true,
                defensive_weekly_boost: (8000, 0.020, 6.0),
                peacetime_weekly_pp: 1.0,
            },
        },
        ProfileEntry {
            tag: "ETH",
            profile: AiProfile {
                aggression: 0.6,
                civilian_focus: 0.3,
                doctrine_priority: 0.3,
                air_focus: 0.1,
                naval_focus: 0.0,
                production_focus_weights: [0.1, 0.5, 0.0, 0.3, 0.1],
                target_civ_factories: 5,
                target_mil_factories: 8,
                civ_to_mil_ratio: 0.8,
                mil_focus_year: 1936,
                target_div_count_peace: 12,
                target_div_count_war: 30,
                priority_targets: Vec::new(),
                force_attack_against: Vec::new(),
                faction_role: FactionRole::Solo,
                faction_target_name: None,
                invite_targets: Vec::new(),
                refuse_cession: true,
                defensive_only: true,
                defensive_weekly_boost: (5000, 0.010, 4.0),
                peacetime_weekly_pp: 0.5,
            },
        },
        ProfileEntry {
            tag: "SPR",
            profile: AiProfile {
                aggression: 0.58,
                civilian_focus: 0.3,
                doctrine_priority: 0.3,
                air_focus: 0.2,
                naval_focus: 0.1,
                production_focus_weights: [0.1, 0.5, 0.0, 0.3, 0.1],
                target_civ_factories: 8,
                target_mil_factories: 12,
                civ_to_mil_ratio: 0.8,
                mil_focus_year: 1936,
                target_div_count_peace: 14,
                target_div_count_war: 28,
                priority_targets: vec!["SPA".into()],
                force_attack_against: vec!["SPA".into()],
                faction_role: FactionRole::Solo,
                faction_target_name: None,
                invite_targets: Vec::new(),
                refuse_cession: true,
                defensive_only: false,
                defensive_weekly_boost: (4500, 0.010, 3.0),
                peacetime_weekly_pp: 0.5,
            },
        },
        ProfileEntry {
            tag: "SPA",
            profile: AiProfile {
                aggression: 0.82,
                civilian_focus: 0.3,
                doctrine_priority: 0.3,
                air_focus: 0.3,
                naval_focus: 0.1,
                production_focus_weights: [0.1, 0.5, 0.0, 0.3, 0.1],
                target_civ_factories: 8,
                target_mil_factories: 12,
                civ_to_mil_ratio: 0.8,
                mil_focus_year: 1936,
                target_div_count_peace: 18,
                target_div_count_war: 36,
                priority_targets: vec!["SPR".into()],
                force_attack_against: vec!["SPR".into()],
                faction_role: FactionRole::Solo,
                faction_target_name: None,
                invite_targets: Vec::new(),
                refuse_cession: true,
                defensive_only: false,
                defensive_weekly_boost: (8000, 0.020, 6.0),
                peacetime_weekly_pp: 0.5,
            },
        },
        ProfileEntry {
            tag: "CNT",
            profile: AiProfile {
                aggression: 0.88,
                civilian_focus: 0.25,
                doctrine_priority: 0.25,
                air_focus: 0.1,
                naval_focus: 0.0,
                production_focus_weights: [0.1, 0.55, 0.0, 0.25, 0.1],
                target_civ_factories: 4,
                target_mil_factories: 8,
                civ_to_mil_ratio: 0.6,
                mil_focus_year: 1936,
                target_div_count_peace: 10,
                target_div_count_war: 28,
                priority_targets: vec!["SPR".into()],
                force_attack_against: vec!["SPR".into()],
                faction_role: FactionRole::Solo,
                faction_target_name: None,
                invite_targets: Vec::new(),
                refuse_cession: true,
                defensive_only: false,
                defensive_weekly_boost: (7500, 0.020, 6.0),
                peacetime_weekly_pp: 0.5,
            },
        },
    ]
}

pub struct ProfileRegistry {
    profiles: HashMap<String, AiProfile>,
}

impl ProfileRegistry {
    pub fn new() -> Self {
        let mut profiles = HashMap::new();
        for entry in hardcoded_profiles() {
            profiles.insert(entry.tag.to_owned(), entry.profile);
        }
        Self { profiles }
    }

    /// 全局共享的 ProfileRegistry，首次访问时延迟初始化。
    ///
    /// 用于热路径（每帧 / 每个国家逐次查表）场景，避免反复构建 HashMap。
    /// 配置静态、不可变，因此可以全程共享。
    pub fn global() -> &'static ProfileRegistry {
        static INSTANCE: OnceLock<ProfileRegistry> = OnceLock::new();
        INSTANCE.get_or_init(ProfileRegistry::new)
    }

    pub fn get(&self, tag: &str) -> AiProfile {
        self.profiles
            .get(tag)
            .cloned()
            .unwrap_or_else(|| AiProfile::from_seed(tag))
    }
}

impl AiProfile {
    pub fn for_country(tag: &str) -> Self {
        ProfileRegistry::global().get(tag)
    }

    pub fn for_country_with_registry(tag: &str, registry: &ProfileRegistry) -> AiProfile {
        registry.get(tag)
    }

    fn from_seed(tag: &str) -> Self {
        let seed = Scorer::new(0).hash_u64(tag);
        let s = Scorer::new(seed);
        Self {
            aggression: 0.3 + (s.hash_u64("aggr") % 1000) as f32 / 1000.0 * 0.5,
            civilian_focus: 0.3 + (s.hash_u64("civ") % 1000) as f32 / 1000.0 * 0.5,
            doctrine_priority: 0.3 + (s.hash_u64("doct") % 1000) as f32 / 1000.0 * 0.5,
            air_focus: 0.2 + (s.hash_u64("air") % 1000) as f32 / 1000.0 * 0.4,
            naval_focus: 0.2 + (s.hash_u64("nav") % 1000) as f32 / 1000.0 * 0.4,
            production_focus_weights: [0.3, 0.4, 0.1, 0.1, 0.1],
            ..Self::default()
        }
    }
}
